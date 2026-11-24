//! Durable per-user directories shared by Desktop settings and daemon discovery.
//! Windows keeps configuration and state in separate `config` and `state`
//! directories under `%LOCALAPPDATA%/precept`; Unix follows XDG config/state
//! conventions. Paths are explicit roots only: no cache directory, old-name
//! fallback, migration, creation or canonicalization.

use std::{env, path::PathBuf};

const APP_DIR: &str = "precept";

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).map(PathBuf::from)
}

fn app_dir(base: Option<PathBuf>) -> Option<PathBuf> {
    base.filter(|path| path.is_absolute())
        .map(|path| path.join(APP_DIR))
}

#[cfg(windows)]
fn windows_dir(local_app_data: Option<PathBuf>, directory: &str) -> Option<PathBuf> {
    app_dir(local_app_data).map(|path| path.join(directory))
}

#[cfg(unix)]
fn unix_config(xdg_config_home: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    let base = xdg_config_home
        .filter(|path| path.is_absolute())
        .or_else(|| {
            home.filter(|path| path.is_absolute())
                .map(|path| path.join(".config"))
        });
    app_dir(base)
}

#[cfg(unix)]
fn unix_state(xdg_state_home: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    let base = xdg_state_home
        .filter(|path| path.is_absolute())
        .or_else(|| {
            home.filter(|path| path.is_absolute())
                .map(|path| path.join(".local/state"))
        });
    app_dir(base)
}

pub(crate) fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        windows_dir(env_path("LOCALAPPDATA"), "config")
    }
    #[cfg(unix)]
    {
        unix_config(env_path("XDG_CONFIG_HOME"), env_path("HOME"))
    }
}

pub(crate) fn state_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        windows_dir(env_path("LOCALAPPDATA"), "state")
    }
    #[cfg(unix)]
    {
        unix_state(env_path("XDG_STATE_HOME"), env_path("HOME"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_roots_are_refused() {
        assert_eq!(app_dir(Some(PathBuf::from("relative"))), None);
        assert_eq!(app_dir(None), None);
    }

    #[cfg(windows)]
    #[test]
    fn windows_separates_config_and_state_under_the_app_directory() {
        let base = std::env::temp_dir();
        let root = base.join(APP_DIR);
        assert_eq!(
            windows_dir(Some(base.clone()), "config"),
            Some(root.join("config"))
        );
        assert_eq!(windows_dir(Some(base), "state"), Some(root.join("state")));
        assert_eq!(windows_dir(Some(PathBuf::from("relative")), "state"), None);
    }

    #[cfg(unix)]
    #[test]
    fn unix_config_prefers_xdg_and_falls_back_to_home() {
        assert_eq!(
            unix_config(
                Some(PathBuf::from("/xdg/config")),
                Some(PathBuf::from("/home/u"))
            ),
            Some(PathBuf::from("/xdg/config/precept"))
        );
        assert_eq!(
            unix_config(None, Some(PathBuf::from("/home/u"))),
            Some(PathBuf::from("/home/u/.config/precept"))
        );
        assert_eq!(
            unix_config(
                Some(PathBuf::from("relative")),
                Some(PathBuf::from("/home/u"))
            ),
            Some(PathBuf::from("/home/u/.config/precept"))
        );
        assert_eq!(unix_config(Some(PathBuf::from("relative")), None), None);
    }

    #[cfg(unix)]
    #[test]
    fn unix_state_prefers_xdg_and_falls_back_to_home() {
        assert_eq!(
            unix_state(
                Some(PathBuf::from("/xdg/state")),
                Some(PathBuf::from("/home/u"))
            ),
            Some(PathBuf::from("/xdg/state/precept"))
        );
        assert_eq!(
            unix_state(None, Some(PathBuf::from("/home/u"))),
            Some(PathBuf::from("/home/u/.local/state/precept"))
        );
        assert_eq!(
            unix_state(
                Some(PathBuf::from("relative")),
                Some(PathBuf::from("/home/u"))
            ),
            Some(PathBuf::from("/home/u/.local/state/precept"))
        );
        assert_eq!(unix_state(Some(PathBuf::from("relative")), None), None);
    }
}
