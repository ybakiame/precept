//! Ephemeral operator session DTOs. Not project Handoff or an agent-facing API.
use serde::{Deserialize, Serialize};
pub const SESSION_API: u32 = 1;
#[cfg(feature = "runtime")]
pub mod runtime;
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    #[default]
    Idle,
    Starting,
    Ready,
    Running,
    WaitingPermission,
    Completed,
    Failed,
    Stopping,
    Stopped,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permission {
    pub request_id: String,
    pub operation: String,
    pub allow_once: Option<String>,
    pub expires_ms: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub session_api_version: u32,
    pub project: String,
    pub session_id: String,
    pub executable: String,
    pub state: State,
    pub process_alive: bool,
    pub agent: String,
    pub load_session_advertised: bool,
    pub last_event_ms: u64,
    pub revision: u64,
    pub events: Vec<String>,
    pub permissions: Vec<Permission>,
    pub stop_reason: Option<String>,
    pub failure: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub session_api_version: u32,
    pub protocol_version: u32,
    pub executables: Vec<String>,
    pub resume_supported: bool,
}
