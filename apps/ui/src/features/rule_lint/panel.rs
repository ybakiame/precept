use super::check;
use crate::*;
use precept_ui_core::{
    i18n::{Locale, Msg},
    rule_lint::{FileStatus, LintReport, LintStatus, Severity},
};
use tokens::palette;
use ui::{BadgeVariant, ButtonSize, ButtonVariant, scale, surface};

const MAX_VISIBLE_FILES: usize = 20;
const MAX_VISIBLE_DIAGNOSTICS: usize = 40;

fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}

fn caption(model: Signal<Model>, value: SharedString) -> impl IntoElement {
    text(value)
        .text_size(px(scale::text::CAPTION))
        .text_color(move || palette(model).muted)
}

#[derive(Clone, PartialEq)]
struct ViewState {
    busy: bool,
    live: bool,
    locale: Locale,
    phase: Phase,
    report: Option<LintReport>,
    error: Option<&'static str>,
}

fn banner(
    model: Signal<Model>,
    title: Msg,
    help: Msg,
    variant: BadgeVariant,
    badge_text: Msg,
) -> AnyElement {
    let m = model.get();
    let reveal = ui::motion::reveal();
    view()
        .gap_1()
        .bind_motion(&reveal)
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(text(m.word(title)).font_weight(FontWeight::SEMIBOLD))
                .child(ui::badge(model, m.word(badge_text)).variant(variant)),
        )
        .child(caption(model, m.word(help).into()))
        .into_element()
}

fn position(diagnostic: &precept_ui_core::rule_lint::Diagnostic) -> String {
    if (diagnostic.end_line, diagnostic.end_column) == (diagnostic.line, diagnostic.column) {
        format!("{}:{}", diagnostic.line, diagnostic.column)
    } else {
        format!(
            "{}:{}-{}:{}",
            diagnostic.line, diagnostic.column, diagnostic.end_line, diagnostic.end_column
        )
    }
}

fn diagnostic_row(
    model: Signal<Model>,
    diagnostic: &precept_ui_core::rule_lint::Diagnostic,
) -> AnyElement {
    let m = model.get();
    let (severity, variant) = match diagnostic.severity {
        Severity::Error => (Msg::RuleLintSeverityError, BadgeVariant::Danger),
        Severity::Warning => (Msg::RuleLintSeverityWarning, BadgeVariant::Warning),
    };
    let mut row = view()
        .gap_1()
        .py_2()
        .border_t_1()
        .border_color(move || palette(model).hairline)
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(ui::badge(model, m.word(severity)).variant(variant))
                .child(
                    text(diagnostic.code.as_str())
                        .font_family("monospace")
                        .text_size(px(scale::text::CAPTION))
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .child(
                    text(position(diagnostic))
                        .font_family("monospace")
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                ),
        )
        .child(text(diagnostic.message.clone()));
    if let Some(related) = &diagnostic.related_file {
        row = row.child(caption(
            model,
            m.word(Msg::RuleLintRelated)
                .replace("{file}", related)
                .into(),
        ));
    }
    row.into_element()
}

fn issue_files(model: Signal<Model>, report: &LintReport) -> AnyElement {
    let m = model.get();
    let mut list = view().gap_2();
    let mut shown = 0_usize;
    let mut total = 0_usize;
    let mut files_shown = 0_usize;
    for file in report
        .files
        .iter()
        .filter(|file| file.status != FileStatus::Ok)
    {
        total += file.diagnostics.len();
        if files_shown == MAX_VISIBLE_FILES {
            continue;
        }
        files_shown += 1;
        let has_errors = file
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error);
        let (status, variant) = match file.status {
            FileStatus::Ok => (Msg::RuleLintFileOk, BadgeVariant::Success),
            FileStatus::Issues if has_errors => (Msg::RuleLintFileIssues, BadgeVariant::Danger),
            FileStatus::Issues => (Msg::RuleLintFileIssues, BadgeVariant::Warning),
            FileStatus::Unavailable => (Msg::RuleLintFileUnavailable, BadgeVariant::Warning),
        };
        let mut group = view().gap_1().child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    text(file.file.clone())
                        .font_family("monospace")
                        .text_size(px(scale::text::CAPTION))
                        .font_weight(FontWeight::SEMIBOLD)
                        .selectable(true),
                )
                .child(ui::badge(model, m.word(status)).variant(variant)),
        );
        for diagnostic in &file.diagnostics {
            if shown == MAX_VISIBLE_DIAGNOSTICS {
                continue;
            }
            shown += 1;
            group = group.child(diagnostic_row(model, diagnostic));
        }
        list = list.child(group);
    }
    if total > shown {
        list = list.child(caption(
            model,
            m.word(Msg::RuleLintMore)
                .replace("{shown}", &shown.to_string())
                .replace("{total}", &total.to_string())
                .into(),
        ));
    }
    list.into_element()
}

