//! The app's `Menu`: a labelled trigger and a list of choices on the app's one
//! popup surface. Built on the semantic `nsh/base` menu (WAI-ARIA menu button:
//! Return / Space / Down open it with focus on the first item, arrows move,
//! Escape and an outside press close it, choosing closes it; menu and menu
//! item roles). The popup grows from its trigger and leaves faster; a checked
//! choice carries its mark in a fixed lane so labels never shift.
use super::prelude::*;
use nsh_base::{menu as base_menu, menu_item as base_menu_item};
use nsh_core::element::StyleRefinement;
use std::rc::Rc;

type LabelFn = Rc<dyn Fn() -> SharedString>;
type StateFn = Rc<dyn Fn() -> bool>;
type PressFn = Rc<dyn Fn()>;

struct Item {
    label: SharedString,
    checked: StateFn,
    on_select: PressFn,
}

/// A menu builder; create it with [`menu`] and add choices in reading order.
pub struct Menu {
    model: Signal<Model>,
    label: LabelFn,
    items: Vec<Item>,
}

/// A menu whose full-width trigger shows `label` (the current choice).
pub fn menu(model: Signal<Model>, label: impl Fn() -> SharedString + 'static) -> Menu {
    Menu {
        model,
        label: Rc::new(label),
        items: Vec::new(),
    }
}

impl Menu {
    /// A choice: its label, whether it is the current one, and what choosing
    /// it does.
    pub fn item(
        mut self,
        label: impl Into<SharedString>,
        checked: impl Fn() -> bool + 'static,
        on_select: impl Fn() + 'static,
    ) -> Self {
        self.items.push(Item {
            label: label.into(),
            checked: Rc::new(checked),
            on_select: Rc::new(on_select),
        });
        self
    }
}

/// The popup surface shared by every anchored popup of the app.
fn popup(model: Signal<Model>) -> impl Fn(StyleRefinement) -> StyleRefinement + 'static {
    move |s| {
        s.min_w(px(scale::layout::POPUP))
            .p_1()
            .gap_0p5()
            .rounded(px(scale::radius::CONTAINER))
            .border_1()
            .border_color(palette(model).border)
            .bg(palette(model).panel)
            .shadow_lg()
            .transition()
            .duration(motion::ms(motion::SLOW))
            .ease_out()
    }
}

const CHEVRON: &str = "M7 10L12 15L17 10";
const CHECK: &str = "M5 12.5L10 17L19 7";

impl IntoElement for Menu {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        let label = self.label.clone();
        // The trigger's visible text names it; the base menu gives it the
        // menu-button role and expanded state.
        let trigger = view()
            .flex_row()
            .items_center()
            .gap_2()
            .w_full()
            .px_2()
            .min_h(px(scale::control::MD))
            .rounded(px(scale::radius::CONTROL))
            .border_2()
            .border_color(move || palette(model).border)
            .bg(move || palette(model).inset)
            .text_color(move || palette(model).text)
            .text_size(px(scale::text::BODY))
            .hover(move |s| s.bg(mix(palette(model).inset, palette(model).text, 0.07)))
            .active(move |s| s.bg(mix(palette(model).inset, palette(model).text, 0.13)))
            .focus(move |s| s.border_color(palette(model).primary))
            .child(text(move || label()).flex_1())
            .child(icon::glyph(CHEVRON, scale::icon::SM));
        let items: Vec<AnyElement> = self
            .items
            .into_iter()
            .map(|item| {
                let (checked, mark, on_select) = (
                    item.checked.clone(),
                    item.checked.clone(),
                    item.on_select.clone(),
                );
                base_menu_item()
                    .checked(move || Some(checked()))
                    .role(Some(AccessibilityRole::MenuItemRadio))
                    .on_select(move || on_select())
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .min_h(px(scale::control::MD))
                    .rounded(px(scale::radius::CONTROL))
                    .text_size(px(scale::text::BODY))
                    .text_color(move || palette(model).text)
                    .hover(move |s| s.bg(palette(model).inset))
                    .active(move |s| s.bg(mix(palette(model).inset, palette(model).text, 0.07)))
                    .focus(move |s| s.bg(palette(model).inset))
                    .child(
                        // Fixed lane: the mark never moves the label.
                        view()
                            .size(px(scale::icon::SM))
                            .items_center()
                            .justify_center()
                            .child(
                                view()
                                    .style_when(move || !mark(), |s| s.hidden())
                                    .child(icon::glyph(CHECK, scale::icon::SM)),
                            ),
                    )
                    .child(text(item.label.clone()))
                    .into_element()
            })
            .collect();
        base_menu()
            .offset(px(4.0))
            .popup_style(popup(model))
            .popup_starting_style(move |s| {
                if reduce_motion() {
                    s
                } else {
                    s.opacity(0.0).scale(motion::ENTER)
                }
            })
            .popup_ending_style(move |s| {
                if reduce_motion() {
                    s
                } else {
                    s.opacity(0.0).duration(motion::BASE).ease_in()
                }
            })
            .trigger(motion::feedback(trigger))
            .children(items)
            .into_element()
    }
}
