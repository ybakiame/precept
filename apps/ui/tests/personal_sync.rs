//! Real Desktop forms and standard age files over a real daemon; not native GUI evidence.
#![cfg(feature = "native")]
mod sync_support;
use serde_json::json;
use sync_support::*;

#[test]
fn desktop_exports_an_age_file_without_a_separate_identity_setup() {
    let fixture = Fixture::new();
    fixture.prepare_profile("Synchronize this confirmed preference.");
    let mut app = start_app(&fixture);
    app.presses("Personal sync").unwrap();
    contains(&mut app, "Export encrypted preferences");
    contains(&mut app, "Import and merge preferences");
    let output = fixture.root.path().join("preferences.age");
    app.type_text("Encryption password", "desktop-test-passphrase")
        .unwrap();
    app.type_text("Output file", &output.to_string_lossy())
        .unwrap();
    app.presses("Export file").unwrap();
    wait_for_file(&mut app, &output);
    let bytes = std::fs::read(&output).unwrap();
    assert!(bytes.starts_with(b"-----BEGIN AGE ENCRYPTED FILE-----"));
    assert!(!String::from_utf8_lossy(&bytes).contains("desktop-test-passphrase"));
    assert!(!String::from_utf8_lossy(&bytes).contains("Synchronize this confirmed preference."));
    assert_eq!(
        fixture.call("profile.sync.status", json!({})).unwrap()["initialized"],
        true
    );
}
