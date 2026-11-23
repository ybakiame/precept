//! Connection drafts, discovery reports and the saved choice. Credentials are
//! never fields in this model: capabilities stay inside the daemon host.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    #[default]
    Synthetic,
    Local,
    Wsl,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Connection {
    pub target: Target,
    pub runtime: String,
    pub endpoint: String,
    pub distribution: String,
    pub user: String,
    pub helper: String,
    pub wsl_runtime: String,
    pub wsl_endpoint: String,
    /// Offline fixture size; a launch option only, never saved.
    #[serde(skip)]
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
/// Version of the read-only discovery report (`precept-bridge --discover`).
pub const DISCOVERY_VERSION: u32 = 1;

/// Whether the daemon answered a health read through the host's operator
/// credential. `Refused` is an ownership/permission refusal of the runtime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum DaemonStatus {
    #[default]
    NotRunning,
    Refused,
    Ready {
        profile_selected: bool,
    },
}

/// What a fixed, read-only check of one daemon host reports: who runs there,
/// its conventional runtime, whether its Linux systemd unit or Windows daemon
/// executable is present and whether the daemon answers. No credential, Profile
/// path or preference value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Discovery {
    pub version: u32,
    pub user: String,
    /// Absolute path of the helper that answered (WSL only; empty locally).
    pub helper: String,
    pub runtime: String,
    pub endpoint: String,
    pub systemd: bool,
    pub unit_installed: bool,
    pub daemon: DaemonStatus,
}
impl Discovery {
    /// Fills the WSL fields of `draft` from this report. The draft still goes
    /// through `Connection::validate` before it is applied.
    pub fn fill_wsl(&self, draft: &mut Connection, distribution: &str) {
        draft.target = Target::Wsl;
        draft.distribution = distribution.into();
        draft.user = self.user.clone();
        draft.helper = self.helper.clone();
        draft.wsl_runtime = self.runtime.clone();
        draft.wsl_endpoint = self.endpoint.clone();
    }
    /// Fills the local fields of `draft` (a daemon on this machine).
    pub fn fill_local(&self, draft: &mut Connection) {
        draft.target = Target::Local;
        draft.runtime = self.runtime.clone();
        draft.endpoint = self.endpoint.clone();
    }
    pub fn ready(&self) -> bool {
        matches!(self.daemon, DaemonStatus::Ready { .. })
    }
    /// Structural check of a report received from another process.
    pub fn validate(&self, wsl: bool) -> bool {
        let mut probe = Connection::default();
        if wsl {
            self.fill_wsl(&mut probe, "probe");
        } else {
            self.fill_local(&mut probe);
        }
        self.version == DISCOVERY_VERSION && probe.validate()
    }
}

/// The connection kept between launches (`connection.json` in the private state
/// or config directory). Only the target and its non-secret coordinates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    pub format: u32,
    pub connection: Connection,
}
impl Saved {
    pub const FORMAT: u32 = 1;
    pub fn new(connection: &Connection) -> Self {
        Self {
            format: Self::FORMAT,
            connection: Connection {
                synthetic_rows: None,
                ..connection.clone()
            },
        }
    }
    /// The saved connection, if it is this format and still valid.
    pub fn connection(self) -> Option<Connection> {
        (self.format == Self::FORMAT && self.connection.validate()).then_some(self.connection)
    }
}

