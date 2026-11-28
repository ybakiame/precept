//! The Workspace page's project management: the add draft (one chosen folder,
//! checked by the daemon), the edit draft (display name and a moved folder),
//! the remove confirmation and the mapping from a
//! folder the Windows dialog returns to the WSL distribution's Linux path.
//! Pure: no I/O. The daemon alone decides; nothing here grants anything.
use serde_json::Value;

/// The daemon's answer for the chosen folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Check {
    #[default]
    Idle,
    Checking,
    Ok {
        root: String,
        name: Option<String>,
        known: bool,
    },
    /// A fixed refusal code from the daemon's scope rules.
    Refused(String),
    /// The request itself failed (a fixed client/bridge reason).
    Failed(&'static str),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AddDraft {
    pub open: bool,
    /// What the user chose or typed (a daemon-host path).
    pub path: String,
    pub name: String,
    /// The user changed the suggested name; later checks keep it.
    pub name_edited: bool,
    pub check: Check,
    pub epoch: u64,
    pub busy: bool,
    /// The last add or remove refusal (a fixed code).
    pub error: Option<String>,
    /// Whether the selected Profile's writes are enabled (`None`: not read yet).
    pub write_enabled: Option<bool>,
    /// Whether a Profile is selected (`None`: not read yet).
    pub profile_selected: Option<bool>,
}

impl AddDraft {
    pub fn set_path(&mut self, path: String) {
        self.path = path.chars().take(4096).collect();
        self.epoch = self.epoch.wrapping_add(1);
        self.check = Check::Idle;
        self.error = None;
    }
    pub fn set_name(&mut self, name: String) {
        self.name = name.chars().take(64).collect();
        self.name_edited = true;
        self.error = None;
    }
    /// Starts a check of the current path; its reply must carry this epoch.
    pub fn begin_check(&mut self) -> u64 {
        self.epoch = self.epoch.wrapping_add(1);
        self.check = Check::Checking;
        self.epoch
    }
    /// The daemon's `project.check` reply; ignored if the path changed since.
    pub fn finish_check(&mut self, epoch: u64, reply: Result<Value, &'static str>) {
        if epoch != self.epoch {
            return;
        }
        self.check = checked(reply);
        // A Profile record's name wins over an edit: it is that project.
        if let Check::Ok {
            name: Some(n),
            known,
            ..
        } = &self.check
            && (*known || !self.name_edited)
        {
            self.name = n.clone();
            self.name_edited = false;
        }
    }
    /// The exact `project.add` parameters, once the daemon accepted the folder
    /// and the name is valid.
    pub fn ready(&self) -> Option<(String, String)> {
        match &self.check {
            Check::Ok { root, .. } if valid_name(&self.name) && !self.busy => {
                Some((self.name.clone(), root.clone()))
            }
            _ => None,
        }
    }
    pub fn reset(&mut self) {
        let epoch = self.epoch.wrapping_add(1);
        let write_enabled = self.write_enabled;
        let profile_selected = self.profile_selected;
        *self = Self {
            epoch,
            write_enabled,
            profile_selected,
            ..Self::default()
        };
    }
}

/// A `project.check` reply as a [`Check`].
fn checked(reply: Result<Value, &'static str>) -> Check {
    match reply {
        Err(e) => Check::Failed(e),
        Ok(v) if v.get("ok").and_then(Value::as_bool) == Some(true) => {
            let Some(root) = v.get("root").and_then(Value::as_str) else {
                return Check::Failed("Invalid reply");
            };
            Check::Ok {
                root: root.to_owned(),
                name: v.get("name").and_then(Value::as_str).map(str::to_owned),
                known: v.get("known").and_then(Value::as_bool).unwrap_or(false),
            }
        }
        Ok(v) => Check::Refused(
            v.get("reason")
                .and_then(Value::as_str)
                .unwrap_or("refused")
                .chars()
                .take(32)
                .collect(),
        ),
    }
}

