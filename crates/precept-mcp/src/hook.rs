//! `precept-hook`: the command an agent harness runs at session start (and
//! when it resumes after compacting its conversation), and, with
//! `--event pre-tool`, before a tool call. At session start it prints a short
//! briefing of the project's effective context; before a tool call it asks
//! the daemon's gate whether the call breaks a committed hard rule and, if
//! so, denies it in the harness's own output shape. Fail-open: any problem
//! prints nothing the harness would act on, exits 0 and leaves one sanitized
//! line on stderr. Hooks are cooperative compliance, not a sandbox.
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
    /// Cursor's `sessionStart` and `beforeShellExecution` hooks
    /// (`~/.cursor/hooks.json`).
    Cursor,
    /// Antigravity's `SessionStart` hook (`~/.gemini/config/hooks.json`).
    Antigravity,
    /// Qoder (`~/.qoder/settings.json`) and Qoder CN
    /// (`~/.qoder-cn/settings.json`), which read the same hook shape as
    /// Claude Code.
    Qoder,
    #[value(name = "qoder_cn")]
    QoderCn,
    /// CodeBuddy (`~/.codebuddy/settings.json`), Claude Code's hook shape.
    CodeBuddy,
    /// WorkBuddy (`~/.workbuddy/settings.json`), Claude Code's hook shape.
    WorkBuddy,
}
impl Vendor {
    fn label(self) -> &'static str {
        match self {
            Self::Claude => "claude-code hook",
            Self::Codex => "codex hook",
            Self::Cursor => "cursor hook",
            Self::Antigravity => "antigravity hook",
            Self::Qoder => "qoder hook",
            Self::QoderCn => "qoder-cn hook",
            Self::CodeBuddy => "codebuddy hook",
            Self::WorkBuddy => "workbuddy hook",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Event {
    SessionStart,
    PreTool,
    /// After the agent read a file: warn when committed rules' claims about
    /// it fail (the document may be stale).
    PostRead,
    /// Before the conversation is compacted: remind the agent to keep its
    /// work session current so the work survives compaction.
    PreCompact,
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
    /// Which hook this run answers.
    #[arg(long, value_enum, default_value = "session-start")]
    event: Event,
}

/// What the gate is asked about: a shell command, or written paths as the
/// harness named them (absolute, or relative to `cwd`).
#[derive(Debug, PartialEq, Eq)]
enum Call {
    Shell(String),
    Write(Vec<String>),
}

/// The tool call in a PreToolUse input. Claude Code and Codex send
/// `tool_name` / `tool_input`; Antigravity sends `toolCall.name` /
/// `toolCall.args` (PascalCase). Tools the gate does not judge are `None`.
fn tool_call(stdin: &[u8]) -> Option<Call> {
    let v: Value = serde_json::from_slice(stdin).ok()?;
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    // Cursor's beforeShellExecution: the command at the top level.
    if v.get("hook_event_name").and_then(Value::as_str) == Some("beforeShellExecution") {
        return s(&v, "command").map(Call::Shell);
    }
    if let Some(call) = v.get("toolCall") {
        let args = call.get("args")?;
        return match call.get("name")?.as_str()? {
            "run_command" => s(args, "CommandLine").map(Call::Shell),
            "write_to_file" | "replace_file_content" | "multi_replace_file_content" => {
                s(args, "TargetFile").map(|p| Call::Write(vec![p]))
            }
            _ => None,
        };
    }
    let input = v.get("tool_input")?;
    match v.get("tool_name")?.as_str()? {
        "Bash" | "shell" | "exec_command" => s(input, "command").map(Call::Shell),
        "Write" | "Edit" | "MultiEdit" => s(input, "file_path").map(|p| Call::Write(vec![p])),
        "NotebookEdit" => s(input, "notebook_path").map(|p| Call::Write(vec![p])),
        // Codex's file edits: the patch names its files.
        "apply_patch" => {
            let patch = s(input, "command")?;
            let paths: Vec<String> = patch
                .lines()
                .filter_map(|l| {
                    [
                        "*** Add File: ",
                        "*** Update File: ",
                        "*** Delete File: ",
                        "*** Move to: ",
                    ]
                    .iter()
                    .find_map(|p| l.strip_prefix(p))
                })
                .map(|p| p.trim().to_owned())
                .filter(|p| !p.is_empty())
                .collect();
            (!paths.is_empty()).then_some(Call::Write(paths))
        }
        _ => None,
    }
}

/// The file a read tool call read (Claude Code's `Read`), as named.
fn read_path(stdin: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(stdin).ok()?;
    if v.get("tool_name")?.as_str()? != "Read" {
        return None;
    }
    v.get("tool_input")?
        .get("file_path")?
        .as_str()
        .map(str::to_owned)
}

/// The warning shown to the agent after it read a file whose claims fail.
fn stale_warning(path: &str, stale: &[Value]) -> Option<String> {
    if stale.is_empty() {
        return None;
    }
    let mut out = format!(
        "Precept: committed rules make claims about `{path}` that do not hold in the repository now, so this file may be out of date. Check it against the code before relying on it:\n"
    );
    for s in stale.iter().take(5) {
        let f = &s["failure"];
        let what = match f["kind"].as_str() {
            Some("exists") => "the file is missing".to_owned(),
            Some("contains") => format!(
                "expected to contain \"{}\"",
                f["expected"].as_str().unwrap_or("")
            ),
            Some("absent") => format!(
                "expected not to contain \"{}\"",
                f["expected"].as_str().unwrap_or("")
            ),
            Some("equals") => format!(
                "`{}` expected {} but is {}",
                f["path"].as_str().unwrap_or("?"),
                f["expected"].as_str().unwrap_or("?"),
                f["found"].as_str().unwrap_or("absent")
            ),
            _ => "a check fails".to_owned(),
        };
        out.push_str(&format!(
            "- rule `{}` ({}): {what}\n",
            s["key"].as_str().unwrap_or("?"),
            s["file"].as_str().unwrap_or("?")
        ));
    }
    Some(out)
}

/// Asks which committed claims about the file in `stdin` fail.
async fn read_check(args: &Args, stdin: &[u8]) -> precept_ipc::Result<Option<String>> {
    let Some(path) = read_path(stdin) else {
        return Ok(None);
    };
    let Some(root) = args.root.as_ref().and_then(|r| r.canonicalize().ok()) else {
        return Ok(None);
    };
    let cwd = session_dirs(stdin)
        .into_iter()
        .next()
        .and_then(|d| d.canonicalize().ok())
        .unwrap_or_else(|| root.clone());
    let Some(relative) = repo_relative(&root, &cwd, &path) else {
        return Ok(None);
    };
    let mut client = connect(args).await?;
    let answer = client
        .request(
            "doc.check",
            json!({"paths":[relative],"client":args.vendor.label()}),
        )
        .await?;
    let stale = answer["stale"].as_array().cloned().unwrap_or_default();
    Ok(stale_warning(&relative, &stale))
}

/// The post-read warning in `vendor`'s shape (`None`: say nothing).
fn read_output(vendor: Vendor, warning: &str) -> Option<Value> {
    match vendor {
        Vendor::Claude => Some(json!({
            "hookSpecificOutput": {"hookEventName": "PostToolUse", "additionalContext": warning}
        })),
        _ => None,
    }
}

/// The reminder shown before compaction: keep the work state in the
/// project's sessions (the agent's own records), naming the open session
/// when there is one.
async fn compact_note(args: &Args) -> precept_ipc::Result<String> {
    let mut client = connect(args).await?;
    let context = client
        .request(
            "context.get",
            json!({"include_documents":false,"client":args.vendor.label()}),
        )
        .await?;
    let open = context["work_sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|s| s["status"] == "open");
    Ok(match open {
        Some(s) => format!(
            "Precept: this conversation is about to be compacted. Keep your work state in the project's sessions so it survives: if session `{}` (rev {}) is yours, bring it current with update_work_state (done, doing, todo, expected_rev); otherwise record your own with start_session. After compaction, call get_effective_context again.",
            s["id"].as_str().unwrap_or("?"),
            s["rev"].as_u64().unwrap_or(0)
        ),
        None => "Precept: this conversation is about to be compacted. If you are mid-task, record it with start_session (goal, done/doing/todo) so the work survives compaction and moves between agents. After compaction, call get_effective_context again.".to_owned(),
    })
}

/// The pre-compaction reminder in `vendor`'s shape (`None`: say nothing).
fn compact_output(vendor: Vendor, note: &str) -> Option<Value> {
    match vendor {
        Vendor::Claude | Vendor::Qoder | Vendor::QoderCn | Vendor::CodeBuddy => Some(json!({
            "hookSpecificOutput": {"hookEventName": "PreCompact", "additionalContext": note}
        })),
        _ => None,
    }
}

/// `path` relative to `root` (lexically: `.` and `..` resolved), or `None`
/// when it is outside. A relative path is taken from `cwd`.
fn repo_relative(root: &std::path::Path, cwd: &std::path::Path, path: &str) -> Option<String> {
    use std::path::Component;
    // Windows canonicalize yields a \\?\ verbatim prefix; joining an absolute
    // input path drops it, so compare without it on both sides.
    let plain = |p: &std::path::Path| -> std::path::PathBuf {
        let s = p.to_string_lossy();
        match s.strip_prefix(r"\\?\") {
            Some(rest) => std::path::PathBuf::from(rest.to_owned()),
            None => p.to_path_buf(),
        }
    };
    let root = plain(root);
    let joined = plain(cwd).join(path);
    let mut parts: Vec<std::ffi::OsString> = Vec::new();
    let mut prefix = std::path::PathBuf::new();
    for c in joined.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => prefix.push(c.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::Normal(p) => parts.push(p.to_owned()),
        }
    }
    let mut full = prefix;
    full.extend(&parts);
    let rest = full.strip_prefix(&root).ok()?;
    let text: Vec<&str> = rest
        .components()
        .map(|c| c.as_os_str().to_str())
        .collect::<Option<_>>()?;
    (!text.is_empty()).then(|| text.join("/"))
}

/// The gate's denial in `vendor`'s PreToolUse shape (`None`: say nothing).
fn deny_output(vendor: Vendor, reason: &str) -> Option<Value> {
    match vendor {
        Vendor::Claude
        | Vendor::Codex
        | Vendor::Qoder
        | Vendor::QoderCn
        | Vendor::CodeBuddy
        | Vendor::WorkBuddy => Some(json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason
            }
        })),
        Vendor::Antigravity => Some(json!({"decision": "deny", "reason": reason})),
        Vendor::Cursor => Some(json!({
            "permission": "deny",
            "user_message": reason,
            "agent_message": reason
        })),
    }
}

