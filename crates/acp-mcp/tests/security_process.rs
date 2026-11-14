//! Rust authority for the seven P0 negative cases, through real daemon/IPC/MCP.
//! Build workspace bins first. Rust constructs a disposable current-v1 database.
#[path = "../../acp-store/tests/support/mod.rs"]
mod storage_fixture;
use acp_ipc::{Access, Client, Credential, Location, load_credential};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
fn exe(name: &str) -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}
struct Fixture {
    _temp: tempfile::TempDir,
    location: Location,
    db: PathBuf,
    before: Vec<u8>,
    daemon: Child,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::Builder::new()
            .prefix("acp-e4-")
            .tempdir()
            .unwrap();
        storage_fixture::fixture(&temp.path().join("fixture"));
        let runtime_dir = temp.path().join("run");
        let endpoint = if cfg!(windows) {
            format!(
                "control-{}",
                temp.path().file_name().unwrap().to_string_lossy()
            )
        } else {
            runtime_dir.join("control.sock").to_string_lossy().into()
        };
        let location = Location {
            runtime_dir,
            endpoint,
        };
        let db = temp.path().join("fixture/state/context.sqlite3");
        let before = fs::read(&db).unwrap();
        let daemon = Self::launch(&location, &db);
        let f = Self {
            _temp: temp,
            location,
            db,
            before,
            daemon,
        };
        f.ready();
        f
    }
    fn launch(location: &Location, db: &Path) -> Child {
        Command::new(exe("acp-daemon"))
            .arg("--runtime-dir")
            .arg(&location.runtime_dir)
            .arg("--endpoint")
            .arg(&location.endpoint)
            .arg("--database")
            .arg(db)
            .args(["--allow-project", "alpha", "--allow-project", "beta"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("build workspace binaries before process security tests")
    }
    fn ready(&self) {
        let end = Instant::now() + Duration::from_secs(12);
        while Instant::now() < end {
            if self.location.runtime_dir.join("agent-beta.cap").exists()
                && self.request(None, "daemon.health", json!({})).is_ok()
            {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("synthetic daemon readiness deadline");
    }
    fn credential(&self, project: Option<&str>) -> Credential {
        load_credential(
            &self.location,
            &self.location.credential_path(project).unwrap(),
        )
        .unwrap()
    }
    fn request(&self, project: Option<&str>, method: &str, args: Value) -> acp_ipc::Result<Value> {
        let c = self.credential(project);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            Client::connect(&self.location, &c)
                .await?
                .request(method, args)
                .await
        })
    }
    fn unchanged(&self) {
        assert_eq!(
            fs::read(&self.db).unwrap(),
            self.before,
            "read-only process suite changed disposable DB"
        );
    }
    fn restart(&mut self) {
        self.daemon.kill().unwrap();
        self.daemon.wait().unwrap();
        self.daemon = Self::launch(&self.location, &self.db);
        self.ready();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}
struct Bridge {
    proc: Child,
    lines: mpsc::Receiver<Value>,
}
impl Bridge {
    fn new(f: &Fixture, project: &str) -> Self {
        let mut proc = Command::new(env!("CARGO_BIN_EXE_acp-mcp"))
            .arg("--runtime-dir")
            .arg(&f.location.runtime_dir)
            .arg("--endpoint")
            .arg(&f.location.endpoint)
            .arg("--project")
            .arg(project)
            .arg("--capability-file")
            .arg(f.location.runtime_dir.join(format!("agent-{project}.cap")))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = proc.stdout.take().unwrap();
        let (sender, lines) = mpsc::sync_channel(4);
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Ok(value) = serde_json::from_str(&line) else {
                    break;
                };
                if sender.send(value).is_err() {
                    break;
                }
            }
        });
        Self { proc, lines }
    }
    fn request(&mut self, message: Value) -> Option<Value> {
        let input = self.proc.stdin.as_mut().unwrap();
        writeln!(input, "{message}").unwrap();
        input.flush().unwrap();
        message.get("id")?;
        Some(
            self.lines
                .recv_timeout(Duration::from_secs(12))
                .expect("actual MCP response deadline"),
        )
    }
    fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"},"name":name,"arguments":arguments}})).unwrap()
    }
    fn value(response: &Value) -> Value {
        assert_ne!(
            response["result"]["isError"], true,
            "expected successful read"
        );
        serde_json::from_str(
            response["result"]["content"][0]["text"]
                .as_str()
                .expect("MCP text result"),
        )
        .unwrap()
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.proc.kill();
        let _ = self.proc.wait();
    }
}
fn rejected(value: &Value) {
    assert!(
        value.get("error").is_some() || value["result"]["isError"] == true,
        "hostile request accepted"
    );
    for marker in [
        "BETA_PRIVATE_PREFERENCE",
        "BETA_PROJECT_CONTRACT",
        "synthetic-beta",
    ] {
        assert!(
            !value.to_string().contains(marker),
            "error payload leaked synthetic foreign marker"
        );
    }
}

