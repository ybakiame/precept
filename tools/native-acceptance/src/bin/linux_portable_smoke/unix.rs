//! Unix body of the Linux packaged smoke (see the bin root).
use super::{desktop, tty};
use precept_ipc::{Access, Client, Error, Location, load_credential, transport};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
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
fn command(binary: &Path) -> Command {
    let mut cmd = Command::new(binary);
    // No inherited PRECEPT_* control (any case: Windows names are case-insensitive).
    precept_native_acceptance::environment::clear_inherited_precept(&mut cmd);
    cmd
}
fn bounded(mut cmd: Command) -> (bool, Value) {
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
struct Fixture {
    // Preserve on failure and success for private failure/retention evidence.
    root: PathBuf,
    bins: PathBuf,
    location: Location,
    daemon: Owned,
}
impl Fixture {
    fn start(bins: PathBuf) -> Self {
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
    fn daemon(bins: &Path, root: &Path, location: &Location) -> Owned {
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
    fn query(&self, args: &[&str]) -> (bool, Value) {
        let mut cmd = command(&self.bins.join("precept"));
        cmd.arg("--runtime-dir")
            .arg(&self.location.runtime_dir)
            .args(["--endpoint", &self.location.endpoint])
            .args(args);
        bounded(cmd)
    }
    fn ok(&self, args: &[&str]) -> Value {
        let (ok, v) = self.query(args);
        assert!(
            ok,
            "synthetic operator request refused: {} {}",
            args[0], v["error"]
        );
        v
    }
    fn mutation(&self, args: &[&str]) -> Value {
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
    fn stop(&mut self) {
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
struct Mcp {
    child: Owned,
    replies: mpsc::Receiver<Value>,
}
impl Mcp {
    fn start(f: &Fixture) -> Self {
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
    fn request(&mut self, method: &str, mut params: Value) -> Value {
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
struct Package {
    precept: PathBuf,
    bins: PathBuf,
    manifest: Value,
}
impl Package {
    /// Shipped executables, as `bin/<name>`.
    fn shipped(&self) -> Vec<String> {
        self.manifest["binaries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["file"].as_str().unwrap().to_owned())
            .collect()
    }
    fn hashes(&self) -> Vec<Value> {
        self.shipped()
            .into_iter()
            .map(|file| {
                let bytes = fs::read(self.precept.join(&file)).unwrap();
                json!({"file":file,"sha256":format!("{:x}", Sha256::digest(bytes))})
            })
            .collect()
    }
    /// Shared objects mapped by one owned process (`/proc/<pid>/maps`): every one
    /// must come from the user distribution, none from the archive or elsewhere.
    /// Includes dynamic GTK/GIO/GStreamer modules and GPU/display integration that
    /// DT_NEEDED alone does not show.
    fn modules(&self, pid: u32, out: &Path, surface: &str) -> Vec<String> {
        let maps = fs::read_to_string(format!("/proc/{pid}/maps")).unwrap();
        let mut libs: Vec<String> = maps
            .lines()
            .filter_map(|l| l.split_whitespace().nth(5))
            .filter(|p| p.starts_with('/') && p.contains(".so"))
            .map(str::to_owned)
            .collect();
        libs.sort();
        libs.dedup();
        // Anything under the extracted Precept/ tree is refused, not only bin/.
        let precept = self.precept.to_str().unwrap();
        let mut distribution = Vec::new();
        let mut wsl_provided = Vec::new();
        for lib in &libs {
            assert!(
                !lib.starts_with(precept),
                "{surface}: library loaded from the archive: {lib}"
            );
            // WSL mounts its GPU/DXCore integration here; it is not a distro package.
            if lib.starts_with("/usr/lib/wsl/") {
                wsl_provided.push(lib.clone());
                continue;
            }
            assert!(
                [
                    "/usr/lib/",
                    "/lib/",
                    "/usr/lib64/",
                    "/lib64/",
                    "/usr/share/",
                    "/usr/libexec/"
                ]
                .iter()
                .any(|p| lib.starts_with(p)),
                "{surface}: library outside the user distribution: {lib}"
            );
            distribution.push(lib.clone());
        }
        fs::write(
            out.join(format!("{surface}-libraries.json")),
            serde_json::to_vec_pretty(
                &json!({"user_distribution":distribution,"wsl_provided":wsl_provided}),
            )
            .unwrap(),
        )
        .unwrap();
        libs
    }
}
#[tokio::main]
pub async fn main() {
    if std::env::consts::OS != "linux" {
        panic!("Linux packaged acceptance only");
    }
    let mode: Vec<_> = std::env::args().collect();
    if mode.get(1).is_some_and(|m| m == "missing-deps") {
        return missing_deps(Path::new(&mode[2]), Path::new(&mode[3]));
    }
    if mode.get(1).is_some_and(|m| m == "archive") {
        return archive(
            Path::new(&mode[2]),
            Path::new(&mode[3]),
            Path::new(&mode[4]),
        );
    }
    if mode.get(1).is_some_and(|m| m == "remove") {
        return remove(Path::new(&mode[2]), Path::new(&mode[3]));
    }
    #[cfg(windows)]
    unsafe {
        // Supply a console to this harness only when launched by WSL without one.
        use windows_sys::Win32::System::Console::{AllocConsole, GetConsoleProcessList};
        let mut ids = [0_u32; 1];
        if GetConsoleProcessList(ids.as_mut_ptr(), 1) == 0 {
            assert_ne!(AllocConsole(), 0);
        }
    }
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4, "EXTRACTED_PACKAGE NEW_REPORT WINDOW_WORKER");
    let precept = PathBuf::from(&args[1])
        .canonicalize()
        .unwrap()
        .join("Precept");
    let out = PathBuf::from(&args[2]);
    // Private (0700) from creation: evidence names private fixture paths.
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(&out).unwrap();
    }
    let manifest: Value =
        serde_json::from_slice(&fs::read(precept.join("runtime/manifest.json")).unwrap()).unwrap();
    let package = Package {
        bins: precept.join("bin"),
        precept,
        manifest,
    };
    let manifest = &package.manifest;
    let bins = package.bins.clone();
    // Every archived file matches the in-archive manifest before any launch.
    for m in manifest["files"].as_array().unwrap() {
        let bytes = fs::read(package.precept.join(m["file"].as_str().unwrap())).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            m["sha256"].as_str().unwrap()
        );
    }
    let before = package.hashes();
    assert_eq!(before.len(), 6, "exactly six product executables");
    assert_eq!(
        fs::read_dir(&bins).unwrap().count(),
        6,
        "bin holds only the product executables"
    );
    // Product files: owned by this user, never group/other writable, user-executable.
    // Exact modes depend on the extracting umask (0755 by default, 0700 under 077).
    {
        use std::os::unix::fs::MetadataExt;
        let uid = unsafe { libc::getuid() };
        for path in [package.precept.clone(), bins.clone()] {
            let m = fs::symlink_metadata(&path).unwrap();
            assert!(m.is_dir() && m.uid() == uid && m.mode() & 0o022 == 0);
        }
        for b in &before {
            let m =
                fs::symlink_metadata(package.precept.join(b["file"].as_str().unwrap())).unwrap();
            assert!(
                m.is_file()
                    && m.uid() == uid
                    && m.mode() & 0o022 == 0
                    && m.mode() & 0o100 != 0
                    && m.nlink() == 1
            );
        }
    }
    let mut f = Fixture::start(bins.clone());
    // Keep the retained root private and out of public evidence. Only this file names it.
    transport::private_dir(&out, true).unwrap();
    package.modules(f.daemon.0.id(), &out, "daemon");
    fs::write(
        out.join("private-fixture-root.txt"),
        f.root.to_str().unwrap(),
    )
    .unwrap();
    let mut cells = Vec::new();
    let mut record = |id: &str| {
        cells.push(json!({"id":id,"status":"PASS"}));
        fs::write(
            out.join("progress.json"),
            serde_json::to_vec_pretty(
                &json!({"source_sha":manifest["source_sha"],"finished":false,"cells":cells}),
            )
            .unwrap(),
        )
        .unwrap();
    };
    let initial = f.ok(&["profile", "info"]);
    assert_eq!(initial["selected"], false);
    assert_eq!(initial["write_enabled"], false);
    record("no-implicit-profile");
    let unsafe_root = f.root.join("unsafe-runtime");
    fs::create_dir(&unsafe_root).unwrap();
    {
        // Broad permissions: group/other may read the would-be runtime directory.
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&unsafe_root, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut unsafe_daemon = command(&bins.join("preceptd"));
    unsafe_daemon
        .arg("--runtime-dir")
        .arg(&unsafe_root)
        .args(["--endpoint"])
        .arg(unsafe_root.join("control.sock"));
    assert!(!bounded(unsafe_daemon).0);
    assert!(!unsafe_root.join("operator.cap").exists());
    let file_root = f.root.join("file-runtime");
    fs::write(&file_root, "RETAIN").unwrap();
    let mut file_daemon = command(&bins.join("preceptd"));
    file_daemon
        .arg("--runtime-dir")
        .arg(&file_root)
        .args(["--endpoint"])
        .arg(f.root.join("file-runtime.sock"));
    assert!(!bounded(file_daemon).0);
    assert_eq!(fs::read(&file_root).unwrap(), b"RETAIN");
    f.ok(&["health"]);
    record("unsafe-permissions-and-file-runtime-refused-without-owner-disruption");
    let mut collision = Fixture::daemon(&bins, &f.root, &f.location);
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(s) = collision.0.try_wait().unwrap() {
            assert!(!s.success());
            break;
        }
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(30));
    }
    f.ok(&["health"]);
    record("endpoint-collision-preserves-owner");
    let db = f.root.join("state/profile.sqlite3");
    f.ok(&["profile", "create", db.to_str().unwrap()]);
    assert!(!f.query(&["profile", "create", db.to_str().unwrap()]).0);
    let binding = f.ok(&["profile", "info"])["binding"].clone();
    f.ok(&[
        "profile",
        "write",
        "enable",
        "--profile-id",
        binding["profile_id"].as_str().unwrap(),
        "--generation",
        binding["generation"].as_str().unwrap(),
        "--confirm",
    ]);
    f.mutation(&[
        "project-register",
        "synthetic",
        f.root
            .join("synthetic")
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap(),
    ]);
    record("explicit-synthetic-profile-grant-register-no-overwrite");
    let agent = load_credential(
        &f.location,
        &f.location.credential_path(Some("synthetic")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        agent.access,
        Access::Agent {
            project: "synthetic".into()
        }
    );
    let mut client = Client::connect(&f.location, &agent).await.unwrap();
    assert_eq!(
        client
            .request("profile.write.disable", json!({}))
            .await
            .unwrap_err(),
        Error::Unauthorized
    );
    record("project-credential-cannot-write");
    let mut wrong = command(&bins.join("precept-mcp"));
    wrong
        .arg("--runtime-dir")
        .arg(&f.location.runtime_dir)
        .args([
            "--endpoint",
            &f.location.endpoint,
            "--project",
            "synthetic",
            "--capability-file",
        ])
        .arg(f.location.runtime_dir.join("operator.cap"));
    assert!(!bounded(wrong).0);
    record("mcp-refuses-operator-credential");
    let mut mcp = Mcp::start(&f);
    let list = mcp.request("tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "get_effective_context");
    assert_eq!(tools[1]["name"], "get_handoff");
    let context = mcp.request(
        "tools/call",
        json!({"name":"get_effective_context","arguments":{}}),
    );
    assert_eq!(context["result"]["isError"], false);
    let handoff = mcp.request(
        "tools/call",
        json!({"name":"get_handoff","arguments":{"task_id":"synthetic"}}),
    );
    assert_eq!(handoff["result"]["isError"], true); // Known missing; no fabricated history.
    let denied = mcp.request(
        "tools/call",
        json!({"name":"confirm_preference","arguments":{}}),
    );
    assert!(denied["error"].is_object());
    record("packaged-mcp-exact-two-read-tools-context-missing-handoff-write-refusal");
    drop(mcp);
    drop(client);
    f.ok(&["profile", "write", "disable"]);
    assert_eq!(f.ok(&["profile", "info"])["write_enabled"], false);
    record("write-grant-explicit-revocation");
    let focus = f.root.join("state/focus.events");
    fs::write(&focus, "").unwrap();
    let ui_args = vec![
        "--runtime-dir".into(),
        f.location.runtime_dir.to_str().unwrap().into(),
        "--endpoint".into(),
        f.location.endpoint.clone(),
        "--operator".into(),
    ];
    {
        let mut ui = tty::Tty::start(&bins.join("precept-tui"), &ui_args, &out, &focus);
        ui.wait("Precept");
        let tui_libs = package.modules(ui.pid(), &out, "pty");
        // The TUI needs no GUI stack: no GTK, GStreamer or Pango is mapped.
        assert!(
            tui_libs
                .iter()
                .all(|l| !["gtk", "gst", "pango", "cairo", "wayland"]
                    .iter()
                    .any(|g| l.contains(g))),
            "TUI loaded GUI libraries"
        );
        ui.capture(&out.join("pty.txt"));
        ui.resize(100, 35);
        ui.wait("Precept");
        ui.close();
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            let raw = ui.raw.lock().unwrap();
            if raw.windows(8).any(|w| w == b"\x1b[?1049l")
                && raw.windows(8).any(|w| w == b"\x1b[?2004l")
            {
                break;
            }
            drop(raw);
            assert!(Instant::now() < end, "terminal restore sequences missing");
            thread::sleep(Duration::from_millis(20));
        }
    }
    record("packaged-pty-render-resize-normal-restore");
    {
        let nonce = &precept_ipc::operator::operation_id().unwrap()[..16];
        let mut ui = desktop::Desktop::start(
            &bins.join("precept-desktop"),
            &ui_args,
            nonce,
            &args[3],
            &out,
            &focus,
        );
        ui.wait_after("PRECEPT_NSH_STATE phase=Ready", 0);
        // A real Wayland connection through WSLg's Weston, not XWayland.
        ui.wait_after("PRECEPT_GTK backend=GdkWaylandDisplay", 0);
        let gtk = ui
            .lines
            .lock()
            .unwrap()
            .iter()
            .find(|l| l.starts_with("PRECEPT_GTK backend="))
            .cloned()
            .unwrap();
        let libs = package.modules(ui.pid(), &out, "wayland");
        assert!(
            libs.iter().any(|l| l.contains("libwayland-client")),
            "Wayland client mapped"
        );
        // Ubuntu's GTK 4 links both backends, so libX11 may be mapped as linkage.
        // No X11 connection is possible: the owned GUI has no DISPLAY and is pinned
        // to the Wayland backend, which GTK itself reports above.
        let environ = fs::read(format!("/proc/{}/environ", ui.pid())).unwrap();
        let vars: Vec<&[u8]> = environ.split(|b| *b == 0).collect();
        assert!(
            !vars.iter().any(|v| v.starts_with(b"DISPLAY=")),
            "GUI has no DISPLAY"
        );
        assert!(
            vars.contains(&&b"GDK_BACKEND=wayland"[..]),
            "GUI pinned to Wayland"
        );
        let x11_linked: Vec<_> = libs
            .iter()
            .filter(|l| l.contains("libX"))
            .cloned()
            .collect();
        let display = json!({
            "WAYLAND_DISPLAY": std::env::var("WAYLAND_DISPLAY").ok(),
            "XDG_RUNTIME_DIR": std::env::var("XDG_RUNTIME_DIR").ok(),
            "WSL_DISTRO_NAME": std::env::var("WSL_DISTRO_NAME").ok(),
            "gtk": gtk,
            "gui_display_env": false,
            "gdk_backend": "wayland",
            "x11_libraries_mapped_as_gtk_linkage": x11_linked,
            // From what the process actually mapped: WSL's d3d12/DXCore libraries
            // mean Mesa's d3d12 (GPU) driver; otherwise Mesa fell back to software.
            "gl_driver": if libs.iter().any(|l| l.starts_with("/usr/lib/wsl/lib/libd3d12")) {
                "mesa-d3d12 (WSL GPU)"
            } else {
                "mesa-software (no WSL GPU library mapped)"
            },
            "GALLIUM_DRIVER": std::env::var("GALLIUM_DRIVER").ok(),
        });
        fs::write(
            out.join("wayland.json"),
            serde_json::to_vec_pretty(&display).unwrap(),
        )
        .unwrap();
        ui.resize(920, 640);
        ui.key("alt+3");
        ui.wait("section=Preferences");
        // Sidebar "Sessions" (Default button) at rest, then under the pointer.
        // The hover wash must stay a slight tint of the dark rest fill so its
        // light label stays readable (regression: an inverted mix made it ~text).
        let rest = ui.capture_sample("wayland", &out, 150, 280);
        ui.hover(150, 280);
        let hover = ui.capture_sample("wayland-hover", &out, 150, 280);
        ui.unhover();
        let luma = |c: [u8; 3]| (299 * c[0] as u32 + 587 * c[1] as u32 + 114 * c[2] as u32) / 1000;
        fs::write(
            out.join("hover.json"),
            serde_json::to_vec_pretty(&json!({"rest":rest,"hover":hover})).unwrap(),
        )
        .unwrap();
        assert!(
            luma(hover) < 110 && luma(hover).abs_diff(luma(rest)) <= 40,
            "hover wash must stay near the rest fill: rest={rest:?} hover={hover:?}"
        );
        ui.close();
    }
    record("packaged-wayland-render-resize-navigate-hover-normal-close");
    record("libraries-from-user-distribution-only");
    f.ok(&["health"]);
    record("ui-close-leaves-daemon-running");
    let state_before = Sha256::digest(fs::read(&db).unwrap());
    let operator =
        load_credential(&f.location, &f.location.credential_path(None).unwrap()).unwrap();
    let pid = f.daemon.0.id();
    f.stop();
    record("graceful-stop-cleans-owned-credentials-releases-runtime-lease");
    f.daemon = Fixture::daemon(&bins, &f.root, &f.location);
    let end = Instant::now() + Duration::from_secs(10);
    while !f.query(&["profile", "info"]).0 {
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(30));
    }
    assert_eq!(f.ok(&["profile", "info"])["selected"], false);
    assert!(Client::connect(&f.location, &operator).await.is_err());
    let pre_crash =
        load_credential(&f.location, &f.location.credential_path(None).unwrap()).unwrap();
    f.daemon.0.kill().unwrap();
    f.daemon.0.wait().unwrap();
    f.daemon = Fixture::daemon(&bins, &f.root, &f.location);
    let end = Instant::now() + Duration::from_secs(10);
    while !f.query(&["profile", "info"]).0 {
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(30));
    }
    assert_eq!(f.ok(&["profile", "info"])["selected"], false);
    assert!(Client::connect(&f.location, &pre_crash).await.is_err());
    f.stop();
    record("normal-and-crash-restart-no-adoption-stale-credential-refused");
    assert_eq!(state_before[..], Sha256::digest(fs::read(&db).unwrap())[..]);
    assert_eq!(
        fs::read(f.root.join("state/auth-retain.txt")).unwrap(),
        b"SYNTHETIC RETAIN\n"
    );
    assert_eq!(
        fs::read(f.root.join("state/unknown-retain.txt")).unwrap(),
        b"SYNTHETIC RETAIN\n"
    );
    assert_eq!(before, package.hashes());
    fs::write(out.join("smoke.json"), serde_json::to_vec_pretty(&json!({"source_sha":manifest["source_sha"],"source_tree":manifest["source_tree"],"platform":"Linux x86_64 WSL2 user distribution with WSLg (Weston, Wayland)","binaries":before,"initial_daemon_pid":pid,"cells":cells,"second_user":"NOT_RUN","ime_at":"NOT_RUN"})).unwrap()).unwrap();
    println!("PASS: owned packaged Linux smoke ({} cells)", cells.len());
}

/// Before the declared GUI packages are installed: the Desktop must fail with the
/// loader's explicit missing-library error, while CLI/daemon/MCP/TUI/bridge run.
fn missing_deps(bins: &Path, out: &Path) {
    fs::create_dir(out).unwrap();
    let mut results = Vec::new();
    for name in [
        "preceptd",
        "precept",
        "precept-mcp",
        "precept-tui",
        "precept-bridge",
        "precept-desktop",
    ] {
        let o = command(&bins.join(name))
            .arg("--help")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&o.stderr);
        let first = stderr.lines().next().unwrap_or_default();
        let missing = first.contains("error while loading shared libraries");
        if name == "precept-desktop" {
            assert!(
                missing && o.status.code() == Some(127),
                "Desktop must name its missing library"
            );
        } else {
            assert!(
                o.status.success() && !missing,
                "{name} must run without GUI packages"
            );
        }
        let library = missing
            .then(|| {
                first
                    .split("libraries: ")
                    .nth(1)
                    .and_then(|r| r.split(':').next())
                    .map(str::to_owned)
            })
            .flatten();
        results.push(json!({"binary":name,"exit":o.status.code(),"missing_library":library}));
    }
    fs::write(
        out.join("missing-deps.json"),
        serde_json::to_vec_pretty(&results).unwrap(),
    )
    .unwrap();
    println!("PASS: missing GUI dependencies are diagnosed; non-GUI binaries run");
}

/// Archive listing policy (mirrors `checkTarListing` in linux-package.mjs): only
/// `Precept/` paths of safe components, regular files and directories only (no links,
/// devices or traversal), executables 0755 under `Precept/bin`, other files 0644.
fn check_listing(archive: &Path) -> Result<usize, String> {
    let o = Command::new("tar")
        .arg("-tvzf")
        .arg(archive)
        .arg("--numeric-owner")
        .stderr(Stdio::null())
        .output()
        .map_err(|_| "tar unavailable")?;
    if !o.status.success() {
        return Err("unreadable archive".into());
    }
    let text = String::from_utf8(o.stdout).map_err(|_| "non-UTF-8 listing")?;
    let mut entries = 0;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let mode = line.split_whitespace().next().unwrap_or_default();
        if !(mode.starts_with('-') || mode.starts_with('d')) {
            return Err("link or device entry".into());
        }
        // mode owner/group size date time name.
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() != 6 {
            return Err("unexpected entry shape".into());
        }
        let (kind, file) = (&mode[..1], fields[5].trim_end_matches('/'));
        let component = |c: &str| {
            c.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
                && c.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        };
        if !(file == "Precept"
            || file
                .strip_prefix("Precept/")
                .is_some_and(|r| r.split('/').all(component)))
        {
            return Err("unsafe path".into());
        }
        let expected = if kind == "d" || file.starts_with("Precept/bin/") {
            "rwxr-xr-x"
        } else {
            "rw-r--r--"
        };
        if &mode[1..] != expected {
            return Err("unexpected mode".into());
        }
        entries += 1;
    }
    Ok(entries)
}

fn tar(args: &[&str], dir: &Path) -> bool {
    Command::new("tar")
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

/// Target-side archive cells: listing policy on the real and hostile archives,
/// fresh extraction, collision refusal, and exact removal (changed, occupied,
/// unknown/Profile/auth retention).
fn archive(archive: &Path, inventory: &Path, work: &Path) {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let private = |p: &Path| fs::DirBuilder::new().mode(0o700).create(p).unwrap();
    private(work);
    let files: Vec<Value> = serde_json::from_slice(&fs::read(inventory).unwrap()).unwrap();
    let mut cells = Vec::new();
    let entries = check_listing(archive).expect("real archive listing");
    cells.push(json!({"id":"listing-policy-real-archive","status":"PASS","entries":entries}));
    // Hostile archives, built here with GNU tar; each must be refused before extraction.
    let hostile = work.join("hostile");
    private(&hostile);
    fs::create_dir_all(hostile.join("src/Precept/bin")).unwrap();
    fs::write(hostile.join("src/Precept/bin/x"), "x").unwrap();
    fs::set_permissions(
        hostile.join("src/Precept/bin/x"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    std::os::unix::fs::symlink("/etc", hostile.join("src/Precept/link")).unwrap();
    let src = hostile.join("src");
    let cases: [(&str, Vec<&str>); 5] = [
        (
            "traversal",
            vec![
                "--transform=s,^Precept/bin/x$,Precept/../x,",
                "Precept/bin/x",
            ],
        ),
        (
            "absolute",
            vec![
                "-P",
                "--transform=s,^Precept/bin/x$,/tmp/precept-hostile-x,",
                "Precept/bin/x",
            ],
        ),
        ("symlink", vec!["Precept/link"]),
        ("setuid", vec!["--mode=u+s", "Precept/bin/x"]),
        (
            "outside-root",
            vec!["--transform=s,^Precept/bin/x$,other/x,", "Precept/bin/x"],
        ),
    ];
    for (name, extra) in cases {
        let out = hostile.join(format!("{name}.tar.gz"));
        let mut args = vec!["--no-recursion", "-czf", out.to_str().unwrap()];
        args.extend(extra);
        assert!(tar(&args, &src), "hostile archive {name} constructed");
        let refused = check_listing(&out).expect_err(name);
        cells
            .push(json!({"id":format!("hostile-{name}-refused"),"status":"PASS","reason":refused}));
    }
    // Fresh extraction into a new private directory, refusing any existing file.
    let a = work.join("extract-a");
    private(&a);
    assert!(tar(
        &["--keep-old-files", "-xzf", archive.to_str().unwrap()],
        &a
    ));
    for f in &files {
        let bytes = fs::read(a.join(f["file"].as_str().unwrap())).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            f["sha256"].as_str().unwrap()
        );
    }
    cells.push(json!({"id":"fresh-extraction-inventory","status":"PASS","files":files.len()}));
    let readme = a.join("Precept/README.txt");
    fs::write(&readme, "LOCAL EDIT RETAINED\n").unwrap();
    assert!(
        !tar(&["--keep-old-files", "-xzf", archive.to_str().unwrap()], &a),
        "collision must fail"
    );
    assert_eq!(fs::read(&readme).unwrap(), b"LOCAL EDIT RETAINED\n");
    cells.push(json!({"id":"extraction-collision-refused-local-file-retained","status":"PASS"}));
    // Removal: a changed product file refuses before any unlink.
    let present = |root: &Path| {
        files
            .iter()
            .all(|f| root.join(f["file"].as_str().unwrap()).exists())
    };
    assert_eq!(
        remove_owned(&a, &files).unwrap_err(),
        "changed file retained"
    );
    assert!(present(&a));
    cells.push(json!({"id":"removal-changed-file-refused-before-unlink","status":"PASS"}));
    // Removal: an occupied product (running daemon) refuses before any unlink.
    let b = work.join("extract-b");
    private(&b);
    assert!(tar(
        &["--keep-old-files", "-xzf", archive.to_str().unwrap()],
        &b
    ));
    fs::write(b.join("Precept/bin/unknown-retain"), "RETAIN").unwrap();
    let state = work.join("state");
    private(&state);
    fs::write(state.join("profile.sqlite3"), "SYNTHETIC RETAIN").unwrap();
    fs::write(state.join("auth-retain.txt"), "SYNTHETIC RETAIN").unwrap();
    let runtime = work.join("runtime");
    let mut daemon = Owned(
        command(&b.join("Precept/bin/preceptd"))
            .arg("--runtime-dir")
            .arg(&runtime)
            .arg("--endpoint")
            .arg(runtime.join("control.sock"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let end = Instant::now() + Duration::from_secs(10);
    while !runtime.join("control.sock").exists() {
        assert!(
            daemon.0.try_wait().unwrap().is_none() && Instant::now() < end,
            "occupying daemon"
        );
        thread::sleep(Duration::from_millis(30));
    }
    assert_eq!(
        remove_owned(&b, &files).unwrap_err(),
        "product process running"
    );
    assert!(present(&b));
    cells.push(json!({"id":"removal-occupied-product-refused-before-unlink","status":"PASS"}));
    unsafe {
        assert_eq!(libc::kill(daemon.0.id() as i32, libc::SIGINT), 0);
    }
    let end = Instant::now() + Duration::from_secs(10);
    while daemon.0.try_wait().unwrap().is_none() {
        assert!(Instant::now() < end, "daemon stop");
        thread::sleep(Duration::from_millis(30));
    }
    let (removed, retained) = remove_owned(&b, &files).unwrap();
    assert_eq!(removed, files.len());
    assert_eq!(retained, vec!["Precept/bin", "Precept"]);
    assert_eq!(fs::read_dir(b.join("Precept/bin")).unwrap().count(), 1);
    assert_eq!(
        fs::read(b.join("Precept/bin/unknown-retain")).unwrap(),
        b"RETAIN"
    );
    assert_eq!(
        fs::read(state.join("profile.sqlite3")).unwrap(),
        b"SYNTHETIC RETAIN"
    );
    assert_eq!(
        fs::read(state.join("auth-retain.txt")).unwrap(),
        b"SYNTHETIC RETAIN"
    );
    cells.push(json!({"id":"exact-removal-retains-unknown-profile-auth","status":"PASS","removed":removed,"retained_directories":retained}));
    fs::write(
        work.join("archive-cells.json"),
        serde_json::to_vec_pretty(&cells).unwrap(),
    )
    .unwrap();
    println!("PASS: archive cells ({})", cells.len());
}

/// Exact post-stop removal from the original inventory: preflight every listed
/// file (allowlisted name, unaliased regular file, hash) and refuse while any
/// process runs a product executable, before the first unlink; then remove only
/// empty known directories. Unknown files are never touched.
fn remove_owned(root: &Path, files: &[Value]) -> Result<(usize, Vec<&'static str>), &'static str> {
    let bins = [
        "preceptd",
        "precept",
        "precept-mcp",
        "precept-desktop",
        "precept-tui",
        "precept-bridge",
    ];
    let allowed = |f: &str| {
        bins.iter().any(|b| f == format!("Precept/bin/{b}"))
            || [
                "Precept/runtime/manifest.json",
                "Precept/notices/NOTICE.txt",
                "Precept/notices/dependency-notices.json",
                "Precept/README.txt",
                "Precept/LICENSE.txt",
            ]
            .contains(&f)
            || f.strip_prefix("Precept/notices/")
                .and_then(|n| n.strip_suffix(".txt"))
                .is_some_and(|h| {
                    h.len() == 64
                        && h.bytes()
                            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                })
    };
    if files.is_empty()
        || !bins.iter().all(|b| {
            files
                .iter()
                .any(|f| f["file"] == format!("Precept/bin/{b}"))
        })
    {
        return Err("incomplete removal inventory");
    }
    for f in files {
        let name = f["file"].as_str().ok_or("invalid inventory")?;
        if !allowed(name) {
            return Err("unsafe removal member");
        }
        let path = root.join(name);
        let mut dir = path.parent();
        while let Some(d) = dir {
            if fs::symlink_metadata(d)
                .map_err(|_| "missing directory")?
                .file_type()
                .is_symlink()
            {
                return Err("alias refused");
            }
            dir = d.parent();
        }
        if !fs::symlink_metadata(&path)
            .map_err(|_| "missing product file")?
            .is_file()
        {
            return Err("regular file required");
        }
        if format!(
            "{:x}",
            Sha256::digest(fs::read(&path).map_err(|_| "unreadable")?)
        ) != f["sha256"].as_str().unwrap_or_default()
        {
            return Err("changed file retained");
        }
    }
    // Linux permits unlinking a running executable; refuse instead while any
    // visible process runs one of these exact product files.
    let products: Vec<PathBuf> = bins
        .iter()
        .map(|b| root.join("Precept/bin").join(b).canonicalize().unwrap())
        .collect();
    for entry in fs::read_dir("/proc").unwrap().flatten() {
        if let Ok(exe) = fs::read_link(entry.path().join("exe"))
            && products.contains(&exe)
        {
            return Err("product process running");
        }
    }
    for f in files {
        fs::remove_file(root.join(f["file"].as_str().unwrap())).map_err(|_| "unlink failed")?;
    }
    let mut retained = Vec::new();
    for d in [
        "Precept/bin",
        "Precept/runtime",
        "Precept/notices",
        "Precept",
    ] {
        if fs::remove_dir(root.join(d)).is_err() && root.join(d).exists() {
            retained.push(d);
        }
    }
    Ok((files.len(), retained))
}

fn remove(root: &Path, inventory: &Path) {
    let files: Vec<Value> = serde_json::from_slice(&fs::read(inventory).unwrap()).unwrap();
    let (removed, retained) =
        remove_owned(root, &files).unwrap_or_else(|e| panic!("removal refused: {e}"));
    println!(
        "{}",
        json!({"result":"PASS","removed_files":removed,"retained_directories":retained})
    );
}
