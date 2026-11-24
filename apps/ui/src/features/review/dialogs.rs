use super::actions::{send, submit};
use super::formatting::{field, focus_shortcut, literal, review_literal, scope_label};
use super::*;

/// The prepared operation, committed only by Apply: what it does, to which
/// exact record, the disclosures, and the request itself behind "Show request".
/// `destructive` builds the delete tier's (danger) variant.
pub(super) fn apply_dialog(model: Signal<Model>, destructive: bool) -> impl IntoElement {
    let raw = signal(false);
    let is_delete = move |m: &Model| {
        matches!(
            m.operator
                .review
                .as_ref()
                .and_then(|r| r.mutation.as_ref())
                .map(|m| &m.command),
            Some(Command::DeletePreference { .. })
        )
    };
    let dialog = if destructive {
        ui::alert_dialog(model, "").icon(icons::ALERT, ui::DialogTone::Danger)
    } else {
        ui::dialog(model, "").icon(icons::CHECK, ui::DialogTone::Accent)
    };
    dialog
        .size(ui::DialogSize::Medium)
        .title_fn(move || model.get().word(Msg::OperatorReview).into())
        .description_fn(move || model.get().word(Msg::OperatorPending).into())
        .content_fn(move || {
            view()
                .gap_2()
                .child(dynamic(move || {
                    let m = model.get();
                    let Some(review) = &m.operator.review else {
                        return view().into_element();
                    };
                    let mut lines = vec![review.method.clone()];
                    if let Some(r) = &review.record {
                        lines.push(format!(
                            "#{} · {} · {} · r{} · {}…",
                            r.id,
                            sanitize(&r.key, 80),
                            scope_label(m.locale, &r.scope),
                            r.revision,
                            r.proposal_digest.chars().take(16).collect::<String>()
                        ));
                    }
                    if let Some(mutation) = &review.mutation {
                        match &mutation.command {
                            Command::ConfirmPreference { .. } => {
                                lines.push(m.word(Msg::OperatorPeerWarning).into())
                            }
                            Command::DeprecatePreference { .. } => {
                                lines.push(m.word(Msg::ReviewDeprecateImpact).into())
                            }
                            Command::DeletePreference { .. } => {
                                lines.push(m.word(Msg::ReviewDeleteBody).into())
                            }
                            Command::SuggestPreference { key, value, .. } => {
                                lines.push(sanitize(key, 120));
                                lines.extend(literal(value).into_iter().take(6));
                            }
                            Command::SuggestBatch { items } => {
                                lines.push(
                                    m.word(Msg::ImportLine)
                                        .replace("{n}", &items.len().to_string()),
                                );
                                for i in items {
                                    lines.push(format!(
                                        "{} · {}",
                                        sanitize(&i.key, 120),
                                        scope_label(m.locale, &i.scope)
                                    ));
                                }
                            }
                            Command::DecideBatch {
                                decision, items, ..
                            } => {
                                let confirm =
                                    *decision == precept_ui_core::operator::BatchDecision::Confirm;
                                lines.push(
                                    m.word(if confirm {
                                        Msg::BatchConfirmLine
                                    } else {
                                        Msg::BatchRejectLine
                                    })
                                    .replace("{n}", &items.len().to_string()),
                                );
                                let (listed, _) = m.operator.batch_records();
                                for i in items {
                                    let key = listed
                                        .iter()
                                        .find(|r| r.id == i.id)
                                        .map(|r| sanitize(&r.key, 80))
                                        .unwrap_or_default();
                                    lines.push(format!(
                                        "#{} · {} · {}…",
                                        i.id,
                                        key,
                                        i.proposal_digest.chars().take(12).collect::<String>()
                                    ));
                                }
                                if confirm {
                                    lines.push(m.word(Msg::OperatorPeerWarning).into());
                                }
                            }
                            Command::RevisePreference {
                                id,
                                key,
                                value,
                                tags,
                                ..
                            } => {
                                lines.push(
                                    m.word(if m.operator.revising_confirmed {
                                        Msg::EditRuleHelp
                                    } else {
                                        Msg::EditSuggestionHelp
                                    })
                                    .replace("{id}", &id.to_string()),
                                );
                                lines.push(format!(
                                    "{} · {}",
                                    sanitize(key, 120),
                                    precept_ui_core::presentation::hashtags(tags)
                                ));
                                lines.extend(literal(value).into_iter().take(6));
                            }
                            _ => {}
                        }
                        if let Some(note) = mutation.command.note() {
                            lines.push(format!(
                                "{}: {}",
                                m.word(Msg::DecisionNote),
                                sanitize(note, 500)
                            ));
                        }
                    }
                    if raw.get() {
                        if let Some(record) = &review.record {
                            lines.extend(review_literal(
                                review,
                                m.operator.digest_expanded,
                                &serde_json::to_string(record).unwrap_or_default(),
                            ));
                        }
                        if let Some(mutation) = &review.mutation {
                            lines.extend(review_literal(
                                review,
                                m.operator.digest_expanded,
                                &serde_json::to_string(mutation).unwrap_or_default(),
                            ));
                        } else {
                            lines.extend(literal(&review.params.to_string()));
                        }
                    }
                    card(model, m.word(Msg::OperatorReview), lines).into_element()
                }))
                .child(
                    ui::button(model, "")
                        .label_fn(move || {
                            model
                                .get()
                                .word(if raw.get() {
                                    Msg::HideRequest
                                } else {
                                    Msg::ShowRequest
                                })
                                .into()
                        })
                        .size(ui::ButtonSize::Small)
                        .variant(ui::ButtonVariant::Ghost)
                        .on_press(move || raw.update(|r| *r = !*r)),
                )
                .into_element()
        })
        .cancel_fn(move || model.get().word(Msg::OperatorCancel).into())
        .confirm_fn(move || model.get().word(Msg::OperatorCommit).into())
        .confirm_disabled(move || !available(model) || model.get().operator.busy)
        .open(move || {
            let m = model.get();
            m.operator.open && m.operator.review.is_some() && is_delete(&m) == destructive
        })
        .on_cancel(move || {
            raw.set(false);
            model.update(|m| {
                m.operator.review = None;
                m.operator.delete_arm = None;
            })
        })
        .on_confirm(move || {
            raw.set(false);
            submit(model)
        })
}

