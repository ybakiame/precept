use super::presentation::{Check, Level};
use crate::{Model, i18n::Msg};

/// The number of problems and of checks worth a look.
pub fn tally(checks: &[Check]) -> (usize, usize) {
    let n = |l: Level| checks.iter().filter(|c| c.level == l).count();
    (n(Level::Problem), n(Level::Attention))
}

pub fn level_word(level: Level) -> Msg {
    match level {
        Level::Ok => Msg::DoctorLevelOk,
        Level::Attention => Msg::DoctorLevelAttention,
        Level::Problem => Msg::DoctorLevelProblem,
        Level::Off => Msg::DoctorLevelOff,
        Level::Unknown => Msg::DoctorLevelUnknown,
    }
}

/// The checks as plain text to paste into an issue or a chat: the project
/// name and what each check saw; never paths, values, commands or tokens.
pub fn report(m: &Model, checks: &[Check]) -> String {
    let mut out = format!(
        "Precept doctor · {}\n",
        m.selected_project
            .as_deref()
            .map(|p| crate::presentation::sanitize(p, 64))
            .unwrap_or_else(|| "-".into())
    );
    for c in checks {
        out.push_str(&format!(
            "[{}] {}: {}\n",
            m.word(level_word(c.level)),
            m.word(c.title),
            crate::presentation::sanitize(&c.detail, 240)
        ));
    }
    out
}
