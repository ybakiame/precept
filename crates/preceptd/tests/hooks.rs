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
    run_with(cmd, &json!({"source":"startup","cwd":cwd}))
}

/// Runs the written command from an unrelated directory with `input` on stdin.
fn run_with(cmd: &str, input: &Value) -> String {
    let mut child = std::process::Command::new("sh")
        .args(["-c", cmd])
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.to_string().as_bytes())
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

#[cfg(unix)]
#[test]
fn antigravity_hook_is_a_named_entry_that_injects_a_system_message_for_its_workspace() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let status = f
        .daemon
        .control("integration.hooks", json!({"project":"alpha"}))
        .unwrap();
    let ag = status["hooks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["vendor"] == "antigravity")
        .unwrap()
        .clone();
    assert_eq!(ag["file"], "~/.gemini/config/hooks.json");
    assert_eq!(ag["installed"], false);
    let on = f
        .daemon
        .control(
            "integration.hooks.apply",
            json!({"project":"alpha","vendor":"antigravity","enabled":true,"expected_sha256":null}),
        )
        .unwrap();
    assert_eq!(on["action"], "added");
    let file = f.temp.path().join(".gemini/config/hooks.json");
    let config: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    let cmd = config["precept-alpha"]["SessionStart"][0]["command"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(cmd, on["command"]);
    assert!(cmd.contains("--vendor antigravity"));
    // Antigravity's input names the workspaces; there is no `cwd`.
    let input =
        |p: &std::path::Path| json!({"conversationId":"c-1","workspacePaths":[p],"modelName":"m"});
    let out = run_with(&cmd, &input(&f.temp.path().join("alpha")));
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(
        v["injectSteps"][0]["systemMessage"]["systemMessage"]
            .as_str()
            .unwrap()
            .contains("# Precept: context for alpha")
    );
    assert_eq!(
        run_with(&cmd, &input(&f.temp.path().join("beta"))),
        "",
        "silent in other workspaces"
    );
    let off = f
        .daemon
        .control(
            "integration.hooks.apply",
            json!({"project":"alpha","vendor":"antigravity","enabled":false,"expected_sha256":on["sha256"]}),
        )
        .unwrap();
    assert_eq!(off["action"], "removed");
    assert!(!fs::read_to_string(&file).unwrap().contains("precept-hook"));
}

#[cfg(unix)]
#[test]
fn mcp_entries_switch_per_client_and_the_written_command_serves_the_project() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let status = f
        .daemon
        .control("integration.mcp", json!({"project":"alpha"}))
        .unwrap();
    assert_eq!(status["available"], true);
    let files: Vec<&str> = status["clients"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["file"].as_str().unwrap())
        .collect();
    assert_eq!(
        files,
        [
            "~/.claude.json",
            "~/.codex/config.toml",
            "~/.gemini/config/mcp_config.json"
        ]
    );
    let agent = f.daemon.credential("agent-alpha.cap");
    assert_eq!(
        f.daemon
            .call(&agent, "integration.mcp", json!({"project":"alpha"})),
        Err(Error::Unauthorized)
    );
    for client in ["claude", "codex", "antigravity"] {
        let on = f
            .daemon
            .control(
                "integration.mcp.apply",
                json!({"project":"alpha","client":client,"enabled":true,"expected_sha256":null}),
            )
            .unwrap();
        assert_eq!(on["action"], "added", "{client}");
        assert_eq!(on["current"], true, "{client}");
    }
    // The Codex entry, read back with a TOML parser, starts a working server.
    let text = fs::read_to_string(f.temp.path().join(".codex/config.toml")).unwrap();
    let table: toml::Table = toml::from_str(&text).unwrap();
    let entry = &table["mcp_servers"]["precept-alpha"];
    let args: Vec<&str> = entry["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    let mut child = std::process::Command::new(entry["command"].as_str().unwrap())
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        for m in [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        ] {
            writeln!(input, "{m}").unwrap();
        }
    }
    let out = String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap();
    assert!(out.contains("get_effective_context") && out.contains("suggest_rule"));

    // Off again, guarded by the reported digest.
    let status = f
        .daemon
        .control("integration.mcp", json!({"project":"alpha"}))
        .unwrap();
    let codex = status["clients"][1].clone();
    assert_eq!(
        f.daemon.control(
            "integration.mcp.apply",
            json!({"project":"alpha","client":"codex","enabled":false,"expected_sha256":null}),
        ),
        Err(Error::Conflict)
    );
    let off = f
        .daemon
        .control(
            "integration.mcp.apply",
            json!({"project":"alpha","client":"codex","enabled":false,"expected_sha256":codex["sha256"]}),
        )
        .unwrap();
    assert_eq!(off["installed"], false);
    assert!(
        !fs::read_to_string(f.temp.path().join(".codex/config.toml"))
            .unwrap()
            .contains("precept-alpha")
    );
}
