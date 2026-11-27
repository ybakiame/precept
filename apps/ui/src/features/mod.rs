//! Feature screens. Each composes the `ui` layer; none styles controls itself.
pub(crate) mod agents;
pub mod brand_preview;
pub(crate) mod connection;
pub(crate) mod doctor;
pub(crate) mod handoff;
pub(crate) mod memory;
pub(crate) mod onboarding;
pub(crate) mod overview;
pub(crate) mod personal_sync;
pub(crate) mod preferences;
pub(crate) mod profile;
pub(crate) mod projects;
pub(crate) mod review;
pub(crate) mod rule_lint;
pub(crate) mod sessions;
pub(crate) mod settings;
pub(crate) mod setup;
pub(crate) mod tags;
pub(crate) mod updates;

use crate::*;
use handoff::handoff;
use overview::{daemon, dashboard, next_step};
use precept_ui_core::Section;
use precept_ui_core::i18n::Msg;
use preferences::preferences;
use shell::*;
use tokens::compact;

/// What the Rules and Handoff pages show: each is rebuilt only when one of
/// these changes. Other sections keep rebuilding on any change (no long list).
#[derive(Clone, PartialEq)]
enum PageKey {
    Rules {
        phase: Phase,
        data: usize,
        project: Option<String>,
        filter: String,
        tags: Vec<String>,
        locale: precept_ui_core::i18n::Locale,
        live: bool,
    },
    Handoff {
        phase: Phase,
        data: usize,
        project: Option<String>,
        task: String,
        queried: String,
        saved: Option<(String, Vec<precept_ui_core::operator::HandoffSummary>)>,
        locale: precept_ui_core::i18n::Locale,
        live: bool,
        epoch: u64,
        status: (
            Option<u64>,
            Option<u64>,
            Option<&'static str>,
            bool,
            Option<&'static str>,
        ),
    },
    Other(u64),
}

#[cfg(test)]
mod page_tests;
/// How often the section page was built, per UI thread (tests only: tests
/// run in parallel, each app on its own engine thread).
#[cfg(test)]
pub(crate) static PAGE_BUILDS: std::sync::Mutex<std::collections::BTreeMap<String, usize>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// The current section's page, rebuilt only when what it shows changes. On
/// the Rules page a status, a read finishing or any other unrelated update
/// no longer rebuilds the page and its long, virtualized list.
pub(crate) fn section_page(model: Signal<Model>) -> impl IntoElement {
    let other = std::cell::Cell::new(0u64);
    let key = computed(move || {
        let m = model.get();
        if m.section == Section::Rules && m.phase == Phase::Ready {
            PageKey::Rules {
                phase: m.phase.clone(),
                data: m
                    .data
                    .as_ref()
                    .map_or(0, |d| std::sync::Arc::as_ptr(d) as usize),
                project: m.selected_project.clone(),
                filter: m.filter.clone(),
                tags: m.tag_filter.clone(),
                locale: m.locale,
                live: m.live,
            }
        } else if m.section == Section::Handoff && m.phase == Phase::Ready {
            PageKey::Handoff {
                phase: m.phase.clone(),
                data: m
                    .data
                    .as_ref()
                    .map_or(0, |d| std::sync::Arc::as_ptr(d) as usize),
                project: m.selected_project.clone(),
                task: m.task_id.clone(),
                queried: m.queried_task.clone(),
                saved: m.handoff_list.clone(),
                locale: m.locale,
                live: m.live,
                epoch: m.epoch,
                status: (
                    m.last_read_ms,
                    m.last_duration_ms,
                    m.read_failure,
                    m.permission_denied,
                    m.wsl_link,
                ),
            }
        } else {
            other.set(other.get() + 1);
            PageKey::Other(other.get())
        }
    });
    dynamic(move || {
        let _ = key.get();
        untracked(|| page(model).into_element())
    })
}

/// The current section's page (Dashboard, Preferences, Handoff, Diagnostics).
pub(crate) fn page(model: Signal<Model>) -> impl IntoElement {
    #[cfg(test)]
    {
        *PAGE_BUILDS
            .lock()
            .unwrap()
            .entry(format!("{:?}", std::thread::current().id()))
            .or_default() += 1;
    }
    let current = model.get();
    let body = view()
        .flex_shrink_0()
        .gap_3()
        .style_when(compact, |s| s.gap_2())
        .on_layout(move |_: &Bounds<Points>| {
            if model.get().phase == Phase::Ready && std::env::var_os("PRECEPT_UI_PROBE").is_some() {
                eprintln!("PRECEPT_NSH_LAYOUT_READY elapsed={:.6}", probe_elapsed());
            }
        });
    if current.phase != Phase::Ready {
        if current.section == Section::Dashboard
            && matches!(current.phase, Phase::Empty | Phase::Unavailable)
        {
            return body.child(next_step(model)).child(daemon(model));
        }
        return body.child(daemon(model)).child(card(
            model,
            current.read_label(),
            vec![current.status(), current.word(Msg::RetryHelp).into()],
        ));
    }
    if current.section == Section::Dashboard {
        return body.child(dashboard(model));
    }
    if current.section == Section::Rules {
        return body.child(preferences(model));
    }
    if current.section == Section::Handoff {
        return body.child(handoff(model));
    }
    let mut lines = current.visible_lines();
    if current.section == Section::Diagnostics {
        lines.extend(super::runtime_lines(current.locale));
        return body.child(
            view()
                .flex_row()
                .items_start()
                .gap_3()
                .style_when(compact, |s| s.flex_col().gap_2())
                .child(view().flex_1().min_w_0().child(daemon(model)))
                .child(view().flex_1().min_w_0().child(card(
                    model,
                    current.word(Msg::Diagnostics),
                    lines,
                ))),
        );
    }
    let title = match current.section {
        Section::Dashboard => "Selected project / repository snapshot",
        Section::Handoff => current.word(Msg::ResumeSummary),
        _ => current.word(Msg::Diagnostics),
    };
    body.child(card(model, title, lines))
}
