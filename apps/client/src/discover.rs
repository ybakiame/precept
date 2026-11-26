//! Connection discovery: a fixed, read-only check of one daemon host, and the
//! one explicit daemon start it may lead to.
//!
//! Inside Linux (the WSL helper's `--discover` / `--start-daemon` modes, or the
//! Linux GUI in-process) it reads the conventional paths of the packaged unit
//! (`$XDG_STATE_HOME/precept/runtime`, default `~/.local/state`), whether systemd and
//! the unit exist and whether the daemon answers a health read. A Windows GUI checks
//! `%LOCALAPPDATA%/precept/state/runtime` and can start the `preceptd.exe` beside itself
//! with fixed arguments. WSL runs through `wsl.exe --exec`: no shell, TCP or credential
//! in argv, environment or output.
use precept_ui_core::connection::Discovery;
#[cfg(target_os = "windows")]
use precept_ui_core::connection::wsl_status as status;

/// The packaged systemd user unit.
pub const UNIT: &str = "preceptd.service";
/// Where the Arch package installs the helper; the default for discovery.
pub const DEFAULT_HELPER: &str = "/usr/bin/precept-bridge";
pub use precept_ui_core::connection::discovery_status::{
    INVALID_REPORT, LOCAL_START_FAILED, NO_EXECUTABLE, NO_SYSTEMD, NO_UNIT, START_FAILED,
    START_TIMEOUT,
};

#[cfg(unix)]
mod host {
    use super::*;
    use precept_ui_core::connection::{DISCOVERY_VERSION, DaemonStatus};
    use std::path::{Path, PathBuf};

    fn unit_installed() -> bool {
        let mut candidates = vec![
            PathBuf::from("/etc/systemd/user").join(UNIT),
            PathBuf::from("/usr/lib/systemd/user").join(UNIT),
            PathBuf::from("/lib/systemd/user").join(UNIT),
        ];
        if let Some(c) = crate::paths::config_dir() {
            candidates.insert(0, c.join("systemd/user").join(UNIT));
        }
        candidates.iter().any(|p| p.is_file())
    }
    pub fn systemd() -> bool {
        Path::new("/run/systemd/system").is_dir()
    }

    /// The read-only report for the current user. `helper` is the answering
    /// executable (WSL) or empty (in-process).
    pub async fn report(helper: String) -> Discovery {
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_default();
        let state = crate::paths::state_dir().unwrap_or_default();
        let runtime = state.join("runtime");
        let endpoint = runtime.join("control.sock");
        let location = precept_ipc::Location {
            runtime_dir: runtime.clone(),
            endpoint: endpoint.to_string_lossy().into_owned(),
        };
        let daemon = if std::fs::symlink_metadata(&endpoint).is_err() {
            DaemonStatus::NotRunning
        } else {
            match crate::native::health_at(&location).await {
                Ok(v) => DaemonStatus::Ready {
                    profile_selected: v
                        .get("profile_selected")
                        .and_then(|p| p.as_bool())
                        .unwrap_or(false),
                },
                Err("Permission denied") => DaemonStatus::Refused,
                Err(_) => DaemonStatus::NotRunning,
            }
        };
        Discovery {
            version: DISCOVERY_VERSION,
            user,
            helper,
            runtime: runtime.to_string_lossy().into_owned(),
            endpoint: endpoint.to_string_lossy().into_owned(),
            systemd: systemd(),
            unit_installed: unit_installed(),
            daemon,
        }
    }

    /// Starts the packaged user unit with a fixed argument vector, then waits up
    /// to ten seconds for the daemon to answer. Never creates, selects or
    /// write-enables a Profile; never edits the unit or any configuration.
    pub async fn start_daemon(helper: String) -> Result<Discovery, &'static str> {
        let before = report(helper.clone()).await;
        if before.ready() {
            return Ok(before);
        }
        if !before.systemd {
            return Err(NO_SYSTEMD);
        }
        if !before.unit_installed {
            return Err(NO_UNIT);
        }
        let systemctl = ["/usr/bin/systemctl", "/bin/systemctl"]
            .into_iter()
            .find(|p| Path::new(p).is_file())
            .ok_or(NO_SYSTEMD)?;
        let status = tokio::process::Command::new(systemctl)
            .args(["--user", "start", UNIT])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .map_err(|_| START_FAILED)?;
        if !status.success() {
            return Err(START_FAILED);
        }
        for _ in 0..40 {
            let now = report(helper.clone()).await;
            if now.ready() {
                return Ok(now);
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        Err(START_TIMEOUT)
    }
}
#[cfg(unix)]
pub use host::{report, start_daemon};

#[cfg(target_os = "windows")]
mod windows_host {
    use super::*;
    use precept_ui_core::connection::{DISCOVERY_VERSION, DaemonStatus};
    use std::path::PathBuf;

