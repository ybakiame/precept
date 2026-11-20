//! One-click session-start hooks through a real daemon: the entry written to
//! the harness's user config runs, briefs inside the project only, and goes
//! away again; Operator-only, guarded by the reported digest.
#[allow(dead_code)] // Shared harness; this suite uses part of it.
mod support;
use precept_ipc::Error;
use serde_json::{Value, json};
use std::{fs, io::Write, process::Stdio};
use support::*;

fn run(cmd: &str, cwd: &std::path::Path) -> String {
    let mut child = std::process::Command::new("sh")
        .args(["-c", cmd])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(json!({"source":"startup","cwd":cwd}).to_string().as_bytes())
        .unwrap();
    String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap()
}

#[cfg(unix)]
#[test]
fn hooks_switch_on_brief_inside_the_project_only_and_switch_off() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let status = f
        .daemon
        .control("integration.hooks", json!({"project":"alpha"}))
        .unwrap();
    assert_eq!(status["available"], true);
    let claude = status["hooks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["vendor"] == "claude")
        .unwrap()
        .clone();
    assert_eq!(claude["installed"], false);
    assert_eq!(claude["file"], "~/.claude/settings.json");

    let agent = f.daemon.credential("agent-alpha.cap");
    assert_eq!(
        f.daemon
            .call(&agent, "integration.hooks", json!({"project":"alpha"})),
        Err(Error::Unauthorized)
    );

    let on = f
        .daemon
        .control(
            "integration.hooks.apply",
            json!({"project":"alpha","vendor":"claude","enabled":true,"expected_sha256":null}),
        )
        .unwrap();
    assert_eq!(on["action"], "added");
    assert_eq!(on["installed"], true);
    // The fixture's base is the daemon user's HOME.
    let settings = f.temp.path().join(".claude/settings.json");
    let config: Value = serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    let cmd = config["hooks"]["SessionStart"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(cmd, on["command"]);

    let root = f.temp.path().join("alpha");
    let inside = run(&cmd, &root);
    let v: Value = serde_json::from_str(&inside).unwrap();
    assert!(
        v["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("# Precept: context for alpha")
    );
    assert_eq!(
        run(&cmd, &f.temp.path().join("beta")),
        "",
        "silent elsewhere"
    );

    // A stale digest is refused; the reported one removes it.
    assert_eq!(
        f.daemon.control(
            "integration.hooks.apply",
            json!({"project":"alpha","vendor":"claude","enabled":false,"expected_sha256":null}),
        ),
        Err(Error::Conflict)
    );
    let off = f
        .daemon
        .control(
            "integration.hooks.apply",
            json!({"project":"alpha","vendor":"claude","enabled":false,"expected_sha256":on["sha256"]}),
        )
        .unwrap();
    assert_eq!(off["action"], "removed");
    assert_eq!(off["installed"], false);
    assert!(
        !fs::read_to_string(&settings)
            .unwrap()
            .contains("precept-hook")
    );
    assert!(
        f.temp
            .path()
            .join(".claude/settings.json.precept.bak")
            .exists()
    );
}