/// Editing one listed project: its display name and, after its folder moved,
/// its new folder (checked by the daemon like an added one).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditDraft {
    /// The project's name: its identity, never edited here.
    pub name: String,
    /// What the daemon reports now.
    pub saved_label: Option<String>,
    pub saved_root: String,
    pub label: String,
    /// The folder field; starts at the saved root.
    pub path: String,
    pub check: Check,
    pub epoch: u64,
    pub busy: bool,
    /// The last refusal (a fixed code).
    pub error: Option<String>,
}

impl EditDraft {
    pub fn open(entry: &crate::ProjectEntry) -> Self {
        Self {
            name: entry.name.clone(),
            saved_label: entry.label.clone(),
            saved_root: entry.root.clone(),
            label: entry.label.clone().unwrap_or_default(),
            path: entry.root.clone(),
            ..Self::default()
        }
    }
    pub fn set_label(&mut self, label: String) {
        self.label = label.chars().take(MAX_LABEL).collect();
        self.error = None;
    }
    pub fn set_path(&mut self, path: String) {
        self.path = path.chars().take(4096).collect();
        self.epoch = self.epoch.wrapping_add(1);
        self.check = Check::Idle;
        self.error = None;
    }
    pub fn begin_check(&mut self) -> u64 {
        self.epoch = self.epoch.wrapping_add(1);
        self.check = Check::Checking;
        self.epoch
    }
    /// The daemon's `project.check` reply; ignored if the path changed since.
    pub fn finish_check(&mut self, epoch: u64, reply: Result<Value, &'static str>) {
        if epoch == self.epoch {
            self.check = checked(reply);
        }
    }
    /// The folder field names another folder than the saved root.
    pub fn moving(&self) -> bool {
        self.path.trim() != self.saved_root
    }
    /// A folder the Profile already records for another project.
    pub fn taken(&self) -> bool {
        matches!(&self.check, Check::Ok { known: true, name, .. } if name.as_deref() != Some(&self.name))
    }
    /// The `project.label` value to send, if the display name changed: the
    /// trimmed text, or `None` to show the name again.
    pub fn label_change(&self) -> Option<Option<String>> {
        let label = self.label.trim();
        let next = (!label.is_empty() && label != self.name).then(|| label.to_owned());
        (next != self.saved_label).then_some(next)
    }
    /// The `project.relocate` root, once the daemon accepted the new folder.
    pub fn relocation(&self) -> Option<String> {
        match &self.check {
            Check::Ok { root, .. }
                if self.moving() && !self.taken() && *root != self.saved_root =>
            {
                Some(root.clone())
            }
            _ => None,
        }
    }
    /// Something to save, and nothing pending: a changed folder must be checked.
    pub fn ready(&self) -> bool {
        let label_ok = self
            .label_change()
            .is_none_or(|l| l.as_deref().is_none_or(valid_label));
        !self.busy
            && label_ok
            && (!self.moving() || self.relocation().is_some())
            && (self.label_change().is_some() || self.relocation().is_some())
    }
}

/// The longest display name, in characters.
pub const MAX_LABEL: usize = 64;
/// The daemon's display-name rule: 1–64 characters, trimmed, no control
/// characters.
pub fn valid_label(label: &str) -> bool {
    (1..=MAX_LABEL).contains(&label.chars().count())
        && label.trim() == label
        && !label.chars().any(char::is_control)
}

/// The daemon's project-name rule (1–64 of letters, digits, `-`, `_`, `.`).
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

/// The Linux path inside `distribution` for a folder the Windows dialog
/// returned (`\\wsl.localhost\<distribution>\…` or `\\wsl$\<distribution>\…`).
/// Anything else (a drive path, another distribution) is refused before any
/// request: the daemon only manages its own system's directories.
pub fn wsl_path(distribution: &str, picked: &str) -> Result<String, &'static str> {
    const OUTSIDE: &str = "outside_distribution";
    let normalized = picked.replace('/', "\\");
    let lower = normalized.to_ascii_lowercase();
    let rest = ["\\\\wsl.localhost\\", "\\\\wsl$\\"]
        .iter()
        .find_map(|prefix| {
            lower
                .strip_prefix(prefix)
                .map(|_| &normalized[prefix.len()..])
        })
        .ok_or(OUTSIDE)?;
    let (distro, path) = rest.split_once('\\').unwrap_or((rest, ""));
    if !distro.eq_ignore_ascii_case(distribution) {
        return Err(OUTSIDE);
    }
    let parts: Vec<&str> = path
        .split('\\')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    if parts
        .iter()
        .any(|p| *p == ".." || p.chars().any(char::is_control))
    {
        return Err(OUTSIDE);
    }
    Ok(format!("/{}", parts.join("/")))
}

