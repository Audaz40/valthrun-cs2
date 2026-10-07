use std::net::SocketAddr;
use std::sync::Weak;

use anyhow::Context;
use futures::{SinkExt, StreamExt};
use radar_shared::protocol::{
    ClientEvent, HandshakeMessage, HandshakeProtocolV1, HandshakeProtocolV2, S2CMessage,
    RADAR_PROTOCOL_VERSION,
};
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::RwLock;
use warp::filters::ws::{Message, WebSocket};

use crate::RadarServer;

pub type ClientId = u32;

/// Size of the per-client outbound message queue. If the queue backs up
/// beyond this, messages are dropped (slow-consumer protection).
const OUTBOUND_QUEUE_CAPACITY: usize = 64;
/// Size of the per-client inbound message queue.
const INBOUND_QUEUE_CAPACITY: usize = 64;

#[derive(Clone)]
pub enum ClientState {
    Uninitialized,
    Publisher { session_id: String },
    Subscriber { session_id: String },
}

pub struct PubClient {
    pub client_id: ClientId,
    pub address: SocketAddr,

    pub state: ClientState,

    pub tx: Sender<S2CMessage>,
}

impl PubClient {
    pub fn new(tx: Sender<S2CMessage>, address: SocketAddr) -> Self {
        Self {
            client_id: 0,
            address,
            state: ClientState::Uninitialized,
            tx,
        }
    }

    /// Non-blocking send. Returns Err if the queue is full or closed.
    pub fn try_send_command(&self, command: S2CMessage) -> Result<(), mpsc::error::TrySendError<S2CMessage>> {
        self.tx.try_send(command)
    }

    /// Graceful close: send a disconnect message, then drop the channel.
    pub async fn shutdown(&self, reason: &str) {
        let _ = self.tx.send(S2CMessage::ResponseError {
            error: reason.to_string(),
        }).await;
    }

    async fn process_protocol_handshake(socket: &mut WebSocket) -> anyhow::Result<()> {
        let message = socket.next().await.context("eof on protocol handshake")??;
        let message = serde_json::from_slice::<HandshakeMessage>(message.as_bytes())
            .context("failed to parse handshake")?;

        match message {
            HandshakeMessage::V1(_) => {
                let _ = socket
                    .send(Message::text(serde_json::to_string(
                        &HandshakeProtocolV1::ResponseError {
                            error: "Aurora requires protocol v2 — please update your client.".into(),
                        },
                    )?))
                    .await;
                anyhow::bail!("unsupported v1 client");
            }
            HandshakeMessage::V2(message) => {
                let HandshakeProtocolV2::RequestInitialize { client_version } = message else {
                    log::debug!("Client sent non-initialize handshake message; disconnecting.");
                    let _ = socket
                        .send(Message::text(serde_json::to_string(
                            &HandshakeProtocolV2::ResponseGenericFailure {
                                message: "invalid handshake sequence".into(),
                            },
                        )?))
                        .await;
                    anyhow::bail!("invalid handshake message");
                };

                if client_version != RADAR_PROTOCOL_VERSION {
                    log::debug!(
                        "Client version {} unsupported (server expects {}). Disconnecting.",
                        client_version,
                        RADAR_PROTOCOL_VERSION
                    );
                    let _ = socket
                        .send(Message::text(serde_json::to_string(
                            &HandshakeProtocolV2::ResponseIncompatible {
                                supported_versions: vec![RADAR_PROTOCOL_VERSION],
                            },
                        )?))
                        .await;
                    anyhow::bail!(
                        "client protocol version {} incompatible with server {}",
                        client_version,
                        RADAR_PROTOCOL_VERSION
                    );
                }

                socket
                    .send(Message::text(serde_json::to_string(
                        &HandshakeProtocolV2::ResponseSuccess {
                            server_version: RADAR_PROTOCOL_VERSION,
                            server_name: Some("Aurora".into()),
                        },
                    )?))
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn serve_from_websocket(
        server: Weak<RwLock<RadarServer>>,
        client_address: SocketAddr,
        mut socket: WebSocket,
    ) {
        if let Err(err) = Self::process_protocol_handshake(&mut socket).await {
            log::debug!(
                "Handshake failed for {}: {:#}; closing connection.",
                client_address,
                err
            );
            let _ = socket.flush().await;
            let _ = socket.close().await;
            return;
        }

        let (message_tx, mut message_tx_rx) = mpsc::channel(OUTBOUND_QUEUE_CAPACITY);
        let (message_rx_tx, message_rx) = mpsc::channel(INBOUND_QUEUE_CAPACITY);

        let server = match server.upgrade() {
            Some(s) => s,
            None => {
                log::warn!(
                    "Accepted ws client from {}, but server is gone. Dropping.",
                    client_address
                );
                return;
            }
        };

        let client_fut = {
            let mut srv = server.write().await;
            srv.register_client(PubClient::new(message_tx, client_address), message_rx).await
        };
        let client_task = tokio::spawn(client_fut);

        // Split the socket into read/write halves.
        let (mut tx, mut rx) = socket.split();

        // Reader task: parses incoming text frames and dispatches to the
        // server's command handler via the inbound channel.
        let reader_jh = tokio::spawn({
            let inbound = message_rx_tx.clone();
            async move {
                while let Some(frame) = rx.next().await {
                    let frame = match frame {
                        Ok(f) => f,
                        Err(err) => {
                            let _ = inbound.send(ClientEvent::RecvError(err.into())).await;
                            break;
                        }
                    };

                    if !frame.is_text() {
                        // Ignore binary/pong/close frames silently; warp
                        // handles ping/pong for us.
                        continue;
                    }

                    let body = frame.as_bytes();
                    match serde_json::from_slice::<radar_shared::protocol::C2SMessage>(body) {
                        Ok(msg) => {
                            if inbound.send(ClientEvent::RecvMessage(msg)).await.is_err() {
                                // Channel closed → handler exited.
                                break;
                            }
                        }
                        Err(err) => {
                            log::trace!(
                                "Unparsable frame from client ({}): {:#}; payload[0..128]={:?}",
                                client_address,
                                err,
                                String::from_utf8_lossy(
                                    &body[..body.len().min(128)]
                                )
                            );
                            let _ = inbound.send(ClientEvent::RecvError(err.into())).await;
                            break;
                        }
                    }
                }
            }
        });

        // Writer task: serializes outbound messages and sends them down the wire.
        let writer_jh = tokio::spawn({
            let inbound = message_rx_tx.clone();
            async move {
                while let Some(message) = message_tx_rx.recv().await {
                    let payload = match serde_json::to_string(&message) {
                        Ok(p) => p,
                        Err(err) => {
                            let _ = inbound.send(ClientEvent::SendError(err.into())).await;
                            break;
                        }
                    };
                    if let Err(err) = tx.send(Message::text(payload)).await {
                        let _ = inbound.send(ClientEvent::SendError(err.into())).await;
                        break;
                    }
                }
            }
        });

        // When either half exits, signal the command handler and wait for cleanup.
        tokio::select! {
            _ = reader_jh => {},
            _ = writer_jh => {},
            _ = client_task => {},
        }

        // Notify the command handler loop (if it hasn't exited yet) so it
        // can perform cleanup/unregistration.
        let _ = message_rx_tx
            .send(ClientEvent::RecvError(anyhow::anyhow!("websocket closed")))
            .await;
    }
}
