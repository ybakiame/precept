//! Keyboard shortcuts: every shortcut is bound through [`bind`] from the
//! catalog in `precept_ui_core::keys`, so a change in the Shortcuts panel rebinds
//! it at once and is kept in `settings.toml`. The panel (a sheet) lists every
//! shortcut by group, filters them, and records a new keystroke for one.
//!
//! While a keystroke is being recorded no shortcut is bound (they would take
//! the keys first), and the recorder captures every key.
use crate::*;
use nsh_core::keymap::{Keystroke, Modifiers};
use precept_ui_core::{
    i18n::Msg,
    keys::{Group, SHORTCUTS, Shortcut},
};
use std::rc::Rc;
use tokens::palette;
use ui::{ButtonSize, ButtonVariant, button, icon_button, scale};

/// The panel's fixed lanes, so keys and actions line up down the list.
const KEYS_LANE: f32 = 200.0;
const ACTIONS_LANE: f32 = 96.0;

/// Why a keystroke cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// It needs Ctrl, Alt or the platform's command key (F-keys excepted).
    NeedsModifier,
    /// NSH cannot parse it, or the system reserves it.
    Reserved,
}

/// `keystroke` if this platform can bind it.
pub(crate) fn usable(keystroke: &str) -> Result<Keystroke, Refusal> {
    let parsed = Keystroke::parse_with(keystroke, try_apple_keys().unwrap_or(false))
        .map_err(|_| Refusal::Reserved)?;
    if parsed.reserved(os()).is_some() {
        return Err(Refusal::Reserved);
    }
    Ok(parsed)
}

/// Binds the action `id` to its current keystrokes, rebinding when the
/// operator changes them. Nothing is bound while a keystroke is recorded.
/// A keystroke this platform cannot bind is skipped, never a panic.
pub(crate) fn bind(
    model: Signal<Model>,
    id: &'static str,
    options: KeyOptions,
    handler: impl Fn() + 'static,
) {
    let handler = Rc::new(handler);
    let chords = computed(move || {
        let m = model.get();
        if m.shortcut_recording.is_some() {
            Vec::new()
        } else {
            m.keymap.chords(id)
        }
    });
    effect(move || {
        for chord in chords.get() {
            if usable(&chord).is_err() {
                continue;
            }
            let handler = handler.clone();
            on_key_with(&chord, options, move |_| handler());
        }
    });
}

/// What one key-down means while recording.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Recorded {
    /// A modifier alone: keep listening.
    Wait,
    /// Escape alone: stop recording without a change.
    Cancel,
    /// The keystroke, in NSH's spelling.
    Keystroke(String),
    Refused(Refusal),
}

/// The key a physical `code` names, as NSH spells keys (`a`, `1`, `/`,
/// `ArrowLeft`, `F5`); the logical `key` when there is no code.
fn base_key(code: &str, key: &str) -> Option<String> {
    if let Some(letter) = code.strip_prefix("Key") {
        return Some(letter.to_ascii_lowercase());
    }
    if let Some(digit) = code.strip_prefix("Digit") {
        return Some(digit.to_owned());
    }
    let punctuation = match code {
        "Minus" => "-",
        "Equal" => "=",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Backslash" => "\\",
        "Semicolon" => ";",
        "Quote" => "'",
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        "Backquote" => "`",
        _ => "",
    };
    if !punctuation.is_empty() {
        return Some(punctuation.to_owned());
    }
    let named = [
        "ArrowLeft",
        "ArrowRight",
        "ArrowUp",
        "ArrowDown",
        "Enter",
        "Tab",
        "Space",
        "Backspace",
        "Delete",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "Insert",
    ];
    if named.contains(&code) || (code.starts_with('F') && code[1..].parse::<u8>().is_ok()) {
        return Some(code.to_owned());
    }
    if code.is_empty() {
        let mut chars = key.chars();
        return match (chars.next(), chars.next()) {
            (Some(c), None) => Some(c.to_ascii_lowercase().to_string()),
            _ if named.contains(&key) => Some(key.to_owned()),
            _ => None,
        };
    }
    None
}

