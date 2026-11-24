use super::*;

impl Operator {
    /// The exact selected record paired with the binding of the snapshot that
    /// sourced it. A held exact inspect snapshot outranks list/queue pages for
    /// the same id, so Detail never acts on a superseded page row.
    /// Picks or unpicks a suggestion for the batch.
    pub fn toggle_batch(&mut self, id: i64) {
        match self.batch.binary_search(&id) {
            Ok(i) => {
                self.batch.remove(i);
            }
            Err(i) if self.batch.len() < precept_core::operator::MAX_BATCH => {
                self.batch.insert(i, id)
            }
            Err(_) => {}
        }
        self.review = None;
    }
    /// The picked suggestions as the current list shows them (the queue on
    /// To review, else the scope's records), with that list's binding.
    pub fn batch_records(&self) -> (Vec<&PreferenceRecord>, Option<&Binding>) {
        let (records, binding) = match (&self.queue, &self.page) {
            (Some(q), _) if self.mode == ReviewMode::Inbox => (&q.records, Some(&q.binding)),
            (_, Some(p)) => (&p.records, Some(&p.binding)),
            _ => return (Vec::new(), None),
        };
        (
            records
                .iter()
                .filter(|r| r.status == "suggested" && self.batch.binary_search(&r.id).is_ok())
                .collect(),
            binding,
        )
    }
    pub(super) fn selected(&self) -> Option<(&PreferenceRecord, &Binding)> {
        let id = self.selected_id?;
        if let Some(i) = &self.inspect
            && i.record.id == id
        {
            return Some((&i.record, &i.binding));
        }
        if let Some(p) = &self.page
            && let Some(r) = p.records.iter().find(|r| r.id == id)
        {
            return Some((r, &p.binding));
        }
        self.queue.as_ref().and_then(|q| {
            q.records
                .iter()
                .find(|r| r.id == id)
                .map(|r| (r, &q.binding))
        })
    }
    /// Adds `raw` to the draft's tags. False (nothing changes) when it is not
    /// a valid tag or the draft already has [`MAX_TAGS`]; a repeat is a no-op.
    pub fn add_tag(&mut self, raw: &str) -> bool {
        let Some(tag) = normalize_tag(raw) else {
            return false;
        };
        if self.tags.contains(&tag) {
            return true;
        }
        if self.tags.len() >= MAX_TAGS {
            return false;
        }
        self.tags.push(tag);
        self.tags.sort();
        true
    }
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| t != tag);
    }
    /// A draft holding the selected record, to edit and save as its
    /// replacement: a suggestion, or a confirmed rule that is switched on
    /// (the edit then takes effect at once).
    pub fn start_revision(&mut self) -> bool {
        let Some(r) = self.record().filter(|r| editable(r)).cloned() else {
            return false;
        };
        let confirmed = r.status == "confirmed";
        self.start_draft();
        self.key = r.key;
        self.value = r.value;
        self.tags = r.tags;
        let join = |v: &[String]| v.join(", ");
        self.applies_agents = join(&r.rule.applies_to.agents);
        self.applies_languages = join(&r.rule.applies_to.languages);
        self.applies_paths = join(&r.rule.applies_to.paths);
        self.applies_tags = join(&r.rule.applies_to.task_tags);
        if r.rule.priority != 0 {
            self.priority = r.rule.priority.to_string();
        }
        self.enforcement = r.rule.enforcement;
        self.revising = Some(r.id);
        self.revising_confirmed = confirmed;
        true
    }
    /// A fresh draft for the add dialog.
    pub fn start_draft(&mut self) {
        self.revising = None;
        self.revising_confirmed = false;
        self.key.clear();
        self.value.clear();
        self.tags.clear();
        self.tag_draft.clear();
        self.origin = Origin::Operator;
        self.applies_agents.clear();
        self.applies_languages.clear();
        self.applies_paths.clear();
        self.applies_tags.clear();
        self.priority.clear();
        self.enforcement = Default::default();
    }
    /// The draft's rule options, or `None` when a value is invalid.
    pub fn draft_rule(&self) -> Option<precept_core::operator::RuleOptions> {
        let split = |s: &str| -> Vec<String> {
            s.split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .collect()
        };
        let priority = match self.priority.trim() {
            "" => 0,
            p => p.replace('−', "-").parse().ok()?,
        };
        let rule = precept_core::operator::RuleOptions {
            applies_to: precept_core::operator::AppliesTo {
                agents: split(&self.applies_agents),
                languages: split(&self.applies_languages),
                paths: split(&self.applies_paths),
                task_tags: split(&self.applies_tags),
            }
            .canonical()?,
            priority,
            enforcement: self.enforcement,
        };
        rule.valid().then_some(rule)
    }
    pub fn record(&self) -> Option<&PreferenceRecord> {
        self.selected().map(|(r, _)| r)
    }
    /// Delete is armed only against one exact id+revision+digest with the id
    /// retyped verbatim; any drift disarms instead of widening the permission.
    pub(super) fn delete_armed_for(&self, r: &PreferenceRecord) -> bool {
        self.delete_arm.as_ref().is_some_and(|a| {
            a.retype == r.id.to_string()
                && a.id == r.id
                && a.revision == r.revision
                && a.proposal_digest == r.proposal_digest
        })
    }
    pub(super) fn clear_review_snapshots(&mut self) {
        self.queue = None;
        self.inspect = None;
        self.inspection = None;
        self.queue_after_id = 0;
        self.queue_watermark = 0;
        self.inspect_revision = 0;
        self.digest_expanded = false;
        self.delete_arm = None;
    }
    pub fn invalidate(&mut self) {
        self.serial += 1;
        self.follow = None;
        self.follow_up = None;
        self.review = None;
        self.info = None;
        self.page = None;
        self.handoff = None;
        self.selected_id = None;
        self.busy = false;
        self.receipt = None;
        self.error = None;
        self.clear_review_snapshots();
        self.review_tab = ReviewTab::Queue;
        // An uncertain operation is retained for identification, never rebound.
    }
}
