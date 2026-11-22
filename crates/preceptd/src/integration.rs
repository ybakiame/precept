//! Integrations Precept writes into a registered repository, only on an
//! explicit Operator action: the AGENTS.md pointer block. A request names the
//! digest of the file the operator previewed (or none for an absent file); a
//! file that changed since is refused, never overwritten. Writes are atomic
//! (temporary file + rename in the same directory) and keep the file's mode.
use precept_core::pointer::{self, Malformed};
use precept_ipc::Error;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

fn path(root: &Path) -> PathBuf {
    root.join(pointer::FILE)
}

/// The current file: absent, or its UTF-8 text. Links, directories, special
/// files and oversized files are refused.
fn read(path: &Path) -> Result<Option<String>, Error> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Unavailable),
    };
    if !meta.file_type().is_file() {
        return Err(Error::UnsafeRuntime);
    }
    if meta.len() > pointer::MAX_BYTES as u64 {
        return Err(Error::TooLarge);
    }
    let raw = fs::read(path).map_err(|_| Error::Unavailable)?;
    String::from_utf8(raw)
        .map(Some)
        .map_err(|_| Error::InvalidRequest)
}

fn digest(current: &Option<String>) -> Value {
    current
        .as_ref()
        .map_or(Value::Null, |t| json!(pointer::digest(t.as_bytes())))
}

/// What the pointer looks like now and what applying would do.
pub fn status(root: &Path) -> Result<Value, Error> {
    let current = read(&path(root))?;
    let state = pointer::state(current.as_deref());
    Ok(json!({
        "path": pointer::FILE,
        "exists": current.is_some(),
        "sha256": digest(&current),
        "state": state.ok(),
        "malformed": state.is_err(),
        "block": pointer::block(),
    }))
}

/// Writes `text` over `path` through a new temporary file in the same
/// directory and a rename, keeping the file's mode.
pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<(), Error> {
    let dir = path.parent().ok_or(Error::UnsafeRuntime)?;
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).map_err(|_| Error::Unavailable)?;
    let name: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let file = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("file")
        .trim_start_matches('.');
    let temp = dir.join(format!(".{file}.precept-{name}"));
    let mode = fs::metadata(path).ok().map(|m| m.permissions());
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| Error::Unavailable)?;
        file.write_all(text.as_bytes())
            .map_err(|_| Error::Unavailable)?;
        file.sync_all().map_err(|_| Error::Unavailable)?;
        if let Some(mode) = mode {
            fs::set_permissions(&temp, mode).map_err(|_| Error::Unavailable)?;
        }
        fs::rename(&temp, path).map_err(|_| Error::Unavailable)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Adds (`enabled`) or removes the block, if the file is still the one the
/// operator previewed (`expected` digest, `None` for absent).
pub fn apply(root: &Path, enabled: bool, expected: Option<&str>) -> Result<Value, Error> {
    let path = path(root);
    let current = read(&path)?;
    let actual = current.as_ref().map(|t| pointer::digest(t.as_bytes()));
    if actual.as_deref() != expected {
        return Err(Error::Conflict);
    }
    let malformed = |_: Malformed| Error::InvalidRequest;
    let action = if enabled {
        let next = pointer::upsert(current.as_deref()).map_err(malformed)?;
        if current.as_deref() == Some(next.as_str()) {
            "unchanged"
        } else {
            write_atomic(&path, &next)?;
            if current.is_some() {
                "updated"
            } else {
                "created"
            }
        }
    } else {
        match current.as_deref() {
            None => "unchanged",
            Some(text) => match pointer::remove(text).map_err(malformed)? {
                None => {
                    fs::remove_file(&path).map_err(|_| Error::Unavailable)?;
                    "deleted"
                }
                Some(next) if next == text => "unchanged",
                Some(next) => {
                    write_atomic(&path, &next)?;
                    "removed"
                }
            },
        }
    };
    let mut out = status(root)?;
    out["action"] = json!(action);
    Ok(out)
}

// ---- Session-start hooks in the agents' own configuration ----------------

