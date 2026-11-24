use crate::load;
use nsh_core::prelude::*;
use precept_ui_core::{Model, Phase};

pub(super) fn next_project(model: Signal<Model>) {
    let next = (model.get().selected + 1) % model.get().project_names.len().max(1);
    switch_project(model, next);
}
pub(super) fn switch_project(model: Signal<Model>, index: usize) {
    model.update(|m| {
        m.select(index);
        m.task_id.clear();
        m.queried_task.clear();
        if m.live {
            m.phase = Phase::Loading;
        }
    });
    if model.get().live || model.get().phase == Phase::Loading {
        load(model);
    } else {
        model.update(|m| m.epoch += 1);
    }
}
pub(super) fn reconnect(model: Signal<Model>) {
    load(model);
}