    const ENDPOINT: &str = "precept";

    fn executable() -> Option<PathBuf> {
        let path = std::env::current_exe().ok()?.parent()?.join("preceptd.exe");
        path.is_file().then_some(path)
    }

    pub fn report() -> Discovery {
        let state = crate::paths::state_dir().unwrap_or_default();
        let runtime = state.join("runtime");
        let location = precept_ipc::Location {
            runtime_dir: runtime.clone(),
            endpoint: ENDPOINT.into(),
        };
        let daemon = match super::current_thread(crate::native::health_at(&location)) {
            Some(Ok(v)) => DaemonStatus::Ready {
                profile_selected: v
                    .get("profile_selected")
                    .and_then(|p| p.as_bool())
                    .unwrap_or(false),
            },
            Some(Err("Permission denied")) => DaemonStatus::Refused,
            _ => DaemonStatus::NotRunning,
        };
        Discovery {
            version: DISCOVERY_VERSION,
            user: std::env::var("USERNAME").unwrap_or_default(),
            helper: String::new(),
            runtime: runtime.to_string_lossy().into_owned(),
            endpoint: ENDPOINT.into(),
            systemd: false,
            unit_installed: executable().is_some(),
            daemon,
        }
    }

    pub fn start() -> Result<Discovery, &'static str> {
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};

        let before = report();
        if before.ready() || matches!(before.daemon, DaemonStatus::Refused) {
            return Ok(before);
        }
        let executable = executable().ok_or(NO_EXECUTABLE)?;
        let state = crate::paths::state_dir().ok_or(LOCAL_START_FAILED)?;
        if let Some(parent) = state.parent()
            && !parent.is_dir()
        {
            precept_ipc::transport::private_dir(parent, true).map_err(|_| LOCAL_START_FAILED)?;
        }
        precept_ipc::transport::private_dir(&state, true).map_err(|_| LOCAL_START_FAILED)?;
        let runtime = state.join("runtime");
        let mut child = Command::new(executable)
            .arg("--runtime-dir")
            .arg(&runtime)
            .args(["--endpoint", ENDPOINT, "--state-dir"])
            .arg(state.join("daemon-state"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000 | 0x00000200)
            .spawn()
            .map_err(|_| LOCAL_START_FAILED)?;
        for _ in 0..40 {
            if child.try_wait().map_err(|_| LOCAL_START_FAILED)?.is_some() {
                return Err(LOCAL_START_FAILED);
            }
            let now = report();
            if now.ready() {
                return Ok(now);
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        Err(START_TIMEOUT)
    }
}

/// The in-process report for a daemon on this machine. Blocking; call it off
/// the UI thread.
#[cfg(unix)]
pub fn local() -> Discovery {
    current_thread(report(String::new())).unwrap_or_default()
}
#[cfg(target_os = "windows")]
pub fn local() -> Discovery {
    windows_host::report()
}
/// Starts the local daemon (explicit user action). Blocking.
#[cfg(unix)]
pub fn start_local() -> Result<Discovery, &'static str> {
    current_thread(start_daemon(String::new())).unwrap_or(Err(START_FAILED))
}
#[cfg(target_os = "windows")]
pub fn start_local() -> Result<Discovery, &'static str> {
    windows_host::start()
}
#[cfg(any(unix, target_os = "windows"))]
fn current_thread<T>(f: impl std::future::Future<Output = T>) -> Option<T> {
    Some(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok()?
            .block_on(f),
    )
}

