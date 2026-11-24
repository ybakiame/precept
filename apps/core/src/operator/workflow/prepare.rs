use super::*;

impl Model {
    pub fn operator_read(&mut self, method: &str) -> Option<Review> {
        if self.operator.busy || self.operator.outcome_unknown || !self.operator.open {
            return None;
        }
        let params = match method {
            "profile.info" => json!({}),
            "profile.preferences.list" => {
                json!({"scope":self.operator_scope()?,"after_id":self.operator.after_id})
            }
            "profile.review.queue" => {
                if self.operator.queue_after_id < 0 {
                    return None;
                }
                json!({"scope":self.operator_scope()?,"after_id":self.operator.queue_after_id})
            }
            "profile.review.inspect" => {
                let scope = self.operator_scope()?;
                let (r, _) = self.operator.selected()?;
                if r.scope != scope || r.id <= 0 || r.revision <= 0 {
                    return None;
                }
                json!({"scope":scope,"id":r.id,"revision":r.revision})
            }
            "profile.handoff.get" if crate::valid_task(&self.operator.task) => {
                json!({"project":self.selected_project.as_ref()?,"task_id":self.operator.task})
            }
            _ => return None,
        };
        self.operator.serial += 1;
        let guard = self.operator_guard()?;
        self.operator.review = None;
        self.operator.delete_arm = None;
        self.operator.busy = true;
        Some(Review {
            guard,
            method: method.into(),
            params,
            mutation: None,
            record: None,
        })
    }
    pub fn operator_review(&mut self, action: &str, operation_id: &str) -> bool {
        if self.operator.busy || self.operator.outcome_unknown || !self.operator.open {
            return false;
        }
        self.operator.review = None;
        let Some(guard) = self.operator_guard() else {
            return false;
        };
        let Some(info) = self.operator.info.as_ref() else {
            return false;
        };
        let (method, params, mutation) = match action {
            "create" | "select" if !info.selected => (
                if action == "create" {
                    "profile.create"
                } else {
                    "profile.select"
                },
                json!({"path":self.operator.path,"remember":self.operator.remember}),
                None,
            ),
            "enable" if info.selected && !info.write_enabled => {
                let Some(b) = &info.binding else {
                    return false;
                };
                (
                    "profile.write.enable",
                    json!({"profile_id":b.profile_id,"generation":b.generation,"confirm":true}),
                    None,
                )
            }
            "disable" if info.write_enabled => ("profile.write.disable", json!({}), None),
            "import" if info.write_enabled => {
                let Some(review) = import::prepare(self, operation_id) else {
                    return false;
                };
                review
            }
            "batch_confirm" | "batch_reject" if info.write_enabled => {
                let Some(binding) = info.binding.clone() else {
                    return false;
                };
                let (records, source) = self.operator.batch_records();
                // Every picked row must still be listed, from this Profile.
                if records.is_empty()
                    || records.len() != self.operator.batch.len()
                    || source != Some(&binding)
                {
                    return false;
                }
                let Some(note) =
                    precept_core::operator::canonical_note(Some(self.operator.note.as_str()))
                else {
                    self.operator.error = Some("InvalidNote".into());
                    return false;
                };
                let items = records
                    .iter()
                    .map(|r| precept_core::operator::BatchItem {
                        id: r.id,
                        scope: r.scope.clone(),
                        proposal_digest: r.proposal_digest.clone(),
                        expected: Expected::Revision(r.revision),
                    })
                    .collect();
                let mutation = Mutation {
                    version: 2,
                    binding,
                    operation_id: operation_id.into(),
                    command: Command::DecideBatch {
                        decision: if action == "batch_confirm" {
                            precept_core::operator::BatchDecision::Confirm
                        } else {
                            precept_core::operator::BatchDecision::Reject
                        },
                        items,
                        channel: self.operator.channel,
                        note,
                    },
                };
                ("profile.mutate", json!({}), Some(mutation))
            }
            "suggest" | "confirm" | "reject" | "deprecate" | "delete" | "switch_on"
            | "switch_off" | "revise"
                if info.write_enabled =>
            {
                let Some(binding) = info.binding.clone() else {
                    return false;
                };
                let Some(scope) = self.operator_scope() else {
                    return false;
                };
                let command = if action == "suggest" {
                    let Some(tags) = canonical_tags(&self.operator.tags) else {
                        return false;
                    };
                    if self.operator.key.trim().is_empty() || self.operator.value.trim().is_empty()
                    {
                        return false;
                    }
                    let Some(rule) = self.operator.draft_rule() else {
                        self.operator.error = Some("InvalidRule".into());
                        return false;
                    };
                    // A suggestion's new tags keep a generated oklch() color.
                    self.pin_tag_colors(&tags);
                    Command::SuggestPreference {
                        scope,
                        key: self.operator.key.clone(),
                        value: self.operator.value.clone(),
                        tags,
                        origin: self.operator.origin,
                        rule,
                        expected: Expected::Absent,
                    }
                } else {
                    let Some((r, source)) = self.operator.selected() else {
                        return false;
                    };
                    // A list/queue row is not an inspection: decisions require
                    // the exact snapshot that disclosed peers and history.
                    let Some(inspect) = &self.operator.inspect else {
                        return false;
                    };
                    // The record, authority and sources must still be the ones
                    // inspected; the draft (its serial) is not part of that.
                    if !self.operator_inspected() {
                        return false;
                    }
                    if r.scope != scope
                        || *source != binding
                        || inspect.record != *r
                        || inspect.binding != binding
                        || self.operator.inspect_revision != r.revision
                    {
                        return false;
                    }
                    // Deprecate only retires a confirmed row; confirm/reject
                    // only decide a suggested one. Delete is armed separately.
                    match action {
                        "confirm" | "reject" if r.status != "suggested" => return false,
                        "revise" if !editable(r) || self.operator.revising != Some(r.id) => {
                            return false;
                        }
                        "deprecate" if r.status != "confirmed" => return false,
                        "switch_on" | "switch_off" if r.status != "confirmed" => return false,
                        "switch_on" if r.enabled => return false,
                        "switch_off" if !r.enabled => return false,
                        "delete" if !self.operator.delete_armed_for(r) => return false,
                        _ => {}
                    }
                    let id = r.id;
                    let proposal_digest = r.proposal_digest.clone();
                    let channel = self.operator.channel;
                    let expected = Expected::Revision(r.revision);
                    // The operator's note, only on decisions that keep one.
                    let note = if matches!(action, "confirm" | "reject" | "deprecate" | "revise") {
                        let Some(note) = precept_core::operator::canonical_note(Some(
                            self.operator.note.as_str(),
                        )) else {
                            self.operator.error = Some("InvalidNote".into());
                            return false;
                        };
                        note
                    } else {
                        None
                    };
                    let revision = if action == "revise" {
                        let Some(tags) = canonical_tags(&self.operator.tags) else {
                            return false;
                        };
                        if self.operator.key.trim().is_empty()
                            || self.operator.value.trim().is_empty()
                        {
                            return false;
                        }
                        let Some(rule) = self.operator.draft_rule() else {
                            self.operator.error = Some("InvalidRule".into());
                            return false;
                        };
                        Some((tags, rule))
                    } else {
                        None
                    };
                    match action {
                        "revise" => {
                            let (tags, rule) = revision.expect("built above");
                            self.pin_tag_colors(&tags);
                            Command::RevisePreference {
                                id,
                                scope,
                                proposal_digest,
                                key: self.operator.key.clone(),
                                value: self.operator.value.clone(),
                                tags,
                                rule,
                                channel,
                                expected,
                                note,
                            }
                        }
                        "switch_on" | "switch_off" => Command::SetPreferenceEnabled {
                            id,
                            scope,
                            enabled: action == "switch_on",
                            channel,
                            expected,
                        },
                        "confirm" => Command::ConfirmPreference {
                            id,
                            scope,
                            proposal_digest,
                            channel,
                            expected,
                            note,
                        },
                        "reject" => Command::RejectPreference {
                            id,
                            scope,
                            proposal_digest,
                            channel,
                            expected,
                            note,
                        },
                        "deprecate" => Command::DeprecatePreference {
                            id,
                            scope,
                            proposal_digest,
                            channel,
                            expected,
                            note,
                        },
                        _ => Command::DeletePreference {
                            id,
                            scope,
                            proposal_digest,
                            channel,
                            expected,
                        },
                    }
                };
                let mutation = Mutation {
                    version: 2,
                    binding,
                    operation_id: operation_id.into(),
                    command,
                };
                ("profile.mutate", json!({}), Some(mutation))
            }
            _ => return false,
        };
        let record = if matches!(
            action,
            "confirm" | "reject" | "deprecate" | "delete" | "switch_on" | "switch_off" | "revise"
        ) {
            self.operator.record().cloned()
        } else {
            None
        };
        self.operator.review = Some(Review {
            record,
            guard,
            method: method.into(),
            params,
            mutation,
        });
        true
    }
    pub fn operator_confirm(&mut self) -> Option<Review> {
        if self.operator.busy {
            return None;
        }
        let review = self.operator.review.take()?;
        if !self.operator_current(&review)
            || review
                .record
                .as_ref()
                .is_some_and(|r| self.operator.record() != Some(r))
        {
            return None;
        }
        // A locally revoked write grant refuses a prepared mutation before any
        // submit. An absent info (uncertain outcome) stays daemon-authoritative.
        if review.mutation.is_some()
            && self
                .operator
                .info
                .as_ref()
                .is_some_and(|i| !i.write_enabled)
        {
            return None;
        }
        // A destructive apply rechecks the arm at submit time: a cleared or
        // drifted retype between prepare and apply refuses instead of writing.
        if matches!(
            review.mutation.as_ref().map(|m| &m.command),
            Some(Command::DeletePreference { .. })
        ) && !review
            .record
            .as_ref()
            .is_some_and(|r| self.operator.delete_armed_for(r))
        {
            return None;
        }
        self.operator.busy = true;
        if review.mutation.is_some() {
            self.operator.receipt = None;
            self.operator.pending = Some(review.clone());
        }
        Some(review)
    }
    pub fn operator_retry(&mut self) -> Option<Review> {
        if self.operator.busy || !self.operator.outcome_unknown {
            return None;
        }
        let review = self.operator.pending.clone()?;
        if !self.operator_current(&review) {
            return None;
        }
        self.operator.busy = true;
        Some(review)
    }
}
