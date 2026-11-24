use super::actions::{available, open_delete, refresh, review, status_is, write_grant};
use super::formatting::{disguising, literal, origin_label, scope_label};
use super::queue::record_meta;
use super::records::status_word;
use super::*;

/// The Detail tab: the exact inspected record, digest prefix with explicit
/// expand, peer-demotion disclosure and the decision / destructive actions.
pub(super) fn detail_tab(model: Signal<Model>, delete_open: Signal<bool>) -> AnyElement {
    let m = model.get();
    let mut body = view().gap_2().flex_shrink_0();
    let Some(record) = m.operator.record() else {
        return body
            .child(
                ui::empty(model, m.word(Msg::ReviewNoSelection))
                    .description(m.word(Msg::ReviewQueueEmptyHelp)),
            )
            .into_element();
    };
    let Some(inspect) = m
        .operator
        .inspect
        .as_ref()
        .filter(|i| i.record.id == record.id)
    else {
        return body
            .child(word_action(
                model,
                Msg::ReviewInspect,
                move || available(model) && model.get().operator.record().is_some(),
                move || refresh(model, "profile.review.inspect"),
            ))
            .into_element();
    };
    let r = &inspect.record;
    // A suggestion that would replace a confirmed rule of its key: the
    // change, against that rule's text.
    // (Read when the inspection arrives; see `send`.)
    if r.status == "suggested"
        && let Some(peer) = inspect.peer_demotions.first()
        && let Some(old) = m.operator.update_of.as_ref().filter(|o| o.id == peer.id)
    {
        body = body.child(update_section(model, &m, old, r));
    }
    let digest = if m.operator.digest_expanded {
        r.proposal_digest.clone()
    } else {
        format!(
            "{}…",
            r.proposal_digest.chars().take(16).collect::<String>()
        )
    };
    let variant = match r.status.as_str() {
        "confirmed" => ui::BadgeVariant::Success,
        "suggested" => ui::BadgeVariant::Warning,
        _ => ui::BadgeVariant::Neutral,
    };
    let caption = |s: String| {
        text(s)
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted)
    };
    // The record: its key and state, its text as written, where it applies.
    body = body.child(
        view()
            .gap_2()
            .p_3()
            .rounded(px(ui::scale::radius::CONTAINER))
            .bg(move || palette(model).panel)
            .border_1()
            .border_color(move || palette(model).hairline)
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        text(sanitize(&r.key, 120))
                            .font_weight(FontWeight::SEMIBOLD)
                            .min_w_0(),
                    )
                    .child(ui::badge(model, status_word(&m, &r.status)).variant(variant))
                    .child(caption(format!(
                        "{} · {} {}",
                        scope_label(m.locale, &r.scope),
                        m.word(Msg::ReviewSource),
                        origin_label(m.locale, r.origin)
                    ))),
            )
            .child(if disguising(&r.value) || disguising(&r.key) {
                // Escaped, exactly as stored: nothing is rendered or hidden.
                let mut lines = view().gap_0p5();
                for line in literal(&r.key).into_iter().chain(literal(&r.value)) {
                    lines = lines.child(text(line).font_family("monospace"));
                }
                lines.into_element()
            } else {
                scroll()
                    .max_h(px(320.0))
                    .w_full()
                    .child(ui::markdown_view(model, r.value.clone()))
                    .into_element()
            })
            .child(record_meta(model, r)),
    );
    body = body.child(review_scope_section(model, r));
    body = body.child(review_related_section(model, inspect));
    body = body.child(review_evidence_section(model, inspect));
    // What only matters when checking exactly what is decided: one quiet
    // line, the notes behind its info button.
    body = body.child(
        view()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(caption(format!(
                "#{} · {} {} · {} {}",
                r.id,
                m.word(Msg::ReviewRevision),
                r.revision,
                m.word(Msg::ReviewDigest),
                digest
            )))
            .child(
                ui::button(model, "")
                    .label_fn(move || {
                        let m = model.get();
                        m.word(if m.operator.digest_expanded {
                            Msg::ReviewDigestCollapse
                        } else {
                            Msg::ReviewDigestExpand
                        })
                        .into()
                    })
                    .size(ui::ButtonSize::Small)
                    .variant(ui::ButtonVariant::Ghost)
                    .on_press(move || {
                        model.update(|m| m.operator.digest_expanded = !m.operator.digest_expanded)
                    }),
            )
            .child(ui::info(
                model,
                move || model.get().word(Msg::ReviewNotesTitle).into(),
                move || {
                    let m = model.get();
                    format!(
                        "{}\n\n{}",
                        m.word(Msg::ReviewProvenanceNote),
                        m.word(Msg::ReviewContractNote)
                    )
                    .into()
                },
            )),
    );
    // Peer-demotion disclosure read from inspect; the authoritative set is
    // computed at commit. A truncated preview still names the full total.
    if inspect.peer_demotion_total > 0 {
        let mut ids = inspect
            .peer_demotions
            .iter()
            .map(|p| p.id.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        if (inspect.peer_demotion_total as usize) > inspect.peer_demotions.len() {
            ids.push_str(", …");
        }
        let disclosure = tr(m.locale, Msg::ReviewPeerDemotion)
            .replace("{count}", &inspect.peer_demotion_total.to_string())
            .replace("{ids}", &ids);
        body = body.child(
            text(disclosure)
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).warning),
        );
    }
    if r.status == "confirmed" {
        body = body.child(
            text(m.word(Msg::ReviewDeprecateImpact))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        );
    }
    body = body.child(
        view()
            .flex_row()
            .flex_wrap()
            .gap_2()
            .child(word_action(
                model,
                Msg::OperatorConfirm,
                move || available(model) && write_grant(model) && status_is(model, "suggested"),
                move || review(model, "confirm"),
            ))
            .child(word_action(
                model,
                Msg::OperatorReject,
                move || available(model) && write_grant(model) && status_is(model, "suggested"),
                move || review(model, "reject"),
            ))
            .child(word_action(
                model,
                Msg::EditSuggestion,
                move || {
                    available(model)
                        && write_grant(model)
                        && model
                            .get()
                            .operator
                            .record()
                            .is_some_and(precept_ui_core::operator::editable)
                },
                move || {
                    model.update(|m| {
                        m.operator.start_revision();
                    })
                },
            ))
            .child(word_action(
                model,
                Msg::ReviewDeprecate,
                move || available(model) && write_grant(model) && status_is(model, "confirmed"),
                move || review(model, "deprecate"),
            ))
            .child(if m.operator.mode == ReviewMode::Project {
                word_action(
                    model,
                    Msg::ExportRule,
                    move || {
                        available(model)
                            && status_is(model, "confirmed")
                            && !model.get().operator.export_busy
                    },
                    move || open_export(model),
                )
                .into_element()
            } else {
                view().into_element()
            })
            .child(
                ui::button(model, "")
                    .label_fn(move || model.get().word(Msg::ReviewDelete).into())
                    .variant(ui::ButtonVariant::Danger)
                    .disabled(move || {
                        !(available(model)
                            && write_grant(model)
                            && model.get().operator.record().is_some())
                    })
                    .on_press(move || open_delete(model, delete_open)),
            ),
    );
    body.into_element()
}
fn review_section(model: Signal<Model>, title: String, lines: Vec<String>) -> AnyElement {
    view()
        .gap_1()
        .p_3()
        .rounded(px(ui::scale::radius::CONTAINER))
        .bg(move || palette(model).panel)
        .border_1()
        .border_color(move || palette(model).hairline)
        .child(text(title).font_weight(FontWeight::SEMIBOLD))
        .child(
            text(lines.join("\n"))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted)
                .selectable(true),
        )
        .into_element()
}

