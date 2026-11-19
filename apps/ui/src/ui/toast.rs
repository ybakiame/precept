//! The app's toasts: asynchronous status that needs no decision (an applied
//! change, a refusal), stacked at the bottom of the window and dismissed on a
//! timer, by Escape or a swipe (the `nsh/base` toast stack and manager). One
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
pub fn notify(tone: Tone, text: impl Into<SharedString>) {
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
            (tone, text.into()),
            ToastOptions::timeout(Some(Duration::from_secs(seconds))),
            nsh_core::time::now(),
        )
    });
    sync(manager, shown);
}

/// The window's toast stack; the shell mounts it once, last.
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
    toast_stack()
        .placement(Anchor::BottomRight)
        .state(state)
        .w(px(340.0))
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
                    .gap_2()
                    .px_4()
                    .py_3()
                    .rounded(px(scale::radius::CONTAINER))
                    .border_1()
                    .border_color(move || palette(model).border)
                    .bg(move || palette(model).panel)
                    .shadow_lg()
                    .transition_all()
                    .duration(motion::ms(motion::BASE))
                    .starting_class("opacity-0 translate-y-4")
                    .ending_class("opacity-0 scale-95")
                    .child(
                        view()
                            .size(px(8.0))
                            .rounded(px(scale::radius::PILL))
                            .flex_shrink_0()
                            .bg(move || match tone() {
                                Tone::Success => palette(model).success,
                                Tone::Error => palette(model).error,
                                Tone::Info => palette(model).primary,
                            }),
                    )
                    .child(
                        text(move || item.get().1.1)
                            .flex_1()
                            .min_w_0()
                            .text_size(px(scale::text::BODY))
                            .text_color(move || palette(model).text),
                    )
            },
        ))
}
