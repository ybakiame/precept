use super::*;

pub(super) struct Owned(pub(super) Child);
impl Drop for Owned {
    pub(super) fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub(super) fn command(binary: &Path) -> Command {
    let mut cmd = Command::new(binary);
    // No inherited PRECEPT_* control (any case: Windows names are case-insensitive).
    precept_native_acceptance::environment::clear_inherited_precept(&mut cmd);
    cmd
}
pub(super) fn bounded(mut cmd: Command) -> (bool, Value) {
    let mut child = Owned(
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() <= 1024 * 1024, "bounded synthetic CLI output");
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    let success = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status.success();
        }
        assert!(Instant::now() < deadline, "owned process deadline");
        thread::sleep(Duration::from_millis(25));
    };
    let bytes = reader.join().unwrap();
    (
        success,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
pub(super) struct Fixture {
    // Preserve on failure and success for private failure/retention evidence.
    pub(super) root: PathBuf,
    pub(super) bins: PathBuf,
    pub(super) location: Location,
    pub(super) daemon: Owned,
}
impl Fixture {
    pub(super) fn start(bins: PathBuf) -> Self {
        let parent = tempfile::Builder::new()
            .prefix("precept-m5-zip-")
            .tempdir()
            .unwrap()
            .keep();
        let root = parent.join("private");
        transport::private_dir(&root, true).unwrap();
        transport::private_dir(&root.join("state"), true).unwrap();
        fs::create_dir(root.join("synthetic")).unwrap();
        fs::write(
            root.join("synthetic/AGENTS.md"),
            "Synthetic archive smoke only.\n",
        )
        .unwrap();
        fs::write(root.join("state/auth-retain.txt"), "SYNTHETIC RETAIN\n").unwrap();
        fs::write(root.join("state/unknown-retain.txt"), "SYNTHETIC RETAIN\n").unwrap();
        let location = Location {
            runtime_dir: root.join("runtime"),
            endpoint: root.join("runtime/control.sock").to_str().unwrap().into(),
        };
        let daemon = Self::daemon(&bins, &root, &location);
        let f = Self {
            root,
            bins,
            location,
            daemon,
        };
        let end = Instant::now() + Duration::from_secs(15);
        while !f.query(&["profile", "info"]).0 {
            assert!(Instant::now() < end, "packaged daemon readiness");
            thread::sleep(Duration::from_millis(50));
        }
        f
    }
    pub(super) fn daemon(bins: &Path, root: &Path, location: &Location) -> Owned {
        let state = root.join("daemon-state");
        precept_native_acceptance::daemon_state::write(
            &state,
            &[("synthetic", &root.join("synthetic"))],
            None,
        );
        let mut cmd = command(&bins.join("preceptd"));
        cmd.arg("--runtime-dir")
            .arg(&location.runtime_dir)
            .args(["--endpoint", &location.endpoint, "--state-dir"])
            .arg(&state)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Ctrl+Break is sent only to this exact newly owned process group.
            cmd.creation_flags(0x200);
        }
        Owned(cmd.spawn().unwrap())
    }
    pub(super) fn query(&self, args: &[&str]) -> (bool, Value) {
        let mut cmd = command(&self.bins.join("precept"));
        cmd.arg("--runtime-dir")
            .arg(&self.location.runtime_dir)
            .args(["--endpoint", &self.location.endpoint])
            .args(args);
        bounded(cmd)
    }
    pub(super) fn ok(&self, args: &[&str]) -> Value {
        let (ok, v) = self.query(args);
        assert!(
            ok,
            "synthetic operator request refused: {} {}",
            args[0], v["error"]
        );
        v
    }
    pub(super) fn mutation(&self, args: &[&str]) -> Value {
        let binding = self.ok(&["profile", "info"])["binding"].clone();
        let epoch = binding["owner_epoch"].to_string();
        let id = precept_ipc::operator::operation_id().unwrap();
        let mut all = args.to_vec();
        all.extend([
            "--profile-id",
            binding["profile_id"].as_str().unwrap(),
            "--generation",
            binding["generation"].as_str().unwrap(),
            "--owner-epoch",
            &epoch,
            "--operation-id",
            &id,
        ]);
        self.ok(&all)
    }
    pub(super) fn stop(&mut self) {
        // SIGINT (Ctrl+C) to the exact owned daemon process only.
        #[cfg(unix)]
        unsafe {
            assert_eq!(libc::kill(self.daemon.0.id() as i32, libc::SIGINT), 0);
        }
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
            assert_ne!(
                GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, self.daemon.0.id()),
                0,
                "owned graceful stop signal"
            );
        }
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.daemon.0.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < end, "graceful daemon exit deadline");
            thread::sleep(Duration::from_millis(30));
        }
        assert!(!self.location.runtime_dir.join("operator.cap").exists());
        assert!(
            !self
                .location
                .runtime_dir
                .join("agent-synthetic.cap")
                .exists()
        );
        assert!(
            !self
                .location
                .runtime_dir
                .join("operator-write.cap")
                .exists()
        );
        // The stable coordination file is retained; acquiring it proves release.
        let _lease = precept_ipc::RuntimeLease::acquire(&self.location).unwrap();
    }
}
pub(super) struct Mcp {
    pub(super) child: Owned,
    pub(super) replies: mpsc::Receiver<Value>,
}
impl Mcp {
    pub(super) fn start(f: &Fixture) -> Self {
        let mut cmd = command(&f.bins.join("precept-mcp"));
        cmd.arg("--runtime-dir")
            .arg(&f.location.runtime_dir)
            .args([
                "--endpoint",
                &f.location.endpoint,
                "--project",
                "synthetic",
                "--capability-file",
            ])
            .arg(f.location.runtime_dir.join("agent-synthetic.cap"));
        let mut child = Owned(
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let stdout = child.0.stdout.take().unwrap();
        let (tx, replies) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout.take(1024 * 1024)).lines() {
                let Ok(line) = line else { break };
                if line.len() > 128 * 1024 {
                    break;
                }
                let Ok(value) = serde_json::from_str(&line) else {
                    break;
                };
                if tx.send(value).is_err() {
                    break;
                }
            }
        });
        Self { child, replies }
    }
    pub(super) fn request(&mut self, method: &str, mut params: Value) -> Value {
        params["_meta"] = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28"});
        writeln!(
            self.child.0.stdin.as_mut().unwrap(),
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})
        )
        .unwrap();
        self.child.0.stdin.as_mut().unwrap().flush().unwrap();
        self.replies
            .recv_timeout(Duration::from_secs(10))
            .expect("packaged MCP deadline")
    }
}
