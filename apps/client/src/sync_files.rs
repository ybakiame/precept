//! Bounded private files used by personal sync. No passphrase or preference
//! content is persisted by this module beyond the operator-selected output.

use precept_core::operator::{PersonalSyncEnvelope, PersonalSyncInvitation};
use precept_ipc::transport::{new_private, open_private};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    io::{Read, Write},
    path::Path,
};

pub const MAX_JSON: u64 = 1024 * 1024;
pub const MAX_PASSPHRASE: u64 = 1024;
pub const INVITATION_FORMAT: &str = "precept-personal-sync-invitation";
pub const INVITATION_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvitationFile {
    pub format: String,
    pub version: u32,
    pub invitation: PersonalSyncInvitation,
    pub invitation_fingerprint: String,
}

impl InvitationFile {
    pub fn new(invitation: PersonalSyncInvitation) -> Result<Self, &'static str> {
        let fingerprint =
            precept_sync::invitation_fingerprint(&invitation).map_err(|_| "InvalidRequest")?;
        Ok(Self {
            format: INVITATION_FORMAT.into(),
            version: INVITATION_VERSION,
            invitation,
            invitation_fingerprint: fingerprint,
        })
    }

    pub fn value(&self) -> Result<Value, &'static str> {
        serde_json::to_value(self).map_err(|_| "InvalidRequest")
    }
}

pub fn invitation_fingerprint(invitation: &PersonalSyncInvitation) -> Result<String, &'static str> {
    precept_sync::invitation_fingerprint(invitation).map_err(|_| "InvalidRequest")
}

pub fn operation_id() -> Result<String, &'static str> {
    precept_ipc::operator::operation_id().map_err(|_| "Unavailable")
}

pub fn ensure_absent(path: &Path) -> Result<(), &'static str> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err("Conflict"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Unavailable"),
    }
}

pub fn read_invitation(path: &Path) -> Result<InvitationFile, &'static str> {
    let file: InvitationFile = read_json(path)?;
    if file.format != INVITATION_FORMAT
        || file.version != INVITATION_VERSION
        || !file.invitation.valid()
        || file.invitation_fingerprint
            != precept_sync::invitation_fingerprint(&file.invitation)
                .map_err(|_| "InvalidRequest")?
    {
        return Err("InvalidRequest");
    }
    Ok(file)
}

pub fn read_envelope(path: &Path) -> Result<PersonalSyncEnvelope, &'static str> {
    let envelope: PersonalSyncEnvelope = read_json(path)?;
    if !envelope.valid() {
        return Err("InvalidRequest");
    }
    Ok(envelope)
}

pub fn read_passphrase(path: &Path) -> Result<String, &'static str> {
    let mut file = open_private_file(path)?;
    if file.metadata().map_err(|_| "Unavailable")?.len() > MAX_PASSPHRASE {
        return Err("TooLarge");
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_PASSPHRASE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "InvalidRequest")?;
    if bytes.len() as u64 > MAX_PASSPHRASE {
        return Err("TooLarge");
    }
    let mut text = String::from_utf8(bytes).map_err(|_| "InvalidRequest")?;
    if text.ends_with("\r\n") {
        text.truncate(text.len() - 2);
    } else if text.ends_with('\n') {
        text.pop();
    }
    Ok(text)
}

pub fn write_new(path: &Path, value: &Value) -> Result<(), &'static str> {
    ensure_absent(path)?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| "InvalidRequest")?;
    if bytes.len() as u64 > MAX_JSON {
        return Err("TooLarge");
    }
    let mut file = new_private(path).map_err(|_| "Unavailable")?;
    file.write_all(&bytes).map_err(|_| "Unavailable")?;
    file.sync_all().map_err(|_| "Unavailable")
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, &'static str> {
    let mut file = open_private_file(path)?;
    if file.metadata().map_err(|_| "Unavailable")?.len() > MAX_JSON {
        return Err("TooLarge");
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_JSON + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "InvalidRequest")?;
    if bytes.len() as u64 > MAX_JSON {
        return Err("TooLarge");
    }
    serde_json::from_slice(&bytes).map_err(|_| "InvalidRequest")
}

fn open_private_file(path: &Path) -> Result<std::fs::File, &'static str> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err("NotFound"),
        Err(_) => Err("Unavailable"),
        Ok(metadata) if metadata.is_file() => open_private(path, false).map_err(|_| "Unavailable"),
        Ok(_) => Err("InvalidRequest"),
    }
}

#[cfg(test)]
mod tests;
