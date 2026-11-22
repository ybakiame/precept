//! The app's toasts: asynchronous status that needs no decision (an applied
//! change, a refusal), floating over the bottom-right corner of the window and dismissed on a
//! timer, by Escape, a swipe or its close button (the `nsh/base` toast stack and manager). One
//! stack is mounted by the shell; features call [`notify`]. A toast never
//! carries the only copy of something the user must act on.
use super::prelude::*;
use nsh_base::{
    Anchor, ToastManager, ToastMotion, ToastOptions, ToastStackState, toast as base_toast,
    toast_stack,
};
use std::{cell::Cell, time::Duration};

/// What a toast reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Success,
    Error,
    Info,
}

type Item = (Tone, SharedString);
type Shown = Vec<(u64, Item, nsh_base::ToastTransitionStatus)>;
/// The mounted stack's manager and what it shows.
type Sink = (Stored<ToastManager<u64, Item>>, Signal<Shown>);

thread_local! {
    static SINK: Cell<Option<Sink>> = const { Cell::new(None) };
    static NEXT: Cell<u64> = const { Cell::new(1) };
}

fn sync(manager: Stored<ToastManager<u64, Item>>, shown: Signal<Shown>) {
    shown.set(manager.with(|m| {
        m.visible(3)
            .map(|(id, item, status)| (*id, item.clone(), status))
            .collect()
    }));
}

/// Shows `text` as a toast (no-op before the shell mounted the stack).
///
/// Callers often notify from inside `model.update`, while the model's value
/// is taken out for the update. The toast renders at once and its controls
/// read the model, so the push waits for the next turn of the executor, on
/// a detached owner (a closing dialog or menu must not cancel it).
pub fn notify(tone: Tone, text: impl Into<SharedString>) {
    let text = text.into();
    Owner::detached().run(|| {
        spawn(async move { show(tone, text) });
    });
}

fn show(tone: Tone, text: SharedString) {
    let Some((manager, shown)) = SINK.with(Cell::get) else {
        return;
    };
    let id = NEXT.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    });
    let seconds = if tone == Tone::Error { 8 } else { 4 };
    manager.with(|m| {
        m.push(
            id,
            (tone, text),
            ToastOptions::timeout(Some(Duration::from_secs(seconds))),
            nsh_core::time::now(),
        )
    });
    sync(manager, shown);
}

fn tone_icon(tone: Tone) -> &'static str {
    match tone {
        Tone::Success => super::icons::CHECK,
        Tone::Error => super::icons::ERROR,
        Tone::Info => super::icons::INFO,
    }
}

fn tone_color(model: Signal<Model>, tone: Tone) -> AdaptiveColor {
    let p = palette(model);
    match tone {
        Tone::Success => p.success,
        Tone::Error => p.error,
        Tone::Info => p.primary,
    }
}

/// The room the stack may take at the window's bottom-right: the newest toast
/// and the peeks of the older ones, or the expanded list. It passes pointer
/// events through wherever no toast is drawn.
const AREA: f32 = 320.0;

