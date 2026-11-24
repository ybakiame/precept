use super::*;

pub(super) async fn main() {
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
