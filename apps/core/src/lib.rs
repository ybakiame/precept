//! Operator app state, presentation and i18n. Pure: no I/O, framework, SQLite or
//! transport; native IPC and the WSL bridge live in `precept-ui-client`.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};

mod model;
#[cfg(test)]
mod probe_tests;
#[cfg(test)]
mod target_tests;
#[cfg(test)]
mod tests;

pub mod activity;
pub mod appearance;
pub mod connection;
pub mod doctor;
pub mod i18n;
pub mod keys;
pub mod mcp;
pub mod memory;
pub mod operator;
pub mod personal_sync;
pub mod presentation;
pub mod profile;
pub mod projects;
pub mod rule_lint;
pub mod rule_templates;
pub mod settings;
pub mod tag_colors;
pub mod work;

pub const PAGE_SIZE: usize = 12;
/// Tags offered before any are in use: common kinds of working preference.
pub const STARTER_TAGS: &[&str] = &[
    "style", "testing", "git", "tooling", "docs", "security", "language", "review",
];
pub const MAX_ROWS: usize = 10_000;
pub fn valid_task(task: &str) -> bool {
    !task.trim().is_empty() && task.chars().count() <= 120 && !task.chars().any(char::is_control)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Health {
    pub api_version: u32,
    pub service: String,
    pub ready: bool,
    pub schema: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Preference {
    pub key: String,
    pub value: String,
    pub scope: String,
    /// Canonical tags of the effective record (a short classification the
    /// operator gave it; agents receive them too).
    #[serde(default)]
    pub tags: Vec<String>,
    pub status: String,
    /// Where it applies (absent = everywhere).
    #[serde(default)]
    pub applies_to: Option<precept_core::operator::AppliesTo>,
    #[serde(default)]
    pub priority: Option<i64>,
    /// `verify` or `hard` (absent = advisory).
    #[serde(default)]
    pub enforcement: Option<String>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Repository {
    pub head: Option<String>,
    pub branch: Option<String>,
    pub dirty: Option<bool>,
    pub change_count: Option<u64>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Context {
    pub project: String,
    pub preferences: Vec<Preference>,
    pub repository: Repository,
    pub documents: Vec<Value>,
    pub warnings: Vec<String>,
    /// Committed `.precept/rules` files that apply, with their check results.
    #[serde(default)]
    pub repository_rules: Vec<RepositoryRule>,
    /// Rule files not committed yet (id, key, file).
    #[serde(default)]
    pub pending_rules: Vec<Value>,
    /// `status` and, for an invalid source, `invalid_files`.
    #[serde(default)]
    pub contract_resolution: Value,
}
/// A committed repository rule as agents receive it.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct RepositoryRule {
    pub id: String,
    pub key: String,
    pub text: String,
    pub file: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub enforcement: Option<String>,
    #[serde(default)]
    pub allowed_values: Vec<String>,
    #[serde(default)]
    pub applies_to: Option<Value>,
    /// `status` (verified, failing, unchecked, unavailable), `passed`,
    /// `total` and the first `failure`.
    #[serde(default)]
    pub checks: Value,
    /// How many tool guards the rule declares.
    #[serde(default)]
    pub guards: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Handoff {
    pub project: String,
    pub task_id: String,
    pub objective: String,
    pub next_action: String,
    pub validation: Vec<Value>,
    pub resume_warnings: Vec<String>,
    pub current_repository: Value,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    pub context: Context,
    pub handoff: Option<Handoff>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Snapshot {
    pub health: Health,
    pub projects: Vec<Project>,
    /// The daemon's whole project set (`project.list`), including projects
    /// not served yet (`unregistered`) or any more (`unavailable`).
    #[serde(default)]
    pub directory: Vec<ProjectEntry>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectEntry {
    pub name: String,
    /// The operator's display name (`project.label`); `None` shows the name.
    #[serde(default)]
    pub label: Option<String>,
    pub root: String,
    pub state: String,
    /// Shared with external agents (MCP); the daemon always reports it.
    #[serde(default)]
    pub agents: bool,
    /// Agents may suggest rules for it (`suggest_rule`).
    #[serde(default)]
    pub propose: bool,
    /// The pre-tool gate answers for its hard rules (`project.gate`).
    #[serde(default)]
    pub gate: bool,
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), &'static str> {
        let mut seen = std::collections::HashSet::new();
        if self.health.api_version != 2
            || !self.health.ready
            || self.projects.len() > 256
            || self.directory.len() > 256
            || self.directory.iter().any(|e| {
                e.label
                    .as_deref()
                    .is_some_and(|l| !projects::valid_label(l))
            })
            || serde_json::to_vec(self).map_or(true, |v| v.len() > 4 * 1024 * 1024)
        {
            return Err("Unsupported snapshot");
        }
        for p in &self.projects {
            if !seen.insert(&p.name)
                || p.context.preferences.len() > MAX_ROWS
                || p.name != p.context.project
                || p.handoff.as_ref().is_some_and(|h| h.project != p.name)
                || p.context
                    .preferences
                    .iter()
                    .any(|r| r.status != "confirmed")
            {
                return Err("Invalid project scope or unconfirmed preference");
            }
        }
        Ok(())
    }
    pub fn fixture() -> Self {
        let data: Self = serde_json::from_str(include_str!("../fixtures/dashboard.json")).unwrap();
        data.validate().unwrap();
        data
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Loading,
    Ready,
    Empty,
    Error,
    Unavailable,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ReadRequest {
    pub epoch: u64,
    pub project: Option<String>,
    pub task: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Dashboard,
    Projects,
    /// Preferences & contracts: what agents receive for the project.
    Rules,
    Handoff,
    Diagnostics,
}
impl Section {
    pub const ALL: [Self; 5] = [
        Self::Dashboard,
        Self::Projects,
        Self::Rules,
        Self::Handoff,
        Self::Diagnostics,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Projects => "Projects",
            Self::Rules => "Preferences & contracts",
            Self::Handoff => "Handoff",
            Self::Diagnostics => "Diagnostics",
        }
    }
}
/// Connection/Profile in Runtime checks, or the separate Workspaces page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SetupTab {
    #[default]
    Connection,
    Profile,
    Workspaces,
}

/// The independent first-launch guide. Agent access is optional; the first
/// three steps must be observed complete before the guide can finish.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OnboardingStep {
    #[default]
    Connection,
    Profile,
    Workspace,
    Agents,
    Complete,
}

/// Connection discovery on the connection page. Each check carries the epoch
/// it was started with; a result for an older selection is dropped.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Probe {
    #[default]
    Idle,
    Checking,
    /// The explicit `systemctl --user start` is in flight.
    Starting,
    Found(connection::Discovery),
    Failed(&'static str),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub operator: operator::Operator,
    pub epoch: u64,
    pub connection_epoch: u64,
    pub profile_epoch: u64,
    pub profile_read_epoch: u64,
    pub connection: connection::Connection,
    pub connection_draft: connection::Connection,
    pub connection_invalid: bool,
    pub settings: bool,
    /// The Runtime checks page (Connection and Profile), or Workspaces.
    pub setup_open: bool,
    pub setup_tab: SetupTab,
    /// The independent first-launch guide; presentation state only.
    pub onboarding_open: bool,
    pub onboarding_step: OnboardingStep,
    /// Whether the operator completed or skipped the launch guide. Saved with
    /// presentation settings so a skipped guide does not return next launch.
    pub onboarding_seen: bool,
    /// Profile-scoped manual personal sync.
    pub sync_open: bool,
    pub sync: personal_sync::State,
    /// The project-scoped Agents (MCP) page.
    pub agents_page: bool,
    pub agents: mcp::AgentsPage,
    /// The project-scoped Sessions page (agents' work sessions).
    pub sessions_page: bool,
    pub sessions: work::SessionsPage,
    /// The project-scoped Memory page (agents' project memory).
    pub memory_page: bool,
    pub memory: memory::MemoryPage,
    /// The selected project's saved Handoffs (`profile.handoff.list`), with
    /// the project they were read for.
    pub handoff_list: Option<(String, Vec<precept_core::operator::HandoffSummary>)>,
    /// Registered WSL distributions (`None` until listed on this page).
    pub distributions: Option<Result<Vec<String>, &'static str>>,
    pub probe: Probe,
    pub probe_epoch: u64,
    /// The manual fields of the connection page are shown.
    pub connection_advanced: bool,
    /// Workspace page: adding a project, and the project armed for removal.
    pub project_add: projects::AddDraft,
    pub project_remove: Option<String>,
    pub project_edit: Option<projects::EditDraft>,
    pub profile_setup: profile::ProfileDraft,
    /// The Profile state the Set-up pages and Workspaces read. Kept apart from
    /// `operator.info`, which fences the review workspace's own requests: a
    /// write there would make an in-flight workspace reply stale (stuck wait).
    pub profile_info: profile::Observed,
    /// Suggestions waiting in the personal scope plus the selected project's,
    /// as last counted by the daemon. `None`: not counted (or the daemon could
    /// not say), which is never shown as zero.
    pub review_count: Option<i64>,
    /// The operator's shortcut overrides (`settings.toml`).
    pub keymap: keys::Keymap,
    /// The Shortcuts panel is open.
    pub shortcuts_open: bool,
    /// The shortcut whose new keystroke is being recorded; no shortcut is
    /// bound meanwhile.
    pub shortcut_recording: Option<&'static str>,
    /// Why the last applied connection could not be saved for the next launch.
    pub save_failure: Option<&'static str>,
    pub theme: appearance::ThemeId,
    pub color_mode: appearance::ColorMode,
    pub locale: i18n::Locale,
    pub last_duration_ms: Option<u64>,
    pub project_names: Vec<String>,
    pub last_read_ms: Option<u64>,
    pub section: Section,
    pub high_contrast: bool,
    /// The sidebar is hidden (desktop; the operator's choice, saved).
    pub sidebar_collapsed: bool,
    pub selected_preference: Option<usize>,
    /// The repository rule whose details are open (index into the project's rules).
    pub selected_rule: Option<usize>,
    /// Operator-only repository diagnostics and the copy-only template dialog.
    pub rule_lint: rule_lint::RuleLintState,
    pub detail_page: usize,
    pub copied: bool,
    pub data: Option<Arc<Snapshot>>,
    pub phase: Phase,
    pub selected: usize,
    pub dark: bool,
    pub detail_focus: bool,
    pub filter: String,
    /// Tags the Preferences page narrows to; a row must carry all of them.
    pub tag_filter: Vec<String>,
    /// Tag name → chosen color (see [`tag_colors`]), saved in `settings.toml`.
    pub tag_colors: BTreeMap<String, String>,
    /// The tag-colors panel is open, and the tag whose picker is showing.
    pub tag_colors_open: bool,
    pub tag_editing: Option<String>,
    pub live: bool,
    pub task_id: String,
    pub page: usize,
    pub permission_denied: bool,
    /// Reason of the last failed read (a fixed string, e.g. a `wsl_status` value).
    pub read_failure: Option<&'static str>,
    /// Live WSL link problem observed by the GUI watcher (`None` = healthy or not
    /// yet connected). Display only: never part of an Operator guard, so it can
    /// not invalidate an uncertain request's identical retry.
    pub wsl_link: Option<&'static str>,
    pub queried_task: String,
    pub selected_project: Option<String>,
}
