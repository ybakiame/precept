//! The app's Markdown editor for a long value (a preference): a framed
//! document with a formatting toolbar, the document itself and a status bar
//! (word and character counts, and an Edit / Markdown switch that shows the
//! stored form agents read). NSH's `RichEditor` draws nothing of its own:
//! every node and mark is styled here, typography and spacing by classes,
//! colors from the app's palette. In a terminal it is a multi-line input over
//! the same Markdown. The caller owns the text.
use super::prelude::*;
use super::{ButtonSize, icon_button, icons};
use nsh_base::rich_editor::{
    commands::{BoxCommand, CommandRegistry},
    extensions::starter,
    formats::{DocFormat, MarkdownFormat},
    model::Schema,
    state::{EditorState, EditorStateConfig},
    view::{
        EditorSession, NodeViews, PartStyles, RichEditorClasses, RichEditorStyles, rich_editor,
    },
};
use nsh_base::text_view::PartStyle;
use nsh_core::element::{Style, StyleRefinement};
use serde_json::{Value, json};
use std::{collections::BTreeMap, rc::Rc};

/// The editor's height range, and the window height its dialog's other parts
/// (header, the other fields, toolbar, status bar, actions, margins) need
/// around it.
const EDITOR_MIN: f32 = 96.0;
const EDITOR_MAX: f32 = 280.0;
const EDITOR_CHROME: f32 = 520.0;

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

/// Words and characters of a Markdown text, for the status bar.
fn counts(text: &str) -> (usize, usize) {
    (text.split_whitespace().count(), text.chars().count())
}

/// Typography and spacing of the document's parts (no colors: those come
/// from the palette in [`styles`]). Sizes are steps of the type scale.
fn classes() -> RichEditorClasses {
    let map = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    };
    let mut c = RichEditorClasses::default();
    c.content = Some("gap-2 px-3 py-3".into());
    c.nodes = Some(map(&[
        ("paragraph", "leading-relaxed"),
        ("heading[level=1]", "text-xl font-semibold"),
        ("heading[level=2]", "text-base font-semibold"),
        ("heading[level=3]", "text-sm font-semibold"),
        ("blockquote", "border-l-2 pl-3 italic"),
        ("codeBlock", "font-mono text-xs rounded-md px-3 py-2"),
        ("bulletList", "gap-1 pl-6"),
        ("orderedList", "gap-1 pl-6"),
        ("taskList", "gap-1"),
        ("horizontalRule", "h-px my-2"),
    ]));
    c.marks = Some(map(&[
        ("bold", "font-semibold"),
        ("italic", "italic"),
        ("strike", "line-through"),
        ("code", "font-mono text-xs rounded px-1"),
        ("link", "underline"),
    ]));
    c
}

/// The document's colors, from the palette (rebuilt when the theme changes).
fn styles(model: Signal<Model>) -> RichEditorStyles {
    let p = palette(model);
    let part = |s: StyleRefinement| Some(PartStyle::from(Style::from(s)));
    let parts = |pairs: Vec<(&str, StyleRefinement)>| {
        Some(PartStyles(
            pairs
                .into_iter()
                .map(|(k, s)| (k.to_owned(), Style::from(s)))
                .collect(),
        ))
    };
    let mut s = RichEditorStyles::default();
    s.placeholder = part(StyleRefinement::default().text_color(p.muted));
    s.list_marker = part(StyleRefinement::default().text_color(p.muted));
    s.caret = part(StyleRefinement::default().bg(p.primary));
    s.selection = part(StyleRefinement::default().bg(p.primary.opacity(0.28)));
    s.nodes = parts(vec![
        (
            "blockquote",
            StyleRefinement::default()
                .border_color(p.border)
                .text_color(p.muted),
        ),
        ("codeBlock", StyleRefinement::default().bg(p.inset)),
        ("horizontalRule", StyleRefinement::default().bg(p.hairline)),
    ]);
    s.marks = parts(vec![
        ("code", StyleRefinement::default().bg(p.inset)),
        ("link", StyleRefinement::default().text_color(p.primary)),
    ]);
    s
}

