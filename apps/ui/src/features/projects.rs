//! Workspace: the daemon's projects, adding one and removing one.
//!
//! The folder is chosen with the operating system's own dialog (or typed) and
//! checked by the daemon (`project.check`); the GUI never reads the filesystem.
//! On Windows with a WSL daemon the dialog starts in the distribution's home and
//! a returned `\\wsl.localhost\<distro>\…` path maps to its Linux path; anything
//! else is refused before a request. Adding and removing need the Profile's
//! write grant; the daemon checks it again.
use crate::*;
use acp_operator_core::{
    ProjectEntry,
    connection::Target,
    i18n::Msg,
    projects::{Check, refusal, wsl_home, wsl_path},
};
use tokens::palette;
use ui::{
    BadgeVariant, ButtonSize, ButtonVariant, alert_dialog, badge, button, field, input, scale,
    surface,
};

fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}
fn note(
    model: Signal<Model>,
    text_fn: impl Fn(&Model) -> Option<Msg> + 'static,
) -> impl IntoElement {
    text(move || {
        let m = model.get();
        text_fn(&m).map(|k| m.word(k)).unwrap_or_default()
    })
    .text_size(px(scale::text::CAPTION))
    .text_color(move || palette(model).muted)
}

/// A project request through the current connection, off the UI thread.
fn call(
    model: Signal<Model>,
    method: &'static str,
    params: serde_json::Value,
    done: impl FnOnce(&mut Model, Result<serde_json::Value, &'static str>) + 'static,
) {
    #[cfg(feature = "ipc")]
    {
        let inputs = acp_operator_client::native::Inputs::from_connection(&model.get().connection);
        spawn(async move {
            let result = blocking(move || inputs.and_then(|i| i.project_call(method, params)))
                .await
                .unwrap_or(Err("Unavailable"));
            model.update(|m| done(m, result));
        });
    }
    #[cfg(not(feature = "ipc"))]
    {
        let _ = (method, params);
        model.update(|m| done(m, Err("Unavailable")));
    }
}

/// Whether this run's writes are on (adding or removing turns them on first).
fn writes_on(model: Signal<Model>) -> bool {
    model.get().project_add.write_enabled == Some(true)
}

fn check(model: Signal<Model>) {
    let path = model.get().project_add.path.clone();
    if path.trim().is_empty() {
        return;
    }
    let mut epoch = 0;
    model.update(|m| epoch = m.project_add.begin_check());
    call(
        model,
        "project.check",
        serde_json::json!({"root": path}),
        move |m, r| m.project_add.finish_check(epoch, r),
    );
}

/// Opens the system folder dialog where the daemon's home is, then checks.
fn choose(model: Signal<Model>) {
    let m = model.get();
    let wsl = m.connection.target == Target::Wsl;
    let distribution = m.connection.distribution.clone();
    let start = if wsl {
        Some(wsl_home(&distribution, &m.connection.user))
    } else {
        std::env::var("HOME").ok()
    };
    let title = m.word(Msg::ChooseFolder).trim_end_matches('…').to_owned();
    spawn(async move {
        let mut dialog = nsh_core::file_dialog::OpenDialog::folders().title(title);
        if let Some(start) = start {
            dialog = dialog.directory(start);
        }
        match pick_paths(dialog).await {
            Ok(Some(paths)) => {
                let Some(picked) = paths.first().map(|p| p.to_string_lossy().into_owned()) else {
                    return;
                };
                let mapped = if wsl {
                    wsl_path(&distribution, &picked)
                } else {
                    Ok(picked.clone())
                };
                match mapped {
                    Ok(path) => {
                        model.update(|m| m.project_add.set_path(path));
                        check(model);
                    }
                    Err(code) => model.update(|m| {
                        m.project_add.set_path(picked);
                        m.project_add.check = Check::Refused(code.into());
                    }),
                }
            }
            Ok(None) => {}
            Err(_) => model.update(|m| m.project_add.error = Some("dialog".into())),
        }
    });
}

/// Add; with writes off, the operator's one click turns them on for this run
/// first (the label says so), then adds.
fn add(model: Signal<Model>) {
    if model.get().project_add.ready().is_none() {
        return;
    }
    if writes_on(model) {
        add_now(model);
    } else {
        super::profile::enable_writes(model, add_now);
    }
}
fn add_now(model: Signal<Model>) {
    let Some((name, root)) = model.get().project_add.ready() else {
        return;
    };
    model.update(|m| {
        m.project_add.busy = true;
        m.project_add.error = None;
    });
    call(
        model,
        "project.add",
        serde_json::json!({"name": name, "root": root}),
        move |m, r| {
            m.project_add.busy = false;
            match r {
                Ok(_) => m.project_add.reset(),
                Err(code) => m.project_add.error = Some(code.into()),
            }
        },
    );
    reload_after(model);
}

fn remove(model: Signal<Model>) {
    if model.get().project_remove.is_none() {
        return;
    }
    if writes_on(model) {
        remove_now(model);
    } else {
        super::profile::enable_writes(model, remove_now);
    }
}
fn remove_now(model: Signal<Model>) {
    let Some(name) = model.get().project_remove.clone() else {
        return;
    };
    model.update(|m| m.project_add.busy = true);
    call(
        model,
        "project.remove",
        serde_json::json!({"name": name}),
        move |m, r| {
            m.project_add.busy = false;
            m.project_remove = None;
            if let Err(code) = r {
                m.project_add.error = Some(code.into());
            }
        },
    );
    reload_after(model);
}

/// Re-reads once the change settled (the list comes from the daemon).
fn reload_after(model: Signal<Model>) {
    spawn(async move {
        for _ in 0..100 {
            if !model.get_untracked().project_add.busy {
                break;
            }
            timer(std::time::Duration::from_millis(50)).await;
        }
        load(model);
    });
}

fn state_badge(model: Signal<Model>, state: &str) -> impl IntoElement {
    let (key, variant) = match state {
        "ready" => (Msg::ProjectReady, BadgeVariant::Success),
        "unregistered" => (Msg::ProjectUnregistered, BadgeVariant::Neutral),
        _ => (Msg::ProjectUnavailable, BadgeVariant::Warning),
    };
    badge(model, model.get_untracked().word(key)).variant(variant)
}

/// A served project's name opens it (in-app navigation is a button); the
/// others are plain text.
fn name_cell(model: Signal<Model>, entry: &ProjectEntry) -> AnyElement {
    let label = acp_operator_core::presentation::sanitize(&entry.name, 64);
    if entry.state != "ready" {
        return text(label)
            .font_weight(FontWeight::MEDIUM)
            .text_size(px(scale::text::BODY))
            .into_element();
    }
    let name = entry.name.clone();
    view()
        .items_start()
        .child(
            button(model, label)
                .variant(ButtonVariant::Ghost)
                .selected({
                    let name = name.clone();
                    move || model.get().project().is_some_and(|p| p.name == name)
                })
                .on_press(move || {
                    let index = model.get().project_names.iter().position(|n| *n == name);
                    if let Some(index) = index {
                        switch_project(model, index);
                    }
                }),
        )
        .into_element()
}

/// The list of projects: name, root, state and Remove, on fixed lanes.
fn list(model: Signal<Model>) -> impl IntoElement {
    let entries = computed(move || {
        let m = model.get();
        let live = m.live;
        let entries: Vec<ProjectEntry> = match &m.data {
            Some(d) if !d.directory.is_empty() => d.directory.clone(),
            Some(d) => d
                .projects
                .iter()
                .map(|p| ProjectEntry {
                    name: p.name.clone(),
                    root: String::new(),
                    state: "ready".into(),
                })
                .collect(),
            None => Vec::new(),
        };
        (live, entries)
    });
    dynamic(move || {
        let (live, entries) = entries.get();
        if entries.is_empty() {
            return note(model, |_| Some(Msg::NoProjectsYet)).into_element();
        }
        view()
            .gap_1()
            .children(entries.into_iter().map(move |entry| {
                let name = entry.name.clone();
                let mut row = view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .min_h(px(scale::control::LG))
                    .child(
                        view()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(name_cell(model, &entry))
                            .child(
                                text(acp_operator_core::presentation::sanitize(&entry.root, 512))
                                    .text_size(px(scale::text::CAPTION))
                                    .text_color(move || palette(model).muted),
                            ),
                    )
                    .child(state_badge(model, &entry.state));
                if live {
                    row = row.child(
                        button(model, "")
                            .label_fn(word(model, Msg::RemoveProject))
                            .spoken({
                                let name = entry.name.clone();
                                move || {
                                    format!("{} {name}", model.get().word(Msg::RemoveConfirm))
                                        .into()
                                }
                            })
                            .size(ButtonSize::Small)
                            .on_press(move || {
                                model.update(|m| m.project_remove = Some(name.clone()))
                            }),
                    );
                }
                row
            }))
            .into_element()
    })
}

/// The add form: folder (dialog or typed), the daemon's check, the name, Add.
fn add_form(model: Signal<Model>) -> impl IntoElement {
    let checked = computed(move || model.get().project_add.check.clone());
    let status = dynamic(move || {
        let (key, variant) = match checked.get() {
            Check::Idle => return view().into_element(),
            Check::Checking => (Msg::Checking, BadgeVariant::Neutral),
            Check::Ok { known: true, .. } => (Msg::FolderKnown, BadgeVariant::Info),
            Check::Ok { .. } => (Msg::FolderOk, BadgeVariant::Success),
            Check::Refused(code) => (refusal(&code), BadgeVariant::Warning),
            Check::Failed(code) => (refusal(code), BadgeVariant::Danger),
        };
        view()
            .items_start()
            .child(badge(model, model.get_untracked().word(key)).variant(variant))
            .into_element()
    });
    let folder = view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            view().flex_1().min_w_0().child(
                input(model, "")
                    .label_fn(word(model, Msg::ProjectFolder))
                    .value(move || model.get().project_add.path.clone())
                    .on_change(move |v: &str| model.update(|m| m.project_add.set_path(v.into())))
                    .on_submit(move |_: &str| check(model)),
            ),
        )
        .child(
            button(model, "")
                .label_fn(word(model, Msg::ChooseFolder))
                .on_press(move || choose(model)),
        )
        .child(
            button(model, "")
                .label_fn(word(model, Msg::CheckFolder))
                .disabled(move || model.get().project_add.path.trim().is_empty())
                .loading(move || model.get().project_add.check == Check::Checking)
                .on_press(move || check(model)),
        );
    let has_profile = move || model.get().project_add.profile_selected == Some(true);
    surface(model).title_fn(word(model, Msg::AddProject)).child(
        view()
            .gap_4()
            .child(
                field(model, word(model, Msg::ProjectFolder), folder)
                    .help(word(model, Msg::ProjectFolderHelp)),
            )
            .child(status)
            .child(
                field(
                    model,
                    word(model, Msg::ProjectName),
                    input(model, "")
                        .label_fn(word(model, Msg::ProjectName))
                        .max_length(64)
                        .value(move || model.get().project_add.name.clone())
                        .invalid(move || {
                            let n = &model.get().project_add.name;
                            !n.is_empty() && !acp_operator_core::projects::valid_name(n)
                        })
                        .on_change(move |v: &str| {
                            model.update(|m| m.project_add.set_name(v.into()))
                        })
                        .on_submit(move |_: &str| add(model)),
                )
                .help(word(model, Msg::ProjectNameHelp)),
            )
            .child(note(model, |m| {
                m.project_add.error.as_deref().map(|e| {
                    if e == "dialog" {
                        Msg::DialogUnavailable
                    } else {
                        refusal(e)
                    }
                })
            }))
            .child(note(model, move |m| match m.project_add.profile_selected {
                Some(false) => Some(Msg::NeedsProfile),
                Some(true) if m.project_add.write_enabled != Some(true) => Some(Msg::NeedsWrite),
                _ => None,
            }))
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        button(model, "")
                            .label_fn(move || {
                                model
                                    .get()
                                    .word(if writes_on(model) {
                                        Msg::AddConfirm
                                    } else {
                                        Msg::EnableAndAdd
                                    })
                                    .into()
                            })
                            .variant(ButtonVariant::Primary)
                            .disabled(move || {
                                !has_profile()
                                    || model.get().project_add.ready().is_none()
                                    || model.get().profile_setup.busy
                            })
                            .loading(move || model.get().project_add.busy)
                            .on_press(move || add(model)),
                    )
                    .child(
                        button(model, "")
                            .label_fn(word(model, Msg::Cancel))
                            .on_press(move || model.update(|m| m.project_add.reset())),
                    ),
            ),
    )
}

