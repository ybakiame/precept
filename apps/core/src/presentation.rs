//! Framework-independent page content. Every returned string is display data.
use crate::i18n::{Locale, Msg};
use crate::{Model, PAGE_SIZE, Phase, Section};

/// A value's first meaningful line for a one-line preview: Markdown heading
/// marks and list bullets stripped, at most `max` characters.
pub fn preview_line(value: &str, max: usize) -> String {
    let line = value
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("```"))
        .unwrap_or("");
    let line = line.trim_start_matches('#').trim_start();
    let line = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .unwrap_or(line);
    let mut out: String = line.chars().take(max).collect();
    if line.chars().count() > max {
        out.push('…');
    }
    out
}

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
    pub fn read_label(&self) -> &'static str {
        // A WSL link or launch problem names its exact cause instead of a
        // generic "disconnected"; it never changes the page phase or data.
        if let Some(msg) = self.wsl_reason() {
            return self.word(msg);
        }
        match self.phase {
            Phase::Loading => self.word(Msg::Reading),
            Phase::Ready if !self.live => self.word(Msg::SyntheticReady),
            Phase::Ready => self.word(Msg::ReadSucceeded),
            Phase::Empty => self.word(Msg::EmptyProjects),
            Phase::Error => self.word(Msg::InvalidTask),
            Phase::Unavailable if self.permission_denied => self.word(Msg::AccessRefused),
            Phase::Unavailable => self.word(Msg::Disconnected),
        }
    }
    pub fn source_label(&self) -> &'static str {
        if self.live {
            "LIVE_READ_ONLY"
        } else {
            "OFFLINE_SYNTHETIC"
        }
    }
    pub fn read_time(&self) -> String {
        self.last_read_ms
            .map(|t| {
                format!(
                    "{} {:02}:{:02}:{:02} UTC",
                    self.word(Msg::LastRead),
                    (t / 3_600_000) % 24,
                    (t / 60_000) % 60,
                    (t / 1000) % 60
                )
            })
            .unwrap_or_else(|| {
                if self.live {
                    self.word(Msg::NoRead).into()
                } else {
                    self.word(Msg::Offline).into()
                }
            })
    }
    pub fn visible_lines(&self) -> Vec<String> {
        if self.phase != Phase::Ready {
            return vec![self.status()];
        }
        let Some(p) = self.project() else {
            return vec!["No project selected".into()];
        };
        if self.locale == Locale::ZhCn {
            return self.chinese_lines();
        }
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
                lines.push(format!(
                    "{} confirmed preferences · Open Preferences to inspect",
                    p.context.preferences.len()
                ));
                lines.extend(self.handoff_lines(false));
                lines.extend(p.context.warnings.iter().take(8).cloned());
            }
            Section::Projects => {
                lines.push("Only the daemon's projects; switching clears the task lookup.".into());
                lines.push(
                    "Repository facts are observed at read time; Unknown is not a clean checkout."
                        .into(),
                );
                lines.extend(p.context.warnings.iter().take(8).cloned());
            }
            Section::Rules => {
                lines.extend(self.preference_lines());
                if let Some(row) = self.preference_detail() {
                    lines.push(format!("Selected preference: {}", sanitize(&row.key, 120)));
                    lines.push(format!(
                        "Value detail {}/{}",
                        self.detail_page + 1,
                        self.detail_pages()
                    ));
                    lines.push(self.detail_chunk());
                }
            }
            Section::Handoff => lines.extend(self.handoff_lines(self.expanded)),
            Section::Diagnostics => {
                // Doctor covers the daemon and integrations; this is the
                // project's document evidence only.
                lines.push("Project contracts — metadata only, no instruction execution".into());
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
    fn chinese_lines(&self) -> Vec<String> {
        let Some(p) = self.project() else {
            return vec![self.word(Msg::NoProject).into()];
        };
        let mut lines = vec![format!("{}: {}", self.word(Msg::Project), p.name)];
        match self.section {
            Section::Projects => {
                lines.push(self.word(Msg::ProjectsHelp).into());
                lines.push(self.word(Msg::RepositoryHelp).into());
                lines.extend(p.context.warnings.iter().take(8).cloned());
            }
            Section::Handoff => {
                if let Some(h) = p
                    .handoff
                    .as_ref()
                    .filter(|h| h.task_id == self.queried_task)
                {
                    lines.extend([
                        format!("{} {}: {}", self.word(Msg::Handoff), h.task_id, h.objective),
                        format!("{}: {}", self.word(Msg::NextAction), h.next_action),
                        format!(
                            "{}: {}",
                            self.word(Msg::Fingerprint),
                            h.current_repository
                                .get("worktree_fingerprint")
                                .and_then(|v| v.as_str())
                                .unwrap_or(self.word(Msg::FingerprintUnavailable))
                        ),
                        self.word(Msg::HistoricalEvidence).into(),
                        self.word(Msg::EarlierNotCurrent).into(),
                    ]);
                    lines.extend(h.resume_warnings.iter().take(8).cloned());
                    if self.expanded {
                        lines.extend(
                            h.validation
                                .iter()
                                .skip(self.evidence_page * 6)
                                .take(6)
                                .map(|v| v.to_string()),
                        );
                    } else {
                        lines.push(format!(
                            "{} {}",
                            h.validation.len(),
                            self.word(Msg::HistoricalEntries)
                        ));
                    }
                } else if self.queried_task.is_empty() {
                    lines.push(self.word(Msg::HandoffEmpty).into());
                } else {
                    lines.push(format!(
                        "{} {} · {}",
                        self.word(Msg::HandoffMissing),
                        self.queried_task,
                        self.word(Msg::InProject)
                    ));
                }
            }
            Section::Diagnostics => {
                lines.push(self.word(Msg::ContractsHelp).into());
                if self.expanded {
                    lines.extend(
                        p.context
                            .documents
                            .iter()
                            .skip(self.evidence_page * 6)
                            .take(6)
                            .map(|v| v.to_string()),
                    );
                } else {
                    lines.push(format!(
                        "{} {}",
                        p.context.documents.len(),
                        self.word(Msg::ContractEntries)
                    ));
                }
            }
            Section::Rules => {
                lines.push(self.word(Msg::PreferencesHeading).into());
                lines.push(self.page_label());
                lines.extend(
                    self.rows()
                        .into_iter()
                        .skip(self.page * PAGE_SIZE)
                        .take(PAGE_SIZE)
                        .map(|r| {
                            format!(
                                "{} = {} [{} / {}]",
                                sanitize(&r.key, 64),
                                sanitize(&r.value, 160),
                                sanitize(&r.scope, 64),
                                hashtags(&r.tags)
                            )
                        }),
                );
                if let Some(row) = self.preference_detail() {
                    lines.push(format!(
                        "{}: {}",
                        self.word(Msg::SelectedPreference),
                        sanitize(&row.key, 120)
                    ));
                    lines.push(format!(
                        "{} {}/{}",
                        self.word(Msg::ValueDetail),
                        self.detail_page + 1,
                        self.detail_pages()
                    ));
                    lines.push(self.detail_chunk());
                }
            }
            Section::Dashboard => {
                let r = &p.context.repository;
                lines.push(format!(
                    "{}: {} · HEAD: {} · {}: {}",
                    self.word(Msg::Branch),
                    r.branch.as_deref().unwrap_or(self.word(Msg::Unknown)),
                    r.head.as_deref().unwrap_or(self.word(Msg::Unknown)),
                    self.word(Msg::Worktree),
                    self.word(match r.dirty {
                        Some(true) => Msg::HasChanges,
                        Some(false) => Msg::NoChanges,
                        None => Msg::Unknown,
                    })
                ));
                lines.push(format!(
                    "{} {} · {}",
                    p.context.preferences.len(),
                    self.word(Msg::ConfirmedPreferences),
                    self.word(Msg::OpenPreferences)
                ));
                let mut handoff = self.clone();
                handoff.section = Section::Handoff;
                handoff.expanded = false;
                lines.extend(handoff.chinese_lines().into_iter().skip(1));
                lines.extend(p.context.warnings.iter().take(8).cloned());
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
            "Confirmed effective preferences · key / value / scope / tags".into(),
            self.page_label(),
        ];
        if rows.is_empty() {
            lines.push("Empty — no confirmed preferences match".into());
        }
        lines.extend(
            rows.into_iter()
                .skip(self.page * PAGE_SIZE)
                .take(PAGE_SIZE)
                .map(|r| {
                    format!(
                        "{} = {} [{} / {}]",
                        sanitize(&r.key, 64),
                        sanitize(&r.value, 160),
                        sanitize(&r.scope, 64),
                        hashtags(&r.tags)
                    )
                }),
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

/// A row's tags as terminal and clipboard text: `#style #git`.
/// Where a rule applies, in the operator's words (`None` for everywhere).
pub fn applies_words(m: &crate::Model, a: &precept_core::operator::AppliesTo) -> Option<String> {
    use crate::i18n::Msg;
    let parts: Vec<String> = [
        (Msg::AppliesLanguages, &a.languages),
        (Msg::AppliesPaths, &a.paths),
        (Msg::AppliesAgents, &a.agents),
        (Msg::AppliesTaskTags, &a.task_tags),
    ]
    .iter()
    .filter(|(_, v)| !v.is_empty())
    .map(|(label, v)| {
        let v: Vec<String> = v.iter().map(|s| sanitize(s, 64)).collect();
        format!("{} {}", m.word(*label).to_lowercase(), v.join(", "))
    })
    .collect();
    (!parts.is_empty()).then(|| {
        m.word(Msg::AppliesSummary)
            .replace("{what}", &parts.join("; "))
    })
}
pub fn hashtags(tags: &[String]) -> String {
    tags.iter()
        .map(|t| format!("#{}", sanitize(t, 32)))
        .collect::<Vec<_>>()
        .join(" ")
}
