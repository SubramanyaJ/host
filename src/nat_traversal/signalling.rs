/**
 * nat_traversal/signalling.rs
 *
 * TLS WebSocket signalling client (self-signed certs allowed for development)
 */

use anyhow::{Context, Result, anyhow};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream, MaybeTlsStream};
use tokio_tungstenite::client_async_tls_with_config;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio::net::TcpStream as TokioTcpStream;
use futures_util::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use native_tls::TlsConnector;
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

// WebSocket signalling client
/*
pub struct SignallingClient {
        ws_stream: WebSocketStream<MaybeTlsStream<TokioTcpStream>>,
        local_fingerprint: Option<String>,
}
*/

pub struct SignallingClient {
        ws_stream: WebSocketStream<MaybeTlsStream<tokio_native_tls::TlsStream<TokioTcpStream>>>,
        local_fingerprint: Option<String>,
}


impl SignallingClient {

        // Connect to signalling server (TLS, accepts self-signed)
        /*
        pub async fn connect(url: &str) -> Result<Self> {

                let req = url.into_client_request()
                        .context("Invalid signalling URL")?;

                // Allow self-signed certificate for development
                let tls = TlsConnector::builder()
                        .danger_accept_invalid_certs(true)
                        .build()
                        .expect("Failed to build TLS connector");

                // Extract host and port from URL
                let host = req.uri().host().ok_or_else(|| anyhow!("Missing hostname"))?;
                let port = req.uri().port_u16().unwrap_or(443);

                // TCP connect
                let stream = TokioTcpStream::connect((host, port))
                        .await
                        .context("TCP connection to signalling server failed")?;

                // Perform TLS + WebSocket handshake
                let (ws_stream, _resp) = client_async_tls_with_config(
                        req,
                        stream,
                        None,
                        Some(tls.into())
                )
                        .await
                        .context("TLS WebSocket handshake failed")?;

                Ok(Self {
                        ws_stream,
                        local_fingerprint: None,
                })
        }
        */

    pub async fn connect(url: &str) -> Result<Self> {
        let req = url.into_client_request()
                .context("Invalid signalling URL")?;

        // Allow self-signed certs in DEV
        let mut tls_builder = TlsConnector::builder();
        tls_builder.danger_accept_invalid_certs(true);
        let tls = tls_builder.build().unwrap();
        let tls = tokio_native_tls::TlsConnector::from(tls);

        // Parse host + port from URL
        let host = req.uri().host().ok_or_else(|| anyhow!("Missing hostname"))?;
        let port = req.uri().port_u16().unwrap_or(443);

        // STEP 1: Raw TCP connect
        let tcp = TokioTcpStream::connect((host, port))
                .await
                .context("TCP connection failed")?;

        // STEP 2: TLS handshake over TCP
        let tls_stream = tls.connect(host, tcp)
                .await
                .context("TLS handshake failed")?;

        // STEP 3: WebSocket upgrade over TLS
        let (ws_stream, _resp) =
                tokio_tungstenite::client_async_tls_with_config(
                        req,
                        tls_stream,
                        None,
                        None
                )
                .await
                .context("WebSocket upgrade failed")?;

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

                // Wait for ack
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
                        _ => Err(anyhow!("Unexpected registration response")),
                }
        }

        /// Send offer and wait for peer offer
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
                        fingerprint: self.local_fingerprint
                                .as_ref()
                                .ok_or_else(|| anyhow!("Not registered"))?
                                .clone(),
                };

                self.send_message(&msg).await?;

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
                                        let external = format!("{}:{}", external_ip, external_port)
                                                .parse()
                                                .context("Invalid external addr")?;
                                        let local = format!("{}:{}", local_ip, local_port)
                                                .parse()
                                                .context("Invalid local addr")?;

                                        return Ok(PeerInfo {
                                                fingerprint: from_fingerprint,
                                                external_addr: external,
                                                local_addr: local,
                                                nonce: peer_nonce,
                                        });
                                }
                                SignallingMessage::Error { message } => {
                                        return Err(anyhow!("Signalling error: {}", message));
                                }
                                _ => {}
                        }
                }
        }

        async fn send_message(&mut self, msg: &SignallingMessage) -> Result<()> {
                let json = serde_json::to_string(msg)
                        .context("Message serialization failed")?;

                self.ws_stream
                        .send(Message::Text(json))
                        .await
                        .context("WebSocket send failed")?;

                Ok(())
        }

        async fn receive_message(&mut self) -> Result<SignallingMessage> {
                loop {
                        let msg = self.ws_stream
                                .next()
                                .await
                                .ok_or_else(|| anyhow!("Connection closed"))??;

                        match msg {
                                Message::Text(text) => {
                                        let parsed = serde_json::from_str(&text)
                                                .context("Failed to decode signalling message")?;
                                        return Ok(parsed);
                                }
                                Message::Ping(data) => {
                                        self.ws_stream.send(Message::Pong(data)).await?;
                                }
                                Message::Pong(_) => {}
                                Message::Close(_) => {
                                        return Err(anyhow!("Server closed WebSocket"));
                                }
                                _ => {}
                        }
                }
        }

        pub async fn close(mut self) -> Result<()> {
                self.ws_stream
                        .close(None)
                        .await
                        .context("Failed closing WebSocket")?;
                Ok(())
        }
}

