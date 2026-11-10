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
#[cfg(test)]
mod tests {
    use super::*;
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
