use crate::*;
use precept_ui_core::{i18n::Msg, rule_templates::RuleTemplate};
use tokens::palette;
use ui::{ButtonSize, dialog, scale, tabs};

fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}

fn template_choices(model: Signal<Model>) -> impl IntoElement {
    RuleTemplate::ALL
        .into_iter()
        .fold(tabs(model), |group, template| {
            group.add_fn(
                move || template.label(model.get().locale).into(),
                move || model.get().rule_lint.selected_template == Some(template),
                move || model.update(|m| m.rule_lint.selected_template = Some(template)),
            )
        })
}

fn preview(model: Signal<Model>) -> impl IntoElement {
    dynamic(move || {
        let Some(template) = model.get().rule_lint.selected_template else {
            return view().into_element();
        };
        view()
            .gap_3()
            .child(
                view()
                    .gap_1()
                    .child(
                        text(move || -> SharedString {
                            model.get().word(Msg::TemplatePath).into()
                        })
                        .text_size(px(scale::text::CAPTION))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(move || palette(model).muted),
                    )
                    .child(
                        text(template.path())
                            .font_family("monospace")
                            .selectable(true),
                    ),
            )
            .child(
                text(move || -> SharedString { template.description(model.get().locale).into() })
                    .text_size(px(scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
            )
            .child(
                view()
                    .gap_1()
                    .child(
                        text(move || -> SharedString {
                            model.get().word(Msg::TemplatePreview).into()
                        })
                        .text_size(px(scale::text::CAPTION))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(move || palette(model).muted),
                    )
                    .child(
                        text(template.text())
                            .font_family("monospace")
                            .text_size(px(scale::text::CAPTION))
                            .selectable(true)
                            .p_3()
                            .rounded(px(scale::radius::CONTROL))
                            .border_1()
                            .border_color(move || palette(model).hairline)
                            .bg(move || palette(model).inset),
                    ),
            )
            .child(
                view().flex_row().flex_wrap().items_center().gap_2().child(
                    ui::button(model, "")
                        .label_fn(word(model, Msg::CopyTemplate))
                        .icon(ui::icons::COPY)
                        .size(ButtonSize::Small)
                        .on_press(move || {
                            set_clipboard(template.text());
                            ui::notify(ui::Tone::Success, model.get().word(Msg::TemplateCopied));
                        }),
                ),
            )
            .into_element()
    })
}

/// Four explicit starting points with exact path/text preview. The dialog is
/// mounted once in the shell and owns no write action; Done and Escape dismiss.
pub(crate) fn templates_dialog(model: Signal<Model>) -> impl IntoElement {
    dialog(model, "")
        .size(ui::DialogSize::Large)
        .title_fn(word(model, Msg::TemplatesTitle))
        .description_fn(word(model, Msg::TemplatesHelp))
        .content_fn(move || {
            view()
                .gap_4()
                .child(template_choices(model))
                .child(preview(model))
                .into_element()
        })
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(word(model, Msg::TemplateDone))
        .open(move || model.get().rule_lint.selected_template.is_some())
        .on_cancel(move || model.update(|m| m.rule_lint.close_templates()))
        .on_confirm(move || model.update(|m| m.rule_lint.close_templates()))
}
