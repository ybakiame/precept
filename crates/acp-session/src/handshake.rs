//! Reduce untrusted initialization to known bounded observations; never retain login instructions.
use crate::{AgentFeatures, AuthMethodKind};
use acp_ipc::{Error, Result};
use serde_json::{Map, Value};
use std::collections::HashSet;

pub(crate) struct Handshake {
    pub agent: String,
    pub load_session: bool,
    pub features: AgentFeatures,
    pub methods: Vec<AuthMethodKind>,
}
fn clean(s: &str, limit: usize) -> String {
    s.chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .take(limit)
        .collect()
}
fn object<'a>(v: &'a Value, key: &str) -> Result<Option<&'a Map<String, Value>>> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(o)) => Ok(Some(o)),
        _ => Err(Error::InvalidRequest),
    }
}
fn flag(o: Option<&Map<String, Value>>, key: &str) -> Result<bool> {
    match o.and_then(|o| o.get(key)) {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        _ => Err(Error::InvalidRequest),
    }
}
pub(crate) fn initialize(v: &Value) -> Result<Handshake> {
    if v.get("protocolVersion") != Some(&Value::from(1)) {
        return Err(Error::Version);
    }
    let caps = v
        .get("agentCapabilities")
        .filter(|v| v.is_object())
        .ok_or(Error::InvalidRequest)?;
    let prompt = object(caps, "promptCapabilities")?;
    let mcp = object(caps, "mcpCapabilities")?;
    let auth = object(caps, "auth")?;
    let logout = match auth.and_then(|o| o.get("logout")) {
        None | Some(Value::Null) => false,
        Some(Value::Object(_)) => true,
        _ => return Err(Error::InvalidRequest),
    };
    let features = AgentFeatures {
        image: flag(prompt, "image")?,
        audio: flag(prompt, "audio")?,
        embedded_context: flag(prompt, "embeddedContext")?,
        mcp_http: flag(mcp, "http")?,
        mcp_sse: flag(mcp, "sse")?,
        logout,
    };
    let mut methods = Vec::new();
    let mut ids = HashSet::new();
    if let Some(raw) = v.get("authMethods") {
        let raw = raw
            .as_array()
            .filter(|a| a.len() <= 8)
            .ok_or(Error::InvalidRequest)?;
        for method in raw {
            let id = method
                .get("id")
                .and_then(Value::as_str)
                .ok_or(Error::InvalidRequest)?;
            if id.is_empty()
                || id.len() > 64
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
                || !ids.insert(id)
            {
                return Err(Error::InvalidRequest);
            }
            let kind = match method.get("type") {
                None => AuthMethodKind::Agent,
                Some(Value::String(s)) if s == "agent" => AuthMethodKind::Agent,
                Some(Value::String(s)) if s == "terminal" => AuthMethodKind::Terminal,
                Some(Value::String(s)) if !s.is_empty() && s.len() <= 64 => {
                    AuthMethodKind::Unsupported
                }
                _ => return Err(Error::InvalidRequest),
            };
            methods.push(kind);
        }
    }
    let info = v.get("agentInfo").unwrap_or(&Value::Null);
    Ok(Handshake {
        agent: format!(
            "{} {}",
            clean(info["name"].as_str().unwrap_or("Unspecified agent"), 80),
            clean(info["version"].as_str().unwrap_or("Unknown"), 40)
        ),
        load_session: flag(caps.as_object(), "loadSession")?,
        features,
        methods,
    })
}
pub(crate) fn is_metadata(kind: &str) -> bool {
    matches!(
        kind,
        "available_commands_update"
            | "current_mode_update"
            | "config_option_update"
            | "usage_update"
            | "session_info_update"
    )
}