/// A harness whose session-start hook Precept can add to its user config.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Vendor {
    /// Claude Code: `~/.claude/settings.json`.
    Claude,
    /// Codex: `~/.codex/hooks.json` (Codex asks to approve a new hook once).
    Codex,
    /// Antigravity: `~/.gemini/config/hooks.json`, whose top-level keys name
    /// hooks; Precept owns the one named `precept-<project>`.
    Antigravity,
    /// Cursor: `~/.cursor/hooks.json`, `{version: 1, hooks: {<event>:
    /// [{command}]}}`; the gate answers `beforeShellExecution` only (Cursor
    /// documents no pre-write input).
    Cursor,
}
impl Vendor {
    pub const ALL: [Vendor; 4] = [
        Vendor::Claude,
        Vendor::Codex,
        Vendor::Antigravity,
        Vendor::Cursor,
    ];
    fn file(self) -> (&'static str, &'static str) {
        match self {
            Vendor::Claude => (".claude", "settings.json"),
            Vendor::Codex => (".codex", "hooks.json"),
            Vendor::Antigravity => (".gemini/config", "hooks.json"),
            Vendor::Cursor => (".cursor", "hooks.json"),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Vendor::Claude => "claude",
            Vendor::Codex => "codex",
            Vendor::Antigravity => "antigravity",
            Vendor::Cursor => "cursor",
        }
    }
}

/// Which harness hook an entry answers: the session-start briefing, or the
/// pre-tool gate (`--event pre-tool`, only for the tools the gate judges).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    #[default]
    SessionStart,
    PreTool,
    /// After a file read: warn when committed claims about it fail (Claude
    /// Code only; other harnesses document no read hook that passes context).
    PostRead,
}
impl HookEvent {
    /// The event's key in `vendor`'s configuration.
    fn key(self, vendor: Vendor) -> &'static str {
        match (self, vendor) {
            (Self::SessionStart, Vendor::Cursor) => "sessionStart",
            (Self::PreTool, Vendor::Cursor) => "beforeShellExecution",
            (Self::SessionStart, _) => "SessionStart",
            (Self::PreTool, _) => "PreToolUse",
            (Self::PostRead, _) => "PostToolUse",
        }
    }
    /// The tool matcher a pre-tool entry carries: shell commands and file
    /// writes, in each harness's tool names.
    fn matcher(self, vendor: Vendor) -> Option<&'static str> {
        match (self, vendor) {
            (Self::SessionStart, _) => None,
            (Self::PreTool, Vendor::Claude) => Some("Bash|Write|Edit|MultiEdit|NotebookEdit"),
            (Self::PreTool, Vendor::Codex) => Some("Bash|apply_patch"),
            (Self::PreTool, Vendor::Antigravity) => {
                Some("run_command|write_to_file|replace_file_content|multi_replace_file_content")
            }
            // beforeShellExecution judges every command.
            (Self::PreTool, Vendor::Cursor) => None,
            (Self::PostRead, _) => Some("Read"),
        }
    }
}

/// Everything a hook command names: the hook executable and the project's
/// launch inputs (the same as `precept-mcp`'s) plus the project root the hook
/// stays silent outside of.
pub struct HookLaunch {
    pub hook: PathBuf,
    pub runtime_dir: PathBuf,
    pub endpoint: String,
    pub capability: PathBuf,
    pub root: PathBuf,
}

