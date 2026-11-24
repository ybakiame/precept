//! Real Desktop pairing with independently entered fingerprints; not native GUI evidence.
#![cfg(feature = "native")]

mod sync_support;

use precept_ipc::operator;
use serde_json::json;
use sync_support::*;

#[test]
fn desktop_pairs_an_uninitialized_device_with_verified_fingerprints() {
    let sender = Fixture::new();
    sender.prepare_profile("Remote preference value.");
    let receiver = Fixture::new();
    receiver.prepare_profile("Local preference value.");
    project_write(&sender, "profile.sync.init", json!({}));
    let invitation: operator::PersonalSyncInvitation =
        serde_json::from_value(sender.call("profile.sync.invitation", json!({})).unwrap()).unwrap();
    let owner_fingerprint = invitation.owner_fingerprint.clone();
    let invitation_fingerprint =
        precept_ui_client::sync_files::invitation_fingerprint(&invitation).unwrap();
    let path = receiver.root.path().join("invitation.json");
    let file = precept_ui_client::sync_files::InvitationFile::new(invitation).unwrap();
    precept_ui_client::sync_files::write_new(&path, &file.value().unwrap()).unwrap();

    let mut app = start_app(&receiver);
    app.press("Personal sync").unwrap();
    contains(&mut app, "Not initialized");
    app.press("Pair device…").unwrap();
    app.type_text("Invitation file", &path.to_string_lossy())
        .unwrap();
    app.type_text("Expected owner fingerprint", &owner_fingerprint)
        .unwrap();
    app.type_text("Expected invitation fingerprint", &invitation_fingerprint)
        .unwrap();
    assert_eq!(
        app.screen().input_text("Invitation file").unwrap(),
        Some(path.to_string_lossy().as_ref())
    );
    assert_eq!(
        app.screen()
            .input_text("Expected owner fingerprint")
            .unwrap(),
        Some(owner_fingerprint.as_str())
    );
    assert_eq!(
        app.screen()
            .input_text("Expected invitation fingerprint")
            .unwrap(),
        Some(invitation_fingerprint.as_str())
    );
    press_when_ready(&mut app, "Pair / trust");
    contains(&mut app, "Initialized");
    assert_eq!(
        receiver.call("profile.sync.status", json!({})).unwrap()["trusted_devices"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
