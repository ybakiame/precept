//! Shared native operator client. No SQLite or writer dependency, no automatic retry.
use crate::{Access, Client, Error, Location, Result, load_credential};
pub use acp_core::operator::*;
use serde_json::Value;

pub async fn control(location: &Location, method: &str, params: Value) -> Result<Value> {
    // Internal fixed operator surface; never a helper for shell/remote dispatch.
    if !matches!(
        method,
        "profile.info"
            | "profile.create"
            | "profile.select"
            | "profile.write.enable"
            | "profile.write.disable"
            | "profile.preferences.list"
            | "profile.preferences.get"
            | "profile.review.queue"
            | "profile.review.count"
            | "profile.review.inspect"
            | "profile.handoff.get"
            | "profile.startup.forget"
            | "project.check"
            | "project.agents"
            | "project.mcp"
    ) {
        return Err(Error::Unsupported);
    }
    let cap = load_credential(location, &location.credential_path(None)?)?;
    if cap.access != Access::Operator {
        return Err(Error::Unauthorized);
    }
    let mut client = Client::connect(location, &cap)
        .await
        .map_err(connect_error)?;
    client.request(method, params).await
}

pub async fn mutate(location: &Location, request: &Mutation) -> Result<Value> {
    let cap = load_credential(location, &location.root()?.join("operator-write.cap"))?;
    let binding = &request.binding;
    if cap.access
        != (Access::OperatorWrite {
            profile_id: binding.profile_id.clone(),
            generation: binding.generation.clone(),
            owner_epoch: binding.owner_epoch,
        })
    {
        return Err(Error::Unauthorized);
    }
    let params = serde_json::to_value(request).map_err(|_| Error::InvalidRequest)?;
    if serde_json::to_vec(&params)
        .map_err(|_| Error::InvalidRequest)?
        .len()
        > 128 * 1024
    {
        return Err(Error::TooLarge);
    }
    let mut client = Client::connect(location, &cap)
        .await
        .map_err(connect_error)?;
    client.request("profile.mutate", params).await
}
/// Adds or removes a project in the daemon's set: an operator write, sent with
/// the current write credential (the daemon re-checks the exact grant).
pub async fn project_write(location: &Location, method: &str, params: Value) -> Result<Value> {
    if !matches!(method, "project.add" | "project.remove") {
        return Err(Error::Unsupported);
    }
    let path = location.root()?.join("operator-write.cap");
    // No write credential means writes are not enabled: a refusal, not a fault.
    if std::fs::symlink_metadata(&path).is_err() {
        return Err(Error::Unauthorized);
    }
    let cap = load_credential(location, &path)?;
    if !matches!(cap.access, Access::OperatorWrite { .. }) {
        return Err(Error::Unauthorized);
    }
    let mut client = Client::connect(location, &cap)
        .await
        .map_err(connect_error)?;
    client.request(method, params).await
}
/// Fresh random operation identity for an explicitly reviewed UI request only.
pub fn operation_id() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::Unavailable)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn connect_error(e: Error) -> Error {
    // No request was submitted. A closed/incomplete handshake is unavailable,
    // while an explicit role or OS ownership refusal remains Unauthorized/UnsafeRuntime.
    if matches!(e, Error::InvalidRequest | Error::Timeout) {
        Error::Unavailable
    } else {
        e
    }
}
