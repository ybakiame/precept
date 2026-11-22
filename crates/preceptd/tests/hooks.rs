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
fn one_mcp_entry_per_client_serves_the_project_of_the_session_folder() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.register(&b, "beta");
    // An older per-project entry is removed by the switch.
    fs::write(
        f.temp.path().join(".claude.json"),
        r#"{"mcpServers":{"precept-alpha":{"command":"/usr/bin/precept-mcp","args":[]}}}"#,
    )
    .unwrap();
    let status = f.daemon.control("integration.mcp", json!({})).unwrap();
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
    assert_eq!(status["clients"][0]["legacy"], 1);
    let agent = f.daemon.credential("agent-alpha.cap");
    assert_eq!(
        f.daemon.call(&agent, "integration.mcp", json!({})),
        Err(Error::Unauthorized)
    );
    // The project is no longer a parameter.
    assert_eq!(
        f.daemon
            .control("integration.mcp", json!({"project":"alpha"})),
        Err(Error::InvalidRequest)
    );
    for (i, client) in ["claude", "codex", "antigravity"].iter().enumerate() {
        let on = f
            .daemon
            .control(
                "integration.mcp.apply",
                json!({"client":client,"enabled":true,"expected_sha256":status["clients"][i]["sha256"]}),
            )
            .unwrap();
        assert_eq!(on["action"], "added", "{client}");
        assert_eq!(on["current"], true, "{client}");
        assert_eq!(on["server"], "precept");
    }
    let claude: Value =
        serde_json::from_str(&fs::read_to_string(f.temp.path().join(".claude.json")).unwrap())
            .unwrap();
    assert!(claude["mcpServers"].get("precept-alpha").is_none());
    // The Codex entry, read back with a TOML parser, starts the server.
    let text = fs::read_to_string(f.temp.path().join(".codex/config.toml")).unwrap();
    let table: toml::Table = toml::from_str(&text).unwrap();
    let entry = &table["mcp_servers"]["precept"];
    let args: Vec<String> = entry["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_owned())
        .collect();
    let command = entry["command"].as_str().unwrap().to_owned();
    // What the agent reads, started in `dir` (subfolders included).
    let read = |dir: &std::path::Path| -> String {
        let mut child = std::process::Command::new(&command)
            .args(&args)
            .current_dir(dir)
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
                json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_effective_context","arguments":{}}}),
            ] {
                writeln!(input, "{m}").unwrap();
            }
        }
        String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap()
    };
    fs::create_dir_all(f.temp.path().join("beta/src")).unwrap();
    let alpha = read(&f.temp.path().join("alpha"));
    assert!(alpha.contains(r#"\"project\":\"alpha\""#), "{alpha}");
    let beta = read(&f.temp.path().join("beta/src"));
    assert!(beta.contains(r#"\"project\":\"beta\""#), "{beta}");
    let nowhere = read(f.temp.path());
    assert!(nowhere.contains("not part of a project registered in Precept"));
    assert!(!nowhere.contains("alpha") && !nowhere.contains("beta"));
    // A client that offers roots is asked for them, and they decide.
    let mut child = std::process::Command::new(&command)
        .args(&args)
        .current_dir(f.temp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut next = || {
        let mut line = String::new();
        std::io::BufRead::read_line(&mut output, &mut line).unwrap();
        serde_json::from_str::<Value>(&line).unwrap()
    };
    writeln!(input, "{}", json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{"roots":{"listChanged":true}}}})).unwrap();
    next();
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    let ask = next();
    assert_eq!(ask["method"], "roots/list");
    let uri = format!("file://{}", f.temp.path().join("beta").display());
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":ask["id"],"result":{"roots":[{"uri":uri}]}})
    )
    .unwrap();
    writeln!(input, "{}", json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_effective_context","arguments":{}}})).unwrap();
    let answer = next().to_string();
    assert!(answer.contains(r#"\"project\":\"beta\""#), "{answer}");
    drop(input);
    let _ = child.wait();

    // Off again, guarded by the reported digest.
    let status = f.daemon.control("integration.mcp", json!({})).unwrap();
    let codex = status["clients"][1].clone();
    assert_eq!(
        f.daemon.control(
            "integration.mcp.apply",
            json!({"client":"codex","enabled":false,"expected_sha256":null}),
        ),
        Err(Error::Conflict)
    );
    let off = f
        .daemon
        .control(
            "integration.mcp.apply",
            json!({"client":"codex","enabled":false,"expected_sha256":codex["sha256"]}),
        )
        .unwrap();
    assert_eq!(off["installed"], false);
    assert!(
        !fs::read_to_string(f.temp.path().join(".codex/config.toml"))
            .unwrap()
            .contains("[mcp_servers.\"precept\"]")
    );
}

fn git(root: &std::path::Path, args: &[&str]) {
    let hooks = root.join(".git-empty-hooks");
    fs::create_dir_all(&hooks).unwrap();
    assert!(
        std::process::Command::new("git")
            .arg("-c")
            .arg(format!("core.hooksPath={}", hooks.display()))
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
            .current_dir(root)
            .stdout(Stdio::null())
            .status()
            .unwrap()
            .success()
    );
}

/// The gate entry the daemon wrote for `vendor`.
fn gate_command(f: &Fixture, vendor: &str) -> String {
    let on = f
        .daemon
        .control(
            "integration.hooks.apply",
            json!({"project":"alpha","vendor":vendor,"event":"pre_tool","enabled":true,
                "expected_sha256":null}),
        )
        .unwrap();
    assert_eq!(on["installed"], true);
    assert_eq!(on["event"], "pre_tool");
    on["command"].as_str().unwrap().to_owned()
}

#[cfg(unix)]
#[test]
fn the_gate_denies_calls_that_break_a_committed_hard_rule_only_while_switched_on() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let root = f.temp.path().join("alpha");
    git(&root, &["init", "-q"]);
    fs::create_dir_all(root.join(".precept/rules")).unwrap();
    let rule = root.join(".precept/rules/release.md");
    fs::write(
        &rule,
        "+++\nkey = \"release\"\nenforcement = \"hard\"\n[[guard]]\ntool = \"shell\"\ndeny = \"git push --force\"\n[[guard]]\ntool = \"write\"\npaths = [\"Cargo.lock\"]\n+++\nNever rewrite published history.\n",
    )
    .unwrap();
    git(&root, &["add", ".precept"]);
    git(&root, &["commit", "-q", "-m", "rules"]);
    let claude = gate_command(&f, "claude");
    let antigravity = gate_command(&f, "antigravity");
    let push = json!({"hook_event_name":"PreToolUse","cwd":root,"tool_name":"Bash",
        "tool_input":{"command":"git push --force origin main"}});
    // Off by default: the hook says nothing and the call proceeds.
    assert_eq!(run_with(&claude, &push), "");
    let agent = f.daemon.credential("agent-alpha.cap");
    assert_eq!(
        f.daemon.call(
            &agent,
            "project.gate",
            json!({"name":"alpha","enabled":true})
        ),
        Err(Error::Unauthorized)
    );
    f.daemon
        .control("project.gate", json!({"name":"alpha","enabled":true}))
        .unwrap();
    let denied: Value = serde_json::from_str(&run_with(&claude, &push)).unwrap();
    assert_eq!(denied["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(
        denied["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .contains("Never rewrite published history.")
    );
    let fine =
        json!({"cwd":root,"tool_name":"Bash","tool_input":{"command":"git push origin main"}});
    assert_eq!(run_with(&claude, &fine), "");
    let edit = json!({"cwd":root.join("src"),"tool_name":"Edit",
        "tool_input":{"file_path":"../Cargo.lock"}});
    fs::create_dir_all(root.join("src")).unwrap();
    assert!(run_with(&claude, &edit).contains("\"deny\""));
    let ag = json!({"workspacePaths":[root],"toolCall":{"name":"run_command",
        "args":{"CommandLine":"git push --force","Cwd":root}}});
    let v: Value = serde_json::from_str(&run_with(&antigravity, &ag)).unwrap();
    assert_eq!(v["decision"], "deny");
    // A repeated denial of one rule (push, lock file, push) tells the agent to stop retrying.
    assert!(
        v["reason"]
            .as_str()
            .unwrap()
            .contains("has now blocked 3 tool calls"),
        "{v}"
    );
    // Outside the project the hook stays silent.
    let elsewhere = json!({"cwd":f.temp.path(),"tool_name":"Bash",
        "tool_input":{"command":"git push --force"}});
    assert_eq!(run_with(&claude, &elsewhere), "");
    // An uncommitted edit of the rule is not reviewed: nothing is denied.
    fs::write(
        &rule,
        fs::read_to_string(&rule)
            .unwrap()
            .replace("Never", "Do not"),
    )
    .unwrap();
    assert_eq!(run_with(&claude, &push), "");
    // Each decision is evidence.
    let log = f
        .daemon
        .control("activity.list", json!({"project":"alpha"}))
        .unwrap();
    let gates: Vec<&str> = log["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "gate")
        .filter_map(|e| e["outcome"].as_str())
        .collect();
    assert!(gates.contains(&"deny") && gates.contains(&"off") && gates.contains(&"allow"));
    assert_eq!(log["chain"]["ok"], true);
    // The next agent's context names what was blocked, and how often.
    let context = f.daemon.call(&agent, "context.get", json!({})).unwrap();
    let denials = context["gate_denials"].as_array().unwrap();
    assert_eq!(denials[0]["rule"], "release");
    assert_eq!(denials[0]["count"], 3, "{denials:?}");
    assert_eq!(denials.len(), 1);
    // The edited rule file is not committed: explain says so.
    let why = f
        .daemon
        .call(&agent, "context.explain", json!({"key":"release"}))
        .unwrap();
    assert_eq!(why["rules"][0]["source"], "repository");
    assert_eq!(why["rules"][0]["reason"], "uncommitted", "{why}");
}
