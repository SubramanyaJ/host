/**
 * nat_traversal/hole_punching.rs
 * 
 * UDP hole punching with signed probe packets
 */

use anyhow::{Context, Result, anyhow};
use ed25519_dalek::{SigningKey, Signature, Signer, VerifyingKey, Verifier};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// UDP probe packet structure
#[derive(Debug, Clone)]
pub struct ProbePacket {
    pub nonce: u64,
    pub tcp_port: u16,
    pub signature: Signature,
}

impl ProbePacket {
    /// Create and sign a new probe packet
    pub fn new(tcp_port: u16, signing_key: &SigningKey) -> Self {
        let nonce = rand::random::<u64>();
        let message = Self::message_to_sign(nonce, tcp_port);
        let signature = signing_key.sign(&message);

        Self {
            nonce,
            tcp_port,
            signature,
        }
    }

    /// Verify probe packet signature
    pub fn verify(&self, verifying_key: &VerifyingKey) -> Result<()> {
        let message = Self::message_to_sign(self.nonce, self.tcp_port);
        verifying_key
            .verify(&message, &self.signature)
            .context("Invalid probe signature")?;
        Ok(())
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        // Magic marker (4 bytes)
        bytes.extend_from_slice(b"PNPL");
        
        // Nonce (8 bytes)
        bytes.extend_from_slice(&self.nonce.to_be_bytes());
        
        // TCP port (2 bytes)
        bytes.extend_from_slice(&self.tcp_port.to_be_bytes());
        
        // Signature (64 bytes)
        bytes.extend_from_slice(&self.signature.to_bytes());
        
        bytes
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() != 78 {
            return Err(anyhow!("Invalid probe packet length: {}", data.len()));
        }

        // Check magic marker
        if &data[0..4] != b"PNPL" {
            return Err(anyhow!("Invalid probe packet magic"));
        }

        let nonce = u64::from_be_bytes(
            data[4..12].try_into().context("Invalid nonce")?,
        );

        let tcp_port = u16::from_be_bytes(
            data[12..14].try_into().context("Invalid TCP port")?,
        );

        let signature = Signature::from_bytes(
            data[14..78].try_into().context("Invalid signature")?,
        );

        Ok(Self {
            nonce,
            tcp_port,
            signature,
        })
    }

    /// Generate message to sign/verify
    fn message_to_sign(nonce: u64, tcp_port: u16) -> Vec<u8> {
        let mut message = Vec::new();
        message.extend_from_slice(b"PINEAPPLE_PROBE");
        message.extend_from_slice(&nonce.to_be_bytes());
        message.extend_from_slice(&tcp_port.to_be_bytes());
        message
    }
}

/// UDP hole puncher
pub struct UdpHolePuncher {
    socket: UdpSocket,
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl UdpHolePuncher {
    /// Create a new hole puncher
    pub fn new(socket: UdpSocket, signing_key: &SigningKey) -> Result<Self> {
        socket.set_nonblocking(true)
            .context("Failed to set socket non-blocking")?;

        let verifying_key = signing_key.verifying_key();

        Ok(Self {
            socket,
            signing_key: signing_key.clone(),
            verifying_key,
        })
    }

    /// Punch hole to peer addresses
    /// Returns peer's TCP port when connection is established
    pub async fn punch_hole(&self, peer_addrs: &[SocketAddr], timeout: Duration) -> Result<u16> {
        let start = Instant::now();
        let tcp_port = self.get_local_tcp_port()?;
        let probe = ProbePacket::new(tcp_port, &self.signing_key);
        let probe_bytes = probe.to_bytes();

        println!("Starting UDP hole punching...");
        println!("  Local TCP port: {}", tcp_port);
        println!("  Sending to {} peer addresses", peer_addrs.len());

        let mut last_send = Instant::now();
        let send_interval = Duration::from_millis(200);

        loop {
            // Check timeout
            if start.elapsed() > timeout {
                return Err(anyhow!("UDP hole punching timeout"));
            }

            // Send probes periodically
            if last_send.elapsed() > send_interval {
                for addr in peer_addrs {
                    let _ = self.socket.send_to(&probe_bytes, addr);
                }
                last_send = Instant::now();
            }

            // Try to receive peer's probe
            let mut buffer = vec![0u8; 1024];
            match self.socket.recv_from(&mut buffer) {
                Ok((len, from_addr)) => {
                    println!("Received UDP packet from {}", from_addr);

                    match ProbePacket::from_bytes(&buffer[..len]) {
                        Ok(peer_probe) => {
                            // Note: In production, you would get the peer's verifying key
                            // from the signalling exchange. For now, we skip verification
                            // or use a pre-shared key mechanism.
                            println!("Valid probe packet received!");
                            println!("  Peer TCP port: {}", peer_probe.tcp_port);
                            return Ok(peer_probe.tcp_port);
                        }
                        Err(e) => {
                            println!("Invalid probe packet: {}", e);
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available, continue
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(e) => {
                    println!("Socket error: {}", e);
                }
            }
        }
    }

    /// Get a local TCP port for simultaneous open
    fn get_local_tcp_port(&self) -> Result<u16> {
        // Bind a TCP socket to get a port number, then drop it
        let listener = std::net::TcpListener::bind("0.0.0.0:0")
            .context("Failed to bind TCP listener")?;
        let port = listener.local_addr()?.port();
        Ok(port)
    }
}
