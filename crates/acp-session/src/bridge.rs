//! Fixed, operator-selected stdio bridge; no agent-supplied commands or inherited credentials.
use acp_ipc::{
    Error, Location, Result,
    session_lease::{Registry, SessionLease},
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
#[derive(Clone)]
pub(crate) struct Bridge {
    executable: PathBuf,
    location: Location,
    registry: Registry,
    ttl: Duration,
}
impl Bridge {
    pub fn new(
        executable: &Path,
        location: Location,
        registry: Registry,
        ttl: Duration,
        roots: &[PathBuf],
    ) -> Result<Self> {
        if !executable.is_absolute()
            || !(Duration::from_secs(1)..=Duration::from_secs(3600)).contains(&ttl)
        {
            return Err(Error::InvalidRequest);
        }
        let executable = executable.canonicalize().map_err(|_| Error::Unavailable)?;
        if !executable.is_file()
            || executable.to_str().is_none()
            || roots.iter().any(|r| executable.starts_with(r))
        {
            return Err(Error::UnsafeRuntime);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if executable
                .metadata()
                .map_err(|_| Error::Unavailable)?
                .permissions()
                .mode()
                & 0o111
                == 0
            {
                return Err(Error::UnsafeRuntime);
            }
        }
        #[cfg(windows)]
        if !executable
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
        {
            return Err(Error::UnsafeRuntime);
        }
        Ok(Self {
            executable,
            location,
            registry,
            ttl,
        })
    }
    pub fn issue(
        &self,
        project: &str,
        session: &str,
        agent: &str,
    ) -> Result<(SessionLease, Value)> {
        if self.executable.canonicalize().ok().as_ref() != Some(&self.executable)
            || !self.executable.is_file()
        {
            return Err(Error::UnsafeRuntime);
        }
        let lease = self
            .registry
            .issue(&self.location, project, session, agent, self.ttl)?;
        let servers = json!([{"name":"acp-context","command":self.executable,"args":["--runtime-dir",self.location.runtime_dir,"--endpoint",self.location.endpoint,"--project",project,"--capability-file",lease.path()],"env":[]}]);
        Ok((lease, servers))
    }
}
