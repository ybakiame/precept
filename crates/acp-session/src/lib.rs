//! Ephemeral operator session DTOs. Not project Handoff or an agent-facing API.
use serde::{Deserialize, Serialize};
pub const SESSION_API: u32 = 2;
#[cfg(feature = "runtime")]
mod bridge;
#[cfg(feature = "runtime")]
mod handshake;
#[cfg(feature = "runtime")]
pub mod runtime;
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    #[default]
    Idle,
    Starting,
    AuthRequired,
    Ready,
    Running,
    WaitingPermission,
    Completed,
    Failed,
    Stopping,
    Stopped,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethodKind {
    Agent,
    Terminal,
    Unsupported,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    #[default]
    Unobserved,
    Required,
    SessionAccepted,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authentication {
    pub status: AuthStatus,
    /// Only kinds are retained. No login IDs, names, descriptions, URLs, argv or env.
    pub methods: Vec<AuthMethodKind>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentFeatures {
    pub image: bool,
    pub audio: bool,
    pub embedded_context: bool,
    pub mcp_http: bool,
    pub mcp_sse: bool,
    pub logout: bool,
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
pub struct Activity {
    pub response_chunks: u32,
    pub response_characters: u64,
    pub tool_events: u32,
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
    pub authentication: Authentication,
    pub agent_features: AgentFeatures,
    pub activity: Activity,
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
