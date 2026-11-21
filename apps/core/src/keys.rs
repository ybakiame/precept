//! The app's keyboard shortcuts: one catalog of actions, each with a stable
//! id, a group, a label and its default keystrokes, and the operator's
//! overrides (kept in `settings.toml`). The shells bind what [`Keymap::chords`]
//! returns; the Shortcuts panel shows and edits the same catalog.
//!
//! A keystroke is NSH's spelling (`"mod+shift+y"`, `"alt+ArrowLeft"`): `mod`
//! is Command on Apple platforms and Control elsewhere. Validation of a
//! recorded keystroke belongs to the UI, which has NSH's parser.
use crate::i18n::Msg;
use std::collections::BTreeMap;

/// Where a shortcut works, as the panel groups them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    General,
    Navigation,
    Review,
}
impl Group {
    pub const ALL: [Self; 3] = [Self::General, Self::Navigation, Self::Review];
    pub fn label(self) -> Msg {
        match self {
            Self::General => Msg::KeysGeneral,
            Self::Navigation => Msg::KeysNavigation,
            Self::Review => Msg::KeysReview,
        }
    }
}

/// One action a keystroke can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    /// Stable id, the key of an override in `settings.toml`.
    pub id: &'static str,
    pub group: Group,
    pub label: Msg,
    /// Default keystrokes, the first the one shown; the others are second
    /// spellings some platforms need (they go when the operator rebinds).
    pub defaults: &'static [&'static str],
}

macro_rules! shortcuts {
    ($($id:literal, $group:ident, $label:ident, [$($key:literal),+];)*) => {
        /// Every shortcut, in the panel's order.
        pub const SHORTCUTS: &[Shortcut] = &[
            $(Shortcut { id: $id, group: Group::$group, label: Msg::$label, defaults: &[$($key),+] }),*
        ];
    };
}

shortcuts! {
    "shortcuts.open", General, KeyShortcutsOpen, ["mod+/", "F1"];
    "reconnect", General, KeyReconnect, ["mod+r"];
    "project.next", General, KeyProjectNext, ["mod+p"];
    "preferences.filter", General, KeyFilter, ["mod+f"];
    "handoff.task", General, KeyTask, ["mod+k"];
    "page.copy", General, KeyCopyPage, ["mod+shift+c"];
    "language.toggle", General, KeyLanguage, ["mod+l"];
    "appearance.toggle", General, KeyDarkLight, ["mod+d"];
    "contrast.toggle", General, KeyContrast, ["mod+shift+h"];
    "theme.graphite", General, KeyThemeGraphite, ["alt+d"];
    "theme.harbor", General, KeyThemeHarbor, ["alt+c"];
    "theme.ledger", General, KeyThemeLedger, ["alt+b"];
    "theme.instrument", General, KeyThemeInstrument, ["alt+a"];
    "go.dashboard", Navigation, KeyGoDashboard, ["alt+1"];
    "go.workspaces", Navigation, KeyGoWorkspaces, ["alt+2"];
    "go.preferences", Navigation, KeyGoRules, ["alt+3"];
    "go.handoff", Navigation, KeyGoHandoff, ["alt+4"];
    "go.diagnostics", Navigation, KeyGoDiagnostics, ["alt+5"];
    "go.review", Navigation, KeyGoReview, ["alt+o"];
    "go.settings", Navigation, KeyGoSettings, ["alt+0"];
    "page.next", Navigation, KeyNext, ["alt+ArrowRight", "alt+Right"];
    "page.previous", Navigation, KeyPrevious, ["alt+ArrowLeft", "alt+Left"];
    "review.suggest", Review, KeySuggest, ["alt+shift+s"];
    "review.confirm", Review, KeyConfirm, ["alt+shift+y"];
    "review.reject", Review, KeyReject, ["alt+shift+n"];
    "review.deprecate", Review, KeyDeprecate, ["alt+shift+w"];
    "review.delete", Review, KeyDelete, ["alt+shift+q"];
    "review.apply", Review, KeyApply, ["alt+shift+v"];
    "review.retry", Review, KeyRetry, ["alt+shift+z"];
    "review.next", Review, KeyNextRecord, ["alt+shift+i"];
    "review.profile", Review, KeyReadProfile, ["alt+shift+r"];
    "review.records", Review, KeyReadRecords, ["alt+shift+l"];
    "review.handoff", Review, KeyReadHandoff, ["alt+shift+h"];
    "review.field.key", Review, KeyFieldKey, ["alt+k"];
    "review.field.value", Review, KeyFieldValue, ["alt+w"];
    "review.field.tags", Review, KeyFieldTags, ["alt+f"];
    "review.field.task", Review, KeyFieldTask, ["alt+t"];
}

