//! Owned synthetic integration only. No profile discovery, model or user credentials.
mod desktop;
mod relay;
mod screen;
mod tty;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Fixture {
    root: tempfile::TempDir,
    repo: PathBuf,
    daemon: Owned,
    relay: Option<Owned>,
    runtime: PathBuf,
    endpoint: String,
    alias: String,
    db: PathBuf,
}
impl Fixture {
    fn new(repo: PathBuf) -> Self {
        let root = tempfile::Builder::new()
            .prefix("acp-s5-")
            .tempdir()
            .unwrap();
        let runtime = root.path().join("runtime");
        let id = root.path().file_name().unwrap().to_str().unwrap();
        let endpoint = if cfg!(windows) {
            format!("{id}-main")
        } else {
            runtime.join("main.sock").to_str().unwrap().to_owned()
        };
        let alias = if cfg!(windows) {
            format!("{id}-relay")
        } else {
            runtime.join("relay.sock").to_str().unwrap().to_owned()
        };
        acp_ipc::transport::private_dir(&root.path().join("state"), true).unwrap();
        for name in ["alpha", "beta"] {
            fs::create_dir(root.path().join(name)).unwrap();
            fs::write(
                root.path().join(name).join("AGENTS.md"),
                "Synthetic local contract.\n",
            )
            .unwrap();
        }
        let db = root.path().join("state/profile.sqlite3");
        let daemon = Self::launch(&repo, root.path(), &runtime, &endpoint, None);
        let mut f = Self {
            root,
            repo,
            daemon,
            relay: None,
            runtime,
            endpoint,
            alias,
            db,
        };
        let end = Instant::now() + Duration::from_secs(20);
        while !f.query(&["profile", "info"]).0 {
            assert!(Instant::now() < end);
            thread::sleep(Duration::from_millis(30));
        }
        let mut relay = Command::new(std::env::current_exe().unwrap())
            .arg("relay")
            .arg(&f.runtime)
            .arg(&f.endpoint)
            .arg(&f.alias)
            .arg(f.root.path().join("drop-next"))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(relay.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        assert_eq!(line.trim(), "RELAY_READY");
        f.relay = Some(Owned(relay));
        f
    }
    fn bin(repo: &Path, name: &str) -> PathBuf {
        repo.join("target/debug")
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
    }
    fn launch(
        repo: &Path,
        base: &Path,
        runtime: &Path,
        endpoint: &str,
        db: Option<&Path>,
    ) -> Owned {
        let mut c = Command::new(Self::bin(repo, "acp-daemon"));
        c.arg("--runtime-dir")
            .arg(runtime)
            .arg("--endpoint")
            .arg(endpoint);
        if let Some(db) = db {
            c.arg("--database").arg(db);
        }
        for name in ["alpha", "beta"] {
            c.arg("--allow-project")
                .arg(name)
                .arg("--allow-root")
                .arg(format!(
                    "{name}={}",
                    base.join(name).canonicalize().unwrap().to_str().unwrap()
                ));
        }
        Owned(
            c.stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        )
    }
    fn query_at(&self, runtime: &Path, endpoint: &str, args: &[&str]) -> (bool, Value) {
        let result = Command::new(Self::bin(&self.repo, "acp-query"))
            .arg("--runtime-dir")
            .arg(runtime)
            .arg("--endpoint")
            .arg(endpoint)
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.stderr.is_empty(),
            "unexpected synthetic CLI diagnostic"
        );
        (
            result.status.success(),
            serde_json::from_slice(&result.stdout).unwrap_or(Value::Null),
        )
    }
    fn query(&self, args: &[&str]) -> (bool, Value) {
        self.query_at(&self.runtime, &self.endpoint, args)
    }
    fn mutate(&self, op: &str, args: &[&str]) -> Value {
        let b = self.query(&["profile", "info"]).1["binding"].clone();
        let mut all = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        all.extend([
            "--profile-id".into(),
            b["profile_id"].as_str().unwrap().into(),
            "--generation".into(),
            b["generation"].as_str().unwrap().into(),
            "--owner-epoch".into(),
            b["owner_epoch"].to_string(),
            "--operation-id".into(),
            op.into(),
        ]);
        let (ok, v) = self.query(&all.iter().map(String::as_str).collect::<Vec<_>>());
        assert!(ok, "synthetic CLI mutation refused: {}", v["error"]);
        v
    }
    fn rows(&self) -> Vec<Value> {
        let (ok, v) = self.query(&[
            "preferences",
            "list",
            "--scope",
            "project",
            "--project",
            "alpha",
        ]);
        assert!(ok);
        v["records"].as_array().unwrap().clone()
    }
    fn stop_daemon(&mut self) {
        let _ = self.daemon.0.kill();
        let _ = self.daemon.0.wait();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.relay.take();
        self.stop_daemon();
    }
}
enum Ui {
    Desktop(desktop::Desktop),
    Tty(tty::Tty),
}
impl Ui {
    fn key(&mut self, key: &str) {
        match self {
            Self::Desktop(d) => d.key(key),
            Self::Tty(t) => {
                let b = match key {
                    "escape" => vec![27],
                    "enter" => vec![13],
                    "tab" => vec![9],
                    "ctrl+a" => vec![1],
                    "ctrl+l" => vec![12],
                    "ctrl+r" => vec![18],
                    "ctrl+p" => vec![16],
                    "alt+right" => b"\x1b[1;3C".to_vec(),
                    "alt+left" => b"\x1b[1;3D".to_vec(),
                    _ => {
                        let parts = key.split('+').collect::<Vec<_>>();
                        assert!(parts[0] == "alt");
                        let c = parts.last().unwrap().chars().next().unwrap();
                        vec![
                            27,
                            if parts.contains(&"shift") {
                                c.to_ascii_uppercase() as u8
                            } else {
                                c as u8
                            },
                        ]
                    }
                };
                t.send(&b);
            }
        }
    }
    fn text(&mut self, text: &str) {
        match self {
            Self::Desktop(d) => d.text(text),
            Self::Tty(t) => t.send(text.as_bytes()),
        }
    }
    fn field(&mut self, key: &str, text: &str) {
        self.key(key);
        self.key("ctrl+a");
        self.text(text);
    }
    fn wait(&self, marker: &str, text: &str) {
        match self {
            Self::Desktop(d) => d.wait(marker),
            Self::Tty(t) => t.wait(text),
        }
    }
    fn apply(&mut self, key: &str, marker: &str, text: &str) {
        self.key(key);
        self.wait("review=true", "Prepared operation");
        self.key("alt+shift+v");
        self.wait(marker, text);
    }
    fn receipt(&mut self, key: &str, n: i64) {
        self.apply(key, &format!("receipt={n} "), &format!("commit {n}"));
    }
    fn capture(&mut self, name: &str, out: &Path) {
        match self {
            Self::Desktop(d) => d.capture(name, out),
            Self::Tty(t) => t.capture(&out.join(format!("{name}.txt"))),
        }
    }
    fn resize(&mut self, w: u16, h: u16) {
        match self {
            Self::Desktop(d) => d.resize(w, h),
            Self::Tty(t) => t.resize(w, h),
        }
    }
    fn close(&mut self) {
        match self {
            Self::Desktop(d) => d.close(),
            Self::Tty(t) => t.close(),
        }
    }
}
fn run(surface: &str, out: &Path, window_script: &str, development: bool) -> Value {
    let repo = std::env::current_dir().unwrap();
    let git = |args: &[&str]| {
        String::from_utf8(Command::new("git").args(args).output().unwrap().stdout)
            .unwrap()
            .trim()
            .to_owned()
    };
    let sha = git(&["rev-parse", "HEAD"]);
    let dirty = !git(&["status", "--porcelain"]).is_empty();
    assert!(!dirty || development, "clean source required");
    fs::create_dir(out).expect("new report directory required");
    let mut f = Fixture::new(repo.clone());
    let nonce = f.root.path().file_name().unwrap().to_string_lossy();
    let hash = nonce
        .bytes()
        .fold(0u64, |a, b| a.wrapping_mul(131).wrapping_add(b as u64));
    let nonce = format!("{hash:016x}");
    let nsh = repo.join("spikes/p1-e/nsh/target/debug").join(format!(
        "acp-nsh-{}{}",
        if surface == "desktop" {
            "native"
        } else {
            "tui"
        },
        std::env::consts::EXE_SUFFIX
    ));
    let args = vec![
        "--runtime-dir".into(),
        f.runtime.to_str().unwrap().into(),
        "--endpoint".into(),
        f.alias.clone(),
        "--operator".into(),
        "--appearance".into(),
        "light".into(),
    ];
    let mut ui = if surface == "desktop" {
        Ui::Desktop(desktop::Desktop::start(
            &nsh,
            &args,
            &nonce,
            window_script,
            out,
        ))
    } else {
        Ui::Tty(tty::Tty::start(&nsh, &args, out))
    };
    ui.key("alt+o");
    ui.wait(
        "selected=false grant=false busy=false",
        "No Profile selected",
    );
    ui.field("alt+p", f.db.to_str().unwrap());
    ui.key("alt+shift+u");
    ui.wait("review=true", "Prepared operation");
    ui.capture("create-review", out);
    ui.key("alt+shift+v");
    ui.wait("selected=true grant=false busy=false", "Profile selected");
    assert_eq!(f.query(&["profile", "info"]).1["write_enabled"], false);
    let competitor_run = f.root.path().join("competitor");
    let competitor_endpoint = if cfg!(windows) {
        format!("s5-competitor-{nonce}")
    } else {
        competitor_run
            .join("main.sock")
            .to_str()
            .unwrap()
            .to_owned()
    };
    let competitor = Fixture::launch(
        &repo,
        f.root.path(),
        &competitor_run,
        &competitor_endpoint,
        Some(&f.db),
    );
    let end = Instant::now() + Duration::from_secs(15);
    let info = loop {
        let (ok, v) = f.query_at(&competitor_run, &competitor_endpoint, &["profile", "info"]);
        if ok {
            break v;
        }
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(20));
    };
    let b = &info["binding"];
    assert!(
        f.query_at(
            &competitor_run,
            &competitor_endpoint,
            &[
                "profile",
                "write",
                "enable",
                "--profile-id",
                b["profile_id"].as_str().unwrap(),
                "--generation",
                b["generation"].as_str().unwrap(),
                "--confirm"
            ]
        )
        .0
    );
    ui.apply("alt+shift+e", "error=Busy", "Busy");
    ui.capture("busy", out);
    assert_eq!(f.query(&["profile", "info"]).1["write_enabled"], false);
    assert!(
        f.query_at(
            &competitor_run,
            &competitor_endpoint,
            &["profile", "write", "disable"]
        )
        .0
    );
    drop(competitor);
    ui.key("alt+shift+r");
    ui.wait("selected=true grant=false busy=false", "Write grant off");
    ui.apply(
        "alt+shift+e",
        "selected=true grant=true busy=false",
        "Write grant active",
    );
    assert_eq!(f.query(&["profile", "info"]).1["write_enabled"], true);
    for name in ["alpha", "beta"] {
        let root = f.root.path().join(name).canonicalize().unwrap();
        f.mutate(
            &format!("register-{name}"),
            &["project-register", name, root.to_str().unwrap()],
        );
    }
    let input = f.root.path().join("handoff.json");
    fs::write(&input,json!({"objective":"Saved synthetic history","completed":"","next_action":"Inspect current repository facts","validation_json":"[{\"status\":\"NOT_RUN\",\"check\":\"synthetic\"}]","snapshot":{"base_sha":"","branch":"","state":"unknown","fingerprint":null}}).to_string()).unwrap();
    f.mutate(
        "handoff",
        &[
            "handoff-put",
            "alpha",
            "shared",
            "--expected-absent",
            "--input",
            input.to_str().unwrap(),
        ],
    );
    ui.key("ctrl+r");
    ui.wait("phase=Ready", "alpha");
    ui.field("alt+k", "editor");
    ui.field("alt+w", "synthetic-value");
    ui.field("alt+f", "synthetic-source");
    ui.receipt("alt+shift+s", 4);
    assert_eq!(f.rows()[0]["status"], "suggested");
    ui.key("alt+shift+l");
    ui.wait("rows=1", "#1");
    ui.key("alt+shift+i");
    ui.wait("record_revision=1", "#1");
    ui.receipt("alt+shift+y", 5);
    assert_eq!(f.rows()[0]["status"], "confirmed");
    ui.capture("confirmed-exact-record", out);
    ui.receipt("alt+shift+s", 6);
    ui.key("alt+shift+l");
    ui.wait("rows=2", "#2");
    ui.key("alt+shift+i");
    ui.key("alt+shift+i");
    ui.wait("record_revision=1", "#2");
    ui.receipt("alt+shift+n", 7);
    assert_eq!(f.rows()[1]["status"], "rejected");
    fs::write(f.root.path().join("drop-next"), b"synthetic fault").unwrap();
    ui.apply("alt+shift+s", "unknown=true", "OutcomeUnknown");
    ui.capture("outcome-unknown", out);
    let before = f.rows();
    assert_eq!(before.len(), 3);
    assert!(!f.root.path().join("drop-next").exists());
    ui.key("alt+shift+z");
    ui.wait("receipt=8 ", "commit 8");
    assert_eq!(f.rows(), before);
    let dropped: Value =
        serde_json::from_slice(&fs::read(f.root.path().join("dropped-receipt.json")).unwrap())
            .unwrap();
    let replayed: Value =
        serde_json::from_slice(&fs::read(f.root.path().join("replayed-receipt.json")).unwrap())
            .unwrap();
    assert_eq!(dropped, replayed);
    assert_eq!(dropped["commit_sequence"], 8);
    fs::write(
        out.join("receipt-proof.json"),
        serde_json::to_vec_pretty(
            &json!({"dropped":dropped,"replayed":replayed,"rows_unchanged":true}),
        )
        .unwrap(),
    )
    .unwrap();
    ui.key("alt+shift+r");
    ui.wait("selected=true grant=true busy=false", "Write grant active");
    ui.key("alt+shift+l");
    ui.wait("rows=3", "#3");
    ui.key("alt+shift+i");
    ui.key("alt+shift+i");
    ui.key("alt+shift+i");
    ui.wait("record_revision=1", "#3");
    ui.key("alt+shift+y");
    ui.wait("review=true", "Prepared operation");
    f.mutate(
        "competing-reject",
        &[
            "preference-reject",
            "3",
            "--scope",
            "project",
            "--project",
            "alpha",
            "--expected-revision",
            "1",
            "--confirm",
        ],
    );
    ui.key("alt+shift+v");
    ui.wait("error=Conflict", "Conflict");
    assert_eq!(f.rows()[2]["status"], "rejected");
    ui.capture("stale-conflict", out);
    ui.key("alt+shift+r");
    ui.wait("selected=true grant=true busy=false", "Write grant active");
    ui.key("alt+shift+s");
    ui.wait("review=true", "Prepared operation");
    ui.key("escape");
    ui.key("alt+o");
    ui.wait("selected=true grant=true busy=false", "Write grant active");
    ui.key("alt+shift+s");
    ui.wait("review=true", "Prepared operation");
    ui.key("ctrl+p");
    thread::sleep(Duration::from_millis(500));
    ui.key("ctrl+p");
    thread::sleep(Duration::from_millis(500));
    ui.key("alt+shift+v");
    thread::sleep(Duration::from_millis(200));
    assert_eq!(f.rows().len(), 3);
    ui.field("alt+t", "shared");
    ui.key("tab");
    ui.key("enter");
    thread::sleep(Duration::from_millis(500));
    if let Ui::Tty(t) = &ui {
        t.wait("Saved synthetic history");
    }
    ui.capture("saved-handoff", out);
    let saved = f.query(&["handoff-record", "alpha", "shared"]).1;
    assert_eq!(saved["record"]["revision"], 1);
    assert!(
        saved["record"]["validation_json"]
            .as_str()
            .unwrap()
            .contains("NOT_RUN")
    );
    ui.key("alt+shift+r");
    ui.wait("selected=true grant=true busy=false", "Write grant active");
    ui.apply(
        "alt+shift+x",
        "selected=true grant=false busy=false",
        "Write grant off",
    );
    assert_eq!(f.query(&["profile", "info"]).1["write_enabled"], false);
    ui.apply(
        "alt+shift+e",
        "selected=true grant=true busy=false",
        "Write grant active",
    );
    assert!(f.query(&["profile", "write", "disable"]).0);
    ui.field("alt+k", "editor");
    ui.apply("alt+shift+s", "error=Unauthorized", "Unauthorized");
    assert_eq!(f.rows().len(), 3);
    ui.capture("revoked-grant", out);
    for n in 1..=5 {
        ui.key(&format!("alt+{n}"));
        thread::sleep(Duration::from_millis(150));
    }
    ui.key("alt+6");
    thread::sleep(Duration::from_millis(200));
    ui.capture("sessions", out);
    ui.key("escape");
    ui.key("alt+b");
    ui.key("ctrl+l");
    ui.key("alt+o");
    thread::sleep(Duration::from_millis(250));
    ui.resize(
        if surface == "desktop" { 640 } else { 80 },
        if surface == "desktop" { 480 } else { 30 },
    );
    ui.capture("compact-zh", out);
    ui.resize(
        if surface == "desktop" { 1080 } else { 160 },
        if surface == "desktop" { 760 } else { 50 },
    );
    ui.key("ctrl+l");
    ui.key("alt+shift+r");
    ui.wait("selected=true grant=false busy=false", "Write grant off");
    f.stop_daemon();
    ui.key("alt+shift+r");
    ui.wait("error=Unavailable", "Unavailable");
    ui.capture("disconnected", out);
    ui.close();
    let backend =
        match &ui {
            Ui::Desktop(d) => {
                let lines = d.lines.lock().unwrap();
                fs::write(out.join("desktop-probes.log"), lines.join("\n")).unwrap();
                if cfg!(unix) {
                    assert!(
                        lines.iter().any(|l| l.contains("backend=GdkWaylandDisplay")
                            && l.contains("mapped=true"))
                    );
                    assert!(
                        lines
                            .iter()
                            .any(|l| l.starts_with("ACP_GTK_FRAME after_paint="))
                    );
                    "WSLg native Wayland"
                } else {
                    "native Windows HWND"
                }
            }
            Ui::Tty(t) => {
                let raw = t.raw.lock().unwrap();
                fs::write(out.join("terminal.vt"), &*raw).unwrap();
                assert!(
                    raw.windows(8).any(|s| s == b"\x1b[?1049l"),
                    "alternate screen restoration missing"
                );
                if cfg!(windows) { "ConPTY" } else { "PTY" }
            }
        };
    assert!(development || git(&["status", "--porcelain"]).is_empty());
    let binaries: Vec<_> = [std::env::current_exe().unwrap(), Fixture::bin(&repo,"acp-daemon"),Fixture::bin(&repo,"acp-query"),nsh].iter().map(|path|json!({"name":path.file_name().unwrap().to_str().unwrap(),"sha256":format!("{:x}",Sha256::digest(fs::read(path).unwrap()))})).collect();
    json!({"status":if development{"DEV_PASS"}else{"PASS"},"tested_git_sha":sha,"worktree_dirty":dirty,"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"surface":surface,"backend":backend,"binaries":binaries,"actions":["unselected Profile","explicit UI create","Busy","enable","exact suggestion/confirm/reject","committed lost reply and explicit same-request retry","Conflict","Escape/project review invalidation","saved exact-task Handoff NOT_RUN history","disable acknowledgement","revoked-grant Unauthorized","five pages/Sessions","theme/locale/resize","disconnect","clean child exit"],"rows":3,"historical_source_substitution":false})
}
fn main() {
    let a: Vec<_> = std::env::args().skip(1).collect();
    if a.first().is_some_and(|s| s == "relay") {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(relay::run(&a[1..]));
        return;
    }
    assert!(a.len() >= 2, "surface and NEW_OUTPUT required");
    assert!(["desktop", "tui"].contains(&a[0].as_str()));
    let out = PathBuf::from(&a[1]);
    assert!(out.is_absolute());
    let result = run(
        &a[0],
        &out,
        a.get(2).map_or("", String::as_str),
        std::env::var_os("ACP_S5_DEVELOPMENT").is_some(),
    );
    fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!("{result}");
}
