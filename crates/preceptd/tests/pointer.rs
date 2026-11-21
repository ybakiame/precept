//! The AGENTS.md pointer through a real daemon: Operator-only, guarded by the
//! previewed digest, preserves the repository's own text, reversible.
#[allow(dead_code)] // Shared harness; this suite uses part of it.
mod support;
use precept_ipc::Error;
use serde_json::json;
use std::fs;
use support::*;

#[test]
fn pointer_is_previewed_guarded_reversible_and_operator_only() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let file = f.temp.path().join("alpha/AGENTS.md");
    let original = fs::read_to_string(&file).unwrap();

    let status = f
        .daemon
        .control("integration.pointer", json!({"project":"alpha"}))
        .unwrap();
    assert_eq!(status["state"], "absent");
    assert!(
        status["block"]
            .as_str()
            .unwrap()
            .contains("on the `precept` MCP server")
    );
    let digest = status["sha256"].as_str().unwrap().to_owned();

    // Agents cannot read or write it; unknown projects and bad digests refused.
    let agent = f.daemon.credential("agent-alpha.cap");
    assert_eq!(
        f.daemon
            .call(&agent, "integration.pointer", json!({"project":"alpha"})),
        Err(Error::Unauthorized)
    );
    assert_eq!(
        f.daemon
            .control("integration.pointer", json!({"project":"beta"})),
        Err(Error::NotFound)
    );
    assert_eq!(
        f.daemon.control(
            "integration.pointer.apply",
            json!({"project":"alpha","enabled":true,"expected_sha256":"zz"})
        ),
        Err(Error::InvalidRequest)
    );
    // A file that changed since the preview is never overwritten.
    fs::write(&file, format!("{original}Edited after preview.\n")).unwrap();
    assert_eq!(
        f.daemon.control(
            "integration.pointer.apply",
            json!({"project":"alpha","enabled":true,"expected_sha256":digest})
        ),
        Err(Error::Conflict)
    );
    fs::write(&file, &original).unwrap();

    let added = f
        .daemon
        .control(
            "integration.pointer.apply",
            json!({"project":"alpha","enabled":true,"expected_sha256":digest}),
        )
        .unwrap();
    assert_eq!(added["action"], "updated");
    assert_eq!(added["state"], "current");
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.starts_with(&original), "the repository's text is kept");
    assert!(text.contains("<!-- precept:begin v1 -->"));

    let removed = f
        .daemon
        .control(
            "integration.pointer.apply",
            json!({"project":"alpha","enabled":false,"expected_sha256":added["sha256"]}),
        )
        .unwrap();
    assert_eq!(removed["action"], "removed");
    assert_eq!(fs::read_to_string(&file).unwrap(), original);
}