fn review_scope_section(
    model: Signal<Model>,
    record: &precept_ui_core::operator::PreferenceRecord,
) -> AnyElement {
    let m = model.get();
    let files = if record.rule.applies_to.paths.is_empty() {
        vec![m.word(Msg::ReviewEverywhere).to_owned()]
    } else {
        record
            .rule
            .applies_to
            .paths
            .iter()
            .map(|path| sanitize(path, 256))
            .collect()
    };
    review_section(model, m.word(Msg::ReviewAffectedFiles).into(), files)
}

fn review_related_section(
    model: Signal<Model>,
    inspect: &precept_ui_core::operator::ReviewInspect,
) -> AnyElement {
    let m = model.get();
    let title = m
        .word(Msg::ReviewRelated)
        .replace("{count}", &inspect.related_total.to_string());
    if inspect.related.is_empty() {
        return review_section(model, title, vec![m.word(Msg::ReviewRelatedEmpty).into()]);
    }
    let mut lines: Vec<String> = inspect
        .related
        .iter()
        .map(|related| {
            let relation = match related.relation {
                precept_ui_core::operator::ReviewRelation::Duplicate => Msg::ReviewDuplicate,
                precept_ui_core::operator::ReviewRelation::SameKey => Msg::ReviewSameKey,
                precept_ui_core::operator::ReviewRelation::SameValue => Msg::ReviewSameValue,
            };
            format!(
                "{} · #{} · {} · r{}",
                m.word(relation),
                related.id,
                sanitize(&related.key, 120),
                related.revision
            )
        })
        .collect();
    if inspect.related_total > inspect.related.len() as i64 {
        lines.push(m.word(Msg::ReviewRelatedTruncated).into());
    }
    review_section(model, title, lines)
}

