//! Owned synthetic integration only. No profile discovery, model or user credentials.
mod desktop;
mod evidence;
mod relay;
mod scenarios;
mod wsl;
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
fn binary_fingerprints(paths: &[PathBuf]) -> Vec<Value> {
    use std::io::Read;
    let mut buffer = [0u8; 64 * 1024];
    paths
        .iter()
        .map(|path| {
            let mut file = fs::File::open(path).unwrap();
            let mut hash = Sha256::new();
            loop {
                let count = file.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            json!({
                "name":path.file_name().unwrap().to_str().unwrap(),
                "sha256":format!("{:x}",hash.finalize())
            })
        })
        .collect()
}
struct Fixture {
    root: tempfile::TempDir,
    repo: PathBuf,
    daemon: Owned,
    relay: Option<Owned>,
    runtime: PathBuf,
    endpoint: String,
    alias: String,
    focus: PathBuf,
    db: PathBuf,
}
impl Fixture {
    fn new(repo: PathBuf) -> Self {
        let root = tempfile::Builder::new()
            .prefix("precept-s5-")
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
        precept_ipc::transport::private_dir(&root.path().join("state"), true).unwrap();
        for name in ["alpha", "beta"] {
            fs::create_dir(root.path().join(name)).unwrap();
            fs::write(
                root.path().join(name).join("AGENTS.md"),
                "Synthetic local contract.\n",
            )
            .unwrap();
        }
        let focus = root.path().join("state/focus.events");
        let mut focus_options = fs::OpenOptions::new();
        focus_options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            focus_options.mode(0o600);
        }
        focus_options.open(&focus).unwrap();
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
            focus,
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
        let mut c = Command::new(Self::bin(repo, "preceptd"));
        precept_native_acceptance::environment::clear_inherited_precept(&mut c)
            .arg("--runtime-dir")
            .arg(runtime)
            .arg("--endpoint")
            .arg(endpoint);
        // Each daemon has its own private state directory beside its runtime;
        // `db` is the Profile the operator asked to select at start.
        let state = runtime.with_extension("state");
        let (alpha, beta) = (base.join("alpha"), base.join("beta"));
        precept_native_acceptance::daemon_state::write(
            &state,
            &[("alpha", &alpha), ("beta", &beta)],
            db,
        );
        c.arg("--state-dir").arg(&state);
        Owned(
            c.stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        )
    }
    fn query_at(&self, runtime: &Path, endpoint: &str, args: &[&str]) -> (bool, Value) {
        let result = precept_native_acceptance::environment::clear_inherited_precept(
            &mut Command::new(Self::bin(&self.repo, "precept")),
        )
        .arg("--runtime-dir")
        .arg(runtime)
        .arg("--endpoint")
        .arg(endpoint)
        .args(args)
        .output()
        .unwrap();
        evidence::event(
            "cli",
            json!({
                "command":args.first().copied().unwrap_or(""),
                "success":result.status.success(),
                "response_sha256":format!("{:x}", Sha256::digest(&result.stdout)),
                "response_bytes":result.stdout.len()
            }),
        );
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
struct Ui(Box<desktop::Desktop>);
impl Ui {
    fn key(&mut self, key: &str) {
        evidence::event("os-key", json!({"chord":key}));
        match self {
            Self(d) => d.key(key),
        }
    }
    fn text(&mut self, text: &str) {
        // Never record field bytes, paths or proposal values.
        evidence::event("os-text", json!({"characters":text.chars().count()}));
        match self {
            Self(d) => d.text(text),
        }
    }
    fn field(&mut self, key: &str, text: &str) {
        let focus_after = match self {
            Self(d) => Some(d.lines.lock().unwrap().len()),
        };
        // A Tab before the focus chord ensures a real focus event even if this
        // input was already focused. Snapshot before both keys: Tab itself may
        // land on the requested field. Neither key activates a business action.
        if focus_after.is_some() {
            self.key("tab");
        }
        self.key(key);
        if let (Self(d), Some(after)) = (&self, focus_after) {
            d.wait_after(&format!("PRECEPT_UI_FOCUS shortcut={key}"), after);
        }
        // ctrl+a selects all, so typing replaces the field content.
        self.key("ctrl+a");
        self.text(text);
    }
    /// Activate a rendered button using only its anchor chord, Tab stops and
    /// Enter/Space. Disabled and concealed controls are skipped.
    /// Counts are calibrated independently on the real GTK and HWND chains.
    /// Load/digest and queue-row stops respectively: GTK 3/3, HWND 3/4.
    /// Read-only UI Automation confirms HWND's load action precedes the queue
    /// row. The observer never actuates a control.
    fn activate(
        &mut self,
        (anchor, activation): (&str, &str),
        desktop_tabs: (usize, usize),
        marker: &str,
        out: &Path,
    ) {
        let tabs = match self {
            Self(_) if cfg!(windows) => desktop_tabs.1,
            Self(_) => desktop_tabs.0,
        };
        let trace = std::env::var_os("PRECEPT_S5_KEYBOARD_TRACE").is_some();
        self.key(anchor);
        let trace_name = trace.then(|| {
            marker
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect::<String>()
        });
        if let Some(name) = &trace_name {
            self.capture(&format!("keyboard-{name}-anchor"), out);
        }
        for i in 0..tabs {
            self.key("tab");
            if let Some(name) = &trace_name {
                self.capture(&format!("keyboard-{name}-tab-{}", i + 1), out);
            }
        }
        self.key(activation);
        self.wait(marker);
    }
    /// Move the review tabs (Queue / Detail / History) by a relative delta with
    /// the global alt+left/right review chord. The app registers both the
    /// "alt+ArrowLeft/Right" and "alt+Left/Right" key spellings, so the chord
    /// reaches the tab handler on every platform.
    fn tab_nav(&mut self, delta: i32) {
        let steps = delta.unsigned_abs();
        let chord = if delta > 0 { "alt+right" } else { "alt+left" };
        for _ in 0..steps {
            self.key(chord);
        }
    }
    fn wait(&self, marker: &str) {
        match self {
            Self(d) => {
                d.wait(marker);
                evidence::event(
                    "surface-observation",
                    json!({"observer":"owned-GUI-probe","marker":marker,"pixels_require_independent_review":true}),
                );
            }
        }
    }
    fn apply(&mut self, key: &str, marker: &str) {
        self.key(key);
        self.wait("review=true");
        self.key("alt+shift+v");
        self.wait(marker);
    }
    fn receipt(&mut self, key: &str, n: i64) {
        self.apply(key, &format!("receipt={n} "));
    }
    fn capture(&mut self, name: &str, out: &Path) {
        let suffix = match self {
            Self(_) => "png",
        };
        let directory = out.file_name().unwrap().to_string_lossy();
        let artifact = if directory.starts_with("s3-") {
            format!("{directory}/{name}.{suffix}")
        } else {
            format!("{name}.{suffix}")
        };
        evidence::event("capture-attempt", json!({"artifact":&artifact}));
        match self {
            Self(d) => d.capture(name, out),
        }
        evidence::event(
            "capture",
            json!({"artifact":artifact,"pixels_or_terminal_frame":true}),
        );
    }
    fn resize(&mut self, w: u16, h: u16) {
        match self {
            Self(d) => d.resize(w, h),
        }
    }
    fn close(&mut self) {
        match self {
            Self(d) => d.close(),
        }
    }
}
impl Ui {
    fn pid(&self) -> u32 {
        match self {
            Self(d) => d.pid(),
        }
    }
    fn action(&mut self, key: &str, focus: &Path, anchor: Option<&str>) {
        if let Some(anchor) = anchor {
            self.key(anchor);
        }
        use std::io::{Read, Seek, SeekFrom};
        let mut source = fs::File::open(focus).unwrap();
        let mut observed = String::new();
        for stop in 0..128 {
            source.seek(SeekFrom::End(0)).unwrap();
            self.key("tab");
            observed.clear();
            source.read_to_string(&mut observed).unwrap();
            assert!(
                source.metadata().unwrap().len() <= 256 * 1024,
                "bounded focus observation"
            );
            for catalog_key in observed.lines() {
                evidence::event(
                    "native-focus",
                    json!({"catalog_key":catalog_key,"tab_stop":stop}),
                );
                if catalog_key == key {
                    self.key("enter");
                    return;
                }
            }
        }
        evidence::event(
            "missing-native-focus-target",
            json!({"catalog_key":key,"reason":"128-tab-observation-budget-exhausted"}),
        );
        std::panic::panic_any(scenarios::MissingFocus);
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
    evidence::begin(
        "profile-owned-flow",
        "actual-os-input + owned-daemon + independent-cli",
    );
    let mut f = Fixture::new(repo.clone());
    let nonce = f.root.path().file_name().unwrap().to_string_lossy();
    let hash = nonce
        .bytes()
        .fold(0u64, |a, b| a.wrapping_mul(131).wrapping_add(b as u64));
    let nonce = format!("{hash:016x}");
    let nsh = repo.join("apps/target/debug").join(format!(
        "precept-nsh-{}{}",
        "native",
        std::env::consts::EXE_SUFFIX
    ));
    let appearance = match std::env::var("PRECEPT_S5_APPEARANCE").as_deref() {
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
    let mut ui = Ui(Box::new(desktop::Desktop::start(
        &nsh,
        &args,
        &nonce,
        window_script,
        out,
        &f.focus,
    )));
    evidence::event("owned-ui", json!({"pid":ui.pid(),"nonce":nonce}));
    ui.key("alt+o");
    ui.wait("selected=false grant=false busy=false");
    ui.field("alt+p", f.db.to_str().unwrap());
    ui.key("alt+shift+u");
    ui.wait("review=true");
    ui.capture("create-review", out);
    ui.key("alt+shift+v");
    ui.wait("selected=true grant=false busy=false");
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
    ui.apply("alt+shift+e", "error=Busy");
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
    ui.wait("selected=true grant=false busy=false");
    ui.apply("alt+shift+e", "selected=true grant=true busy=false");
    assert_eq!(f.query(&["profile", "info"]).1["write_enabled"], true);
    evidence::finish("PASS");
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
    ui.resize(1080, 1000);
    ui.key("ctrl+r");
    ui.wait("phase=Ready");
    ui.field("alt+k", "editor");
    ui.field("alt+w", "synthetic-value");
    ui.field("alt+f", "synthetic-source");
    ui.receipt("alt+shift+s", 4);
    evidence::begin(
        "exact-inspect-provenance",
        "actual-os-input + independent-cli",
    );
    assert_eq!(f.rows()[0]["status"], "suggested");
    ui.key("alt+shift+l");
    ui.wait("rows=1");
    ui.key("alt+shift+i");
    ui.wait("record_revision=1");
    ui.key("alt+shift+y");
    ui.wait("inspected_id=1 inspected_revision=1");
    let exact = f.query(&[
        "review-inspect",
        "1",
        "--scope",
        "project",
        "--project",
        "alpha",
        "--revision",
        "1",
    ]);
    assert!(exact.0);
    assert_eq!(exact.1["record"], f.record(1));
    assert_eq!(
        exact.1["record"]["scope"],
        json!({"kind":"project","name":"alpha"})
    );
    assert_eq!(exact.1["record"]["key"], "editor");
    assert_eq!(exact.1["record"]["value"], "synthetic-value");
    assert_eq!(exact.1["record"]["source"], "synthetic-source");
    assert_eq!(exact.1["record"]["origin"], "agent_reported");
    assert!(exact.1["decisions"].as_array().unwrap().len() <= 8);
    evidence::event(
        "exact-immutable-inspection-cli-proof",
        json!({
            "id":1,"revision":1,"proposal_digest":exact.1["record"]["proposal_digest"],
            "all_immutable_fields_equal":true,"scope":"project-alpha","history_bounded":true,
            "native_render":false
        }),
    );
    ui.capture("inspected-exact-immutable-provenance", out);
    ui.receipt("alt+shift+y", 5);
    assert_eq!(f.rows()[0]["status"], "confirmed");
    ui.capture("confirmed-exact-record", out);
    evidence::finish("PASS");
    evidence::begin("reject", "actual-os-input + independent-cli");
    ui.receipt("alt+shift+s", 6);
    ui.key("alt+shift+l");
    ui.wait("rows=2");
    ui.key("alt+shift+i");
    ui.key("alt+shift+i");
    ui.wait("record_revision=1");
    ui.key("alt+shift+n");
    ui.wait("inspected_id=2 inspected_revision=1");
    ui.receipt("alt+shift+n", 7);
    assert_eq!(f.rows()[1]["status"], "rejected");
    evidence::finish("PASS");
    fs::write(f.root.path().join("drop-next"), b"synthetic fault").unwrap();
    ui.apply("alt+shift+s", "unknown=true");
    ui.capture("outcome-unknown", out);
    let before = f.rows();
    assert_eq!(before.len(), 3);
    assert!(!f.root.path().join("drop-next").exists());
    thread::sleep(Duration::from_millis(500));
    assert_eq!(
        f.rows(),
        before,
        "unknown outcome must not automatically replay"
    );
    assert!(!f.root.path().join("replayed-receipt.json").exists());
    ui.key("alt+shift+z");
    ui.wait("receipt=8 ");
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
    ui.wait("selected=true grant=true busy=false");
    ui.key("alt+shift+l");
    ui.wait("rows=3");
    ui.key("alt+shift+i");
    ui.key("alt+shift+i");
    ui.key("alt+shift+i");
    ui.wait("record_revision=1");
    ui.key("alt+shift+y");
    ui.wait("inspected_id=3 inspected_revision=1");
    ui.key("alt+shift+y");
    evidence::begin(
        "stale-revision",
        "actual-os-input + independent-concurrent-cli",
    );
    ui.wait("review=true");
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
    ui.wait("error=Conflict");
    assert_eq!(f.rows()[2]["status"], "rejected");
    ui.capture("stale-conflict", out);
    evidence::finish("PASS");
    ui.key("alt+shift+r");
    ui.wait("selected=true grant=true busy=false");
    ui.key("alt+shift+s");
    ui.wait("review=true");
    ui.key("escape");
    ui.key("alt+o");
    ui.wait("selected=true grant=true busy=false");
    ui.key("alt+shift+s");
    ui.wait("review=true");
    ui.key("ctrl+p");
    thread::sleep(Duration::from_millis(500));
    ui.key("ctrl+p");
    thread::sleep(Duration::from_millis(500));
    ui.key("alt+shift+v");
    thread::sleep(Duration::from_millis(200));
    assert_eq!(f.rows().len(), 3);
    ui.field("alt+t", "shared");
    ui.key("alt+shift+h");
    ui.wait("handoff_revision=1");
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
    ui.wait("selected=true grant=true busy=false");
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
    evidence::begin("altered-digest", "independent-cli-only (not-native-typing)");
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
    evidence::finish("PASS");
    // Queue tab: load the suggested-only page. The load action is a rendered
    // button reached by the fixed Tab count from the source-field anchor (see
    // Ui::activate); the disabled First/Next paging controls are skipped.
    evidence::begin(
        "tab-enter-space-alt",
        "actual-os-input + owned-surface-render",
    );
    ui.activate(("alt+f", "enter"), (3, 3), "queue=1", out);
    ui.capture("review-queue", out);
    // Open the exact record from its queue row; Detail auto-selects and inspects.
    ui.activate(
        ("alt+f", "enter"),
        (3, 4),
        "inspected_id=4 inspected_revision=1",
        out,
    );
    ui.wait("peer_total=1");
    ui.capture("review-detail", out);
    // Exercise the same global Alt+left/right review chords on both surfaces.
    ui.tab_nav(1);
    ui.wait("review_tab=History");
    ui.capture("review-history-empty", out);
    ui.tab_nav(-1);
    ui.wait("review_tab=Detail");
    ui.tab_nav(-1);
    ui.wait("review_tab=Queue");
    ui.tab_nav(1);
    ui.wait("review_tab=Detail");
    // The digest is a 16-hex prefix until an explicit expand reveals the whole.
    let digest4 = f.digest(id4);
    ui.activate(("alt+f", "space"), (3, 3), "digest_expanded=true", out);
    ui.capture("review-digest-expanded", out);
    evidence::finish("PASS");
    // Confirm the suggestion through the shared chord; it becomes confirmed.
    evidence::begin("confirm-peer-demotion", "actual-os-input + independent-cli");
    let n = f.commit_sequence() + 1;
    ui.receipt("alt+shift+y", n);
    assert_eq!(f.record(id4)["status"], "confirmed");
    assert_eq!(f.record(1)["status"], "deprecated");
    let target_inspect = f.query(&[
        "review-inspect",
        "4",
        "--scope",
        "project",
        "--project",
        "alpha",
        "--revision",
        "2",
    ]);
    let peer_inspect = f.query(&[
        "review-inspect",
        "1",
        "--scope",
        "project",
        "--project",
        "alpha",
        "--revision",
        "3",
    ]);
    assert!(target_inspect.0 && peer_inspect.0);
    let target_decision = target_inspect.1["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["action"] == "confirm")
        .unwrap();
    let peer_decision = peer_inspect.1["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["action"] == "demote")
        .unwrap();
    assert_eq!(target_decision["receipt_commit_sequence"], n);
    assert_eq!(peer_decision["receipt_commit_sequence"], n);
    fs::write(
        out.join("atomic-peer-proof.json"),
        serde_json::to_vec_pretty(&json!({
            "target":target_decision,"peer":peer_decision,"shared_commit_sequence":n
        }))
        .unwrap(),
    )
    .unwrap();
    evidence::event(
        "atomic-peer-demotion-proof",
        json!({"artifact":"atomic-peer-proof.json","native_render":false}),
    );
    ui.capture("review-confirmed", out);
    evidence::finish("PASS");
    // Deprecate the confirmed record: re-inspect it from the record list, read its
    evidence::begin("history-bounded", "actual-os-input + independent-cli");
    // decision history, then prepare/apply the deprecate chord.
    ui.key("alt+shift+l");
    ui.wait("rows=4");
    for _ in 0..4 {
        ui.key("alt+shift+i");
    }
    // A decision chord without a current inspect performs only that read.
    ui.key("alt+shift+w");
    ui.wait("inspected_id=4 inspected_revision=2");
    ui.tab_nav(1);
    ui.wait("review_tab=History");
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
    assert!(inspect["decisions"].as_array().unwrap().len() <= 8);
    evidence::finish("PASS");
    evidence::begin("deprecate", "actual-os-input + independent-cli");
    ui.tab_nav(-1);
    ui.wait("review_tab=Detail");
    let n = f.commit_sequence() + 1;
    ui.receipt("alt+shift+w", n);
    assert_eq!(f.record(id4)["status"], "deprecated");
    assert_eq!(f.record(1)["status"], "deprecated");
    ui.capture("review-deprecated", out);
    evidence::finish("PASS");
    // Delete through the destructive tier. Re-inspect the deprecated record first.
    ui.key("alt+shift+l");
    ui.wait("rows=4");
    for _ in 0..4 {
        ui.key("alt+shift+i");
    }
    ui.key("alt+shift+q");
    ui.wait("inspected_id=4 inspected_revision=3");
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
    let channel = "desktop";
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
    assert!(inspect["decisions"].as_array().unwrap().len() <= 8);
    let confirmed = inspect["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["action"] == "confirm")
        .unwrap();
    assert_eq!(
        demotion["receipt_commit_sequence"],
        confirmed["receipt_commit_sequence"]
    );
    evidence::event(
        "atomic-peer-demotion-cli-proof",
        json!({
            "target":4,"peer":1,"shared_commit_sequence":confirmed["receipt_commit_sequence"],
            "client_asserted_channel":channel,"native_render":false
        }),
    );
    fs::write(
        out.join("review-history-proof.json"),
        serde_json::to_vec_pretty(&inspect["decisions"]).unwrap(),
    )
    .unwrap();
    // NEGATIVE: a wrong retyped ID and a blind Enter on the empty field must both
    // refuse the delete; no write lands and the record stays present.
    let seq_before = f.commit_sequence();
    evidence::begin("delete-wrong-id", "actual-os-input + independent-cli");
    // Wrong retype: the tier shows the mismatch and Enter prepares nothing.
    ui.resize(640, 480);
    ui.key("alt+shift+q");
    ui.wait("PRECEPT_UI_DIALOG opened=true initial_focus=true focus_bound=true");
    ui.capture("review-delete-compact", out);
    ui.text("999");
    ui.wait("delete_retype_chars=3 delete_ready=false");
    ui.key("enter");
    ui.capture("review-delete-refused", out);
    assert_eq!(
        f.commit_sequence(),
        seq_before,
        "a wrong retype must not write"
    );
    assert_eq!(f.record(id4)["status"], "deprecated");
    assert_eq!(f.record(id4)["revision"], 3);
    evidence::finish("PASS");
    evidence::begin("delete-escape-reopen", "actual-os-input + independent-cli");
    // Escape cancels only the destructive tier and discards its retype.
    ui.key("escape");
    match &ui {
        Ui(d) => d.wait("delete_armed=false"),
    }
    ui.resize(1080, 1000);
    // Blind Enter on a fresh empty retype field also prepares nothing.
    ui.key("alt+shift+q");
    ui.wait("PRECEPT_UI_DIALOG opened=true initial_focus=true focus_bound=true");
    ui.wait("delete_retype_chars=0 delete_ready=false");
    evidence::finish("PASS");
    evidence::begin("delete-blind-enter", "actual-os-input + independent-cli");
    ui.key("enter");
    assert_eq!(
        f.commit_sequence(),
        seq_before,
        "a blind Enter must not write"
    );
    assert_eq!(f.record(id4)["status"], "deprecated");
    ui.key("escape");
    ui.wait("delete_armed=false");
    evidence::finish("PASS");
    evidence::begin("delete-exact-id", "actual-os-input + independent-cli");
    // POSITIVE: arm with the exact ID and apply; only the exact retype prepares
    // and commits the delete.
    ui.key("alt+shift+q");
    ui.wait("PRECEPT_UI_DIALOG opened=true initial_focus=true focus_bound=true");
    ui.text("4");
    ui.wait("delete_retype_chars=1 delete_ready=true");
    ui.capture("review-delete-armed", out);
    let n = f.commit_sequence() + 1;
    ui.key("alt+shift+v");
    ui.wait(&format!("receipt={n} "));
    let delete_receipt: Value =
        serde_json::from_slice(&fs::read(f.root.path().join("last-receipt.json")).unwrap())
            .unwrap();
    assert_eq!(delete_receipt["commit_sequence"], n);
    assert_eq!(delete_receipt.as_object().unwrap().len(), 4);
    let affected = delete_receipt["affected"].as_array().unwrap();
    assert_eq!(affected.len(), 1);
    assert_eq!(affected[0]["record_id"], "4");
    assert_eq!(affected[0]["deleted"], true);
    assert_eq!(affected[0].as_object().unwrap().len(), 5);
    fs::write(
        out.join("delete-receipt-proof.json"),
        serde_json::to_vec_pretty(&delete_receipt).unwrap(),
    )
    .unwrap();
    evidence::event(
        "identity-only-delete-receipt",
        json!({"artifact":"delete-receipt-proof.json","native_render":false}),
    );
    ui.capture("review-deleted", out);
    evidence::finish("PASS");
    evidence::begin(
        "deleted-content-history",
        "actual-os-input + independent-cli",
    );
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
    evidence::finish("PASS");
    // ---- end S2 review workspace ----
    // The tall viewport stays until the existing compact/restore resize below
    // returns the surface to its normal size.
    ui.apply("alt+shift+x", "selected=true grant=false busy=false");
    assert_eq!(f.query(&["profile", "info"]).1["write_enabled"], false);
    ui.apply("alt+shift+e", "selected=true grant=true busy=false");
    assert!(f.query(&["profile", "write", "disable"]).0);
    ui.field("alt+k", "editor");
    ui.apply("alt+shift+s", "error=Unauthorized");
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
        {
            let Ui(d) = &ui;
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
    ui.resize(640, 480);
    ui.capture("compact-zh", out);
    // Inspect real retained content/history in zh-CN, then prove the compact
    // destructive disclosure/refusal without another preference write.
    ui.resize(1080, 1000);
    ui.key("alt+shift+l");
    ui.wait("rows=3");
    ui.key("alt+shift+i");
    ui.key("alt+shift+w");
    ui.wait("inspection_current=true inspected_id=1 inspected_revision=3");
    ui.capture("review-zh-detail", out);
    ui.tab_nav(1);
    ui.wait("review_tab=History");
    ui.capture("review-zh-history", out);
    ui.key("alt+shift+e");
    ui.wait("review=true");
    ui.key("alt+shift+v");
    ui.wait("selected=true grant=true busy=false");
    // A grant-epoch change deliberately clears selection and inspection.
    ui.key("alt+shift+l");
    ui.wait("rows=3");
    ui.key("alt+shift+i");
    ui.key("alt+shift+w");
    ui.wait("inspection_current=true inspected_id=1 inspected_revision=3");
    ui.resize(640, 480);
    ui.key("alt+shift+q");
    ui.wait("PRECEPT_UI_DIALOG opened=true initial_focus=true focus_bound=true");
    let zh_sequence = f.commit_sequence();
    ui.text("999");
    ui.wait("delete_retype_chars=3 delete_ready=false");
    ui.key("enter");
    ui.capture("review-zh-delete-refused", out);
    assert_eq!(f.commit_sequence(), zh_sequence);
    assert_eq!(f.record(1)["revision"], 3);
    assert_eq!(f.record(1)["status"], "deprecated");
    ui.key("escape");
    match &ui {
        Ui(d) => d.wait("delete_armed=false"),
    }
    ui.resize(1080, 760);
    ui.key("ctrl+l");
    ui.apply("alt+shift+x", "selected=true grant=false busy=false");
    ui.key("alt+shift+r");
    ui.wait("selected=true grant=false busy=false");
    f.stop_daemon();
    evidence::begin("disconnect", "actual-os-input + owned-daemon-stop");
    ui.key("alt+shift+r");
    ui.wait("error=Unavailable");
    ui.capture("disconnected", out);
    evidence::finish("PASS");
    evidence::begin(
        "keyboard-restoration",
        "actual-os-input + native-terminal-or-window-cleanup",
    );
    ui.close();
    let backend =
        match &ui {
            Ui(d) => {
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
                            .any(|l| l.starts_with("PRECEPT_GTK_FRAME after_paint="))
                    );
                    "WSLg native Wayland"
                } else {
                    "native Windows HWND"
                }
            }
        };
    evidence::finish("PASS");
    assert!(development || git(&["status", "--porcelain"]).is_empty());
    json!({"status":if development{"DEV_PASS"}else{"PASS"},"tested_git_sha":sha,"worktree_dirty":dirty,"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"surface":surface,"appearance":appearance,"backend":backend,"historical_source_substitution":false})
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
    if a.first().is_some_and(|s| s == "wsl-desktop") {
        wsl_desktop(&a[1..]);
        return;
    }
    assert!(a.len() >= 2, "surface and NEW_OUTPUT required");
    assert_eq!(a[0], "desktop");
    let out = PathBuf::from(&a[1]);
    assert!(out.is_absolute());
    let repo = std::env::current_dir().unwrap();
    let git = |args: &[&str]| {
        let result = Command::new("git").args(args).output().unwrap();
        assert!(result.status.success(), "source metadata unavailable");
        String::from_utf8(result.stdout).unwrap().trim().to_owned()
    };
    let development = std::env::var_os("PRECEPT_S5_DEVELOPMENT").is_some();
    let only = std::env::var("PRECEPT_S3_CASE").ok();
    assert!(
        only.is_none() || development,
        "case selection is development-only, never full acceptance"
    );
    let sha = git(&["rev-parse", "HEAD"]);
    let dirty = !git(&["status", "--porcelain"]).is_empty();
    assert!(!dirty || development, "clean source required");
    let artifact_root = repo
        .join(".tmp/artifacts/local-p1-m4")
        .canonicalize()
        .expect("existing owned artifact root");
    assert!(
        out.parent()
            .unwrap()
            .canonicalize()
            .unwrap()
            .starts_with(&artifact_root),
        "owned artifact tree required"
    );
    assert!(
        !out.components()
            .any(|part| matches!(part, std::path::Component::ParentDir)),
        "literal new artifact path required"
    );
    fs::create_dir(&out).expect("new report directory required");
    let nsh = repo.join("apps/target/debug").join(format!(
        "precept-nsh-{}{}",
        "native",
        std::env::consts::EXE_SUFFIX
    ));
    let binary_paths = [
        std::env::current_exe().unwrap(),
        Fixture::bin(&repo, "preceptd"),
        Fixture::bin(&repo, "precept"),
        nsh,
    ];
    let binaries = binary_fingerprints(&binary_paths);
    evidence::init(&out, &a[0], &sha, json!(binaries));
    std::panic::set_hook(Box::new(|_| {
        eprintln!("NATIVE_SCENARIO_FAILED detail=redacted; see ledger and owned captures")
    }));
    let baseline = if only.is_some() {
        Ok(json!({"status":"NOT_RUN","reason":"explicit-development-case-selection"}))
    } else {
        std::panic::catch_unwind(|| {
            run(
                &a[0],
                &out,
                a.get(2).map_or("", String::as_str),
                development,
            )
        })
    };
    if baseline.is_err() {
        evidence::finish("FAIL");
    }
    scenarios::run(&out, a.get(2).map_or("", String::as_str), only.as_deref());
    assert_eq!(
        binary_fingerprints(&binary_paths),
        binaries,
        "binaries changed during native acceptance"
    );
    let clean_after = git(&["status", "--porcelain"]).is_empty();
    let ledger = evidence::snapshot();
    let complete = ledger["cells"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| only.as_ref().is_none_or(|id| c["id"] == *id))
        .all(|c| c["status"] == "PASS");
    let pass = baseline.is_ok() && complete && (development || clean_after);
    let result = json!({
        "status":if !pass { "INCOMPLETE" } else if only.is_some() { "DEV_PARTIAL_PASS" } else if development { "DEV_PASS" } else { "PASS" },
        "tested_git_sha":sha,"worktree_dirty":dirty,"clean_after":clean_after,
        "development_case":only,
        "surface":a[0],"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,
        "binaries":binaries,"baseline":baseline.unwrap_or(json!({"status":"FAIL","detail":"see-partial-ledger"})),
        "ledger":"ledger.json","cells":ledger["cells"],
        "visual_review":"NOT_RUN (owned screenshots retained for independent inspection)",
        "historical_source_substitution":false
    });
    fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!("{result}");
    if !pass {
        std::process::exit(1);
    }
}

/// S1-B: `wsl-desktop NEW_OUTPUT WINDOW_SCRIPT DISTRIBUTION USER PREFIX`.
fn wsl_desktop(a: &[String]) {
    assert_eq!(
        a.len(),
        5,
        "wsl-desktop NEW_OUTPUT WINDOW_SCRIPT DISTRIBUTION USER PREFIX"
    );
    let out = PathBuf::from(&a[0]);
    assert!(out.is_absolute());
    let repo = std::env::current_dir().unwrap();
    let git = |args: &[&str]| {
        let result = Command::new("git").args(args).output().unwrap();
        assert!(result.status.success(), "source metadata unavailable");
        String::from_utf8(result.stdout).unwrap().trim().to_owned()
    };
    let development = std::env::var_os("PRECEPT_S5_DEVELOPMENT").is_some();
    let sha = git(&["rev-parse", "HEAD"]);
    let dirty = !git(&["status", "--porcelain"]).is_empty();
    assert!(!dirty || development, "clean source required");
    let artifact_root = repo
        .join(".tmp/artifacts/local-p1-m5")
        .canonicalize()
        .expect("existing owned artifact root");
    assert!(
        out.parent()
            .unwrap()
            .canonicalize()
            .unwrap()
            .starts_with(&artifact_root)
            && !out
                .components()
                .any(|p| matches!(p, std::path::Component::ParentDir)),
        "owned literal artifact path required"
    );
    fs::create_dir(&out).expect("new report directory required");
    let gui = repo
        .join("apps/target/debug")
        .join(format!("precept-desktop{}", std::env::consts::EXE_SUFFIX));
    let target = wsl::Wsl {
        distribution: a[2].clone(),
        user: a[3].clone(),
        prefix: a[4].clone(),
    };
    let windows = binary_fingerprints(&[std::env::current_exe().unwrap(), gui.clone()]);
    let linux = wsl::linux_binaries(&target);
    evidence::init_with(
        &out,
        "wsl-desktop",
        &sha,
        json!({"windows":windows,"linux":linux}),
        wsl::CELLS,
    );
    std::panic::set_hook(Box::new(|info| {
        eprintln!(
            "WSL_SCENARIO_FAILED at {}",
            info.location().map(|l| l.to_string()).unwrap_or_default()
        )
    }));
    let outcome = std::panic::catch_unwind(|| wsl::run(&gui, &out, &a[1], target));
    if outcome.is_err() {
        evidence::finish("FAIL");
    }
    let ledger = evidence::snapshot();
    let pass = outcome.is_ok()
        && ledger["cells"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["status"] == "PASS")
        && (development || git(&["status", "--porcelain"]).is_empty());
    let result = json!({
        "status": if !pass { "INCOMPLETE" } else if development { "DEV_PASS" } else { "PASS" },
        "tested_git_sha": sha, "worktree_dirty": dirty, "surface": "wsl-desktop",
        "platform": std::env::consts::OS, "windows_binaries": windows, "linux_binaries": linux,
        "cells": ledger["cells"], "ledger": "ledger.json",
        "visual_review": "owned screenshots retained for inspection"
    });
    fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!("{result}");
    if !pass {
        std::process::exit(1);
    }
}
