use super::*;

pub(super) fn scope_label(locale: Locale, scope: &Scope) -> String {
    match scope {
        Scope::Global => tr(locale, Msg::ReviewScopeGlobal).into(),
        Scope::Project { name } => tr(locale, Msg::ReviewScopeProject).replace("{name}", name),
    }
}
/// Who a record comes from, in the operator's words.
pub(super) fn origin_label(locale: Locale, origin: Origin) -> &'static str {
    tr(
        locale,
        match origin {
            Origin::Operator => Msg::OriginOperatorShort,
            Origin::Agent => Msg::OriginAgentShort,
        },
    )
}
/// A record's origin as a small badge: who, with its glyph.
pub(super) fn decision_word(locale: Locale, action: DecisionAction) -> &'static str {
    match action {
        DecisionAction::Confirm => tr(locale, Msg::DecisionConfirm),
        DecisionAction::Reject => tr(locale, Msg::DecisionReject),
        DecisionAction::Deprecate => tr(locale, Msg::DecisionDeprecate),
        DecisionAction::Demote => tr(locale, Msg::DecisionDemote),
        DecisionAction::Delete => tr(locale, Msg::DecisionDelete),
        DecisionAction::Enable => tr(locale, Msg::DecisionEnable),
        DecisionAction::Disable => tr(locale, Msg::DecisionDisable),
        DecisionAction::Replace => tr(locale, Msg::DecisionReplace),
    }
}
pub(super) fn channel_word(locale: Locale, channel: Channel) -> &'static str {
    match channel {
        Channel::Cli => tr(locale, Msg::ChannelCli),
        Channel::Desktop => tr(locale, Msg::ChannelDesktop),
    }
}
/// Binds `shortcut` to focus `reference` in `pane` of the open workspace, and
/// lets the field go when the workspace closes.
pub(super) fn focus_shortcut(
    model: Signal<Model>,
    shortcut: &'static str,
    pane: Pane,
    reference: ElementRef,
) {
    let target = reference;
    crate::shortcuts::bind(
        model,
        shortcut,
        KeyOptions {
            when: nsh_core::keymap::When::Always,
            ..Default::default()
        },
        move || {
            if model.get().operator.open {
                model.update(|m| {
                    m.operator.pane = pane;
                    m.operator.review = None;
                    m.operator.delete_arm = None;
                });
                // Native Windows cannot focus a field before its pane is shown.
                let target = target;
                nsh_core::element::request_animation_frame(move |_| {
                    if model.with(|m| {
                        m.operator.open
                            && m.operator.pane == pane
                            && !m.operator.busy
                            && !m.operator.outcome_unknown
                    }) {
                        target.focus();
                    }
                });
            }
        },
    );
    effect(move || {
        if !model.get().operator.open {
            reference.blur();
        }
    });
}
pub(super) fn field(
    model: Signal<Model>,
    key: Msg,
    get: fn(&precept_ui_core::operator::Operator) -> &str,
    set: fn(&mut precept_ui_core::operator::Operator, String),
    max: usize,
) -> impl IntoElement {
    let reference = element_ref();
    let shortcut = match key {
        Msg::OperatorKey => "review.field.key",
        Msg::OperatorValue => "review.field.value",
        _ => "review.field.task",
    };
    let pane = match key {
        Msg::OperatorTask => Pane::Handoff,
        _ => Pane::Preferences,
    };
    focus_shortcut(model, shortcut, pane, reference);
    ui::field(
        model,
        move || model.get().word(key).into(),
        ui::input(model, "")
            .label_fn(move || model.get().word(key).into())
            .element_ref(&reference)
            .value(move || get(&model.get().operator).to_owned())
            .on_focus(move || {
                if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
                    eprintln!("PRECEPT_UI_FOCUS shortcut={shortcut}");
                }
            })
            .disabled(move || model.get().operator.busy || model.get().operator.outcome_unknown)
            .on_change(move |v: &str| {
                model.update(|m| {
                    m.operator.review = None;
                    m.operator.delete_arm = None;
                    m.operator.serial += 1;
                    set(&mut m.operator, v.chars().take(max + 1).collect());
                })
            }),
    )
}
/// The displayed preview never reveals the full digest without deliberate
/// expansion. The immutable mutation/request bytes remain untouched.
pub(super) fn review_literal(review: &Review, expanded: bool, value: &str) -> Vec<String> {
    if !expanded && let Some(record) = &review.record {
        let prefix = record.proposal_digest.chars().take(16).collect::<String>();
        return literal(&value.replace(&record.proposal_digest, &format!("{prefix}…")));
    }
    literal(value)
}
/// Preserve all record characters for review; make control/bidi content visible.
/// Whether `s` holds characters that could disguise what it says (control
/// characters other than line breaks and tabs, bidirectional overrides): it
/// is then shown escaped, never rendered.
pub(super) fn disguising(s: &str) -> bool {
    s.chars().any(|c| {
        (c.is_control() && c != '\n' && c != '\t')
            || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
    })
}
pub(super) fn literal(s: &str) -> Vec<String> {
    let escaped: String = s
        .chars()
        .flat_map(|c| {
            if c.is_control() || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}') {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect();
    escaped
        .chars()
        .collect::<Vec<_>>()
        .chunks(120)
        .map(|c| c.iter().collect())
        .collect()
}