/// Parses the helper's one-line report, refusing anything that is not a valid
/// discovery of this version.
pub fn parse_report(raw: &[u8], wsl: bool) -> Result<Discovery, &'static str> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "snake_case")]
    enum Outcome {
        Ok(Discovery),
        Error(String),
    }
    let line = raw.split(|b| *b == b'\n').next().unwrap_or_default();
    match serde_json::from_slice::<Outcome>(line).map_err(|_| INVALID_REPORT)? {
        Outcome::Ok(d) if d.validate(wsl) => Ok(d),
        Outcome::Ok(_) => Err(INVALID_REPORT),
        // Only the fixed reasons above cross the boundary; anything else is a
        // generic failure, never a forwarded message.
        Outcome::Error(e) => Err(precept_ui_core::connection::discovery_status::KNOWN
            .into_iter()
            .find(|known| *known == e)
            .unwrap_or(START_FAILED)),
    }
}
/// The helper's one-line JSON output for a discovery mode.
pub fn render_outcome(outcome: Result<Discovery, &'static str>) -> String {
    match outcome {
        Ok(d) => serde_json::json!({ "ok": d }).to_string(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

/// Registered WSL distributions (`wsl.exe --list --quiet`), in WSL's order.
#[cfg(target_os = "windows")]
pub fn wsl_distributions() -> Result<Vec<String>, &'static str> {
    let out = run_wsl(&["--list", "--quiet"], std::time::Duration::from_secs(15))
        .map_err(|_| status::HOST_UNAVAILABLE)?;
    Ok(crate::native::distribution_names(&out.stdout))
}
#[cfg(not(target_os = "windows"))]
pub fn wsl_distributions() -> Result<Vec<String>, &'static str> {
    Err("Windows WSL owner transport unavailable on this OS")
}

/// Runs the helper's read-only `--discover` in `distribution` as its default
/// user. May start a stopped distribution (an explicit user action).
#[cfg(target_os = "windows")]
pub fn wsl_discover(distribution: &str, helper: &str) -> Result<Discovery, &'static str> {
    wsl_mode(distribution, None, helper, "--discover")
}
/// Starts the packaged unit in `distribution` as `user` (explicit user action).
#[cfg(target_os = "windows")]
pub fn wsl_start_daemon(
    distribution: &str,
    user: &str,
    helper: &str,
) -> Result<Discovery, &'static str> {
    wsl_mode(distribution, Some(user), helper, "--start-daemon")
}
#[cfg(not(target_os = "windows"))]
pub fn wsl_discover(_: &str, _: &str) -> Result<Discovery, &'static str> {
    Err("Windows WSL owner transport unavailable on this OS")
}
#[cfg(not(target_os = "windows"))]
pub fn wsl_start_daemon(_: &str, _: &str, _: &str) -> Result<Discovery, &'static str> {
    Err("Windows WSL owner transport unavailable on this OS")
}

#[cfg(target_os = "windows")]
const RELEASE_API: &str = "https://api.github.com/repos/ybakiame/precept/releases/latest";

#[cfg(target_os = "windows")]
fn install_command(
    distribution: &str,
    user: &str,
    program: &str,
    args: &[&str],
    seconds: u64,
) -> Result<Vec<u8>, &'static str> {
    let mut command = vec![
        "--distribution",
        distribution,
        "--user",
        user,
        "--exec",
        program,
    ];
    command.extend(args.iter().copied());
    let out = run_wsl(&command, std::time::Duration::from_secs(seconds))?;
    if out.code == Some(0) {
        Ok(out.stdout)
    } else {
        Err(status::INSTALL_FAILED)
    }
}

#[cfg(target_os = "windows")]
fn release_download(raw: &[u8]) -> Result<(String, String), &'static str> {
    let release: serde_json::Value =
        serde_json::from_slice(raw).map_err(|_| status::INSTALL_FAILED)?;
    let assets = release["assets"].as_array().ok_or(status::INSTALL_FAILED)?;
    let find = |name: &str| {
        assets.iter().find_map(|asset| {
            let url = asset["browser_download_url"].as_str()?;
            (asset["name"].as_str()?.ends_with(name) && url.starts_with("https://github.com/"))
                .then(|| url.to_owned())
        })
    };
    let archive = find("-linux-x86_64-musl.tar.gz").ok_or(status::INSTALL_FAILED)?;
    let checksums = find("SHA256SUMS").ok_or(status::INSTALL_FAILED)?;
    Ok((archive, checksums))
}

pub fn user_helper(user: &str) -> String {
    format!("/home/{user}/.local/bin/precept-bridge")
}

