//! Profile-scoped personal-sync workflow. Pure state only: files and IPC live
//! in the client/UI layers, and the daemon remains the only writer.

use crate::{Model, connection::Connection, operator::*};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, PartialEq, Eq)]
pub struct Guard {
    connection: Connection,
    connection_epoch: u64,
    profile_epoch: u64,
    profile_id: Option<String>,
    generation: Option<String>,
}

impl Guard {
    pub fn capture(model: &Model) -> Self {
        let binding = model
            .profile_info
            .as_ref()
            .and_then(|info| info.binding.as_ref());
        Self {
            connection: model.connection.clone(),
            connection_epoch: model.connection_epoch,
            profile_epoch: model.profile_epoch,
            profile_id: binding.map(|binding| binding.profile_id.clone()),
            generation: binding.map(|binding| binding.generation.clone()),
        }
    }

    pub fn matches(&self, model: &Model) -> bool {
        let current = Self::capture(model);
        self == &current
    }

    pub fn scope(&self) -> Option<PersonalSyncProfileScope> {
        Some(PersonalSyncProfileScope {
            profile_id: self.profile_id.clone()?,
            generation: self.generation.clone()?,
        })
    }
}

impl std::fmt::Debug for Guard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SyncGuard(redacted)")
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct ExportDraft {
    pub passphrase: Zeroizing<String>,
    pub output_path: String,
    pub show_password: bool,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct ImportDraft {
    pub envelope_path: String,
    pub passphrase: Zeroizing<String>,
}

impl std::fmt::Debug for ExportDraft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ExportDraft(redacted)")
    }
}

