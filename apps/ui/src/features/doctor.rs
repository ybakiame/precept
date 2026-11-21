//! Doctor: the checks of [`precept_ui_core::doctor`] for the selected
//! workspace, each with what it saw and a way to the page that fixes it.
//! Read-only: nothing here writes; Check again re-reads, Copy report copies
//! text without paths, values or credentials.
use super::{agents, profile};
use crate::*;
use precept_ui_core::{
    SetupTab,
    doctor::{self, Check, Fix, Level},
    i18n::Msg,
};
use tokens::palette;
use ui::{BadgeVariant, ButtonSize, ButtonVariant, badge, button, scale, surface};

fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}

fn variant(level: Level) -> BadgeVariant {
    match level {
        Level::Ok => BadgeVariant::Success,
        Level::Attention => BadgeVariant::Warning,
        Level::Problem => BadgeVariant::Danger,
        Level::Off | Level::Unknown => BadgeVariant::Neutral,
    }
}

fn fix(model: Signal<Model>, to: Fix) {
    match to {
        Fix::Connection => shell::go_setup(model, SetupTab::Connection),
        Fix::Profile => shell::go_setup(model, SetupTab::Profile),
        Fix::Workspaces => shell::go_setup(model, SetupTab::Workspaces),
        Fix::Agents => shell::go(model, shell::Destination::Agents),
        Fix::Review => shell::go(model, shell::Destination::Review),
    }
}

/// Re-reads everything the checks use: the snapshot, the Profile, the review
/// count and the selected workspace's integrations and activity.
fn recheck(model: Signal<Model>) {
    load(model);
    profile::read(model);
    profile::count(model);
    if let Some(name) = model.get().selected_project.clone() {
        agents::fetch(model, name);
    }
}

fn row(model: Signal<Model>, index: usize, c: Check) -> impl IntoElement {
    let m = model.get_untracked();
    let title = m.word(c.title);
    let mut row = view()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .min_h(px(scale::control::LG))
        .style_when(move || index > 0, |s| s.border_t_1())
        .border_color(move || palette(model).hairline)
        .child(
            view()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(text(title).font_weight(FontWeight::SEMIBOLD))
                .child(
                    text(c.detail.clone())
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                ),
        )
        .child(badge(model, m.word(doctor::level_word(c.level))).variant(variant(c.level)));
    if let Some(to) = c.fix {
        let spoken = m.word(Msg::DoctorFixSpoken).replace("{name}", title);
        row = row.child(
            button(model, "")
                .label_fn(word(model, Msg::DoctorFix))
                .spoken(move || spoken.clone().into())
                .size(ButtonSize::Small)
                .on_press(move || fix(model, to)),
        );
    }
    row
}

/// The Doctor panel (mounted by the shell on Diagnostics).
pub(crate) fn panel(model: Signal<Model>) -> impl IntoElement {
    let checks = computed(move || {
        let m = model.get();
        (doctor::checks(&m), m.locale)
    });
    let summary = move || {
        let m = model.get();
        let (p, a) = doctor::tally(&checks.get().0);
        if p + a == 0 {
            m.word(Msg::DoctorAllOk).to_owned()
        } else {
            m.word(Msg::DoctorSummary)
                .replace("{p}", &p.to_string())
                .replace("{a}", &a.to_string())
        }
    };
    surface(model)
        .title_fn(word(model, Msg::DoctorTitle))
        .child(
            text(word(model, Msg::DoctorIntro))
                .text_size(px(scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    text(move || SharedString::from(summary()))
                        .flex_1()
                        .min_w(px(180.0))
                        .font_weight(FontWeight::MEDIUM),
                )
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::DoctorRecheck))
                        .icon(ui::icons::RESET)
                        .size(ButtonSize::Small)
                        .variant(ButtonVariant::Ghost)
                        .on_press(move || recheck(model)),
                )
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::DoctorCopyReport))
                        .size(ButtonSize::Small)
                        .on_press(move || {
                            let m = model.get();
                            set_clipboard(doctor::report(&m, &checks.get().0));
                            ui::notify(ui::Tone::Success, m.word(Msg::DoctorCopied));
                        }),
                ),
        )
        .child(dynamic(move || {
            let (list, _) = checks.get();
            view()
                .children(
                    list.into_iter()
                        .enumerate()
                        .map(move |(i, c)| row(model, i, c)),
                )
                .into_element()
        }))
}
