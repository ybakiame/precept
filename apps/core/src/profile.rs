//! The Profile setup draft: the operator's explicit create or select of the one
//! Profile this daemon serves (application-wide, not per project).
//!
//! The path starts as the daemon's suggestion (inside its private state
//! directory) until the operator edits it; nothing is created or selected
//! without the operator's click.

/// The last Profile state read for the Set-up pages (`None`: not read yet).
/// Debug never prints the Profile's identifiers.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Observed(pub Option<crate::operator::ProfileInfo>);
impl std::fmt::Debug for Observed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Observed(redacted)")
    }
}
impl std::ops::Deref for Observed {
    type Target = Option<crate::operator::ProfileInfo>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for Observed {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The setup card's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileDraft {
    pub path: String,
    /// The operator typed a path: a later suggestion no longer replaces it.
    pub path_edited: bool,
    /// Create/select also selects it at every daemon start (on by default).
    pub remember: bool,
    pub busy: bool,
    /// The last refusal (a fixed code).
    pub error: Option<String>,
}
impl Default for ProfileDraft {
    fn default() -> Self {
        Self {
            path: String::new(),
            path_edited: false,
            remember: true,
            busy: false,
            error: None,
        }
    }
}
impl ProfileDraft {
    /// Takes the daemon's suggestion unless the operator already typed a path.
    pub fn suggest(&mut self, suggested: Option<&str>) {
        if !self.path_edited
            && let Some(path) = suggested
        {
            self.path = path.to_owned();
        }
    }
    pub fn set_path(&mut self, path: String) {
        self.path = path;
        self.path_edited = true;
        self.error = None;
    }
    /// The request parameters for create/select, if the path is usable.
    pub fn params(&self) -> Option<serde_json::Value> {
        let path = self.path.trim();
        (!path.is_empty() && !self.busy)
            .then(|| serde_json::json!({"path": path, "remember": self.remember}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_suggestion_fills_the_path_until_the_operator_types() {
        let mut d = ProfileDraft::default();
        assert!(d.remember, "select at start is on by default");
        assert!(d.params().is_none());
        d.suggest(Some("/home/u/.local/state/acp/profile.sqlite3"));
        assert_eq!(
            d.params().unwrap(),
            serde_json::json!({"path":"/home/u/.local/state/acp/profile.sqlite3","remember":true})
        );
        d.set_path("/home/u/other.sqlite3".into());
        d.suggest(Some("/home/u/.local/state/acp/profile.sqlite3"));
        assert_eq!(d.path, "/home/u/other.sqlite3");
        d.remember = false;
        d.busy = true;
        assert!(
            d.params().is_none(),
            "no second request while one is running"
        );
    }
}