/// A POSIX shell word: plain when safe, else single-quoted.
fn word(s: &str) -> String {
    let plain = !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./:=@+,".contains(&b));
    if plain {
        s.to_owned()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// The exact command written for `vendor`, `project` and `event`.
pub fn command(launch: &HookLaunch, vendor: Vendor, project: &str, event: HookEvent) -> String {
    let path = |p: &Path| p.to_string_lossy().into_owned();
    let mut words = vec![
        word(&path(&launch.hook)),
        "--vendor".into(),
        vendor.name().into(),
        "--project".into(),
        word(project),
        "--root".into(),
        word(&path(&launch.root)),
        "--runtime-dir".into(),
        word(&path(&launch.runtime_dir)),
        "--endpoint".into(),
        word(&launch.endpoint),
        "--capability-file".into(),
        word(&path(&launch.capability)),
    ];
    match event {
        HookEvent::SessionStart => {}
        HookEvent::PreTool => words.extend(["--event".into(), "pre-tool".into()]),
        HookEvent::PostRead => words.extend(["--event".into(), "post-read".into()]),
    }
    words.join(" ")
}

/// Whether `cmd` is a Precept hook for `project` and `event` (any version
/// of the command).
fn ours(cmd: &str, project: &str, event: HookEvent) -> bool {
    let first = cmd.split_whitespace().next().unwrap_or("");
    let words: Vec<&str> = cmd.split_whitespace().collect();
    let named = words
        .windows(2)
        .find(|w| w[0] == "--event")
        .map_or("session-start", |w| w[1]);
    let wanted = match event {
        HookEvent::SessionStart => "session-start",
        HookEvent::PreTool => "pre-tool",
        HookEvent::PostRead => "post-read",
    };
    first.trim_matches('\'').ends_with("precept-hook")
        && words
            .windows(2)
            .any(|w| w[0] == "--project" && w[1].trim_matches('\'') == project)
        && named == wanted
}

fn config_file(home: &Path, vendor: Vendor) -> PathBuf {
    let (dir, file) = vendor.file();
    home.join(dir).join(file)
}

/// The commands of `event` handlers in a Claude-shaped hooks object.
fn grouped_commands(config: &crate::ojson::J, vendor: Vendor, event: HookEvent) -> Vec<String> {
    use crate::ojson::J;
    let Some(J::Arr(groups)) = config.get("hooks").and_then(|h| h.get(event.key(vendor))) else {
        return vec![];
    };
    groups
        .iter()
        .flat_map(|g| match g.get("hooks") {
            Some(J::Arr(hs)) => hs.iter().collect::<Vec<_>>(),
            // Cursor lists handlers directly.
            _ => vec![g],
        })
        .filter_map(|h| h.get("command").and_then(|c| c.as_str()).map(str::to_owned))
        .collect()
}

/// The commands of `event` handlers in Antigravity's named hooks: each
/// top-level value's event list holds handlers directly or, as tool events
/// do, groups with `hooks`.
fn named_commands(config: &crate::ojson::J, event: HookEvent) -> Vec<String> {
    use crate::ojson::J;
    let J::Obj(entries) = config else {
        return vec![];
    };
    let mut out = Vec::new();
    for (_, hook) in entries {
        let Some(J::Arr(items)) = hook.get(event.key(Vendor::Antigravity)) else {
            continue;
        };
        for item in items {
            if let Some(c) = item.get("command").and_then(J::as_str) {
                out.push(c.to_owned());
            }
            if let Some(J::Arr(hs)) = item.get("hooks") {
                out.extend(
                    hs.iter()
                        .filter_map(|h| h.get("command").and_then(J::as_str).map(str::to_owned)),
                );
            }
        }
    }
    out
}

/// Removes `project`'s Precept handlers of `event` from Antigravity's named
/// hooks (any version, under any name), drops what that empties, then adds
/// the handler to Precept's own named hook when `enabled`.
fn edit_named(
    config: &mut crate::ojson::J,
    project: &str,
    cmd: &str,
    enabled: bool,
    event: HookEvent,
) {
    use crate::ojson::J;
    let mine = |h: &J| {
        h.get("command")
            .and_then(J::as_str)
            .is_some_and(|c| ours(c, project, event))
    };
    let name = format!("precept-{project}");
    let J::Obj(entries) = config else {
        return;
    };
    for (_, hook) in entries.iter_mut() {
        let J::Obj(events) = hook else {
            continue;
        };
        let Some((_, J::Arr(items))) = events
            .iter_mut()
            .find(|(k, _)| k == event.key(Vendor::Antigravity))
        else {
            continue;
        };
        let before = items.len();
        let mut touched = false;
        items.retain(|i| !mine(i));
        for i in items.iter_mut() {
            if let J::Obj(fields) = i
                && let Some((_, J::Arr(hs))) = fields.iter_mut().find(|(k, _)| k == "hooks")
            {
                let n = hs.len();
                hs.retain(|h| !mine(h));
                touched |= hs.len() != n;
            }
        }
        if touched {
            items.retain(|i| !matches!(i.get("hooks"), Some(J::Arr(hs)) if hs.is_empty()));
        }
        if (touched || items.len() != before) && items.is_empty() {
            events.retain(|(k, _)| k != event.key(Vendor::Antigravity));
        }
    }
    entries.retain(|(_, hook)| !matches!(hook, J::Obj(events) if events.is_empty()));
    if enabled {
        let handler = J::Obj(vec![
            ("type".into(), J::str("command")),
            ("command".into(), J::str(cmd)),
            ("timeout".into(), J::Num(10.into())),
        ]);
        // Tool events take matcher groups; SessionStart lists handlers.
        let item = match event.matcher(Vendor::Antigravity) {
            Some(m) => J::Obj(vec![
                ("matcher".into(), J::str(m)),
                ("hooks".into(), J::Arr(vec![handler])),
            ]),
            None => handler,
        };
        if !entries.iter().any(|(k, _)| *k == name) {
            entries.push((name.clone(), J::Obj(vec![])));
        }
        if let Some((_, J::Obj(events))) = entries.iter_mut().find(|(k, _)| *k == name) {
            events.push((event.key(Vendor::Antigravity).into(), J::Arr(vec![item])));
        }
    }
}

/// One harness's hook state for `project`.
pub fn hook_status(
    home: &Path,
    vendor: Vendor,
    project: &str,
    expected: &str,
    event: HookEvent,
) -> Result<Value, Error> {
    let (dir, file) = vendor.file();
    let current = read_config(&config_file(home, vendor))?;
    let parsed = current.as_deref().map(|t| {
        if t.trim().is_empty() {
            Some(crate::ojson::J::Obj(vec![]))
        } else {
            crate::ojson::parse(t)
        }
    });
    let malformed = match &parsed {
        Some(None) => true,
        Some(Some(j)) => !matches!(j, crate::ojson::J::Obj(_)),
        None => false,
    };
    let commands = parsed
        .flatten()
        .map(|j| match vendor {
            Vendor::Antigravity => named_commands(&j, event),
            _ => grouped_commands(&j, vendor, event),
        })
        .unwrap_or_default();
    let mine: Vec<&String> = commands
        .iter()
        .filter(|c| ours(c, project, event))
        .collect();
    Ok(json!({
        "vendor": vendor,
        "event": event,
        "file": format!("~/{dir}/{file}"),
        "exists": current.is_some(),
        "sha256": digest(&current),
        "malformed": malformed,
        "installed": !mine.is_empty(),
        "current": mine.len() == 1 && mine[0] == expected,
        "command": expected,
    }))
}

/// A config file Precept may edit: absent, or bounded UTF-8 in a regular file.
pub(crate) fn read_config(path: &Path) -> Result<Option<String>, Error> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Unavailable),
    };
    if !meta.file_type().is_file() {
        return Err(Error::UnsafeRuntime);
    }
    if meta.len() > pointer::MAX_BYTES as u64 {
        return Err(Error::TooLarge);
    }
    let raw = fs::read(path).map_err(|_| Error::Unavailable)?;
    String::from_utf8(raw)
        .map(Some)
        .map_err(|_| Error::InvalidRequest)
}

