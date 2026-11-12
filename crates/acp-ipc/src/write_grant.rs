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
        if !matches!(access, Access::OperatorWrite { .. }) {
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