#[cfg(target_os = "windows")]
pub fn wsl_install_update(distribution: &str, user: &str) -> Result<Discovery, &'static str> {
    if !valid_inputs(distribution, Some(user), DEFAULT_HELPER) {
        return Err("Invalid WSL connection inputs");
    }
    let latest = install_command(distribution, user, "curl", &["-fsSL", RELEASE_API], 45)?;
    let (archive_url, sums_url) = release_download(&latest)?;
    let state = format!("/home/{user}/.local/state/precept");
    let downloads = format!("{state}/downloads");
    let archive = format!("{downloads}/precept-linux.tar.gz");
    let sums = format!("{downloads}/SHA256SUMS");
    let library = format!("/home/{user}/.local/lib/precept");
    let bin = format!("/home/{user}/.local/bin");
    let unit = format!("/home/{user}/.config/systemd/user/preceptd.service");
    install_command(
        distribution,
        user,
        "install",
        &["-d", "-m", "700", &downloads, &library, &bin],
        15,
    )?;
    install_command(
        distribution,
        user,
        "curl",
        &["-fsSL", &archive_url, "-o", &archive],
        180,
    )?;
    install_command(
        distribution,
        user,
        "curl",
        &["-fsSL", &sums_url, "-o", &sums],
        45,
    )?;
    let observed = install_command(distribution, user, "sha256sum", &[&archive], 15)?;
    let observed = String::from_utf8_lossy(&observed);
    let observed = observed.split_whitespace().next().unwrap_or_default();
    let expected = install_command(distribution, user, "cat", &[&sums], 15)?;
    let expected = String::from_utf8_lossy(&expected);
    let matches = expected.lines().any(|line| {
        let mut fields = line.split_whitespace();
        fields.next() == Some(observed)
            && fields
                .next_back()
                .is_some_and(|name| name.ends_with("-linux-x86_64-musl.tar.gz"))
    });
    if !matches {
        return Err(status::INSTALL_FAILED);
    }
    install_command(
        distribution,
        user,
        "tar",
        &["-xzf", &archive, "-C", &library],
        60,
    )?;
    install_command(
        distribution,
        user,
        "cp",
        &["-a", &format!("{library}/Precept/bin/."), &bin],
        30,
    )?;
    install_command(
        distribution,
        user,
        "install",
        &[
            "-d",
            "-m",
            "700",
            &format!("/home/{user}/.config/systemd/user"),
        ],
        15,
    )?;
    install_command(
        distribution,
        user,
        "sed",
        &[
            "-i",
            &format!("s|/usr/bin/precept|/home/{user}/.local/bin/precept|"),
            &format!("{library}/Precept/lib/systemd/user/preceptd.service"),
        ],
        15,
    )?;
    install_command(
        distribution,
        user,
        "cp",
        &[
            &format!("{library}/Precept/lib/systemd/user/preceptd.service"),
            &unit,
        ],
        15,
    )?;
    install_command(distribution, user, "chmod", &["600", &unit], 15)?;
    install_command(
        distribution,
        user,
        "systemctl",
        &["--user", "daemon-reload"],
        30,
    )?;
    install_command(
        distribution,
        user,
        "systemctl",
        &["--user", "enable", "--now", "preceptd.service"],
        45,
    )?;
    wsl_start_daemon(distribution, user, &user_helper(user))
}

#[cfg(not(target_os = "windows"))]
pub fn wsl_install_update(_: &str, _: &str) -> Result<Discovery, &'static str> {
    Err("Windows WSL owner transport unavailable on this OS")
}

/// Distribution, user and helper pass the same validation as an applied WSL
/// connection before any of them reaches `wsl.exe`.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn valid_inputs(distribution: &str, user: Option<&str>, helper: &str) -> bool {
    use precept_ui_core::connection::{Connection, Target};
    Connection {
        target: Target::Wsl,
        distribution: distribution.into(),
        user: user.unwrap_or("probe").into(),
        helper: helper.into(),
        wsl_runtime: "/probe".into(),
        wsl_endpoint: "/probe".into(),
        ..Default::default()
    }
    .validate()
}

#[cfg(target_os = "windows")]
fn wsl_mode(
    distribution: &str,
    user: Option<&str>,
    helper: &str,
    mode: &str,
) -> Result<Discovery, &'static str> {
    if !valid_inputs(distribution, user, helper) {
        return Err("Invalid WSL connection inputs");
    }
    let mut args = vec!["--distribution", distribution];
    if let Some(user) = user {
        args.extend(["--user", user]);
    }
    args.extend(["--exec", helper, mode]);
    let out = run_wsl(&args, std::time::Duration::from_secs(45))?;
    if out.code == Some(0) {
        return parse_report(&out.stdout, true);
    }
    let registered = (out.code == Some(-1))
        .then(|| crate::native::registered_distribution(distribution))
        .flatten();
    Err(status::launch_failure(out.code, registered))
}

