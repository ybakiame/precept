use super::*;

pub(super) struct Owned(pub(super) Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub(super) fn binary_fingerprints(paths: &[PathBuf]) -> Vec<Value> {
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
pub(super) struct Fixture {
    pub(super) root: tempfile::TempDir,
    pub(super) repo: PathBuf,
    pub(super) daemon: Owned,
    pub(super) relay: Option<Owned>,
    pub(super) runtime: PathBuf,
    pub(super) endpoint: String,
    pub(super) alias: String,
    pub(super) focus: PathBuf,
    pub(super) db: PathBuf,
}
impl Fixture {
    pub(super) fn new(repo: PathBuf) -> Self {
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
    pub(super) fn bin(repo: &Path, name: &str) -> PathBuf {
        repo.join("target/debug")
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
    }
    pub(super) fn launch(
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
    pub(super) fn query_at(&self, runtime: &Path, endpoint: &str, args: &[&str]) -> (bool, Value) {
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
    pub(super) fn query(&self, args: &[&str]) -> (bool, Value) {
        self.query_at(&self.runtime, &self.endpoint, args)
    }
    pub(super) fn try_mutate(&self, op: &str, args: &[&str]) -> (bool, Value) {
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
    pub(super) fn mutate(&self, op: &str, args: &[&str]) -> Value {
        let (ok, v) = self.try_mutate(op, args);
        assert!(ok, "synthetic CLI mutation refused: {}", v["error"]);
        v
    }
    /// The exact persisted record. Its immutable `proposal_digest` is what the
    /// S1 CLI requires for every confirm/reject/deprecate/delete; reading it back
    /// binds the decision to the precise content the operator reviewed.
    pub(super) fn record(&self, id: i64) -> Value {
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
    pub(super) fn digest(&self, id: i64) -> String {
        self.record(id)["proposal_digest"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    /// Current global transaction sequence; the next committed mutation is +1.
    pub(super) fn commit_sequence(&self) -> i64 {
        let (ok, v) = self.query(&["review-queue", "--scope", "project", "--project", "alpha"]);
        assert!(ok, "synthetic queue read refused: {}", v["error"]);
        v["commit_sequence"].as_i64().unwrap()
    }
    pub(super) fn rows(&self) -> Vec<Value> {
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
    pub(super) fn stop_daemon(&mut self) {
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
pub(super) struct Ui(pub(super) Box<desktop::Desktop>);
impl Ui {
    pub(super) fn key(&mut self, key: &str) {
        evidence::event("os-key", json!({"chord":key}));
        match self {
            Self(d) => d.key(key),
        }
    }
    pub(super) fn text(&mut self, text: &str) {
        // Never record field bytes, paths or proposal values.
        evidence::event("os-text", json!({"characters":text.chars().count()}));
        match self {
            Self(d) => d.text(text),
        }
    }
    pub(super) fn field(&mut self, key: &str, text: &str) {
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
    pub(super) fn activate(
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
    pub(super) fn tab_nav(&mut self, delta: i32) {
        let steps = delta.unsigned_abs();
        let chord = if delta > 0 { "alt+right" } else { "alt+left" };
        for _ in 0..steps {
            self.key(chord);
        }
    }
    pub(super) fn wait(&self, marker: &str) {
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
    pub(super) fn apply(&mut self, key: &str, marker: &str) {
        self.key(key);
        self.wait("review=true");
        self.key("alt+shift+v");
        self.wait(marker);
    }
    pub(super) fn receipt(&mut self, key: &str, n: i64) {
        self.apply(key, &format!("receipt={n} "));
    }
    pub(super) fn capture(&mut self, name: &str, out: &Path) {
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
    pub(super) fn resize(&mut self, w: u16, h: u16) {
        match self {
            Self(d) => d.resize(w, h),
        }
    }
    pub(super) fn close(&mut self) {
        match self {
            Self(d) => d.close(),
        }
    }
}
impl Ui {
    pub(super) fn pid(&self) -> u32 {
        match self {
            Self(d) => d.pid(),
        }
    }
    pub(super) fn action(&mut self, key: &str, focus: &Path, anchor: Option<&str>) {
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
