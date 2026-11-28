//! Workspace: the daemon's projects, adding, editing (display name, moved
//! folder) and removing one.
//!
//! The folder is chosen with the operating system's own dialog (or typed) and
//! checked by the daemon (`project.check`); the GUI never reads the filesystem.
//! On Windows with a WSL daemon the dialog starts in the distribution's home and
//! a returned `\\wsl.localhost\<distro>\…` path maps to its Linux path; anything
//! else is refused before a request. Adding, moving and removing need the
//! Profile's write grant; the daemon checks it again. A display name is the
//! operator's presentation only: the name stays the project's identity.
use crate::*;
use precept_ui_core::{
    ProjectEntry,
    connection::Target,
    i18n::Msg,
    projects::{Check, EditDraft, refusal, valid_label, wsl_home, wsl_path},
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
    calls(model, vec![(method, params)], done);
}

/// Project requests in order, stopping at the first refusal, which `done`
/// receives; otherwise the last reply.
fn calls(
    model: Signal<Model>,
    requests: Vec<(&'static str, serde_json::Value)>,
    done: impl FnOnce(&mut Model, Result<serde_json::Value, &'static str>) + 'static,
) {
    #[cfg(feature = "ipc")]
    {
        let inputs = precept_ui_client::native::Inputs::from_connection(&model.get().connection);
        // Detached: a call started from a dialog must settle after the dialog
        // closes (closing disposes the dialog's owner and its tasks).
        Owner::detached().run(|| {
            spawn(async move {
                let result = blocking(move || {
                    let inputs = inputs?;
                    let mut last = Ok(serde_json::Value::Null);
                    for (method, params) in requests {
                        last = Ok(inputs.project_call(method, params)?);
                    }
                    last
                })
                .await
                .unwrap_or(Err("Unavailable"));
                model.update(|m| done(m, result));
            });
        });
    }
    #[cfg(not(feature = "ipc"))]
    {
        let _ = requests;
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
    pick_folder(model, |model, picked| match picked {
        Ok(path) => {
            model.update(|m| m.project_add.set_path(path));
            check(model);
        }
        Err((picked, code)) => model.update(|m| {
            m.project_add.set_path(picked);
            m.project_add.check = Check::Refused(code.into());
        }),
    });
}

/// The system folder dialog, starting in the daemon's home. `done` gets the
/// daemon-host path, or the picked path and why it cannot be one.
fn pick_folder(
    model: Signal<Model>,
    done: impl FnOnce(Signal<Model>, Result<String, (String, &'static str)>) + 'static,
) {
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
                done(model, mapped.map_err(|code| (picked, code)));
            }
            Ok(None) => {}
            Err(_) => model.update(|m| {
                m.project_add.error = Some("dialog".into());
                if let Some(edit) = m.project_edit.as_mut() {
                    edit.error = Some("dialog".into());
                }
            }),
        }
    });
}

/// Add; with writes off, the operator's one click turns them on first (the
/// label says so), then adds.
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
                Ok(_) => {
                    m.project_add.reset();
                    ui::notify(
                        ui::Tone::Success,
                        m.word(Msg::ToastProjectAdded).replace("{name}", &name),
                    );
                }
                Err(code) => m.project_add.error = Some(code.into()),
            }
        },
    );
    reload_after(model);
}

