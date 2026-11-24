use super::*;

impl Model {
    pub fn complete_operator(&mut self, request: &Review, result: Result<Value, &str>) -> bool {
        if !self.operator_current(request) {
            return false;
        }
        self.operator.busy = false;
        self.operator.review = None;
        let result = result.and_then(|v| {
            match request.method.as_str() {
                "profile.preferences.list" => {
                    let page: PreferencePage =
                        serde_json::from_value(v).map_err(|_| "InvalidRequest")?;
                    if page.records.len() > 12
                        || page.records.iter().any(|r| {
                            r.scope != request.guard.scope
                                || r.id <= request.guard.after_id
                                || r.revision <= 0
                        })
                    {
                        return Err("InvalidRequest");
                    }
                    if self.operator.info.as_ref().and_then(|i| i.binding.as_ref())
                        != Some(&page.binding)
                    {
                        return Err("StaleGeneration");
                    }
                    // Loading more appends to the rows shown when it continues
                    // exactly where they end; any other read replaces them.
                    // (A copy: the selection below is checked against the old rows.)
                    let page = append_page(
                        self.operator.page.clone(),
                        page,
                        request.guard.after_id,
                        |p| (&p.binding, p.next_after_id),
                        |old, new| {
                            old.records.extend(new.records);
                            old.next_after_id = new.next_after_id;
                        },
                    );
                    self.operator.after_id = 0;
                    // A refresh keeps the selection (and its exact inspection)
                    // only while the page still holds that exact record: same
                    // id, revision and digest. Anything else is cleared.
                    let same = self.operator.record().is_some_and(|old| {
                        page.records.iter().any(|r| {
                            r.id == old.id
                                && r.revision == old.revision
                                && r.proposal_digest == old.proposal_digest
                        })
                    });
                    if !same {
                        self.operator.selected_id = None;
                        self.operator.inspect = None;
                        self.operator.inspect_revision = 0;
                        self.operator.digest_expanded = false;
                    }
                    // A just-saved preference may arrive through the scope's
                    // record list as well as the queue: open it the same way.
                    if let Some(id) = self.operator.follow
                        && page.records.iter().any(|r| r.id == id)
                    {
                        self.operator.follow = None;
                        self.operator.selected_id = Some(id);
                        self.operator.review_tab = ReviewTab::Detail;
                        self.operator.follow_up = Some("profile.review.inspect");
                    }
                    self.operator.page = Some(page);
                }
                "profile.review.queue" => {
                    let queue: ReviewQueue =
                        serde_json::from_value(v).map_err(|_| "InvalidRequest")?;
                    let after_id = request.guard.queue_after_id;
                    if queue.commit_sequence < 0
                        || request
                            .guard
                            .queue_sequence
                            .is_some_and(|sequence| queue.commit_sequence < sequence)
                        || queue.records.len() > 12
                        || queue.records.iter().any(|r| {
                            r.scope != request.guard.scope
                                || r.id <= after_id
                                || r.status != "suggested"
                                || r.revision <= 0
                                || digest_from_hex(&r.proposal_digest).is_none()
                        })
                        || queue.records.windows(2).any(|w| w[0].id >= w[1].id)
                    {
                        return Err("InvalidRequest");
                    }
                    if let Some(next) = queue.next_after_id
                        && (queue.records.len() != 12
                            || queue.records.last().map(|r| r.id) != Some(next))
                    {
                        return Err("InvalidRequest");
                    }
                    // Paging is not a snapshot: a later page may never report a
                    // watermark below the disclosed baseline of this session.
                    if after_id > 0 && queue.commit_sequence < request.guard.queue_watermark {
                        return Err("InvalidRequest");
                    }
                    if self.operator.info.as_ref().and_then(|i| i.binding.as_ref())
                        != Some(&queue.binding)
                    {
                        return Err("StaleGeneration");
                    }
                    if after_id == 0 {
                        self.operator.queue_watermark = queue.commit_sequence;
                    }
                    let queue = append_page(
                        self.operator.queue.take(),
                        queue,
                        after_id,
                        |q| (&q.binding, q.next_after_id),
                        |old, new| {
                            old.records.extend(new.records);
                            old.next_after_id = new.next_after_id;
                            old.commit_sequence = new.commit_sequence;
                        },
                    );
                    self.operator.queue_after_id = 0;
                    // A queue refresh replaces the source of review. An older
                    // exact inspect may not hide a moved or removed candidate.
                    self.operator.inspect = None;
                    self.operator.inspect_revision = 0;
                    self.operator.digest_expanded = false;
                    self.operator.page = None;
                    self.operator.queue = Some(queue);
                    if let Some(id) = self.operator.follow.take()
                        && self
                            .operator
                            .queue
                            .as_ref()
                            .is_some_and(|q| q.records.iter().any(|r| r.id == id))
                    {
                        self.operator.selected_id = Some(id);
                        self.operator.review_tab = ReviewTab::Detail;
                        self.operator.follow_up = Some("profile.review.inspect");
                    }
                    if self.operator.selected_id.is_some() && self.operator.record().is_none() {
                        self.operator.selected_id = None;
                    }
                }
                "profile.review.inspect" => {
                    let inspect: ReviewInspect =
                        serde_json::from_value(v).map_err(|_| "InvalidRequest")?;
                    let id = request
                        .params
                        .get("id")
                        .and_then(Value::as_i64)
                        .ok_or("InvalidRequest")?;
                    let revision = request
                        .params
                        .get("revision")
                        .and_then(Value::as_i64)
                        .ok_or("InvalidRequest")?;
                    let r = &inspect.record;
                    if inspect.commit_sequence < 0
                        || (request.guard.queue_watermark > 0
                            && inspect.commit_sequence < request.guard.queue_watermark)
                        || request
                            .guard
                            .queue_sequence
                            .is_some_and(|sequence| inspect.commit_sequence < sequence)
                        || r.id != id
                        || r.revision != revision
                        || r.revision <= 0
                        || r.scope != request.guard.scope
                        || digest_from_hex(&r.proposal_digest).is_none()
                        || inspect.decisions.len() > 8
                        || inspect
                            .decisions
                            .windows(2)
                            .any(|w| w[0].receipt_commit_sequence <= w[1].receipt_commit_sequence)
                        || inspect.decisions.iter().any(|d| {
                            d.preference_id != id
                                || d.from_revision <= 0
                                || d.to_revision.checked_sub(1) != Some(d.from_revision)
                                || d.receipt_commit_sequence <= 0
                                || d.receipt_commit_sequence > inspect.commit_sequence
                                || digest_from_hex(&d.proposal_digest).is_none()
                                || d.proposal_digest != r.proposal_digest
                        })
                        || inspect.peer_demotions.len() > 12
                        || inspect
                            .peer_demotions
                            .windows(2)
                            .any(|w| w[0].id >= w[1].id)
                        || inspect.peer_demotions.len() as i64
                            != inspect.peer_demotion_total.min(12)
                        || inspect
                            .peer_demotions
                            .iter()
                            .any(|p| p.id <= 0 || p.id == id || p.revision <= 0)
                        || inspect.related.len() > 12
                        || inspect.related_total < 0
                        || inspect.related.len() as i64 != inspect.related_total.min(12)
                        || inspect
                            .related
                            .windows(2)
                            .any(|w| (w[0].relation, w[0].id) >= (w[1].relation, w[1].id))
                        || inspect.related.iter().any(|p| {
                            p.id <= 0
                                || p.id == id
                                || p.revision <= 0
                                || p.key.is_empty()
                                || p.key.chars().count() > 120
                                || match p.relation {
                                    ReviewRelation::Duplicate | ReviewRelation::SameKey => {
                                        p.key != r.key
                                    }
                                    ReviewRelation::SameValue => p.key == r.key,
                                }
                        })
                        || !matches!(
                            (&r.scope, &inspect.evidence),
                            (Scope::Project { .. }, Some(_)) | (Scope::Global, None)
                        )
                        || inspect.evidence.is_some_and(|e| {
                            (e.gate_denials == 0) != e.last_gate_denial_ms.is_none()
                        })
                    {
                        return Err("InvalidRequest");
                    }
                    if self.operator.info.as_ref().and_then(|i| i.binding.as_ref())
                        != Some(&inspect.binding)
                    {
                        return Err("StaleGeneration");
                    }
                    self.operator.inspect = Some(inspect);
                    self.operator.inspect_revision = revision;
                    self.operator.digest_expanded = false;
                }
                "profile.handoff.get" => {
                    let preview: HandoffPreview =
                        serde_json::from_value(v).map_err(|_| "InvalidRequest")?;
                    if preview.record.as_ref().is_some_and(|r| {
                        Some(&r.project) != request.guard.project.as_ref()
                            || r.task_id != request.guard.task
                            || r.revision <= 0
                    }) {
                        return Err("InvalidRequest");
                    }
                    if self.operator.info.as_ref().and_then(|i| i.binding.as_ref())
                        != Some(&preview.binding)
                    {
                        return Err("StaleGeneration");
                    }
                    self.operator.handoff = Some(preview);
                    self.operator.handoff_page = 0;
                }
                "profile.mutate" => {
                    let receipt: Receipt =
                        serde_json::from_value(v).map_err(|_| "OutcomeUnknown")?;
                    let mutation = request.mutation.as_ref().ok_or("OutcomeUnknown")?;
                    if receipt.operation_id != mutation.operation_id
                        || receipt.generation != mutation.binding.generation
                        || receipt.commit_sequence <= 0
                    {
                        return Err("OutcomeUnknown");
                    }
                    // Preference IDs are profile-global: Writer's identity-only
                    // receipt leaves project empty even for project-scoped rows.
                    if let Command::DeletePreference { id, .. } = &mutation.command
                        && !receipt.affected.iter().any(|a| {
                            a.kind == "preference"
                                && a.record_id == id.to_string()
                                && a.deleted
                                && a.project.is_empty()
                                && request
                                    .record
                                    .as_ref()
                                    .and_then(|r| r.revision.checked_add(1))
                                    == Some(a.revision)
                        })
                    {
                        return Err("OutcomeUnknown");
                    }
                    // Save, then confirm: a preference the operator just saved
                    // opens as the exact record to confirm (two explicit steps).
                    // An edit opens its replacement the same way.
                    if matches!(
                        &mutation.command,
                        Command::SuggestPreference { .. } | Command::RevisePreference { .. }
                    ) {
                        // The new record has the highest id of the write.
                        self.operator.follow = receipt
                            .affected
                            .iter()
                            .filter(|a| a.kind == "preference" && !a.deleted)
                            .filter_map(|a| a.record_id.parse::<i64>().ok())
                            .max();
                        if self.operator.follow.is_some() {
                            // The scoped pages find it in their record list;
                            // the inbox in the queue.
                            if self.operator.mode == ReviewMode::Inbox {
                                self.operator.queue_after_id = 0;
                                self.operator.follow_up = Some("profile.review.queue");
                            } else {
                                self.operator.after_id = 0;
                                self.operator.follow_up = Some("profile.preferences.list");
                            }
                        }
                    }
                    self.operator.receipt = Some(receipt);
                    self.operator.note.clear();
                    self.operator.revising = None;
                    self.operator.batch.clear();
                    if matches!(&mutation.command, Command::SuggestBatch { .. }) {
                        self.operator.clear_import();
                    }
                    self.operator.pending = None;
                    self.operator.outcome_unknown = false;
                    self.operator.page = None;
                    self.operator.selected_id = None;
                    // Content and decision history snapshots never survive a
                    // commit; the receipt alone remains as identity-only proof.
                    self.operator.clear_review_snapshots();
                }
                "profile.write.disable" => {
                    if v.get("write_enabled").and_then(Value::as_bool) != Some(false) {
                        return Err("OutcomeUnknown");
                    }
                    if let Some(info) = &mut self.operator.info {
                        info.write_enabled = false;
                    }
                    self.operator.page = None;
                    self.operator.clear_review_snapshots();
                }
                _ => {
                    let info: ProfileInfo =
                        serde_json::from_value(v).map_err(|_| "InvalidRequest")?;
                    if info.selected != info.binding.is_some()
                        || (!info.selected && info.write_enabled)
                    {
                        return Err("InvalidRequest");
                    }
                    if self.operator.info.as_ref().and_then(|i| i.binding.as_ref())
                        != info.binding.as_ref()
                    {
                        self.operator.page = None;
                        self.operator.selected_id = None;
                        self.operator.clear_review_snapshots();
                    }
                    self.operator.info = Some(info);
                }
            }
            Ok(())
        });
        // A retry of the pending uncertain mutation that produced no daemon
        // decision ("Unavailable": nothing was submitted, e.g. a lost WSL bridge
        // or a stopped daemon) settles nothing: the uncertainty and the exact
        // request are kept for a later identical retry.
        let unsubmitted_retry = request.mutation.is_some()
            && self.operator.outcome_unknown
            && self.operator.pending.as_ref() == Some(request)
            && result.as_ref().err() == Some(&"Unavailable");
        self.operator.error = result.err().map(str::to_owned);
        if self.operator.error.is_some() {
            self.operator.outcome_unknown = request.mutation.is_some()
                && (self.operator.error.as_deref() == Some("OutcomeUnknown") || unsubmitted_retry);
            if !self.operator.outcome_unknown {
                self.operator.pending = None;
                self.operator.clear_review_snapshots();
            }
            // An uncertain mutation keeps its guard identity (watermark,
            // inspected revision/digest) so the identical retry can settle it.
            self.operator.info = None;
            self.operator.page = None;
            self.operator.handoff = None;
            if !self.operator.outcome_unknown {
                self.operator.selected_id = None;
            }
        }
        // A settled request is fenced off: a duplicate or delayed completion can
        // never apply twice. Only an uncertain mutation keeps its guard so the
        // identical same-epoch retry can still settle it once.
        if !self.operator.outcome_unknown {
            self.operator.serial += 1;
        }
        if request.method == "profile.review.inspect" && self.operator.error.is_none() {
            self.operator.inspection = self.operator_guard();
        }
        true
    }
}

/// `new` appended to `old` when it was read from exactly where `old` ends
/// (same binding, `after_id` its next cursor); otherwise `new` alone.
fn append_page<P>(
    old: Option<P>,
    new: P,
    after_id: i64,
    cursor: impl Fn(&P) -> (&Binding, Option<i64>),
    extend: impl FnOnce(&mut P, P),
) -> P {
    match old {
        Some(mut old)
            if after_id > 0 && {
                let (binding, next) = cursor(&old);
                binding == cursor(&new).0 && next == Some(after_id)
            } =>
        {
            extend(&mut old, new);
            old
        }
        _ => new,
    }
}
