//! Actual production binaries, isolated Profiles/checkouts and two-tool MCP stdio.
#[path = "support/mcp.rs"]
mod mcp;
#[allow(dead_code)]
mod support;
use acp_core::operator::{Channel, Origin};
use acp_write::{Binding, Command, Expected, Scope};
use mcp::Mcp;
use serde_json::{Value, json};
use std::{fs, process::Command as Process};
use support::*;

const SOURCE: &str = "ACP.contracts.md";
fn rule(review: &str) -> Value {
    json!({"id":"fmt","revision":1,"review":review,"key":"formatter","allowed_values":["rustfmt"]})
}
fn put(f: &Fixture, rules: Vec<Value>) {
    fs::write(
        f.temp.path().join("alpha").join(SOURCE),
        format!(
            "```acp-contracts-v1\n{}\n```\n",
            json!({"schema_version":1,"rules":rules})
        ),
    )
    .unwrap();
}
fn setup() -> (Fixture, Binding) {
    let f = Fixture::created(false);
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
            },
            _ => Command::ConfirmPreference {
                id,
                scope,
                proposal_digest: digest,
                channel: Channel::Cli,
                expected: Expected::Revision(1),
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
    put(&f, vec![rule("accepted")]);
    let source_before = fs::read(f.temp.path().join("alpha").join(SOURCE)).unwrap();
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
        fs::read(f.temp.path().join("alpha").join(SOURCE)).unwrap(),
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
            v["effective_contracts"][0]["allowed_values"],
            json!(["rustfmt"])
        );
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
    assert!(beta["effective_contracts"].as_array().unwrap().is_empty());
    assert_eq!(logical(&f.db), before);
    let tools = mcp.request("tools/list", json!({}));
    let tools = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "get_effective_context");
    assert_eq!(tools[1]["name"], "get_handoff");
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
    assert!(
        Process::new("git")
            .args(["-c", "init.templateDir=", "init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
    let hooks = root.join("owned-empty-hooks");
    fs::create_dir(&hooks).unwrap();
    assert!(
        Process::new("git")
            .arg("-c")
            .arg(format!("core.hooksPath={}", hooks.to_str().unwrap()))
            .args([
                "-c",
                "commit.gpgSign=false",
                "-c",
                "user.name=Synthetic",
                "-c",
                "user.email=synthetic@example.invalid",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "synthetic"
            ])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
    let before = logical(&f.db);
    let mut mcp = Mcp::start(&f.daemon, "alpha");
    let absent = mcp.context(json!({}));
    assert_eq!(absent["contract_resolution"]["status"], "not_configured");
    assert_eq!(absent["preferences"][0]["value"], "keep\nmultiline");
    let head = absent["repository"]["head"].clone();
    assert!(head.is_string());
    put(&f, vec![rule("unreviewed")]);
    let pending = mcp.context(json!({}));
    assert_eq!(pending["preferences"][0]["value"], "keep\nmultiline");
    put(&f, vec![rule("accepted")]);
    let affected = context(&f, json!({}));
    assert_eq!(affected["contract_resolution"]["status"], "partial");
    assert_eq!(affected["preferences"], json!([]));
    let file = root.join(SOURCE);
    for bytes in [
        "malformed".to_owned(),
        "```acp-contracts-v2\n{}\n```\n".into(),
        "x".repeat(16385),
    ] {
        fs::write(&file, bytes).unwrap();
        for v in [context(&f, json!({})), mcp.context(json!({}))] {
            assert_eq!(v["contract_resolution"]["status"], "invalid_source");
            assert_eq!(v["preferences"], json!([]));
            assert_eq!(v["effective_contracts"], json!([]));
            assert_eq!(v["repository"]["head"], head);
            assert_safe(&v);
        }
    }
    fs::remove_file(&file).unwrap();
    assert_eq!(
        mcp.context(json!({}))["contract_resolution"]["status"],
        "not_configured"
    );
    fs::create_dir(&file).unwrap();
    assert_eq!(
        context(&f, json!({}))["contract_resolution"]["status"],
        "invalid_source"
    );
    fs::remove_dir(&file).unwrap();
    put(&f, vec![rule("accepted")]);
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
        assert_eq!(v["effective_contracts"], json!([]));
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
    let mut r = rule("accepted");
    r["selector"] = json!({"paths":["src/lib.rs"],"languages":["rust"],"agents":["coder"],"task_tags":["review"]});
    put(&f, vec![r]);
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
        if args.get("selector").is_none() {
            assert_eq!(ipc["contract_resolution"]["status"], "partial");
        } else if args["selector"]["language"] == "rust" {
            assert_eq!(
                ipc["effective_contracts"][0]["allowed_values"],
                json!(["rustfmt"])
            );
        } else {
            assert_eq!(ipc["preferences"][0]["value"], "personal");
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
            Err(acp_ipc::Error::InvalidRequest)
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