/// The Workspace page (mounted by the shell, outside the per-change page
/// rebuild, so its fields keep focus while typing).
pub(crate) fn page(model: Signal<Model>) -> impl IntoElement {
    view()
        .gap_6()
        .max_w(px(scale::layout::FORM))
        .child(list(model))
        .child(note(model, |m| (!m.live).then_some(Msg::DemoProjects)))
        .child(
            view()
                .items_start()
                .style_when(
                    move || {
                        let m = model.get();
                        !m.live || m.project_add.open
                    },
                    |s| s.hidden(),
                )
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::AddProject))
                        .on_press(move || model.update(|m| m.project_add.open = true)),
                ),
        )
        .child(
            view()
                .style_when(move || !model.get().project_add.open, |s| s.hidden())
                .child(add_form(model)),
        )
}

/// The removal confirmation; a sibling of the page's scroll region.
pub(crate) fn remove_dialog(model: Signal<Model>) -> impl IntoElement {
    alert_dialog(model, "")
        .title_fn(word(model, Msg::RemoveTitle))
        .description_fn(move || {
            let m = model.get();
            format!(
                "{} — {}",
                m.project_remove.clone().unwrap_or_default(),
                m.word(Msg::RemoveBody)
            )
            .into()
        })
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(move || {
            model
                .get()
                .word(if writes_on(model) {
                    Msg::RemoveConfirm
                } else {
                    Msg::EnableAndRemove
                })
                .into()
        })
        .confirm_disabled(move || model.get().project_add.busy)
        .open(move || model.get().project_remove.is_some())
        .on_cancel(move || model.update(|m| m.project_remove = None))
        .on_confirm(move || remove(model))
}
