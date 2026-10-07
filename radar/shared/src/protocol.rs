use serde::{Deserialize, Serialize};
use typescript_type_def::TypeDef;

use crate::RadarState;

/// Protocol version — bump when making incompatible wire changes.
pub const RADAR_PROTOCOL_VERSION: u32 = 3;
/// Human-readable server identification sent in log messages / logs.
pub const AURORA_SERVER_NAME: &str = "Aurora";

#[derive(Serialize, Deserialize, Clone, Debug, TypeDef)]
pub enum SubscribeResult {
    Success,
    SessionDoesNotExists,
}

#[derive(Serialize, Deserialize, Clone, Debug, TypeDef)]
#[serde(rename_all = "kebab-case", tag = "type", content = "payload")]
pub enum S2CMessage {
    // ---- Generic responses ----
    ResponseSuccess {},
    ResponseError {
        error: String,
    },

    ResponseInvalidClientState {},
    ResponseInitializePublish {
        session_id: String,
        session_auth_token: String,
    },
    ResponseSubscribeSuccess {},
    ResponseSessionInvalidId {},

    // ---- Server-initiated notifications ----
    NotifyRadarState { state: RadarState },
    NotifyViewCount { viewers: usize },
    NotifySessionClosed {},
}

#[derive(Serialize, Deserialize, TypeDef)]
#[serde(rename_all = "kebab-case", tag = "type", content = "payload")]
pub enum C2SMessage {
    InitializePublish {
        #[serde(default)]
        session_auth_token: Option<String>,
    },
    InitializeSubscribe { session_id: String },

    NotifyRadarState { state: RadarState },

    Disconnect { reason: String },
}

/// Event funneled from the transport layer up to the command handler.
pub enum ClientEvent<T> {
    RecvMessage(T),
    RecvError(anyhow::Error),
    SendError(anyhow::Error),
}

/* ---------- Protocol V1 (legacy, rejected) ---------- */

#[derive(Serialize, Deserialize, TypeDef)]
pub enum HandshakeProtocolV1 {
    InitializePublish { version: u32 },
    InitializeSubscribe { version: u32 },
    ResponseError { error: String },
}

/* ---------- Protocol V2/V3 handshake ---------- */

#[derive(Serialize, Deserialize, TypeDef)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    tag = "type",
    content = "payload"
)]
pub enum HandshakeProtocolV2 {
    RequestInitialize { client_version: u32 },

    ResponseSuccess {
        server_version: u32,
        #[serde(default)]
        server_name: Option<String>,
    },
    ResponseIncompatible { supported_versions: Vec<u32> },
    ResponseGenericFailure { message: String },
}

#[derive(Serialize, Deserialize, TypeDef)]
#[serde(untagged)]
pub enum HandshakeMessage {
    V1(HandshakeProtocolV1),
    V2(HandshakeProtocolV2),
}