/// The message for a daemon refusal code or a client-side reason.
pub fn refusal(code: &str) -> crate::i18n::Msg {
    use crate::i18n::Msg::*;
    match code {
        "not_absolute" => RefusedNotAbsolute,
        "not_found" => RefusedNotFound,
        "not_directory" => RefusedNotDirectory,
        "no_home" => RefusedNoHome,
        "outside_home" => RefusedOutsideHome,
        "is_home" => RefusedIsHome,
        "system_path" => RefusedSystemPath,
        "other_filesystem" => RefusedOtherFilesystem,
        "not_owner" => RefusedNotOwner,
        "not_git_root" => RefusedNotGitRoot,
        "overlaps_daemon" => RefusedOverlapsDaemon,
        "overlaps_profile" => RefusedOverlapsProfile,
        "overlaps_project" => RefusedOverlapsProject,
        "outside_distribution" => RefusedOutsideDistribution,
        "Conflict" => ProjectExists,
        "Capacity" => ProjectCapacity,
        "OutcomeUnknown" => ProjectOutcomeUnknown,
        "Unauthorized" => NeedsWrite,
        "startup_missing" => StartupMissing,
        "startup_unsafe" => StartupUnsafe,
        "startup_unreadable" | "startup_refused" => StartupRefused,
        _ => ProjectFailed,
    }
}

