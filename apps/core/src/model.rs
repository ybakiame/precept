use super::*;

impl Model {
    pub fn loading() -> Self {
        Self {
            operator: Default::default(),
            epoch: 0,
            connection: Default::default(),
            connection_draft: Default::default(),
            connection_invalid: false,
            settings: false,
            setup_open: false,
            setup_tab: SetupTab::Connection,
            agents_page: false,
            sessions_page: false,
            memory_page: false,
            memory: Default::default(),
            sessions: Default::default(),
            handoff_list: None,
            agents: Default::default(),
            distributions: None,
            probe: Probe::Idle,
            probe_epoch: 0,
            connection_advanced: false,
            project_add: Default::default(),
            project_remove: None,
            profile_setup: Default::default(),
            profile_info: Default::default(),
            review_count: None,
            keymap: Default::default(),
            shortcuts_open: false,
            shortcut_recording: None,
            save_failure: None,
            theme: Default::default(),
            color_mode: appearance::ColorMode::System,
            locale: Default::default(),
            last_duration_ms: None,
            project_names: Vec::new(),
            last_read_ms: None,
            section: Section::Dashboard,
            high_contrast: false,
            sidebar_collapsed: false,
            selected_preference: None,
            selected_rule: None,
            detail_page: 0,
            copied: false,
            data: None,
            phase: Phase::Loading,
            selected: 0,
            dark: false,
            detail_focus: false,
            filter: String::new(),
            tag_filter: Vec::new(),
            tag_colors: BTreeMap::new(),
            tag_colors_open: false,
            tag_editing: None,
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
    /// A target transition invalidates pending reads, scope, evidence and timestamps together.
    pub fn apply_connection(&mut self) -> bool {
        if !self.connection_draft.validate() {
            self.connection_invalid = true;
            return false;
        }
        self.epoch = self.epoch.checked_add(1).expect("request epoch exhausted");
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
        self.detail_page = 0;
        self.selected_preference = None;
        self.selected_rule = None;
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
        self.operator.invalidate();
        self.data = None;
        self.selected_preference = None;
        self.selected_rule = None;
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
            self.operator.invalidate();
            self.selected = index;
            self.selected_project = Some(name);
            self.page = 0;
            self.selected_preference = None;
            self.selected_rule = None;
            self.detail_page = 0;
        }
    }
    pub fn next(&mut self) {
        if !self.project_names.is_empty() {
            self.select((self.selected + 1) % self.project_names.len());
        }
    }
    /// The confirmed rows the Preferences page shows: every selected tag
    /// carried, and the filter text in the key, value, scope or a tag (a
    /// leading `#` matches tags only).
    pub fn rows(&self) -> Vec<&Preference> {
        let needle = self.filter.trim().to_lowercase();
        let (needle, tags_only) = match needle.strip_prefix('#') {
            Some(rest) => (rest.to_owned(), true),
            None => (needle, false),
        };
        self.project()
            .map(|p| {
                p.context
                    .preferences
                    .iter()
                    .filter(|r| self.tag_filter.iter().all(|t| r.tags.contains(t)))
                    .filter(|r| {
                        needle.is_empty()
                            || r.tags.iter().any(|t| t.contains(&needle))
                            || (!tags_only
                                && [&r.key, &r.value, &r.scope]
                                    .iter()
                                    .any(|v| v.to_lowercase().contains(&needle)))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Every tag on the project's confirmed rows with how many carry it, most
    /// used first (ties by name): the Preferences page's tag filters.
    pub fn tag_counts(&self) -> Vec<(String, usize)> {
        let mut counts = BTreeMap::<String, usize>::new();
        for r in self.project().map_or(&[][..], |p| &p.context.preferences) {
            for t in &r.tags {
                *counts.entry(t.clone()).or_default() += 1;
            }
        }
        let mut counts: Vec<_> = counts.into_iter().collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        counts
    }
    /// Narrows the Preferences page to `tag` too, or stops narrowing to it.
    pub fn toggle_tag(&mut self, tag: &str) {
        if let Some(at) = self.tag_filter.iter().position(|t| t == tag) {
            self.tag_filter.remove(at);
        } else {
            self.tag_filter.push(tag.to_owned());
            self.tag_filter.sort();
        }
        self.page = 0;
        self.selected_preference = None;
        self.selected_rule = None;
        self.detail_page = 0;
    }
    pub fn clear_tags(&mut self) {
        self.tag_filter.clear();
        self.page = 0;
        self.selected_preference = None;
        self.selected_rule = None;
        self.detail_page = 0;
    }
    /// Tags offered while a preference's tags are edited: the ones already in
    /// use here (most used first), then the starter set, without those chosen
    /// and narrowed to what `draft` starts with.
    pub fn tag_suggestions(&self, chosen: &[String], draft: &str) -> Vec<String> {
        let draft = precept_core::operator::normalize_tag(draft).unwrap_or_default();
        let mut seen = std::collections::BTreeSet::new();
        let operator = self
            .operator
            .page
            .iter()
            .flat_map(|p| &p.records)
            .chain(self.operator.queue.iter().flat_map(|q| &q.records))
            .flat_map(|r| r.tags.iter().cloned());
        self.tag_counts()
            .into_iter()
            .map(|(t, _)| t)
            .chain(operator)
            .chain(STARTER_TAGS.iter().map(|t| (*t).to_owned()))
            .filter(|t| !chosen.contains(t) && t.starts_with(&draft))
            .filter(|t| seen.insert(t.clone()))
            .take(8)
            .collect()
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
    pub(super) fn connection_status(&self) -> String {
        match self.phase {
            Phase::Loading => "Loading — read-only query in progress".into(),
            Phase::Empty => "Empty — no projects yet".into(),
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
        self.selected_rule = None;
        self.detail_page = 0;
    }
    pub fn page_by(&mut self, delta: isize) {
        let last = self.rows().len().saturating_sub(1) / PAGE_SIZE;
        self.page = self.page.saturating_add_signed(delta).min(last);
        self.selected_preference = None;
        self.selected_rule = None;
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
    pub fn select_rule(&mut self, index: usize) {
        self.selected_preference = None;
        self.selected_rule = self
            .project()
            .is_some_and(|p| index < p.context.repository_rules.len())
            .then_some(index);
    }
    pub fn rule_detail(&self) -> Option<&RepositoryRule> {
        self.project()?
            .context
            .repository_rules
            .get(self.selected_rule?)
    }
    pub fn select_preference(&mut self, index: usize) {
        self.selected_rule = None;
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