/// A toolbar command by the built-in registry's name and arguments.
fn make(schema: &Schema, name: &str, args: Value) -> Option<BoxCommand> {
    CommandRegistry::builtin().make(name, &args, schema).ok()
}

/// One toggle of the toolbar: what it runs and what makes it pressed.
struct Tool {
    icon: &'static str,
    label: &'static str,
    command: Option<BoxCommand>,
    active: Option<(&'static str, Option<Value>)>,
}

fn tools(schema: &Schema) -> Vec<Vec<Tool>> {
    let mark = |icon, label, name: &'static str| Tool {
        icon,
        label,
        command: make(schema, "toggleMark", json!({ "name": name })),
        active: Some((name, None)),
    };
    let list = |icon, label, name: &'static str| Tool {
        icon,
        label,
        command: make(
            schema,
            "toggleList",
            json!({ "listName": name, "itemName": "listItem" }),
        ),
        active: Some((name, None)),
    };
    vec![
        vec![
            Tool {
                icon: icons::UNDO,
                label: "Undo",
                command: make(schema, "undo", Value::Null),
                active: None,
            },
            Tool {
                icon: icons::REDO,
                label: "Redo",
                command: make(schema, "redo", Value::Null),
                active: None,
            },
        ],
        vec![
            mark(icons::BOLD, "Bold", "bold"),
            mark(icons::ITALIC, "Italic", "italic"),
            mark(icons::STRIKE, "Strikethrough", "strike"),
            mark(icons::CODE, "Inline code", "code"),
        ],
        vec![
            Tool {
                icon: icons::HEADING,
                label: "Heading",
                command: make(
                    schema,
                    "toggleNode",
                    json!({ "name": "heading", "toggleName": "paragraph", "attrs": { "level": 2 } }),
                ),
                active: Some(("heading", Some(json!({ "level": 2 })))),
            },
            list(icons::LIST, "Bulleted list", "bulletList"),
            list(icons::LIST_ORDERED, "Numbered list", "orderedList"),
            Tool {
                icon: icons::QUOTE,
                label: "Quote",
                command: make(schema, "toggleWrap", json!({ "name": "blockquote" })),
                active: Some(("blockquote", None)),
            },
            Tool {
                icon: icons::CODE_BLOCK,
                label: "Code block",
                command: make(
                    schema,
                    "toggleNode",
                    json!({ "name": "codeBlock", "toggleName": "paragraph" }),
                ),
                active: Some(("codeBlock", None)),
            },
        ],
    ]
}

