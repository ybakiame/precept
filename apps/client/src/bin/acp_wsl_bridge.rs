//! WSL-side end of the Windows GUI bridge. Started by `wsl.exe --exec`.
//!
//! - `--runtime-dir R --endpoint E`: the long-lived stdio bridge; speaks only the
//!   typed protocol in `acp_operator_client::bridge`.
//! - `--discover`: one read-only discovery report (JSON line) for the current
//!   user: conventional unit paths, systemd/unit presence, daemon health.
//! - `--start-daemon`: `systemctl --user start acp-daemon.service`, then the
//!   report. An explicit Windows-side user action; nothing else is started.
//!
//! No shell, network listener, SQLite or Writer; no credential in any output.
use acp_ipc::Location;
use clap::Parser;

#[derive(Parser)]
#[command(about = "ACP Windows GUI bridge (stdio protocol only)")]
struct Args {
    #[command(flatten)]
    location: Location,
}

fn main() {
    let runtime = || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
    };
    // Exactly one fixed mode flag, nothing else, or the bridge arguments.
    let mode: Vec<_> = std::env::args_os().skip(1).collect();
    #[cfg(unix)]
    if let [flag] = mode.as_slice()
        && (flag == "--discover" || flag == "--start-daemon")
    {
        let helper = std::env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Ok(runtime) = runtime() else {
            std::process::exit(2);
        };
        use acp_operator_client::discover;
        let outcome = if flag == "--discover" {
            Ok(runtime.block_on(discover::report(helper)))
        } else {
            runtime.block_on(discover::start_daemon(helper))
        };
        println!("{}", discover::render_outcome(outcome));
        std::process::exit(0);
    }
    #[cfg(not(unix))]
    if mode
        .iter()
        .any(|a| a == "--discover" || a == "--start-daemon")
    {
        std::process::exit(2);
    }
    let args = Args::parse();
    // Absolute paths only; acp-ipc re-validates ownership/ACLs on every request.
    if !args.location.runtime_dir.is_absolute() || !args.location.endpoint.starts_with('/') {
        std::process::exit(2);
    }
    let code = runtime()
        .map(|runtime| runtime.block_on(acp_operator_client::bridge::serve(args.location)))
        .unwrap_or(2);
    std::process::exit(code);
}
