//! Operator app state, presentation and i18n. Pure: no I/O, framework, SQLite or
//! transport; native IPC and the WSL bridge live in `acp-operator-client`.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

pub mod appearance;
pub mod connection;
pub mod i18n;
pub mod operator;
pub mod presentation;
pub mod session;

pub const PAGE_SIZE: usize = 12;
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
    pub source: String,
    pub status: String,
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
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), &'static str> {
        let mut seen = std::collections::HashSet::new();
        if self.health.api_version != 2
            || !self.health.ready
            || self.projects.len() > 64
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
    Preferences,
    Handoff,
    Diagnostics,
}
impl Section {
    pub const ALL: [Self; 5] = [
        Self::Dashboard,
        Self::Projects,
        Self::Preferences,
        Self::Handoff,
        Self::Diagnostics,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Projects => "Projects",
            Self::Preferences => "Preferences",
            Self::Handoff => "Handoff",
            Self::Diagnostics => "Diagnostics",
        }
    }
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
    pub sessions: bool,
    pub session: session::SessionModel,
    pub connection: connection::Connection,
    pub connection_draft: connection::Connection,
    pub connection_invalid: bool,
    pub settings: bool,
    pub connection_editor: bool,
    /// Registered WSL distributions (`None` until listed on this page).
    pub distributions: Option<Result<Vec<String>, &'static str>>,
    pub probe: Probe,
    pub probe_epoch: u64,
    /// The manual fields of the connection page are shown.
    pub connection_advanced: bool,
    /// Why the last applied connection could not be saved for the next launch.
    pub save_failure: Option<&'static str>,
    pub theme: appearance::ThemeId,
    pub color_mode: appearance::ColorMode,
    pub locale: i18n::Locale,
    pub last_duration_ms: Option<u64>,
    pub project_names: Vec<String>,
    pub last_read_ms: Option<u64>,
    pub section: Section,
    pub expanded: bool,
    pub evidence_page: usize,
    pub high_contrast: bool,
    pub selected_preference: Option<usize>,
    pub detail_page: usize,
    pub copied: bool,
    pub data: Option<Arc<Snapshot>>,
    pub phase: Phase,
    pub selected: usize,
    pub dark: bool,
    pub detail_focus: bool,
    pub filter: String,
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
impl Model {
    pub fn loading() -> Self {
        Self {
            operator: Default::default(),
            epoch: 0,
            sessions: false,
            session: Default::default(),
            connection: Default::default(),
            connection_draft: Default::default(),
            connection_invalid: false,
            settings: false,
            connection_editor: false,
            distributions: None,
            probe: Probe::Idle,
            probe_epoch: 0,
            connection_advanced: false,
            save_failure: None,
            theme: Default::default(),
            color_mode: appearance::ColorMode::Dark,
            locale: Default::default(),
            last_duration_ms: None,
            project_names: Vec::new(),
            last_read_ms: None,
            section: Section::Dashboard,
            expanded: false,
            evidence_page: 0,
            high_contrast: false,
            selected_preference: None,
            detail_page: 0,
            copied: false,
            data: None,
            phase: Phase::Loading,
            selected: 0,
            dark: false,
            detail_focus: false,
            filter: String::new(),
            live: false,
            task_id: String::new(),
            page: 0,
            permission_denied: false,
            read_failure: None,
            wsl_link: None,
            queried_task: String::new(),
            selected_project: None,
        }
    }
    pub fn fixture() -> Self {
        let mut m = Self::loading();
        m.receive(Ok(Snapshot::fixture()));
        m.queried_task = m
            .project()
            .and_then(|p| p.handoff.as_ref())
            .map(|h| h.task_id.clone())
            .unwrap_or_default();
        m
    }
    pub fn target_label(&self) -> &'static str {
        self.word(match self.connection.target {
            connection::Target::Synthetic => i18n::Msg::SyntheticTarget,
            connection::Target::Local => i18n::Msg::LocalTarget,
            connection::Target::Wsl => i18n::Msg::WslTarget,
        })
    }
    pub fn word(&self, key: i18n::Msg) -> &'static str {
        i18n::tr(self.locale, key)
    }
    /// A target transition invalidates pending reads, scope, evidence and timestamps together.
    /// Starts a discovery check (or the explicit daemon start) for the current
    /// draft; returns the epoch its result must carry.
    pub fn begin_probe(&mut self, starting: bool) -> u64 {
        self.probe_epoch = self.probe_epoch.wrapping_add(1);
        self.probe = if starting {
            Probe::Starting
        } else {
            Probe::Checking
        };
        self.probe_epoch
    }
    /// A discovery result. Ignored if a newer check started since; a valid
    /// report fills the draft's coordinates for its target.
    pub fn finish_probe(
        &mut self,
        epoch: u64,
        result: Result<connection::Discovery, &'static str>,
    ) {
        if epoch != self.probe_epoch {
            return;
        }
        let wsl = self.connection_draft.target == connection::Target::Wsl;
        self.probe = match result {
            Ok(d) if d.validate(wsl) => {
                if wsl {
                    let distribution = self.connection_draft.distribution.clone();
                    d.fill_wsl(&mut self.connection_draft, &distribution);
                } else {
                    d.fill_local(&mut self.connection_draft);
                }
                self.connection_invalid = false;
                Probe::Found(d)
            }
            Ok(_) => Probe::Failed(connection::discovery_status::INVALID_REPORT),
            Err(e) => Probe::Failed(e),
        };
    }
    /// A manual edit of the draft. An in-flight check can no longer overwrite it.
    pub fn edit_draft(&mut self, edit: impl FnOnce(&mut connection::Connection)) {
        edit(&mut self.connection_draft);
        self.connection_invalid = false;
        if matches!(self.probe, Probe::Checking) {
            self.probe_epoch = self.probe_epoch.wrapping_add(1);
            self.probe = Probe::Idle;
        }
    }
    /// Selecting another target or distribution forgets the previous check.
    pub fn select_target(&mut self, target: connection::Target, distribution: Option<&str>) {
        self.connection_draft.target = target;
        if let Some(d) = distribution {
            self.connection_draft.distribution = d.into();
        }
        self.connection_invalid = false;
        self.probe_epoch = self.probe_epoch.wrapping_add(1);
        self.probe = Probe::Idle;
    }
    pub fn apply_connection(&mut self) -> bool {
        if !self.connection_draft.validate() {
            self.connection_invalid = true;
            return false;
        }
        self.epoch = self.epoch.checked_add(1).expect("request epoch exhausted");
        self.session = Default::default();
        self.operator.invalidate();
        self.connection = self.connection_draft.clone();
        self.connection_invalid = false;
        self.live = self.connection.target != connection::Target::Synthetic;
        self.data = None;
        self.project_names.clear();
        self.selected_project = None;
        self.selected = 0;
        self.task_id.clear();
        self.queried_task.clear();
        self.filter.clear();
        self.page = 0;
        self.evidence_page = 0;
        self.detail_page = 0;
        self.selected_preference = None;
        self.expanded = false;
        self.copied = false;
        self.last_read_ms = None;
        self.last_duration_ms = None;
        self.permission_denied = false;
        self.phase = Phase::Loading;
        true
    }
    /// Invalidating a request also invalidates every older in-flight completion.
    pub fn begin_read(&mut self) -> Result<ReadRequest, &'static str> {
        self.epoch = self.epoch.checked_add(1).expect("request epoch exhausted");
        self.session = Default::default();
        self.operator.invalidate();
        self.data = None;
        self.selected_preference = None;
        self.copied = false;
        self.permission_denied = false;
        self.read_failure = None;
        if !self.task_id.is_empty() && !valid_task(&self.task_id) {
            self.phase = Phase::Error;
            return Err("Invalid task ID");
        }
        self.phase = Phase::Loading;
        self.queried_task = self.task_id.clone();
        Ok(ReadRequest {
            epoch: self.epoch,
            project: self.selected_project.clone(),
            task: self.queried_task.clone(),
        })
    }
    pub fn complete_read(
        &mut self,
        request: &ReadRequest,
        result: Result<Snapshot, &'static str>,
        now_ms: u64,
    ) -> bool {
        if request.epoch != self.epoch
            || request.project != self.selected_project
            || request.task != self.queried_task
        {
            return false;
        }
        let result = result.and_then(|snapshot| {
            snapshot.validate()?;
            let project = if let Some(name) = &request.project {
                Some(
                    snapshot
                        .projects
                        .iter()
                        .find(|p| &p.name == name)
                        .ok_or("Permission denied")?,
                )
            } else {
                snapshot.projects.first()
            };
            if project
                .and_then(|p| p.handoff.as_ref())
                .is_some_and(|h| h.task_id != request.task)
            {
                return Err("Unexpected task response");
            }
            Ok(snapshot)
        });
        self.receive(result);
        if matches!(self.phase, Phase::Ready | Phase::Empty) {
            if let Some(name) = &request.project
                && let Some(index) = self.project_names.iter().position(|p| p == name)
            {
                self.select(index);
            }
            // Bind the initially chosen allowlisted project for later reads;
            // a daemon list reordering must not silently move an exact task lookup.
            if self.phase == Phase::Ready && request.project.is_none() {
                self.select(self.selected);
            }
            if self.live {
                self.last_read_ms = Some(now_ms);
            }
        }
        true
    }
    pub fn receive(&mut self, result: Result<Snapshot, &'static str>) {
        self.permission_denied = result.as_ref().is_err_and(|e| *e == "Permission denied");
        self.read_failure = result.as_ref().err().copied();
        if result.is_ok() {
            self.wsl_link = None;
        }
        match result {
            Ok(s) if s.validate().is_ok() => {
                self.project_names = s.projects.iter().map(|p| p.name.clone()).collect();
                self.selected = self.selected.min(s.projects.len().saturating_sub(1));
                self.phase = if s.projects.is_empty() {
                    Phase::Empty
                } else {
                    Phase::Ready
                };
                self.data = Some(Arc::new(s));
                self.page = 0;
            }
            _ => {
                self.data = None;
                self.project_names.clear();
                self.phase = Phase::Unavailable;
            }
        }
    }
    pub fn project(&self) -> Option<&Project> {
        if self.phase != Phase::Ready {
            return None;
        }
        self.data.as_ref()?.projects.get(self.selected)
    }
    pub fn select(&mut self, index: usize) {
        if let Some(name) = self.project_names.get(index).cloned() {
            self.session = Default::default();
            self.operator.invalidate();
            self.selected = index;
            self.selected_project = Some(name);
            self.page = 0;
            self.selected_preference = None;
            self.detail_page = 0;
            self.evidence_page = 0;
            self.expanded = false;
        }
    }
    pub fn next(&mut self) {
        if !self.project_names.is_empty() {
            self.select((self.selected + 1) % self.project_names.len());
        }
    }
    pub fn rows(&self) -> Vec<&Preference> {
        let needle = self.filter.to_lowercase();
        self.project()
            .map(|p| {
                p.context
                    .preferences
                    .iter()
                    .filter(|r| {
                        self.filter.is_empty()
                            || [&r.key, &r.value, &r.scope, &r.source]
                                .iter()
                                .any(|v| v.to_lowercase().contains(&needle))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn status(&self) -> String {
        if self.locale == i18n::Locale::ZhCn {
            return format!(
                "{} · {} · {}",
                self.source_label(),
                self.read_label(),
                self.read_time()
            );
        }
        let source = if self.live {
            "LIVE_READ_ONLY"
        } else {
            "OFFLINE_SYNTHETIC"
        };
        let history = self
            .last_read_ms
            .map(|t| {
                format!(
                    " · Last successful read {:02}:{:02}:{:02} UTC",
                    (t / 3_600_000) % 24,
                    (t / 60_000) % 60,
                    (t / 1000) % 60
                )
            })
            .unwrap_or_default();
        presentation::sanitize(
            &format!("{source} · {}{history}", self.connection_status()),
            512,
        )
    }
    /// For a WSL target: the live link problem, else the last read failure.
    pub fn wsl_reason(&self) -> Option<i18n::Msg> {
        if self.connection.target != connection::Target::Wsl {
            return None;
        }
        self.wsl_link
            .or(if self.phase == Phase::Unavailable {
                self.read_failure
            } else {
                None
            })
            .and_then(connection::wsl_status::message)
    }
    fn connection_status(&self) -> String {
        match self.phase {
            Phase::Loading => "Loading — read-only query in progress".into(),
            Phase::Empty => "Empty — no allowlisted projects".into(),
            Phase::Error => {
                "Error — invalid task ID; use 1–120 characters without control characters".into()
            }
            Phase::Unavailable => {
                if let Some(msg) = self.wsl_reason() {
                    format!("Unavailable — {}", self.word(msg))
                } else if self.permission_denied {
                    "Permission denied — check the private operator capability; reconnect manually"
                        .into()
                } else {
                    "Unavailable — local daemon refused access or could not be reached".into()
                }
            }
            Phase::Ready => match &self.data {
                Some(d) if d.health.ready => format!(
                    "{} · {} · schema {} · read-only",
                    if self.live {
                        "Last read succeeded (Reconnect refreshes)"
                    } else {
                        "Offline synthetic snapshot (captured health)"
                    },
                    d.health.service,
                    d.health.schema
                ),
                _ => "Not ready".into(),
            },
        }
    }
    pub fn set_filter(&mut self, value: &str) {
        self.filter = value
            .chars()
            .filter(|c| !c.is_control())
            .take(120)
            .collect();
        self.page = 0;
        self.selected_preference = None;
        self.detail_page = 0;
    }
    pub fn page_by(&mut self, delta: isize) {
        let last = self.rows().len().saturating_sub(1) / PAGE_SIZE;
        self.page = self.page.saturating_add_signed(delta).min(last);
        self.selected_preference = None;
        self.detail_page = 0;
    }
    pub fn page_label(&self) -> String {
        let count = self.rows().len();
        if self.locale == i18n::Locale::ZhCn {
            return format!(
                "{} 条已确认记录 · 第 {}/{} 页",
                count,
                self.page + 1,
                count.div_ceil(PAGE_SIZE).max(1)
            );
        }
        format!(
            "{} confirmed rows · page {}/{}",
            count,
            self.page + 1,
            count.div_ceil(PAGE_SIZE).max(1)
        )
    }
    pub fn cancel_read(&mut self) {
        if self.phase == Phase::Loading {
            self.epoch += 1;
            self.phase = Phase::Unavailable;
            self.data = None;
        }
    }
    pub fn select_preference(&mut self, index: usize) {
        self.selected_preference = (index < self.rows().len()).then_some(index);
        self.detail_page = 0;
        self.copied = false;
    }
    pub fn preference_detail(&self) -> Option<&Preference> {
        self.rows().get(self.selected_preference?).copied()
    }
    pub fn detail_chunk(&self) -> String {
        self.preference_detail()
            .map(|row| {
                presentation::sanitize(
                    &row.value
                        .chars()
                        .skip(self.detail_page * 512)
                        .take(512)
                        .collect::<String>(),
                    512,
                )
            })
            .unwrap_or_default()
    }
    pub fn detail_pages(&self) -> usize {
        self.preference_detail()
            .map_or(1, |p| p.value.chars().count().div_ceil(512).max(1))
    }
    pub fn detail_by(&mut self, delta: isize) {
        self.detail_page = self
            .detail_page
            .saturating_add_signed(delta)
            .min(self.detail_pages() - 1);
    }
    /// Compatibility entry point for the pinned optional desktop benchmark.
    pub fn detail_lines(&self) -> Vec<String> {
        self.visible_lines()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancel_invalidates_pending_results_and_detail_follows_visible_scope() {
        let mut m = Model::fixture();
        let pending = m.begin_read().unwrap();
        m.cancel_read();
        assert_eq!(m.phase, Phase::Unavailable);
        assert!(!m.complete_read(&pending, Ok(Snapshot::fixture()), 1));
        assert!(m.project().is_none());
        let mut snapshot = Snapshot::fixture();
        snapshot.projects[0].context.preferences[0].value = "中文\u{1b}".repeat(600);
        m.receive(Ok(snapshot));
        m.select_preference(0);
        assert!(m.detail_chunk().chars().count() <= 512);
        assert!(!m.detail_chunk().contains('\u{1b}'));
        m.detail_by(1000);
        assert_eq!(m.detail_page + 1, m.detail_pages());
        m.set_filter("absent");
        assert!(m.preference_detail().is_none());
        m.set_filter("");
        m.select_preference(0);
        m.select(1);
        assert!(m.preference_detail().is_none());
        assert_eq!(m.detail_page, 0);
    }
    #[test]
    fn delayed_responses_cannot_restore_scope_or_revoked_authority() {
        let mut fixture = Snapshot::fixture();
        for p in &mut fixture.projects {
            p.handoff = None;
        }
        let mut m = Model::fixture();
        m.live = true;
        m.select(0);
        let alpha = m.begin_read().unwrap();
        m.select(1);
        let beta = m.begin_read().unwrap();
        assert!(m.project().is_none());
        assert!(!m.complete_read(&alpha, Ok(fixture.clone()), 10));
        assert!(m.complete_read(&beta, Ok(fixture.clone()), 20));
        assert_eq!(m.project().unwrap().name, "beta");
        assert_eq!(m.last_read_ms, Some(20));
        let old = m.begin_read().unwrap();
        let newest = m.begin_read().unwrap();
        assert!(m.complete_read(&newest, Err("Permission denied"), 30));
        assert!(!m.complete_read(&old, Ok(fixture.clone()), 40));
        assert!(m.permission_denied && m.project().is_none());
        assert_eq!(m.last_read_ms, Some(20));
        let restarting = m.begin_read().unwrap();
        m.task_id = "invalid\n".into();
        assert!(m.begin_read().is_err());
        assert!(!m.complete_read(&restarting, Ok(fixture.clone()), 50));
        assert_eq!(m.phase, Phase::Error);
        m.task_id.clear();
        let restarted = m.begin_read().unwrap();
        assert!(m.complete_read(&restarted, Ok(fixture.clone()), 60));
        assert_eq!(m.project().unwrap().name, "beta");
    }
    #[test]
    fn rapid_exact_task_lookups_ignore_older_success_and_error() {
        let mut m = Model::fixture();
        m.select(0);
        m.live = true;
        m.task_id = "older".into();
        let older = m.begin_read().unwrap();
        m.task_id = "newest".into();
        let newest = m.begin_read().unwrap();
        let mut snapshot = Snapshot::fixture();
        snapshot.projects[0].handoff.as_mut().unwrap().task_id = "newest".into();
        assert!(m.complete_read(&newest, Ok(snapshot), 1));
        assert!(!m.complete_read(&older, Err("Permission denied"), 2));
        assert!(!m.complete_read(&older, Ok(Snapshot::fixture()), 3));
        assert_eq!(m.queried_task, "newest");
        assert_eq!(
            m.project().unwrap().handoff.as_ref().unwrap().task_id,
            "newest"
        );
        assert!(!m.permission_denied);
    }
    #[test]
    fn unexpected_scope_task_and_not_ready_fail_closed() {
        let mut m = Model::fixture();
        m.live = true;
        m.select(0);
        m.task_id = "missing".into();
        let request = m.begin_read().unwrap();
        assert!(m.complete_read(&request, Ok(Snapshot::fixture()), 1));
        assert!(m.project().is_none());
        let request = m.begin_read().unwrap();
        let mut snapshot = Snapshot::fixture();
        snapshot.projects.remove(0);
        m.complete_read(&request, Ok(snapshot), 2);
        assert!(m.permission_denied && m.data.is_none());
        let mut snapshot = Snapshot::fixture();
        snapshot.health.ready = false;
        assert!(snapshot.validate().is_err());
    }
    #[test]
    fn pages_bound_historical_evidence_and_sanitize_display_and_copy() {
        let mut m = Model::fixture();
        let mut snapshot = Snapshot::fixture();
        let h = snapshot.projects[0].handoff.as_mut().unwrap();
        h.validation = (0..100)
            .map(|i| serde_json::json!({"index":i,"text":"x".repeat(1000)}))
            .collect();
        h.objective = "unsafe\u{1b}[2J".into();
        m.receive(Ok(snapshot));
        m.set_section(Section::Handoff);
        assert!(!m.visible_lines().join("\n").contains("\u{1b}"));
        assert!(!m.visible_lines().join("\n").contains("\"index\""));
        m.expanded = true;
        m.evidence_by(isize::MAX);
        assert_eq!(m.evidence_page, 16);
        assert!(m.visible_lines().len() <= 16);
        assert!(
            m.visible_lines()
                .iter()
                .all(|line| line.chars().count() < 550)
        );
        m.set_section(Section::Diagnostics);
        assert!(!m.expanded);
        assert_eq!(m.evidence_page, 0);
        for section in Section::ALL {
            m.set_section(section);
            assert!(m.visible_lines()[0].contains("alpha"));
        }
    }
    #[test]
    fn states_selection_filter_and_unknown_evidence() {
        let mut m = Model::fixture();
        assert_eq!(m.phase, Phase::Ready);
        assert!(
            m.detail_lines()
                .iter()
                .any(|s| s.contains("Fingerprint: Unavailable"))
        );
        m.next();
        assert_eq!(m.project().unwrap().name, "beta");
        m.select(99);
        assert_eq!(m.selected, 1);
        m.filter = "never-match".into();
        assert!(m.rows().is_empty());
        m.receive(Err("synthetic"));
        assert_eq!(m.phase, Phase::Unavailable);
        assert!(m.project().is_none());
        let mut empty = Snapshot::fixture();
        empty.projects.clear();
        m.receive(Ok(empty));
        assert_eq!(m.phase, Phase::Empty);
        m.phase = Phase::Error;
        assert!(m.status().contains("Error"));
        assert!(Model::loading().status().contains("Loading"));
    }
    #[test]
    fn rejects_cross_project_and_unconfirmed_rows() {
        let mut s = Snapshot::fixture();
        s.projects[0].context.project = "beta".into();
        assert!(s.validate().is_err());
        for status in ["suggested", "rejected", "deprecated"] {
            let mut s = Snapshot::fixture();
            s.projects[0].context.preferences[0].status = status.into();
            assert!(s.validate().is_err());
        }
    }
    #[test]
    fn ten_thousand_rows_filter_and_selection() {
        let mut s = Snapshot::fixture();
        let row = s.projects[0].context.preferences[0].clone();
        s.projects[0].context.preferences = (0..10_000)
            .map(|i| Preference {
                key: format!("event-{i}"),
                ..row.clone()
            })
            .collect();
        let mut m = Model::loading();
        m.receive(Ok(s));
        m.filter = "event-9999".into();
        assert_eq!(m.rows().len(), 1);
        m.next();
        assert_eq!(m.project().unwrap().name, "beta");
    }
    #[test]
    fn input_bounds_pages_and_permission_fail_closed() {
        for invalid in ["", " ", "task\nnext", "\u{1b}[2J"] {
            assert!(!valid_task(invalid));
        }
        assert!(!valid_task(&"x".repeat(121)));
        assert!(valid_task("task / 'quoted' 中文"));
        let mut m = Model::fixture();
        m.page_by(isize::MAX);
        assert_eq!(m.page, 0);
        m.set_filter(&"x".repeat(1000));
        assert_eq!(m.filter.len(), 120);
        m.receive(Err("Permission denied"));
        assert!(m.data.is_none());
        assert!(m.status().contains("Permission denied"));
        m.receive(Ok(Snapshot::fixture()));
        assert!(!m.permission_denied);
        m.phase = Phase::Loading;
        assert!(m.project().is_none());
        assert!(m.rows().is_empty());
        let mut s = Snapshot::fixture();
        let row = s.projects[0].context.preferences[0].clone();
        s.projects[0].context.preferences = vec![row; MAX_ROWS + 1];
        assert!(s.validate().is_err());
    }
}

#[cfg(test)]
mod probe_tests {
    use super::*;
    use connection::{DISCOVERY_VERSION, DaemonStatus, Discovery, Target};
    fn report() -> Discovery {
        Discovery {
            version: DISCOVERY_VERSION,
            user: "operator".into(),
            helper: "/usr/bin/acp-wsl-bridge".into(),
            runtime: "/home/operator/.local/state/acp/runtime".into(),
            endpoint: "/home/operator/.local/state/acp/runtime/control.sock".into(),
            systemd: true,
            unit_installed: true,
            daemon: DaemonStatus::Ready {
                profile_selected: false,
            },
        }
    }
    #[test]
    fn stale_or_superseded_checks_never_overwrite_the_draft() {
        let mut m = Model::loading();
        m.select_target(Target::Wsl, Some("archlinux"));
        let first = m.begin_probe(false);
        // Another distribution is chosen before the first result arrives.
        m.select_target(Target::Wsl, Some("Ubuntu-24.04"));
        let second = m.begin_probe(false);
        m.finish_probe(first, Ok(report()));
        assert_eq!(m.probe, Probe::Checking);
        assert!(m.connection_draft.wsl_runtime.is_empty());
        m.finish_probe(second, Ok(report()));
        assert!(matches!(m.probe, Probe::Found(_)));
        assert_eq!(m.connection_draft.distribution, "Ubuntu-24.04");
        assert!(m.connection_draft.validate());
        // A manual edit during a check wins over its late result.
        let third = m.begin_probe(false);
        m.edit_draft(|d| d.wsl_runtime = "/srv/acp/runtime".into());
        m.finish_probe(third, Ok(report()));
        assert_eq!(m.connection_draft.wsl_runtime, "/srv/acp/runtime");
        assert_eq!(m.probe, Probe::Idle);
        // An invalid report fails closed and changes nothing.
        let fourth = m.begin_probe(false);
        m.finish_probe(
            fourth,
            Ok(Discovery {
                user: "-x".into(),
                ..report()
            }),
        );
        assert_eq!(
            m.probe,
            Probe::Failed(connection::discovery_status::INVALID_REPORT)
        );
        assert_eq!(m.connection_draft.wsl_runtime, "/srv/acp/runtime");
        // Applying never happens implicitly.
        assert_eq!(m.connection.target, Target::Synthetic);
    }
}

#[cfg(test)]
mod target_tests {
    #[test]
    fn wsl_status_names_the_cause_without_touching_phase_data_or_guards() {
        use crate::connection::{Connection, Target, wsl_status};
        let mut m = Model::fixture();
        m.connection = Connection {
            target: Target::Wsl,
            distribution: "Synthetic".into(),
            user: "operator".into(),
            helper: "/s/acp-wsl-bridge".into(),
            wsl_runtime: "/s/runtime".into(),
            wsl_endpoint: "/s/runtime/control.sock".into(),
            ..Default::default()
        };
        m.receive(Err(wsl_status::DISTRIBUTION_NOT_FOUND));
        assert_eq!(m.phase, Phase::Unavailable);
        assert_eq!(m.read_label(), m.word(i18n::Msg::WslDistributionNotFound));
        assert!(
            m.status()
                .contains(m.word(i18n::Msg::WslDistributionNotFound))
        );
        // A live link problem is display-only: phase, data and the Operator
        // guard are unchanged, so an uncertain request can still be retried.
        m.receive(Ok(Snapshot::fixture()));
        assert_eq!(m.phase, Phase::Ready);
        m.operator.open = true;
        let guard = m.operator_guard();
        assert!(guard.is_some());
        m.wsl_link = Some(wsl_status::BRIDGE_DISCONNECTED);
        assert_eq!(m.phase, Phase::Ready);
        assert!(m.data.is_some());
        assert!(m.operator_guard() == guard);
        assert_eq!(m.read_label(), m.word(i18n::Msg::WslBridgeDisconnected));
        // A successful read clears both; local targets never show WSL text.
        m.receive(Ok(Snapshot::fixture()));
        assert_eq!((m.wsl_link, m.read_failure), (None, None));
        m.connection.target = Target::Local;
        m.wsl_link = Some(wsl_status::BRIDGE_DISCONNECTED);
        assert_eq!(m.wsl_reason(), None);
    }

    use super::*;
    #[test]
    fn target_switch_invalidates_delayed_scope_data_and_timestamps() {
        let mut m = Model::fixture();
        m.live = true;
        m.last_read_ms = Some(123);
        m.last_duration_ms = Some(8);
        m.task_id = "shared".into();
        m.selected_project = Some("alpha".into());
        let old = m.begin_read().unwrap();
        m.connection_draft = connection::Connection::default();
        assert!(m.apply_connection());
        assert!(m.data.is_none() && m.project_names.is_empty() && m.selected_project.is_none());
        assert!(
            m.task_id.is_empty()
                && m.queried_task.is_empty()
                && m.last_read_ms.is_none()
                && m.last_duration_ms.is_none()
        );
        assert!(!m.complete_read(&old, Ok(Snapshot::fixture()), 999));
        let next = m.begin_read().unwrap();
        let mut snapshot = Snapshot::fixture();
        for p in &mut snapshot.projects {
            p.handoff = None;
        }
        assert!(m.complete_read(&next, Ok(snapshot), 1000));
        assert!(!m.live && m.last_read_ms.is_none());
        m.connection_draft.target = connection::Target::Local;
        let epoch = m.epoch;
        assert!(!m.apply_connection());
        assert_eq!(m.epoch, epoch);
        assert!(m.data.is_some());
    }
    #[test]
    fn first_read_binds_project_and_empty_allowlist_still_records_success() {
        let mut m = Model::loading();
        m.live = true;
        m.task_id = "shared".into();
        let request = m.begin_read().unwrap();
        assert!(m.complete_read(&request, Ok(Snapshot::fixture()), 1));
        assert_eq!(m.selected_project.as_deref(), Some("alpha"));
        let mut reversed = Snapshot::fixture();
        reversed.projects.reverse();
        let request = m.begin_read().unwrap();
        assert!(m.complete_read(&request, Ok(reversed), 2));
        assert_eq!(m.project().unwrap().name, "alpha");
        m.selected_project = None;
        let mut empty = Snapshot::fixture();
        empty.projects.clear();
        let request = m.begin_read().unwrap();
        assert!(m.complete_read(&request, Ok(empty), 3));
        assert_eq!(m.phase, Phase::Empty);
        assert_eq!(m.last_read_ms, Some(3));
    }
    #[test]
    fn initial_unqueried_handoff_is_refused() {
        let mut m = Model::loading();
        let request = m.begin_read().unwrap();
        assert!(m.complete_read(&request, Ok(Snapshot::fixture()), 1));
        assert_eq!(m.phase, Phase::Unavailable);
        assert!(m.data.is_none());
    }
    #[test]
    fn chinese_page_copy_retains_git_handoff_and_disclosed_value() {
        let mut m = Model::fixture();
        m.locale = i18n::Locale::ZhCn;
        let lines = m.visible_lines().join("\n");
        let p = m.project().unwrap();
        assert!(lines.contains(p.context.repository.branch.as_deref().unwrap()));
        assert!(lines.contains(&p.handoff.as_ref().unwrap().objective));
        assert!(lines.contains(&p.context.warnings[0]));
        m.section = Section::Preferences;
        let mut snapshot = Snapshot::fixture();
        snapshot.projects[0].context.preferences[0].value = "中文".repeat(600);
        m.receive(Ok(snapshot));
        m.select_preference(0);
        m.detail_by(1);
        assert!(m.visible_lines().join("\n").contains(&m.detail_chunk()));
        assert!(m.visible_lines().iter().all(|s| s.chars().count() < 550));
    }
    #[test]
    fn chinese_ui_preserves_original_values_and_evidence() {
        let mut m = Model::fixture();
        m.locale = i18n::Locale::ZhCn;
        m.section = Section::Handoff;
        m.expanded = true;
        let lines = m.visible_lines().join("\n");
        let h = m.project().unwrap().handoff.as_ref().unwrap();
        assert!(lines.contains(&h.objective));
        assert!(lines.contains(&h.validation[0].to_string()));
        assert!(lines.contains("并非当前会话验证"));
        m.section = Section::Preferences;
        assert!(m.visible_lines().join("\n").contains(&m.rows()[0].value));
    }
}