/// The Windows dialog's starting folder for a WSL user's home.
pub fn wsl_home(distribution: &str, user: &str) -> String {
    format!("\\\\wsl.localhost\\{distribution}\\home\\{user}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn unc_paths_of_the_connected_distribution_map_to_linux_paths_only() {
        for (picked, linux) in [
            (
                "\\\\wsl.localhost\\archlinux\\home\\me\\code\\precept",
                "/home/me/code/precept",
            ),
            ("\\\\wsl$\\archlinux\\home\\me\\repo\\", "/home/me/repo"),
            (
                "\\\\WSL.LOCALHOST\\ArchLinux\\home\\me\\Repo",
                "/home/me/Repo",
            ),
            ("//wsl.localhost/archlinux/home/me/./repo", "/home/me/repo"),
        ] {
            assert_eq!(
                wsl_path("archlinux", picked).as_deref(),
                Ok(linux),
                "{picked}"
            );
        }
        for picked in [
            "C:\\Users\\me\\repo",
            "\\\\wsl.localhost\\Ubuntu\\home\\me\\repo",
            "\\\\wsl.localhost\\archlinux\\home\\me\\..\\other",
            "\\\\server\\share\\repo",
            "relative\\repo",
            "",
        ] {
            assert_eq!(
                wsl_path("archlinux", picked),
                Err("outside_distribution"),
                "{picked}"
            );
        }
        assert_eq!(
            wsl_home("archlinux", "me"),
            "\\\\wsl.localhost\\archlinux\\home\\me"
        );
    }
    #[test]
    fn a_stale_check_never_lands_and_a_known_name_wins_over_an_edit() {
        let mut d = AddDraft::default();
        d.set_path("/home/me/a".into());
        let first = d.begin_check();
        d.set_path("/home/me/b".into());
        let second = d.begin_check();
        d.finish_check(
            first,
            Ok(json!({"ok":true,"root":"/home/me/a","name":"a","known":false})),
        );
        assert_eq!(d.check, Check::Checking);
        d.finish_check(
            second,
            Ok(json!({"ok":true,"root":"/home/me/b","name":"b","known":false})),
        );
        assert_eq!(d.name, "b");
        assert_eq!(d.ready(), Some(("b".into(), "/home/me/b".into())));
        d.set_name("bee".into());
        let third = d.begin_check();
        d.finish_check(
            third,
            Ok(json!({"ok":true,"root":"/home/me/b","name":"b","known":false})),
        );
        assert_eq!(d.name, "bee", "an edited name survives a new check");
        let fourth = d.begin_check();
        d.finish_check(
            fourth,
            Ok(json!({"ok":true,"root":"/home/me/b","name":"old","known":true})),
        );
        assert_eq!(d.name, "old", "a Profile record keeps its name");
        let fifth = d.begin_check();
        d.finish_check(fifth, Ok(json!({"ok":false,"reason":"not_git_root"})));
        assert_eq!(d.check, Check::Refused("not_git_root".into()));
        assert_eq!(d.ready(), None);
        d.set_name("../x".into());
        assert!(!valid_name(&d.name));
    }
    fn entry() -> crate::ProjectEntry {
        crate::ProjectEntry {
            name: "alpha".into(),
            label: None,
            root: "/home/u/alpha".into(),
            state: "unavailable".into(),
            agents: true,
            propose: false,
            gate: false,
        }
    }
    #[test]
    fn an_edit_saves_a_changed_display_name_and_only_a_checked_new_folder() {
        let mut d = EditDraft::open(&entry());
        assert_eq!(d.path, "/home/u/alpha");
        assert!(!d.ready(), "nothing changed");
        // The display name: trimmed; empty or the name itself shows the name.
        d.set_label("  Alpha · 主线 ".into());
        assert_eq!(d.label_change(), Some(Some("Alpha · 主线".into())));
        assert!(d.ready());
        d.set_label("alpha".into());
        assert_eq!(d.label_change(), None);
        d.set_label("x\ny".into());
        assert!(!d.ready(), "control characters");
        d.set_label(String::new());
        // A moved folder must be checked before it can be saved.
        d.set_path("/home/u/precept".into());
        assert!(d.moving() && !d.ready());
        let epoch = d.begin_check();
        d.finish_check(
            epoch,
            Ok(json!({"ok":true,"root":"/home/u/precept","name":"precept","known":false})),
        );
        assert_eq!(d.relocation().as_deref(), Some("/home/u/precept"));
        assert!(d.ready());
        // A stale reply after another edit is ignored.
        d.set_path("/home/u/other".into());
        d.finish_check(
            epoch,
            Ok(json!({"ok":true,"root":"/home/u/other","name":"other","known":false})),
        );
        assert_eq!(d.check, Check::Idle);
        assert!(!d.ready());
        // A folder the Profile records for another project is not a move.
        let epoch = d.begin_check();
        d.finish_check(
            epoch,
            Ok(json!({"ok":true,"root":"/home/u/other","name":"beta","known":true})),
        );
        assert!(d.taken() && d.relocation().is_none() && !d.ready());
        // Its own earlier record there is fine.
        let epoch = d.begin_check();
        d.finish_check(
            epoch,
            Ok(json!({"ok":true,"root":"/home/u/other","name":"alpha","known":true})),
        );
        assert!(d.ready());
        // A refusal blocks the save; the folder back as it was saves the label alone.
        let epoch = d.begin_check();
        d.finish_check(epoch, Ok(json!({"ok":false,"reason":"not_git_root"})));
        assert!(!d.ready());
        d.set_path("/home/u/alpha".into());
        d.set_label("Alpha".into());
        assert!(d.ready() && d.relocation().is_none());
        // Clearing a saved display name is a change.
        let mut labelled = entry();
        labelled.label = Some("Alpha".into());
        let mut d = EditDraft::open(&labelled);
        assert_eq!(d.label, "Alpha");
        d.set_label(" ".into());
        assert_eq!(d.label_change(), Some(None));
        assert!(d.ready());
    }
    #[test]
    fn display_names_follow_the_daemons_rule() {
        assert!(valid_label("Precept · 主线"));
        for bad in ["", " a", "a ", "a\tb", &"x".repeat(65)] {
            assert!(!valid_label(bad), "{bad:?}");
        }
        assert!(valid_label(&"字".repeat(64)));
    }
}
