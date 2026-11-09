//! Presentation-only experiment: no framework, SQLite or mutation types.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

pub const PAGE_SIZE: usize = 12;
pub const MAX_ROWS: usize = 10_000;
pub fn valid_task(task: &str) -> bool {
    !task.trim().is_empty() && task.chars().count() <= 120 && !task.chars().any(char::is_control)
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub mod native;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Health {
    pub api_version: u32,
    pub service: String,
    pub ready: bool,
    pub schema: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Preference {
    pub key: String,
    pub value: String,
    pub scope: String,
    pub source: String,
    pub status: String,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Repository {
    pub head: Option<String>,
    pub branch: Option<String>,
    pub dirty: Option<bool>,
    pub change_count: Option<u64>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Context {
    pub project: String,
    pub preferences: Vec<Preference>,
    pub repository: Repository,
    pub documents: Vec<Value>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Handoff {
    pub project: String,
    pub task_id: String,
    pub objective: String,
    pub next_action: String,
    pub validation: Vec<Value>,
    pub resume_warnings: Vec<String>,
    pub current_repository: Value,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Project {
    pub name: String,
    pub context: Context,
    pub handoff: Option<Handoff>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Snapshot {
    pub health: Health,
    pub projects: Vec<Project>,
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), &'static str> {
        let mut seen = std::collections::HashSet::new();
        if self.health.api_version != 1
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
pub struct Model {
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
    pub queried_task: String,
    pub selected_project: Option<String>,
}
impl Model {
    pub fn loading() -> Self {
        Self {
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
    pub fn receive(&mut self, result: Result<Snapshot, &'static str>) {
        self.permission_denied = result.as_ref().is_err_and(|e| *e == "Permission denied");
        match result {
            Ok(s) if s.validate().is_ok() => {
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
        if self.data.as_ref().is_some_and(|s| index < s.projects.len()) {
            self.selected = index;
            self.selected_project = self
                .data
                .as_ref()
                .and_then(|s| s.projects.get(index))
                .map(|p| p.name.clone());
            self.page = 0;
        }
    }
    pub fn next(&mut self) {
        if let Some(s) = &self.data
            && !s.projects.is_empty()
        {
            self.select((self.selected + 1) % s.projects.len());
        }
    }
    pub fn rows(&self) -> Vec<&Preference> {
        self.project()
            .map(|p| {
                p.context
                    .preferences
                    .iter()
                    .filter(|r| {
                        self.filter.is_empty()
                            || [&r.key, &r.value, &r.scope, &r.source]
                                .iter()
                                .any(|v| v.to_lowercase().contains(&self.filter.to_lowercase()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn status(&self) -> String {
        let source = if self.live {
            "LIVE_READ_ONLY"
        } else {
            "OFFLINE_SYNTHETIC"
        };
        format!("{source} · {}", self.connection_status())
    }
    fn connection_status(&self) -> String {
        match self.phase {
            Phase::Loading => "Loading — read-only query in progress".into(),
            Phase::Empty => "Empty — no allowlisted projects".into(),
            Phase::Error => {
                "Error — invalid task ID; use 1–120 characters without control characters".into()
            }
            Phase::Unavailable => {
                if self.permission_denied {
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
    }
    pub fn page_by(&mut self, delta: isize) {
        let last = self.rows().len().saturating_sub(1) / PAGE_SIZE;
        self.page = self.page.saturating_add_signed(delta).min(last);
    }
    pub fn page_label(&self) -> String {
        let count = self.rows().len();
        format!(
            "{} confirmed rows · page {}/{}",
            count,
            self.page + 1,
            count.div_ceil(PAGE_SIZE).max(1)
        )
    }
    pub fn detail_lines(&self) -> Vec<String> {
        if self.phase != Phase::Ready {
            return vec![self.status()];
        }
        let Some(p) = self.project() else {
            return vec!["No project selected".into()];
        };
        let r = &p.context.repository;
        let mut out = vec![
            format!("Project: {}", p.name),
            format!(
                "Branch: {} · HEAD: {} · dirty: {}",
                r.branch.as_deref().unwrap_or("Unknown"),
                r.head.as_deref().unwrap_or("Unknown"),
                r.dirty.map(|v| v.to_string()).unwrap_or("Unknown".into())
            ),
            "Confirmed effective preferences · key / value / scope / source".into(),
        ];
        if self.rows().is_empty() {
            out.push("Empty — no confirmed preferences match".into());
        }
        out.push(self.page_label());
        out.extend(
            self.rows()
                .iter()
                .skip(self.page * PAGE_SIZE)
                .take(PAGE_SIZE)
                .map(|r| format!("{} = {} [{} / {}]", r.key, r.value, r.scope, r.source)),
        );
        out.push("Project contracts (read-only references)".into());
        out.extend(
            p.context
                .documents
                .iter()
                .map(|d| serde_json::to_string(d).unwrap()),
        );
        if let Some(h) = p
            .handoff
            .as_ref()
            .filter(|h| h.task_id == self.queried_task)
        {
            out.push(format!("Handoff {}: {}", h.task_id, h.objective));
            out.push(format!("Next: {}", h.next_action));
            out.push(format!(
                "Fingerprint: {}",
                h.current_repository
                    .get("worktree_fingerprint")
                    .and_then(Value::as_str)
                    .unwrap_or("Unavailable — inspect differences manually")
            ));
            out.extend(h.resume_warnings.iter().cloned());
            out.push("Recorded validation evidence (not current session verification)".into());
            out.extend(
                h.validation
                    .iter()
                    .map(|v| serde_json::to_string(v).unwrap()),
            );
        } else {
            out.push(if self.queried_task.is_empty() {
                "Handoff — enter a task ID and choose Lookup".into()
            } else {
                format!(
                    "Empty — no handoff for task {} in this project",
                    self.queried_task
                )
            });
        }
        out.extend(p.context.warnings.iter().cloned());
        // Display data as text, with bounded lines and no terminal control bytes.
        if out.len() > 96 {
            out.truncate(95);
            out.push("Additional evidence lines omitted; inspect the original locally".into());
        }
        out.into_iter()
            .map(|line| {
                let mut chars = line.chars().filter(|c| !c.is_control());
                let mut text: String = chars.by_ref().take(4096).collect();
                if chars.next().is_some() {
                    text.push_str("… [display truncated; inspect original locally]");
                }
                text
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
