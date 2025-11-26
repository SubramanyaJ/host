/**
 * nat_traversal/types.rs
 * 
 * Core types for NAT traversal
 */

use std::net::SocketAddr;
use ed25519_dalek::SigningKey;

/// Peer connection information
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub fingerprint: String,
    pub external_addr: SocketAddr,
    pub local_addr: SocketAddr,
    pub nonce: u64,
}

/// NAT traversal configuration
#[derive(Clone)]
pub struct NatTraversalConfig {
    /// Signalling server URL (wss://host:port)
    pub signalling_url: String,
    
    /// STUN server address (host:port)
    pub stun_server_addr: SocketAddr,
    
    /// Local identity fingerprint
    pub local_fingerprint: String,
    
    /// Ed25519 signing key for UDP probes
    pub signing_key: SigningKey,
    
    /// Local TCP port to bind (0 for random)
    pub tcp_port: u16,
}

/// Connection state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Idle,
    ConnectingSignalling,
    Registering,
    StunDiscovery,
    SendingOffer,
    WaitingForOffer,
    UdpHolePunching,
    TcpConnecting,
    Connected,
    Failed(String),
}
