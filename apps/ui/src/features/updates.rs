use crate::*;
use precept_ui_client::update::{DownloadedUpdate, Update, UpdateError};
use precept_ui_core::i18n::Msg;
#[cfg(all(feature = "ipc", not(test)))]
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokens::palette;
use ui::{ButtonVariant, button, scale, surface};

#[cfg_attr(test, allow(dead_code))]
#[derive(Clone, Debug)]
enum Phase {
    Idle,
    Checking,
    UpToDate,
    Available(Update),
    Downloading(Update, u64),
    Ready(DownloadedUpdate),
    Installing(DownloadedUpdate),
    Failed(UpdateError),
}

impl PartialEq for Phase {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Idle, Self::Idle)
            | (Self::Checking, Self::Checking)
            | (Self::UpToDate, Self::UpToDate) => true,
            (Self::Available(a), Self::Available(b)) => a.version() == b.version(),
            (Self::Downloading(a, received_a), Self::Downloading(b, received_b)) => {
                a.version() == b.version() && received_a == received_b
            }
            (Self::Ready(a), Self::Ready(b)) | (Self::Installing(a), Self::Installing(b)) => {
                a.version() == b.version()
            }
            (Self::Failed(a), Self::Failed(b)) => a == b,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PhaseTag {
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Ready,
    Installing,
    Failed,
}

impl Phase {
    fn tag(&self) -> PhaseTag {
        match self {
            Self::Idle => PhaseTag::Idle,
            Self::Checking => PhaseTag::Checking,
            Self::UpToDate => PhaseTag::UpToDate,
            Self::Available(_) => PhaseTag::Available,
            Self::Downloading(_, _) => PhaseTag::Downloading,
            Self::Ready(_) => PhaseTag::Ready,
            Self::Installing(_) => PhaseTag::Installing,
            Self::Failed(_) => PhaseTag::Failed,
        }
    }
}

fn busy(state: &Signal<Phase>) -> bool {
    matches!(
        state.get(),
        Phase::Checking | Phase::Downloading(_, _) | Phase::Installing(_)
    )
}

#[cfg(all(feature = "ipc", not(test)))]
fn check(state: Signal<Phase>) {
    state.set(Phase::Checking);
    Owner::detached().run(|| {
        spawn(async move {
            let result = blocking(precept_ui_client::update::check)
                .await
                .unwrap_or(Err(UpdateError::Network));
            state.set(match result {
                Ok(Some(update)) => Phase::Available(update),
                Ok(None) => Phase::UpToDate,
                Err(error) => Phase::Failed(error),
            });
        });
    });
}

#[cfg(any(not(feature = "ipc"), test))]
fn check(state: Signal<Phase>) {
    state.set(Phase::Failed(UpdateError::Unavailable));
}

#[cfg(all(feature = "ipc", not(test)))]
fn download(state: Signal<Phase>, update: Update) {
    state.set(Phase::Downloading(update.clone(), 0));
    let progress = Arc::new(AtomicU64::new(0));
    let worker_progress = progress.clone();
    let task = blocking(move || {
        precept_ui_client::update::download(&update, move |received| {
            worker_progress.store(received, Ordering::Relaxed);
        })
    });
    Owner::detached().run(|| {
        spawn(async move {
            while !task.is_finished() {
                let received = progress.load(Ordering::Relaxed);
                if let Phase::Downloading(update, _) = state.get_untracked() {
                    state.set(Phase::Downloading(update, received));
                }
                timer(std::time::Duration::from_millis(100)).await;
            }
            let result = task.await.unwrap_or(Err(UpdateError::Download));
            state.set(match result {
                Ok(update) => Phase::Ready(update),
                Err(error) => Phase::Failed(error),
            });
        });
    });
}

#[cfg(any(not(feature = "ipc"), test))]
fn download(state: Signal<Phase>, _: Update) {
    state.set(Phase::Failed(UpdateError::Unavailable));
}

#[cfg(all(feature = "ipc", not(test)))]
fn install(state: Signal<Phase>, update: DownloadedUpdate) {
    state.set(Phase::Installing(update.clone()));
    Owner::detached().run(|| {
        spawn(async move {
            let result = blocking(move || precept_ui_client::update::install(&update))
                .await
                .unwrap_or(Err(UpdateError::Install));
            match result {
                Ok(()) => nsh_core::element::desktop::restart(),
                Err(error) => {
                    state.set(Phase::Failed(error));
                }
            }
        });
    });
}

