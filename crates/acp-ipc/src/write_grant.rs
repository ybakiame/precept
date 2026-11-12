//! Ephemeral write authority. No SQLite, selection, owner lease or mutation API.
use crate::{Access, Credential, Error, Location, Result, transport};
use std::{
    fs,
    io::Write,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use subtle::ConstantTimeEq;

pub(crate) fn binding_valid(access: &Access) -> bool {
    match access {
        Access::OperatorWrite {
            profile_id,
            generation,
            owner_epoch,
        } => {
            [profile_id, generation].iter().all(|v| {
                v.len() == 32
                    && v.bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            }) && *owner_epoch > 0
        }
        _ => false,
    }
}
pub struct Grant {
    credential: Credential,
    active: AtomicBool,
}
impl Grant {
    pub fn access(&self) -> Access {
        self.credential.access.clone()
    }
    pub fn check(&self) -> Result<()> {
        if self.active.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }
}
#[derive(Clone, Default)]
pub struct Registry(Arc<Mutex<Option<Arc<Grant>>>>);
impl Registry {
    pub fn close(&self) {
        if let Some(g) = self.0.lock().unwrap().as_ref() {
            g.active.store(false, Ordering::SeqCst);
        }
    }
    /// Only caller holding the process RuntimeLease may clean a stale token.
    pub fn cleanup_stale(&self, location: &Location) -> Result<()> {
        if self.0.lock().unwrap().is_some() {
            return Err(Error::Busy);
        }
        let path = location.root()?.join("operator-write.cap");
        if fs::symlink_metadata(&path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
            return Ok(());
        }
        let c = crate::load_credential(location, &path)?;
        if c.format != 3 || !matches!(c.access, Access::OperatorWrite { .. }) {
            return Err(Error::UnsafeRuntime);
        }
        fs::remove_file(path).map_err(|_| Error::UnsafeRuntime)
    }
    pub fn issue(&self, location: &Location, access: Access) -> Result<Arc<Grant>> {
        let mut current = self.0.lock().unwrap();
        if current.is_some() {
            return Err(Error::Busy);
        }
        if !binding_valid(&access) {
            return Err(Error::Unauthorized);
        }
        let path = location.root()?.join("operator-write.cap");
        let mut token = [0; 32];
        getrandom::fill(&mut token).map_err(|_| Error::Unavailable)?;
        let credential = Credential {
            format: 3,
            access,
            token,
            binding: None,
        };
        let mut file = transport::new_private(&path)?;
        let written = (|| {
            file.write_all(&serde_json::to_vec(&credential).map_err(|_| Error::Unavailable)?)
                .map_err(|_| Error::Unavailable)?;
            file.sync_all().map_err(|_| Error::Unavailable)?;
            transport::private_file(&path)
        })();
        drop(file);
        if let Err(e) = written {
            let _ = fs::remove_file(&path);
            return Err(e);
        }
        let grant = Arc::new(Grant {
            credential,
            active: AtomicBool::new(true),
        });
        *current = Some(grant.clone());
        Ok(grant)
    }
    pub fn finish_revoke(&self, location: &Location) -> Result<()> {
        self.close();
        let previous = self.0.lock().unwrap().take();
        if let Some(g) = previous {
            let path = location.root()?.join("operator-write.cap");
            match crate::load_credential(location, &path) {
                Ok(c) if c.format == 3 && bool::from(c.token.ct_eq(&g.credential.token)) => {
                    fs::remove_file(path).map_err(|_| Error::UnsafeRuntime)?
                }
                _ if fs::symlink_metadata(&path)
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) => {}
                _ => return Err(Error::UnsafeRuntime),
            }
        }
        Ok(())
    }
    pub(crate) fn authenticate(&self, token: &[u8; 32]) -> Result<Arc<Grant>> {
        let current = self.0.lock().unwrap();
        let g = current
            .as_ref()
            .filter(|g| bool::from(g.credential.token.ct_eq(token)))
            .ok_or(Error::Unauthorized)?;
        g.check()?;
        Ok(g.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn access() -> Access {
        Access::OperatorWrite {
            profile_id: "0".repeat(32),
            generation: "1".repeat(32),
            owner_epoch: 1,
        }
    }
    #[test]
    fn write_envelope_cannot_be_control_agent_or_session_and_cleanup_is_matching_only() {
        let t = tempfile::tempdir().unwrap();
        let loc = Location {
            runtime_dir: t.path().join("run"),
            endpoint: "unused".into(),
        };
        let _runtime = crate::RuntimeLease::acquire(&loc).unwrap();
        let registry = Registry::default();
        assert!(matches!(
            registry.issue(&loc, Access::Operator),
            Err(Error::Unauthorized)
        ));
        let grant = registry.issue(&loc, access()).unwrap();
        assert_eq!(registry.cleanup_stale(&loc), Err(Error::Busy));
        let path = loc.runtime_dir.join("operator-write.cap");
        let c = crate::load_credential(&loc, &path).unwrap();
        let raw = serde_json::to_value(&c).unwrap();
        for change in [
            json!({"format":1}),
            json!({"format":2,"binding":{"session":"s","agent":"a"}}),
            json!({"access":{"role":"agent","project":"alpha"}}),
            json!({"access":{"role":"operator"}}),
            json!({"access":{"role":"operator_write","profile_id":"bad","generation":"1".repeat(32),"owner_epoch":1}}),
            json!({"access":{"role":"operator_write","profile_id":"0".repeat(32),"generation":"1".repeat(32),"owner_epoch":0}}),
        ] {
            let mut invalid = raw.clone();
            for (k, v) in change.as_object().unwrap() {
                invalid[k] = v.clone();
            }
            let p = loc.runtime_dir.join("malformed.cap");
            let mut file = transport::new_private(&p).unwrap();
            file.write_all(invalid.to_string().as_bytes()).unwrap();
            drop(file);
            assert!(matches!(
                crate::load_credential(&loc, &p),
                Err(Error::Unauthorized)
            ));
            fs::remove_file(p).unwrap();
        }
        registry.close();
        assert_eq!(grant.check(), Err(Error::Unauthorized));
        let other = loc.runtime_dir.join("different.cap");
        let mut file = transport::new_private(&other).unwrap();
        let mut changed = raw;
        changed["token"] = json!(vec![9; 32]);
        file.write_all(changed.to_string().as_bytes()).unwrap();
        drop(file);
        fs::remove_file(&path).unwrap();
        fs::rename(&other, &path).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(registry.finish_revoke(&loc), Err(Error::UnsafeRuntime));
        assert!(
            fs::read(&path).unwrap() == bytes,
            "unrelated credential must remain unchanged"
        );
        Registry::default().cleanup_stale(&loc).unwrap();
        assert!(!path.exists());
    }
}