fn attrs_of(value: &Option<Value>) -> Option<nsh_base::rich_editor::model::Attrs> {
    let Value::Object(map) = value.as_ref()? else {
        return None;
    };
    Some(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
}

/// The toolbar: undo and redo, the marks, then the blocks; each a square
/// icon button named for screen readers, pressed while it applies at the
/// selection. `revision` changes with the document and the selection.
fn toolbar(model: Signal<Model>, editor: &EditorSession, revision: Signal<u64>) -> AnyElement {
    let schema = editor.state().schema().clone();
    let mut bar = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_0p5()
        .px_1()
        .py_1()
        .border_b_1()
        .border_color(move || palette(model).hairline)
        .bg(move || palette(model).sidebar);
    for (index, group) in tools(&schema).into_iter().enumerate() {
        if index > 0 {
            bar = bar.child(
                view()
                    .w(px(1.0))
                    .h(px(scale::icon::MD))
                    .mx_1()
                    .bg(move || palette(model).hairline),
            );
        }
        for tool in group {
            let Some(command) = tool.command else {
                continue;
            };
            let (run_editor, can_editor, on_editor) =
                (editor.clone(), editor.clone(), editor.clone());
            let run = command.clone();
            let active = tool.active.clone();
            let label = tool.label;
            bar = bar.child(
                icon_button(model, tool.icon, move || label.into())
                    .size(ButtonSize::Small)
                    .selected(move || {
                        let _ = revision.get();
                        active.as_ref().is_some_and(|(name, attrs)| {
                            on_editor.is_active(Some(name), attrs_of(attrs))
                        })
                    })
                    .disabled(move || {
                        let _ = revision.get();
                        !can_editor.can(&command)
                    })
                    .on_press(move || {
                        run_editor.run(&run);
                        run_editor.focus();
                        revision.update(|r| *r += 1);
                    }),
            );
        }
    }
    bar.into_element()
}

/// Read-only Markdown (a rule's text): rendered with the theme's text color,
/// selectable for copying.
pub fn markdown_view(model: Signal<Model>, markdown: String) -> impl IntoElement {
    view()
        .text_color(move || palette(model).text)
        .text_size(px(scale::text::BODY))
        .child(nsh_base::text_view().markdown(markdown).selectable(true))
}

/// A labelled Markdown editor (`id`: its stable test id): `initial` is read
/// once when it is made; `on_change` receives the Markdown after every edit.
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
    let source_text = signal(start);
    let revision = signal(0_u64);
    let source = signal(false);
    // The document's colors change with the theme only, not with each edit.
    let look = computed(move || {
        let m = model.get();
        (m.theme, m.color_mode, m.high_contrast, m.dark)
    });
    let height = move || px((window().height.0 - EDITOR_CHROME).clamp(EDITOR_MIN, EDITOR_MAX));
    let changed = editor.clone();
    let document = rich_editor()
        .editor(editor.clone())
        .test_id(id)
        .accessibility_label(move || Some(label()))
        .classes(classes())
        .styles(move || {
            let _ = look.get();
            untracked(|| styles(model))
        })
        .w_full()
        .min_h(px(EDITOR_MIN))
        .h(height)
        .text_size(px(scale::text::BODY))
        .text_color(move || palette(model).text)
        .style_when(move || source.get(), |s| s.hidden())
        .on_selection_change(move |_: &_| revision.update(|r| *r += 1))
        .on_change(move |_: &_| {
            revision.update(|r| *r += 1);
            if let Some(markdown) = markdown(&changed) {
                source_text.set(markdown.clone());
                on_change(markdown);
            }
        });
    // The stored form, as agents read it.
    let markdown_view = scroll()
        .w_full()
        .min_h(px(EDITOR_MIN))
        .h(height)
        .style_when(move || !source.get(), |s| s.hidden())
        .child(
            view().px_3().py_3().flex_shrink_0().child(
                text(move || source_text.get())
                    .font_family("monospace")
                    .text_size(px(scale::text::CAPTION))
                    .text_color(move || palette(model).text),
            ),
        );
    let status = view()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .px_2()
        .py_1()
        .border_t_1()
        .border_color(move || palette(model).hairline)
        .child(
            text(move || {
                let (words, chars) = counts(&source_text.get());
                format!("{words}\u{a0}words · {chars}\u{a0}characters")
            })
            .text_size(px(scale::text::CAPTION))
            .text_color(move || palette(model).muted),
        )
        .child(
            super::tabs(model)
                .add("Edit", move || !source.get(), move || source.set(false))
                .add("Markdown", move || source.get(), move || source.set(true)),
        );
    view()
        .w_full()
        .rounded(px(scale::radius::CONTROL))
        .border_1()
        .border_color(move || palette(model).border)
        .bg(move || palette(model).panel)
        .overflow_hidden()
        .focus_within(move |s| {
            s.border_color(palette(model).primary)
                .ring(px(3.0))
                .ring_color(palette(model).primary.opacity(0.25))
        })
        .child(toolbar(model, &editor, revision))
        .child(document)
        .child(markdown_view)
        .child(status)
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

    #[test]
    fn every_toolbar_command_exists_in_the_starter_schema() {
        let schema = starter::schema();
        for group in tools(&schema) {
            for tool in group {
                assert!(tool.command.is_some(), "{} has no command", tool.label);
            }
        }
    }

    #[test]
    fn the_status_bar_counts_words_and_characters() {
        assert_eq!(counts("Prefer **small** commits."), (3, 25));
        assert_eq!(counts(""), (0, 0));
    }
}