/// A recorded key-down as a keystroke: `mod` is the command key on Apple
/// platforms and Control elsewhere.
pub(crate) fn record(key: &str, code: &str, m: Modifiers, apple: bool) -> Recorded {
    if matches!(
        key,
        "Shift" | "Control" | "Alt" | "Meta" | "Super" | "OS" | "AltGraph" | "CapsLock" | "Hyper"
    ) {
        return Recorded::Wait;
    }
    let plain = !(m.ctrl || m.alt || m.shift || m.cmd);
    if key == "Escape" && plain {
        return Recorded::Cancel;
    }
    let Some(base) = base_key(code, key) else {
        return Recorded::Refused(Refusal::Reserved);
    };
    let (command, control) = if apple {
        (m.cmd, m.ctrl)
    } else {
        (m.ctrl, false)
    };
    let function = base.starts_with('F') && base.len() > 1;
    if !(command || control || m.alt || function) {
        return Recorded::Refused(Refusal::NeedsModifier);
    }
    let mut parts = Vec::new();
    if command {
        parts.push("mod");
    }
    if control {
        parts.push("ctrl");
    }
    if m.alt {
        parts.push("alt");
    }
    if m.shift {
        parts.push("shift");
    }
    let mut keystroke = parts.join("+");
    if !keystroke.is_empty() {
        keystroke.push('+');
    }
    keystroke.push_str(&base);
    Recorded::Keystroke(keystroke)
}

/// Opens the Shortcuts panel.
pub(crate) fn open(model: Signal<Model>) {
    model.update(|m| m.shortcuts_open = true);
}

fn close(model: Signal<Model>) {
    model.update(|m| {
        m.shortcuts_open = false;
        m.shortcut_recording = None;
    });
}

