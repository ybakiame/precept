#[cfg(feature = "ipc")]
use crate::INPUTS;
#[cfg(all(feature = "ipc", not(test)))]
use crate::persist_settings;
use crate::{console, features, load, ui};
use nsh_core::prelude::*;
use precept_ui_core::Model;

#[component]
pub fn App() -> impl IntoElement {
    // Debug-only design-system gallery; never reached from normal navigation.
    if std::env::var_os("PRECEPT_UI_GALLERY").is_some() {
        return ui::gallery::gallery().into_element();
    }
    #[cfg(feature = "ipc")]
    let inputs = INPUTS.get().cloned().unwrap_or_default();
    #[cfg(all(feature = "ipc", not(test)))]
    let mut saved_onboarding_seen = false;
    let mut initial = Model::loading();
    initial.operator.channel = precept_ui_core::operator::Channel::Desktop;
    #[cfg(feature = "ipc")]
    {
        // Saved settings first; explicit launch arguments win over them.
        #[cfg(not(test))]
        if persist_settings()
            && let Some(saved) = precept_ui_client::settings::load_settings()
        {
            saved_onboarding_seen = saved.onboarding_seen;
            saved.apply(
                &mut initial,
                precept_ui_core::settings::Explicit {
                    theme: inputs.theme.is_some(),
                    mode: inputs.appearance.is_some(),
                    language: inputs.locale.is_some(),
                },
            );
        }
        if let Some(locale) = inputs
            .locale
            .as_deref()
            .and_then(precept_ui_core::i18n::Locale::parse)
        {
            initial.locale = locale;
        }
        if let Some(theme) = inputs
            .theme
            .as_deref()
            .and_then(precept_ui_core::appearance::ThemeId::from_id)
        {
            initial.theme = theme;
        }
        if let Some(mode) = inputs
            .appearance
            .as_deref()
            .and_then(precept_ui_core::appearance::ColorMode::from_id)
        {
            initial.color_mode = mode;
        }
        initial.operator.open = inputs.operator;
        initial.settings = inputs.settings;
        if inputs.connection_settings {
            initial.setup_open = true;
            initial.setup_tab = precept_ui_core::SetupTab::Connection;
        }
        initial.section = match inputs.section.as_str() {
            "projects" => {
                initial.setup_open = true;
                initial.setup_tab = precept_ui_core::SetupTab::Workspaces;
                precept_ui_core::Section::Dashboard
            }
            "rules" | "preferences" => precept_ui_core::Section::Rules,
            "handoff" => precept_ui_core::Section::Handoff,
            "diagnostics" => precept_ui_core::Section::Diagnostics,
            _ => precept_ui_core::Section::Dashboard,
        };
        initial.connection = inputs.connection();
        initial.connection_draft = initial.connection.clone();
        initial.live = inputs.is_live();
        // Explicit launch arguments win; otherwise the saved connection, and on a
        // first desktop launch Runtime checks (Connection tab). Never in unit
        // tests, which must not read or write the developer's settings.
        #[cfg(not(test))]
        if !inputs.is_live() && inputs.synthetic_rows.is_none() {
            match precept_ui_client::settings::load() {
                Some(saved) => {
                    initial.live = saved.target != precept_ui_core::connection::Target::Synthetic;
                    initial.connection = saved.clone();
                    initial.connection_draft = saved;
                }
                None => {
                    let guided_launch = !inputs.connection_settings
                        && !inputs.settings
                        && !inputs.operator
                        && inputs.section == "dashboard";
                    if guided_launch && !saved_onboarding_seen {
                        initial.onboarding_open = true;
                        initial.onboarding_step = precept_ui_core::OnboardingStep::Connection;
                    } else if !guided_launch {
                        initial.setup_open = true;
                        initial.setup_tab = precept_ui_core::SetupTab::Connection;
                    }
                    // The primary workflow: a daemon inside WSL2.
                    if cfg!(target_os = "windows") {
                        initial.connection_draft.target = precept_ui_core::connection::Target::Wsl;
                    }
                }
            }
        }
        initial.task_id = inputs.task_id.unwrap_or_default();
        initial.selected_project = inputs.project;
    }
    initial.dark = initial.color_mode != precept_ui_core::appearance::ColorMode::Light;
    set_color_scheme(match initial.color_mode {
        precept_ui_core::appearance::ColorMode::System => Appearance::System,
        precept_ui_core::appearance::ColorMode::Light => Appearance::Light,
        _ => Appearance::Dark,
    });
    let model = signal(initial);
    // Save the settings (appearance, language, shortcuts) when they change,
    // never the values the app started with.
    #[cfg(all(feature = "ipc", not(test)))]
    if persist_settings() {
        let chosen = computed(move || precept_ui_core::settings::Settings::of(&model.get()));
        let first = std::cell::Cell::new(true);
        effect(move || {
            let settings = chosen.get();
            if first.replace(false) {
                return;
            }
            // A refused or failed save leaves this session's choice as it is.
            let _ = precept_ui_client::settings::save_settings(&settings);
        });
    }
    if model.get_untracked().setup_open
        && model.get_untracked().setup_tab == precept_ui_core::SetupTab::Connection
    {
        features::connection::opened(model);
    }
    load(model);
    #[cfg(feature = "ipc")]
    wsl_watch(model);
    console(model).into_element()
}

/// Live WSL link status every 2 s for a WSL target: a lost bridge (distribution
/// stopped, helper exited) or an unreachable daemon is shown without waiting
/// for a user action. It never starts a bridge or a process, and only updates
/// the display-only `wsl_link` field (page phase, data and guards are untouched).
#[cfg(feature = "ipc")]
fn wsl_watch(model: Signal<Model>) {
    use precept_ui_core::connection::Target;
    spawn(async move {
        loop {
            timer(std::time::Duration::from_secs(2)).await;
            let connection = model.get_untracked().connection.clone();
            if connection.target != Target::Wsl {
                if model.get_untracked().wsl_link.is_some() {
                    model.update(|m| m.wsl_link = None);
                }
                continue;
            }
            let probe = connection.clone();
            let status = blocking(move || {
                precept_ui_client::native::Inputs::from_connection(&probe)
                    .ok()
                    .and_then(|inputs| inputs.wsl_link_status())
            })
            .await
            .unwrap_or(None);
            if model.get_untracked().connection == connection
                && model.get_untracked().wsl_link != status
            {
                model.update(|m| m.wsl_link = status);
            }
        }
    });
}
