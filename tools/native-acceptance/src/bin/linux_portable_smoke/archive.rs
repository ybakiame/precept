use super::*;

/// Before the declared GUI packages are installed: the Desktop must fail with the
/// loader's explicit missing-library error, while CLI/daemon/MCP/bridge run.
pub(super) fn missing_deps(bins: &Path, out: &Path) {
    fs::create_dir(out).unwrap();
    let mut results = Vec::new();
    for name in [
        "preceptd",
        "precept",
        "precept-mcp",
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
pub(super) fn check_listing(archive: &Path) -> Result<usize, String> {
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

pub(super) fn tar(args: &[&str], dir: &Path) -> bool {
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
pub(super) fn archive(archive: &Path, inventory: &Path, work: &Path) {
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
