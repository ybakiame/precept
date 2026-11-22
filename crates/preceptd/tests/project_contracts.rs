//! Actual production binaries, isolated Profiles/checkouts and two-tool MCP stdio.
#[path = "support/mcp.rs"]
mod mcp;
#[allow(dead_code)]
mod support;
use mcp::Mcp;
use precept_core::operator::{Channel, Origin};
use precept_write::{Binding, Command, Expected, Scope};
use serde_json::{Value, json};
use std::{fs, process::Command as Process};
use support::*;

const DIR: &str = ".precept/rules";
/// A rule file for `formatter` allowing `rustfmt`, with `extra` front matter.
fn rule(extra: &str) -> String {
    format!("+++\nkey = \"formatter\"\nallowed = [\"rustfmt\"]\n{extra}+++\nUse rustfmt.\n")
}
fn put(f: &Fixture, raw: &str) {
    let dir = f.temp.path().join("alpha").join(DIR);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("fmt.md"), raw).unwrap();
}
fn git(f: &Fixture, args: &[&str]) {
    let root = f.temp.path().join("alpha");
    let hooks = f.temp.path().join("owned-empty-hooks");
    fs::create_dir_all(&hooks).unwrap();
    assert!(
        Process::new("git")
            .arg("-c")
            .arg(format!("core.hooksPath={}", hooks.to_str().unwrap()))
            .args([
                "-c",
                "init.templateDir=",
                "-c",
                "commit.gpgSign=false",
                "-c",
                "user.name=Synthetic",
                "-c",
                "user.email=synthetic@example.invalid",
            ])
            .args(args)
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
}
/// A Git checkout with one empty commit.
fn init(f: &Fixture) {
    git(f, &["init", "-q"]);
    git(f, &["commit", "-q", "--allow-empty", "-m", "synthetic"]);
}
/// Commit the rule files: the repository's review.
fn commit(f: &Fixture) {
    git(f, &["add", DIR]);
    git(f, &["commit", "-q", "-m", "rules"]);
}
fn setup() -> (Fixture, Binding) {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.register(&b, "beta");
    (f, b)
}
fn preference(
    f: &Fixture,
    b: &Binding,
    scope: Scope,
    key: &str,
    value: &str,
    action: Option<&str>,
) -> i64 {
    let next = f
        .daemon
        .control("profile.review.queue", json!({"scope":scope,"after_id":0}))
        .unwrap()["commit_sequence"]
        .as_i64()
        .unwrap();
    let op = format!("suggest-{next}");
    let result = f
        .daemon
        .mutate(
            b,
            &op,
            Command::SuggestPreference {
                rule: Default::default(),
                scope: scope.clone(),
                key: key.into(),
                value: value.into(),
                tags: vec!["test".into()],
                origin: Origin::Agent,
                expected: Expected::Absent,
            },
        )
        .unwrap();
    let id = result["affected"][0]["record_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    if let Some(action) = action {
        let record = f
            .daemon
            .control(
                "profile.review.inspect",
                json!({"scope":scope,"id":id,"revision":1}),
            )
            .unwrap();
        let digest = record["record"]["proposal_digest"]
            .as_str()
            .unwrap()
            .to_owned();
        let command = match action {
            "reject" => Command::RejectPreference {
                id,
                scope,
                proposal_digest: digest,
                channel: Channel::Cli,
                expected: Expected::Revision(1),
                note: None,
            },
            _ => Command::ConfirmPreference {
                id,
                scope,
                proposal_digest: digest,
                channel: Channel::Cli,
                expected: Expected::Revision(1),
                note: None,
            },
        };
        f.daemon
            .mutate(b, &format!("{action}-{id}"), command)
            .unwrap();
    }
    id
}
fn context(f: &Fixture, args: Value) -> Value {
    f.daemon
        .call(&f.daemon.credential("agent-alpha.cap"), "context.get", args)
        .unwrap()
}
fn assert_safe(v: &Value) {
    let text = v.to_string();
    for secret in [
        "PRIVATE_SOURCE_65",
        "INACTIVE_65",
        "FOREIGN_65",
        "REJECTED_65",
        "DEPRECATED_65",
        "proposal_digest",
        "decisions",
        "provenance",
        "historical_validation",
    ] {
        assert!(!text.contains(secret), "private context field");
    }
    for p in v["preferences"].as_array().unwrap() {
        assert!(p.get("source").is_none());
    }
}

#[test]
fn production_ipc_and_mcp_mask_contract_conflicts_and_preserve_fallback_privacy() {
    let (f, b) = setup();
    init(&f);
    put(&f, &rule(""));
    commit(&f);
    let source = f.temp.path().join("alpha").join(DIR).join("fmt.md");
    let source_before = fs::read(&source).unwrap();
    preference(
        &f,
        &b,
        Scope::Global,
        "formatter",
        "personal-global",
        Some("confirm"),
    );
    preference(
        &f,
        &b,
        Scope::Project {
            name: "alpha".into(),
        },
        "formatter",
        "personal-project",
        Some("confirm"),
    );
    preference(&f, &b, Scope::Global, "fallback", "keep", Some("confirm"));
    preference(&f, &b, Scope::Global, "pending", "INACTIVE_65", None);
    preference(
        &f,
        &b,
        Scope::Global,
        "rejected",
        "REJECTED_65",
        Some("reject"),
    );
    let id = preference(
        &f,
        &b,
        Scope::Global,
        "deprecated",
        "DEPRECATED_65",
        Some("confirm"),
    );
    let record = f
        .daemon
        .control(
            "profile.review.inspect",
            json!({"scope":{"kind":"global"},"id":id,"revision":2}),
        )
        .unwrap();
    f.daemon
        .mutate(
            &b,
            "deprecate",
            Command::DeprecatePreference {
                id,
                scope: Scope::Global,
                proposal_digest: record["record"]["proposal_digest"].as_str().unwrap().into(),
                channel: Channel::Cli,
                expected: Expected::Revision(2),
                note: None,
            },
        )
        .unwrap();
    preference(
        &f,
        &b,
        Scope::Project {
            name: "beta".into(),
        },
        "foreign",
        "FOREIGN_65",
        Some("confirm"),
    );
    assert_eq!(
        fs::read(&source).unwrap(),
        source_before,
        "operator confirmation cannot alter repository claims"
    );
    let before = logical(&f.db);
    let mut mcp = Mcp::start(&f.daemon, "alpha");
    for v in [
        context(&f, json!({})),
        mcp.context(json!({})),
        f.daemon
            .control("context.get", json!({"project":"alpha"}))
            .unwrap(),
    ] {
        assert_safe(&v);
        assert_eq!(v["preferences"].as_array().unwrap().len(), 1);
        assert_eq!(v["preferences"][0]["key"], "fallback");
        assert_eq!(
            v["repository_rules"][0]["allowed_values"],
            json!(["rustfmt"])
        );
        assert_eq!(v["repository_rules"][0]["review"], "accepted");
        assert_eq!(v["repository_rules"][0]["text"], "Use rustfmt.");
        assert_eq!(v["contract_resolution"]["status"], "ready");
    }
    // No alpha source/contract follows the other project capability.
    let beta = f
        .daemon
        .call(
            &f.daemon.credential("agent-beta.cap"),
            "context.get",
            json!({}),
        )
        .unwrap();
    assert_eq!(beta["contract_resolution"]["status"], "not_configured");
    assert!(beta["repository_rules"].as_array().unwrap().is_empty());
    assert_eq!(logical(&f.db), before);
    let tools = mcp.request("tools/list", json!({}));
    let tools = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 11);
    assert_eq!(tools[0]["name"], "get_effective_context");
    assert_eq!(tools[1]["name"], "explain_rule");
    assert_eq!(tools[2]["name"], "get_handoff");
    assert_eq!(tools[3]["name"], "suggest_rule");
    assert_eq!(
        tools[0]["inputSchema"]["properties"]["selector"]["additionalProperties"],
        false
    );
}