/// What every other pre-tool answer prints: Cursor's permission hooks treat
/// empty output as invalid and block, so it is told `allow` explicitly;
/// other harnesses proceed on silence.
fn allow_output(vendor: Vendor) -> Option<Value> {
    (vendor == Vendor::Cursor).then(|| json!({"permission": "allow"}))
}

/// Asks the gate about the call in `stdin`; `Some(reason)` to deny.
async fn gate(args: &Args, stdin: &[u8]) -> precept_ipc::Result<Option<String>> {
    let Some(call) = tool_call(stdin) else {
        return Ok(None);
    };
    let mut client = connect(args).await?;
    let params = match call {
        Call::Shell(command) => {
            json!({"tool":"shell","command":command,"client":args.vendor.label()})
        }
        Call::Write(paths) => {
            // Only paths inside the project are judged.
            let Some(root) = args.root.as_ref().and_then(|r| r.canonicalize().ok()) else {
                return Ok(None);
            };
            let cwd = session_dirs(stdin)
                .into_iter()
                .next()
                .and_then(|d| d.canonicalize().ok())
                .unwrap_or_else(|| root.clone());
            let paths: Vec<String> = paths
                .iter()
                .filter_map(|p| repo_relative(&root, &cwd, p))
                .take(32)
                .collect();
            if paths.is_empty() {
                return Ok(None);
            }
            json!({"tool":"write","paths":paths,"client":args.vendor.label()})
        }
    };
    let answer = client.request("gate.check", params).await?;
    Ok((answer["decision"] == "deny").then(|| {
        answer["reason"]
            .as_str()
            .unwrap_or("Blocked by a repository rule.")
            .to_owned()
    }))
}

