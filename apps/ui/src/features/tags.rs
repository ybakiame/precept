//! Tag colors: a side sheet listing every tag the app knows, each with its
//! color, how many confirmed preferences carry it and whether its color was
//! chosen or generated. Change color opens the picker under that row (one at
//! a time); dragging previews the color on the row's chip, and the color is
//! saved to `settings.toml` once each change ends. Presentation only: no
//! Profile write, nothing an agent receives.
use crate::*;
use precept_ui_core::Model;
use precept_ui_core::i18n::Msg;
use tokens::palette;

/// Opens the sheet (from the Preferences page).
pub(crate) fn open(model: Signal<Model>) {
    model.update(|m| {
        m.tag_colors_open = true;
        m.tag_editing = None;
    });
}

/// One tag: chip (its live color), count and kind, the change and reset
/// actions, and the picker below while it is the one being edited.
fn row(
    model: Signal<Model>,
    preview: Signal<Option<(String, String)>>,
    name: String,
    count: usize,
) -> AnyElement {
    let m = model.get();
    let editing = m.tag_editing.as_deref() == Some(name.as_str());
    let chosen = m.tag_colors.contains_key(&name);
    let (dot, spoken, toggle, reset, picked, label, value, live, done) = (
        name.clone(),
        name.clone(),
        name.clone(),
        name.clone(),
        name.clone(),
        name.clone(),
        name.clone(),
        name.clone(),
        name.clone(),
    );
    let color = move || match preview.get() {
        Some((tag, color)) if tag == dot => {
            ui::color::data_color(&color).unwrap_or_else(|| ui::tag_color(model, &tag))
        }
        _ => ui::tag_color(model, &dot),
    };
    let mut body = view().gap_2().py_2().child(
        view()
            .flex_row()
            .items_center()
            .gap_2()
            .child(ui::color_dot(color, 12.0))
            .child(
                text(format!(
                    "#{}",
                    precept_ui_core::presentation::sanitize(&name, 32)
                ))
                .font_weight(FontWeight::MEDIUM)
                .flex_1()
                .min_w_0(),
            )
            .child(
                text(format!(
                    "{} · {}",
                    m.word(Msg::ConfirmedCount)
                        .replace("{count}", &count.to_string()),
                    m.word(if chosen {
                        Msg::TagColorChosen
                    } else {
                        Msg::TagColorGenerated
                    })
                ))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
            )
            .child(
                ui::button(model, "")
                    .label_fn(move || {
                        model
                            .get()
                            .word(if editing {
                                Msg::TagColorDone
                            } else {
                                Msg::TagColorChange
                            })
                            .into()
                    })
                    .spoken(move || {
                        model
                            .get()
                            .word(Msg::TagColorChangeOf)
                            .replace("{tag}", &spoken)
                            .into()
                    })
                    .size(ui::ButtonSize::Small)
                    .selected(move || editing)
                    .on_press(move || {
                        preview.set(None);
                        model.update(|m| {
                            m.tag_editing = if editing { None } else { Some(toggle.clone()) }
                        })
                    }),
            ),
    );
    if editing {
        let grow = ui::motion::arrive(0.0);
        body = body.child(
            view()
                .gap_2()
                .p_3()
                .rounded(px(ui::scale::radius::CONTAINER))
                .bg(move || palette(model).inset)
                .bind_motion(&grow)
                .child(ui::color_picker(
                    model,
                    move || {
                        model
                            .get()
                            .word(Msg::TagColorOf)
                            .replace("{tag}", &label)
                            .into()
                    },
                    move || match preview.get() {
                        Some((tag, color)) if tag == value => color,
                        _ => model.get().tag_color(&value),
                    },
                    move |color| preview.set(Some((live.clone(), color))),
                    move |color| {
                        preview.set(None);
                        model.update(|m| m.set_tag_color(&picked, &color));
                    },
                ))
                .child(
                    view().flex_row().justify_end().child(
                        ui::button(model, "")
                            .label_fn(move || model.get().word(Msg::TagColorReset).into())
                            .size(ui::ButtonSize::Small)
                            .variant(ui::ButtonVariant::Ghost)
                            .icon(ui::icons::RESET)
                            .disabled(move || !model.get().tag_colors.contains_key(&done))
                            .on_press(move || {
                                preview.set(None);
                                model.update(|m| m.reset_tag_color(&reset));
                            }),
                    ),
                ),
        );
    }
    body.into_element()
}

pub(crate) fn panel(model: Signal<Model>) -> impl IntoElement {
    // The color being dragged, before it is saved: (tag, CSS color).
    let preview = signal(None::<(String, String)>);
    ui::sheet(model)
        .title_fn(move || model.get().word(Msg::TagColorsTitle).into())
        .close_fn(move || model.get().word(Msg::TagColorsClose).into())
        .open(move || model.get().tag_colors_open)
        .on_close(move || {
            preview.set(None);
            model.update(|m| {
                m.tag_colors_open = false;
                m.tag_editing = None;
            })
        })
        .body(move || {
            view()
                .gap_3()
                .child(
                    text(move || model.get().word(Msg::TagColorsHelp))
                        .text_size(px(ui::scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                )
                .child(dynamic(move || {
                    let tags = model.get().known_tags();
                    if tags.is_empty() {
                        return ui::empty(model, model.get().word(Msg::TagColorsEmpty))
                            .into_element();
                    }
                    let mut list = view();
                    for (name, count) in tags {
                        list = list.child(row(model, preview, name, count));
                    }
                    list.into_element()
                }))
                .into_element()
        })
}

#[cfg(test)]
mod tests {
    use nsh_core::prelude::*;
    use nsh_engine::{EngineConfig, Root};

    #[component]
    fn TagConsole() -> impl IntoElement {
        let mut model = precept_ui_core::Model::loading();
        model.section = precept_ui_core::Section::Preferences;
        model.receive(Ok(precept_ui_core::Snapshot::fixture()));
        crate::console(signal(model))
    }

    #[test]
    fn a_tag_color_is_picked_from_a_preset_saved_and_reset() {
        let mut cfg = EngineConfig::new(Root::of::<TagConsole>());
        cfg.viewport = Size::new(px(1080.0), px(760.0));
        let mut app = nsh_testing::Headless::start(cfg).unwrap();
        app.wait_for("#git · 1").unwrap();
        app.press("Tag colors…").unwrap();
        app.wait_for("Tag colors").unwrap();
        app.wait_for("#language").unwrap();
        assert!(app.screen().shows("1 confirmed · Generated"));
        app.press("Change the color of #git").unwrap();
        app.wait_for("Use generated color").unwrap();
        // A preset swatch ends the change at once: the color is chosen.
        app.press("oklch(68% 0.13 36)").unwrap();
        app.wait_for("1 confirmed · Chosen").unwrap();
        app.press("Use generated color").unwrap();
        app.wait_until("generated again", std::time::Duration::from_secs(5), |s| {
            !s.shows("1 confirmed · Chosen")
        })
        .unwrap();
        app.press("Change the color of #git").unwrap();
        app.wait_for_gone("Use generated color", std::time::Duration::from_secs(5))
            .unwrap();
    }
}
