//! The daemon's own state directory (`--state-dir`): the project set and the
//! remembered startup Profile. Daemon-owned, private (precept-ipc's owner-only
//! checks), strict JSON with unknown fields refused, bounded, written
//! atomically. A file that is malformed, shared or oversized refuses the
//! start; the daemon never repairs or rewrites state to make itself start.
use precept_core::repo_fs::RootIdentity;
use precept_ipc::{
    Error, Result,
    transport::{new_private, open_private, private_dir},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// A defensive bound, not a product promise: it keeps a damaged or runaway file
/// from minting unbounded agent credentials and keeps per-request root checks
/// small. The real capacities are bounded elsewhere (IPC peers, Profile commits).
pub const MAX_PROJECTS: usize = 256;
const MAX_FILE: u64 = 256 * 1024;
pub const PROJECTS: &str = "projects.json";
pub const STARTUP: &str = "startup.json";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectEntry {
    name: String,
    root: PathBuf,
    /// Persistent filesystem identity. Legacy entries without one stay unavailable.
    identity: Option<RootIdentity>,
    /// External agents may connect (its agent credential is published).
    /// Written explicitly; absent reads as shared.
    #[serde(default = "shared")]
    agents: bool,
    /// Agents may suggest rules for it (the operator's switch). Absent reads
    /// as off.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    propose: bool,
    /// The pre-tool gate answers for it (the operator's switch). Absent
    /// reads as off.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    gate: bool,
}
/// The operator's per-project switches kept with the project set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Switches {
    /// External agents off (agent credential not published).
    pub withheld: BTreeSet<String>,
    /// Agents may suggest rules.
    pub proposers: BTreeSet<String>,
    /// The pre-tool gate answers for the project's hard rules.
    pub gated: BTreeSet<String>,
}
impl Switches {
    /// A project (re)joins or leaves: shared with agents, no suggestions, no gate.
    pub fn reset(&mut self, name: &str) {
        self.withheld.remove(name);
        self.proposers.remove(name);
        self.gated.remove(name);
    }
}
fn shared() -> bool {
    true
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectsFile {
    format: u32,
    projects: Vec<ProjectEntry>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StartupFile {
    format: u32,
    profile: PathBuf,
}

pub struct StateDir {
    dir: PathBuf,
}

impl StateDir {
    /// Opens (creating 0700 if absent) the private state directory. It must be
    /// absolute and outside any Git checkout.
    pub fn open(path: &Path) -> Result<Self> {
        if !path.is_absolute() {
            return Err(Error::UnsafeRuntime);
        }
        private_dir(path, true)?;
        let dir = path.canonicalize().map_err(|_| Error::UnsafeRuntime)?;
        if dir.ancestors().any(|a| a.join(".git").exists()) {
            return Err(Error::UnsafeRuntime);
        }
        Ok(Self { dir })
    }
    pub fn path(&self) -> &Path {
        &self.dir
    }
    pub(crate) fn read(&self, name: &str) -> Result<Option<String>> {
        self.read_bounded(name, MAX_FILE)
    }
    pub(crate) fn read_bounded(&self, name: &str, max: u64) -> Result<Option<String>> {
        let path = self.dir.join(name);
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Error::UnsafeRuntime),
            Ok(_) => {}
        }
        let mut file = open_private(&path, false)?;
        if file.metadata().map_err(|_| Error::Unavailable)?.len() > max {
            return Err(Error::TooLarge);
        }
        let mut text = String::new();
        Read::by_ref(&mut file)
            .take(max + 1)
            .read_to_string(&mut text)
            .map_err(|_| Error::InvalidRequest)?;
        if text.len() as u64 > max {
            return Err(Error::TooLarge);
        }
        Ok(Some(text))
    }
    pub(crate) fn write(&self, name: &str, body: &[u8]) -> Result<()> {
        self.write_bounded(name, body, MAX_FILE)
    }
    pub(crate) fn write_bounded(&self, name: &str, body: &[u8], max: u64) -> Result<()> {
        if body.len() as u64 > max {
            return Err(Error::TooLarge);
        }
        let temporary = self.dir.join(format!(".{name}.{}.tmp", std::process::id()));
        let _ = std::fs::remove_file(&temporary);
        let written = new_private(&temporary).and_then(|mut f| {
            f.write_all(body)
                .and_then(|_| f.sync_all())
                .map_err(|_| Error::Unavailable)
        });
        if let Err(e) = written.and_then(|_| {
            std::fs::rename(&temporary, self.dir.join(name)).map_err(|_| Error::Unavailable)
        }) {
            let _ = std::fs::remove_file(&temporary);
            return Err(e);
        }
        Ok(())
    }

    /// The project set as written (names and absolute roots, not yet checked
    /// against the filesystem), and the names withheld from external agents.
    /// Absent file: no projects.
    /// The project set, the projects withheld from agents, and those whose
    /// agents may suggest rules.
    #[allow(clippy::type_complexity)]
    pub fn load_projects(
        &self,
    ) -> Result<(
        BTreeMap<String, PathBuf>,
        BTreeMap<String, RootIdentity>,
        Switches,
    )> {
        let Some(text) = self.read(PROJECTS)? else {
            return Ok(Default::default());
        };
        let file: ProjectsFile = serde_json::from_str(&text).map_err(|_| Error::InvalidRequest)?;
        if file.format != 1 || file.projects.len() > MAX_PROJECTS {
            return Err(Error::InvalidRequest);
        }
        let mut seen = HashSet::new();
        let mut projects = BTreeMap::new();
        let mut identities = BTreeMap::new();
        let mut switches = Switches::default();
        for entry in file.projects {
            // Names collide case-insensitively, like their credential files.
            if !precept_ipc::valid_project(&entry.name)
                || !seen.insert(entry.name.to_ascii_lowercase())
                || !entry.root.is_absolute()
                || entry.root.to_str().is_none_or(|r| r.len() > 4096)
            {
                return Err(Error::InvalidRequest);
            }
            if !entry.agents {
                switches.withheld.insert(entry.name.clone());
            }
            if entry.propose {
                switches.proposers.insert(entry.name.clone());
            }
            if entry.gate {
                switches.gated.insert(entry.name.clone());
            }
            if let Some(identity) = entry.identity {
                identities.insert(entry.name.clone(), identity);
            }
            projects.insert(entry.name, entry.root);
        }
        Ok((projects, identities, switches))
    }
    pub fn save_projects(
        &self,
        projects: &BTreeMap<String, PathBuf>,
        identities: &BTreeMap<String, RootIdentity>,
        switches: &Switches,
    ) -> Result<()> {
        if projects.len() > MAX_PROJECTS || identities.len() > projects.len() {
            return Err(Error::Capacity);
        }
        let file = ProjectsFile {
            format: 1,
            projects: projects
                .iter()
                .map(|(name, root)| ProjectEntry {
                    name: name.clone(),
                    root: root.clone(),
                    identity: identities.get(name).cloned(),
                    agents: !switches.withheld.contains(name),
                    propose: switches.proposers.contains(name),
                    gate: switches.gated.contains(name),
                })
                .collect(),
        };
        self.write(
            PROJECTS,
            &serde_json::to_vec_pretty(&file).map_err(|_| Error::Unavailable)?,
        )
    }

    /// The Profile the operator explicitly asked to select at start.
    pub fn load_startup(&self) -> Result<Option<PathBuf>> {
        let Some(text) = self.read(STARTUP)? else {
            return Ok(None);
        };
        let file: StartupFile = serde_json::from_str(&text).map_err(|_| Error::InvalidRequest)?;
        if file.format != 1 || !file.profile.is_absolute() {
            return Err(Error::InvalidRequest);
        }
        Ok(Some(file.profile))
    }
    pub fn save_startup(&self, profile: &Path) -> Result<()> {
        if !profile.is_absolute() {
            return Err(Error::InvalidRequest);
        }
        let file = StartupFile {
            format: 1,
            profile: profile.to_path_buf(),
        };
        self.write(
            STARTUP,
            &serde_json::to_vec_pretty(&file).map_err(|_| Error::Unavailable)?,
        )
    }
    pub fn forget_startup(&self) -> Result<()> {
        match std::fs::remove_file(self.dir.join(STARTUP)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Error::Unavailable),
            _ => Ok(()),
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn scratch(name: &str) -> (tempfile::TempDir, StateDir) {
        let temp = tempfile::Builder::new().prefix(name).tempdir().unwrap();
        let state = StateDir::open(&temp.path().join("state")).unwrap();
        (temp, state)
    }
    fn identity() -> RootIdentity {
        #[cfg(unix)]
        {
            RootIdentity::Unix {
                device: 1,
                inode: 2,
            }
        }
        #[cfg(windows)]
        {
            RootIdentity::Windows {
                volume: 1,
                index: 2,
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            RootIdentity::Path {
                path: PathBuf::from("/synthetic"),
            }
        }
    }
    #[test]
    fn projects_round_trip_privately_and_absent_means_none() {
        let (_t, s) = scratch("precept-state-rt-");
        assert!(s.load_projects().unwrap().0.is_empty());
        let mut p = BTreeMap::new();
        p.insert("alpha".to_string(), PathBuf::from("/home/u/alpha"));
        let all = BTreeSet::from(["alpha".to_string()]);
        let switches = Switches {
            withheld: all.clone(),
            proposers: all.clone(),
            gated: all,
        };
        let identities = BTreeMap::from([("alpha".to_string(), identity())]);
        s.save_projects(&p, &identities, &switches).unwrap();
        assert_eq!(
            s.load_projects().unwrap(),
            (p.clone(), identities.clone(), switches)
        );
        s.save_projects(&p, &identities, &Switches::default())
            .unwrap();
        assert_eq!(s.load_projects().unwrap().2, Switches::default());
        let mode = std::fs::metadata(s.path().join(PROJECTS))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(
            std::fs::read_dir(s.path()).unwrap().count(),
            1,
            "no temporary left"
        );
        s.save_startup(Path::new("/home/u/profile.sqlite3"))
            .unwrap();
        assert_eq!(
            s.load_startup().unwrap().unwrap(),
            Path::new("/home/u/profile.sqlite3")
        );
        s.forget_startup().unwrap();
        s.forget_startup().unwrap();
        assert!(s.load_startup().unwrap().is_none());
    }
    #[test]
    fn malformed_shared_oversized_or_colliding_state_refuses() {
        let (_t, s) = scratch("precept-state-bad-");
        let put = |text: &str| {
            let _ = std::fs::remove_file(s.path().join(PROJECTS));
            let mut f = new_private(&s.path().join(PROJECTS)).unwrap();
            f.write_all(text.as_bytes()).unwrap();
        };
        for bad in [
            "not json",
            r#"{"format":2,"projects":[]}"#,
            r#"{"format":1,"projects":[],"extra":1}"#,
            r#"{"format":1,"projects":[{"name":"a","root":"relative"}]}"#,
            r#"{"format":1,"projects":[{"name":"../x","root":"/a"}]}"#,
            r#"{"format":1,"projects":[{"name":"a","root":"/a"},{"name":"A","root":"/b"}]}"#,
        ] {
            put(bad);
            assert!(s.load_projects().is_err(), "{bad}");
        }
        let many: Vec<String> = (0..=MAX_PROJECTS)
            .map(|i| format!(r#"{{"name":"p{i}","root":"/p{i}"}}"#))
            .collect();
        put(&format!(
            r#"{{"format":1,"projects":[{}]}}"#,
            many.join(",")
        ));
        assert_eq!(s.load_projects().err(), Some(Error::InvalidRequest));
        put(r#"{"format":1,"projects":[]}"#);
        std::fs::set_permissions(
            s.path().join(PROJECTS),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(s.load_projects().is_err(), "a shared file refuses");
        let mut big = BTreeMap::new();
        for i in 0..=MAX_PROJECTS {
            big.insert(format!("p{i}"), PathBuf::from(format!("/p{i}")));
        }
        assert_eq!(
            s.save_projects(&big, &BTreeMap::new(), &Switches::default())
                .err(),
            Some(Error::Capacity)
        );
    }
    #[test]
    fn the_state_directory_must_be_private_absolute_and_outside_git() {
        let temp = tempfile::tempdir().unwrap();
        assert!(StateDir::open(Path::new("relative/state")).is_err());
        std::fs::create_dir(temp.path().join("repo")).unwrap();
        std::fs::create_dir(temp.path().join("repo/.git")).unwrap();
        assert!(StateDir::open(&temp.path().join("repo/state")).is_err());
        std::fs::create_dir(temp.path().join("open")).unwrap();
        std::fs::set_permissions(
            temp.path().join("open"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        assert!(StateDir::open(&temp.path().join("open")).is_err());
    }
}
