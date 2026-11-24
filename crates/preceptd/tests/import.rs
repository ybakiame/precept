//! Importing the rules a repository's AGENTS.md already writes down, through a
//! real daemon: the scan reads only that file,
//! the operator's picks become ordinary suggestions in one write, and a
//! second scan marks them as already in the Profile.
#[allow(dead_code)] // Shared harness; this suite uses part of it.
mod support;
use precept_core::operator::{Origin, SuggestItem};
use precept_ipc::Error;
use precept_write::{Command, Scope};
use serde_json::{Value, json};
use std::fs;
use support::*;

#[test]
fn a_scan_suggests_and_the_operator_files_picks_as_suggestions() {
    let mut f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let root = f.temp.path().join("alpha");
    fs::write(
        root.join("AGENTS.md"),
        "# Alpha\n\n## Commits\n\nUse conventional commits.\n\n## Testing\n\nRun the gates.\n",
    )
    .unwrap();
    let scan = || {
        f.daemon
            .control("import.scan", json!({"project":"alpha"}))
            .unwrap()
    };
    let found = scan();
    let candidates = found["candidates"].as_array().unwrap().clone();
    let keys: Vec<&str> = candidates
        .iter()
        .map(|c| c["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["agents-commits", "agents-testing"]);
    assert_eq!(
        candidates[0]["scope"],
        json!({"kind":"project","name":"alpha"})
    );
    assert!(candidates.iter().all(|c| c["exists"] == false));
    // Agents cannot scan.
    let agent = f.daemon.credential("agent-alpha.cap");
    assert_eq!(
        f.daemon
            .call(&agent, "import.scan", json!({"project":"alpha"})),
        Err(Error::Unauthorized)
    );

    // The operator picks two; they arrive as suggestions in one write.
    let item = |c: &Value| SuggestItem {
        scope: serde_json::from_value(c["scope"].clone()).unwrap(),
        key: c["key"].as_str().unwrap().into(),
        value: c["value"].as_str().unwrap().into(),
        tags: serde_json::from_value(c["tags"].clone()).unwrap(),
        origin: Origin::Operator,
        rule: serde_json::from_value(c["rule"].clone()).unwrap(),
    };
    let receipt = f
        .daemon
        .mutate(
            &b,
            "import-1",
            Command::SuggestBatch {
                items: vec![item(&candidates[0])],
            },
        )
        .unwrap();
    assert_eq!(receipt["affected"].as_array().unwrap().len(), 1);
    let queue = |scope: Scope| {
        f.daemon
            .control("profile.review.queue", json!({"scope":scope,"after_id":0}))
            .unwrap()["records"]
            .as_array()
            .unwrap()
            .clone()
    };
    let project = queue(Scope::Project {
        name: "alpha".into(),
    });
    assert_eq!(project.len(), 1);
    assert_eq!(project[0]["status"], "suggested");
    assert_eq!(project[0]["tags"], json!(["agents-md", "imported"]));
    let again = scan();
    let exists: Vec<bool> = again["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["exists"].as_bool().unwrap())
        .collect();
    assert_eq!(exists, [true, false]);
    // An empty batch is malformed.
    assert_eq!(
        f.daemon
            .mutate(&b, "import-empty", Command::SuggestBatch { items: vec![] })
            .err(),
        Some(Error::InvalidRequest)
    );
    f.daemon.kill();
    f.daemon = Daemon::relaunch(f.temp.path(), "run-b");
    f.daemon.ready();
    f.daemon
        .control("profile.select", json!({"path":f.db}))
        .expect("imported suggestions verify");
}

#[test]
fn known_sources_are_project_scoped_read_only_and_suggestion_only() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.register(&b, "beta");
    let alpha = f.temp.path().join("alpha");
    let beta = f.temp.path().join("beta");
    let sources = [
        ("AGENTS.md", "## Shared\n\nAlpha shared instructions.\n"),
        ("CLAUDE.md", "## Claude\n\nAlpha Claude instructions.\n"),
        (".cursorrules", "Legacy Cursor instructions.\n"),
        (
            ".cursor/rules/style.mdc",
            "---\nalwaysApply: true\n---\nCursor style instructions.\n",
        ),
        (".continue/rules/test.md", "Continue test instructions.\n"),
        (
            ".github/copilot-instructions.md",
            "Copilot repository instructions.\n",
        ),
        (
            ".github/instructions/api.instructions.md",
            "---\napplyTo: src/\n---\nCopilot API instructions.\n",
        ),
        (
            ".claude/rules/rust.md",
            "---\npaths: crates/\n---\nClaude Rust instructions.\n",
        ),
        (
            ".cursor/rules/glob.mdc",
            "---\nglobs: src/**/*.rs\n---\nUnsupported glob instructions.\n",
        ),
    ];
    for (source, text) in sources {
        let path = alpha.join(source);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fs::write(
        beta.join("AGENTS.md"),
        "## Beta\n\nBeta only instructions.\n",
    )
    .unwrap();

    let before = logical(&f.db);
    let found = f
        .daemon
        .control("import.scan", json!({"project":"alpha"}))
        .unwrap();
    assert_eq!(logical(&f.db), before, "scan is read-only");
    let candidates = found["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 8);
    assert!(candidates.iter().all(|c| {
        c["scope"] == json!({"kind":"project","name":"alpha"}) && c["exists"] == false
    }));
    let kinds: Vec<&str> = candidates
        .iter()
        .map(|c| c["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "agents_md",
            "claude_md",
            "cursor",
            "copilot",
            "cursor",
            "continue",
            "copilot",
            "claude"
        ]
    );
    assert!(found["skipped"].as_array().unwrap().iter().any(|s| {
        s["source"] == ".cursor/rules/glob.mdc"
            && s["reason"] == "unsupported_applicability"
            && s["detail"] == "src/**/*.rs"
    }));

    let agent = f.daemon.credential("agent-alpha.cap");
    for credential in [&agent, &f.daemon.credential("operator-write.cap")] {
        assert_eq!(
            f.daemon
                .call(credential, "import.scan", json!({"project":"alpha"})),
            Err(Error::Unauthorized)
        );
    }
    assert_eq!(
        f.daemon
            .control("import.scan", json!({"project":"missing"})),
        Err(Error::NotFound)
    );
    assert_eq!(
        f.daemon
            .control("import.scan", json!({"project":"alpha","root":alpha})),
        Err(Error::InvalidRequest)
    );

    let other = f
        .daemon
        .control("import.scan", json!({"project":"beta"}))
        .unwrap();
    let other = other.to_string();
    assert!(other.contains("Beta only instructions."));
    assert!(!other.contains("Alpha shared instructions."));
    assert!(!other.contains(".cursor/rules/style.mdc"));

    let cursor = candidates
        .iter()
        .find(|c| c["source"] == ".cursor/rules/style.mdc")
        .unwrap();
    let item = SuggestItem {
        scope: serde_json::from_value(cursor["scope"].clone()).unwrap(),
        key: cursor["key"].as_str().unwrap().into(),
        value: cursor["value"].as_str().unwrap().into(),
        tags: serde_json::from_value(cursor["tags"].clone()).unwrap(),
        origin: Origin::Operator,
        rule: serde_json::from_value(cursor["rule"].clone()).unwrap(),
    };
    f.daemon
        .mutate(
            &b,
            "import-cursor",
            Command::SuggestBatch { items: vec![item] },
        )
        .unwrap();
    let context = f.daemon.call(&agent, "context.get", json!({})).unwrap();
    assert!(!context.to_string().contains("Cursor style instructions."));
    let again = f
        .daemon
        .control("import.scan", json!({"project":"alpha"}))
        .unwrap();
    assert!(
        again["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| { c["source"] == ".cursor/rules/style.mdc" && c["exists"] == true })
    );
}