#[cfg(target_os = "windows")]
struct Output {
    code: Option<i32>,
    stdout: Vec<u8>,
}
/// Runs `wsl.exe` with a fixed argument vector, no console window, stderr
/// discarded, stdout capped at 64 KiB, killed after `timeout`.
#[cfg(target_os = "windows")]
fn run_wsl(args: &[&str], timeout: std::time::Duration) -> Result<Output, &'static str> {
    use std::io::Read;
    use std::os::windows::process::CommandExt;
    let executable = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .ok_or(status::HOST_UNAVAILABLE)?
        .join("System32/wsl.exe");
    let mut child = std::process::Command::new(executable)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|_| status::HOST_UNAVAILABLE)?;
    let mut stdout = child.stdout.take().ok_or(status::LAUNCH_FAILED)?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = (&mut stdout).take(64 * 1024).read_to_end(&mut buf);
        buf
    });
    let deadline = std::time::Instant::now() + timeout;
    let code = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s.code(),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(status::BRIDGE_TIMEOUT);
            }
        }
    };
    Ok(Output {
        code,
        stdout: reader.join().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use precept_ui_core::connection::{DISCOVERY_VERSION, DaemonStatus};
    fn sample() -> Discovery {
        Discovery {
            version: DISCOVERY_VERSION,
            user: "operator".into(),
            helper: DEFAULT_HELPER.into(),
            runtime: "/home/operator/.local/state/precept/runtime".into(),
            endpoint: "/home/operator/.local/state/precept/runtime/control.sock".into(),
            systemd: true,
            unit_installed: true,
            daemon: DaemonStatus::NotRunning,
        }
    }
    #[test]
    fn outcomes_round_trip_and_only_fixed_reasons_cross_the_boundary() {
        let ok = render_outcome(Ok(sample()));
        assert_eq!(parse_report(ok.as_bytes(), true), Ok(sample()));
        for reason in [NO_SYSTEMD, NO_UNIT, START_FAILED, START_TIMEOUT] {
            let raw = render_outcome(Err(reason));
            assert_eq!(parse_report(raw.as_bytes(), true), Err(reason));
        }
        let forwarded = br#"{"error":"/home/operator/secret path leaked"}"#;
        assert_eq!(parse_report(forwarded, true), Err(START_FAILED));
        let mut bad = sample();
        bad.user = "-x".into();
        let raw = render_outcome(Ok(bad));
        assert_eq!(parse_report(raw.as_bytes(), true), Err(INVALID_REPORT));
        for junk in [
            &b""[..],
            b"not json",
            br#"{"ok":{}}"#,
            br#"{"ok":null,"error":"x"}"#,
        ] {
            assert!(parse_report(junk, true).is_err());
        }
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn release_download_selects_musl_and_checksum_assets_only_from_github() {
        let mut release = serde_json::json!({"assets":[
            {"name":"precept-0.4.5-linux-x86_64-musl.tar.gz","browser_download_url":"https://github.com/ybakiame/precept/releases/download/v0.4.5/precept-0.4.5-linux-x86_64-musl.tar.gz"},
            {"name":"SHA256SUMS","browser_download_url":"https://github.com/ybakiame/precept/releases/download/v0.4.5/SHA256SUMS"}
        ]});
        let (archive, sums) = release_download(&serde_json::to_vec(&release).unwrap()).unwrap();
        assert!(archive.ends_with("-linux-x86_64-musl.tar.gz"));
        assert!(sums.ends_with("SHA256SUMS"));
        release["assets"][0]["browser_download_url"] =
            serde_json::json!("https://example.invalid/asset");
        assert!(release_download(&serde_json::to_vec(&release).unwrap()).is_err());
        assert_eq!(
            user_helper("operator"),
            "/home/operator/.local/bin/precept-bridge"
        );
    }

    #[test]
    fn wsl_inputs_are_validated_before_reaching_wsl_exe() {
        assert!(valid_inputs("archlinux", None, DEFAULT_HELPER));
        assert!(valid_inputs(
            "Ubuntu-24.04",
            Some("operator"),
            DEFAULT_HELPER
        ));
        for (d, u, h) in [
            ("--exec", None, DEFAULT_HELPER),
            ("archlinux", Some("-root"), DEFAULT_HELPER),
            ("archlinux", None, "precept-bridge"),
            ("archlinux", None, "/usr/bin/../bin/sh"),
            ("arch\nlinux", None, DEFAULT_HELPER),
            ("", None, DEFAULT_HELPER),
        ] {
            assert!(!valid_inputs(d, u, h), "{d} {u:?} {h}");
        }
    }
    #[cfg(unix)]
    #[test]
    fn report_uses_the_unit_state_paths_and_reports_no_daemon_without_a_socket() {
        let dir = tempfile_dir();
        // SAFETY: test-local environment; this test is the only reader.
        unsafe {
            std::env::set_var("XDG_STATE_HOME", &dir);
            std::env::set_var("USER", "operator");
        }
        let d = local();
        assert_eq!(d.runtime, format!("{}/precept/runtime", dir.display()));
        assert_eq!(
            d.endpoint,
            format!("{}/precept/runtime/control.sock", dir.display())
        );
        assert_eq!(d.daemon, DaemonStatus::NotRunning);
        assert_eq!(d.user, "operator");
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[cfg(unix)]
    fn tempfile_dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("precept-discover-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
