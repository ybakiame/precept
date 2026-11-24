//! Real Desktop components and private files over a real daemon; not native GUI evidence.
#![cfg(feature = "native")]

mod sync_support;

use serde_json::{Value, json};
use std::io::Write;
use sync_support::*;

#[test]
fn desktop_creates_an_identity_and_exports_an_encrypted_file() {
    let fixture = Fixture::new();
    fixture.prepare_profile("Synchronize this confirmed preference.");
    let mut app = start_app(&fixture);
    app.press("Personal sync").unwrap();
    contains(&mut app, "Not initialized");
    app.press("Create identity…").unwrap();
    contains(&mut app, "Initialized");
    assert_eq!(
        fixture.call("profile.sync.status", json!({})).unwrap()["initialized"],
        true
    );

    let passphrase = fixture.root.path().join("passphrase.txt");
    let envelope = fixture.root.path().join("sync.json");
    let mut file = precept_ipc::transport::new_private(&passphrase).unwrap();
    file.write_all(b"desktop-test-passphrase\n").unwrap();
    file.sync_all().unwrap();

    app.press("Export encrypted file…").unwrap();
    app.press("Passphrase").unwrap();
    app.type_text("Passphrase file", &passphrase.to_string_lossy())
        .unwrap();
    app.type_text("Output file", &envelope.to_string_lossy())
        .unwrap();
    assert_eq!(
        app.screen().input_text("Passphrase file").unwrap(),
        Some(passphrase.to_string_lossy().as_ref())
    );
    assert_eq!(
        app.screen().input_text("Output file").unwrap(),
        Some(envelope.to_string_lossy().as_ref())
    );
    app.press("Export file").unwrap();
    wait_for_file(&mut app, &envelope);
    contains(&mut app, "Trusted devices");
    let exported: Value = serde_json::from_slice(&std::fs::read(&envelope).unwrap()).unwrap();
    assert_eq!(exported["format"], "precept-personal-sync");
    assert!(!exported.to_string().contains("desktop-test-passphrase"));
}