/// The session's directories from the hook input: `cwd` (Claude Code, Codex,
/// Cursor's shell hook), else Antigravity's `workspacePaths` or Cursor's
/// `workspace_roots` (a leading `~/` is the home), else this process's
/// working directory.
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
        .and_then(|v| v.get("workspacePaths").or_else(|| v.get("workspace_roots")))
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
        Vendor::Claude
        | Vendor::Codex
        | Vendor::Qoder
        | Vendor::QoderCn
        | Vendor::CodeBuddy
        | Vendor::WorkBuddy => json!({
            "hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": briefing}
        }),
        Vendor::Cursor => json!({"additional_context": briefing}),
        Vendor::Antigravity => json!({
            "injectSteps": [{"systemMessage": {"systemMessage": briefing}}]
        }),
    }
}

async fn connect(args: &Args) -> precept_ipc::Result<Client> {
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
    Client::connect(&args.location, &credential).await
}

async fn briefing(args: &Args, moment: Moment) -> precept_ipc::Result<String> {
    let mut client = connect(args).await?;
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
    let outside = args.root.as_ref().is_some_and(|root| !inside(root, &stdin));
    if args.event == Event::PreTool {
        let denied = if outside {
            None
        } else {
            let run =
                tokio::time::timeout(std::time::Duration::from_secs(4), gate(&args, &stdin)).await;
            match run {
                Ok(Ok(reason)) => reason,
                Ok(Err(e)) => {
                    eprintln!("precept-hook: gate not asked ({e})");
                    None
                }
                Err(_) => {
                    eprintln!("precept-hook: gate not asked (timeout)");
                    None
                }
            }
        };
        let out = match denied {
            Some(reason) => deny_output(args.vendor, &reason),
            None => allow_output(args.vendor),
        };
        if let Some(out) = out {
            println!("{out}");
        }
        return;
    }
    if outside {
        return;
    }
    if args.event == Event::PreCompact {
        let run =
            tokio::time::timeout(std::time::Duration::from_secs(4), compact_note(&args)).await;
        match run {
            Ok(Ok(note)) => {
                if let Some(out) = compact_output(args.vendor, &note) {
                    println!("{out}");
                }
            }
            Ok(Err(e)) => eprintln!("precept-hook: compact note skipped ({e})"),
            Err(_) => eprintln!("precept-hook: compact note skipped (timeout)"),
        }
        return;
    }
    if args.event == Event::PostRead {
        let run =
            tokio::time::timeout(std::time::Duration::from_secs(4), read_check(&args, &stdin))
                .await;
        match run {
            Ok(Ok(Some(warning))) => {
                if let Some(out) = read_output(args.vendor, &warning) {
                    println!("{out}");
                }
            }
            Ok(Ok(None)) => {}
            Ok(Err(e)) => eprintln!("precept-hook: read not checked ({e})"),
            Err(_) => eprintln!("precept-hook: read not checked (timeout)"),
        }
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
    fn tool_calls_are_read_from_each_harness_shape() {
        let call = |v: Value| tool_call(v.to_string().as_bytes());
        assert_eq!(
            call(json!({"tool_name":"Bash","tool_input":{"command":"git push --force"}})),
            Some(Call::Shell("git push --force".into()))
        );
        assert_eq!(
            call(json!({"tool_name":"Edit","tool_input":{"file_path":"/r/Cargo.lock"}})),
            Some(Call::Write(vec!["/r/Cargo.lock".into()]))
        );
        assert_eq!(
            call(json!({"tool_name":"apply_patch","tool_input":{"command":
                "*** Begin Patch\n*** Update File: src/a.rs\n@@\n*** Add File: Cargo.lock\n*** End Patch"}})),
            Some(Call::Write(vec!["src/a.rs".into(), "Cargo.lock".into()]))
        );
        assert_eq!(
            call(
                json!({"toolCall":{"name":"run_command","args":{"CommandLine":"rm -rf x","Cwd":"/r"}}})
            ),
            Some(Call::Shell("rm -rf x".into()))
        );
        assert_eq!(
            call(json!({"toolCall":{"name":"write_to_file","args":{"TargetFile":"/r/a"}}})),
            Some(Call::Write(vec!["/r/a".into()]))
        );
        assert_eq!(
            call(json!({"tool_name":"Read","tool_input":{"file_path":"a"}})),
            None
        );
        assert_eq!(
            call(json!({"toolCall":{"name":"view_file","args":{}}})),
            None
        );
        assert_eq!(tool_call(b"not json"), None);
    }

    #[test]
    fn paths_are_made_repository_relative_or_dropped() {
        let root = Path::new("/w/repo");
        let cwd = Path::new("/w/repo/src");
        assert_eq!(
            repo_relative(root, cwd, "lib.rs").as_deref(),
            Some("src/lib.rs")
        );
        assert_eq!(
            repo_relative(root, cwd, "../Cargo.lock").as_deref(),
            Some("Cargo.lock")
        );
        assert_eq!(
            repo_relative(root, cwd, "/w/repo/./vendor/x").as_deref(),
            Some("vendor/x")
        );
        assert_eq!(repo_relative(root, cwd, "../../other/x"), None);
        assert_eq!(repo_relative(root, cwd, "/etc/passwd"), None);
        assert_eq!(repo_relative(root, cwd, ".."), None);
    }

    #[cfg(windows)]
    #[test]
    fn verbatim_prefixes_do_not_hide_the_repository() {
        let root = Path::new(r"\\?\C:\w\repo");
        let cwd = Path::new(r"C:\w\repo\src");
        assert_eq!(
            repo_relative(root, cwd, "lib.rs").as_deref(),
            Some("src/lib.rs")
        );
        assert_eq!(
            repo_relative(root, cwd, r"C:\w\repo\docs\a.md").as_deref(),
            Some("docs/a.md")
        );
        assert_eq!(repo_relative(root, cwd, r"C:\other\x"), None);
    }

    #[test]
    fn denials_match_each_vendor() {
        let claude = deny_output(Vendor::Claude, "no").unwrap();
        assert_eq!(claude["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(
            claude["hookSpecificOutput"]["permissionDecisionReason"],
            "no"
        );
        assert_eq!(deny_output(Vendor::Qoder, "no"), Some(claude.clone()));
        assert_eq!(deny_output(Vendor::QoderCn, "no"), Some(claude.clone()));
        assert_eq!(deny_output(Vendor::CodeBuddy, "no"), Some(claude.clone()));
        assert_eq!(deny_output(Vendor::WorkBuddy, "no"), Some(claude.clone()));
        assert_eq!(deny_output(Vendor::Codex, "no"), Some(claude));
        assert_eq!(
            deny_output(Vendor::Antigravity, "no"),
            Some(json!({"decision":"deny","reason":"no"}))
        );
        assert_eq!(
            deny_output(Vendor::Cursor, "no"),
            Some(json!({"permission":"deny","user_message":"no","agent_message":"no"}))
        );
        assert_eq!(
            allow_output(Vendor::Cursor),
            Some(json!({"permission":"allow"}))
        );
        assert_eq!(allow_output(Vendor::Claude), None);
        assert_eq!(
            tool_call(
                br#"{"hook_event_name":"beforeShellExecution","command":"git push -f","cwd":"/r"}"#
            ),
            Some(Call::Shell("git push -f".into()))
        );
    }
    #[test]
    fn compact_is_read_and_shapes_match_each_vendor() {
        assert_eq!(moment(br#"{"source":"compact"}"#), Moment::Compact);
        assert_eq!(moment(br#"{"source":"startup"}"#), Moment::Start);
        assert_eq!(moment(b"not json"), Moment::Start);
        assert_eq!(
            output(Vendor::Claude, "b")["hookSpecificOutput"]["additionalContext"],
            "b"
        );
        assert_eq!(output(Vendor::Qoder, "b"), output(Vendor::Claude, "b"));
        assert_eq!(output(Vendor::QoderCn, "b"), output(Vendor::Claude, "b"));
        assert_eq!(output(Vendor::CodeBuddy, "b"), output(Vendor::Claude, "b"));
        assert_eq!(output(Vendor::WorkBuddy, "b"), output(Vendor::Claude, "b"));
        assert_eq!(
            compact_output(Vendor::Qoder, "n"),
            Some(json!({
                "hookSpecificOutput": {"hookEventName": "PreCompact", "additionalContext": "n"}
            }))
        );
        assert_eq!(
            compact_output(Vendor::Claude, "n"),
            compact_output(Vendor::Qoder, "n")
        );
        assert_eq!(
            compact_output(Vendor::QoderCn, "n"),
            compact_output(Vendor::Qoder, "n")
        );
        assert_eq!(
            compact_output(Vendor::CodeBuddy, "n"),
            compact_output(Vendor::Qoder, "n")
        );
        assert_eq!(compact_output(Vendor::WorkBuddy, "n"), None);
        assert_eq!(compact_output(Vendor::Cursor, "n"), None);
        assert_eq!(compact_output(Vendor::Antigravity, "n"), None);
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