/// The window's toast stack; the shell mounts it once, last. On the desktop it
/// floats in an overlay at the bottom-right corner and never takes layout
/// space.
pub fn toaster(model: Signal<Model>) -> impl IntoElement {
    let manager = stored(ToastManager::<u64, Item>::new(ToastMotion::sonner()));
    let shown: Signal<Shown> = signal(Vec::new());
    let state = ToastStackState::new();
    SINK.with(|s| s.set(Some((manager, shown))));
    set_interval(Duration::from_millis(50), move || {
        if manager.with(|m| {
            m.advance(nsh_core::time::now(), state.is_expanded())
                .changed
        }) {
            sync(manager, shown);
        }
    });
    let stack = toast_stack()
        .placement(Anchor::BottomRight)
        .state(state)
        .w_full()
        .h_full()
        .pointer_events(PointerEvents::BoxNone)
        .child(each(
            shown,
            |(id, _, _)| *id,
            move |item, _| {
                let tone = move || item.get().1.0;
                base_toast()
                    .transition_status(move || item.get().2)
                    .accessibility_label(move || Some(item.get().1.1))
                    .on_dismiss_request(move || {
                        manager.with(|m| m.dismiss(&item.get().0, nsh_core::time::now()));
                    })
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .ps_3()
                    .pe_2()
                    .py_3()
                    .rounded(px(scale::radius::OVERLAY))
                    .border_1()
                    .border_color(move || palette(model).hairline)
                    .bg(move || palette(model).raised)
                    .shadow_xl()
                    .transition_all()
                    .duration(motion::ms(motion::BASE))
                    .ease_bezier(
                        motion::SETTLE.0,
                        motion::SETTLE.1,
                        motion::SETTLE.2,
                        motion::SETTLE.3,
                    )
                    .starting_class("opacity-0 translate-y-4")
                    .ending_class("opacity-0 scale-95")
                    // The tone: an icon of its own shape in a tinted disc.
                    .child(
                        view()
                            .size(px(scale::disc::MD))
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .rounded(px(scale::radius::PILL))
                            .bg(move || {
                                tone_color(model, tone()).opacity(super::tokens::tint::REST)
                            })
                            .text_color(move || tone_color(model, tone()))
                            .child(dynamic(move || {
                                icon::glyph(tone_icon(tone()), scale::icon::MD).into_element()
                            })),
                    )
                    .child(
                        text(move || item.get().1.1)
                            .flex_1()
                            .min_w_0()
                            .text_size(px(scale::text::BODY))
                            .text_color(move || palette(model).text),
                    )
                    .child(
                        super::button::icon_button(model, super::icons::CLOSE, move || {
                            model
                                .get()
                                .word(precept_ui_core::i18n::Msg::DismissNotification)
                                .into()
                        })
                        .size(super::button::ButtonSize::Small)
                        .on_press(move || {
                            manager.with(|m| m.dismiss(&item.get().0, nsh_core::time::now()));
                            sync(manager, shown);
                        }),
                    )
            },
        ));
    overlay()
        .bottom_4()
        .right_4()
        .w(move || px(scale::layout::TOAST.min(window().width.0 - 32.0).max(1.0)))
        .h(px(AREA))
        .pointer_events(PointerEvents::BoxNone)
        .child(stack)
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsh_engine::{EngineConfig, Root};

    #[component]
    fn ToastPage() -> impl IntoElement {
        let model = signal(Model::loading());
        view()
            .flex_1()
            .child(
                super::super::button::button(model, "Notify")
                    .on_press(|| notify(Tone::Success, "Saved as a suggestion")),
            )
            .child(view().flex_1().test_id("page-body").child(text("Body")))
            .child(toaster(model))
    }

    fn page(app: &nsh_testing::Headless) -> Bounds<Points> {
        app.screen()
            .layout_nodes()
            .into_iter()
            .find(|n| n.test_id.as_deref() == Some("page-body"))
            .unwrap()
            .bounds
    }

    /// A toast floats over the page: showing one never moves or shrinks the
    /// page (the regression: the stack once took a row of layout).
    #[test]
    fn a_toast_floats_and_never_takes_page_space() {
        let mut cfg = EngineConfig::new(Root::of::<ToastPage>());
        cfg.viewport = Size::new(px(900.0), px(600.0));
        let mut app = nsh_testing::Headless::start(cfg).unwrap();
        app.wait_for("Body").unwrap();
        let before = page(&app);
        app.press("Notify").unwrap();
        app.wait_for("Saved as a suggestion").unwrap();
        app.wait_for("Dismiss notification").unwrap();
        let after = page(&app);
        assert_eq!(after, before);
        // Nothing below the page takes space: it reaches the window's bottom.
        assert!(
            (after.origin.y.0 + after.size.height.0 - 600.0).abs() < 0.5,
            "page ends at {} with a toast shown",
            after.origin.y.0 + after.size.height.0
        );
    }
}
