//! The app's tags: short classification labels of a preference.
//!
//! - [`tag`] shows one: a neutral inset chip with a tag glyph (`#name` in a
//!   terminal), never a semantic color.
//! - [`tag_toggle`] filters by one: a small button with its count whose
//!   selected state is the button's own (fill, `selected` for screen readers,
//!   and a check glyph), never color alone.
//! - [`tag_editor`] edits a draft's tags: chips that pop in with a remove
//!   button each, an input that adds on Enter or a comma, the limit and
//!   validity in its help line, and suggestion buttons below.
use super::prelude::*;
use acp_ui_core::i18n::Msg;
use acp_ui_core::operator::{MAX_TAGS, normalize_tag};
use std::rc::Rc;

/// One tag, as shown on a row or a record: its color as a dot and a light
/// tint behind theme text (`#name` in its color in a terminal).
pub fn tag(model: Signal<Model>, name: &str) -> AnyElement {
    let key = name.to_owned();
    let label: SharedString = acp_ui_core::presentation::sanitize(name, 32).into();
    if terminal() {
        return text(format!("#{label}"))
            .text_color(move || super::tag_color(model, &key))
            .into_element();
    }
    let (tint, edge, dot) = (key.clone(), key.clone(), key);
    view()
        .flex_row()
        .items_center()
        .gap_1()
        .px_1p5()
        .py_0p5()
        .rounded(px(scale::radius::SMALL))
        .border_1()
        .bg(move || mix(palette(model).inset, super::tag_color(model, &tint), 0.16))
        .border_color(move || {
            mix(
                palette(model).hairline,
                super::tag_color(model, &edge),
                0.45,
            )
        })
        .text_color(move || palette(model).text)
        .transition_colors()
        .duration(motion::ms(motion::FAST))
        .ease_out()
        .child(super::color_dot(move || super::tag_color(model, &dot), 8.0))
        .child(text(label).text_size(px(scale::text::CAPTION)))
        .into_element()
}

/// A row of tags that wraps; nothing when there are none.
pub fn tag_list(model: Signal<Model>, names: &[String]) -> AnyElement {
    let mut row = view().flex_row().flex_wrap().gap_1();
    for name in names {
        row = row.child(tag(model, name));
    }
    row.into_element()
}

/// A filter by `name` carried by `count` rows; pressing it toggles it.
pub fn tag_toggle(
    model: Signal<Model>,
    name: &str,
    count: usize,
    selected: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> super::button::Button {
    let label: SharedString = format!(
        "#{} · {count}",
        acp_ui_core::presentation::sanitize(name, 32)
    )
    .into();
    let selected = Rc::new(selected);
    let glyph = selected.clone();
    let dot = name.to_owned();
    super::button(model, label)
        .size(super::ButtonSize::Small)
        .variant(super::ButtonVariant::Ghost)
        .selected(move || selected())
        .leading(dynamic(move || {
            if glyph() {
                let pop = motion::pop();
                view()
                    .text_color(move || palette(model).primary)
                    .bind_motion(&pop)
                    .child(icon::glyph(super::icons::CHECK, scale::icon::SM))
                    .into_element()
            } else {
                let dot = dot.clone();
                view()
                    .size(px(scale::icon::SM))
                    .items_center()
                    .justify_center()
                    .child(super::color_dot(move || super::tag_color(model, &dot), 8.0))
                    .into_element()
            }
        }))
        .on_press(on_press)
}

type TagsFn = Rc<dyn Fn() -> Vec<String>>;

/// The draft's tags: what is chosen, what is typed, what is offered, and what
/// adding, removing and typing do. `add` returns whether the tag was taken.
pub struct TagEditor {
    model: Signal<Model>,
    reference: Option<ElementRef>,
    tags: TagsFn,
    draft: Rc<dyn Fn() -> String>,
    suggestions: TagsFn,
    on_draft: Rc<dyn Fn(&str)>,
    on_add: Rc<dyn Fn(&str) -> bool>,
    on_remove: Rc<dyn Fn(&str)>,
    disabled: Rc<dyn Fn() -> bool>,
}

/// An editor over `tags` (read each time they change).
pub fn tag_editor(
    model: Signal<Model>,
    tags: impl Fn() -> Vec<String> + 'static,
    draft: impl Fn() -> String + 'static,
) -> TagEditor {
    TagEditor {
        model,
        reference: None,
        tags: Rc::new(tags),
        draft: Rc::new(draft),
        suggestions: Rc::new(Vec::new),
        on_draft: Rc::new(|_| {}),
        on_add: Rc::new(|_| false),
        on_remove: Rc::new(|_| {}),
        disabled: Rc::new(|| false),
    }
}

impl TagEditor {
    pub fn suggestions(mut self, suggestions: impl Fn() -> Vec<String> + 'static) -> Self {
        self.suggestions = Rc::new(suggestions);
        self
    }
    pub fn on_draft(mut self, on_draft: impl Fn(&str) + 'static) -> Self {
        self.on_draft = Rc::new(on_draft);
        self
    }
    pub fn on_add(mut self, on_add: impl Fn(&str) -> bool + 'static) -> Self {
        self.on_add = Rc::new(on_add);
        self
    }
    pub fn on_remove(mut self, on_remove: impl Fn(&str) + 'static) -> Self {
        self.on_remove = Rc::new(on_remove);
        self
    }
    pub fn disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
        self.disabled = Rc::new(disabled);
        self
    }
    /// The input's reference, for a shortcut that focuses it.
    pub fn element_ref(mut self, reference: &ElementRef) -> Self {
        self.reference = Some(*reference);
        self
    }
}

