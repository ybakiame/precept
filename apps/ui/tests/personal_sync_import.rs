//! Real Desktop conflict review over independent daemons; not native GUI evidence.
#![cfg(feature = "native")]

mod sync_support;

use precept_ipc::operator;
use serde_json::json;
use sync_support::*;

#[test]
fn desktop_requires_an_explicit_conflict_choice_and_excludes_reapplied_entries() {
    let sender = Fixture::new();
    sender.prepare_profile("Remote preference value.");
    let receiver = Fixture::new();
    receiver.prepare_profile("Different local preference value.");
    project_write(&sender, "profile.sync.init", json!({}));
    project_write(&receiver, "profile.sync.init", json!({}));

    let envelope = receiver.root.path().join("preferences.age");
    let exported = project_write(
        &sender,
        "profile.sync.export",
        json!(operator::PersonalSyncExportRequest {
            recipient: None,
            passphrase: Some("desktop-test-passphrase".into()),
            keys: None,
        }),
    );
    precept_ui_client::sync_files::write_envelope(&envelope, &exported).unwrap();

    let mut app = start_app(&receiver);
    app.press("Personal sync").unwrap();
    app.type_text("Encrypted file", &envelope.to_string_lossy())
        .unwrap();
    app.type_text("Decryption password", "desktop-test-passphrase")
        .unwrap();
    app.press("Decrypt and preview").unwrap();
    contains(&mut app, "Conflict");
    contains(&mut app, "0 key(s) selected");
    app.press("Accept remote").unwrap();
    contains(&mut app, "1 key(s) selected");
    app.press("Apply…").unwrap();
    contains(&mut app, "Apply selected preferences?");
    app.press("Apply preferences").unwrap();
    contains(&mut app, "Applied 1 key(s); changed 1.");
    assert_eq!(app.screen().count("Apply…").unwrap(), 0);

    assert!(
        !app.screen()
            .texts()
            .iter()
            .any(|text| text.contains("Remote preference value.")),
        "terminal success drops the decrypted preview"
    );

    assert_eq!(
        app.screen().input_text("Encrypted file").unwrap(),
        Some(envelope.to_string_lossy().as_ref())
    );
    app.type_text("Decryption password", "desktop-test-passphrase")
        .unwrap();
    app.press("Decrypt and preview").unwrap();
    contains(&mut app, "Already applied");
    contains(&mut app, "0 key(s) selected");
}
