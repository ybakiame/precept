//! Shared imports for the app's styled controls. A control does
//! `use super::prelude::*;` and builds on the semantic `nsh/base` components,
//! reading every color from `palette(model)` and every geometry from `scale`.
pub(crate) use super::{icon, motion, scale};
pub(crate) use crate::Model;
pub(crate) use crate::ui::tokens::{compact, mix, palette};
pub(crate) use nsh_base::{
    button as base_button, dialog as base_dialog, input as base_input, tab as base_tab,
    tabs as base_tabs,
};
pub(crate) use nsh_core::prelude::*;