/// The catalog entry for `id`.
pub fn shortcut(id: &str) -> Option<&'static Shortcut> {
    SHORTCUTS.iter().find(|s| s.id == id)
}

/// The longest keystroke kept in an override.
pub const MAX_KEYSTROKE: usize = 40;

/// Whether `text` could be a keystroke worth keeping: short, printable ASCII,
/// no spaces. The UI checks it with NSH's parser before it is offered.
pub fn plausible(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_KEYSTROKE && text.bytes().all(|b| b.is_ascii_graphic())
}

/// The operator's overrides: action id → its one keystroke.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keymap {
    overrides: BTreeMap<String, String>,
}

impl Keymap {
    /// The overrides kept from `entries`: known ids with plausible keystrokes.
    pub fn from_overrides(entries: BTreeMap<String, String>) -> Self {
        Self {
            overrides: entries
                .into_iter()
                .filter(|(id, key)| shortcut(id).is_some() && plausible(key))
                .collect(),
        }
    }
    pub fn overrides(&self) -> &BTreeMap<String, String> {
        &self.overrides
    }
    /// The keystrokes bound to `id`: its override, else its defaults.
    pub fn chords(&self, id: &str) -> Vec<String> {
        if let Some(key) = self.overrides.get(id) {
            return vec![key.clone()];
        }
        shortcut(id)
            .map(|s| s.defaults.iter().map(|k| (*k).to_owned()).collect())
            .unwrap_or_default()
    }
    /// The keystroke shown for `id`.
    pub fn primary(&self, id: &str) -> Option<String> {
        self.chords(id).into_iter().next()
    }
    pub fn is_custom(&self, id: &str) -> bool {
        self.overrides.contains_key(id)
    }
    /// The other action already using `key`, compared case-insensitively.
    pub fn conflict(&self, id: &str, key: &str) -> Option<&'static Shortcut> {
        SHORTCUTS.iter().find(|s| {
            s.id != id
                && self
                    .chords(s.id)
                    .iter()
                    .any(|k| k.eq_ignore_ascii_case(key))
        })
    }
    /// Binds `id` to `key` (a default keystroke clears the override).
    pub fn set(&mut self, id: &str, key: &str) {
        let Some(entry) = shortcut(id) else { return };
        if !plausible(key) {
            return;
        }
        if entry.defaults.len() == 1 && entry.defaults[0].eq_ignore_ascii_case(key) {
            self.overrides.remove(id);
        } else {
            self.overrides.insert(id.to_owned(), key.to_owned());
        }
    }
    pub fn reset(&mut self, id: &str) {
        self.overrides.remove(id);
    }
    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_defaults_never_collide() {
        let map = Keymap::default();
        for (i, a) in SHORTCUTS.iter().enumerate() {
            assert!(SHORTCUTS[i + 1..].iter().all(|b| b.id != a.id), "{}", a.id);
            for key in a.defaults {
                assert!(plausible(key), "{key}");
                assert_eq!(map.conflict(a.id, key), None, "{} {key}", a.id);
            }
        }
    }

    #[test]
    fn overrides_replace_defaults_and_reset_restores_them() {
        let mut map = Keymap::default();
        assert_eq!(map.chords("go.review"), ["alt+o"]);
        map.set("go.review", "mod+shift+p");
        assert_eq!(map.chords("go.review"), ["mod+shift+p"]);
        assert!(map.is_custom("go.review"));
        assert_eq!(
            map.conflict("reconnect", "MOD+SHIFT+P").map(|s| s.id),
            Some("go.review")
        );
        map.reset("go.review");
        assert_eq!(map.chords("go.review"), ["alt+o"]);
        // Setting a single default back clears the override.
        map.set("reconnect", "mod+e");
        map.set("reconnect", "mod+r");
        assert!(!map.is_custom("reconnect"));
    }

    #[test]
    fn unknown_ids_and_implausible_keystrokes_are_dropped() {
        let entries = BTreeMap::from([
            ("nope".to_owned(), "mod+x".to_owned()),
            ("reconnect".to_owned(), "mod r".to_owned()),
            ("project.next".to_owned(), "mod+j".to_owned()),
        ]);
        let map = Keymap::from_overrides(entries);
        assert_eq!(map.overrides().len(), 1);
        assert_eq!(map.primary("project.next").as_deref(), Some("mod+j"));
    }
}
