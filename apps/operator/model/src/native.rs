//! Trusted operator UI client using unchanged authenticated native IPC.
use super::*;
use acp_ipc::{Access, Client, Location, load_credential};
use clap::Parser;
use std::path::PathBuf;

#[derive(Clone, Parser, Default)]
pub struct Inputs {
    /// UI locale only; never translates stored values or protocol evidence.
    #[arg(long, value_parser=["en","zh-CN"], default_value="en")]
    pub locale: String,
    #[arg(long, value_parser=["system","light","dark"], default_value="dark")]
    pub appearance: String,
    #[arg(long, value_parser=["instrument","ledger","harbor"], default_value="harbor")]
    pub theme: String,
    #[arg(long, value_parser=["dashboard","projects","preferences","handoff","diagnostics"], default_value="dashboard")]
    pub section: String,
    #[arg(long)]
    pub settings: bool,
    #[arg(long)]
    pub connection_settings: bool,
    #[arg(long)]
    pub sessions: bool,
    #[arg(long)]
    pub operator: bool,
    /// Explicit private runtime; omit both location flags for offline synthetic data.
    #[arg(long, requires = "endpoint")]
    pub runtime_dir: Option<PathBuf>,
    #[arg(long, requires = "runtime_dir")]
    pub endpoint: Option<String>,
    /// Initial project; must appear in the daemon's exact operator allowlist.
    #[arg(long)]
    pub project: Option<String>,
    /// Explicit handoff lookup. Omit to show honest empty/unqueried state.
    #[arg(long)]
    pub task_id: Option<String>,
    /// Offline synthetic renderer workload; never combines with native IPC.
    #[arg(long, conflicts_with = "runtime_dir", value_parser = clap::value_parser!(u32).range(1..=10000))]
    pub synthetic_rows: Option<u32>,
    /// Windows owner access to an explicitly selected WSL distribution. No shell or TCP.
    #[arg(long, requires_all = ["wsl_user", "wsl_bridge", "wsl_runtime_dir", "wsl_endpoint"], conflicts_with_all = ["runtime_dir", "endpoint", "synthetic_rows"])]
    pub wsl_distribution: Option<String>,
    #[arg(long, requires = "wsl_distribution")]
    pub wsl_user: Option<String>,
    /// Absolute Linux path of the acp-wsl-bridge executable inside the distribution.
    #[arg(long, requires = "wsl_distribution")]
    pub wsl_bridge: Option<String>,
    #[arg(long, requires = "wsl_distribution")]
    pub wsl_runtime_dir: Option<String>,
    #[arg(long, requires = "wsl_distribution")]
    pub wsl_endpoint: Option<String>,
}
impl Inputs {
    pub fn connection(&self) -> crate::connection::Connection {
        use crate::connection::{Connection, Target};
        Connection {
            target: if self.wsl_distribution.is_some() {
                Target::Wsl
            } else if self.runtime_dir.is_some() {
                Target::Local
            } else {
                Target::Synthetic
            },
            runtime: self
                .runtime_dir
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            endpoint: self.endpoint.clone().unwrap_or_default(),
            distribution: self.wsl_distribution.clone().unwrap_or_default(),
            user: self.wsl_user.clone().unwrap_or_default(),
            helper: self.wsl_bridge.clone().unwrap_or_default(),
            wsl_runtime: self.wsl_runtime_dir.clone().unwrap_or_default(),
            wsl_endpoint: self.wsl_endpoint.clone().unwrap_or_default(),
            synthetic_rows: self.synthetic_rows,
        }
    }
    pub fn from_connection(c: &crate::connection::Connection) -> Result<Self, &'static str> {
        use crate::connection::Target;
        if !c.validate() {
            return Err("Invalid connection inputs");
        }
        Ok(match c.target {
            Target::Synthetic => Self {
                synthetic_rows: c.synthetic_rows,
                ..Default::default()
            },
            Target::Local => Self {
                runtime_dir: Some(c.runtime.clone().into()),
                endpoint: Some(c.endpoint.clone()),
                ..Default::default()
            },
            Target::Wsl => Self {
                wsl_distribution: Some(c.distribution.clone()),
                wsl_user: Some(c.user.clone()),
                wsl_bridge: Some(c.helper.clone()),
                wsl_runtime_dir: Some(c.wsl_runtime.clone()),
                wsl_endpoint: Some(c.wsl_endpoint.clone()),
                ..Default::default()
            },
        })
    }
    pub fn is_live(&self) -> bool {
        self.runtime_dir.is_some() || self.wsl_distribution.is_some()
    }
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
    pub fn fetch(&self) -> Result<Snapshot, &'static str> {
        self.fetch_for(
            self.project.as_deref(),
            self.task_id.as_deref().unwrap_or(""),
        )
    }
    pub fn fetch_for(&self, project: Option<&str>, task: &str) -> Result<Snapshot, &'static str> {
        if !task.is_empty() && !valid_task(task) {
            return Err("Invalid task ID");
        }
        if project.is_some_and(|p| !acp_ipc::valid_project(p)) {
            return Err("Permission denied");
        }
        if self.wsl_distribution.is_some() {
            return self.fetch_wsl(project, task);
        }
        let Some(runtime) = &self.runtime_dir else {
            let mut data = Snapshot::fixture();
            if let Some(count) = self.synthetic_rows {
                let row = data.projects[0].context.preferences[0].clone();
                data.projects[0].context.preferences = (0..count)
                    .map(|i| Preference {
                        key: format!("event-{i:05}"),
                        ..row.clone()
                    })
                    .collect();
            }
            for p in &mut data.projects {
                if p.handoff.as_ref().is_some_and(|h| h.task_id != task) {
                    p.handoff = None;
                }
            }
            if project.is_some_and(|p| !data.projects.iter().any(|v| v.name == p)) {
                return Err("Permission denied");
            }
            return Ok(data);
        };
        let location = Location {
            runtime_dir: runtime.clone(),
            endpoint: self.endpoint.clone().ok_or("Explicit endpoint required")?,
        };
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Client unavailable")?
            .block_on(snapshot_at(&location, project, task))
    }
    /// Validated identity of the explicitly selected WSL connection.
    pub(crate) fn wsl_key(&self) -> Result<crate::bridge::Key, &'static str> {
        let field = |v: &Option<String>, m: &'static str| v.clone().ok_or(m);
        let key = crate::bridge::Key {
            distribution: field(&self.wsl_distribution, "Explicit WSL distribution required")?,
            user: field(&self.wsl_user, "Explicit WSL owner required")?,
            helper: field(&self.wsl_bridge, "Explicit WSL helper required")?,
            runtime: field(&self.wsl_runtime_dir, "Explicit WSL runtime required")?,
            endpoint: field(&self.wsl_endpoint, "Explicit WSL endpoint required")?,
        };
        let valid = crate::connection::Connection {
            target: crate::connection::Target::Wsl,
            distribution: key.distribution.clone(),
            user: key.user.clone(),
            helper: key.helper.clone(),
            wsl_runtime: key.runtime.clone(),
            wsl_endpoint: key.endpoint.clone(),
            ..Default::default()
        }
        .validate();
        if !valid || key.distribution.starts_with('-') {
            return Err("Invalid WSL connection inputs");
        }
        Ok(key)
    }
    /// Exact `wsl.exe` argument vector: no shell, no interpolation, no secrets.
    pub(crate) fn wsl_arguments(&self) -> Result<Vec<String>, &'static str> {
        let k = self.wsl_key()?;
        Ok(vec![
            "--distribution".into(),
            k.distribution,
            "--user".into(),
            k.user,
            "--exec".into(),
            k.helper,
            "--runtime-dir".into(),
            k.runtime,
            "--endpoint".into(),
            k.endpoint,
        ])
    }
    /// The live WSL bridge. `start` is true only for explicit user reads.
    pub(crate) fn wsl_bridge(&self, start: bool) -> Result<crate::bridge::Client, &'static str> {
        let key = self.wsl_key()?;
        crate::bridge::bridge(&key, start, || self.wsl_command())
    }
    #[cfg(target_os = "windows")]
    fn wsl_command(&self) -> Result<std::process::Command, &'static str> {
        use std::os::windows::process::CommandExt;
        let executable = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .ok_or("WSL executable unavailable")?
            .join("System32/wsl.exe");
        let mut command = std::process::Command::new(executable);
        command.args(self.wsl_arguments()?);
        // The native desktop must not create a Windows console window.
        command.creation_flags(0x08000000);
        Ok(command)
    }
    #[cfg(not(target_os = "windows"))]
    fn wsl_command(&self) -> Result<std::process::Command, &'static str> {
        self.wsl_arguments()?;
        Err("Windows WSL owner transport unavailable on this OS")
    }
    fn fetch_wsl(&self, project: Option<&str>, task: &str) -> Result<Snapshot, &'static str> {
        use crate::bridge::{CallError, Op};
        let value = self
            .wsl_bridge(true)?
            .call(
                Op::Snapshot {
                    project: project.map(str::to_owned),
                    task: task.to_owned(),
                },
                std::time::Duration::from_secs(20),
            )
            .map_err(|e| match e {
                CallError::Remote("Permission denied") => "Permission denied",
                CallError::Timeout => "WSL query timed out",
                _ => "WSL daemon unavailable or access refused",
            })?;
        let data: Snapshot = serde_json::from_value(value).map_err(|_| "Invalid WSL snapshot")?;
        data.validate()?;
        if project.is_some_and(|p| !data.projects.iter().any(|v| v.name == p)) {
            return Err("Permission denied");
        }
        Ok(data)
    }
}