/// Records a listed project in the selected Profile; with writes off, the
/// one click turns them on first, as Add does.
fn register(model: Signal<Model>, name: String) {
    if writes_on(model) {
        register_now(model, name);
    } else {
        super::profile::enable_writes(model, move |model| register_now(model, name));
    }
}
fn register_now(model: Signal<Model>, name: String) {
    model.update(|m| {
        m.project_add.busy = true;
        m.project_add.error = None;
    });
    call(
        model,
        "project.register",
        serde_json::json!({"name": name}),
        move |m, r| {
            m.project_add.busy = false;
            match r {
                Ok(_) => ui::notify(
                    ui::Tone::Success,
                    m.word(Msg::ToastProjectRegistered).replace("{name}", &name),
                ),
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
            match r {
                Ok(_) => ui::notify(
                    ui::Tone::Success,
                    m.word(Msg::ToastProjectRemoved).replace("{name}", &name),
                ),
                Err(code) => m.project_add.error = Some(code.into()),
            }
        },
    );
    reload_after(model);
}

/// Opens the edit dialog for one listed project.
fn edit(model: Signal<Model>, entry: &ProjectEntry) {
    let draft = EditDraft::open(entry);
    model.update(|m| m.project_edit = Some(draft));
}
fn edit_update(model: Signal<Model>, f: impl FnOnce(&mut EditDraft)) {
    model.update(|m| {
        if let Some(edit) = m.project_edit.as_mut() {
            f(edit);
        }
    });
}
fn edit_check(model: Signal<Model>) {
    let Some(path) = model.get().project_edit.as_ref().map(|e| e.path.clone()) else {
        return;
    };
    if path.trim().is_empty() {
        return;
    }
    let mut epoch = 0;
    edit_update(model, |e| epoch = e.begin_check());
    call(
        model,
        "project.check",
        serde_json::json!({"root": path.trim()}),
        move |m, r| {
            if let Some(e) = m.project_edit.as_mut() {
                e.finish_check(epoch, r);
            }
        },
    );
}
fn edit_choose(model: Signal<Model>) {
    pick_folder(model, |model, picked| match picked {
        Ok(path) => {
            edit_update(model, |e| e.set_path(path));
            edit_check(model);
        }
        Err((picked, code)) => edit_update(model, |e| {
            e.set_path(picked);
            e.check = Check::Refused(code.into());
        }),
    });
}
/// Whether saving moves the project (an operator write).
fn edit_moves(model: Signal<Model>) -> bool {
    model
        .get()
        .project_edit
        .as_ref()
        .is_some_and(|e| e.relocation().is_some())
}
/// Save; a move with writes off turns them on first (the label says so).
fn save(model: Signal<Model>) {
    if !model
        .get()
        .project_edit
        .as_ref()
        .is_some_and(EditDraft::ready)
    {
        return;
    }
    if edit_moves(model) && !writes_on(model) {
        super::profile::enable_writes(model, save_now);
    } else {
        save_now(model);
    }
}
fn save_now(model: Signal<Model>) {
    let Some(draft) = model.get().project_edit.clone() else {
        return;
    };
    let name = draft.name.clone();
    let label = draft.label_change();
    let root = draft.relocation();
    let mut requests = Vec::new();
    if let Some(label) = &label {
        requests.push((
            "project.label",
            serde_json::json!({"name": name, "label": label}),
        ));
    }
    if let Some(root) = &root {
        requests.push((
            "project.relocate",
            serde_json::json!({"name": name, "root": root}),
        ));
    }
    if requests.is_empty() {
        return;
    }
    edit_update(model, |e| {
        e.busy = true;
        e.error = None;
    });
    let shown = label
        .clone()
        .unwrap_or_else(|| draft.saved_label.clone())
        .unwrap_or_else(|| name.clone());
    let moved = root.is_some();
    calls(model, requests, move |m, r| match r {
        Ok(_) => {
            m.project_edit = None;
            let key = if moved {
                Msg::ToastProjectMoved
            } else {
                Msg::ToastProjectSaved
            };
            ui::notify(ui::Tone::Success, m.word(key).replace("{name}", &shown));
        }
        Err(code) => {
            if let Some(e) = m.project_edit.as_mut() {
                e.busy = false;
                e.error = Some(code.into());
            }
        }
    });
    reload_after(model);
}

/// Re-reads once the change settled (the list comes from the daemon).
fn reload_after(model: Signal<Model>) {
    // Detached for the same reason as `call`: Remove starts it from a dialog.
    Owner::detached().run(|| {
        spawn(async move {
            for _ in 0..100 {
                let m = model.get_untracked();
                if !m.project_add.busy && m.project_edit.as_ref().is_none_or(|e| !e.busy) {
                    break;
                }
                timer(std::time::Duration::from_millis(50)).await;
            }
            load(model);
        });
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

/// Workspace names are management rows only; switching the active project
/// lives solely in the sidebar workspace switcher. The display name leads.
fn name_cell(entry: &ProjectEntry) -> AnyElement {
    let shown = entry.label.as_deref().unwrap_or(&entry.name);
    text(precept_ui_core::presentation::sanitize(shown, 64))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(scale::text::BODY))
        .into_element()
}
/// The name (when a display name hides it) and the folder.
fn detail_line(entry: &ProjectEntry) -> Option<String> {
    use precept_ui_core::presentation::sanitize;
    let root = (!entry.root.is_empty()).then(|| sanitize(&entry.root, 512));
    match (&entry.label, root) {
        (Some(_), Some(root)) => Some(format!("{} · {root}", sanitize(&entry.name, 64))),
        (Some(_), None) => Some(sanitize(&entry.name, 64)),
        (None, root) => root,
    }
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
                    label: None,
                    root: String::new(),
                    state: "ready".into(),
                    agents: true,
                    propose: false,
                    gate: false,
                })
                .collect(),
            None => Vec::new(),
        };
        (live, entries, m.locale)
    });
    surface(model)
        .title_fn(word(model, Msg::ManagedWorkspaces))
        .child(note(model, |_| Some(Msg::WorkspacesIntro)))
        .child(dynamic(move || {
            let (live, entries, _) = entries.get();
            if entries.is_empty() {
                return ui::empty(model, model.get_untracked().word(Msg::NoProjectsYet))
                    .icon(icons::section_path(precept_ui_core::Section::Projects))
                    .description(model.get_untracked().word(Msg::NextStepSetupBody))
                    .action(
                        button(model, "")
                            .label_fn(word(model, Msg::AddProject))
                            .icon(icons::PLUS)
                            .variant(ButtonVariant::Primary)
                            .on_press(move || model.update(|m| m.project_add.open = true)),
                    )
                    .into_element();
            }
            view()
                .children(entries.into_iter().enumerate().map(move |(index, entry)| {
                    let name = entry.name.clone();
                    let mut row = view()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .py_2()
                        .min_h(px(scale::control::LG))
                        .style_when(move || index > 0, |s| s.border_t_1())
                        .border_color(move || palette(model).hairline)
                        .child({
                            let mut info =
                                view().flex_1().min_w_0().gap_0p5().child(name_cell(&entry));
                            if let Some(detail) = detail_line(&entry) {
                                info = info.child(
                                    text(detail)
                                        .font_family("monospace")
                                        .text_size(px(scale::text::CAPTION))
                                        .text_color(move || palette(model).muted),
                                );
                            }
                            info
                        })
                        .child(state_badge(model, &entry.state));
                    if live && entry.state == "unregistered" {
                        let name = entry.name.clone();
                        row = row.child(
                            button(model, "")
                                // Like Add, says when the press also turns writes on.
                                .label_fn(move || {
                                    model
                                        .get()
                                        .word(if writes_on(model) {
                                            Msg::RegisterProject
                                        } else {
                                            Msg::EnableAndRegister
                                        })
                                        .into()
                                })
                                .spoken({
                                    let name = entry.name.clone();
                                    move || {
                                        let m = model.get();
                                        let action = if writes_on(model) {
                                            m.word(Msg::RegisterProjectSpoken)
                                        } else {
                                            m.word(Msg::EnableAndRegisterSpoken)
                                        };
                                        format!("{action} {name}").into()
                                    }
                                })
                                .size(ButtonSize::Small)
                                .disabled(move || model.get().project_add.busy)
                                .on_press(move || register(model, name.clone())),
                        );
                    }
                    if live {
                        let editing = entry.clone();
                        row = row.child(
                            button(model, "")
                                .label_fn(word(model, Msg::EditProject))
                                .spoken({
                                    let shown =
                                        entry.label.clone().unwrap_or_else(|| entry.name.clone());
                                    move || {
                                        format!(
                                            "{} {shown}",
                                            model.get().word(Msg::EditProjectSpoken)
                                        )
                                        .into()
                                    }
                                })
                                .size(ButtonSize::Small)
                                .on_press(move || edit(model, &editing)),
                        );
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
        }))
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
        let reveal = ui::motion::reveal();
        view()
            .items_start()
            .bind_motion(&reveal)
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
                            !n.is_empty() && !precept_ui_core::projects::valid_name(n)
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

/// The Workspaces tab (mounted inside Runtime checks, outside per-change page
/// rebuilds, so its fields keep focus while typing). Purely manages workspaces.
pub(crate) fn page(model: Signal<Model>) -> impl IntoElement {
    let profile_notice = dynamic(move || {
        let m = model.get();
        if !m.live || m.project_add.profile_selected != Some(false) {
            return view().into_element();
        }
        view()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_2()
            .p_3()
            .rounded(px(scale::radius::CONTAINER))
            .border_1()
            .border_color(move || palette(model).hairline)
            .bg(move || palette(model).panel)
            .child(badge(model, m.word(Msg::OperatorUnselected)).variant(BadgeVariant::Warning))
            .child(
                text(move || model.get().word(Msg::NeedsProfile))
                    .flex_1()
                    .min_w(px(180.0))
                    .text_size(px(scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
            )
            .child(
                button(model, "")
                    .label_fn(word(model, Msg::GoToProfile))
                    .size(ButtonSize::Small)
                    .variant(ButtonVariant::Primary)
                    .on_press(move || shell::go_setup(model, precept_ui_core::SetupTab::Profile)),
            )
            .into_element()
    });
    view()
        .gap_4()
        .max_w(px(scale::layout::CONTENT))
        .child(profile_notice)
        .child(list(model))
        .child(
            view()
                .style_when(move || model.get().live, |s| s.hidden())
                .child(note(model, |m| (!m.live).then_some(Msg::DemoProjects))),
        )
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
                        .icon(icons::PLUS)
                        .variant(ButtonVariant::Primary)
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
        .icon(icons::TRASH, ui::DialogTone::Danger)
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

/// The daemon's answer for a moved folder, as a badge.
fn edit_status(model: Signal<Model>) -> AnyElement {
    dynamic(move || {
        let d = model.get().project_edit.clone().unwrap_or_default();
        if !d.moving() {
            return view().into_element();
        }
        let (key, variant) = match &d.check {
            Check::Idle => return view().into_element(),
            Check::Checking => (Msg::Checking, BadgeVariant::Neutral),
            Check::Ok { .. } if d.taken() => (Msg::FolderTaken, BadgeVariant::Warning),
            Check::Ok { .. } => (Msg::FolderMoveOk, BadgeVariant::Success),
            Check::Refused(code) => (refusal(code), BadgeVariant::Warning),
            Check::Failed(code) => (refusal(code), BadgeVariant::Danger),
        };
        let reveal = ui::motion::reveal();
        view()
            .items_start()
            .bind_motion(&reveal)
            .child(badge(model, model.get_untracked().word(key)).variant(variant))
            .into_element()
    })
    .into_element()
}

/// Editing one project: its display name and, after its folder moved, the new
/// folder. A sibling of the page's scroll region, like the removal dialog.
pub(crate) fn edit_dialog(model: Signal<Model>) -> impl IntoElement {
    let draft = move || model.get().project_edit.clone().unwrap_or_default();
    ui::dialog(model, "")
        .icon(
            icons::section_path(precept_ui_core::Section::Projects),
            ui::DialogTone::Accent,
        )
        .title_fn(word(model, Msg::EditProjectTitle))
        .description_fn(move || {
            let m = model.get();
            let name = m
                .project_edit
                .as_ref()
                .map(|e| e.name.clone())
                .unwrap_or_default();
            m.word(Msg::EditProjectBody)
                .replace(
                    "{name}",
                    &precept_ui_core::presentation::sanitize(&name, 64),
                )
                .into()
        })
        .content_fn(move || {
            let folder = view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    view().flex_1().min_w_0().child(
                        input(model, "")
                            .test_id("project.edit.folder")
                            .label_fn(word(model, Msg::ProjectFolder))
                            .value(move || draft().path)
                            .on_change(move |v: &str| edit_update(model, |e| e.set_path(v.into())))
                            .on_submit(move |_: &str| edit_check(model)),
                    ),
                )
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::ChooseFolder))
                        .on_press(move || edit_choose(model)),
                )
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::CheckFolder))
                        .disabled(move || !draft().moving())
                        .loading(move || draft().check == Check::Checking)
                        .on_press(move || edit_check(model)),
                );
            let body = view()
                .w_full()
                .gap_4()
                .child(
                    field(
                        model,
                        word(model, Msg::DisplayName),
                        input(model, "")
                            .test_id("project.edit.label")
                            .label_fn(word(model, Msg::DisplayName))
                            .max_length(precept_ui_core::projects::MAX_LABEL as u32)
                            .value(move || draft().label)
                            .invalid(move || {
                                let l = draft().label;
                                !l.trim().is_empty() && !valid_label(l.trim())
                            })
                            .on_change(move |v: &str| edit_update(model, |e| e.set_label(v.into())))
                            .on_submit(move |_: &str| save(model)),
                    )
                    .help(word(model, Msg::DisplayNameHelp)),
                )
                .child(
                    field(model, word(model, Msg::ProjectFolder), folder)
                        .help(word(model, Msg::ProjectMoveHelp)),
                );
            body.child(edit_status(model))
                .child(note(model, |m| {
                    m.project_edit
                        .as_ref()
                        .and_then(|e| e.error.as_deref())
                        .map(|e| {
                            if e == "dialog" {
                                Msg::DialogUnavailable
                            } else {
                                refusal(e)
                            }
                        })
                }))
                .into_element()
        })
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(move || {
            model
                .get()
                .word(if edit_moves(model) && !writes_on(model) {
                    Msg::EnableAndSave
                } else {
                    Msg::SaveProject
                })
                .into()
        })
        .confirm_disabled(move || {
            let m = model.get();
            m.profile_setup.busy || !m.project_edit.as_ref().is_some_and(EditDraft::ready)
        })
        .open(move || model.get().project_edit.is_some())
        .on_cancel(move || model.update(|m| m.project_edit = None))
        .on_confirm(move || save(model))
}