/// Claude Code's and Codex's shape: `hooks.<Event>` groups of handlers (a
/// tool event's group names its matcher).
fn edit_grouped(
    config: &mut crate::ojson::J,
    vendor: Vendor,
    project: &str,
    cmd: &str,
    enabled: bool,
    event: HookEvent,
) -> Result<(), Error> {
    use crate::ojson::J;
    let hooks = config
        .entry("hooks", J::Obj(vec![]))
        .ok_or(Error::InvalidRequest)?;
    if !matches!(hooks, J::Obj(_)) {
        return Err(Error::InvalidRequest);
    }
    let start = hooks
        .entry(event.key(vendor), J::Arr(vec![]))
        .ok_or(Error::InvalidRequest)?;
    let J::Arr(groups) = start else {
        return Err(Error::InvalidRequest);
    };
    // Drop this project's Precept handlers (any version), then empty groups.
    for g in groups.iter_mut() {
        if let J::Obj(fields) = g
            && let Some((_, J::Arr(hs))) = fields.iter_mut().find(|(k, _)| k == "hooks")
        {
            hs.retain(|h| {
                !h.get("command")
                    .and_then(J::as_str)
                    .is_some_and(|c| ours(c, project, event))
            });
        }
    }
    groups.retain(|g| !matches!(g.get("hooks"), Some(J::Arr(hs)) if hs.is_empty()));
    // Cursor lists handlers directly, without groups.
    if vendor == Vendor::Cursor {
        groups.retain(|h| {
            !h.get("command")
                .and_then(J::as_str)
                .is_some_and(|c| ours(c, project, event))
        });
        if enabled {
            groups.push(J::Obj(vec![
                ("command".into(), J::str(cmd)),
                ("timeout".into(), J::Num(10.into())),
            ]));
        }
        if groups.is_empty() {
            hooks.remove(event.key(vendor));
        }
        // Cursor requires `version`; an existing one is kept.
        config.entry("version", J::Num(1.into()));
        return Ok(());
    }
    if enabled {
        let mut group = vec![];
        if let Some(m) = event.matcher(vendor) {
            group.push(("matcher".into(), J::str(m)));
        }
        group.push((
            "hooks".into(),
            J::Arr(vec![J::Obj(vec![
                ("type".into(), J::str("command")),
                ("command".into(), J::str(cmd)),
                ("timeout".into(), J::Num(10.into())),
            ])]),
        ));
        groups.push(J::Obj(group));
    }
    if groups.is_empty() {
        hooks.remove(event.key(vendor));
    }
    Ok(())
}