/// Shared snapshot read used by the local client and the WSL bridge helper.
pub(crate) async fn snapshot_at(
    location: &Location,
    project: Option<&str>,
    task: &str,
) -> Result<Snapshot, &'static str> {
    fetch(location, project, task).await.map_err(|e| match e {
        acp_ipc::Error::Unauthorized | acp_ipc::Error::UnsafeRuntime => "Permission denied",
        _ => "Local daemon unavailable or access refused",
    })
}

/// Stable helper exit classification; never forwards stderr or private paths.
pub const HELPER_ACCESS_REFUSED: i32 = 3;
pub fn helper_failure(code: Option<i32>) -> &'static str {
    if code == Some(HELPER_ACCESS_REFUSED) {
        "Permission denied"
    } else {
        "WSL daemon unavailable or access refused"
    }
}

#[cfg(unix)]
fn missing_unix_endpoint(location: &Location) -> bool {
    let path = std::path::Path::new(&location.endpoint);
    let Ok(root) = location.root() else {
        return false;
    };
    path.is_absolute()
        && path.parent().and_then(|p| p.canonicalize().ok()).as_ref() == Some(&root)
        && std::fs::symlink_metadata(path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
}

async fn fetch(
    location: &Location,
    selected: Option<&str>,
    task: &str,
) -> acp_ipc::Result<Snapshot> {
    use acp_ipc::Error;
    use serde_json::json;
    let path = location.credential_path(None).map_err(|error| {
        if std::fs::symlink_metadata(&location.runtime_dir)
            .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        {
            Error::Unavailable
        } else {
            error
        }
    })?;
    let credential = load_credential(location, &path).map_err(|error| {
        // Normal daemon shutdown removes its capability. Absence alone does
        // not establish an authorization refusal; retain an unavailable state.
        if std::fs::symlink_metadata(&path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        {
            Error::Unavailable
        } else {
            error
        }
    })?;
    if credential.access != Access::Operator {
        return Err(Error::Unauthorized);
    }
    let connected = Client::connect(location, &credential).await;
    #[cfg(unix)]
    let connected = connected.map_err(|error| {
        // IPC deliberately treats absent and unsafe Unix sockets alike. Only
        // refine the UI failure after the guarded connect has already refused;
        // an observed missing socket in the validated runtime is unavailable.
        if matches!(error, Error::UnsafeRuntime) && missing_unix_endpoint(location) {
            return Error::Unavailable;
        }
        error
    });
    let mut client = connected?;
    let health = serde_json::from_value(client.request("daemon.health", json!({})).await?)
        .map_err(|_| Error::Unavailable)?;
    #[derive(Deserialize)]
    struct Name {
        name: String,
    }
    let names: Vec<Name> = serde_json::from_value(client.request("project.list", json!({})).await?)
        .map_err(|_| Error::Unavailable)?;
    if names.len() > 64 || selected.is_some_and(|p| !names.iter().any(|n| n.name == p)) {
        return Err(Error::Unauthorized);
    }
    let selected = selected
        .or_else(|| names.first().map(|n| n.name.as_str()))
        .map(str::to_owned);
    // Only the selected project is queried. Other names are navigation entries.
    let mut projects = Vec::new();
    for Name { name } in names {
        if selected.as_deref() != Some(name.as_str()) {
            projects.push(Project {
                context: Context {
                    project: name.clone(),
                    preferences: Vec::new(),
                    repository: Repository::default(),
                    documents: Vec::new(),
                    warnings: Vec::new(),
                },
                name,
                handoff: None,
            });
            continue;
        }
        let connected = Client::connect(location, &credential).await;
        #[cfg(unix)]
        let connected = connected.map_err(|error| {
            // IPC deliberately treats absent and unsafe Unix sockets alike. Only
            // refine the UI failure after the guarded connect has already refused;
            // an observed missing socket in the validated runtime is unavailable.
            if matches!(error, Error::UnsafeRuntime) && missing_unix_endpoint(location) {
                return Error::Unavailable;
            }
            error
        });
        let mut client = connected?;
        let context = serde_json::from_value(
            client
                .request(
                    "context.get",
                    json!({"project":name,"include_documents":false}),
                )
                .await?,
        )
        .map_err(|_| Error::Unavailable)?;
        let handoff = if task.is_empty() {
            None
        } else {
            match client
                .request("handoff.get", json!({"project":name,"task_id":task}))
                .await
            {
                Ok(v) => Some(serde_json::from_value(v).map_err(|_| Error::Unavailable)?),
                Err(Error::NotFound) => None,
                Err(e) => return Err(e),
            }
        };
        projects.push(Project {
            name,
            context,
            handoff,
        });
    }
    let data = Snapshot { health, projects };
    data.validate().map_err(|_| Error::Unavailable)?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helper_exit_codes_preserve_denial_without_forwarding_stderr() {
        assert_eq!(
            helper_failure(Some(HELPER_ACCESS_REFUSED)),
            "Permission denied"
        );
        for code in [None, Some(1), Some(2), Some(127)] {
            assert_eq!(
                helper_failure(code),
                "WSL daemon unavailable or access refused"
            );
        }
    }
    #[test]
    fn wsl_is_explicit_shell_free_and_never_a_fixture_fallback() {
        use clap::Parser;
        assert!(Inputs::try_parse_from(["ui", "--wsl-distribution", "synthetic"]).is_err());
        let base = Inputs {
            wsl_distribution: Some("synthetic".into()),
            wsl_user: Some("operator".into()),
            wsl_bridge: Some("/opt/acp/bin/acp-wsl-bridge".into()),
            wsl_runtime_dir: Some("/tmp/private".into()),
            wsl_endpoint: Some("/tmp/private/control.sock".into()),
            ..Default::default()
        };
        assert!(base.is_live());
        let args = base.wsl_arguments().unwrap();
        assert_eq!(
            args,
            [
                "--distribution",
                "synthetic",
                "--user",
                "operator",
                "--exec",
                "/opt/acp/bin/acp-wsl-bridge",
                "--runtime-dir",
                "/tmp/private",
                "--endpoint",
                "/tmp/private/control.sock"
            ]
        );
        // Project/task never become helper argv; they travel as typed frames.
        assert!(
            !args
                .iter()
                .any(|v| ["bash", "sh", "-c"].contains(&v.as_str()))
        );
        for invalid in ["../other", "bad\nname", "--system", "-d"] {
            let mut bad = base.clone();
            bad.wsl_distribution = Some(invalid.into());
            assert!(bad.wsl_arguments().is_err(), "{invalid}");
        }
        for (helper, runtime) in [
            ("/opt/../secret", "/tmp/p"),
            ("relative", "/tmp/p"),
            ("/opt/b", "tmp"),
        ] {
            assert!(
                Inputs {
                    wsl_bridge: Some(helper.into()),
                    wsl_runtime_dir: Some(runtime.into()),
                    ..base.clone()
                }
                .wsl_arguments()
                .is_err()
            );
        }
        // Without an explicit user read no bridge is ever started.
        assert_eq!(
            base.wsl_bridge(false).err(),
            Some("WSL bridge disconnected; reconnect explicitly")
        );
    }
}