#[test]
fn confirmed_only_statuses_and_complete_operator_parity() {
    let f = Fixture::new();
    for project in ["alpha", "beta"] {
        let mut bridge = Bridge::new(&f, project);
        let value = Bridge::value(&bridge.call("get_effective_context", json!({})));
        assert_eq!(
            value,
            f.request(None, "context.get", json!({"project":project}))
                .unwrap()
        );
        for row in value["preferences"].as_array().unwrap() {
            assert_eq!(row["status"], "confirmed");
        }
        for marker in [
            "PENDING_NOT_FOR_AGENT",
            "REJECTED_NOT_FOR_AGENT",
            "DEPRECATED_NOT_FOR_AGENT",
        ] {
            assert!(!value.to_string().contains(marker));
        }
    }
    f.unchanged();
}
#[test]
fn global_override_and_documents_never_cross_scope() {
    let f = Fixture::new();
    for project in ["alpha", "beta"] {
        let mut bridge = Bridge::new(&f, project);
        for documents in [false, true] {
            let value = Bridge::value(&bridge.call(
                "get_effective_context",
                json!({"include_documents":documents}),
            ));
            assert_eq!(
                value,
                f.request(
                    None,
                    "context.get",
                    json!({"project":project,"include_documents":documents})
                )
                .unwrap()
            );
            let values: BTreeMap<_, _> = value["preferences"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| (p["key"].as_str().unwrap(), p["value"].as_str().unwrap()))
                .collect();
            let expected = if project == "alpha" {
                BTreeMap::from([
                    ("commit.style", "newest"),
                    ("interaction.language", "zh-CN"),
                ])
            } else {
                BTreeMap::from([
                    ("commit.style", "conventional"),
                    ("interaction.language", "zh-CN"),
                    ("project.private.marker", "BETA_PRIVATE_PREFERENCE"),
                ])
            };
            assert_eq!(values, expected);
            if project == "alpha" {
                for marker in ["BETA_PROJECT_CONTRACT", "BETA_PRIVATE_PREFERENCE"] {
                    assert!(!value.to_string().contains(marker));
                }
            } else {
                assert!(!value.to_string().contains("ALPHA_PROJECT_CONTRACT"));
            }
            assert_eq!(value["documents"][0].get("content").is_some(), documents);
        }
    }
    f.unchanged();
}
#[test]
fn project_selector_injection_and_malformed_arguments_are_refused() {
    let f = Fixture::new();
    let mut bridge = Bridge::new(&f, "alpha");
    for args in [
        json!({"project":"beta","include_documents":true}),
        json!({"project_id":"beta"}),
        json!({"include_documents":1}),
        json!([]),
        Value::Null,
        json!({"unknown":true}),
    ] {
        rejected(&bridge.call("get_effective_context", args));
    }
    for args in [
        json!({"task_id":"shared","project":"beta"}),
        json!({"task_id":"shared","project_id":"beta"}),
        json!({"task_id":1}),
        json!({}),
    ] {
        rejected(&bridge.call("get_handoff", args));
    }
    f.unchanged();
}
#[test]
fn same_task_id_and_foreign_only_tasks_are_project_bound() {
    let f = Fixture::new();
    for (project, other) in [("alpha", "beta"), ("beta", "alpha")] {
        let mut bridge = Bridge::new(&f, project);
        let value = Bridge::value(&bridge.call("get_handoff", json!({"task_id":"shared"})));
        assert_eq!(
            value,
            f.request(
                None,
                "handoff.get",
                json!({"project":project,"task_id":"shared"})
            )
            .unwrap()
        );
        assert_eq!(value["objective"], format!("synthetic-{project}"));
        assert!(!value.to_string().contains(&format!("synthetic-{other}")));
        assert_eq!(value["validation"].as_array().unwrap().len(), 4);
    }
    let mut alpha = Bridge::new(&f, "alpha");
    rejected(&alpha.call("get_handoff", json!({"task_id":"beta-only"})));
    f.unchanged();
}
#[test]
fn unregistered_unknown_and_mutating_tools_have_no_authority() {
    let f = Fixture::new();
    let mut bridge = Bridge::new(&f, "alpha");
    for tool in [
        "unknown",
        "profile.preferences.list",
        "profile.preferences.get",
        "profile.review.queue",
        "profile.review.inspect",
        "profile.handoff.get",
        "profile.mutate",
        "profile.write.enable",
        "preference_confirm",
        "preference_suggest",
        "project_add",
        "handoff.list",
        "project.list",
        "daemon.shutdown",
    ] {
        rejected(&bridge.call(tool, json!({})));
    }
    assert!(
        f.request(None, "context.get", json!({"project":"unregistered"}))
            .is_err()
    );
    let outcome = Command::new(env!("CARGO_BIN_EXE_acp-mcp"))
        .arg("--runtime-dir")
        .arg(&f.location.runtime_dir)
        .arg("--endpoint")
        .arg(&f.location.endpoint)
        .arg("--project")
        .arg("unregistered")
        .arg("--capability-file")
        .arg(f.location.runtime_dir.join("agent-alpha.cap"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .unwrap();
    assert!(!outcome.status.success() && outcome.stdout.is_empty());
    f.unchanged();
}
#[test]
fn real_stdio_two_schema_versions_and_notifications_remain_compatible() {
    let f = Fixture::new();
    for version in ["2025-11-25", "2026-07-28"] {
        let mut bridge = Bridge::new(&f, "alpha");
        let initialized=bridge.request(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":version}})).unwrap();
        assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
        assert!(
            bridge
                .request(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
                .is_none()
        );
        let listing = bridge
            .request(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
            .unwrap();
        let tools = listing["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        let names: std::collections::BTreeSet<_> =
            tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            std::collections::BTreeSet::from(["get_effective_context", "get_handoff"])
        );
        for tool in tools {
            assert_eq!(tool["inputSchema"]["additionalProperties"], false);
            assert!(tool["inputSchema"]["properties"].get("project").is_none());
        }
        rejected(&bridge.call("preference_confirm", json!({"id":1})));
        rejected(&bridge.call("get_effective_context", json!({"project_id":"beta"})));
    }
    f.unchanged();
}
#[test]
fn bound_ipc_agents_cannot_list_or_escalate_after_restart_and_rotation() {
    let mut f = Fixture::new();
    let old = f.credential(Some("alpha"));
    for restarted in [false, true] {
        if restarted {
            f.restart();
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            assert!(rt.block_on(Client::connect(&f.location, &old)).is_err());
        }
        let credential = f.credential(Some("alpha"));
        assert_eq!(
            credential.access,
            Access::Agent {
                project: "alpha".into()
            }
        );
        for (method, args) in [
            ("project.list", json!({})),
            ("context.get", json!({"project":"beta"})),
            ("context.get", json!({"project_id":"beta"})),
            ("handoff.get", json!({"project":"beta","task_id":"shared"})),
            ("preference.confirm", json!({"id":1})),
            ("daemon.shutdown", json!({})),
        ] {
            assert!(f.request(Some("alpha"), method, args).is_err());
        }
        let mut forged = credential;
        forged.access = Access::Operator;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(rt.block_on(Client::connect(&f.location, &forged)).is_err());
        let mut bridge = Bridge::new(&f, "alpha");
        rejected(&bridge.call("get_effective_context", json!({"project":"beta"})));
        rejected(&bridge.call("handoff.list", json!({})));
        assert_eq!(
            Bridge::value(&bridge.call("get_effective_context", json!({})))["project"],
            "alpha"
        );
    }
    f.unchanged();
}
