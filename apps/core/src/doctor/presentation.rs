use crate::i18n::Msg;

/// How a check came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Working as intended.
    Ok,
    /// Works, but something the operator likely wants to look at.
    Attention,
    /// Broken: agents miss context because of it.
    Problem,
    /// Deliberately not set up (a switch that is off).
    Off,
    /// Not observed (yet): never shown as a pass.
    Unknown,
}

/// Where a check's fix lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fix {
    Connection,
    Profile,
    Workspaces,
    Agents,
    Review,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub title: Msg,
    pub level: Level,
    /// Localized, sanitized detail.
    pub detail: String,
    pub fix: Option<Fix>,
}

pub(super) fn check(
    title: Msg,
    level: Level,
    detail: impl Into<String>,
    fix: Option<Fix>,
) -> Check {
    Check {
        title,
        level,
        detail: detail.into(),
        fix,
    }
}
