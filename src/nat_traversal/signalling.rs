/**
 * nat_traversal/signalling.rs
 * 
 * TLS WebSocket signalling client
 */

use anyhow::{Context, Result, anyhow};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream, MaybeTlsStream};
use tokio::net::TcpStream as TokioTcpStream;
use futures_util::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use crate::nat_traversal::types::PeerInfo;

/// Signalling message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignallingMessage {
    Register {
        fingerprint: String,
    },
    RegisterAck {
        success: bool,
        message: String,
    },
    Offer {
        target_fingerprint: String,
        external_ip: String,
        external_port: u16,
        local_ip: String,
        local_port: u16,
        nonce: u64,
        fingerprint: String,
    },
    ForwardOffer {
        from_fingerprint: String,
        external_ip: String,
        external_port: u16,
        local_ip: String,
        local_port: u16,
        nonce: u64,
    },
    OfferResponse {
        success: bool,
        message: Option<String>,
    },
    Keepalive,
    Error {
        message: String,
    },
}

/// Signalling client errors
#[derive(Debug)]
pub enum SignallingError {
    ConnectionFailed(String),
    RegistrationFailed(String),
    SendFailed(String),
    ReceiveFailed(String),
    InvalidMessage(String),
}

impl std::fmt::Display for SignallingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignallingError::ConnectionFailed(e) => write!(f, "Connection failed: {}", e),
            SignallingError::RegistrationFailed(e) => write!(f, "Registration failed: {}", e),
            SignallingError::SendFailed(e) => write!(f, "Send failed: {}", e),
            SignallingError::ReceiveFailed(e) => write!(f, "Receive failed: {}", e),
            SignallingError::InvalidMessage(e) => write!(f, "Invalid message: {}", e),
        }
    }
}

impl std::error::Error for SignallingError {}

/// WebSocket signalling client
pub struct SignallingClient {
    ws_stream: WebSocketStream<MaybeTlsStream<TokioTcpStream>>,
    local_fingerprint: Option<String>,
}

impl SignallingClient {
    /// Connect to signalling server
    pub async fn connect(url: &str) -> Result<Self> {
        let (ws_stream, _) = connect_async(url)
            .await
            .context("Failed to connect to WebSocket")?;

        Ok(Self {
            ws_stream,
            local_fingerprint: None,
        })
    }

    /// Register with the signalling server
    pub async fn register(&mut self, fingerprint: &str) -> Result<()> {
        let msg = SignallingMessage::Register {
            fingerprint: fingerprint.to_string(),
        };

        self.send_message(&msg).await?;

        // Wait for acknowledgment
        let response = self.receive_message().await?;
        match response {
            SignallingMessage::RegisterAck { success, message } => {
                if success {
                    self.local_fingerprint = Some(fingerprint.to_string());
                    Ok(())
                } else {
                    Err(anyhow!("Registration failed: {}", message))
                }
            }
            _ => Err(anyhow!("Unexpected response to registration")),
        }
    }

    /// Send offer to peer and wait for their info
    pub async fn send_offer(
        &mut self,
        target_fingerprint: &str,
        external_addr: SocketAddr,
        local_addr: SocketAddr,
    ) -> Result<PeerInfo> {
        let nonce = rand::random::<u64>();

        let msg = SignallingMessage::Offer {
            target_fingerprint: target_fingerprint.to_string(),
            external_ip: external_addr.ip().to_string(),
            external_port: external_addr.port(),
            local_ip: local_addr.ip().to_string(),
            local_port: local_addr.port(),
            nonce,
            fingerprint: self.local_fingerprint.as_ref()
                .ok_or_else(|| anyhow!("Not registered"))?
                .clone(),
        };

        self.send_message(&msg).await?;

        // Wait for peer's offer
        loop {
            let response = self.receive_message().await?;
            match response {
                SignallingMessage::ForwardOffer {
                    from_fingerprint,
                    external_ip,
                    external_port,
                    local_ip,
                    local_port,
                    nonce: peer_nonce,
                } => {
                    let external_addr = format!("{}:{}", external_ip, external_port)
                        .parse()
                        .context("Invalid peer external address")?;
                    let local_addr = format!("{}:{}", local_ip, local_port)
                        .parse()
                        .context("Invalid peer local address")?;

                    return Ok(PeerInfo {
                        fingerprint: from_fingerprint,
                        external_addr,
                        local_addr,
                        nonce: peer_nonce,
                    });
                }
                SignallingMessage::Error { message } => {
                    return Err(anyhow!("Signalling error: {}", message));
                }
                _ => {
                    // Ignore other messages
                }
            }
        }
    }

    /// Send a signalling message
    async fn send_message(&mut self, msg: &SignallingMessage) -> Result<()> {
        let json = serde_json::to_string(msg)
            .context("Failed to serialize message")?;
        
        self.ws_stream
            .send(Message::Text(json))
            .await
            .context("Failed to send WebSocket message")?;

        Ok(())
    }

    /// Receive a signalling message
    async fn receive_message(&mut self) -> Result<SignallingMessage> {
        loop {
            let msg = self.ws_stream
                .next()
                .await
                .ok_or_else(|| anyhow!("WebSocket connection closed"))??;

            match msg {
                Message::Text(text) => {
                    let msg = serde_json::from_str(&text)
                        .context("Failed to parse signalling message")?;
                    return Ok(msg);
                }
                Message::Ping(data) => {
                    self.ws_stream.send(Message::Pong(data)).await?;
                }
                Message::Pong(_) => {
                    // Ignore pongs
                }
                Message::Close(_) => {
                    return Err(anyhow!("WebSocket closed by server"));
                }
                _ => {
                    // Ignore binary and other message types
                }
            }
        }
    }

    /// Close the signalling connection
    pub async fn close(mut self) -> Result<()> {
        self.ws_stream
            .close(None)
            .await
            .context("Failed to close WebSocket")?;
        Ok(())
    }
}
