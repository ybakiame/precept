//! Debug-only gallery: every `ui` control in its variants, sizes and states, in
//! light and dark. It is mounted only when `ACP_UI_GALLERY` is set (see `App`),
//! never reached from normal navigation, so the look can be inspected without
//! touching product screens. A headless test mounts it to verify the controls
//! render and the destructive dialog opens and dismisses.
use super::prelude::*;
use super::{
    BadgeVariant, ButtonSize, ButtonVariant, alert_dialog, badge, button, empty, input, surface,
    tabs,
};
use acp_operator_core::appearance::{ColorMode, ThemeId};

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
                        .child(row(model, "Empty", vec![empty_state]))
                        .child(row(model, "Destructive", vec![destructive]))
                        .child(cards)
                        .child(row(model, "Scale", vec![scale_demo])),
                ),
            ),
        )
        .child(dialog_overlay)
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
        ] {
            assert!(
                app.screen().texts().iter().any(|t| t.contains(label)),
                "gallery missing {label}"
            );
        }
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
    }
}
