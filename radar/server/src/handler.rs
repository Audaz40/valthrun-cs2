use std::sync::Arc;
use std::sync::atomic::Ordering;

use radar_shared::protocol::{C2SMessage, S2CMessage};
use tokio::sync::RwLock;

use crate::client::{ClientId, ClientState, PubClient};
use crate::server::{PubSessionOwner, PubSessionSubscribeResult, RadarServer, ServerMetrics};

pub struct ServerCommandHandler {
    pub server: Arc<RwLock<RadarServer>>,
    pub client: Arc<RwLock<PubClient>>,
    pub client_id: ClientId,
    pub metrics: Arc<ServerMetrics>,
}

impl ServerCommandHandler {
    pub async fn handle_command(&self, command: C2SMessage) -> S2CMessage {
        match command {
            C2SMessage::InitializePublish { session_auth_token } => {
                let mut server = self.server.write().await;

                let session = if let Some(auth_token) = session_auth_token {
                    match server.pub_session_reclaim(self.client_id, &auth_token).await {
                        Some(s) => s,
                        None => return S2CMessage::ResponseSessionInvalidId {},
                    }
                } else {
                    match server.pub_session_create(self.client_id).await {
                        Some(s) => s,
                        None => return S2CMessage::ResponseInvalidClientState {},
                    }
                };

                log::info!(
                    "Client {} initialized as publisher for session `{}`",
                    self.client_id,
                    session.session_id
                );

                S2CMessage::ResponseInitializePublish {
                    session_id: session.session_id.clone(),
                    session_auth_token: session.session_auth_token.clone(),
                }
            }

            C2SMessage::InitializeSubscribe { session_id } => {
                let mut server = self.server.write().await;
                match server.pub_session_subscribe(&session_id, self.client_id).await {
                    PubSessionSubscribeResult::Success => {
                        log::debug!(
                            "Client {} subscribed to session `{}`",
                            self.client_id,
                            session_id
                        );
                        S2CMessage::ResponseSubscribeSuccess {}
                    }
                    PubSessionSubscribeResult::InvalidClientId
                    | PubSessionSubscribeResult::InvalidClientState => {
                        S2CMessage::ResponseInvalidClientState {}
                    }
                    PubSessionSubscribeResult::InvalidSessionId
                    | PubSessionSubscribeResult::SessionOffline => {
                        S2CMessage::ResponseSessionInvalidId {}
                    }
                }
            }

            C2SMessage::NotifyRadarState { state } => {
                // Hold a write lock for the duration: we need to mutate
                // session.last_update/broadcast counters. This is a single
                // short critical section per radar tick.
                let mut server = self.server.write().await;
                let session_id = {
                    let client = self.client.read().await;
                    match &client.state {
                        ClientState::Publisher { session_id } => session_id.clone(),
                        _ => return S2CMessage::ResponseInvalidClientState {},
                    }
                };

                // Verify ownership and broadcast in one pass.
                let sent = match server.pub_session_find_mut(&session_id) {
                    None => return S2CMessage::ResponseSessionInvalidId {},
                    Some(session) => {
                        match &session.owner {
                            PubSessionOwner::Owned { client_id } if *client_id == self.client_id => {}
                            _ => {
                                return S2CMessage::ResponseError {
                                    error: "not authorized to publish to this session".into(),
                                };
                            }
                        }
                        let s = session.broadcast(&S2CMessage::NotifyRadarState { state });
                        self.metrics
                            .messages_broadcast
                            .fetch_add(s as u64, Ordering::Relaxed);
                        s
                    }
                };

                log::trace!(
                    "Broadcast radar state to {} subscriber(s) on `{}`",
                    sent,
                    session_id
                );
                S2CMessage::ResponseSuccess {}
            }

            C2SMessage::Disconnect { .. } => S2CMessage::ResponseSuccess {},
        }
    }
}
