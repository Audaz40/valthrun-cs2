use std::collections::BTreeMap;
use std::fmt;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio::time;

use radar_shared::protocol::{C2SMessage, ClientEvent, S2CMessage};
use rand::distributions::Alphanumeric;
use rand::Rng;

use crate::client::PubClient;
use crate::handler::ServerCommandHandler;
use crate::{ClientId, ClientState};

/* =============================================================================
 * Server-wide atomic counters for metrics (lock-free)
 * ===========================================================================*/

#[derive(Default)]
pub struct ServerMetrics {
    pub total_connections: AtomicU64,
    pub active_publishers: AtomicU64,
    pub active_subscribers: AtomicU64,
    pub messages_broadcast: AtomicU64,
    pub bytes_broadcast: AtomicU64,
    pub sessions_created: AtomicU64,
    pub start_time: Instant,
}

impl ServerMetrics {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            ..Default::default()
        }
    }

    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_connections: self.total_connections.load(Ordering::Relaxed),
            active_publishers: self.active_publishers.load(Ordering::Relaxed),
            active_subscribers: self.active_subscribers.load(Ordering::Relaxed),
            messages_broadcast: self.messages_broadcast.load(Ordering::Relaxed),
            sessions_created: self.sessions_created.load(Ordering::Relaxed),
            uptime_secs: self.uptime().as_secs(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MetricsSnapshot {
    pub total_connections: u64,
    pub active_publishers: u64,
    pub active_subscribers: u64,
    pub messages_broadcast: u64,
    pub sessions_created: u64,
    pub uptime_secs: u64,
}

impl fmt::Display for MetricsSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "conns={} pubs={} subs={} msgs={} sessions={} uptime={}s",
            self.total_connections,
            self.active_publishers,
            self.active_subscribers,
            self.messages_broadcast,
            self.sessions_created,
            self.uptime_secs
        )
    }
}

/* =============================================================================
 * Session ID generation — slightly longer & human-friendly
 * ===========================================================================*/

fn generate_session_id() -> String {
    // Aurora uses 8-char session IDs (lowercase) for a larger keyspace.
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .map(|c| (c as char).to_ascii_lowercase())
        .filter(|c| c.is_ascii_alphanumeric())
        // filter out confusable chars
        .filter(|c| !matches!(c, '0' | 'o' | '1' | 'l' | 'i'))
        .take(8)
        .collect::<String>()
}

fn generate_auth_token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .map(char::from)
        .take(24)
        .collect::<String>()
}

/* =============================================================================
 * Session
 * ===========================================================================*/

/// Broadcast budget per publisher per second (messages).
/// Protects the server from a misbehaving/malicious publisher flooding subscribers.
pub const MAX_BROADCASTS_PER_SECOND: u32 = 60;

pub enum PubSessionOwner {
    Owned { client_id: ClientId },
    Unbound { timestamp: Instant },
}

pub struct PubSession {
    pub owner: PubSessionOwner,

    pub session_id: String,
    pub session_auth_token: String,
    pub created_at: Instant,

    /// Last time we received a radar-state update from the owner.
    pub last_update: Option<Instant>,

    /// Subscriber channels. BTreeMap keeps deterministic ordering.
    subscriber: BTreeMap<ClientId, mpsc::Sender<S2CMessage>>,

    /// Rolling counter of broadcasts this second (for rate-limiting).
    broadcast_window_start: Instant,
    broadcast_window_count: u32,
}

impl PubSession {
    fn new(owner_id: ClientId, session_id: String, session_auth_token: String) -> Self {
        Self {
            owner: PubSessionOwner::Owned {
                client_id: owner_id,
            },
            session_id,
            session_auth_token,
            created_at: Instant::now(),
            last_update: None,
            subscriber: BTreeMap::new(),
            broadcast_window_start: Instant::now(),
            broadcast_window_count: 0,
        }
    }

