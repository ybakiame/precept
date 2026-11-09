//! Presentation-only experiment: no framework, SQLite or mutation types.
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        if self.health.api_version != 1 || self.projects.len() > 64 {
            return Err("Unsupported snapshot");
        }
        for p in &self.projects {
            if !seen.insert(&p.name)
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
    pub data: Option<Snapshot>,
    pub phase: Phase,
    pub selected: usize,
    pub dark: bool,
    pub detail_focus: bool,
    pub filter: String,
    pub live: bool,
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
        }
    }
    pub fn fixture() -> Self {
        let mut m = Self::loading();
        m.receive(Ok(Snapshot::fixture()));
        m
    }
    pub fn receive(&mut self, result: Result<Snapshot, &'static str>) {
        match result {
            Ok(s) if s.validate().is_ok() => {
                self.selected = self.selected.min(s.projects.len().saturating_sub(1));
                self.phase = if s.projects.is_empty() {
                    Phase::Empty
                } else {
                    Phase::Ready
                };
                self.data = Some(s);
            }
            _ => {
                self.data = None;
                self.phase = Phase::Unavailable;
            }
        }
    }
    pub fn project(&self) -> Option<&Project> {
        self.data.as_ref()?.projects.get(self.selected)
    }
    pub fn select(&mut self, index: usize) {
        if self.data.as_ref().is_some_and(|s| index < s.projects.len()) {
            self.selected = index;
        }
    }
    pub fn next(&mut self) {
        if let Some(s) = &self.data
            && !s.projects.is_empty()
        {
            self.selected = (self.selected + 1) % s.projects.len();
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
                            || format!("{} {}", r.key, r.value).contains(&self.filter)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn status(&self) -> String {
        match self.phase {
            Phase::Loading => "Loading — read-only query in progress".into(),
            Phase::Empty => "Empty — no allowlisted projects".into(),
            Phase::Error => "Error — synthetic error state; reconnect to retry".into(),
            Phase::Unavailable => {
                "Unavailable — local daemon refused access or could not be reached".into()
            }
            Phase::Ready => match &self.data {
                Some(d) if d.health.ready => format!(
                    "{} · {} · schema {} · read-only",
                    if self.live {
                        "Connected"
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
        out.extend(
            self.rows()
                .iter()
                .map(|r| format!("{} = {} [{} / {}]", r.key, r.value, r.scope, r.source)),
        );
        out.push("Project contracts (read-only references)".into());
        out.extend(
            p.context
                .documents
                .iter()
                .map(|d| serde_json::to_string(d).unwrap()),
        );
        if let Some(h) = &p.handoff {
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
            out.push("No handoff for task shared".into());
        }
        out.extend(p.context.warnings.iter().cloned());
        out
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
        assert!(m.status().starts_with("Error"));
        assert!(Model::loading().status().starts_with("Loading"));
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
}