/// What the help line says about the draft right now.
fn help(model: Signal<Model>, tags: &[String], draft: &str) -> (SharedString, bool) {
    let m = model.get();
    let draft = draft.trim().trim_end_matches(',');
    if !draft.is_empty() && normalize_tag(draft).is_none() {
        return (m.word(Msg::TagInvalid).into(), true);
    }
    if tags.len() >= MAX_TAGS && !draft.is_empty() {
        return (m.word(Msg::TagLimit).into(), true);
    }
    let count = m
        .word(Msg::TagCount)
        .replace("{count}", &tags.len().to_string());
    (format!("{} · {count}", m.word(Msg::TagsHelp)).into(), false)
}

impl IntoElement for TagEditor {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        let Self {
            tags,
            draft,
            suggestions,
            on_draft,
            on_add,
            on_remove,
            disabled,
            reference,
            ..
        } = self;
        let state = {
            let (tags, draft) = (tags.clone(), draft.clone());
            computed(move || help(model, &tags(), &draft()))
        };
        // Typing a comma commits what precedes it; Enter commits the rest.
        let change = {
            let (on_add, on_draft) = (on_add.clone(), on_draft.clone());
            move |value: &str| {
                if let Some((done, rest)) = value.rsplit_once(',') {
                    let mut kept = String::new();
                    for part in done.split(',') {
                        if !part.trim().is_empty() && !on_add(part) {
                            kept = part.to_owned();
                        }
                    }
                    on_draft(if kept.is_empty() { rest } else { &kept });
                } else {
                    on_draft(value);
                }
            }
        };
        let submit = {
            let (on_add, on_draft) = (on_add.clone(), on_draft.clone());
            move |value: &str| {
                if !value.trim().is_empty() && on_add(value) {
                    on_draft("");
                }
            }
        };
        let chips = {
            let (tags, on_remove, disabled) = (tags.clone(), on_remove.clone(), disabled.clone());
            dynamic(move || {
                let mut row = view().flex_row().flex_wrap().items_center().gap_1();
                for name in tags() {
                    let remove = on_remove.clone();
                    let spoken_name = name.clone();
                    let pressed = name.clone();
                    let pop = motion::pop();
                    let disabled = disabled.clone();
                    row = row.child(
                        view()
                            .flex_row()
                            .items_center()
                            .bind_motion(&pop)
                            .child(tag(model, &name))
                            .child(
                                super::icon_button(model, super::icons::CLOSE, move || {
                                    model
                                        .get()
                                        .word(Msg::TagRemove)
                                        .replace("{tag}", &spoken_name)
                                        .into()
                                })
                                .size(super::ButtonSize::Small)
                                .disabled(move || disabled())
                                .on_press(move || remove(&pressed)),
                            ),
                    );
                }
                row.into_element()
            })
        };
        let offered = {
            let (suggestions, on_add, on_draft, disabled) = (
                suggestions.clone(),
                on_add.clone(),
                on_draft.clone(),
                disabled.clone(),
            );
            let tags = tags.clone();
            dynamic(move || {
                let names = suggestions();
                if names.is_empty() || tags().len() >= MAX_TAGS {
                    return view().into_element();
                }
                let mut row = view().flex_row().flex_wrap().items_center().gap_1().child(
                    text(model.get().word(Msg::TagSuggestions))
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                );
                for name in names {
                    let (add, draft, spoken) = (on_add.clone(), on_draft.clone(), name.clone());
                    let disabled = disabled.clone();
                    row = row.child(
                        super::button(model, format!("#{name}"))
                            .size(super::ButtonSize::Small)
                            .variant(super::ButtonVariant::Ghost)
                            .icon(super::icons::PLUS)
                            .spoken(move || {
                                model
                                    .get()
                                    .word(Msg::TagAdd)
                                    .replace("{tag}", &spoken)
                                    .into()
                            })
                            .disabled(move || disabled())
                            .on_press(move || {
                                if add(&name) {
                                    draft("");
                                }
                            }),
                    );
                }
                row.into_element()
            })
        };
        let mut input = super::input(model, "")
            .label_fn(move || model.get().word(Msg::OperatorTags).into())
            .placeholder(model.get().word(Msg::TagPlaceholder))
            .value(move || draft())
            .invalid(move || state.get().1)
            .disabled({
                let disabled = disabled.clone();
                move || disabled()
            })
            .max_length(64)
            .on_change(change)
            .on_submit(submit);
        if let Some(reference) = &reference {
            input = input.element_ref(reference);
        }
        super::field(
            model,
            move || model.get().word(Msg::OperatorTags).into(),
            view()
                .w_full()
                .gap_2()
                .child(chips)
                .child(input)
                .child(offered),
        )
        .help(move || state.get().0)
        .into_element()
    }
}
