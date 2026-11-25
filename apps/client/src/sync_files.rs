//! Bounded standard age files selected by the operator. Only ciphertext is read
//! here; decryption and Profile mutation remain daemon-owned.
use precept_core::operator::PersonalSyncEnvelope;
use precept_ipc::transport::new_private;
use serde_json::Value;
use std::{
    io::{Read, Write},
    path::Path,
};

pub const MAX_JSON: u64 = 1024 * 1024;

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

pub fn read_envelope(path: &Path) -> Result<PersonalSyncEnvelope, &'static str> {
    let mut file = precept_ipc::transport::open_ciphertext(path).map_err(|error| match error {
        precept_ipc::Error::NotFound => "NotFound",
        precept_ipc::Error::InvalidRequest => "InvalidRequest",
        _ => "Unavailable",
    })?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_JSON + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Unavailable")?;
    if bytes.len() as u64 > MAX_JSON {
        return Err("TooLarge");
    }
    precept_sync::envelope_from_age(&bytes).map_err(|_| "InvalidRequest")
}

pub fn write_envelope(path: &Path, value: &Value) -> Result<(), &'static str> {
    ensure_absent(path)?;
    let envelope: PersonalSyncEnvelope =
        serde_json::from_value(value.clone()).map_err(|_| "InvalidRequest")?;
    if !envelope.valid() {
        return Err("InvalidRequest");
    }
    let mut file = new_private(path).map_err(|_| "Unavailable")?;
    file.write_all(envelope.ciphertext.as_bytes())
        .map_err(|_| "Unavailable")?;
    file.sync_all().map_err(|_| "Unavailable")
}

#[cfg(test)]
mod tests;
