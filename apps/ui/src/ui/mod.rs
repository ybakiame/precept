//! The app's `ui` layer: styled, reusable controls built on the semantic
//! `nsh/base` components. Every color comes from `palette(model)`, every
//! geometry from `scale`, every motion from `motion`. Features compose these
//! controls; they do not restyle base components or write colors, radii or
//! fonts. A look a feature needs becomes a new variant or size here first.
//!
//! `gallery` is a debug-only surface (mounted with `ACP_UI_GALLERY`, never in
//! normal navigation) showing every control in its variants and states, in
//! light and dark.
mod prelude;

pub mod icon;
pub(crate) mod icons;
pub mod motion;
pub mod scale;
mod status;
pub(crate) mod tokens;

mod alert_dialog;
mod badge;
mod button;
mod choice;
mod empty;
mod field;
mod info;
mod input;
mod markdown;
mod menu;
mod nav;
mod properties;
mod radio;
mod sheet;
mod surface;
mod tabs;
pub mod toast;

pub mod gallery;

pub use alert_dialog::{DialogSize, DialogTone, alert_dialog, dialog};
pub use badge::{BadgeVariant, badge};
pub use button::{ButtonSize, ButtonVariant, button, copy_button, icon_button};
pub use choice::choice_card;
pub use empty::empty;
pub use field::field;
pub use info::info;
pub use input::input;
pub use markdown::markdown_input;
pub use menu::menu;
pub use nav::{Slide, nav_item, nav_item_counted, step_item};
pub use properties::properties;
pub use radio::radio_group;
pub use sheet::sheet;
pub use status::{Status, status_dot};
pub use surface::{card, surface};
pub use tabs::tabs;
pub use toast::{Tone, notify, toaster};