    /// Broadcast a message to all subscribers, dropping slow subscribers
    /// rather than blocking the server. Returns the number of subscribers
    /// the message was *successfully* sent to.
    pub fn broadcast(&mut self, message: &S2CMessage) -> usize {
        // Rate-limit per session (per-second window).
        let now = Instant::now();
        if now.duration_since(self.broadcast_window_start) >= Duration::from_secs(1) {
            self.broadcast_window_start = now;
            self.broadcast_window_count = 0;
        }
        self.broadcast_window_count = self.broadcast_window_count.saturating_add(1);
        if self.broadcast_window_count > MAX_BROADCASTS_PER_SECOND {
            log::trace!(
                "Session {}: broadcast rate-limit hit ({} in window), dropping message",
                self.session_id,
                self.broadcast_window_count
            );
            return 0;
        }

        if matches!(message, S2CMessage::NotifyRadarState { .. }) {
            self.last_update = Some(now);
        }

        let mut sent = 0usize;
        // Pre-serialize once for JSON, then send the raw string to avoid
        // re-serializing per subscriber. Callers still pass an enum though,
        // so we keep the simple "clone per send" semantics here, but we
        // evict dead subscribers as we iterate.
        let mut dead: Vec<ClientId> = Vec::new();
        for (&id, tx) in &self.subscriber {
            match tx.try_send(message.clone()) {
                Ok(()) => {
                    sent += 1;
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    // Subscriber is slow; drop the message rather than block,
                    // but don't evict (one full buffer isn't disconnection).
                    log::trace!("Subscriber {} buffer full, dropping message", id);
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    dead.push(id);
                }
            }
        }

        for id in dead {
            self.subscriber.remove(&id);
            log::debug!("Session {}: evicted dead subscriber {}", self.session_id, id);
        }

        sent
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscriber.len()
    }

    pub fn is_stale(&self, unbound_timeout: Duration, idle_timeout: Duration) -> bool {
        match &self.owner {
            PubSessionOwner::Unbound { timestamp } => timestamp.elapsed() > unbound_timeout,
            PubSessionOwner::Owned { .. } => match self.last_update {
                // A publisher that hasn't sent any update in `idle_timeout`
                // is considered dead (network drop without close frame).
                Some(last) => last.elapsed() > idle_timeout,
                // Give a fresh publisher some grace time before declaring stale.
                None => self.created_at.elapsed() > Duration::from_secs(120),
            },
        }
    }
}

/* =============================================================================
 * Static HTTP serving
 * ===========================================================================*/

pub enum HttpServeDirectory {
    None,
    Disk { path: std::path::PathBuf },
    Bundled,
}

/* =============================================================================
 * Server
 * ===========================================================================*/

pub struct RadarServer {
    ref_self: Weak<RwLock<RadarServer>>,
    client_id_counter: ClientId,

    clients: BTreeMap<ClientId, Arc<RwLock<PubClient>>>,
    pub_sessions: BTreeMap<String, PubSession>,

    www_acceptor: Option<JoinHandle<()>>,

    pub metrics: Arc<ServerMetrics>,

    /// How long an unbound (publisher-disconnected) session is retained
    /// so the publisher can reconnect with its auth token.
    pub unbound_timeout: Duration,
    /// How long an owned session is kept alive without any radar-state update
    /// before we assume the publisher died without closing cleanly.
    pub idle_timeout: Duration,
}

impl RadarServer {
    pub fn new() -> Arc<RwLock<Self>> {
        let metrics = Arc::new(ServerMetrics::new());

        let mut result = Self {
            ref_self: Default::default(),
            client_id_counter: 0, // starts at 0; we increment before issuing (see register_client)
            clients: Default::default(),
            pub_sessions: Default::default(),
            www_acceptor: None,
            metrics,
            unbound_timeout: Duration::from_secs(180), // 3 min for reconnect
            idle_timeout: Duration::from_secs(30),
        };

        Arc::new_cyclic(|weak| {
            result.ref_self = weak.clone();
            tokio::spawn(Self::tick_task(weak.clone()));
            tokio::spawn(Self::metrics_log_task(weak.clone()));
            RwLock::new(result)
        })
    }

