//! Repository-rule diagnostics and copy-only templates. The Model owns scope,
//! request identity and results; this feature owns only native request dispatch
//! and presentation. Diagnostics are always plain text, never Markdown/HTML.
use crate::*;

mod panel;
mod templates;
#[cfg(test)]
mod tests;

pub(crate) use panel::panel;
pub(crate) use templates::templates_dialog;

/// Starts one Operator read through the existing local/WSL project-call path.
/// No review panel, write grant or repository write is involved.
pub(super) fn check(model: Signal<Model>) {
    let mut request = None;
    model.update(|m| request = m.request_rule_lint().ok());
    let Some(request) = request else { return };
    let params = serde_json::json!({"project": request.project});
    #[cfg(feature = "ipc")]
    {
        let inputs = precept_ui_client::native::Inputs::from_connection(&model.get().connection);
        // Detached: navigating away must not cancel a read whose stale reply the
        // Model already knows how to refuse.
        Owner::detached().run(|| {
            spawn(async move {
                let result =
                    blocking(move || inputs.and_then(|i| i.project_call("rules.lint", params)))
                        .await
                        .unwrap_or(Err("Unavailable"));
                model.update(|m| {
                    m.complete_rule_lint(&request, result);
                });
            });
        });
    }
    #[cfg(not(feature = "ipc"))]
    {
        let _ = params;
        model.update(|m| {
            m.complete_rule_lint(&request, Err("Unavailable"));
        });
    }
}