/// Adds (`enabled`) or removes the Precept entry of `event` for `project`
/// if the file is still the one the status reported (`expected` digest).
/// Every other key and hook is kept in place; the previous file is kept as
/// `<file>.precept.bak`.
pub fn hook_apply(
    home: &Path,
    vendor: Vendor,
    project: &str,
    cmd: &str,
    enabled: bool,
    expected: Option<&str>,
    event: HookEvent,
) -> Result<Value, Error> {
    use crate::ojson::J;
    let path = config_file(home, vendor);
    let current = read_config(&path)?;
    let actual = current.as_ref().map(|t| pointer::digest(t.as_bytes()));
    if actual.as_deref() != expected {
        return Err(Error::Conflict);
    }
    let mut config = match current.as_deref() {
        None => J::Obj(vec![]),
        Some(t) if t.trim().is_empty() => J::Obj(vec![]),
        Some(t) => crate::ojson::parse(t).ok_or(Error::InvalidRequest)?,
    };
    if !matches!(config, J::Obj(_)) {
        return Err(Error::InvalidRequest);
    }
    if vendor == Vendor::Antigravity {
        edit_named(&mut config, project, cmd, enabled, event);
    } else {
        edit_grouped(&mut config, vendor, project, cmd, enabled, event)?;
    }
    let next = crate::ojson::pretty(&config);
    let action = if current.as_deref() == Some(next.as_str()) || (current.is_none() && !enabled) {
        "unchanged"
    } else {
        if let Some(dir) = path.parent()
            && !dir.exists()
        {
            fs::create_dir_all(dir).map_err(|_| Error::Unavailable)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
            }
        }
        if let Some(old) = &current {
            let backup = path.with_file_name(format!("{}.precept.bak", vendor.file().1));
            write_atomic(&backup, old)?;
        }
        write_atomic(&path, &next)?;
        #[cfg(unix)]
        if current.is_none() {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        if enabled { "added" } else { "removed" }
    };
    let mut out = hook_status(home, vendor, project, cmd, event)?;
    out["action"] = json!(action);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previewed_digest_guards_every_write_and_links_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let s = status(root).unwrap();
        assert_eq!(s["state"], "absent");
        assert_eq!(s["sha256"], Value::Null);
        let created = apply(root, true, None).unwrap();
        assert_eq!(created["action"], "created");
        assert_eq!(created["state"], "current");
        let digest = created["sha256"].as_str().unwrap().to_owned();
        assert_eq!(apply(root, true, None), Err(Error::Conflict));
        assert_eq!(
            apply(root, true, Some(&digest)).unwrap()["action"],
            "unchanged"
        );
        let removed = apply(root, false, Some(&digest)).unwrap();
        assert_eq!(removed["action"], "deleted");
        assert!(!root.join("AGENTS.md").exists());

        fs::write(root.join("AGENTS.md"), "# Rules\n").unwrap();
        let d = pointer::digest(b"# Rules\n");
        let added = apply(root, true, Some(&d)).unwrap();
        assert_eq!(added["action"], "updated");
        let text = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(text.starts_with("# Rules\n\n<!-- precept:begin v1 -->"));
        let d = added["sha256"].as_str().unwrap().to_owned();
        assert_eq!(apply(root, false, Some(&d)).unwrap()["action"], "removed");
        assert_eq!(
            fs::read_to_string(root.join("AGENTS.md")).unwrap(),
            "# Rules\n"
        );
        assert!(
            fs::read_dir(root).unwrap().count() == 1,
            "no temporary file left"
        );

        #[cfg(unix)]
        {
            fs::remove_file(root.join("AGENTS.md")).unwrap();
            std::os::unix::fs::symlink("/etc/hostname", root.join("AGENTS.md")).unwrap();
            assert_eq!(status(root), Err(Error::UnsafeRuntime));
            assert_eq!(apply(root, true, None), Err(Error::UnsafeRuntime));
        }
    }

    fn launch(root: &Path) -> HookLaunch {
        HookLaunch {
            hook: "/usr/bin/precept-hook".into(),
            runtime_dir: "/run/x".into(),
            endpoint: "/run/x/control.sock".into(),
            capability: "/run/x/agent-alpha.cap".into(),
            root: root.into(),
        }
    }

    #[test]
    fn hooks_keep_other_settings_and_hooks_in_place_and_are_reversible() {
        let home = tempfile::tempdir().unwrap();
        let home = home.path();
        let cmd = command(
            &launch(Path::new("/home/u/my repo")),
            Vendor::Claude,
            "alpha",
            HookEvent::SessionStart,
        );
        assert!(cmd.starts_with(
            "/usr/bin/precept-hook --vendor claude --project alpha --root '/home/u/my repo'"
        ));
        let s = hook_status(home, Vendor::Claude, "alpha", &cmd, HookEvent::SessionStart).unwrap();
        assert_eq!(s["installed"], false);
        assert_eq!(s["file"], "~/.claude/settings.json");
        // Absent file: created owner-only with just our entry.
        let added = hook_apply(
            home,
            Vendor::Claude,
            "alpha",
            &cmd,
            true,
            None,
            HookEvent::SessionStart,
        )
        .unwrap();
        assert_eq!(added["action"], "added");
        assert_eq!(added["current"], true);
        let file = home.join(".claude/settings.json");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let d = added["sha256"].as_str().unwrap().to_owned();
        assert_eq!(
            hook_apply(
                home,
                Vendor::Claude,
                "alpha",
                &cmd,
                true,
                Some(&d),
                HookEvent::SessionStart
            )
            .unwrap()["action"],
            "unchanged"
        );
        // An existing file: other keys, order and hooks stay.
        let theirs = "{\n  \"model\": \"opus\",\n  \"hooks\": {\n    \"SessionStart\": [{\"matcher\": \"startup\", \"hooks\": [{\"type\": \"command\", \"command\": \"echo hi\"}]}],\n    \"Stop\": []\n  },\n  \"theme\": \"dark\"\n}\n";
        fs::write(&file, theirs).unwrap();
        let d = pointer::digest(theirs.as_bytes());
        assert_eq!(
            hook_apply(
                home,
                Vendor::Claude,
                "alpha",
                &cmd,
                true,
                Some("0".repeat(64).as_str()),
                HookEvent::SessionStart
            ),
            Err(Error::Conflict)
        );
        let added = hook_apply(
            home,
            Vendor::Claude,
            "alpha",
            &cmd,
            true,
            Some(&d),
            HookEvent::SessionStart,
        )
        .unwrap();
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.find("model").unwrap() < text.find("hooks").unwrap());
        assert!(text.find("hooks").unwrap() < text.find("theme").unwrap());
        assert!(text.contains("echo hi") && text.contains("\"Stop\""));
        assert_eq!(
            fs::read_to_string(home.join(".claude/settings.json.precept.bak")).unwrap(),
            theirs
        );
        // Another project's entry stays when alpha is removed.
        let beta = command(
            &launch(Path::new("/b")),
            Vendor::Claude,
            "beta",
            HookEvent::SessionStart,
        );
        let d = added["sha256"].as_str().unwrap().to_owned();
        let both = hook_apply(
            home,
            Vendor::Claude,
            "beta",
            &beta,
            true,
            Some(&d),
            HookEvent::SessionStart,
        )
        .unwrap();
        let d = both["sha256"].as_str().unwrap().to_owned();
        let removed = hook_apply(
            home,
            Vendor::Claude,
            "alpha",
            &cmd,
            false,
            Some(&d),
            HookEvent::SessionStart,
        )
        .unwrap();
        assert_eq!(removed["installed"], false);
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.contains("--project beta") && !text.contains("--project alpha"));
        assert!(text.contains("echo hi"));
        // Not JSON: refused, untouched.
        fs::write(home.join(".codex"), "").ok();
        fs::create_dir_all(home.join(".codex2")).unwrap();
        let codex = home.join(".codex");
        let _ = fs::remove_file(&codex);
        fs::create_dir_all(&codex).unwrap();
        fs::write(codex.join("hooks.json"), "{nope").unwrap();
        let s = hook_status(home, Vendor::Codex, "alpha", &cmd, HookEvent::SessionStart).unwrap();
        assert_eq!(s["malformed"], true);
        let d = s["sha256"].as_str().unwrap().to_owned();
        assert_eq!(
            hook_apply(
                home,
                Vendor::Codex,
                "alpha",
                &cmd,
                true,
                Some(&d),
                HookEvent::SessionStart
            ),
            Err(Error::InvalidRequest)
        );
        assert_eq!(
            fs::read_to_string(codex.join("hooks.json")).unwrap(),
            "{nope"
        );
    }

    #[test]
    fn antigravity_named_hooks_keep_others_and_own_only_precept_entries() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let cmd = command(
            &launch(Path::new("/r")),
            Vendor::Antigravity,
            "alpha",
            HookEvent::SessionStart,
        );
        assert!(cmd.contains("--vendor antigravity --project alpha"));
        let file = home.join(".gemini/config/hooks.json");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        let theirs = r#"{
  "my-linter": {"PostToolUse": [{"matcher": "run_command", "hooks": [{"type": "command", "command": "./lint.sh"}]}]},
  "session-init": {"SessionStart": [{"type": "command", "command": "./init.sh"}], "enabled": true}
}"#;
        fs::write(&file, theirs).unwrap();
        let s = hook_status(
            home,
            Vendor::Antigravity,
            "alpha",
            &cmd,
            HookEvent::SessionStart,
        )
        .unwrap();
        assert_eq!(s["file"], "~/.gemini/config/hooks.json");
        assert_eq!(s["installed"], false);
        let d = s["sha256"].as_str().map(str::to_owned);
        let added = hook_apply(
            home,
            Vendor::Antigravity,
            "alpha",
            &cmd,
            true,
            d.as_deref(),
            HookEvent::SessionStart,
        )
        .unwrap();
        assert_eq!(added["action"], "added");
        assert_eq!(added["current"], true);
        let written: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(
            written["precept-alpha"]["SessionStart"][0],
            json!({"type":"command","command":cmd,"timeout":10})
        );
        assert_eq!(
            written["session-init"]["SessionStart"][0]["command"],
            "./init.sh"
        );
        assert_eq!(
            written["my-linter"]["PostToolUse"][0]["matcher"],
            "run_command"
        );
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.find("my-linter") < text.find("session-init"));
        assert!(text.find("session-init") < text.find("precept-alpha"));
        // An outdated command anywhere is replaced, not duplicated.
        let old = cmd.replace("--root /r", "--root /old");
        let stale = text.replace(&cmd, &old);
        fs::write(&file, &stale).unwrap();
        let s = hook_status(
            home,
            Vendor::Antigravity,
            "alpha",
            &cmd,
            HookEvent::SessionStart,
        )
        .unwrap();
        assert_eq!(
            (s["installed"].clone(), s["current"].clone()),
            (json!(true), json!(false))
        );
        let d = s["sha256"].as_str().map(str::to_owned);
        hook_apply(
            home,
            Vendor::Antigravity,
            "alpha",
            &cmd,
            true,
            d.as_deref(),
            HookEvent::SessionStart,
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(&file)
                .unwrap()
                .matches("precept-hook")
                .count(),
            1
        );
        // Off: only Precept's entry goes; the user's hooks stay as they were.
        let d = pointer::digest(fs::read_to_string(&file).unwrap().as_bytes());
        let removed = hook_apply(
            home,
            Vendor::Antigravity,
            "alpha",
            &cmd,
            false,
            Some(&d),
            HookEvent::SessionStart,
        )
        .unwrap();
        assert_eq!(removed["action"], "removed");
        let after: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert!(after.get("precept-alpha").is_none());
        assert_eq!(after, serde_json::from_str::<Value>(theirs).unwrap());
    }

    #[test]
    fn gate_entries_sit_beside_session_start_and_leave_independently() {
        for vendor in Vendor::ALL {
            let dir = tempfile::tempdir().unwrap();
            let home = dir.path();
            let l = launch(Path::new("/r"));
            let start = command(&l, vendor, "alpha", HookEvent::SessionStart);
            let gate = command(&l, vendor, "alpha", HookEvent::PreTool);
            assert!(gate.ends_with("--event pre-tool"));
            let digest = |e| {
                hook_status(home, vendor, "alpha", "", e).unwrap()["sha256"]
                    .as_str()
                    .map(str::to_owned)
            };
            hook_apply(
                home,
                vendor,
                "alpha",
                &start,
                true,
                None,
                HookEvent::SessionStart,
            )
            .unwrap();
            let d = digest(HookEvent::SessionStart);
            let added = hook_apply(
                home,
                vendor,
                "alpha",
                &gate,
                true,
                d.as_deref(),
                HookEvent::PreTool,
            )
            .unwrap();
            assert_eq!(added["installed"], true, "{vendor:?}");
            assert_eq!(added["current"], true);
            let raw = fs::read_to_string(config_file(home, vendor)).unwrap();
            assert!(
                raw.contains(HookEvent::PreTool.key(vendor))
                    && raw.contains(HookEvent::SessionStart.key(vendor))
            );
            if let Some(matcher) = HookEvent::PreTool.matcher(vendor) {
                assert!(raw.contains(matcher), "{vendor:?}");
            }
            if vendor == Vendor::Cursor {
                let v: Value = serde_json::from_str(&raw).unwrap();
                assert_eq!(v["version"], 1);
                assert_eq!(
                    v["hooks"]["beforeShellExecution"][0]["command"],
                    gate.as_str()
                );
                assert_eq!(v["hooks"]["sessionStart"][0]["command"], start.as_str());
            }
            let start_status =
                hook_status(home, vendor, "alpha", &start, HookEvent::SessionStart).unwrap();
            assert_eq!(start_status["current"], true, "{vendor:?}");
            // Removing the gate keeps the briefing.
            let d = digest(HookEvent::PreTool);
            hook_apply(
                home,
                vendor,
                "alpha",
                &gate,
                false,
                d.as_deref(),
                HookEvent::PreTool,
            )
            .unwrap();
            let raw = fs::read_to_string(config_file(home, vendor)).unwrap();
            assert!(!raw.contains(HookEvent::PreTool.key(vendor)), "{vendor:?}");
            assert!(!raw.contains("pre-tool"));
            assert_eq!(
                hook_status(home, vendor, "alpha", &start, HookEvent::SessionStart).unwrap()["current"],
                true
            );
        }
    }
}
