//! A Markdown editor for a long value (a preference). On the desktop it is a
//! `RichEditor` over the starter schema, read from and written back to
//! Markdown, the stored form agents read; in a terminal it is a multi-line
//! text input over the same Markdown. The caller owns the text.
use super::prelude::*;
use nsh_base::rich_editor::{
    extensions::starter,
    formats::{DocFormat, MarkdownFormat},
    state::{EditorState, EditorStateConfig},
    view::{EditorSession, NodeViews, rich_editor},
};
use std::rc::Rc;

/// The editor's height range, and the window height its dialog's other parts
/// (header, the other fields, actions, margins) need around it.
const EDITOR_MIN: f32 = 96.0;
const EDITOR_MAX: f32 = 320.0;
const EDITOR_CHROME: f32 = 440.0;

/// Markdown → the editor's session (an empty document if it does not parse).
fn session(markdown: &str) -> Option<EditorSession> {
    let schema = starter::schema();
    let plugins = starter::plugins(&schema);
    let format = MarkdownFormat::default();
    let doc = format
        .parse(&schema, markdown)
        .ok()
        .or_else(|| format.parse(&schema, "").ok())?;
    let state =
        EditorState::create(EditorStateConfig::new(schema).doc(doc).plugins(plugins)).ok()?;
    Some(EditorSession::new(state, NodeViews::new()))
}
/// The session's document as Markdown.
fn markdown(session: &EditorSession) -> Option<String> {
    // Without the writer's trailing newline: agents read the value as written.
    MarkdownFormat::default()
        .serialize(session.state().doc())
        .ok()
        .map(|text| text.trim_end().to_owned())
}

/// A labelled Markdown editor (`id`: its stable test id): `initial` is read
/// once when it is made;
/// `on_change` receives the Markdown after every edit.
pub fn markdown_input(
    model: Signal<Model>,
    id: &'static str,
    label: impl Fn() -> SharedString + 'static,
    initial: impl Fn() -> String + 'static,
    on_change: impl Fn(String) + 'static,
) -> impl IntoElement {
    let on_change = Rc::new(on_change);
    let label = Rc::new(label);
    if terminal() {
        let change = on_change.clone();
        let label = label.clone();
        return super::input::input(model, "")
            .label_fn(move || label())
            .multiline(6)
            .test_id(id)
            .value(initial)
            .on_change(move |v: &str| change(v.to_owned()))
            .into_element();
    }
    let start = untracked(initial);
    let Some(editor) = session(&start) else {
        // A schema that cannot be built never loses the value: plain text.
        let change = on_change.clone();
        return super::input::input(model, "")
            .label_fn(move || label())
            .multiline(6)
            .test_id(id)
            .value(move || start.clone())
            .on_change(move |v: &str| change(v.to_owned()))
            .into_element();
    };
    view()
        .w_full()
        .min_h(px(EDITOR_MIN))
        // Yields height on a short window so the dialog around it still fits.
        .max_h(move || px((window().height.0 - EDITOR_CHROME).clamp(EDITOR_MIN, EDITOR_MAX)))
        .p_2()
        .border_1()
        .rounded(px(scale::radius::CONTROL))
        .border_color(move || palette(model).border)
        .bg(move || palette(model).panel)
        .focus_within(move |s| {
            s.border_color(palette(model).primary)
                .ring(px(3.0))
                .ring_color(palette(model).primary.opacity(0.25))
        })
        .child(
            rich_editor()
                .editor(editor.clone())
                .test_id(id)
                .accessibility_label(move || Some(label()))
                .size_full()
                .on_change(move |_: &_| {
                    if let Some(text) = markdown(&editor) {
                        on_change(text);
                    }
                }),
        )
        .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn markdown_round_trips_through_the_editor_schema() {
        for text in [
            "Prefer **small** commits.",
            "- run tests\n- then lint",
            "plain",
        ] {
            let s = session(text).unwrap();
            assert_eq!(markdown(&s).unwrap(), text);
        }
    }
}