    async fn tick_task(this: Weak<RwLock<Self>>) {
        let mut interval = time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let Some(this) = this.upgrade() else {
                return;
            };
            let mut this = this.write().await;
            this.tick().await;
        }
    }

    async fn metrics_log_task(this: Weak<RwLock<Self>>) {
        let mut interval = time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let Some(this) = this.upgrade() else {
                return;
            };
            let this = this.read().await;
            log::info!(
                "[aurora-metrics] {} sessions={}",
                this.metrics.snapshot(),
                this.pub_sessions.len()
            );
        }
    }

    async fn tick(&mut self) {
        let unbound_timeout = self.unbound_timeout;
        let idle_timeout = self.idle_timeout;

        let expired: Vec<String> = self
            .pub_sessions
            .iter()
            .filter(|(_, s)| s.is_stale(unbound_timeout, idle_timeout))
            .map(|(id, s)| {
                log::info!(
                    "Session {} expired (owner staleness); closing.",
                    s.session_id
                );
                id.clone()
            })
            .collect();

        for session_id in expired {
            self.pub_session_close(&session_id).await;
        }
    }

    pub async fn listen_http(
        &mut self,
        addr: impl Into<SocketAddr>,
        static_serve: HttpServeDirectory,
    ) -> anyhow::Result<()> {
        if self.www_acceptor.is_some() {
            anyhow::bail!("www already started");
        }

        let server = self.ref_self.clone();
        let metrics = self.metrics.clone();

        // ---- WebSocket routes (publish + subscribe) ----
        let ws_route = warp::path("subscribe")
            .or(warp::path("publish"))
            .and(warp::addr::remote())
            .and(warp::ws())
            .map(move |_, address: Option<SocketAddr>, ws: warp::ws::Ws| {
                let server = server.clone();
                let metrics = metrics.clone();
                ws.on_upgrade(move |socket| async move {
                    let Some(address) = address else { return };
                    metrics.total_connections.fetch_add(1, Ordering::Relaxed);
                    PubClient::serve_from_websocket(server, address, socket).await;
                })
            })
            .boxed();

        // ---- Health endpoint for operators/load balancers ----
        let metrics_handle = self.metrics.clone();
        let health_route = warp::path("health")
            .map(move || {
                let snap = metrics_handle.snapshot();
                warp::reply::json(&snap)
            })
            .boxed();

        // ---- Server info (public) ----
        let info_route = warp::path("info").map(|| {
            #[derive(Serialize)]
            struct Info<'a> {
                name: &'a str,
                protocol_version: u32,
                version: &'a str,
            }
            warp::reply::json(&Info {
                name: "Aurora Radar",
                protocol_version: radar_shared::protocol::RADAR_PROTOCOL_VERSION,
                version: env!("CARGO_PKG_VERSION"),
            })
        }).boxed();

        let routes: warp::filters::BoxedFilter<(Box<dyn warp::Reply>,)> = match static_serve {
            HttpServeDirectory::Disk { path } => ws_route
                .or(health_route)
                .or(info_route)
                .or(warp::fs::dir(path.clone()))
                .or(warp::fs::file(path.join("index.html")))
                .map(|reply| -> Box<dyn warp::Reply> { Box::new(reply) })
                .boxed(),
            HttpServeDirectory::Bundled => {
                anyhow::bail!("bundled static assets are currently not supported");
            }
            HttpServeDirectory::None => ws_route
                .or(health_route)
                .or(info_route)
                .map(|reply| -> Box<dyn warp::Reply> { Box::new(reply) })
                .boxed(),
        };

        // Wrap routes with permissive CORS headers for web radar clients and
        // a tiny per-request access log (method + path + status).
        use warp::http::{HeaderValue, Method};
        let wrapped = routes.with(warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type", "upgrade", "sec-websocket-key", "sec-websocket-version", "sec-websocket-extensions", "connection"])
            .allow_methods(&[Method::GET, Method::POST, Method::OPTIONS])
            .allow_credentials(false)
        ).with(warp::log::custom(|info| {
            log::debug!(
                "{} {} {} ({})",
                info.method(),
                info.path(),
                info.status().as_u16(),
                info.elapsed().as_micros()
            );
        }));

        let (address, future) = warp::serve(wrapped).try_bind_ephemeral(addr)?;
        self.www_acceptor = Some(tokio::spawn(future));

        log::info!(
            "Aurora radar server listening on {}  (health /health, info /info)",
            address
        );
        Ok(())
    }

    pub async fn unregister_client(&mut self, client_id: ClientId, clean_disconnect: bool) {
        let client = match self.clients.remove(&client_id) {
            Some(client) => client,
            None => return,
        };

        let client_state = {
            let client = client.read().await;
            client.state.clone()
        };

        match client_state {
            ClientState::Publisher { session_id } => {
                self.metrics.active_publishers.fetch_sub(1, Ordering::Relaxed);
                if clean_disconnect {
                    self.pub_session_close(&session_id).await;
                } else {
                    self.pub_session_unbind(&session_id).await;
                }
            }
            ClientState::Subscriber { session_id } => {
                self.metrics.active_subscribers.fetch_sub(1, Ordering::Relaxed);
                self.pub_session_unsubscribe(&session_id, client_id).await;
            }
            ClientState::Uninitialized => { /* nothing to do */ }
        }

        log::debug!("Disconnected client {} (clean={})", client_id, clean_disconnect);
    }

    pub async fn register_client(
        &mut self,
        mut client: PubClient,
        mut rx: mpsc::Receiver<ClientEvent<C2SMessage>>,
    ) -> impl std::future::Future<Output = ()> {
        // Issue a fresh client id. Use wrapping_add to stay panic-free even
        // after 2^32 connections (extremely unlikely in practice).
        self.client_id_counter = self.client_id_counter.wrapping_add(1);
        let client_id = self.client_id_counter;

        log::debug!(
            "Registered new client from {} as client id {}",
            client.address,
            client_id
        );

        client.client_id = client_id;
        let client = Arc::new(RwLock::new(client));
        self.clients.insert(client_id, client.clone());

        let command_handler = ServerCommandHandler {
            server: self.ref_self.upgrade().expect("server is alive"),
            client: client.clone(),
            client_id,
            metrics: self.metrics.clone(),
        };

        async move {
            let clean_disconnect = loop {
                let Some(event) = rx.recv().await else {
                    break false;
                };
                match event {
                    ClientEvent::RecvMessage(command) => {
                        if let C2SMessage::Disconnect { reason: message } = &command {
                            log::debug!(
                                "Client {} requested disconnect: {}",
                                command_handler.client_id,
                                message
                            );
                            break true;
                        }

                        let result = command_handler.handle_command(command).await;
                        if let Err(serr) = command_handler
                            .client
                            .read()
                            .await
                            .try_send_command(result)
                        {
                            log::warn!(
                                "Failed to queue response for client {}: {}",
                                command_handler.client_id,
                                serr
                            );
                        }
                    }
                    ClientEvent::RecvError(err) => {
                        log::debug!(
                            "Client {} recv error: {:#}",
                            command_handler.client_id,
                            err
                        );
                        break false;
                    }
                    ClientEvent::SendError(err) => {
                        log::debug!(
                            "Client {} send error: {:#}",
                            command_handler.client_id,
                            err
                        );
                        break false;
                    }
                }
            };

            command_handler
                .server
                .write()
                .await
                .unregister_client(command_handler.client_id, clean_disconnect)
                .await;
        }
    }

    pub async fn pub_session_create(&mut self, owner_id: ClientId) -> Option<&PubSession> {
        let owner = self.clients.get(&owner_id)?;

        // Fast pre-check without holding owner lock across inserts.
        {
            let owner = owner.read().await;
            if !matches!(owner.state, ClientState::Uninitialized) {
                return None;
            }
        }

        // Generate a unique session id (extremely unlikely collision, but check).
        let session_id = loop {
            let id = generate_session_id();
            if !self.pub_sessions.contains_key(&id) {
                break id;
            }
        };
        let session_auth_token = generate_auth_token();

        let session = PubSession::new(owner_id, session_id.clone(), session_auth_token.clone());
        self.pub_sessions.insert(session_id.clone(), session);
        self.metrics.sessions_created.fetch_add(1, Ordering::Relaxed);
        self.metrics.active_publishers.fetch_add(1, Ordering::Relaxed);

        log::info!("Created new session `{}`", session_id);

        {
            let mut owner = owner.write().await;
            owner.state = ClientState::Publisher {
                session_id: session_id.clone(),
            };
        }

        self.pub_sessions.get(&session_id)
    }

    pub async fn pub_session_reclaim(
        &mut self,
        client_id: ClientId,
        session_auth_token: &str,
    ) -> Option<&PubSession> {
        let owner = self.clients.get(&client_id)?;

        {
            let owner = owner.read().await;
            if !matches!(owner.state, ClientState::Uninitialized) {
                return None;
            }
        }

        // Look up session by auth token (constant-time scan; session count is small).
        let sid = {
            let mut found = None;
            for (sid, s) in &self.pub_sessions {
                if s.session_auth_token == session_auth_token {
                    found = Some(sid.clone());
                    break;
                }
            }
            found?
        };

        let session = self.pub_sessions.get_mut(&sid)?;
        if !matches!(&session.owner, PubSessionOwner::Unbound { .. }) {
            return None;
        }
        session.owner = PubSessionOwner::Owned { client_id };
        self.metrics.active_publishers.fetch_add(1, Ordering::Relaxed);

        log::info!("Publisher {} reclaimed session `{}`", client_id, sid);
        {
            let mut owner = owner.write().await;
            owner.state = ClientState::Publisher {
                session_id: sid.clone(),
            };
        }
        self.pub_sessions.get(&sid)
    }

    pub async fn pub_session_unbind(&mut self, session_id: &str) {
        let Some(session) = self.pub_sessions.get_mut(session_id) else {
            return;
        };
        if !matches!(&session.owner, PubSessionOwner::Owned { .. }) {
            return;
        }
        log::info!("Publisher unbound from session `{}` (retained for reconnect)", session_id);
        session.owner = PubSessionOwner::Unbound {
            timestamp: Instant::now(),
        };
    }

    pub async fn pub_session_close(&mut self, session_id: &str) {
        let Some(session) = self.pub_sessions.remove(session_id) else {
            return;
        };
        log::info!("Session `{}` closed ({} subscribers notified)", session_id, session.subscriber.len());

        // Close all subscriber channels.
        session.broadcast(&S2CMessage::NotifySessionClosed {});
        for client_id in session.subscriber.keys().copied().collect::<Vec<_>>() {
            if let Some(client) = self.clients.get(&client_id) {
                let mut client = client.write().await;
                if let ClientState::Subscriber { session_id: ref sid } = client.state {
                    if sid == session_id {
                        client.state = ClientState::Uninitialized;
                    }
                }
            }
            self.metrics.active_subscribers.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub fn pub_session_find(&self, session_id: &str) -> Option<&PubSession> {
        self.pub_sessions.get(session_id)
    }

    pub fn pub_session_find_mut(&mut self, session_id: &str) -> Option<&mut PubSession> {
        self.pub_sessions.get_mut(session_id)
    }

    pub async fn pub_session_unsubscribe(&mut self, session_id: &str, client_id: ClientId) {
        let viewers_after = if let Some(session) = self.pub_sessions.get_mut(session_id) {
            session.subscriber.remove(&client_id);
            let count = session.subscriber_count();
            session.broadcast(&S2CMessage::NotifyViewCount { viewers: count });
            count
        } else {
            0
        };

        if let Some(client) = self.clients.get(&client_id) {
            let mut client = client.write().await;
            if let ClientState::Subscriber { session_id: ref sid } = client.state {
                if sid != session_id {
                    log::warn!(
                        "Client {} state references a different session ({} vs {}) during unsubscribe",
                        client_id,
                        sid,
                        session_id
                    );
                }
                client.state = ClientState::Uninitialized;
            }
        }

        log::debug!(
            "Client {} unsubscribed from `{}` ({} viewers remain)",
            client_id,
            session_id,
            viewers_after
        );
    }

    pub async fn pub_session_subscribe(
        &mut self,
        session_id: &str,
        client_id: ClientId,
    ) -> PubSessionSubscribeResult {
        let Some(client) = self.clients.get(&client_id) else {
            return PubSessionSubscribeResult::InvalidClientId;
        };

        {
            let client = client.read().await;
            if !matches!(client.state, ClientState::Uninitialized) {
                return PubSessionSubscribeResult::InvalidClientState;
            }
        }

        let Some(session) = self.pub_sessions.get_mut(session_id) else {
            return PubSessionSubscribeResult::InvalidSessionId;
        };

        // Don't allow a session with no active owner to be subscribed to.
        if matches!(session.owner, PubSessionOwner::Unbound { .. }) {
            return PubSessionSubscribeResult::SessionOffline;
        }

        let tx = {
            let client = client.read().await;
            client.tx.clone()
        };

        session.subscriber.insert(client_id, tx);
        let viewer_count = session.subscriber_count();
        session.broadcast(&S2CMessage::NotifyViewCount { viewers: viewer_count });
        self.metrics.active_subscribers.fetch_add(1, Ordering::Relaxed);

        {
            let mut client = client.write().await;
            client.state = ClientState::Subscriber {
                session_id: session.session_id.clone(),
            };
        }

        log::debug!(
            "Client {} subscribed to session `{}` ({} viewers)",
            client_id,
            session_id,
            viewer_count
        );

        PubSessionSubscribeResult::Success
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PubSessionSubscribeResult {
    Success,
    InvalidClientState,
    InvalidSessionId,
    InvalidClientId,
    SessionOffline,
}
