//! Debug-only gallery: every `ui` control in its variants, sizes and states, in
//! light and dark. It is mounted only when `ACP_UI_GALLERY` is set (see `App`),
//! never reached from normal navigation, so the look can be inspected without
//! touching product screens. A headless test mounts it to verify the controls
//! render and the destructive dialog opens and dismisses.
use super::prelude::*;
use super::{
    BadgeVariant, ButtonSize, ButtonVariant, alert_dialog, badge, button, empty, field, input,
    menu, nav_item, properties, radio_group, surface, tabs,
};
use acp_ui_core::appearance::{ColorMode, ThemeId};

const INBOX: &str = "M3 13H8L10 16H14L16 13H21M5 6H19L21 13V19H3V6Z";
const REFRESH: &str = "M20 12A8 8 0 1 1 12 4M20 4V8H16";

fn set_mode(model: Signal<Model>, mode: ColorMode) {
    model.update(|m| {
        m.color_mode = mode;
        m.dark = mode != ColorMode::Light;
    });
    set_color_scheme(match mode {
        ColorMode::Light => Appearance::Light,
        _ => Appearance::Dark,
    });
}

fn row(model: Signal<Model>, title: &'static str, children: Vec<AnyElement>) -> impl IntoElement {
    let mut panel = surface(model).title(title);
    for child in children {
        panel = panel.child(child);
    }
    panel
}

