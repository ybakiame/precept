//! Ephemeral session authority. No persisted registry, operator rights or token diagnostics.
use crate::{Access, Credential, Error, Location, Result, Stream, transport};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub session: String,
    pub agent: String,
}
struct Entry {
    credential: Credential,
    active: AtomicBool,
    deadline: Instant,
    context: AtomicU64,
    handoff: AtomicU64,
}
impl Entry {
    fn check(&self) -> Result<()> {
        if self.active.load(Ordering::SeqCst) && Instant::now() < self.deadline {
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    }
}
#[derive(Clone, Default)]
pub struct Registry(Arc<Mutex<Option<Arc<Entry>>>>);
/// No secret is copied into the connection handle; validity is checked per request.
#[derive(Clone)]
pub struct Peer {
    access: Access,
    lease: Option<Arc<Entry>>,
}
impl Peer {
    pub fn is_session(&self) -> bool {
        self.lease.is_some()
    }
    pub fn access(&self) -> Access {
        self.access.clone()
    }
    pub fn check(&self) -> Result<()> {
        self.lease.as_ref().map_or(Ok(()), |e| e.check())
    }
    pub fn record(&self, method: &str) {
        if let Some(e) = &self.lease {
            let counter = match method {
                "context.get" => &e.context,
                "handoff.get" => &e.handoff,
                _ => return,
            };
            let _ = counter.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                Some(n.saturating_add(1))
            });
        }
    }
}
pub struct SessionLease {
    registry: Registry,
    entry: Arc<Entry>,
    location: Location,
    path: PathBuf,
}
impl SessionLease {
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
    pub fn expired(&self) -> bool {
        Instant::now() >= self.entry.deadline
    }
    pub fn observation(&self) -> Value {
        json!({"enabled":true,"active":self.entry.check().is_ok(),"context_calls":self.entry.context.load(Ordering::SeqCst),"handoff_calls":self.entry.handoff.load(Ordering::SeqCst)})
    }
    pub fn revoke(&self) {
        self.entry.active.store(false, Ordering::SeqCst);
        let mut current = self.registry.0.lock().unwrap();
        if current
            .as_ref()
            .is_some_and(|e| Arc::ptr_eq(e, &self.entry))
        {
            *current = None;
        }
        drop(current);
        // Remove only a private, still matching file. Cleanup cannot re-enable authority.
        if crate::load_credential(&self.location, &self.path)
            .is_ok_and(|c| c.format == 2 && bool::from(c.token.ct_eq(&self.entry.credential.token)))
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}
impl Drop for SessionLease {
    fn drop(&mut self) {
        self.revoke();
    }
}
impl Registry {
    /// Caller holds the exclusive RuntimeLease; no secret survives a daemon restart.
    pub fn cleanup_stale(&self, location: &Location) -> Result<()> {
        if self.0.lock().unwrap().is_some() {
            return Err(Error::Busy);
        }
        for item in fs::read_dir(location.root()?).map_err(|_| Error::UnsafeRuntime)? {
            let item = item.map_err(|_| Error::UnsafeRuntime)?;
            let name = item.file_name();
            let Some(name) = name.to_str() else { continue };
            if !name.starts_with("session-") || !name.ends_with(".cap") {
                continue;
            }
            let nonce = &name[8..name.len() - 4];
            if nonce.len() != 32 || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::UnsafeRuntime);
            }
            let c = crate::load_credential(location, &item.path())?;
            if c.format != 2 {
                return Err(Error::UnsafeRuntime);
            }
            fs::remove_file(item.path()).map_err(|_| Error::UnsafeRuntime)?;
        }
        Ok(())
    }
    /// Called only for a successfully spawned, explicitly opted-in direct child.
    pub fn issue(
        &self,
        location: &Location,
        project: &str,
        session: &str,
        agent: &str,
        ttl: Duration,
    ) -> Result<SessionLease> {
        if !crate::valid_project(project)
            || !crate::valid_project(agent)
            || !crate::valid_project(session)
            || !(Duration::from_secs(1)..=Duration::from_secs(3600)).contains(&ttl)
        {
            return Err(Error::InvalidRequest);
        }
        let mut current = self.0.lock().unwrap();
        if current.is_some() {
            return Err(Error::Busy);
        }
        let mut token = [0; 32];
        let mut nonce = [0; 16];
        getrandom::fill(&mut token).map_err(|_| Error::Unavailable)?;
        getrandom::fill(&mut nonce).map_err(|_| Error::Unavailable)?;
        let name = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let path = location.root()?.join(format!("session-{name}.cap"));
        let credential = Credential {
            format: 2,
            access: Access::Agent {
                project: project.into(),
            },
            token,
            binding: Some(Binding {
                session: session.into(),
                agent: agent.into(),
            }),
        };
        let mut file = transport::new_private(&path)?;
        let write = (|| {
            file.write_all(&serde_json::to_vec(&credential).map_err(|_| Error::Unavailable)?)
                .map_err(|_| Error::Unavailable)?;
            file.sync_all().map_err(|_| Error::Unavailable)?;
            transport::private_file(&path)
        })();
        drop(file);
        if let Err(e) = write {
            let _ = fs::remove_file(&path);
            return Err(e);
        }
        let entry = Arc::new(Entry {
            credential,
            active: AtomicBool::new(true),
            deadline: Instant::now() + ttl,
            context: AtomicU64::new(0),
            handoff: AtomicU64::new(0),
        });
        *current = Some(entry.clone());
        Ok(SessionLease {
            registry: self.clone(),
            entry,
            location: location.clone(),
            path,
        })
    }
    pub async fn accept(&self, stream: &mut Stream, credentials: &[Credential]) -> Result<Peer> {
        let hello: crate::Hello = serde_json::from_value(crate::read_frame(stream).await?)
            .map_err(|_| Error::Unauthorized)?;
        if hello.api_version != 1 {
            return Err(Error::Version);
        }
        let peer = if let Some(c) = credentials
            .iter()
            .find(|c| bool::from(c.token.ct_eq(&hello.credential)))
        {
            Peer {
                access: c.access.clone(),
                lease: None,
            }
        } else {
            let current = self.0.lock().unwrap();
            let e = current
                .as_ref()
                .filter(|e| bool::from(e.credential.token.ct_eq(&hello.credential)))
                .ok_or(Error::Unauthorized)?;
            e.check()?;
            Peer {
                access: e.credential.access.clone(),
                lease: Some(e.clone()),
            }
        };
        peer.check()?;
        crate::write_frame(
            stream,
            &json!({"api_version":1,"ready":true,"access":peer.access}),
        )
        .await?;
        Ok(peer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (tempfile::TempDir, crate::RuntimeLease, Location, Registry) {
        let tmp = tempfile::tempdir().unwrap();
        let location = Location {
            runtime_dir: tmp.path().join("run"),
            endpoint: "unused".into(),
        };
        let owner = crate::RuntimeLease::acquire(&location).unwrap();
        (tmp, owner, location, Registry::default())
    }
    async fn connect(registry: &Registry, c: &Credential) -> Result<Peer> {
        let (a, mut b) = tokio::io::duplex(4096);
        let mut a: Stream = Box::new(a);
        let server = registry.accept(&mut a, &[]);
        let client = async {
            crate::write_frame(
                &mut b,
                &crate::Hello {
                    api_version: 1,
                    credential: c.token,
                },
            )
            .await
            .unwrap();
        };
        let (result, ()) = tokio::join!(server, client);
        result
    }
    #[tokio::test]
    async fn revoke_denies_established_peer_new_handshake_and_old_generation() {
        let (_tmp, _owner, loc, registry) = setup();
        let first = registry
            .issue(
                &loc,
                "alpha",
                "generation-1",
                "agent",
                Duration::from_secs(20),
            )
            .unwrap();
        let old = crate::load_credential(&loc, first.path()).unwrap();
        let peer = connect(&registry, &old).await.unwrap();
        assert!(peer.check().is_ok());
        peer.record("context.get");
        assert_eq!(first.observation()["context_calls"], 1);
        first.revoke();
        assert_eq!(peer.check(), Err(Error::Unauthorized));
        assert!(!first.path().exists());
        assert!(matches!(
            connect(&registry, &old).await,
            Err(Error::Unauthorized)
        ));
        let second = registry
            .issue(
                &loc,
                "beta",
                "generation-2",
                "other-agent",
                Duration::from_secs(20),
            )
            .unwrap();
        let new = crate::load_credential(&loc, second.path()).unwrap();
        drop(first);
        assert_eq!(peer.check(), Err(Error::Unauthorized));
        assert!(matches!(
            connect(&registry, &old).await,
            Err(Error::Unauthorized)
        ));
        assert_eq!(
            connect(&registry, &new).await.unwrap().access(),
            Access::Agent {
                project: "beta".into()
            }
        );
        assert_eq!(second.observation()["context_calls"], 0);
    }
    #[tokio::test]
    async fn expiry_denies_open_peer_without_relying_on_a_cleanup_tick() {
        let (_tmp, _owner, loc, registry) = setup();
        let lease = registry
            .issue(
                &loc,
                "alpha",
                "generation-1",
                "agent",
                Duration::from_secs(1),
            )
            .unwrap();
        let c = crate::load_credential(&loc, lease.path()).unwrap();
        let peer = connect(&registry, &c).await.unwrap();
        tokio::time::sleep(Duration::from_millis(1050)).await;
        assert_eq!(peer.check(), Err(Error::Unauthorized));
        assert!(matches!(
            connect(&registry, &c).await,
            Err(Error::Unauthorized)
        ));
        assert!(lease.expired());
        lease.revoke();
        assert!(!lease.path().exists());
    }
    #[test]
    fn credential_formats_are_distinct_and_stale_cleanup_fails_closed() {
        let (_tmp, mut owner, loc, registry) = setup();
        let legacy = owner.credentials(&["alpha".into()]).unwrap();
        assert_eq!(legacy.len(), 2);
        let lease = registry
            .issue(
                &loc,
                "alpha",
                "generation-1",
                "agent",
                Duration::from_secs(20),
            )
            .unwrap();
        let original = crate::load_credential(&loc, lease.path()).unwrap();
        let raw = serde_json::to_value(&original).unwrap();
        for mutation in [
            json!({"format":1}),
            json!({"binding":null}),
            json!({"binding":{"session":"../escape","agent":"agent"}}),
            json!({"access":{"role":"operator"}}),
            json!({"extra":true}),
        ] {
            let mut v = raw.clone();
            for (k, x) in mutation.as_object().unwrap() {
                v[k] = x.clone();
            }
            let path = loc.root().unwrap().join("malformed.cap");
            let mut file = transport::new_private(&path).unwrap();
            file.write_all(v.to_string().as_bytes()).unwrap();
            drop(file);
            assert!(matches!(
                crate::load_credential(&loc, &path),
                Err(Error::Unauthorized)
            ));
            fs::remove_file(path).unwrap();
        }
        let collision = loc.root().unwrap().join("session-collision.cap");
        transport::new_private(&collision).unwrap();
        assert_eq!(
            Registry::default().cleanup_stale(&loc),
            Err(Error::UnsafeRuntime)
        );
        fs::remove_file(collision).unwrap();
        Registry::default().cleanup_stale(&loc).unwrap();
        assert!(!lease.path().exists());
        assert!(loc.credential_path(None).unwrap().exists());
    }
}
