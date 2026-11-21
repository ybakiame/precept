//! Importing rules a repository already writes down, through a real daemon:
//! the scan reads only the project's own instruction files and rule folders,
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
    fs::create_dir_all(root.join(".agent-control/rules")).unwrap();
    fs::write(
        root.join(".agent-control/rules/style.md"),
        "---\nid: personal-style\ntitle: Short answers\nscope: global\npriority: 3\ntags:\n- tone\n---\nKeep answers short.\n",
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
    assert_eq!(keys, ["agents-commits", "agents-testing", "personal-style"]);
    assert_eq!(
        candidates[0]["scope"],
        json!({"kind":"project","name":"alpha"})
    );
    assert_eq!(candidates[2]["scope"], json!({"kind":"global"}));
    assert_eq!(candidates[2]["rule"]["priority"], 3);
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
                items: vec![item(&candidates[0]), item(&candidates[2])],
            },
        )
        .unwrap();
    assert_eq!(receipt["affected"].as_array().unwrap().len(), 2);
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
    assert_eq!(queue(Scope::Global)[0]["key"], "personal-style");
    let again = scan();
    let exists: Vec<bool> = again["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["exists"].as_bool().unwrap())
        .collect();
    assert_eq!(exists, [true, false, true]);
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