#[cfg(any(not(feature = "ipc"), test))]
fn install(state: Signal<Phase>, _: DownloadedUpdate) {
    state.set(Phase::Failed(UpdateError::Unavailable));
}

fn action(model: Signal<Model>, state: Signal<Phase>) -> impl IntoElement {
    button(model, "")
        .test_id("updates.action")
        .label_fn(move || {
            let key = match state.get() {
                Phase::Available(_) => Msg::DownloadUpdate,
                Phase::Ready(_) | Phase::Installing(_) => Msg::InstallRestart,
                _ => Msg::CheckForUpdates,
            };
            model.get().word(key).into()
        })
        .variant(ButtonVariant::Primary)
        .loading(move || busy(&state))
        .disabled(move || busy(&state))
        .on_press(move || match state.get_untracked() {
            Phase::Available(update) => download(state, update),
            Phase::Ready(update) => install(state, update),
            Phase::Idle
            | Phase::Checking
            | Phase::UpToDate
            | Phase::Downloading(_, _)
            | Phase::Installing(_)
            | Phase::Failed(_) => check(state),
        })
}

fn status(model: Signal<Model>, state: Signal<Phase>) -> impl IntoElement {
    text(move || {
        let m = model.get();
        match state.get() {
            Phase::Idle => String::new(),
            Phase::Checking => m.word(Msg::UpdateChecking).into(),
            Phase::UpToDate => m.word(Msg::UpdateUpToDate).into(),
            Phase::Available(update) => m
                .word(Msg::UpdateAvailable)
                .replace("{version}", update.version()),
            Phase::Downloading(update, received) => {
                let percent = update
                    .size()
                    .filter(|size| *size > 0)
                    .map(|size| received.min(size).saturating_mul(100) / size)
                    .unwrap_or_default();
                m.word(Msg::UpdateDownloading)
                    .replace("{percent}", &percent.to_string())
            }
            Phase::Ready(update) => m
                .word(Msg::UpdateReady)
                .replace("{version}", update.version()),
            Phase::Installing(_) => m.word(Msg::UpdateInstalling).into(),
            Phase::Failed(error) => m
                .word(match error {
                    UpdateError::Network => Msg::UpdateFailedNetwork,
                    UpdateError::Manifest => Msg::UpdateFailedManifest,
                    UpdateError::Download => Msg::UpdateFailedDownload,
                    UpdateError::Install => Msg::UpdateFailedInstall,
                    UpdateError::UnsafeInstallation => Msg::UpdateFailedUnsafe,
                    UpdateError::Unavailable => Msg::UpdateUnavailable,
                })
                .into(),
        }
    })
    .text_size(px(scale::text::CAPTION))
    .text_color(move || {
        if matches!(state.get(), Phase::Failed(_)) {
            palette(model).error
        } else {
            palette(model).muted
        }
    })
}

pub(crate) fn provide_state() {
    provide_context(signal(Phase::Idle));
}

pub(crate) fn updates(model: Signal<Model>) -> impl IntoElement {
    let state = use_context::<Signal<Phase>>().unwrap_or_else(|| signal(Phase::Idle));
    let tag = computed(move || state.get().tag());
    let reveal = ui::motion::reveal();
    let content = dynamic(move || {
        let _ = tag.get();
        view()
            .gap_3()
            .items_start()
            .bind_motion(&reveal)
            .child(status(model, state))
            .child(action(model, state))
            .into_element()
    });
    let installed = format!("v{}", env!("CARGO_PKG_VERSION"));
    surface(model)
        .title_fn(move || model.get().word(Msg::SoftwareUpdates).into())
        .child(
            text(move || model.get().word(Msg::UpdateHelp))
                .text_size(px(scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
        .child(
            text(move || {
                model
                    .get()
                    .word(Msg::InstalledVersion)
                    .replace("{version}", &installed)
            })
            .text_size(px(scale::text::CAPTION))
            .text_color(move || palette(model).muted),
        )
        .child(content)
}
