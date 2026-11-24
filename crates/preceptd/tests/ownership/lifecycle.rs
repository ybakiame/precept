use super::*;

fn cli(f: &Fixture, args: &[&str]) -> (bool, Value) {
    let output = std::process::Command::new(exe("precept"))
        .arg("--runtime-dir")
        .arg(&f.daemon.location.runtime_dir)
        .arg("--endpoint")
        .arg(&f.daemon.location.endpoint)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.stderr.is_empty(),
        "CLI must sanitize errors on stdout"
    );
    let value = serde_json::from_slice(&output.stdout).expect("bounded CLI JSON");
    (output.status.success(), value)
}
fn cli_mutate(f: &Fixture, b: &precept_write::Binding, op: &str, args: &[&str]) -> (bool, Value) {
    let epoch = b.owner_epoch.to_string();
    let mut argv = args.to_vec();
    argv.extend([
        "--profile-id",
        &b.profile_id,
        "--generation",
        &b.generation,
        "--owner-epoch",
        &epoch,
        "--operation-id",
        op,
    ]);
    cli(f, &argv)
}
#[test]
fn native_cli_lifecycle_seven_commands_preview_cas_and_exact_retry() {
    let f = Fixture::unselected();
    assert_eq!(cli(&f, &["profile", "info"]).1["selected"], false);
    assert!(cli(&f, &["profile", "create", f.db.to_str().unwrap()]).0);
    let info = cli(&f, &["profile", "info"]).1;
    assert_eq!(info["write_enabled"], false);
    assert_eq!(
        cli(&f, &["profile", "select", f.db.to_str().unwrap()]).1["error"],
        "RestartRequired"
    );
    let b0: precept_write::Binding = serde_json::from_value(info["binding"].clone()).unwrap();
    let state = logical(&f.db);
    assert_eq!(
        cli(
            &f,
            &[
                "profile",
                "write",
                "enable",
                "--profile-id",
                &b0.profile_id,
                "--generation",
                &b0.generation
            ]
        )
        .1["error"],
        "InvalidRequest"
    );
    assert_eq!(logical(&f.db), state);
    let info = cli(
        &f,
        &[
            "profile",
            "write",
            "enable",
            "--profile-id",
            &b0.profile_id,
            "--generation",
            &b0.generation,
            "--confirm",
        ],
    );
    assert!(info.0);
    let b: precept_write::Binding = serde_json::from_value(info.1["binding"].clone()).unwrap();
    let root = f.temp.path().join("alpha").canonicalize().unwrap();
    assert!(
        cli_mutate(
            &f,
            &b,
            "cli-register",
            &["project-register", "alpha", root.to_str().unwrap()]
        )
        .0
    );
    let result = cli_mutate(
        &f,
        &b,
        "cli-suggest",
        &[
            "preference-suggest",
            "--scope",
            "project",
            "--project",
            "alpha",
            "--key",
            "editor",
            "--value",
            "synthetic",
            "--tag",
            "operator",
            "--origin",
            "agent",
        ],
    );
    assert!(result.0);
    let id = result.1["affected"][0]["record_id"].as_str().unwrap();
    let state = logical(&f.db);
    let replay = cli_mutate(
        &f,
        &b,
        "cli-suggest",
        &[
            "preference-suggest",
            "--scope",
            "project",
            "--project",
            "alpha",
            "--key",
            "editor",
            "--value",
            "synthetic",
            "--tag",
            "operator",
            "--origin",
            "agent",
        ],
    );
    assert_eq!(result, replay);
    assert_eq!(logical(&f.db), state);
    assert_eq!(
        cli_mutate(
            &f,
            &b,
            "cli-suggest",
            &[
                "preference-suggest",
                "--scope",
                "project",
                "--project",
                "alpha",
                "--key",
                "editor",
                "--value",
                "changed",
                "--tag",
                "operator",
                "--origin",
                "agent"
            ]
        )
        .1["error"],
        "IdempotencyConflict"
    );
    assert_eq!(logical(&f.db), state);
    let preview = cli(
        &f,
        &[
            "preferences",
            "get",
            id,
            "--scope",
            "project",
            "--project",
            "alpha",
        ],
    );
    assert!(preview.0);
    assert_eq!(preview.1["record"]["revision"], 1);
    assert_eq!(preview.1["record"]["status"], "suggested");
    assert_eq!(preview.1["record"]["origin"], "agent");
    let digest = preview.1["record"]["proposal_digest"].as_str().unwrap();
    assert_eq!(digest.len(), 64);
    // The operator review reads are reachable from the same bounded CLI.
    let queue = cli(
        &f,
        &["review-queue", "--scope", "project", "--project", "alpha"],
    );
    assert!(queue.0);
    assert_eq!(queue.1["records"].as_array().unwrap().len(), 1);
    assert_eq!(queue.1["records"][0]["proposal_digest"], digest);
    let inspected = cli(
        &f,
        &[
            "review-inspect",
            id,
            "--scope",
            "project",
            "--project",
            "alpha",
            "--revision",
            "1",
        ],
    );
    assert!(inspected.0);
    assert_eq!(inspected.1["record"]["proposal_digest"], digest);
    assert!(inspected.1["decisions"].as_array().unwrap().is_empty());
    assert_eq!(
        cli(
            &f,
            &[
                "review-inspect",
                id,
                "--scope",
                "project",
                "--project",
                "alpha",
                "--revision",
                "2"
            ]
        )
        .1["error"],
        "Conflict"
    );
    assert!(
        cli(&f, &["preferences", "list", "--scope", "global"]).1["records"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        cli(&f, &["preferences", "get", id, "--scope", "global"]).1["error"],
        "NotFound"
    );
    assert_eq!(
        cli(
            &f,
            &[
                "preferences",
                "list",
                "--scope",
                "project",
                "--project",
                "outside"
            ]
        )
        .1["error"],
        "Unauthorized"
    );
    assert_eq!(
        cli_mutate(
            &f,
            &b,
            "cli-no-confirm",
            &[
                "preference-confirm",
                id,
                "--scope",
                "project",
                "--project",
                "alpha",
                "--expected-revision",
                "1",
                "--proposal-digest",
                digest
            ]
        )
        .1["error"],
        "InvalidRequest"
    );
    assert_eq!(logical(&f.db), state);
    assert!(
        cli_mutate(
            &f,
            &b,
            "cli-confirm",
            &[
                "preference-confirm",
                id,
                "--scope",
                "project",
                "--project",
                "alpha",
                "--expected-revision",
                "1",
                "--proposal-digest",
                digest,
                "--confirm"
            ]
        )
        .0
    );
    let state = logical(&f.db);
    assert_eq!(
        cli_mutate(
            &f,
            &b,
            "cli-cas",
            &[
                "preference-deprecate",
                id,
                "--scope",
                "project",
                "--project",
                "alpha",
                "--expected-revision",
                "1",
                "--proposal-digest",
                digest,
                "--confirm"
            ]
        )
        .1["error"],
        "Conflict"
    );
    assert_eq!(logical(&f.db), state);
    assert!(
        cli_mutate(
            &f,
            &b,
            "cli-deprecate",
            &[
                "preference-deprecate",
                id,
                "--scope",
                "project",
                "--project",
                "alpha",
                "--expected-revision",
                "2",
                "--proposal-digest",
                digest,
                "--confirm"
            ]
        )
        .0
    );
    assert!(
        cli_mutate(
            &f,
            &b,
            "cli-delete",
            &[
                "preference-delete",
                id,
                "--scope",
                "project",
                "--project",
                "alpha",
                "--expected-revision",
                "3",
                "--proposal-digest",
                digest,
                "--confirm"
            ]
        )
        .0
    );
    let result = cli_mutate(
        &f,
        &b,
        "cli-suggest-reject",
        &[
            "preference-suggest",
            "--scope",
            "global",
            "--key",
            "editor",
            "--value",
            "synthetic",
            "--tag",
            "operator",
            "--origin",
            "operator",
        ],
    );
    assert!(result.0);
    let id = result.1["affected"][0]["record_id"].as_str().unwrap();
    let global_preview = cli(&f, &["preferences", "get", id, "--scope", "global"]).1;
    assert_eq!(global_preview["record"]["origin"], "operator");
    let global_digest = global_preview["record"]["proposal_digest"]
        .as_str()
        .unwrap();
    assert_ne!(global_digest, digest);
    assert!(
        cli_mutate(
            &f,
            &b,
            "cli-reject",
            &[
                "preference-reject",
                id,
                "--scope",
                "global",
                "--expected-revision",
                "1",
                "--proposal-digest",
                global_digest,
                "--confirm"
            ]
        )
        .0
    );
    assert!(cli(&f, &["handoff-record", "alpha", "task"]).1["record"].is_null());
    let input = f.temp.path().join("handoff.json");
    fs::write(&input,json!({"objective":"synthetic objective","completed":"","next_action":"inspect","validation_json":"[]","snapshot":{"base_sha":"","branch":"","state":"unknown","fingerprint":null}}).to_string()).unwrap();
    assert!(
        cli_mutate(
            &f,
            &b,
            "cli-handoff",
            &[
                "handoff-put",
                "alpha",
                "task",
                "--expected-absent",
                "--input",
                input.to_str().unwrap()
            ]
        )
        .0
    );
    assert_eq!(
        cli(&f, &["handoff-record", "alpha", "task"]).1["record"]["revision"],
        1
    );
    assert_eq!(
        cli(&f, &["handoff", "alpha", "task"]).1["objective"],
        "synthetic objective"
    );
    assert!(cli(&f, &["profile", "write", "disable"]).0);
    assert_eq!(cli(&f, &["profile", "info"]).1["write_enabled"], false);
    let state = logical(&f.db);
    assert!(
        !cli_mutate(
            &f,
            &b,
            "cli-disabled",
            &[
                "preference-suggest",
                "--scope",
                "global",
                "--key",
                "a",
                "--value",
                "b",
                "--tag",
                "c",
                "--origin",
                "operator"
            ]
        )
        .0
    );
    assert_eq!(logical(&f.db), state);
}