/// The gallery surface. Owns its demo state; nothing here touches product data.
#[component]
pub fn Gallery() -> impl IntoElement {
    let model = signal(Model::loading());
    let tab = signal(0_usize);
    let draft = signal(String::new());
    let invalid = signal(false);
    let loading = signal(false);
    let dialog = signal(false);

    let header = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(
            text("UI Gallery")
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(scale::text::TITLE)),
        )
        .child(button(model, "Light").on_press(move || set_mode(model, ColorMode::Light)))
        .child(button(model, "Dark").on_press(move || set_mode(model, ColorMode::Dark)))
        .child(
            button(model, "Contrast")
                .on_press(move || model.update(|m| m.high_contrast = !m.high_contrast)),
        )
        .children(ThemeId::ALL.into_iter().map(move |theme| {
            button(model, theme.name()).on_press(move || model.update(|m| m.theme = theme))
        }));

    let buttons = row(
        model,
        "Buttons",
        vec![
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(button(model, "Default"))
                .child(button(model, "Primary").variant(ButtonVariant::Primary))
                .child(button(model, "Danger").variant(ButtonVariant::Danger))
                .child(button(model, "Disabled").disabled(|| true))
                .child(
                    button(model, "Loading")
                        .variant(ButtonVariant::Primary)
                        .loading(move || loading.get()),
                )
                .child(
                    button(model, "Toggle loading").on_press(move || loading.update(|v| *v = !*v)),
                )
                .child(button(model, "With icon").icon(REFRESH))
                .into_element(),
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(button(model, "Small").size(ButtonSize::Small))
                .child(button(model, "Medium").size(ButtonSize::Medium))
                .child(button(model, "Large").size(ButtonSize::Large))
                .child(
                    button(model, "Small danger")
                        .size(ButtonSize::Small)
                        .variant(ButtonVariant::Danger),
                )
                .into_element(),
        ],
    );

    let tab_bar = view()
        .gap_2()
        .child(
            tabs(model)
                .add("Queue", move || tab.get() == 0, move || tab.set(0))
                .add("Detail", move || tab.get() == 1, move || tab.set(1))
                .add_fn(
                    || "History".into(),
                    move || tab.get() == 2,
                    move || tab.set(2),
                ),
        )
        .child(text(move || {
            format!(
                "Selected tab: {}",
                ["Queue", "Detail", "History"][tab.get()]
            )
        }))
        .into_element();

    let badges = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(badge(model, "Neutral"))
        .child(badge(model, "Info").variant(BadgeVariant::Info))
        .child(badge(model, "Confirmed").variant(BadgeVariant::Success))
        .child(badge(model, "Pending").variant(BadgeVariant::Warning))
        .child(badge(model, "Rejected").variant(BadgeVariant::Danger))
        .into_element();

    let fields = view()
        .gap_2()
        .child(
            input(model, "Retype name")
                .placeholder("Roadmap")
                .value(move || draft.get())
                .on_change(move |v| draft.set(v.to_string()))
                .invalid(move || invalid.get()),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(
                    button(model, "Toggle invalid").on_press(move || invalid.update(|v| *v = !*v)),
                )
                .child(text(move || {
                    if invalid.get() {
                        "Name does not match".to_string()
                    } else {
                        "Valid".to_string()
                    }
                }))
                .into_element(),
        )
        .child(
            input(model, "Disabled field")
                .value(|| "read only".to_string())
                .disabled(|| true),
        )
        .child(
            input(model, "Large field")
                .large()
                .max_length(64)
                .placeholder("Up to 64 characters")
                .value(String::new),
        )
        .into_element();

    let choice = signal(0usize);
    let radios = ["archlinux", "Ubuntu-24.04", "Debian"]
        .into_iter()
        .enumerate()
        .fold(
            radio_group(model, || "Distribution".into()),
            |group, (i, name)| group.option(name, move || choice.get() == i, move || choice.set(i)),
        )
        .into_element();
    let labeled = field(
        model,
        || "Linux runtime directory".into(),
        input(model, "Linux runtime directory")
            .value(|| "/home/operator/.local/state/acp/runtime".into()),
    )
    .help(|| "An absolute path private to this user.".into())
    .into_element();

    let place = signal(0usize);
    let navigation = ["Overview", "Preferences", "Settings"]
        .into_iter()
        .enumerate()
        .fold(
            view().w(px(scale::layout::SIDEBAR)).gap_0p5(),
            |list, (i, name)| {
                list.child(nav_item(
                    model,
                    INBOX,
                    move || name.into(),
                    move || place.get() == i,
                    move || place.set(i),
                ))
            },
        )
        .into_element();
    let project = signal(0usize);
    let picker = view()
        .w(px(scale::layout::SIDEBAR))
        .child(["alpha", "beta"].into_iter().enumerate().fold(
            menu(model, move || ["alpha", "beta"][project.get()].into()),
            |m, (i, name)| m.item(name, move || project.get() == i, move || project.set(i)),
        ))
        .into_element();
    let facts = properties(model)
        .row("Branch", "master")
        .row("Worktree", "Changes observed")
        .row(
            "A long label that wraps",
            "A long value that wraps across the lane without moving the next row",
        )
        .into_element();

    let empty_state = empty(model, "No pending reviews")
        .description("New suggestions appear here for an explicit decision.")
        .icon(INBOX)
        .action(button(model, "Refresh"))
        .into_element();

    let destructive = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(
            button(model, "Delete…")
                .variant(ButtonVariant::Danger)
                .on_press(move || dialog.set(true)),
        )
        .child(text("Opens a destructive confirmation").text_size(px(scale::text::CAPTION)))
        .into_element();

    let dialog_overlay = alert_dialog(model, "")
        .title_fn(|| "Delete “Roadmap”?".into())
        .description("This removes the record and cannot be undone.")
        .cancel("Keep")
        .confirm("Delete")
        .open(move || dialog.get())
        .on_cancel(move || dialog.set(false))
        .on_confirm(move || dialog.set(false))
        .into_element();

    // Overlays and feedback: a dialog commits with primary; a sheet with a
    // footer; toasts in each tone; an info popover; the Markdown editor.
    let form_open = signal(false);
    let sheet_open = signal(false);
    let markdown = signal(String::from("Prefer **small** commits."));
    let overlays = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(button(model, "Add preference…").on_press(move || form_open.set(true)))
        .child(button(model, "Details").on_press(move || sheet_open.set(true)))
        .child(
            button(model, "Success toast")
                .on_press(|| super::notify(super::Tone::Success, "Applied · commit 3")),
        )
        .child(
            button(model, "Info toast")
                .on_press(|| super::notify(super::Tone::Info, "Configuration copied")),
        )
        .child(
            button(model, "Error toast")
                .on_press(|| super::notify(super::Tone::Error, "Unauthorized: writes are off")),
        )
        .child(super::info(
            model,
            || "About this page".into(),
            || "A longer explanation that would crowd the page.".into(),
        ))
        .into_element();
    // Status dots beside their words, and icon buttons (square, named).
    let statuses = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_4()
        .children(
            [
                (super::Status::Live, "Connected"),
                (super::Status::Busy, "Reading…"),
                (super::Status::Idle, "Preview"),
                (super::Status::Down, "Unreachable"),
                (super::Status::Attention, "Needs attention"),
            ]
            .into_iter()
            .map(move |(status, word)| {
                view()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(super::status_dot(model, move || status))
                    .child(text(word))
                    .into_element()
            }),
        )
        .child(
            super::icon_button(model, super::icons::COPY, || "Copy".into())
                .size(super::ButtonSize::Small),
        )
        .child(super::icon_button(model, super::icons::CLOSE, || {
            "Dismiss".into()
        }))
        .into_element();
    let editor = super::markdown_input(
        model,
        "gallery-markdown",
        || "Value".into(),
        move || markdown.get_untracked(),
        move |v| markdown.set(v),
    )
    .into_element();
    // `ACP_UI_GALLERY=dialog` opens the form dialog, `=toast` shows a toast,
    // `=sheet` opens the sheet: overlays reviewable in a still capture.
    #[cfg(not(target_arch = "wasm32"))]
    match std::env::var("ACP_UI_GALLERY").as_deref() {
        Ok("dialog") => form_open.set(true),
        Ok("sheet") => sheet_open.set(true),
        Ok("toast") => {
            request_animation_frame(|_| {
                super::notify(
                    super::Tone::Success,
                    "Saved as a suggestion · review it next",
                );
                super::notify(super::Tone::Error, "Unauthorized: writes are off");
            });
        }
        _ => {}
    }
    let form_key = signal(String::from("commit-style"));
    let form_overlay = super::dialog(model, "")
        .size(super::DialogSize::Large)
        .icon(super::icons::PLUS, super::DialogTone::Accent)
        .title_fn(|| "Add a personal preference".into())
        .description(
            "Saved as a suggestion. You confirm the exact record before agents receive it.",
        )
        .content_fn(move || {
            view()
                .w_full()
                .gap_4()
                .child(super::field(
                    model,
                    || "Key".into(),
                    super::input(model, "Key")
                        .value(move || form_key.get())
                        .on_change(move |v| form_key.set(v.to_owned())),
                ))
                .child(super::field(
                    model,
                    || "Value".into(),
                    super::markdown_input(
                        model,
                        "gallery-dialog-value",
                        || "Value".into(),
                        || {
                            "## Commits\n\nUse **conventional commits**; an *imperative* subject.\n\n- Keep each commit `small`\n- Run the tests first\n\n> Agents read this Markdown as written."
                                .into()
                        },
                        |_| {},
                    ),
                ))
                .child(super::field(
                    model,
                    || "Source".into(),
                    super::input(model, "Source").value(|| "operator".into()),
                ))
                .into_element()
        })
        .cancel("Cancel")
        .confirm("Save as suggestion")
        .open(move || form_open.get())
        .on_cancel(move || form_open.set(false))
        .on_confirm(move || form_open.set(false))
        .into_element();
    let sheet_overlay = super::sheet(model)
        .title_fn(|| "Record details".into())
        .close_fn(|| "Close details".into())
        .open(move || sheet_open.get())
        .on_close(move || sheet_open.set(false))
        .body(|| text("Supplementary work beside the list.").into_element())
        .footer(move || {
            button(model, "Done")
                .variant(ButtonVariant::Primary)
                .on_press(move || sheet_open.set(false))
                .into_element()
        })
        .into_element();

    let cards = row(
        model,
        "Surface",
        vec![
            super::card(
                model,
                "Card",
                vec!["A line of content".into(), "Another line".into()],
            )
            .into_element(),
        ],
    );

    let scale_demo = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(text("Caption").text_size(px(scale::text::CAPTION)))
        .child(text("Body").text_size(px(scale::text::BODY)))
        .child(text("Subtitle").text_size(px(scale::text::SUBTITLE)))
        .child(text("Title").text_size(px(scale::text::TITLE)))
        .child(text("Display").text_size(px(scale::text::DISPLAY)))
        .child(
            view()
                .w(px(scale::icon::MD))
                .h(px(scale::icon::MD))
                .rounded(px(scale::radius::PILL))
                .bg(move || palette(model).primary),
        )
        .into_element();

    view()
        .flex_1()
        .font_family("sans-serif")
        .text_size(px(scale::text::BODY))
        .bg(move || palette(model).background)
        .text_color(move || palette(model).text)
        .child(
            view().flex_1().p_4().gap_3().child(header).child(
                scroll().flex_1().child(
                    view()
                        .gap_3()
                        .flex_shrink_0()
                        .child(buttons)
                        .child(row(model, "Tabs", vec![tab_bar]))
                        .child(row(model, "Badges", vec![badges]))
                        .child(row(model, "Inputs", vec![fields]))
                        .child(row(model, "Field", vec![labeled]))
                        .child(row(model, "Radio group", vec![radios]))
                        .child(row(model, "Navigation", vec![navigation]))
                        .child(row(model, "Menu", vec![picker]))
                        .child(row(model, "Properties", vec![facts]))
                        .child(row(model, "Empty", vec![empty_state]))
                        .child(row(model, "Destructive", vec![destructive]))
                        .child(row(model, "Status and icon buttons", vec![statuses]))
                        .child(row(model, "Overlays", vec![overlays]))
                        .child(row(model, "Markdown", vec![editor]))
                        .child(cards)
                        .child(row(model, "Scale", vec![scale_demo])),
                ),
            ),
        )
        .child(dialog_overlay)
        .child(form_overlay)
        .child(sheet_overlay)
        .child(super::toaster(model))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsh_engine::{EngineConfig, Root};

    #[test]
    fn gallery_renders_every_control_and_the_destructive_dialog_dismisses() {
        let mut app =
            nsh_testing::Headless::start(EngineConfig::new(Root::of::<Gallery>())).unwrap();
        app.wait_for("UI Gallery").unwrap();
        // Variants, sizes and states are present.
        for label in [
            "Default",
            "Primary",
            "Danger",
            "Disabled",
            "Loading",
            "Small",
            "Medium",
            "Large",
            "Confirmed",
            "Pending",
            "Rejected",
            "No pending reviews",
            "Toggle invalid",
            "Linux runtime directory",
            "An absolute path private to this user.",
            "Ubuntu-24.04",
            "Overview",
            "Worktree",
        ] {
            assert!(
                app.screen().texts().iter().any(|t| t.contains(label)),
                "gallery missing {label}"
            );
        }
        // A radio press checks its option (the app owns the value).
        app.press("Ubuntu-24.04").unwrap();
        // A destination press selects it; facts read as one element each.
        app.press("Preferences").unwrap();
        app.wait_for("Branch: master").unwrap();
        // Tabs move the selection by keyboard activation.
        app.press("Detail").unwrap();
        app.wait_for("Selected tab: Detail").unwrap();
        // Light and dark both render.
        app.press("Light").unwrap();
        app.press("Dark").unwrap();
        // The destructive dialog opens, names its consequence, and Escape closes it.
        app.press("Delete…").unwrap();
        app.wait_for("Delete “Roadmap”?").unwrap();
        app.wait_for("This removes the record and cannot be undone.")
            .unwrap();
        app.press_key("Escape").unwrap();
        app.wait_until("dialog dismissed", std::time::Duration::from_secs(5), |s| {
            !s.texts().iter().any(|t| t.contains("Delete “Roadmap”?"))
        })
        .unwrap();
        // A form dialog commits with a primary Save; Cancel closes it.
        app.press("Add preference…").unwrap();
        app.wait_for("Add a personal preference").unwrap();
        app.press("Cancel").unwrap();
        app.wait_for_gone(
            "Add a personal preference",
            std::time::Duration::from_secs(5),
        )
        .unwrap();
        // A sheet with its footer action.
        app.press("Details").unwrap();
        app.wait_for("Supplementary work beside the list.").unwrap();
        app.press("Done").unwrap();
        app.wait_for_gone(
            "Supplementary work beside the list.",
            std::time::Duration::from_secs(5),
        )
        .unwrap();
        // A toast reports, then leaves on its own.
        app.press("Success toast").unwrap();
        app.wait_for("Applied · commit 3").unwrap();
        // The Markdown editor is there, named.
        assert!(app.screen().count("id:gallery-markdown").unwrap() >= 1);
    }
}
