use super::support::{Fixture, logical, suggest};
use precept_ipc::Error;
use precept_write::{Command, Expected, Scope};
use rusqlite::Connection;
use serde_json::json;
#[cfg(windows)]
use std::process::Command as Process;
use std::{fs, path::Path};

#[cfg(unix)]
fn directory_link(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}
#[cfg(windows)]
fn directory_link(target: &Path, link: &Path) {
    // Directory junctions need no symlink privilege; both ends are owned fixtures.
    let output = Process::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "owned fixture junction creation refused"
    );
}

#[test]
fn s3_symlink_or_junction_swapped_root_and_foreign_registration_refuse() {
    let f = Fixture::created();
    let binding = f.daemon.enable();
    f.register(&binding, "alpha");
    let alpha = f.temp.path().join("alpha");
    let beta = f.temp.path().join("beta");
    let before = logical(&f.db);
    // A listed name cannot move to another root, and a name outside the set
    // cannot be registered (only `project.add` adds one). Neither writes.
    for (name, root, refusal) in [
        ("alpha", beta.canonicalize().unwrap(), Error::Unauthorized),
        ("outside", beta.canonicalize().unwrap(), Error::Unauthorized),
    ] {
        assert_eq!(
            f.daemon.mutate(
                &binding,
                "foreign-register",
                Command::RegisterProject {
                    name: name.into(),
                    root: root.to_str().unwrap().into(),
                    expected: Expected::Absent,
                },
            ),
            Err(refusal)
        );
    }
    fs::rename(&alpha, f.temp.path().join("alpha-original")).unwrap();
    directory_link(&beta, &alpha);
    assert_eq!(alpha.canonicalize().unwrap(), beta.canonicalize().unwrap());
    assert_eq!(
        f.daemon.control(
            "profile.review.queue",
            json!({"scope":{"kind":"project","name":"alpha"},"after_id":0}),
        ),
        Err(Error::UnsafeRuntime)
    );
    let mut proposal = suggest();
    if let Command::SuggestPreference { scope, .. } = &mut proposal {
        *scope = Scope::Project {
            name: "alpha".into(),
        };
    }
    assert_eq!(
        f.daemon.mutate(&binding, "swapped-root", proposal),
        Err(Error::UnsafeRuntime)
    );
    assert_eq!(logical(&f.db), before);
}

#[test]
fn s3_contained_identity_corruption_never_rebinds_an_existing_writer() {
    for field in ["profile_id", "generation"] {
        let f = Fixture::created();
        let binding = f.daemon.enable();
        let original = if field == "profile_id" {
            &binding.profile_id
        } else {
            &binding.generation
        };
        let replacement = if original == &"a".repeat(32) {
            "b".repeat(32)
        } else {
            "a".repeat(32)
        };
        // Explicit contained fault injection into an empty, owned synthetic
        // Profile: structurally valid identity drift, not normal writer usage.
        let db = Connection::open(&f.db).unwrap();
        db.execute(&format!("UPDATE acp_profile SET {field}=?1"), [replacement])
            .unwrap();
        drop(db);
        precept_store::SqliteStore::open(&f.db).unwrap();
        let before = logical(&f.db);
        assert_eq!(
            f.daemon.control("profile.info", json!({})),
            Err(Error::OwnerMismatch)
        );
        assert_eq!(
            f.daemon.mutate(&binding, "stale-identity", suggest()),
            Err(Error::OwnerMismatch)
        );
        assert_eq!(logical(&f.db), before);
    }
}
