//! `precept-hook` against a real daemon: the session-start briefing in each
//! harness's shape, the compaction variant, the activity entry, and fail-open
//! on a refused credential.
#[allow(dead_code)] // Shared harness; this suite uses part of it.
mod support;
use precept_core::operator::{Channel, Origin};
use precept_write::{Command, Expected, Scope};
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command as Process, Stdio};
use support::*;

fn hook(f: &Fixture, vendor: &str, cap: &std::path::Path, stdin: &str) -> (bool, String, String) {
    hook_event(f, vendor, cap, stdin, None)
}

fn hook_event(
    f: &Fixture,
    vendor: &str,
    cap: &std::path::Path,
    stdin: &str,
    event: Option<&str>,
) -> (bool, String, String) {
    let mut cmd = Process::new(exe("precept-hook"));
    cmd.arg("--runtime-dir")
        .arg(&f.daemon.location.runtime_dir)
        .arg("--endpoint")
        .arg(&f.daemon.location.endpoint)
        .args([
            "--project",
            "alpha",
            "--vendor",
            vendor,
            "--capability-file",
        ])
        .arg(cap);
    if let Some(event) = event {
        cmd.args(["--event", event]);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.success(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn session_start_briefing_reaches_each_harness_shape_and_fails_open() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let receipt = f
        .daemon
        .mutate(
            &b,
            "hook-suggest",
            Command::SuggestPreference {
                rule: Default::default(),
                scope: Scope::Global,
                key: "commit-style".into(),
                value: "Conventional commits, imperative mood.".into(),
                tags: vec!["git".into()],
                origin: Origin::Operator,
                expected: Expected::Absent,
            },
        )
        .unwrap();
    let id: i64 = receipt["affected"][0]["record_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let preview = f
        .daemon
        .control(
            "profile.preferences.get",
            json!({"scope":Scope::Global,"id":id}),
        )
        .unwrap();
    f.daemon
        .mutate(
            &b,
            "hook-confirm",
            Command::ConfirmPreference {
                id,
                scope: Scope::Global,
                proposal_digest: preview["record"]["proposal_digest"]
                    .as_str()
                    .unwrap()
                    .into(),
                channel: Channel::Desktop,
                expected: Expected::Revision(1),
                note: None,
            },
        )
        .unwrap();
    let cap = f.daemon.location.runtime_dir.join("agent-alpha.cap");

    let (ok, out, _) = hook(&f, "claude", &cap, r#"{"source":"startup"}"#);
    assert!(ok);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "SessionStart");
    let text = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(text.contains("# Precept: context for alpha"));
    assert!(text.contains("### commit-style"));
    assert!(text.contains("Conventional commits"));
    assert!(!text.contains("compacted"));

    let (ok, out, _) = hook(&f, "codex", &cap, r#"{"source":"compact"}"#);
    assert!(ok);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(
        v["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("was compacted")
    );

    let (ok, out, _) = hook(&f, "qoder", &cap, r#"{"source":"startup"}"#);
    assert!(ok);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "SessionStart");
    assert!(
        v["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("# Precept")
    );

    let (ok, out, _) = hook(&f, "cursor", &cap, "");
    assert!(ok);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(
        v["additional_context"]
            .as_str()
            .unwrap()
            .contains("commit-style")
    );

    // The daemon's activity names each hook.
    let events = f
        .daemon
        .control("activity.list", json!({"project":"alpha"}))
        .unwrap()["events"]
        .clone();
    let clients: Vec<&str> = events
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["client"].as_str())
        .collect();
    assert_eq!(
        clients,
        [
            "cursor hook",
            "qoder hook",
            "codex hook",
            "claude-code hook"
        ]
    );

    // Unshared project: no briefing, exit 0, one stderr line, nothing on stdout.
    f.daemon
        .control("project.agents", json!({"name":"alpha","enabled":false}))
        .unwrap();
    let (ok, out, err) = hook(&f, "claude", &cap, "{}");
    assert!(ok, "fail-open");
    assert!(out.is_empty());
    assert!(err.starts_with("precept-hook: no briefing"));
    assert!(!err.contains("Conventional"));
}

#[test]
fn pre_compact_reminds_the_agent_to_keep_its_work_session_current() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.daemon
        .control("project.propose", json!({"name":"alpha","enabled":true}))
        .unwrap();
    let agent = f.daemon.credential("agent-alpha.cap");
    let session = f
        .daemon
        .call(&agent, "session.start", json!({"goal":"Ship it"}))
        .unwrap();
    let id = session["id"].as_str().unwrap();
    let cap = f.daemon.location.runtime_dir.join("agent-alpha.cap");
    let stdin = r#"{"hook_event_name":"PreCompact","trigger":"auto"}"#;
    let (ok, out, _) = hook_event(&f, "qoder", &cap, stdin, Some("pre-compact"));
    assert!(ok);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "PreCompact");
    let text = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(text.contains(id) && text.contains("expected_rev"), "{text}");
    assert!(text.contains("get_effective_context"));
    // Claude Code reads the same shape; the other harnesses get nothing.
    let (ok, out, _) = hook_event(&f, "claude", &cap, stdin, Some("pre-compact"));
    assert!(ok);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "PreCompact");
    let (ok, out, _) = hook_event(&f, "cursor", &cap, stdin, Some("pre-compact"));
    assert!(ok);
    assert!(out.is_empty());
    // The entry is the operator's to add, and only where the harness has the
    // event.
    assert_eq!(
        f.daemon.control(
            "integration.hooks.apply",
            json!({"project":"alpha","vendor":"codex","event":"pre_compact","enabled":true,
                "expected_sha256":null}),
        ),
        Err(precept_ipc::Error::Unsupported)
    );
    let on = f
        .daemon
        .control(
            "integration.hooks.apply",
            json!({"project":"alpha","vendor":"qoder","event":"pre_compact","enabled":true,
                "expected_sha256":null}),
        )
        .unwrap();
    assert_eq!(on["installed"], true);
    assert!(
        on["command"]
            .as_str()
            .unwrap()
            .ends_with("--event pre-compact")
    );
    let status = f
        .daemon
        .control("integration.hooks", json!({"project":"alpha"}))
        .unwrap();
    assert_eq!(status["compact_hooks"].as_array().unwrap().len(), 4);
}
