//! WSL-side end of the Windows GUI bridge. Started by `wsl.exe --exec` with an
//! explicit private runtime and endpoint; speaks only the typed stdio protocol in
//! `acp_operator_client::bridge`. No shell, network listener, SQLite or Writer.
use acp_ipc::Location;
use clap::Parser;

#[derive(Parser)]
#[command(about = "ACP Windows GUI bridge (stdio protocol only)")]
struct Args {
    #[command(flatten)]
    location: Location,
}

fn main() {
    let args = Args::parse();
    // Absolute paths only; acp-ipc re-validates ownership/ACLs on every request.
    if !args.location.runtime_dir.is_absolute() || !args.location.endpoint.starts_with('/') {
        std::process::exit(2);
    }
    let code = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map(|runtime| runtime.block_on(acp_operator_client::bridge::serve(args.location)))
        .unwrap_or(2);
    std::process::exit(code);
}