fn review_evidence_section(
    model: Signal<Model>,
    inspect: &precept_ui_core::operator::ReviewInspect,
) -> AnyElement {
    let m = model.get();
    let title = m.word(Msg::ReviewEvidence).to_owned();
    let Some(evidence) = inspect.evidence else {
        return review_section(model, title, vec![m.word(Msg::ReviewEvidenceGlobal).into()]);
    };
    if evidence.gate_denials == 0 {
        return review_section(model, title, vec![m.word(Msg::ReviewEvidenceNone).into()]);
    }
    let mut lines = vec![
        m.word(Msg::ReviewEvidenceCount)
            .replace("{count}", &evidence.gate_denials.to_string()),
    ];
    if let Some(last) = evidence.last_gate_denial_ms {
        lines.push(
            m.word(Msg::ReviewLastGateDenial)
                .replace("{time}", &precept_ui_core::activity::date_time(last)),
        );
    }
    review_section(model, title, lines)
}

/// Reads the confirmed rule `id` an inspected suggestion would replace.
pub(super) fn fetch_update_of(
    model: Signal<Model>,
    scope: precept_ui_core::operator::Scope,
    id: i64,
) {
    crate::features::profile::call(
        model,
        "profile.preferences.get",
        serde_json::json!({"scope": scope, "id": id}),
        move |_, m, r| {
            if let Ok(v) = r
                && let Ok(record) = serde_json::from_value::<
                    precept_ui_core::operator::PreferenceRecord,
                >(v["record"].clone())
                && record.id == id
            {
                m.operator.update_of = Some(record);
            }
        },
    );
}

/// "Updates #N": the confirmed rule's text against the suggestion's, and
/// what else changes.
pub(super) fn update_section(
    model: Signal<Model>,
    m: &Model,
    old: &precept_ui_core::operator::PreferenceRecord,
    new: &precept_ui_core::operator::PreferenceRecord,
) -> AnyElement {
    let caption = |s: String| {
        text(s)
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted)
    };
    let mut section = view()
        .gap_2()
        .p_3()
        .rounded(px(ui::scale::radius::CONTAINER))
        .bg(move || palette(model).panel)
        .border_1()
        .border_color(move || palette(model).hairline)
        .child(
            text(
                m.word(Msg::UpdatesRule)
                    .replace("{id}", &old.id.to_string())
                    .replace("{key}", &sanitize(&old.key, 120)),
            )
            .font_weight(FontWeight::SEMIBOLD),
        )
        .child(caption(m.word(Msg::UpdatesRuleHelp).into()));
    section = section.child(ui::text_diff(model, old.value.clone(), new.value.clone()));

    if old.tags != new.tags {
        section = section.child(caption(
            m.word(Msg::UpdatesTags)
                .replace("{old}", &precept_ui_core::presentation::hashtags(&old.tags))
                .replace("{new}", &precept_ui_core::presentation::hashtags(&new.tags)),
        ));
    }
    if old.rule != new.rule {
        section = section.child(caption(m.word(Msg::UpdatesOptions).into()));
    }
    section.into_element()
}

/// Reads what writing the inspected confirmed convention as a rule file
/// would do, and opens the preview.
pub(super) fn open_export(model: Signal<Model>) {
    let m = model.get();
    let (Some(project), Some(id)) = (m.project().map(|p| p.name.clone()), m.operator.selected_id)
    else {
        return;
    };
    model.update(|m| m.operator.export_busy = true);
    crate::features::profile::call(
        model,
        "rules.export",
        serde_json::json!({"project": project, "id": id}),
        move |_, m, r| {
            m.operator.export_busy = false;
            match r.and_then(|v| {
                serde_json::from_value::<precept_ui_core::operator::RuleExport>(v)
                    .map_err(|_| "Unavailable")
            }) {
                Ok(preview) => m.operator.export = Some((id, preview)),
                Err(code) => ui::notify(ui::Tone::Error, code),
            }
        },
    );
}