#[test]
fn actual_absence_invalidity_dirty_edit_and_unreviewed_profile_text_are_distinct() {
    let (f, b) = setup();
    preference(
        &f,
        &b,
        Scope::Global,
        "formatter",
        "keep\nmultiline",
        Some("confirm"),
    );
    let root = f.temp.path().join("alpha");
    init(&f);
    let before = logical(&f.db);
    let mut mcp = Mcp::start(&f.daemon, "alpha");
    let absent = mcp.context(json!({}));
    assert_eq!(absent["contract_resolution"]["status"], "not_configured");
    assert_eq!(absent["preferences"][0]["value"], "keep\nmultiline");
    let head = absent["repository"]["head"].clone();
    assert!(head.is_string());
    put(&f, &rule(""));
    let pending = mcp.context(json!({}));
    assert_eq!(pending["preferences"][0]["value"], "keep\nmultiline");
    assert_eq!(pending["pending_rules"][0]["key"], "formatter");
    assert_eq!(pending["repository_rules"], json!([]));
    commit(&f);
    let affected = context(&f, json!({}));
    // The multiline value cannot be typed against the allowed values: withheld.
    assert_eq!(affected["contract_resolution"]["status"], "partial");
    assert_eq!(affected["preferences"], json!([]));
    assert_eq!(affected["repository_rules"][0]["key"], "formatter");
    // An uncommitted edit of a committed rule is pending again.
    put(&f, &rule("priority = 3\n"));
    let edited = context(&f, json!({}));
    assert_eq!(edited["repository_rules"], json!([]));
    assert_eq!(edited["pending_rules"][0]["key"], "formatter");
    let file = root.join(DIR).join("fmt.md");
    for bytes in [
        "malformed".to_owned(),
        "+++\nkey = \"formatter\"\nunknown = 1\n+++\nx\n".into(),
        "x".repeat(16385),
    ] {
        fs::write(&file, bytes).unwrap();
        for v in [context(&f, json!({})), mcp.context(json!({}))] {
            assert_eq!(v["contract_resolution"]["status"], "invalid_source");
            assert_eq!(v["preferences"], json!([]));
            assert_eq!(v["repository_rules"], json!([]));
            assert_eq!(
                v["contract_resolution"]["invalid_files"],
                json!([".precept/rules/fmt.md"])
            );
            assert_safe(&v);
        }
    }
    let _ = &head;
    fs::remove_dir_all(root.join(".precept")).unwrap();
    assert_eq!(
        mcp.context(json!({}))["contract_resolution"]["status"],
        "not_configured"
    );
    fs::create_dir(root.join(".precept")).unwrap();
    fs::write(root.join(DIR), "not a directory").unwrap();
    assert_eq!(
        context(&f, json!({}))["contract_resolution"]["status"],
        "invalid_source"
    );
    fs::remove_file(root.join(DIR)).unwrap();
    put(&f, &rule(""));
    #[cfg(unix)]
    let refused = {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o0)).unwrap();
        let results = [context(&f, json!({})), mcp.context(json!({}))];
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        results
    };
    #[cfg(windows)]
    let refused = {
        use std::os::windows::fs::OpenOptionsExt;
        let _exclusive = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&file)
            .unwrap();
        [context(&f, json!({})), mcp.context(json!({}))]
    };
    for v in refused {
        assert_eq!(v["contract_resolution"]["status"], "source_unavailable");
        assert_eq!(v["preferences"], json!([]));
        assert_eq!(v["repository_rules"], json!([]));
    }
    assert_eq!(logical(&f.db), before);
}