/// Fixed reasons a discovery or daemon start can fail with. They cross the
/// WSL boundary as these exact strings and map to localized messages.
pub mod discovery_status {
    use crate::i18n::Msg;
    pub const NO_SYSTEMD: &str = "systemd is not running in this Linux environment";
    pub const NO_UNIT: &str = "The preceptd user service is not installed";
    pub const NO_EXECUTABLE: &str = "preceptd is not installed beside Precept Desktop";
    pub const START_FAILED: &str = "systemd could not start preceptd";
    pub const LOCAL_START_FAILED: &str = "The local preceptd process could not start";
    pub const START_TIMEOUT: &str = "preceptd did not become ready";
    pub const INVALID_REPORT: &str = "Invalid discovery report";
    pub const KNOWN: [&str; 6] = [
        NO_SYSTEMD,
        NO_UNIT,
        NO_EXECUTABLE,
        START_FAILED,
        LOCAL_START_FAILED,
        START_TIMEOUT,
    ];
    pub fn message(reason: &str) -> Option<Msg> {
        Some(match reason {
            NO_SYSTEMD => Msg::HintSystemd,
            NO_UNIT => Msg::HintService,
            NO_EXECUTABLE => Msg::HintDaemonExecutable,
            START_FAILED => Msg::DiscoveryStartFailed,
            LOCAL_START_FAILED => Msg::LocalStartFailed,
            START_TIMEOUT => Msg::DiscoveryStartTimeout,
            INVALID_REPORT => Msg::DiscoveryInvalid,
            _ => return None,
        })
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
    fn report() -> Discovery {
        Discovery {
            version: DISCOVERY_VERSION,
            user: "operator".into(),
            helper: "/usr/bin/precept-bridge".into(),
            runtime: "/home/operator/.local/state/precept/runtime".into(),
            endpoint: "/home/operator/.local/state/precept/runtime/control.sock".into(),
            systemd: true,
            unit_installed: true,
            daemon: DaemonStatus::Ready {
                profile_selected: false,
            },
        }
    }
    #[test]
    fn discovery_fills_a_valid_draft_and_refuses_unsafe_reports() {
        let d = report();
        assert!(d.validate(true) && d.ready());
        // This Unix local report is valid only on Unix; Windows discovery uses
        // its own absolute runtime and named-pipe endpoint.
        assert_eq!(d.validate(false), cfg!(unix));
        let mut draft = Connection::default();
        d.fill_wsl(&mut draft, "archlinux");
        assert!(draft.validate());
        assert_eq!(draft.wsl_endpoint, d.endpoint);
        for bad in [
            Discovery {
                version: 2,
                ..report()
            },
            Discovery {
                user: "-root".into(),
                ..report()
            },
            Discovery {
                user: "a b".into(),
                ..report()
            },
            Discovery {
                runtime: "relative/runtime".into(),
                ..report()
            },
            Discovery {
                endpoint: "/a/../b".into(),
                ..report()
            },
            Discovery {
                helper: "precept-bridge".into(),
                ..report()
            },
        ] {
            assert!(!bad.validate(true), "{bad:?}");
        }
        // Unknown fields from a newer or hostile helper refuse to parse.
        let mut v = serde_json::to_value(report()).unwrap();
        v["token"] = "x".into();
        assert!(serde_json::from_value::<Discovery>(v).is_err());
        let json = serde_json::to_value(DaemonStatus::Ready {
            profile_selected: true,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"state":"ready","profile_selected":true})
        );
    }
    #[test]
    fn discovery_reasons_have_localized_messages() {
        for reason in discovery_status::KNOWN {
            let msg = discovery_status::message(reason).unwrap();
            for locale in [crate::i18n::Locale::En, crate::i18n::Locale::ZhCn] {
                assert!(!crate::i18n::tr(locale, msg).is_empty());
            }
        }
    }
    #[test]
    fn saved_connection_round_trips_without_fixture_rows_and_refuses_invalid() {
        let mut c = Connection::default();
        report().fill_wsl(&mut c, "archlinux");
        c.synthetic_rows = Some(10);
        let text = serde_json::to_string(&Saved::new(&c)).unwrap();
        assert!(!text.contains("synthetic_rows"));
        let back: Saved = serde_json::from_str(&text).unwrap();
        let restored = back.connection().unwrap();
        assert_eq!(restored.synthetic_rows, None);
        assert_eq!(restored.distribution, "archlinux");
        let mut invalid = Saved::new(&c);
        invalid.connection.wsl_runtime = "relative".into();
        assert!(invalid.connection().is_none());
        let mut future = Saved::new(&c);
        future.format = 2;
        assert!(future.connection().is_none());
        assert!(
            serde_json::from_str::<Saved>(r#"{"format":1,"connection":{},"extra":1}"#).is_err()
        );
    }
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
        c.helper = "/synthetic/precept-ui-snapshot".into();
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