impl std::fmt::Debug for ImportDraft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ImportDraft(redacted)")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Dialog {
    #[default]
    None,
    Apply,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormAction {
    Export,
    Preview,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConflictChoice {
    #[default]
    Skip,
    Accept,
    KeepLocal,
    Manual,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ManualDraft {
    pub value: String,
    pub tags: String,
    pub enabled: bool,
    pub rule: RuleOptions,
}

#[derive(Clone, PartialEq, Eq)]
pub struct PreviewState {
    pub preview: PersonalSyncPreview,
    pub envelope: PersonalSyncEnvelope,
    pub passphrase: Option<Zeroizing<String>>,
    pub guard: Guard,
    pub operation_id: String,
    selected: BTreeSet<String>,
    conflicts: BTreeMap<String, ConflictChoice>,
    manual: BTreeMap<String, ManualDraft>,
}

fn clear_content(content: &mut PersonalSyncContent) {
    content.value.zeroize();
    for tag in &mut content.tags {
        tag.zeroize();
    }
}

impl PreviewState {
    pub fn new(
        preview: PersonalSyncPreview,
        envelope: PersonalSyncEnvelope,
        passphrase: Option<String>,
        guard: Guard,
        operation_id: String,
    ) -> Self {
        let selected = preview
            .items
            .iter()
            .filter(|item| {
                !item.applied
                    && !matches!(item.merge, PersonalSyncMerge::Conflict)
                    && !matches!(
                        item.merge,
                        PersonalSyncMerge::Stale | PersonalSyncMerge::Noop
                    )
            })
            .map(|item| item.key.clone())
            .collect();
        Self {
            preview,
            envelope,
            passphrase: passphrase.map(Zeroizing::new),
            guard,
            operation_id,
            selected,
            conflicts: BTreeMap::new(),
            manual: BTreeMap::new(),
        }
    }

    pub fn selected(&self, key: &str) -> bool {
        self.selected.contains(key)
    }

    pub fn set_selected(&mut self, key: &str, selected: bool) {
        if self.preview.items.iter().any(|item| {
            item.key == key && (item.applied || item.merge == PersonalSyncMerge::Conflict)
        }) {
            return;
        }
        if selected {
            self.selected.insert(key.to_owned());
        } else {
            self.selected.remove(key);
        }
    }

    pub fn choice(&self, key: &str) -> ConflictChoice {
        self.conflicts.get(key).copied().unwrap_or_default()
    }

    pub fn set_choice(&mut self, key: &str, choice: ConflictChoice) {
        if self
            .preview
            .items
            .iter()
            .any(|item| item.key == key && item.merge == PersonalSyncMerge::Conflict)
        {
            self.conflicts.insert(key.to_owned(), choice);
        }
    }

    pub fn manual_draft(&self, key: &str) -> ManualDraft {
        self.manual.get(key).cloned().unwrap_or_else(|| {
            let incoming = self
                .preview
                .bundle
                .entries
                .iter()
                .find(|entry| entry.content.key == key)
                .map(|entry| entry.content.clone());
            ManualDraft {
                value: incoming
                    .as_ref()
                    .map_or_else(String::new, |content| content.value.clone()),
                tags: incoming
                    .as_ref()
                    .map_or_else(|| "sync".into(), |content| content.tags.join(", ")),
                enabled: incoming.as_ref().is_none_or(|content| content.enabled),
                rule: incoming
                    .as_ref()
                    .map_or_else(RuleOptions::default, |content| content.rule.clone()),
            }
        })
    }

    pub fn set_manual(&mut self, key: &str, draft: ManualDraft) {
        if self.choice(key) == ConflictChoice::Manual {
            self.manual.insert(key.to_owned(), draft);
        }
    }

    pub fn manual_content(&self, key: &str) -> Option<PersonalSyncContent> {
        let draft = self.manual_draft(key);
        let raw: Vec<&str> = draft
            .tags
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .collect();
        let tags = canonical_tags(&raw)?;
        let content = PersonalSyncContent {
            key: key.to_owned(),
            value: draft.value,
            tags,
            rule: draft.rule,
            enabled: draft.enabled,
        };
        content.valid().then_some(content)
    }

    pub fn selections(&self) -> Option<Vec<PersonalSyncSelection>> {
        let mut selections = Vec::new();
        for selection in &self.preview.selections {
            let item = self
                .preview
                .items
                .iter()
                .find(|item| item.key == selection.key)?;
            if item.applied {
                continue;
            }
            let mut selection = selection.clone();
            if item.merge == PersonalSyncMerge::Conflict {
                selection.resolution = match self.choice(&item.key) {
                    ConflictChoice::Skip => continue,
                    ConflictChoice::Accept => PersonalSyncResolution::Accept,
                    ConflictChoice::KeepLocal => PersonalSyncResolution::KeepLocal,
                    ConflictChoice::Manual => {
                        PersonalSyncResolution::Manual(self.manual_content(&item.key)?)
                    }
                };
            } else if !self.selected(&item.key) {
                continue;
            }
            if !selection.valid() {
                return None;
            }
            selections.push(selection);
        }
        (!selections.is_empty()).then_some(selections)
    }

    pub fn selected_count(&self) -> usize {
        self.selections().map_or(0, |selections| selections.len())
    }

    pub fn prepare(&self) -> Option<ApplyState> {
        let selections = self.selections()?;
        Some(ApplyState {
            submission: PersonalSyncApplySubmission {
                operation_id: self.operation_id.clone(),
                expected_commit: self.preview.expected_commit,
                channel: Channel::Desktop,
                envelope: self.envelope.clone(),
                passphrase: None,
                local_versions: self.preview.local_versions.clone(),
                sender_device_id: self.preview.sender_device_id.clone(),
                device_id: self.preview.device_id.clone(),
                selections,
            },
            passphrase: self.passphrase.clone(),
            guard: self.guard.clone(),
            uncertain: false,
            error: None,
            result: None,
            abort: None,
        })
    }

    fn clear_secrets(&mut self) {
        self.passphrase = None;
        for entry in &mut self.preview.bundle.entries {
            clear_content(&mut entry.content);
        }
        for content in &mut self.preview.local_contents {
            clear_content(content);
        }
        for draft in self.manual.values_mut() {
            draft.value.zeroize();
            draft.tags.zeroize();
        }
    }
}

impl Drop for PreviewState {
    fn drop(&mut self) {
        self.clear_secrets();
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplyState {
    pub submission: PersonalSyncApplySubmission,
    pub passphrase: Option<Zeroizing<String>>,
    pub guard: Guard,
    pub uncertain: bool,
    pub error: Option<String>,
    pub result: Option<PersonalSyncApplyResult>,
    pub abort: Option<PersonalSyncAbortResult>,
}

impl ApplyState {
    pub fn params(&self) -> Value {
        let mut submission = self.submission.clone();
        submission.passphrase = self
            .passphrase
            .as_ref()
            .map(|passphrase| passphrase.as_str().to_owned());
        serde_json::to_value(submission).unwrap_or(Value::Null)
    }

    pub fn retryable(&self, model: &Model) -> bool {
        (self.uncertain || self.error.is_none())
            && self.result.is_none()
            && self.abort.is_none()
            && self.guard.matches(model)
    }

    pub fn succeed(&mut self, result: PersonalSyncApplyResult) {
        self.clear_secrets();
        self.result = Some(result);
        self.uncertain = false;
        self.error = None;
    }

    pub fn refuse(&mut self, code: &'static str) {
        self.clear_secrets();
        self.error = Some(code.into());
        self.uncertain = false;
    }

    pub fn settle_abort(&mut self, abort: PersonalSyncAbortResult) {
        self.clear_secrets();
        self.abort = Some(abort);
        self.uncertain = false;
        self.error = None;
    }

    fn clear_secrets(&mut self) {
        self.passphrase = None;
        self.submission.passphrase = None;
        for selection in &mut self.submission.selections {
            if let PersonalSyncResolution::Manual(content) = &mut selection.resolution {
                clear_content(content);
            }
        }
        self.submission.selections.clear();
    }
}

impl Drop for ApplyState {
    fn drop(&mut self) {
        self.clear_secrets();
    }
}

#[derive(Clone, Default, PartialEq)]
pub struct State {
    pub status: Option<PersonalSyncStatus>,
    pub status_read: bool,
    pub busy: bool,
    pub form_action: Option<FormAction>,
    pub error: Option<String>,
    pub serial: u64,
    pub guard: Option<Guard>,
    pub export: ExportDraft,
    pub import: ImportDraft,
    pub dialog: Dialog,
    pub dialog_generation: u64,
    pub preview: Option<PreviewState>,
    pub apply: Option<ApplyState>,
}

impl State {
    pub fn invalidate_for(&mut self, model: &Model) {
        self.invalidate(Guard::capture(model));
    }

    pub fn invalidate(&mut self, guard: Guard) {
        if self.guard.as_ref() == Some(&guard) {
            return;
        }
        self.clear();
        self.guard = Some(guard);
    }

    pub fn clear(&mut self) {
        let serial = self.serial;
        *self = Self {
            serial,
            ..Self::default()
        };
    }

    pub fn begin(&mut self) -> u64 {
        self.serial = self.serial.wrapping_add(1);
        self.busy = true;
        self.error = None;
        self.serial
    }

    pub fn finish(
        &mut self,
        serial: u64,
        origin_guard: &Guard,
        current_guard: &Guard,
        result: Result<Value, &'static str>,
    ) -> bool {
        if serial != self.serial
            || self.guard.as_ref() != Some(origin_guard)
            || origin_guard != current_guard
        {
            return false;
        }
        self.busy = false;
        match result {
            Ok(value) => {
                self.error = None;
                Some(value)
            }
            Err(code) => {
                self.error = Some(code.to_owned());
                None
            }
        }
        .is_some()
    }

    pub fn accept_status(&mut self, value: Value) -> bool {
        let Some(status) = serde_json::from_value(value).ok() else {
            return false;
        };
        self.status = Some(status);
        self.status_read = true;
        true
    }

    pub fn close_dialog(&mut self) {
        self.dialog_generation = self.dialog_generation.wrapping_add(1);
        self.dialog = Dialog::None;
    }

    pub fn dialog_is_current(&self, guard: &Guard, generation: u64, expected: &Dialog) -> bool {
        self.guard.as_ref() == Some(guard)
            && self.dialog_generation == generation
            && std::mem::discriminant(&self.dialog) == std::mem::discriminant(expected)
    }

    pub fn discard_preview(&mut self) {
        self.preview = None;
    }
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PersonalSyncState(redacted)")
    }
}

#[cfg(test)]
mod tests;
