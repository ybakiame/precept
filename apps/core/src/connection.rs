//! Session-only connection drafts. Credentials are never fields in this model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Target {
    #[default]
    Synthetic,
    Local,
    Wsl,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Connection {
    pub target: Target,
    pub runtime: String,
    pub endpoint: String,
    pub distribution: String,
    pub user: String,
    pub helper: String,
    pub wsl_runtime: String,
    pub wsl_endpoint: String,
    pub synthetic_rows: Option<u32>,
}
impl Connection {
    pub fn validate(&self) -> bool {
        let safe = |s: &str| {
            !s.is_empty()
                && s.len() <= 1024
                && !s.chars().any(|c| {
                    c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
                })
        };
        let linux = |s: &str| safe(s) && s.starts_with('/') && !s.split('/').any(|p| p == "..");
        match self.target {
            Target::Synthetic => self.synthetic_rows.is_none_or(|n| (1..=10000).contains(&n)),
            Target::Local => {
                safe(&self.runtime)
                    && std::path::Path::new(&self.runtime).is_absolute()
                    && safe(&self.endpoint)
            }
            Target::Wsl => {
                let d = &self.distribution;
                let u = &self.user;
                d.len() <= 64
                    && d.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
                    && d.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_. ".contains(&c))
                    && u.len() <= 64
                    && u.as_bytes()
                        .first()
                        .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
                    && u.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
                    && [&self.helper, &self.wsl_runtime, &self.wsl_endpoint]
                        .into_iter()
                        .all(|s| linux(s))
            }
        }
    }
}
/// Explicit WSL connection states. Fixed strings: they travel from the native
/// layer as read-failure reasons and map to localized messages.
pub mod wsl_status {
    use crate::i18n::Msg;
    pub const HOST_UNAVAILABLE: &str = "WSL is not available on this Windows host";
    pub const DISTRIBUTION_NOT_FOUND: &str = "WSL distribution not found";
    pub const USER_UNAVAILABLE: &str = "WSL user not available in this distribution";
    pub const LAUNCH_FAILED: &str = "WSL could not start the bridge";
    pub const HELPER_MISSING: &str = "Bridge helper not found or not executable in WSL";
    pub const HELPER_REFUSED: &str = "Bridge helper refused the runtime or endpoint";
    pub const HELPER_INCOMPATIBLE: &str = "Incompatible bridge helper version";
    pub const BRIDGE_TIMEOUT: &str = "WSL bridge did not respond";
    pub const BRIDGE_DISCONNECTED: &str = "WSL bridge disconnected";
    pub const DAEMON_UNREACHABLE: &str = "WSL daemon not running or not reachable";
    pub const QUERY_TIMED_OUT: &str = "WSL query timed out";
    /// The runtime was refused (ownership/permissions); shared with local reads.
    pub const RUNTIME_REFUSED: &str = "Permission denied";

    /// Classifies a launch that ended before the protocol hello, from the
    /// launcher's exit status. For `wsl.exe`: -1 is a WSL-level failure
    /// (distribution or user), 1 a failed exec of the helper (which never exits 1
    /// itself) and 2 a helper argument refusal. `distribution_registered` comes
    /// from `wsl.exe --list --quiet`, consulted only after a -1.
    pub fn launch_failure(
        exit: Option<i32>,
        distribution_registered: Option<bool>,
    ) -> &'static str {
        match exit {
            Some(-1) => match distribution_registered {
                Some(false) => DISTRIBUTION_NOT_FOUND,
                Some(true) => USER_UNAVAILABLE,
                None => LAUNCH_FAILED,
            },
            Some(1) => HELPER_MISSING,
            Some(2) => HELPER_REFUSED,
            _ => LAUNCH_FAILED,
        }
    }
    pub fn message(reason: &str) -> Option<Msg> {
        Some(match reason {
            HOST_UNAVAILABLE => Msg::WslHostUnavailable,
            DISTRIBUTION_NOT_FOUND => Msg::WslDistributionNotFound,
            USER_UNAVAILABLE => Msg::WslUserUnavailable,
            LAUNCH_FAILED => Msg::WslLaunchFailed,
            HELPER_MISSING => Msg::WslHelperMissing,
            HELPER_REFUSED => Msg::WslHelperRefused,
            HELPER_INCOMPATIBLE => Msg::WslHelperIncompatible,
            BRIDGE_TIMEOUT => Msg::WslBridgeTimeout,
            BRIDGE_DISCONNECTED => Msg::WslBridgeDisconnected,
            DAEMON_UNREACHABLE => Msg::WslDaemonUnreachable,
            QUERY_TIMED_OUT => Msg::WslQueryTimedOut,
            RUNTIME_REFUSED => Msg::WslRuntimeRefused,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wsl_launch_failures_are_classified_from_the_exit_status_only() {
        use wsl_status::*;
        assert_eq!(
            launch_failure(Some(-1), Some(false)),
            DISTRIBUTION_NOT_FOUND
        );
        assert_eq!(launch_failure(Some(-1), Some(true)), USER_UNAVAILABLE);
        assert_eq!(launch_failure(Some(-1), None), LAUNCH_FAILED);
        assert_eq!(launch_failure(Some(1), None), HELPER_MISSING);
        assert_eq!(launch_failure(Some(2), None), HELPER_REFUSED);
        for other in [None, Some(0), Some(101), Some(127)] {
            assert_eq!(launch_failure(other, None), LAUNCH_FAILED);
        }
        // Every reason has a localized message in both catalogs.
        for reason in [
            HOST_UNAVAILABLE,
            DISTRIBUTION_NOT_FOUND,
            USER_UNAVAILABLE,
            LAUNCH_FAILED,
            HELPER_MISSING,
            HELPER_REFUSED,
            HELPER_INCOMPATIBLE,
            BRIDGE_TIMEOUT,
            BRIDGE_DISCONNECTED,
            DAEMON_UNREACHABLE,
            QUERY_TIMED_OUT,
            RUNTIME_REFUSED,
        ] {
            let msg = message(reason).unwrap();
            for locale in [crate::i18n::Locale::En, crate::i18n::Locale::ZhCn] {
                assert!(!crate::i18n::tr(locale, msg).is_empty());
            }
        }
        assert_eq!(message("/home/user/private"), None);
    }
    #[test]
    fn drafts_never_silently_fall_back_to_fixtures() {
        let mut c = Connection {
            target: Target::Local,
            ..Default::default()
        };
        assert!(!c.validate());
        c.target = Target::Wsl;
        c.distribution = "Synthetic Linux".into();
        c.user = "operator".into();
        c.helper = "/synthetic/acp-ui-snapshot".into();
        c.wsl_runtime = "/synthetic/runtime".into();
        c.wsl_endpoint = "/synthetic/ipc.sock".into();
        assert!(c.validate());
        c.helper = "/synthetic/../private".into();
        assert!(!c.validate());
        c.helper = "/synthetic/helper".into();
        c.user = "operator;sh".into();
        assert!(!c.validate());
    }
}