#[test]
fn real_selectors_unknown_match_nonmatch_and_malformed_never_change_authority() {
    let (f, b) = setup();
    preference(
        &f,
        &b,
        Scope::Global,
        "formatter",
        "personal",
        Some("confirm"),
    );
    init(&f);
    put(
        &f,
        &rule(
            "[applies_to]\npaths = [\"src/lib.rs\"]\nlanguages = [\"rust\"]\nagents = [\"coder\"]\ntask_tags = [\"review\"]\n",
        ),
    );
    commit(&f);
    let before = logical(&f.db);
    let mut mcp = Mcp::start(&f.daemon, "alpha");
    let matching =
        json!({"path":"src/lib.rs","language":"rust","agent_id":"coder","task_tags":["review"]});
    for args in [
        json!({}),
        json!({"selector":matching}),
        json!({"selector":{"path":"Src/lib.rs"}}),
        json!({"selector":{"task_tags":[]}}),
    ] {
        let ipc = context(&f, args.clone());
        let bridged = mcp.context(args.clone());
        assert_eq!(ipc, bridged);
        assert_safe(&ipc);
        if args["selector"]["path"] == "Src/lib.rs" {
            // A definite mismatch: the rule does not apply.
            assert_eq!(ipc["repository_rules"], json!([]));
            assert_eq!(ipc["preferences"][0]["value"], "personal");
        } else {
            // Matching, or a dimension the request leaves open.
            assert_eq!(
                ipc["repository_rules"][0]["allowed_values"],
                json!(["rustfmt"])
            );
            assert_eq!(ipc["contract_resolution"]["status"], "ready");
            assert_eq!(ipc["preferences"], json!([]));
        }
    }
    for args in [
        json!({"project":"beta"}),
        json!({"selector":{"project":"beta"}}),
        json!({"selector":{"root":"/other"}}),
        json!({"selector":{"path":"../bad"}}),
        json!({"selector":null}),
        json!({"selector":{"language":""}}),
        json!({"selector":{"unknown":true}}),
    ] {
        assert_eq!(
            f.daemon.call(
                &f.daemon.credential("agent-alpha.cap"),
                "context.get",
                args.clone()
            ),
            Err(precept_ipc::Error::InvalidRequest)
        );
        assert_eq!(
            mcp.request(
                "tools/call",
                json!({"name":"get_effective_context","arguments":args})
            )["result"]["isError"],
            true
        );
    }
    assert_eq!(logical(&f.db), before);
}

