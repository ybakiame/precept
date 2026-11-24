use precept_ipc::{Access, Client, Location, load_credential};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Fixture {
    _temp: tempfile::TempDir,
    home: PathBuf,
    checkout: PathBuf,
    profile: PathBuf,
    location: Location,
    daemon: Child,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::Builder::new()
            .prefix("precept-setup-")
            .tempdir()
            .unwrap();
        let home = temp.path().join("home");
        let checkout = home.join("checkout");
        let profile_dir = home.join("profile");
        let runtime = home.join(".local/state/precept/runtime");
        let state = home.join(".local/state/precept/daemon-state");
        fs::create_dir_all(&checkout).unwrap();
        precept_ipc::transport::private_dir(&profile_dir, true).unwrap();
        for directory in [
            home.join(".local"),
            home.join(".local/state"),
            home.join(".local/state/precept"),
            state.clone(),
        ] {
            precept_ipc::transport::private_dir(&directory, true).unwrap();
        }
        let mut projects =
            precept_ipc::transport::new_private(&state.join("projects.json")).unwrap();
        projects
            .write_all(br#"{"format":1,"projects":[]}"#)
            .unwrap();
        let endpoint = if cfg!(windows) {
            format!(
                "control-{}",
                temp.path().file_name().unwrap().to_string_lossy()
            )
        } else {
            runtime.join("control.sock").to_string_lossy().into_owned()
        };
        let location = Location {
            runtime_dir: runtime,
            endpoint,
        };
        let daemon = Self::launch(&home, &location, &state);
        let fixture = Self {
            _temp: temp,
            home,
            checkout,
            profile: profile_dir.join("context.sqlite3"),
            location,
            daemon,
        };
        fixture.ready();
        fixture
    }
    fn launch(home: &Path, location: &Location, state: &Path) -> Child {
        Command::new(product_exe("preceptd"))
            .arg("--runtime-dir")
            .arg(&location.runtime_dir)
            .arg("--endpoint")
            .arg(&location.endpoint)
            .arg("--state-dir")
            .arg(state)
            .env("HOME", home)
            .env("USERPROFILE", home)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("build workspace binaries before setup process tests")
    }
    fn ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(12);
        while Instant::now() < deadline {
            let path = self.location.runtime_dir.join("operator.cap");
            if path.exists()
                && let Ok(credential) = load_credential(&self.location, &path)
                && credential.access == Access::Operator
            {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                let connected = runtime.block_on(async {
                    let mut client = Client::connect(&self.location, &credential).await?;
                    client.request("daemon.health", json!({})).await
                });
                if connected.is_ok() {
                    return;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("synthetic daemon readiness deadline");
    }
    fn precept(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_precept"));
        command
            .arg("--runtime-dir")
            .arg(&self.location.runtime_dir)
            .arg("--endpoint")
            .arg(&self.location.endpoint)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home);
        command
    }
    fn setup(&self, plan: bool, input: &str) -> std::process::Output {
        self.setup_mode(plan, input, false, false)
    }
    fn setup_mode(
        &self,
        plan: bool,
        input: &str,
        integrate: bool,
        gate: bool,
    ) -> std::process::Output {
        let mut command = self.precept();
        command.arg("setup");
        if plan {
            command.arg("--plan");
        }
        command
            .arg("--profile")
            .arg(&self.profile)
            .arg("--project")
            .arg(&self.checkout)
            .arg("--name")
            .arg("alpha")
            .arg("--remember")
            .arg("false")
            .arg("--share")
            .arg("true")
            .arg("--propose")
            .arg("false")
            .arg("--integrate")
            .arg(integrate.to_string())
            .arg("--gate")
            .arg(gate.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
    fn json(&self, arguments: &[&str]) -> Value {
        let output = self.precept().args(arguments).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

fn product_exe(name: &str) -> PathBuf {
    let current = std::env::current_exe().unwrap();
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    if let Some(profile) = current.ancestors().find(|path| {
        path.file_name()
            .is_some_and(|name| matches!(name.to_str(), Some("debug" | "release")))
    }) {
        let candidate = profile.join(&file);
        if candidate.exists() {
            return candidate;
        }
    }
    if let Some(deps) = current.parent()
        && let Some(target) = deps.parent()
    {
        let candidate = target.join(&file);
        if candidate.exists() {
            return candidate;
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug")
        .join(file)
}

fn stdout(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid CLI JSON: {error}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn doctor_json_reports_real_daemon_layers_without_echoing_private_paths() {
    let fixture = Fixture::new();
    let output = fixture
        .precept()
        .args(["doctor", "--json", "--project", "alpha"])
        .output()
        .unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    let report: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(report["format"], "precept-doctor");
    assert_eq!(report["version"], 1);
    assert_eq!(report["product_version"], env!("CARGO_PKG_VERSION"));
    assert!(!text.contains(&fixture.home.to_string_lossy().to_string()));
    let check = |id: &str| {
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(check("runtime.location")["status"], "ok");
    assert_eq!(check("endpoint.location")["status"], "ok");
    assert_eq!(check("daemon.health")["status"], "ok");
    assert_eq!(check("daemon.api")["status"], "ok");
    assert_eq!(check("daemon.product")["status"], "ok");
    assert_eq!(check("profile.selected")["status"], "off");
    assert_eq!(check("project.binding")["status"], "problem");
    assert_eq!(check("project.binding")["reason"], "project_not_found");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn guided_setup_applies_previewed_integrations_and_mcp_returns_context() {
    let fixture = Fixture::new();
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&fixture.checkout)
            .env("HOME", &fixture.home)
            .env("USERPROFILE", &fixture.home)
            .status()
            .unwrap()
            .success()
    );
    let applied = fixture.setup_mode(
        false,
        "yes\nyes\nyes\nyes\nyes\nyes\nyes\nyes\nyes\nyes\nyes\nyes\n",
        true,
        true,
    );
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let applied = stdout(&applied);
    assert_eq!(applied["status"], "ready");
    assert_eq!(applied["write_enabled"], false);
    assert_eq!(applied["cooperative_gate"], true);
    assert_eq!(applied["enforcement_claimed"], false);
    assert_eq!(applied["changes"].as_array().unwrap().len(), 8);
    for file in [
        ".claude.json",
        ".claude/settings.json",
        ".codex/config.toml",
        ".codex/hooks.json",
    ] {
        assert!(fixture.home.join(file).is_file(), "missing {file}");
    }
    assert!(
        fs::read_to_string(fixture.home.join(".claude/settings.json"))
            .unwrap()
            .contains("PreToolUse")
    );
    assert!(
        fs::read_to_string(fixture.home.join(".codex/hooks.json"))
            .unwrap()
            .contains("PreToolUse")
    );

    let doctor = fixture
        .precept()
        .args(["doctor", "--json", "--project", "alpha"])
        .output()
        .unwrap();
    let doctor_text = String::from_utf8(doctor.stdout).unwrap();
    let doctor: Value = serde_json::from_str(&doctor_text).unwrap();
    let check = |id: &str| {
        doctor["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(check("daemon.health")["status"], "ok");
    assert_eq!(check("profile.writes")["status"], "off");
    assert_eq!(check("profile.writes")["reason"], "writes_disabled");
    assert_eq!(check("project.state")["status"], "ok");
    assert_eq!(check("integration.gate.claude")["status"], "ok");
    assert_eq!(check("integration.gate.codex")["status"], "ok");
    assert!(!doctor_text.contains("enforced"));
    assert!(!doctor_text.contains(&fixture.home.to_string_lossy().to_string()));

    let launch = fixture.json(&["project", "mcp", "alpha"]);
    let mut command = Command::new(launch["executable"].as_str().unwrap());
    for argument in launch["args"].as_array().unwrap() {
        command.arg(argument.as_str().unwrap());
    }
    let mut child = command
        .current_dir(&fixture.checkout)
        .env("HOME", &fixture.home)
        .env("USERPROFILE", &fixture.home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(
            stdin,
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}})
        )
        .unwrap();
        writeln!(
            stdin,
            "{}",
            json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .unwrap();
        writeln!(
            stdin,
            "{}",
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_effective_context","arguments":{}}})
        )
        .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let mut responses = BufReader::new(output.stdout.as_slice())
        .lines()
        .map(|line| serde_json::from_str::<Value>(&line.unwrap()).unwrap());
    let initialized = responses.next().unwrap();
    assert_eq!(initialized["id"], 1);
    assert_eq!(
        initialized["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    let context = responses.next().unwrap();
    assert_eq!(context["id"], 2);
    assert_eq!(context["result"]["isError"], false);
    let context: Value =
        serde_json::from_str(context["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(context["project"], "alpha");
}

#[test]
fn cancelling_after_write_enable_revokes_the_grant() {
    let fixture = Fixture::new();
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&fixture.checkout)
            .env("HOME", &fixture.home)
            .env("USERPROFILE", &fixture.home)
            .status()
            .unwrap()
            .success()
    );
    let cancelled = fixture.setup(false, "yes\nyes\nno\n");
    assert_eq!(cancelled.status.code(), Some(3));
    assert_eq!(stdout(&cancelled)["error"], "InvalidRequest");
    assert!(fixture.profile.exists());
    assert_eq!(fixture.json(&["projects"]), json!([]));
    assert_eq!(fixture.json(&["profile", "info"])["write_enabled"], false);
}

#[test]
fn setup_revokes_its_write_grant_when_confirmation_input_ends() {
    let fixture = Fixture::new();
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&fixture.checkout)
            .env("HOME", &fixture.home)
            .env("USERPROFILE", &fixture.home)
            .status()
            .unwrap()
            .success()
    );
    let applied = fixture.setup(false, "yes\nyes\nyes\nyes\n");
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let applied = stdout(&applied);
    assert_eq!(applied["status"], "ready");
    assert_eq!(applied["write_enabled"], false);
    assert_eq!(fixture.json(&["profile", "info"])["write_enabled"], false);
}

#[test]
fn first_activation_previews_cancels_and_reaches_context_without_remembering_writes() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.checkout.join(".precept/rules")).unwrap();
    fs::write(
        fixture.checkout.join(".precept/rules/setup.md"),
        "+++\nkey = \"setup.benchmark\"\ntitle = \"Setup benchmark\"\n+++\nUse the reviewed setup context.\n",
    )
    .unwrap();
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&fixture.checkout)
            .env("HOME", &fixture.home)
            .env("USERPROFILE", &fixture.home)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("git")
            .args(["add", ".precept/rules/setup.md"])
            .current_dir(&fixture.checkout)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("git")
            .args([
                "-c",
                "user.name=Precept Test",
                "-c",
                "user.email=precept-test@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "Add synthetic setup contract",
            ])
            .current_dir(&fixture.checkout)
            .status()
            .unwrap()
            .success()
    );

    let plan = fixture.setup(true, "");
    assert!(plan.status.success());
    let plan = stdout(&plan);
    assert_eq!(plan["status"], "planned");
    assert_eq!(plan["profile"]["action"], "create");
    assert_eq!(plan["project"]["action"], "add");
    assert_eq!(plan["write"]["action"], "enable");
    assert!(!fixture.profile.exists());
    assert_eq!(fixture.json(&["projects"]), json!([]));

    let cancelled = fixture.setup(false, "no\n");
    assert_eq!(cancelled.status.code(), Some(3));
    assert_eq!(stdout(&cancelled)["error"], "InvalidRequest");
    assert!(!fixture.profile.exists());
    assert_eq!(fixture.json(&["projects"]), json!([]));

    let started = Instant::now();
    let applied = fixture.setup(false, "yes\nyes\nyes\nyes\nyes\n");
    let elapsed = started.elapsed();
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let applied = stdout(&applied);
    assert_eq!(applied["status"], "ready");
    assert_eq!(applied["project"], "alpha");
    assert_eq!(applied["agent_read"], true);
    assert_eq!(applied["agent_propose"], false);
    assert_eq!(applied["cooperative_gate"], false);
    assert_eq!(applied["write_enabled"], false);
    assert!(elapsed < Duration::from_secs(600));
    assert!(fixture.profile.exists());
    let projects = fixture.json(&["projects"]);
    assert_eq!(projects[0]["name"], "alpha");
    assert_eq!(projects[0]["state"], "ready");
    assert_eq!(projects[0]["agents"], true);
    let context = fixture.json(&["context", "alpha"]);
    assert_eq!(context["project"], "alpha");
    assert_eq!(context["repository_rules"][0]["key"], "setup.benchmark");
    assert!(context["contract_resolution"]["status"].is_string());
    let info = fixture.json(&["profile", "info"]);
    assert_eq!(info["write_enabled"], false);
}
