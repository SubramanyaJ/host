/**
 * nat_traversal/tcp_connect.rs
 * 
 * TCP simultaneous open implementation
 */

use anyhow::{Context, Result, anyhow};
use std::net::{SocketAddr, TcpStream, TcpListener};
use std::time::{Duration, Instant};
use std::io::ErrorKind;

/// TCP connection error
#[derive(Debug)]
pub enum TcpConnectError {
    Timeout,
    BindFailed(String),
    ConnectFailed(String),
}

impl std::fmt::Display for TcpConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TcpConnectError::Timeout => write!(f, "Connection timeout"),
            TcpConnectError::BindFailed(e) => write!(f, "Bind failed: {}", e),
            TcpConnectError::ConnectFailed(e) => write!(f, "Connect failed: {}", e),
        }
    }
}

impl std::error::Error for TcpConnectError {}

/// Perform TCP simultaneous open
/// 
/// This is a complex technique where both peers:
/// 1. Bind to a local port
/// 2. Attempt to connect to each other simultaneously
/// 3. NATs will typically allow the SYN packets through because of the prior UDP hole punching
pub async fn tcp_simultaneous_open(
    local_port: u16,
    peer_addr: SocketAddr,
    timeout: Duration,
) -> Result<TcpStream> {
    println!("Starting TCP simultaneous open...");
    println!("  Local port: {}", local_port);
    println!("  Peer address: {}", peer_addr);

    let start = Instant::now();

    // Strategy 1: Try direct connection first (might work if peer connected first)
    match try_connect(peer_addr, Duration::from_millis(500)) {
        Ok(stream) => {
            println!("Direct TCP connection succeeded!");
            return Ok(stream);
        }
        Err(_) => {
            println!("Direct connection failed, trying simultaneous open...");
        }
    }

    // Strategy 2: Simultaneous open
    // Bind to specific local port
    let local_addr = SocketAddr::from(([0, 0, 0, 0], local_port));
    
    // Set SO_REUSEADDR to allow rebinding
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )?;
    
    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;
    
    socket.bind(&local_addr.into())?;
    socket.set_nonblocking(true)?;

    // Initiate connection attempt
    match socket.connect(&peer_addr.into()) {
        Ok(_) => {
            // Connected immediately (rare)
            let std_socket: std::net::TcpStream = socket.into();
            std_socket.set_nonblocking(false)?;
            println!("TCP connection established immediately!");
            return Ok(std_socket);
        }
        Err(e) if e.kind() == ErrorKind::WouldBlock => {
            // Connection in progress, this is expected
        }
        Err(e) => {
            return Err(anyhow!("Failed to initiate connection: {}", e));
        }
    }

    // Convert to std socket
    let std_socket: std::net::TcpStream = socket.into();

    // Wait for connection to complete
    loop {
        if start.elapsed() > timeout {
            return Err(anyhow!("TCP simultaneous open timeout"));
        }

        // Check if connection is established by checking peer_addr
        match std_socket.peer_addr() {
            Ok(_) => {
                // Already connected!
                println!("TCP simultaneous open succeeded!");
                std_socket.set_nonblocking(false)?;
                return Ok(std_socket);
            }
            Err(_) => {
                // Not connected yet, wait and retry
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

/// Try a simple TCP connection with timeout
fn try_connect(addr: SocketAddr, timeout: Duration) -> Result<TcpStream> {
    let stream = TcpStream::connect_timeout(&addr, timeout)
        .context("Connection failed")?;
    Ok(stream)
}

/// Alternative approach: Listen and connect simultaneously
pub async fn tcp_listen_and_connect(
    local_port: u16,
    peer_addr: SocketAddr,
    timeout: Duration,
) -> Result<TcpStream> {
    let start = Instant::now();
    
    // Start listening
    let listener = TcpListener::bind(format!("0.0.0.0:{}", local_port))
        .context("Failed to bind listener")?;
    listener.set_nonblocking(true)?;

    // Try both listening and connecting
    loop {
        if start.elapsed() > timeout {
            return Err(anyhow!("TCP connection timeout"));
        }

        // Try to accept incoming connection
        match listener.accept() {
            Ok((stream, addr)) => {
                println!("Accepted TCP connection from {}", addr);
                stream.set_nonblocking(false)?;
                return Ok(stream);
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                // No incoming connection yet
            }
            Err(e) => {
                println!("Accept error: {}", e);
            }
        }

        // Try to connect outbound
        match TcpStream::connect_timeout(&peer_addr, Duration::from_millis(100)) {
            Ok(stream) => {
                println!("Outbound TCP connection succeeded!");
                return Ok(stream);
            }
            Err(_) => {
                // Connection failed, keep trying
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
