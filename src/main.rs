use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal,
};
use pineapple::{messages, network, pqxdh, Session};
use pineapple::nat_traversal::{NatTraversal, NatTraversalConfig};
use ed25519_dalek::SigningKey;
use std::{
    env,
    io::{self, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        std::process::exit(1);
    }

    match args[1].as_str() {
        "nat" => {
            if args.len() < 3 {
                eprintln!("Error: Missing peer fingerprint");
                eprintln!();
                eprintln!("Usage: {} nat <peer_fingerprint>", args[0]);
                eprintln!();
                eprintln!("Example:");
                eprintln!("  Peer 1: {} nat bob", args[0]);
                eprintln!("  Peer 2: {} nat alice", args[0]);
                eprintln!();
                eprintln!("The peer fingerprint is just an identifier (like a username).");
                eprintln!("You do NOT need to know the peer's IP address!");
                eprintln!("The signalling server will automatically relay connection info.");
                std::process::exit(1);
            }
            let peer_fingerprint = &args[2];
            run_nat_traversal(peer_fingerprint)?
        }
        "listen" => {
            if args.len() < 3 {
                eprintln!("Usage: {} listen <port>", args[0]);
                eprintln!();
                eprintln!("Note: This mode requires direct network access (no NAT).");
                eprintln!("      For connections behind NAT, use 'nat' mode instead.");
                std::process::exit(1);
            }
            let port = &args[2];
            run_alice(port)?
        }
        "connect" => {
            if args.len() < 3 {
                eprintln!("Usage: {} connect <ip:port>", args[0]);
                eprintln!();
                eprintln!("Note: This mode requires direct network access (no NAT).");
                eprintln!("      For connections behind NAT, use 'nat' mode instead.");
                std::process::exit(1);
            }
            let address = &args[2];
            run_bob(address)?
        }
        _ => {
            eprintln!("Error: Invalid mode '{}'", args[1]);
            eprintln!();
            print_usage(&args[0]);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_usage(program_name: &str) {
    eprintln!("pineapple - Quantum-safe P2P messaging with NAT traversal");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  {} nat <peer_fingerprint>    # NAT traversal mode (RECOMMENDED)", program_name);
    eprintln!("  {} listen <port>              # Direct listen mode (no NAT)", program_name);
    eprintln!("  {} connect <ip:port>          # Direct connect mode (no NAT)", program_name);
    eprintln!();
    eprintln!("NAT TRAVERSAL MODE (Recommended):");
    eprintln!("  This mode works behind NAT/firewalls using signalling + STUN servers.");
    eprintln!("  You ONLY need the peer's fingerprint (identifier), NO IP addresses!");
    eprintln!();
    eprintln!("  Required environment variables:");
    eprintln!("    SIGNALLING_URL      WebSocket signalling server");
    eprintln!("                        Example: wss://your-server.com:8443");
    eprintln!();
    eprintln!("    STUN_SERVER         STUN server for NAT discovery");
    eprintln!("                        Example: your-server.com:3478");
    eprintln!();
    eprintln!("    LOCAL_FINGERPRINT   Your identity (like a username)");
    eprintln!("                        Example: alice");
    eprintln!("                        (Optional: defaults to random ID)");
    eprintln!();
    eprintln!("  Example workflow:");
    eprintln!("    # Peer 1 (Alice)");
    eprintln!("    export SIGNALLING_URL=\"wss://example.com:8443\"");
    eprintln!("    export STUN_SERVER=\"example.com:3478\"");
    eprintln!("    export LOCAL_FINGERPRINT=\"alice\"");
    eprintln!("    {} nat bob", program_name);
    eprintln!();
    eprintln!("    # Peer 2 (Bob)");
    eprintln!("    export SIGNALLING_URL=\"wss://example.com:8443\"");
    eprintln!("    export STUN_SERVER=\"example.com:3478\"");
    eprintln!("    export LOCAL_FINGERPRINT=\"bob\"");
    eprintln!("    {} nat alice", program_name);
    eprintln!();
    eprintln!("DIRECT MODES (Legacy - No NAT support):");
    eprintln!("  These modes require both peers on the same network or with port forwarding.");
    eprintln!("  Use 'nat' mode for real-world scenarios behind NAT/firewalls.");
    eprintln!();
    eprintln!("For more information, see README.md");
}

/// Run NAT traversal mode - connects through signalling + STUN servers
fn run_nat_traversal(peer_fingerprint: &str) -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         pineapple - NAT Traversal Mode                  ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // Get configuration from environment variables
    let signalling_url = env::var("SIGNALLING_URL")
        .context("SIGNALLING_URL environment variable not set. Example: wss://your-server.com:8443")?;
    
    let stun_server = env::var("STUN_SERVER")
        .context("STUN_SERVER environment variable not set. Example: your-server.com:3478")?;
    
    let local_fingerprint = env::var("LOCAL_FINGERPRINT")
        .unwrap_or_else(|_| {
            let random_id = format!("peer_{}", rand::random::<u32>());
            println!("⚠️  LOCAL_FINGERPRINT not set, using random ID: {}", random_id);
            println!();
            random_id
        });
    
    println!("Configuration:");
    println!("  Signalling Server : {}", signalling_url);
    println!("  STUN Server       : {}", stun_server);
    println!("  My Fingerprint    : {}", local_fingerprint);
    println!("  Target Peer       : {}", peer_fingerprint);
    println!();
    
    if local_fingerprint == peer_fingerprint {
        eprintln!("❌ Error: Cannot connect to yourself!");
        eprintln!("   Your LOCAL_FINGERPRINT cannot be the same as the target peer.");
        std::process::exit(1);
    }
    
    // Parse STUN server address
    let stun_addr: std::net::SocketAddr = stun_server
        .parse()
        .context("Invalid STUN server address. Expected format: host:port")?;
    
    // Generate signing key for UDP probes
    let signing_key = SigningKey::from_bytes(&rand::random::<[u8; 32]>());
    
    // Configure NAT traversal
    let config = NatTraversalConfig {
        signalling_url,
        stun_server_addr: stun_addr,
        local_fingerprint: local_fingerprint.clone(),
        signing_key,
        tcp_port: 0, // Random port
    };
    
    // Create NAT traversal instance
    let mut nat = NatTraversal::new(config);
    
    println!("🔍 Starting NAT traversal pipeline...");
    println!("   This may take 5-30 seconds depending on network conditions.");
    println!();
    
    // Execute NAT traversal
    let runtime = tokio::runtime::Runtime::new()?;
    let stream = runtime.block_on(async {
        nat.connect(peer_fingerprint).await
    })?;
    
    println!();
    println!("✅ NAT traversal complete!");
    println!("✅ TCP connection established directly with peer!");
    println!("🔒 Starting encrypted session...");
    println!();
    
    // Now proceed with PQXDH handshake and session
    // The role (initiator vs responder) is determined by fingerprint comparison
    let is_initiator = local_fingerprint < peer_fingerprint.to_string();
    
    if is_initiator {
        run_session_initiator(stream)?;
    } else {
        run_session_responder(stream)?;
    }
    
    Ok(())
}

/// Run as session initiator (Alice)
fn run_session_initiator(mut stream: TcpStream) -> Result<()> {
    println!("📋 Role: Initiator");
    println!("🔐 Performing PQXDH handshake...");
    
    let alice = pqxdh::User::new();
    send_public_keys(&mut stream, &alice)?;
    
    let mut bob = receive_public_keys(&mut stream)?;
    
    let (session, init_message) = Session::new_initiator(&alice, &mut bob)?;
    
    network::send_message(
        &mut stream,
        &network::serialize_pqxdh_init_message(&init_message),
    )?;
    
    println!("✅ Session established!");
    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Type your message and press Enter to send.");
    println!("  To send a file: !path/to/file.txt");
    println!("  Press Ctrl+L to clear screen.");
    println!("  Press Ctrl+C to exit.");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    
    chat_loop(session, stream)?;
    
    Ok(())
}

/// Run as session responder (Bob)
fn run_session_responder(mut stream: TcpStream) -> Result<()> {
    println!("📋 Role: Responder");
    println!("🔐 Performing PQXDH handshake...");
    
    let mut bob = pqxdh::User::new();
    
    let alice = receive_public_keys(&mut stream)?;
    send_public_keys(&mut stream, &bob)?;
    
    let init_message_data = network::receive_message(&mut stream)?;
    let init_message = network::deserialize_pqxdh_init_message(&init_message_data)?;
    
    let session = Session::new_responder(&mut bob, &init_message)?;
    
    println!("✅ Session established!");
    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  Type your message and press Enter to send.");
    println!("  To send a file: !path/to/file.txt");
    println!("  Press Ctrl+L to clear screen.");
    println!("  Press Ctrl+C to exit.");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    
    chat_loop(session, stream)?;
    
    Ok(())
}

/// Legacy direct listen mode (Alice)
fn run_alice(port: &str) -> Result<()> {
    println!("pineapple - Direct Listen Mode");
    println!("⚠️  Warning: This mode does NOT work behind NAT/firewalls!");
    println!();
    println!("Waiting for connection on port {}...", port);

    let listener = std::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .context("Failed to bind to port")?;

    let (mut stream, addr) = listener
        .accept()
        .context("Failed to accept connection")?;

    println!("Incoming connection from {}", addr);
    println!("Accept? (yes/no)");

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("yes") {
        println!("Connection rejected.");
        return Ok(());
    }

    println!("Connection accepted!");
    println!("Performing handshake...");

    let alice = pqxdh::User::new();
    send_public_keys(&mut stream, &alice)?;

    let mut bob = receive_public_keys(&mut stream)?;

    let (session, init_message) = Session::new_initiator(&alice, &mut bob)?;

    network::send_message(
        &mut stream,
        &network::serialize_pqxdh_init_message(&init_message),
    )?;

    println!("Session established!");
    println!("Type your message and press Enter.");
    println!("To send a file, type !path/to/file.txt");
    println!("Press Ctrl+L to clear screen. Press Ctrl+C to exit.");

    chat_loop(session, stream)?;

    Ok(())
}

/// Legacy direct connect mode (Bob)
fn run_bob(address: &str) -> Result<()> {
    println!("pineapple - Direct Connect Mode");
    println!("⚠️  Warning: This mode does NOT work behind NAT/firewalls!");
    println!();
    println!("Connecting to {}...", address);

    let mut stream = TcpStream::connect(address)
        .context("Failed to connect to peer")?;

    println!("Connected!");
    println!("Performing handshake...");

    let mut bob = pqxdh::User::new();

    let alice = receive_public_keys(&mut stream)?;
    send_public_keys(&mut stream, &bob)?;

    let init_message_data = network::receive_message(&mut stream)?;
    let init_message = network::deserialize_pqxdh_init_message(&init_message_data)?;

    let session = Session::new_responder(&mut bob, &init_message)?;

    println!("Session established!");
    println!("Type your message and press Enter.");
    println!("To send a file, type !path/to/file.txt");
    println!("Press Ctrl+L to clear screen. Press Ctrl+C to exit.");

    chat_loop(session, stream)?;

    Ok(())
}

fn send_public_keys(stream: &mut TcpStream, user: &pqxdh::User) -> Result<()> {
    let bundle = network::serialize_prekey_bundle(user);
    network::send_message(stream, &bundle)?;
    Ok(())
}

fn receive_public_keys(stream: &mut TcpStream) -> Result<pqxdh::User> {
    let bundle_data = network::receive_message(stream)?;
    let user = network::deserialize_prekey_bundle(&bundle_data)?;
    Ok(user)
}

fn chat_loop(session: Session, mut stream: TcpStream) -> Result<()> {
    let stream_clone = stream.try_clone()?;
    let session = Arc::new(Mutex::new(session));
    let session_clone = Arc::clone(&session);
    let input_buffer = Arc::new(Mutex::new(String::new()));
    let input_buffer_clone = Arc::clone(&input_buffer);
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = Arc::clone(&running);

    terminal::enable_raw_mode()?;

    let receive_handle = thread::spawn(move || {
        let mut stream = stream_clone;

        loop {
            if !running_clone.load(Ordering::SeqCst) {
                break;
            }

            match network::receive_message(&mut stream) {
                Ok(msg_data) => {
                    if msg_data == b"\x1B[2J\x1B[H" {
                        print!("\x1B[2J\x1B[H");
                        let buf = input_buffer_clone.lock().unwrap();
                        print!("You: {}", *buf);
                        io::stdout().flush().unwrap();
                        continue;
                    }

                    match network::deserialize_ratchet_message(&msg_data) {
                        Ok(msg) => {
                            let mut sess = session_clone.lock().unwrap();

                            match sess.receive(msg) {
                                Ok(plaintext_bytes) => {
                                    match messages::deserialize_message(&plaintext_bytes) {
                                        Ok(messages::MessageType::Text(text)) => {
                                            let buf = input_buffer_clone.lock().unwrap();
                                            print!("\r\x1B[K");
                                            println!("Peer: {}", text);
                                            print!("You: {}", *buf);
                                            io::stdout().flush().unwrap();
                                        }
                                        Ok(messages::MessageType::File { filename, data }) => {
                                            let save_path = format!("received_{}", filename);
                                            let buf = input_buffer_clone.lock().unwrap();
                                            print!("\r\x1B[K");

                                            match std::fs::write(&save_path, data) {
                                                Ok(_) => {
                                                    println!(
                                                        "Received file - {} -> {}",
                                                        filename,
                                                        save_path,
                                                    );
                                                }
                                                Err(e) => {
                                                    eprintln!("Failed to save file: {}", e);
                                                }
                                            }

                                            print!("You: {}", *buf);
                                            io::stdout().flush().unwrap();
                                        }
                                        Err(e) => {
                                            let buf = input_buffer_clone.lock().unwrap();
                                            print!("\r\x1B[K");
                                            eprintln!("Failed to parse message: {}", e);
                                            print!("You: {}", *buf);
                                            io::stdout().flush().unwrap();
                                        }
                                    }
                                }
                                Err(e) => {
                                    let buf = input_buffer_clone.lock().unwrap();
                                    print!("\r\x1B[K");
                                    eprintln!("Failed to decrypt message: {}", e);
                                    print!("You: {}", *buf);
                                    io::stdout().flush().unwrap();
                                }
                            }
                        }
                        Err(e) => {
                            let buf = input_buffer_clone.lock().unwrap();
                            print!("\r\x1B[K");
                            eprintln!("Failed to deserialize message: {}", e);
                            print!("You: {}", *buf);
                            io::stdout().flush().unwrap();
                        }
                    }
                }
                Err(_) => {
                    print!("\r\x1B[K");
                    println!("Connection closed by peer.");
                    terminal::disable_raw_mode().unwrap();
                    std::process::exit(0);
                }
            }
        }
    });

    print!("You: ");
    io::stdout().flush()?;

    loop {
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(k) = event::read()? {
                let mut buf = input_buffer.lock().unwrap();

                match (k.code, k.modifiers) {
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        print!("\r\n");
                        running.store(false, Ordering::SeqCst);
                        terminal::disable_raw_mode()?;
                        std::process::exit(0);
                    }
                    (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                        let clear_msg = b"\x1B[2J\x1B[H";
                        if network::send_message(&mut stream, clear_msg).is_ok() {
                            print!("\x1B[2J\x1B[H");
                            buf.clear();
                            print!("You: ");
                            io::stdout().flush()?;
                        }
                    }
                    (KeyCode::Enter, _) => {
                        let line = buf.clone();
                        buf.clear();

                        if !line.trim().is_empty() {
                            match messages::parse_input(&line) {
                                Ok(messages::MessageType::Text(text)) => {
                                    print!("\r\x1B[K");
                                    println!("You: {}", text);

                                    let msg_bytes = messages::serialize_message(
                                        &messages::MessageType::Text(text),
                                    );
                                    let mut sess = session.lock().unwrap();

                                    match sess.send_bytes(&msg_bytes) {
                                        Ok(msg) => {
                                            drop(sess);
                                            let msg_data =
                                                network::serialize_ratchet_message(&msg);

                                            if let Err(e) = network::send_message(
                                                &mut stream,
                                                &msg_data,
                                            ) {
                                                eprintln!("Failed to send message: {}", e);
                                                break Ok(());
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to encrypt message: {}", e);
                                        }
                                    }
                                }
                                Ok(messages::MessageType::File { filename, data }) => {
                                    print!("\r\x1B[K");
                                    println!(
                                        "Sending file: {} ({} bytes)",
                                        filename,
                                        data.len(),
                                    );

                                    let msg_bytes = messages::serialize_message(
                                        &messages::MessageType::File {
                                            filename: filename.clone(),
                                            data,
                                        },
                                    );
                                    let mut sess = session.lock().unwrap();

                                    match sess.send_bytes(&msg_bytes) {
                                        Ok(msg) => {
                                            drop(sess);
                                            let msg_data =
                                                network::serialize_ratchet_message(&msg);

                                            if let Err(e) = network::send_message(
                                                &mut stream,
                                                &msg_data,
                                            ) {
                                                eprintln!("Failed to send file: {}", e);
                                                break Ok(());
                                            }

                                            println!("File sent: {}", filename);
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to encrypt file: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error: {}", e);
                                }
                            }
                        }

                        print!("You: ");
                        io::stdout().flush()?;
                    }
                    (KeyCode::Backspace, _) => {
                        if !buf.is_empty() {
                            buf.pop();
                            print!("\r\x1B[KYou: {}", *buf);
                            io::stdout().flush()?;
                        }
                    }
                    (KeyCode::Char(c), _) => {
                        buf.push(c);
                        print!("{}", c);
                        io::stdout().flush()?;
                    }
                    _ => {}
                }
            }
        }
    }
}
