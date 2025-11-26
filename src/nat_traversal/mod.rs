/**
 * nat_traversal/mod.rs
 * 
 * NAT traversal module implementing:
 * - TLS WebSocket signalling client
 * - STUN client
 * - UDP hole punching
 * - TCP simultaneous open
 */

mod signalling;
mod stun;
mod hole_punching;
mod tcp_connect;
mod types;

pub use signalling::{SignallingClient, SignallingMessage, SignallingError};
pub use stun::{StunClient, StunResponse};
pub use hole_punching::{UdpHolePuncher, ProbePacket};
pub use tcp_connect::{tcp_simultaneous_open, TcpConnectError};
pub use types::{PeerInfo, NatTraversalConfig, ConnectionState};

use anyhow::{Context, Result};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// Complete NAT traversal state machine
pub struct NatTraversal {
    config: NatTraversalConfig,
    signalling: Option<SignallingClient>,
    state: ConnectionState,
}

impl NatTraversal {
    /// Create a new NAT traversal manager
    pub fn new(config: NatTraversalConfig) -> Self {
        Self {
            config,
            signalling: None,
            state: ConnectionState::Idle,
        }
    }

    /// Execute the complete NAT traversal pipeline
    /// Returns a connected TCP stream ready for pineapple session
    pub async fn connect(&mut self, peer_fingerprint: &str) -> Result<TcpStream> {
        // Step 1: Connect to signalling server
        self.state = ConnectionState::ConnectingSignalling;
        let mut signalling = SignallingClient::connect(&self.config.signalling_url)
            .await
            .context("Failed to connect to signalling server")?;

        // Step 2: Register our identity
        self.state = ConnectionState::Registering;
        signalling
            .register(&self.config.local_fingerprint)
            .await
            .context("Failed to register with signalling server")?;

        // Step 3: STUN discovery
        self.state = ConnectionState::StunDiscovery;
        let stun_client = StunClient::new(&self.config.stun_server_addr)?;
        let stun_response = stun_client
            .query()
            .await
            .context("STUN query failed")?;

        let external_addr = SocketAddr::new(stun_response.external_ip, stun_response.external_port);
        let local_addr = stun_client.local_addr();

        println!("NAT discovery complete:");
        println!("  External: {}", external_addr);
        println!("  Local: {}", local_addr);

        // Step 4: Send offer
        self.state = ConnectionState::SendingOffer;
        let peer_info = signalling
            .send_offer(peer_fingerprint, external_addr, local_addr)
            .await
            .context("Failed to send offer")?;

        println!("Received peer info:");
        println!("  External: {}", peer_info.external_addr);
        println!("  Local: {}", peer_info.local_addr);

        // Step 5: UDP hole punching
        self.state = ConnectionState::UdpHolePunching;
        let hole_puncher = UdpHolePuncher::new(
            stun_client.into_socket(),
            &self.config.signing_key,
        )?;

        let peer_addrs = vec![peer_info.external_addr, peer_info.local_addr];
        let tcp_port = hole_puncher
            .punch_hole(&peer_addrs, Duration::from_secs(30))
            .await
            .context("UDP hole punching failed")?;

        println!("UDP hole punched! Peer TCP port: {}", tcp_port);

        // Step 6: TCP simultaneous open
        self.state = ConnectionState::TcpConnecting;
        let local_tcp_port = self.config.tcp_port;
        let peer_tcp_addr = SocketAddr::new(peer_info.external_addr.ip(), tcp_port);

        let tcp_stream = tcp_simultaneous_open(local_tcp_port, peer_tcp_addr, Duration::from_secs(10))
            .await
            .context("TCP simultaneous open failed")?;

        println!("TCP connection established!");

        // Step 7: Cleanup
        self.state = ConnectionState::Connected;
        signalling.close().await?;
        self.signalling = None;

        Ok(tcp_stream)
    }

    /// Get current connection state
    pub fn state(&self) -> &ConnectionState {
        &self.state
    }
}