#[test]
fn a_confirmed_convention_written_and_committed_becomes_the_repository_rule() {
    let (f, b) = setup();
    init(&f);
    let alpha = Scope::Project {
        name: "alpha".into(),
    };
    let id = preference(
        &f,
        &b,
        alpha,
        "commit-style",
        "Small commits.",
        Some("confirm"),
    );
    let personal = preference(&f, &b, Scope::Global, "personal", "x", Some("confirm"));
    // Only a confirmed convention of this project can be written.
    assert_eq!(
        f.daemon
            .control("rules.export", json!({"project":"alpha","id":personal})),
        Err(precept_ipc::Error::NotFound)
    );
    assert_eq!(
        f.daemon.call(
            &f.daemon.credential("agent-alpha.cap"),
            "rules.export.apply",
            json!({"project":"alpha","id":id,"enabled":true})
        ),
        Err(precept_ipc::Error::Unauthorized)
    );
    let preview = f
        .daemon
        .control("rules.export", json!({"project":"alpha","id":id}))
        .unwrap();
    assert_eq!(preview["state"], "absent");
    assert_eq!(preview["path"], ".precept/rules/commit-style.md");
    let written = f
        .daemon
        .control(
            "rules.export.apply",
            json!({"project":"alpha","id":id,"enabled":true,"expected_sha256":null}),
        )
        .unwrap();
    assert_eq!(written["action"], "created");
    let file = f.temp.path().join("alpha/.precept/rules/commit-style.md");
    assert_eq!(fs::read_to_string(&file).unwrap(), preview["text"]);
    let pending = context(&f, json!({}));
    assert_eq!(pending["pending_rules"][0]["key"], "commit-style");
    assert!(
        pending["preferences"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["key"] == "commit-style")
    );
    commit(&f);
    let served = context(&f, json!({}));
    assert_eq!(served["repository_rules"][0]["key"], "commit-style");
    assert_eq!(served["repository_rules"][0]["text"], "Small commits.");
    assert!(
        served["preferences"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["key"] != "commit-style")
    );
    // Writing again is a no-op; nothing is overwritten.
    let same = f
        .daemon
        .control("rules.export", json!({"project":"alpha","id":id}))
        .unwrap();
    assert_eq!(same["state"], "same");
}
