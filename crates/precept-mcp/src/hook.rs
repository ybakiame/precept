//! `precept-hook`: the command an agent harness runs at session start (and
//! when it resumes after compacting its conversation). It reads the project's
//! effective context through the agent capability, as `precept-mcp` does, and
//! prints a short briefing in the harness's own output shape. Read-only and
//! fail-open: any problem prints nothing the harness would act on, exits 0
//! and leaves one sanitized line on stderr.
use clap::{Parser, ValueEnum};
use precept_core::briefing::{Moment, render};
use precept_ipc::{Access, Client, Location, load_credential};
use serde_json::{Value, json};
use std::io::Read;

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Vendor {
    /// Claude Code (and Codex, which reads the same hook shape).
    Claude,
    Codex,
    /// Cursor's `sessionStart` hook.
    Cursor,
    /// Antigravity's `SessionStart` hook (`~/.gemini/config/hooks.json`).
    Antigravity,
}
impl Vendor {
    fn label(self) -> &'static str {
        match self {
            Self::Claude => "claude-code hook",
            Self::Codex => "codex hook",
            Self::Cursor => "cursor hook",
            Self::Antigravity => "antigravity hook",
        }
    }
}

#[derive(Parser)]
struct Args {
    #[command(flatten)]
    location: Location,
    #[arg(long)]
    project: String,
    #[arg(long)]
    capability_file: std::path::PathBuf,
    #[arg(long, value_enum)]
    vendor: Vendor,
    /// The project's root. A user-level hook runs in every session; outside
    /// this root it prints nothing at all.
    #[arg(long)]
    root: Option<std::path::PathBuf>,
}

/// The session's directories from the hook input: `cwd` (Claude Code, Codex,
/// Cursor), else Antigravity's `workspacePaths` (a leading `~/` is the home),
/// else this process's working directory.
fn session_dirs(stdin: &[u8]) -> Vec<std::path::PathBuf> {
    let input = serde_json::from_slice::<Value>(stdin).ok();
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let path = |s: &str| match (s.strip_prefix("~/"), &home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => std::path::PathBuf::from(s),
    };
    if let Some(cwd) = input
        .as_ref()
        .and_then(|v| v.get("cwd"))
        .and_then(Value::as_str)
    {
        return vec![path(cwd)];
    }
    let workspaces: Vec<std::path::PathBuf> = input
        .as_ref()
        .and_then(|v| v.get("workspacePaths"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(64)
        .map(path)
        .collect();
    if !workspaces.is_empty() {
        return workspaces;
    }
    std::env::current_dir().into_iter().collect()
}

/// Whether any of the session's directories is inside `root`. Unresolvable
/// paths are outside.
fn inside(root: &std::path::Path, stdin: &[u8]) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    session_dirs(stdin)
        .iter()
        .filter_map(|d| d.canonicalize().ok())
        .any(|d| d.starts_with(&root))
}

/// The harness's hook input: only `source` is read (`compact` after the
/// conversation was compacted). Anything unreadable is a plain start.
fn moment(stdin: &[u8]) -> Moment {
    let source = serde_json::from_slice::<Value>(stdin)
        .ok()
        .and_then(|v| v.get("source").and_then(Value::as_str).map(str::to_owned));
    if source.as_deref() == Some("compact") {
        Moment::Compact
    } else {
        Moment::Start
    }
}

/// The briefing in `vendor`'s output shape.
fn output(vendor: Vendor, briefing: &str) -> Value {
    match vendor {
        Vendor::Claude | Vendor::Codex => json!({
            "hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": briefing}
        }),
        Vendor::Cursor => json!({"additional_context": briefing}),
        Vendor::Antigravity => json!({
            "injectSteps": [{"systemMessage": {"systemMessage": briefing}}]
        }),
    }
}

async fn briefing(args: &Args, moment: Moment) -> precept_ipc::Result<String> {
    if !precept_ipc::valid_project(&args.project) {
        return Err(precept_ipc::Error::InvalidRequest);
    }
    let credential = load_credential(&args.location, &args.capability_file)?;
    if credential.access
        != (Access::Agent {
            project: args.project.clone(),
        })
    {
        return Err(precept_ipc::Error::Unauthorized);
    }
    let mut client = Client::connect(&args.location, &credential).await?;
    let context = client
        .request(
            "context.get",
            json!({"include_documents":false,"client":args.vendor.label()}),
        )
        .await?;
    Ok(render(&args.project, &context, moment))
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    let mut stdin = Vec::new();
    let _ = std::io::stdin().take(1024 * 1024).read_to_end(&mut stdin);
    if let Some(root) = &args.root
        && !inside(root, &stdin)
    {
        return;
    }
    let run = tokio::time::timeout(
        std::time::Duration::from_secs(4),
        briefing(&args, moment(&stdin)),
    )
    .await;
    match run {
        Ok(Ok(text)) => println!("{}", output(args.vendor, &text)),
        Ok(Err(e)) => eprintln!("precept-hook: no briefing ({e})"),
        Err(_) => eprintln!("precept-hook: no briefing (timeout)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn compact_is_read_and_shapes_match_each_vendor() {
        assert_eq!(moment(br#"{"source":"compact"}"#), Moment::Compact);
        assert_eq!(moment(br#"{"source":"startup"}"#), Moment::Start);
        assert_eq!(moment(b"not json"), Moment::Start);
        assert_eq!(
            output(Vendor::Claude, "b")["hookSpecificOutput"]["additionalContext"],
            "b"
        );
        assert_eq!(output(Vendor::Cursor, "b")["additional_context"], "b");
        assert_eq!(
            output(Vendor::Antigravity, "b")["injectSteps"][0]["systemMessage"]["systemMessage"],
            "b"
        );
        let dir = std::env::temp_dir();
        let sub = dir.join(format!("precept-hook-{}", std::process::id()));
        std::fs::create_dir_all(&sub).unwrap();
        let input =
            |p: &std::path::Path| format!("{{\"cwd\":{}}}", Value::from(p.to_str().unwrap()));
        assert!(inside(&dir, input(&sub).as_bytes()));
        assert!(!inside(&sub, input(&dir).as_bytes()));
        assert!(!inside(&sub, br#"{"cwd":"/no/such/dir"}"#));
        // Antigravity names its workspaces; any one inside the root counts.
        let workspaces = |ps: &[&std::path::Path]| {
            json!({"conversationId":"c","workspacePaths":ps.iter().map(|p| p.to_str().unwrap()).collect::<Vec<_>>()})
                .to_string()
        };
        assert!(inside(
            &sub,
            workspaces(&[Path::new("/no/such"), &sub]).as_bytes()
        ));
        assert!(!inside(&sub, workspaces(&[&dir]).as_bytes()));
        std::fs::remove_dir(&sub).unwrap();
    }
}