/// Writes the previewed rule file, guarded by the digest the preview saw.
pub(super) fn write_export(model: Signal<Model>) {
    let m = model.get();
    let (Some(project), Some((id, preview))) = (
        m.project().map(|p| p.name.clone()),
        m.operator.export.clone(),
    ) else {
        return;
    };
    model.update(|m| m.operator.export_busy = true);
    crate::features::profile::call(
        model,
        "rules.export.apply",
        serde_json::json!({"project": project, "id": id, "enabled": true,
            "expected_sha256": preview.sha256}),
        move |model, m, r| {
            m.operator.export_busy = false;
            match r {
                Ok(v) => {
                    m.operator.export = None;
                    ui::notify(
                        ui::Tone::Success,
                        m.word(if v["action"] == "updated" {
                            Msg::ToastExportUpdated
                        } else {
                            Msg::ToastExported
                        }),
                    );
                    // The new file shows as pending on Preferences & contracts.
                    // Not inside this update: the loader updates the model itself.
                    spawn(async move { crate::load(model) });
                }
                Err("Conflict") => ui::notify(ui::Tone::Error, m.word(Msg::ExportChanged)),
                Err(code) => ui::notify(ui::Tone::Error, code),
            }
        },
    );
}

/// The rule file preview: path, what writing does, and the exact text.
/// Writing is offered only for an absent path and a valid file.
pub(crate) fn export_dialog(model: Signal<Model>) -> impl IntoElement {
    let preview = move || model.get().operator.export.clone().map(|(_, p)| p);
    ui::dialog(model, "")
        .size(ui::DialogSize::Large)
        .title_fn(move || model.get().word(Msg::ExportTitle).into())
        .description_fn(move || {
            preview()
                .map(|p| sanitize(&p.path, 160))
                .unwrap_or_default()
                .into()
        })
        .content_fn(move || {
            let m = model.get();
            let Some(p) = preview() else {
                return view().into_element();
            };
            let state = match (p.valid, p.state.as_str()) {
                (false, _) => Some((Msg::ExportInvalid, palette(model).danger)),
                (_, "same") => Some((Msg::ExportSame, palette(model).muted)),
                (_, "different") => Some((Msg::ExportDifferent, palette(model).muted)),
                (_, "invalid") => Some((Msg::ExportInvalidFile, palette(model).danger)),
                _ => None,
            };
            let mut body = view().gap_3().w_full().child(
                text(m.word(Msg::ExportHelp))
                    .text_size(px(ui::scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
            );
            if let Some((msg, color)) = state {
                body = body.child(
                    text(m.word(msg))
                        .text_size(px(ui::scale::text::CAPTION))
                        .text_color(move || color),
                );
            }
            // An update shows the change; a new file, its text.
            if p.state == "different"
                && let Some(current) = p.current.clone()
            {
                return body
                    .child(ui::text_diff(model, current, p.text))
                    .into_element();
            }
            body.child(
                scroll()
                    .max_h(px(360.0))
                    .w_full()
                    .rounded(px(ui::scale::radius::CONTROL))
                    .bg(move || palette(model).inset)
                    .child(
                        text(p.text)
                            .p_3()
                            .selectable(true)
                            .font_family("monospace")
                            .text_size(px(ui::scale::text::CAPTION)),
                    ),
            )
            .into_element()
        })
        .cancel_fn(move || model.get().word(Msg::CloseDialog).into())
        .confirm_fn(move || {
            let m = model.get();
            m.word(if preview().is_some_and(|p| p.state == "different") {
                Msg::ExportUpdate
            } else {
                Msg::ExportWrite
            })
            .into()
        })
        .confirm_disabled(move || {
            model.get().operator.export_busy
                || preview()
                    .is_none_or(|p| !p.valid || !matches!(p.state.as_str(), "absent" | "different"))
        })
        .open(move || preview().is_some())
        .on_cancel(move || model.update(|m| m.operator.export = None))
        .on_confirm(move || write_export(model))
}