/// The words for a refusal or a conflict, under the row being changed.
fn problem_text(m: &Model, problem: &Problem) -> String {
    match problem {
        Problem::Refused(Refusal::NeedsModifier) => m.word(Msg::KeysNeedsModifier).into(),
        Problem::Refused(Refusal::Reserved) => m.word(Msg::KeysReserved).into(),
        Problem::Conflict(other) => m
            .word(Msg::KeysConflict)
            .replace("{name}", m.word(other.label)),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Problem {
    Refused(Refusal),
    Conflict(&'static Shortcut),
}

/// The recorder: a focused view that captures every key until a keystroke
/// is taken, refused or Escape cancels.
fn recorder(
    model: Signal<Model>,
    id: &'static str,
    problem: Signal<Option<(&'static str, Problem)>>,
) -> AnyElement {
    let field = element_ref();
    request_animation_frame(move |_| field.focus());
    view()
        .element_ref(&field)
        .focusable(true)
        .accessible(true)
        .role(AccessibilityRole::Button)
        .label(move || model.get().word(Msg::KeysRecording))
        .capture_keys(KeyCapture::All)
        .on_key_down(move |e: &KeyEvent| {
            if e.repeat {
                return;
            }
            match record(
                &e.key,
                &e.code,
                e.modifiers,
                try_apple_keys().unwrap_or(false),
            ) {
                Recorded::Wait => {}
                Recorded::Cancel => model.update(|m| m.shortcut_recording = None),
                Recorded::Refused(why) => problem.set(Some((id, Problem::Refused(why)))),
                Recorded::Keystroke(keystroke) => {
                    if let Err(why) = usable(&keystroke) {
                        problem.set(Some((id, Problem::Refused(why))));
                        return;
                    }
                    if let Some(other) = model.with(|m| m.keymap.conflict(id, &keystroke)) {
                        problem.set(Some((id, Problem::Conflict(other))));
                        return;
                    }
                    problem.set(None);
                    model.update(|m| {
                        m.keymap.set(id, &keystroke);
                        m.shortcut_recording = None;
                    });
                }
            }
        })
        .flex_row()
        .items_center()
        .gap_2()
        .px_2()
        .py_0p5()
        .rounded(px(scale::radius::SMALL))
        .border_1()
        .border_color(move || palette(model).primary)
        .bg(move || palette(model).primary.opacity(ui::tokens::tint::REST))
        .child(
            text(move || model.get().word(Msg::KeysRecording))
                .text_size(px(scale::text::CAPTION))
                .text_color(move || palette(model).text),
        )
        .into_element()
}

/// One shortcut: its name, its keys (or the recorder), and its commands.
fn row(
    model: Signal<Model>,
    shortcut: &'static Shortcut,
    problem: Signal<Option<(&'static str, Problem)>>,
) -> AnyElement {
    let id = shortcut.id;
    let m = model.get_untracked();
    let recording = m.shortcut_recording == Some(id);
    let custom = m.keymap.is_custom(id);
    let keys: AnyElement = if recording {
        recorder(model, id, problem)
    } else {
        // Each keystroke as caps; a second spelling after a muted "or".
        let mut lane = view().flex_row().flex_wrap().items_center().gap_1p5();
        // Two spellings of one key (`alt+ArrowLeft`, `alt+Left`) show once.
        let mut shown: Vec<Vec<String>> = Vec::new();
        let chords: Vec<String> = m
            .keymap
            .chords(id)
            .into_iter()
            .filter(|k| {
                let parts = ui::key_parts(k);
                let new = !shown.contains(&parts);
                shown.push(parts);
                new
            })
            .collect();
        for (i, k) in chords.into_iter().enumerate() {
            if i > 0 {
                lane = lane.child(
                    text(m.word(Msg::KeysOr))
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                );
            }
            lane = lane.child(ui::keycaps(model, &k));
        }
        lane.into_element()
    };
    let mut actions = view().flex_row().items_center().gap_1();
    actions = actions.child(
        button(model, "")
            .label_fn(move || model.get().word(Msg::KeysChange).into())
            .spoken(move || {
                let m = model.get();
                format!("{} {}", m.word(Msg::KeysChange), m.word(shortcut.label)).into()
            })
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::Small)
            .selected(move || model.get().shortcut_recording == Some(id))
            .on_press(move || {
                problem.set(None);
                model.update(|m| {
                    m.shortcut_recording = if m.shortcut_recording == Some(id) {
                        None
                    } else {
                        Some(id)
                    }
                });
            }),
    );

    if custom {
        actions = actions.child(
            icon_button(model, ui::icons::RESET, move || {
                let m = model.get();
                format!("{} {}", m.word(Msg::KeysReset), m.word(shortcut.label)).into()
            })
            .size(ButtonSize::Small)
            .on_press(move || {
                problem.set(None);
                model.update(|m| m.keymap.reset(id));
            }),
        );
    }
    let mut body = view()
        .gap_1()
        .py_2()
        .border_b_1()
        .border_color(move || palette(model).hairline)
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                // The name wraps in its lane; "Custom" sits under it.
                .child(
                    view()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .items_start()
                        .child(text(m.word(shortcut.label)).text_size(px(scale::text::BODY)))
                        .child(
                            view().style_when(move || !custom, |s| s.hidden()).child(
                                ui::badge(model, m.word(Msg::KeysCustom))
                                    .variant(ui::BadgeVariant::Info),
                            ),
                        ),
                )
                .child(view().w(px(KEYS_LANE)).flex_shrink_0().child(keys))
                .child(
                    view()
                        .w(px(ACTIONS_LANE))
                        .flex_shrink_0()
                        .items_end()
                        .child(actions),
                ),
        );
    if let Some((at, p)) = problem.get_untracked()
        && at == id
    {
        body = body.child(
            text(problem_text(&m, &p))
                .text_size(px(scale::text::CAPTION))
                .text_color(move || palette(model).error),
        );
    }
    body.into_element()
}

/// The list, by group, of the shortcuts matching `filter`.
fn list(
    model: Signal<Model>,
    filter: Signal<String>,
    problem: Signal<Option<(&'static str, Problem)>>,
) -> AnyElement {
    let key = computed(move || {
        let m = model.get();
        (m.locale, m.keymap.clone(), m.shortcut_recording)
    });
    dynamic(move || {
        let _ = key.get();
        let _ = problem.get();
        let wanted = filter.get().trim().to_lowercase();
        let m = model.get_untracked();
        let matches = |s: &Shortcut| {
            wanted.is_empty()
                || m.word(s.label).to_lowercase().contains(&wanted)
                || m.keymap
                    .chords(s.id)
                    .iter()
                    .any(|k| ui::key_parts(k).join("+").to_lowercase().contains(&wanted))
        };
        let mut body = view().gap_4().flex_shrink_0();
        let mut any = false;
        for group in Group::ALL {
            let rows: Vec<_> = SHORTCUTS
                .iter()
                .filter(|s| s.group == group && matches(s))
                .collect();
            if rows.is_empty() {
                continue;
            }
            any = true;
            body = body.child(
                view()
                    .child(
                        text(m.word(group.label()))
                            .role(AccessibilityRole::Header)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(scale::text::CAPTION))
                            .text_color(move || palette(model).muted),
                    )
                    .children(rows.into_iter().map(move |s| row(model, s, problem))),
            );
        }
        if !any {
            body = body.child(ui::empty(model, m.word(Msg::KeysNoMatch)).icon(ui::icons::KEYBOARD));
        }
        body.into_element()
    })
    .into_element()
}

/// The Shortcuts panel: a sheet from the trailing edge. Escape while
/// recording only stops the recording.
pub(crate) fn panel(model: Signal<Model>) -> impl IntoElement {
    let filter = signal(String::new());
    let problem: Signal<Option<(&'static str, Problem)>> = signal(None);
    ui::sheet(model)
        .width(scale::layout::SHEET_WIDE)
        .title_fn(move || model.get().word(Msg::KeysTitle).into())
        .close_fn(move || model.get().word(Msg::KeysClose).into())
        .open(move || model.get().shortcuts_open)
        .on_close(move || {
            if model.get().shortcut_recording.is_some() {
                model.update(|m| m.shortcut_recording = None);
            } else {
                close(model);
            }
        })
        .body(move || {
            view()
                .gap_4()
                .child(
                    text(move || model.get().word(Msg::KeysHelp))
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                )
                .child(
                    ui::input(model, "")
                        .label_fn(move || model.get().word(Msg::KeysFilter).into())
                        .placeholder(model.get_untracked().word(Msg::KeysFilter))
                        .value(move || filter.get())
                        .on_change(move |v| filter.set(v.to_owned())),
                )
                .child(list(model, filter, problem))
                .into_element()
        })
        .footer(move || {
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    button(model, "")
                        .label_fn(move || model.get().word(Msg::KeysResetAll).into())
                        .icon(ui::icons::RESET)
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .disabled(move || model.get().keymap.overrides().is_empty())
                        .on_press(move || {
                            problem.set(None);
                            model.update(|m| {
                                m.shortcut_recording = None;
                                m.keymap.reset_all();
                            });
                        }),
                )
                .into_element()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use precept_ui_core::keys::Keymap;

    fn mods(ctrl: bool, alt: bool, shift: bool, cmd: bool) -> Modifiers {
        Modifiers {
            ctrl,
            alt,
            shift,
            cmd,
        }
    }

    #[test]
    fn every_default_keystroke_parses_on_every_desktop() {
        for s in SHORTCUTS {
            for key in s.defaults {
                for apple in [false, true] {
                    assert!(Keystroke::parse_with(key, apple).is_ok(), "{} {key}", s.id);
                }
            }
        }
    }

    #[test]
    fn recording_spells_keystrokes_as_nsh_does() {
        let r = |key, code, m, apple| record(key, code, m, apple);
        assert_eq!(
            r("Shift", "ShiftLeft", mods(false, false, true, false), false),
            Recorded::Wait
        );
        assert_eq!(
            r("Escape", "Escape", mods(false, false, false, false), false),
            Recorded::Cancel
        );
        assert_eq!(
            r("Y", "KeyY", mods(false, true, true, false), false),
            Recorded::Keystroke("alt+shift+y".into())
        );
        assert_eq!(
            r("?", "Slash", mods(true, false, true, false), false),
            Recorded::Keystroke("mod+shift+/".into())
        );
        assert_eq!(
            r("k", "KeyK", mods(false, false, false, true), true),
            Recorded::Keystroke("mod+k".into())
        );
        assert_eq!(
            r("k", "KeyK", mods(true, false, false, false), true),
            Recorded::Keystroke("ctrl+k".into())
        );
        assert_eq!(
            r("F5", "F5", mods(false, false, false, false), false),
            Recorded::Keystroke("F5".into())
        );
        assert_eq!(
            r(
                "ArrowLeft",
                "ArrowLeft",
                mods(false, true, false, false),
                false
            ),
            Recorded::Keystroke("alt+ArrowLeft".into())
        );
        assert_eq!(
            r("a", "KeyA", mods(false, false, true, false), false),
            Recorded::Refused(Refusal::NeedsModifier)
        );
        // Headless input has no physical code: the logical key stands in.
        assert_eq!(
            r("j", "", mods(false, true, false, false), false),
            Recorded::Keystroke("alt+j".into())
        );
        for keystroke in ["alt+shift+y", "mod+shift+/", "F5", "alt+ArrowLeft", "alt+j"] {
            assert!(
                Keystroke::parse_with(keystroke, false).is_ok(),
                "{keystroke}"
            );
        }
    }

    #[component]
    fn ShortcutsPanel() -> impl IntoElement {
        let model = signal(Model::loading());
        model.update(|m| m.shortcuts_open = true);
        view()
            .flex_1()
            .child(text(move || {
                format!(
                    "Reconnect is {}",
                    model.get().keymap.primary("reconnect").unwrap_or_default()
                )
            }))
            .child(panel(model))
    }

    #[test]
    fn the_panel_records_a_free_keystroke_and_refuses_a_taken_one() {
        let mut cfg = nsh_engine::EngineConfig::new(nsh_engine::Root::of::<ShortcutsPanel>());
        cfg.viewport = Size::new(px(1080.0), px(760.0));
        // The simulated Linux backend delivers raw key-downs to the focus.
        cfg.environment.os = nsh_core::platform::Os::Linux;
        let mut app = nsh_testing::Headless::start(cfg).unwrap();
        app.wait_for("Reconnect is mod+r").unwrap();
        app.press("role:button[Change Reconnect and read again]")
            .unwrap();
        app.wait_for("Press the new keys · Esc cancels").unwrap();
        // Taken by "Next project": refused, named, nothing changes.
        app.press_key("ctrl+p").unwrap();
        app.wait_for("Already used by “Next project”.").unwrap();
        app.wait_for("Reconnect is mod+r").unwrap();
        // A free keystroke is taken at once.
        app.press_key("alt+shift+k").unwrap();
        app.wait_for("Reconnect is alt+shift+k").unwrap();
        app.wait_for("Custom").unwrap();
        // Escape while recording stops the recording; the panel stays.
        app.press("role:button[Change Reconnect and read again]")
            .unwrap();
        app.wait_for("Press the new keys · Esc cancels").unwrap();
        app.press_key("Escape").unwrap();
        app.wait_for_gone(
            "Press the new keys · Esc cancels",
            std::time::Duration::from_secs(5),
        )
        .unwrap();
        app.wait_for("Keyboard shortcuts").unwrap();
        // Reset restores the default.
        app.press("role:button[Reset Reconnect and read again]")
            .unwrap();
        app.wait_for("Reconnect is mod+r").unwrap();
    }

    #[component]
    fn Rebinding() -> impl IntoElement {
        let model = signal(Model::loading());
        let fired = signal(0_u32);
        let global = KeyOptions {
            when: nsh_core::keymap::When::Always,
            ..Default::default()
        };
        bind(model, "project.next", global, move || {
            fired.update(|n| *n += 1)
        });
        view()
            .child(text(move || format!("Fired {}", fired.get())))
            .child(super::super::ui::button(model, "Rebind").on_press(move || {
                model.update(|m| {
                    m.keymap = {
                        let mut k = Keymap::default();
                        k.set("project.next", "alt+j");
                        k
                    }
                })
            }))
            .child(
                super::super::ui::button(model, "Record")
                    .on_press(move || model.update(|m| m.shortcut_recording = Some("reconnect"))),
            )
    }

    #[test]
    fn a_rebound_shortcut_fires_on_its_new_keys_only_and_none_fire_while_recording() {
        let mut app =
            nsh_testing::Headless::start(nsh_engine::EngineConfig::new(nsh_engine::Root::of::<
                Rebinding,
            >()))
            .unwrap();
        app.wait_for("Fired 0").unwrap();
        app.press_key("mod+p").unwrap();
        app.wait_for("Fired 1").unwrap();
        app.press("Rebind").unwrap();
        app.press_key("mod+p").unwrap();
        app.press_key("alt+j").unwrap();
        app.wait_for("Fired 2").unwrap();
        app.press("Record").unwrap();
        app.press_key("alt+j").unwrap();
        app.press_key("mod+p").unwrap();
        assert!(app.screen().texts().contains(&"Fired 2"));
    }
}