fn report_view(model: Signal<Model>, report: &LintReport) -> AnyElement {
    let m = model.get();
    let reveal = ui::motion::reveal();
    let mut body = view().gap_3().bind_motion(&reveal);
    match report.status {
        LintStatus::Ok => {
            body = body.child(banner(
                model,
                Msg::RuleLintClean,
                Msg::RuleLintCleanHelp,
                BadgeVariant::Success,
                Msg::RuleLintBadgeClean,
            ));
        }
        LintStatus::NotConfigured => {
            body = body.child(banner(
                model,
                Msg::RuleLintNotConfigured,
                Msg::RuleLintNotConfiguredHelp,
                BadgeVariant::Neutral,
                Msg::RuleLintBadgeNotConfigured,
            ));
        }
        LintStatus::Issues => {
            body = body.child(
                view().flex_row().flex_wrap().items_center().gap_2().child(
                    ui::badge(
                        model,
                        m.word(Msg::RuleLintIssues)
                            .replace("{errors}", &report.summary.errors.to_string())
                            .replace("{warnings}", &report.summary.warnings.to_string())
                            .replace("{files}", &report.summary.files.to_string()),
                    )
                    .variant(if report.summary.errors > 0 {
                        BadgeVariant::Danger
                    } else {
                        BadgeVariant::Warning
                    }),
                ),
            );
            body = body.child(issue_files(model, report));
        }
        LintStatus::Unavailable => {
            body = body.child(banner(
                model,
                Msg::RuleLintUnavailable,
                Msg::RuleLintUnavailableHelp,
                BadgeVariant::Warning,
                Msg::RuleLintFileUnavailable,
            ));
            body = body.child(issue_files(model, report));
        }
    }
    body = body.child(
        view().items_start().child(
            ui::button(model, "")
                .label_fn(word(model, Msg::RuleLintCopyReport))
                .size(ButtonSize::Small)
                .on_press({
                    let report = report.clone();
                    move || {
                        drop(set_clipboard(report.plain_text()));
                        ui::notify(ui::Tone::Success, model.get().word(Msg::RuleLintCopied));
                    }
                }),
        ),
    );
    body.into_element()
}

fn state_view(model: Signal<Model>, state: ViewState) -> AnyElement {
    let m = model.get();
    if state.busy {
        let reveal = ui::motion::reveal();
        return view()
            .flex_row()
            .items_center()
            .gap_2()
            .bind_motion(&reveal)
            .child(caption(model, m.word(Msg::RuleLintChecking).into()))
            .into_element();
    }
    if state.error.is_some() {
        return banner(
            model,
            Msg::RuleLintError,
            Msg::RuleLintErrorHelp,
            BadgeVariant::Danger,
            Msg::RuleLintSeverityError,
        );
    }
    if let Some(report) = state.report {
        return report_view(model, &report);
    }
    if !state.live {
        return banner(
            model,
            Msg::RuleLintOffline,
            Msg::RuleLintOfflineHelp,
            BadgeVariant::Neutral,
            Msg::Offline,
        );
    }
    match state.phase {
        Phase::Loading | Phase::Empty => banner(
            model,
            Msg::RuleLintWaiting,
            Msg::RuleLintWaitingHelp,
            BadgeVariant::Neutral,
            Msg::NoProject,
        ),
        Phase::Error | Phase::Unavailable => banner(
            model,
            Msg::RuleLintUnavailable,
            Msg::RuleLintConnectionHelp,
            BadgeVariant::Warning,
            Msg::RuleLintFileUnavailable,
        ),
        Phase::Ready => caption(
            model,
            precept_ui_core::i18n::tr(state.locale, Msg::RuleLintIdle).into(),
        )
        .into_element(),
    }
}

/// The Preferences & contracts diagnostics region: explicit operator read,
/// fixed states, bounded plain-text findings and a copy-only template entry.
pub(crate) fn panel(model: Signal<Model>) -> impl IntoElement {
    let state = computed(move || {
        let m = model.get();
        ViewState {
            busy: m.rule_lint.busy,
            live: m.live,
            locale: m.locale,
            phase: m.phase.clone(),
            report: m.rule_lint.report.clone(),
            error: m.rule_lint.error,
        }
    });
    surface(model)
        .title_fn(word(model, Msg::RuleLintHeading))
        .child(
            text(move || -> SharedString { model.get().word(Msg::RuleLintIntro).into() })
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
                    ui::button(model, "")
                        .label_fn(word(model, Msg::CheckRules))
                        .icon(ui::icons::CODE)
                        .size(ButtonSize::Small)
                        .disabled(move || !model.get().rule_lint_ready())
                        .loading(move || model.get().rule_lint.busy)
                        .on_press(move || check(model)),
                )
                .child(
                    ui::button(model, "")
                        .label_fn(word(model, Msg::TemplatesOpen))
                        .icon(ui::icons::FILE)
                        .size(ButtonSize::Small)
                        .variant(ButtonVariant::Ghost)
                        .on_press(move || model.update(|m| m.rule_lint.open_templates())),
                ),
        )
        .child(dynamic(move || state_view(model, state.get())))
}
