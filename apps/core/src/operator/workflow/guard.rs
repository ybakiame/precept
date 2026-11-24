use super::*;

impl Model {
    pub fn operator_scope(&self) -> Option<Scope> {
        if self.operator.global {
            Some(Scope::Global)
        } else {
            Some(Scope::Project {
                name: self.selected_project.clone()?,
            })
        }
    }
    pub fn operator_guard(&self) -> Option<Guard> {
        // Local and WSL targets get identical epoch/connection/source fences.
        (matches!(self.connection.target, Target::Local | Target::Wsl)
            && self.connection.validate())
        .then(|| Guard {
            epoch: self.epoch,
            connection: self.connection.clone(),
            project: self.selected_project.clone(),
            info: self.operator.info.clone(),
            selected_id: self.operator.selected_id,
            source_snapshot: self.data.clone(),
            source_available: matches!(self.phase, crate::Phase::Ready | crate::Phase::Empty),
            source_live: self.live,
            permission_denied: self.permission_denied,
            open: self.operator.open,
            queue_sequence: self.operator.queue.as_ref().map(|q| q.commit_sequence),
            task: self.operator.task.clone(),
            context_task: self.task_id.clone(),
            queried_task: self.queried_task.clone(),
            scope: self.operator_scope().unwrap_or(Scope::Global),
            after_id: self.operator.after_id,
            queue_after_id: self.operator.queue_after_id,
            queue_watermark: self.operator.queue_watermark,
            inspect_revision: self.operator.inspect_revision,
            inspect_record: self.operator.inspect.as_ref().map(|i| i.record.clone()),
            serial: self.operator.serial,
        })
    }
    /// Whether the exact inspection still belongs to the current selection,
    /// authority and observed sources. A false value requires a fresh inspect.
    /// The draft's serial is not part of it: editing a draft (an edit of this
    /// very record) does not change what was inspected.
    pub fn operator_inspected(&self) -> bool {
        self.operator.inspection.as_ref().is_some_and(|inspected| {
            self.operator_guard().is_some_and(|mut guard| {
                guard.serial = inspected.serial;
                guard == *inspected
            })
        })
    }
    pub fn operator_current(&self, review: &Review) -> bool {
        let Some(mut guard) = self.operator_guard() else {
            return false;
        };
        // OutcomeUnknown hides live grant controls, but keeps the captured
        // authority for exact manual replay only. A newly observed grant or
        // binding always wins; epoch/serial/connection/source fences still apply.
        if self.operator.outcome_unknown
            && self.operator.info.is_none()
            && let Some(pending) = &self.operator.pending
        {
            guard.info = pending.guard.info.clone();
        }
        guard == review.guard
    }
}
