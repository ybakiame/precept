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
    fn try_mutate(&self, op: &str, args: &[&str]) -> (bool, Value) {
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
        self.query(&all.iter().map(String::as_str).collect::<Vec<_>>())
    }
    fn mutate(&self, op: &str, args: &[&str]) -> Value {
        let (ok, v) = self.try_mutate(op, args);
        assert!(ok, "synthetic CLI mutation refused: {}", v["error"]);
        v
    }
    /// The exact persisted record. Its immutable `proposal_digest` is what the
    /// S1 CLI requires for every confirm/reject/deprecate/delete; reading it back
    /// binds the decision to the precise content the operator reviewed.
    fn record(&self, id: i64) -> Value {
        let id = id.to_string();
        let (ok, v) = self.query(&[
            "preferences",
            "get",
            &id,
            "--scope",
            "project",
            "--project",
            "alpha",
        ]);
        assert!(ok, "synthetic record read refused: {}", v["error"]);
        v["record"].clone()
    }
    fn digest(&self, id: i64) -> String {
        self.record(id)["proposal_digest"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    /// Current global transaction sequence; the next committed mutation is +1.
    fn commit_sequence(&self) -> i64 {
        let (ok, v) = self.query(&["review-queue", "--scope", "project", "--project", "alpha"]);
        assert!(ok, "synthetic queue read refused: {}", v["error"]);
        v["commit_sequence"].as_i64().unwrap()
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
    // Boxed: the bounded worker handle set is larger on Windows than the TTY state.
    Desktop(Box<desktop::Desktop>),
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
                    "ctrl+e" => vec![5],
                    "ctrl+u" => vec![21],
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
        match self {
            // Desktop ctrl+a selects all, so typing replaces the field content.
            Self::Desktop(_) => self.key("ctrl+a"),
            // The TUI input reads ctrl+a as cursor-to-start (not select-all), so
            // move to end (ctrl+e) and kill back to start (ctrl+u) to clear first.
            // Both are editing keys with no app mod binding; the focused input
            // consumes them (app mod bindings stay suppressed while editing).
            Self::Tty(_) => {
                self.key("ctrl+e");
                self.key("ctrl+u");
            }
        }
        self.text(text);
    }
    /// Activate a rendered button with keyboard only: focus a known anchor field
    /// (an alt chord), Tab forward a fixed number of stops, then Enter. Disabled
    /// and hidden controls are skipped and the tab list is one roving stop.
    /// Anchoring on the source field keeps the count independent of the
    /// surface-specific chrome above it. The two counts are calibrated separately
    /// against each real surface, because the PTY/TUI and the GTK (Wayland)
    /// desktop focus chains do not enumerate the same stops: from the source field
    /// the TUI reaches the load action and the digest expand in four Tabs and the
    /// first queue row in five, while the native desktop chain reaches each of
    /// those targets in three. The desktop count is therefore passed explicitly
    /// rather than derived from the TUI one by a fixed offset.
    fn activate(
        &mut self,
        anchor: &str,
        tty_tabs: usize,
        desktop_tabs: usize,
        marker: &str,
        text: &str,
    ) {
        let tabs = match self {
            Self::Desktop(_) => desktop_tabs,
            Self::Tty(_) => tty_tabs,
        };
        self.key(anchor);
        for _ in 0..tabs {
            self.key("tab");
        }
        self.key("enter");
        self.wait(marker, text);
    }
    /// Move the review tabs (Queue / Detail / History) by a relative delta with
    /// the global alt+left/right review chord on both surfaces. The app registers
    /// both the TUI ("alt+ArrowLeft/Right") and GTK desktop ("alt+Left/Right") key
    /// spellings, so the same chord reaches the shared tab handler on either.
    fn tab_nav(&mut self, delta: i32) {
        let steps = delta.unsigned_abs();
        let chord = if delta > 0 { "alt+right" } else { "alt+left" };
        for _ in 0..steps {
            self.key(chord);
        }
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
    let nsh = repo.join("apps/operator/target/debug").join(format!(
        "acp-nsh-{}{}",
        if surface == "desktop" {
            "native"
        } else {
            "tui"
        },
        std::env::consts::EXE_SUFFIX
    ));
    let appearance = match std::env::var("ACP_S5_APPEARANCE").as_deref() {
        Ok("dark") => "dark",
        Ok("light") | Err(std::env::VarError::NotPresent) => "light",
        _ => panic!("fixed appearance required"),
    };
    let args = vec![
        "--runtime-dir".into(),
        f.runtime.to_str().unwrap().into(),
        "--endpoint".into(),
        f.alias.clone(),
        "--operator".into(),
        "--appearance".into(),
        appearance.into(),
    ];
    let mut ui = if surface == "desktop" {
        Ui::Desktop(Box::new(desktop::Desktop::start(
            &nsh,
            &args,
            &nonce,
            window_script,
            out,
        )))
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
    // The review workspace renders below the preference records; give both
    // surfaces enough height that the Queue/Detail/History content and the
    // destructive tier stay inside the captured viewport. Restored below.
    ui.resize(
        if surface == "desktop" { 1080 } else { 160 },
        if surface == "desktop" { 1000 } else { 90 },
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
    ui.key("alt+shift+y");
    ui.wait("inspected_id=1 inspected_revision=1", "Show full digest");
    ui.receipt("alt+shift+y", 5);
    assert_eq!(f.rows()[0]["status"], "confirmed");
    ui.capture("confirmed-exact-record", out);
    ui.receipt("alt+shift+s", 6);
    ui.key("alt+shift+l");
    ui.wait("rows=2", "#2");
    ui.key("alt+shift+i");
    ui.key("alt+shift+i");
    ui.wait("record_revision=1", "#2");
    ui.key("alt+shift+n");
    ui.wait("inspected_id=2 inspected_revision=1", "Show full digest");
    ui.receipt("alt+shift+n", 7);
    assert_eq!(f.rows()[1]["status"], "rejected");
    fs::write(f.root.path().join("drop-next"), b"synthetic fault").unwrap();
    ui.apply("alt+shift+s", "unknown=true", "Outcome unknown");
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
    ui.wait("inspected_id=3 inspected_revision=1", "Show full digest");
    ui.key("alt+shift+y");
    ui.wait("review=true", "Prepared operation");
    // S1 made `--proposal-digest` mandatory for every decision. Bind the exact
    // persisted digest of record #3 so the competing CLI reject names the same
    // proposal the UI prepared; a stale or absent digest is a clap usage error.
    let digest3 = f.digest(3);
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
            "--proposal-digest",
            &digest3,
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
    ui.key("alt+shift+h");
    ui.wait("handoff_revision=1", "Saved synthetic history");
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
    // ---- S2 review workspace: Queue / Detail / History tabs, deprecate and the
    // destructive delete tier, driven by real chords on both surfaces. A dedicated
    // suggestion (#4) walks the whole lifecycle and demonstrates atomic demotion
    // of the same-key confirmed peer (#1); deleting it returns the row count to
    // three without restoring the deprecated peer. ----
    ui.field("alt+k", "editor");
    ui.field("alt+w", "synthetic-value");
    ui.field("alt+f", "synthetic-source");
    let n = f.commit_sequence() + 1;
    ui.receipt("alt+shift+s", n);
    assert_eq!(f.rows().len(), 4);
    let id4 = f.rows()[3]["id"].as_i64().unwrap();
    assert_eq!(id4, 4);
    assert_eq!(f.record(id4)["status"], "suggested");
    // The S1 digest gate is real: a wrong proposal digest is refused before any
    // write, and the record stays suggested.
    let (ok, v) = f.try_mutate(
        "wrong-digest-confirm",
        &[
            "preference-confirm",
            "4",
            "--scope",
            "project",
            "--project",
            "alpha",
            "--expected-revision",
            "1",
            "--proposal-digest",
            &"0".repeat(64),
            "--confirm",
        ],
    );
    assert!(!ok, "a wrong proposal digest must be refused");
    assert_eq!(v["error"], "ReviewMismatch");
    assert_eq!(f.record(id4)["status"], "suggested");
    // Queue tab: load the suggested-only page. The load action is a rendered
    // button reached by the fixed Tab count from the source-field anchor (see
    // Ui::activate); the disabled First/Next paging controls are skipped.
    ui.activate("alt+f", 4, 3, "queue=1", "#4");
    ui.capture("review-queue", out);
    // Open the exact record from its queue row; Detail auto-selects and inspects.
    ui.activate(
        "alt+f",
        5,
        3,
        "inspected_id=4 inspected_revision=1",
        "Show full digest",
    );
    ui.wait("peer_total=1", "Deprecates 1 same-key confirmed records");
    ui.capture("review-detail", out);
    // Exercise the same global Alt+left/right review chords on both surfaces.
    ui.tab_nav(1);
    ui.wait(
        "review_tab=History",
        "No decisions recorded for this record",
    );
    ui.capture("review-history-empty", out);
    ui.tab_nav(-1);
    ui.wait("review_tab=Detail", "Show full digest");
    ui.tab_nav(-1);
    ui.wait("review_tab=Queue", "#4");
    ui.tab_nav(1);
    ui.wait("review_tab=Detail", "Show full digest");
    // The digest is a 16-hex prefix until an explicit expand reveals the whole.
    let digest4 = f.digest(id4);
    ui.activate("alt+f", 4, 3, "digest_expanded=true", "Collapse digest");
    ui.capture("review-digest-expanded", out);
    if let Ui::Tty(t) = &ui {
        assert!(
            t.state.lock().unwrap().text().contains(&digest4),
            "expanded proposal digest not rendered"
        );
    }
    // Confirm the suggestion through the shared chord; it becomes confirmed.
    let n = f.commit_sequence() + 1;
    ui.receipt("alt+shift+y", n);
    assert_eq!(f.record(id4)["status"], "confirmed");
    ui.capture("review-confirmed", out);
    // Deprecate the confirmed record: re-inspect it from the record list, read its
    // decision history, then prepare/apply the deprecate chord.
    ui.key("alt+shift+l");
    ui.wait("rows=4", "#4");
    for _ in 0..4 {
        ui.key("alt+shift+i");
    }
    // A decision chord without a current inspect performs only that read.
    ui.key("alt+shift+w");
    ui.wait("inspected_id=4 inspected_revision=2", "Show full digest");
    ui.tab_nav(1);
    ui.wait("review_tab=History", "Channel is client-asserted");
    ui.capture("review-history-confirm", out);
    let inspect = f
        .query(&[
            "review-inspect",
            "4",
            "--scope",
            "project",
            "--project",
            "alpha",
            "--revision",
            "2",
        ])
        .1;
    assert!(
        inspect["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["action"] == "confirm"),
        "history must show the confirm decision"
    );
    ui.tab_nav(-1);
    ui.wait(
        "review_tab=Detail",
        "Deprecation makes the record inactive in agent effective context.",
    );
    let n = f.commit_sequence() + 1;
    ui.receipt("alt+shift+w", n);
    assert_eq!(f.record(id4)["status"], "deprecated");
    assert_eq!(f.record(1)["status"], "deprecated");
    ui.capture("review-deprecated", out);
    // Delete through the destructive tier. Re-inspect the deprecated record first.
    ui.key("alt+shift+l");
    ui.wait("rows=4", "#4");
    for _ in 0..4 {
        ui.key("alt+shift+i");
    }
    ui.key("alt+shift+q");
    ui.wait("inspected_id=4 inspected_revision=3", "Show full digest");
    let inspect = f
        .query(&[
            "review-inspect",
            "4",
            "--scope",
            "project",
            "--project",
            "alpha",
            "--revision",
            "3",
        ])
        .1;
    let actions: Vec<_> = inspect["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["action"].as_str().unwrap())
        .collect();
    assert!(actions.contains(&"confirm") && actions.contains(&"deprecate"));
    let channel = if surface == "desktop" {
        "desktop"
    } else {
        "tui"
    };
    for (action, from, to) in [("confirm", 1, 2), ("deprecate", 2, 3)] {
        let decision = inspect["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["action"] == action)
            .unwrap();
        assert_eq!(decision["channel"], channel);
        assert_eq!(decision["from_revision"], from);
        assert_eq!(decision["to_revision"], to);
        assert_eq!(decision["proposal_digest"], digest4);
    }
    let peer = f
        .query(&[
            "review-inspect",
            "1",
            "--scope",
            "project",
            "--project",
            "alpha",
            "--revision",
            "3",
        ])
        .1;
    let demotion = peer["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["action"] == "demote")
        .unwrap();
    assert_eq!(demotion["channel"], channel);
    assert_eq!(demotion["from_revision"], 2);
    assert_eq!(demotion["to_revision"], 3);
    fs::write(
        out.join("review-history-proof.json"),
        serde_json::to_vec_pretty(&inspect["decisions"]).unwrap(),
    )
    .unwrap();
    // NEGATIVE: a wrong retyped ID and a blind Enter on the empty field must both
    // refuse the delete; no write lands and the record stays present.
    let seq_before = f.commit_sequence();
    // Wrong retype: the tier shows the mismatch and Enter prepares nothing.
    ui.resize(
        if surface == "desktop" { 640 } else { 80 },
        if surface == "desktop" { 480 } else { 30 },
    );
    ui.key("alt+shift+q");
    ui.wait(
        "ACP_UI_DIALOG opened=true initial_focus=true focus_bound=true",
        "Delete project alpha preference #4?",
    );
    ui.capture("review-delete-compact", out);
    ui.text("999");
    ui.wait(
        "delete_retype_chars=3 delete_ready=false",
        "The typed ID does not match",
    );
    ui.key("enter");
    ui.capture("review-delete-refused", out);
    assert_eq!(
        f.commit_sequence(),
        seq_before,
        "a wrong retype must not write"
    );
    assert_eq!(f.record(id4)["status"], "deprecated");
    assert_eq!(f.record(id4)["revision"], 3);
    // Escape cancels only the destructive tier and discards its retype.
    ui.key("escape");
    match &ui {
        Ui::Desktop(d) => d.wait("delete_armed=false"),
        Ui::Tty(t) => t.wait_gone("Delete project alpha preference #4?"),
    }
    ui.resize(
        if surface == "desktop" { 1080 } else { 160 },
        if surface == "desktop" { 1000 } else { 90 },
    );
    // Blind Enter on a fresh empty retype field also prepares nothing.
    ui.key("alt+shift+q");
    ui.wait(
        "ACP_UI_DIALOG opened=true initial_focus=true focus_bound=true",
        "Delete project alpha preference #4?",
    );
    ui.key("enter");
    assert_eq!(
        f.commit_sequence(),
        seq_before,
        "a blind Enter must not write"
    );
    assert_eq!(f.record(id4)["status"], "deprecated");
    ui.key("escape");
    ui.wait("delete_armed=false", "Show full digest");
    // POSITIVE: arm with the exact ID and apply; only the exact retype prepares
    // and commits the delete.
    ui.key("alt+shift+q");
    ui.wait(
        "ACP_UI_DIALOG opened=true initial_focus=true focus_bound=true",
        "Delete project alpha preference #4?",
    );
    ui.text("4");
    ui.wait(
        "delete_retype_chars=1 delete_ready=true",
        "Deletion armed; apply to commit",
    );
    ui.capture("review-delete-armed", out);
    let n = f.commit_sequence() + 1;
    ui.key("alt+shift+v");
    ui.wait(
        &format!("receipt={n} "),
        "Deleted. Content and decision history are gone; receipts remain identity-only.",
    );
    ui.capture("review-deleted", out);
    // The deleted record keeps no content or decision history; the queue is empty
    // and the row count returns to the three earlier records.
    let (ok, _) = f.query(&[
        "preferences",
        "get",
        "4",
        "--scope",
        "project",
        "--project",
        "alpha",
    ]);
    assert!(!ok, "a deleted record must keep no content");
    let (ok, deleted) = f.query(&[
        "review-inspect",
        "4",
        "--scope",
        "project",
        "--project",
        "alpha",
        "--revision",
        "3",
    ]);
    assert!(
        !ok,
        "deleted content and decision history must be unavailable"
    );
    assert_eq!(deleted["error"], "NotFound");
    let queue = f
        .query(&["review-queue", "--scope", "project", "--project", "alpha"])
        .1;
    assert!(queue["records"].as_array().unwrap().is_empty());
    assert_eq!(f.rows().len(), 3);
    // ---- end S2 review workspace ----
    // The tall viewport stays until the existing compact/restore resize below
    // returns the surface to its normal size; a height-only resize here redraws
    // the TUI with collapsed spaces.
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
    for (i, section) in [
        "Dashboard",
        "Projects",
        "Preferences",
        "Handoff",
        "Diagnostics",
    ]
    .iter()
    .enumerate()
    {
        let n = i + 1;
        ui.key(&format!("alt+{n}"));
        thread::sleep(Duration::from_millis(150));
        if let Ui::Desktop(d) = &ui {
            d.wait(&format!("section={section}"));
        }
        ui.capture(&format!("page-{n}"), out);
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
    // Inspect real retained content/history in zh-CN, then prove the compact
    // destructive disclosure/refusal without another preference write.
    ui.resize(
        if surface == "desktop" { 1080 } else { 160 },
        if surface == "desktop" { 1000 } else { 90 },
    );
    ui.key("alt+shift+l");
    ui.wait("rows=3", "#1");
    ui.key("alt+shift+i");
    ui.key("alt+shift+w");
    ui.wait(
        "inspection_current=true inspected_id=1 inspected_revision=3",
        "修订: 3",
    );
    ui.capture("review-zh-detail", out);
    ui.tab_nav(1);
    ui.wait("review_tab=History", "渠道由客户端声明");
    ui.capture("review-zh-history", out);
    ui.key("alt+shift+e");
    ui.wait("review=true", "操作已准备，尚未发送写入");
    ui.key("alt+shift+v");
    ui.wait("selected=true grant=true busy=false", "写入授权有效");
    // A grant-epoch change deliberately clears selection and inspection.
    ui.key("alt+shift+l");
    ui.wait("rows=3", "#1");
    ui.key("alt+shift+i");
    ui.key("alt+shift+w");
    ui.wait(
        "inspection_current=true inspected_id=1 inspected_revision=3",
        "修订: 3",
    );
    ui.resize(
        if surface == "desktop" { 640 } else { 80 },
        if surface == "desktop" { 480 } else { 30 },
    );
    ui.key("alt+shift+q");
    ui.wait(
        "ACP_UI_DIALOG opened=true initial_focus=true focus_bound=true",
        "偏好 #1？",
    );
    let zh_sequence = f.commit_sequence();
    ui.text("999");
    ui.wait(
        "delete_retype_chars=3 delete_ready=false",
        "输入的 ID 不匹配",
    );
    ui.key("enter");
    ui.capture("review-zh-delete-refused", out);
    assert_eq!(f.commit_sequence(), zh_sequence);
    assert_eq!(f.record(1)["revision"], 3);
    assert_eq!(f.record(1)["status"], "deprecated");
    ui.key("escape");
    match &ui {
        Ui::Desktop(d) => d.wait("delete_armed=false"),
        Ui::Tty(t) => t.wait_gone("偏好 #1？"),
    }
    ui.resize(
        if surface == "desktop" { 1080 } else { 160 },
        if surface == "desktop" { 760 } else { 50 },
    );
    ui.key("ctrl+l");
    ui.apply(
        "alt+shift+x",
        "selected=true grant=false busy=false",
        "Write grant off",
    );
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
    json!({"status":if development{"DEV_PASS"}else{"PASS"},"tested_git_sha":sha,"worktree_dirty":dirty,"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"surface":surface,"appearance":appearance,"backend":backend,"binaries":binaries,"actions":["unselected Profile","explicit UI create","Busy","enable","exact suggestion/confirm/reject","committed lost reply and explicit same-request retry","Conflict","Escape/project review invalidation","saved exact-task Handoff NOT_RUN history","S2 review queue load and Queue/Detail/History tabs","exact inspect with digest prefix expand","wrong-digest CLI ReviewMismatch","confirm/deprecate via chords","destructive delete wrong-retype/blind-Enter refusal","exact-retype delete with identity-only receipt","deleted record keeps no content; empty queue","disable acknowledgement","revoked-grant Unauthorized","five pages/Sessions","theme/locale/resize","disconnect","clean child exit"],"rows":3,"historical_source_substitution":false})
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
