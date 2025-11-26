/**
 * nat_traversal/stun.rs
 * 
 * STUN client for NAT discovery
 */

use anyhow::{Context, Result, anyhow};
use std::net::{SocketAddr, UdpSocket, IpAddr};
use std::time::Duration;

/// STUN message types
const STUN_BINDING_REQUEST: u16 = 0x0001;
const STUN_BINDING_RESPONSE: u16 = 0x0101;

/// STUN magic cookie
const STUN_MAGIC_COOKIE: u32 = 0x2112A442;

/// STUN attribute types
const ATTR_MAPPED_ADDRESS: u16 = 0x0001;
const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;

/// STUN query response
#[derive(Debug, Clone)]
pub struct StunResponse {
    pub external_ip: IpAddr,
    pub external_port: u16,
}

/// STUN client
pub struct StunClient {
    socket: UdpSocket,
    server_addr: SocketAddr,
}

impl StunClient {
    /// Create a new STUN client
    pub fn new(server_addr: &SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .context("Failed to bind UDP socket")?;
        
        socket.set_read_timeout(Some(Duration::from_secs(5)))
            .context("Failed to set read timeout")?;

        Ok(Self {
            socket,
            server_addr: *server_addr,
        })
    }

    /// Query STUN server for external address
    pub async fn query(&self) -> Result<StunResponse> {
        let transaction_id: [u8; 12] = rand::random();
        let request = self.build_binding_request(&transaction_id);

        // Send STUN binding request
        self.socket
            .send_to(&request, self.server_addr)
            .context("Failed to send STUN request")?;

        // Receive response
        let mut buffer = vec![0u8; 1024];
        let (len, _) = self.socket
            .recv_from(&mut buffer)
            .context("Failed to receive STUN response")?;

        self.parse_binding_response(&buffer[..len], &transaction_id)
    }

    /// Build a STUN binding request
    fn build_binding_request(&self, transaction_id: &[u8; 12]) -> Vec<u8> {
        let mut request = Vec::new();

        // Message type (16 bits)
        request.extend_from_slice(&STUN_BINDING_REQUEST.to_be_bytes());

        // Message length (16 bits) - 0 for now, no attributes
        request.extend_from_slice(&0u16.to_be_bytes());

        // Magic cookie (32 bits)
        request.extend_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());

        // Transaction ID (96 bits)
        request.extend_from_slice(transaction_id);

        request
    }

    /// Parse STUN binding response
    fn parse_binding_response(&self, data: &[u8], expected_transaction_id: &[u8; 12]) -> Result<StunResponse> {
        if data.len() < 20 {
            return Err(anyhow!("STUN response too short"));
        }

        // Check message type
        let msg_type = u16::from_be_bytes([data[0], data[1]]);
        if msg_type != STUN_BINDING_RESPONSE {
            return Err(anyhow!("Invalid STUN response type: 0x{:04x}", msg_type));
        }

        // Check magic cookie
        let magic = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        if magic != STUN_MAGIC_COOKIE {
            return Err(anyhow!("Invalid magic cookie"));
        }

        // Check transaction ID
        if &data[8..20] != expected_transaction_id {
            return Err(anyhow!("Transaction ID mismatch"));
        }

        // Parse message length
        let msg_len = u16::from_be_bytes([data[2], data[3]]) as usize;
        if data.len() < 20 + msg_len {
            return Err(anyhow!("STUN response truncated"));
        }

        // Parse attributes
        let mut offset = 20;
        while offset < 20 + msg_len {
            if offset + 4 > data.len() {
                break;
            }

            let attr_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let attr_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
            offset += 4;

            if offset + attr_len > data.len() {
                break;
            }

            let attr_data = &data[offset..offset + attr_len];

            if attr_type == ATTR_XOR_MAPPED_ADDRESS {
                return self.parse_xor_mapped_address(attr_data, expected_transaction_id);
            } else if attr_type == ATTR_MAPPED_ADDRESS {
                return self.parse_mapped_address(attr_data);
            }

            // Move to next attribute (attributes are padded to 4-byte boundaries)
            offset += (attr_len + 3) & !3;
        }

        Err(anyhow!("No address attribute found in STUN response"))
    }

    /// Parse XOR-MAPPED-ADDRESS attribute
    fn parse_xor_mapped_address(&self, data: &[u8], transaction_id: &[u8; 12]) -> Result<StunResponse> {
        if data.len() < 8 {
            return Err(anyhow!("XOR-MAPPED-ADDRESS too short"));
        }

        let family = data[1];
        let xor_port = u16::from_be_bytes([data[2], data[3]]);
        let port = xor_port ^ (STUN_MAGIC_COOKIE >> 16) as u16;

        let ip = match family {
            0x01 => {
                // IPv4
                if data.len() < 8 {
                    return Err(anyhow!("Invalid IPv4 address length"));
                }
                let xor_addr = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                let addr = xor_addr ^ STUN_MAGIC_COOKIE;
                IpAddr::from(addr.to_be_bytes())
            }
            0x02 => {
                // IPv6
                if data.len() < 20 {
                    return Err(anyhow!("Invalid IPv6 address length"));
                }
                let mut addr_bytes = [0u8; 16];
                addr_bytes.copy_from_slice(&data[4..20]);

                // XOR with magic cookie + transaction ID
                let mut xor_key = [0u8; 16];
                xor_key[0..4].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
                xor_key[4..16].copy_from_slice(transaction_id);

                for i in 0..16 {
                    addr_bytes[i] ^= xor_key[i];
                }

                IpAddr::from(addr_bytes)
            }
            _ => {
                return Err(anyhow!("Unknown address family: {}", family));
            }
        };

        Ok(StunResponse {
            external_ip: ip,
            external_port: port,
        })
    }

    /// Parse MAPPED-ADDRESS attribute (fallback)
    fn parse_mapped_address(&self, data: &[u8]) -> Result<StunResponse> {
        if data.len() < 8 {
            return Err(anyhow!("MAPPED-ADDRESS too short"));
        }

        let family = data[1];
        let port = u16::from_be_bytes([data[2], data[3]]);

        let ip = match family {
            0x01 => {
                // IPv4
                if data.len() < 8 {
                    return Err(anyhow!("Invalid IPv4 address length"));
                }
                IpAddr::from([data[4], data[5], data[6], data[7]])
            }
            0x02 => {
                // IPv6
                if data.len() < 20 {
                    return Err(anyhow!("Invalid IPv6 address length"));
                }
                let mut addr_bytes = [0u8; 16];
                addr_bytes.copy_from_slice(&data[4..20]);
                IpAddr::from(addr_bytes)
            }
            _ => {
                return Err(anyhow!("Unknown address family: {}", family));
            }
        };

        Ok(StunResponse {
            external_ip: ip,
            external_port: port,
        })
    }

    /// Get local socket address
    pub fn local_addr(&self) -> SocketAddr {
        self.socket.local_addr().expect("Failed to get local address")
    }

    /// Convert into UDP socket for hole punching
    pub fn into_socket(self) -> UdpSocket {
        self.socket
    }
}
