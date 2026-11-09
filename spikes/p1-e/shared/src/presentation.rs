//! Framework-independent page content. Every returned string is display data.
use crate::{Model, PAGE_SIZE, Phase, Section};

pub fn sanitize(value: &str, limit: usize) -> String {
    let mut chars = value.chars().filter(|c| {
        !c.is_control() && !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
    });
    let mut out: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        out.push_str("… [display truncated]");
    }
    out
}
impl Model {
    pub fn visible_lines(&self) -> Vec<String> {
        if self.phase != Phase::Ready {
            return vec![self.status()];
        }
        let Some(p) = self.project() else {
            return vec!["No project selected".into()];
        };
        let mut lines = vec![format!("Project: {}", p.name)];
        match self.section {
            Section::Dashboard => {
                let r = &p.context.repository;
                lines.push(format!(
                    "Branch: {} · HEAD: {} · dirty: {}",
                    r.branch.as_deref().unwrap_or("Unknown"),
                    r.head.as_deref().unwrap_or("Unknown"),
                    r.dirty.map(|v| v.to_string()).unwrap_or("Unknown".into())
                ));
                lines.extend(self.preference_lines());
                lines.extend(self.handoff_lines(false));
                lines.extend(p.context.warnings.iter().take(8).cloned());
            }
            Section::Projects => {
                lines.push(
                    "Only daemon-allowlisted projects; switching clears the task lookup.".into(),
                );
                lines.push(
                    "Repository facts are observed at read time; Unknown is not a clean checkout."
                        .into(),
                );
                lines.extend(p.context.warnings.iter().take(8).cloned());
            }
            Section::Preferences => lines.extend(self.preference_lines()),
            Section::Handoff => lines.extend(self.handoff_lines(self.expanded)),
            Section::Diagnostics => {
                lines.extend([
                    "Read-only operator IPC · API v1 · no daemon autostart".into(),
                    "NSH Rust Desktop / TUI · pinned framework · ADR-0003 Proposed".into(),
                    "Ctrl R reconnect · P project · F filter · K task · D theme · Shift H contrast"
                        .into(),
                    "Ctrl 1–5 pages · Escape blur · Alt arrows preference pages · Ctrl Q close"
                        .into(),
                    "Copy visible page is explicit; values are sanitized and bounded.".into(),
                    "Health is the last query result; no continuous connection is claimed.".into(),
                    "Project contracts — metadata only, no instruction execution".into(),
                ]);
                if self.expanded {
                    lines.extend(
                        p.context
                            .documents
                            .iter()
                            .skip(self.evidence_page * 6)
                            .take(6)
                            .map(|d| d.to_string()),
                    );
                } else {
                    lines.push(format!(
                        "{} contract references · collapsed",
                        p.context.documents.len()
                    ));
                }
            }
        }
        lines
            .into_iter()
            .take(48)
            .map(|s| sanitize(&s, 512))
            .collect()
    }
    fn preference_lines(&self) -> Vec<String> {
        let rows = self.rows();
        let mut lines = vec![
            "Confirmed effective preferences · key / value / scope / source".into(),
            self.page_label(),
        ];
        if rows.is_empty() {
            lines.push("Empty — no confirmed preferences match".into());
        }
        lines.extend(
            rows.into_iter()
                .skip(self.page * PAGE_SIZE)
                .take(PAGE_SIZE)
                .map(|r| format!("{} = {} [{} / {}]", r.key, r.value, r.scope, r.source)),
        );
        lines
    }
    fn handoff_lines(&self, expanded: bool) -> Vec<String> {
        let Some(p) = self.project() else {
            return Vec::new();
        };
        let Some(h) = p
            .handoff
            .as_ref()
            .filter(|h| h.task_id == self.queried_task)
        else {
            return vec![if self.queried_task.is_empty() {
                "Handoff — enter a task ID and choose Lookup".into()
            } else {
                format!(
                    "Empty — no handoff for task {} in this project",
                    self.queried_task
                )
            }];
        };
        let mut lines = vec![
            format!("Handoff {}: {}", h.task_id, h.objective),
            format!("Next: {}", h.next_action),
            format!(
                "Fingerprint: {}",
                h.current_repository
                    .get("worktree_fingerprint")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unavailable — inspect differences manually")
            ),
            "Recorded validation evidence (not current session verification)".into(),
        ];
        lines.extend(h.resume_warnings.iter().take(8).cloned());
        if expanded {
            lines.extend(
                h.validation
                    .iter()
                    .skip(self.evidence_page * 6)
                    .take(6)
                    .map(|v| v.to_string()),
            );
        } else {
            lines.push(format!(
                "{} historical evidence entries · collapsed",
                h.validation.len()
            ));
        }
        lines
    }
    pub fn evidence_pages(&self) -> usize {
        let count = self
            .project()
            .map(|p| match self.section {
                Section::Diagnostics => p.context.documents.len(),
                Section::Handoff => p
                    .handoff
                    .as_ref()
                    .filter(|h| h.task_id == self.queried_task)
                    .map_or(0, |h| h.validation.len()),
                _ => 0,
            })
            .unwrap_or(0);
        count.div_ceil(6).max(1)
    }
    pub fn evidence_by(&mut self, delta: isize) {
        self.evidence_page = self
            .evidence_page
            .saturating_add_signed(delta)
            .min(self.evidence_pages() - 1);
    }
    pub fn set_section(&mut self, section: Section) {
        self.section = section;
        self.expanded = false;
        self.evidence_page = 0;
    }
}