/// Add preference: key, a Markdown value, its tags and where it comes from.
/// Saving prepares a suggestion (never a confirmation); Apply commits it, then
/// the new exact record opens to confirm.
/// One comma-separated `applies_to` field of the draft.
pub(super) fn applies_field(
    model: Signal<Model>,
    label: Msg,
    get: fn(&precept_ui_core::operator::Operator) -> &String,
    set: fn(&mut precept_ui_core::operator::Operator, String),
) -> AnyElement {
    ui::field(
        model,
        move || model.get().word(label).into(),
        ui::input(model, "")
            .label_fn(move || model.get().word(label).into())
            .value(move || get(&model.get().operator).clone())
            .disabled(move || model.get().operator.busy || model.get().operator.outcome_unknown)
            .on_change(move |v: &str| {
                let v: String = v.chars().take(512).collect();
                model.update(|m| {
                    set(&mut m.operator, v);
                    m.operator.review = None;
                    m.operator.serial += 1;
                })
            }),
    )
    .into_element()
}
/// The draft's rule options: where it applies, its priority and its declared
/// enforcement.
pub(super) fn rule_options(model: Signal<Model>) -> AnyElement {
    let pick = |e: precept_ui_core::operator::Enforcement| {
        move || model.update(|m| m.operator.enforcement = e)
    };
    let is =
        |e: precept_ui_core::operator::Enforcement| move || model.get().operator.enforcement == e;
    view()
        .gap_3()
        .child(text(move || model.get().word(Msg::AppliesTitle)).font_weight(FontWeight::MEDIUM))
        .child(
            text(move || model.get().word(Msg::AppliesHelp))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_3()
                .child(view().flex_1().min_w(px(200.0)).child(applies_field(
                    model,
                    Msg::AppliesLanguages,
                    |o| &o.applies_languages,
                    |o, v| o.applies_languages = v,
                )))
                .child(view().flex_1().min_w(px(200.0)).child(applies_field(
                    model,
                    Msg::AppliesPaths,
                    |o| &o.applies_paths,
                    |o, v| o.applies_paths = v,
                )))
                .child(view().flex_1().min_w(px(200.0)).child(applies_field(
                    model,
                    Msg::AppliesAgents,
                    |o| &o.applies_agents,
                    |o, v| o.applies_agents = v,
                )))
                .child(view().flex_1().min_w(px(200.0)).child(applies_field(
                    model,
                    Msg::AppliesTaskTags,
                    |o| &o.applies_tags,
                    |o, v| o.applies_tags = v,
                ))),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_3()
                .child(view().w(px(200.0)).child(applies_field(
                    model,
                    Msg::PriorityLabel,
                    |o| &o.priority,
                    |o, v| o.priority = v,
                )))
                .child(
                    view().flex_1().child(ui::field(
                        model,
                        move || model.get().word(Msg::EnforcementLabel).into(),
                        ui::radio_group(model, move || {
                            model.get().word(Msg::EnforcementLabel).into()
                        })
                        .option(
                            model.get().word(Msg::EnforcementAdvisory),
                            is(precept_ui_core::operator::Enforcement::Advisory),
                            pick(precept_ui_core::operator::Enforcement::Advisory),
                        )
                        .option(
                            model.get().word(Msg::EnforcementVerify),
                            is(precept_ui_core::operator::Enforcement::Verify),
                            pick(precept_ui_core::operator::Enforcement::Verify),
                        )
                        .option(
                            model.get().word(Msg::EnforcementHard),
                            is(precept_ui_core::operator::Enforcement::Hard),
                            pick(precept_ui_core::operator::Enforcement::Hard),
                        ),
                    )),
                ),
        )
        .child(
            text(move || model.get().word(Msg::EnforcementHelp))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
        .into_element()
}
pub(super) fn add_dialog(model: Signal<Model>) -> impl IntoElement {
    let ready = move || {
        let o = model.get().operator;
        !o.key.trim().is_empty()
            && !o.value.trim().is_empty()
            && precept_ui_core::operator::canonical_tags(&o.tags).is_some()
    };
    let tags_ref = element_ref();
    focus_shortcut(model, "review.field.tags", Pane::Preferences, tags_ref);
    ui::dialog(model, "")
        .size(ui::DialogSize::Large)
        .icon(icons::PLUS, ui::DialogTone::Accent)
        .title_fn(move || {
            let m = model.get();
            if let Some(id) = m.operator.revising {
                return m
                    .word(if m.operator.revising_confirmed {
                        Msg::EditRuleTitle
                    } else {
                        Msg::EditSuggestionTitle
                    })
                    .replace("{id}", &id.to_string())
                    .into();
            }
            m.word(if m.operator.global {
                Msg::AddPersonalPreference
            } else {
                Msg::AddProjectConvention
            })
            .into()
        })
        .description_fn(move || {
            let m = model.get();
            match m.operator.revising {
                Some(id) => m
                    .word(if m.operator.revising_confirmed {
                        Msg::EditRuleHelp
                    } else {
                        Msg::EditSuggestionHelp
                    })
                    .replace("{id}", &id.to_string())
                    .into(),
                None => m.word(Msg::AddPreferenceHelp).into(),
            }
        })
        .content_fn(move || {
            view()
                .w_full()
                .gap_4()
                .child(field(
                    model,
                    Msg::OperatorKey,
                    |o| &o.key,
                    |o, v| o.key = v,
                    120,
                ))
                .child(ui::field(
                    model,
                    move || model.get().word(Msg::OperatorValue).into(),
                    ui::markdown_input(
                        model,
                        "preference-value",
                        move || model.get().word(Msg::OperatorValue).into(),
                        move || model.get().operator.value.clone(),
                        move |v| model.update(|m| m.operator.value = v),
                    ),
                ))
                .child(
                    ui::tag_editor(
                        model,
                        move || model.get().operator.tags,
                        move || model.get().operator.tag_draft,
                    )
                    .element_ref(&tags_ref)
                    .suggestions(move || {
                        let m = model.get();
                        m.tag_suggestions(&m.operator.tags, &m.operator.tag_draft)
                    })
                    .on_draft(move |v| {
                        let v: String = v.chars().take(64).collect();
                        model.update(|m| m.operator.tag_draft = v)
                    })
                    .on_add(move |v| {
                        let mut taken = false;
                        model.update(|m| {
                            taken = m.operator.add_tag(v);
                            m.operator.review = None;
                            m.operator.serial += 1;
                        });
                        taken
                    })
                    .on_remove(move |v| {
                        model.update(|m| {
                            m.operator.remove_tag(v);
                            m.operator.review = None;
                            m.operator.serial += 1;
                        })
                    })
                    .disabled(move || {
                        model.get().operator.busy || model.get().operator.outcome_unknown
                    }),
                )
                .child(
                    view()
                        .style_when(
                            move || model.get().operator.revising.is_some(),
                            |s| s.hidden(),
                        )
                        .child(ui::field(
                            model,
                            move || model.get().word(Msg::OriginQuestion).into(),
                            ui::radio_group(model, move || {
                                model.get().word(Msg::OriginQuestion).into()
                            })
                            .option(
                                model.get().word(Msg::OriginOperator),
                                move || model.get().operator.origin == Origin::Operator,
                                move || model.update(|m| m.operator.origin = Origin::Operator),
                            )
                            .option(
                                model.get().word(Msg::OriginAgent),
                                move || model.get().operator.origin == Origin::Agent,
                                move || model.update(|m| m.operator.origin = Origin::Agent),
                            ),
                        )),
                )
                .child(rule_options(model))
                .into_element()
        })
        .cancel_fn(move || model.get().word(Msg::Cancel).into())
        .confirm_fn(move || {
            let m = model.get();
            m.word(if m.operator.revising_confirmed {
                Msg::SaveRuleEdit
            } else if m.operator.revising.is_some() {
                Msg::SaveEdit
            } else {
                Msg::SaveAsSuggestion
            })
            .into()
        })
        .confirm_disabled(move || !ready() || !available(model) || !write_grant(model))
        .open(move || {
            let m = model.get();
            (m.operator.adding || m.operator.revising.is_some()) && m.operator.open
        })
        .on_cancel(move || {
            model.update(|m| {
                m.operator.adding = false;
                m.operator.revising = None;
            });
        })
        .on_confirm(move || {
            model.update(|m| m.operator.adding = false);
            if model.get().operator.revising.is_some() {
                review(model, "revise");
            } else {
                review(model, "suggest");
            }
        })
}

/// A write whose reply was lost: the outcome is unknown until the identical
/// request is retried (or abandoned). Inline, not a toast: it needs a decision.
pub(super) fn unknown_banner(model: Signal<Model>) -> impl IntoElement {
    view()
        .gap_2()
        .style_when(
            move || !model.get().operator.outcome_unknown,
            |s| s.hidden(),
        )
        .child(
            text(move || model.get().word(Msg::OperatorUnknown))
                .text_color(move || palette(model).warning),
        )
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(word_action(
                    model,
                    Msg::OperatorRetry,
                    move || {
                        let m = model.get();
                        !m.operator.busy
                            && m.operator
                                .pending
                                .as_ref()
                                .is_some_and(|p| m.operator_current(p))
                    },
                    move || {
                        let mut request = None;
                        model.update(|m| request = m.operator_retry());
                        if let Some(r) = request {
                            send(model, r);
                        }
                    },
                ))
                .child(word_action(
                    model,
                    Msg::OperatorAbandon,
                    move || !model.get().operator.busy,
                    move || {
                        model.update(|m| {
                            m.operator.outcome_unknown = false;
                            m.operator.review = None;
                            m.operator.invalidate();
                            m.operator.error = Some("OutcomeUnknown".into());
                        })
                    },
                )),
        )
}
