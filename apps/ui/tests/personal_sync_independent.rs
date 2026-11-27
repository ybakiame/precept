//! Independently initialized devices exchange standard age files without replacing identity.
#![cfg(feature = "native")]
mod sync_support;
use serde_json::json;
use sync_support::*;

#[test]
fn desktop_imports_without_pairing_or_replacing_the_receivers_identity() {
    let sender = Fixture::new();
    sender.prepare_profile("Remote preference value.");
    let receiver = Fixture::new();
    receiver.prepare_profile("Local preference value.");
    project_write(&sender, "profile.sync.init", json!({}));
    project_write(&receiver, "profile.sync.init", json!({}));
    let before = receiver.call("profile.sync.status", json!({})).unwrap();
    let exported = project_write(
        &sender,
        "profile.sync.export",
        json!({
            "recipient": null, "passphrase": "desktop-test-passphrase", "keys": null
        }),
    );
    let path = receiver.root.path().join("preferences.age");
    precept_ui_client::sync_files::write_envelope(&path, &exported).unwrap();
    let mut app = start_app(&receiver);
    app.presses("Personal sync").unwrap();
    app.type_text("Encrypted file", &path.to_string_lossy())
        .unwrap();
    app.type_text("Decryption password", "desktop-test-passphrase")
        .unwrap();
    app.presses("Decrypt and preview").unwrap();
    contains(&mut app, "Conflict");
    app.presses("Accept remote").unwrap();
    app.presses("Apply…").unwrap();
    contains(&mut app, "Apply selected preferences?");
    app.presses("Apply preferences").unwrap();
    contains(&mut app, "Applied 1 key(s); changed 1.");
    let after = receiver.call("profile.sync.status", json!({})).unwrap();
    assert_eq!(before["device_id"], after["device_id"]);
    assert_eq!(before["owner_fingerprint"], after["owner_fingerprint"]);
    assert_eq!(after["trusted_devices"], json!([]));
}
