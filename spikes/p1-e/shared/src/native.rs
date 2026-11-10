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
    #[arg(long, requires_all = ["wsl_user", "wsl_snapshot", "wsl_runtime_dir", "wsl_endpoint"], conflicts_with_all = ["runtime_dir", "endpoint", "synthetic_rows"])]
    pub wsl_distribution: Option<String>,
    #[arg(long, requires = "wsl_distribution")]
    pub wsl_user: Option<String>,
    /// Absolute Linux path of the existing acp-ui-snapshot executable.
    #[arg(long, requires = "wsl_distribution")]
    pub wsl_snapshot: Option<String>,
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
            helper: self.wsl_snapshot.clone().unwrap_or_default(),
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
                wsl_snapshot: Some(c.helper.clone()),
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
            .block_on(fetch(&location, project, task))
            .map_err(|e| match e {
                acp_ipc::Error::Unauthorized | acp_ipc::Error::UnsafeRuntime => "Permission denied",
                _ => "Local daemon unavailable or access refused",
            })
    }
    fn wsl_arguments(
        &self,
        project: Option<&str>,
        task: &str,
    ) -> Result<Vec<String>, &'static str> {
        let distro = self
            .wsl_distribution
            .as_deref()
            .ok_or("Explicit WSL distribution required")?;
        let user = self
            .wsl_user
            .as_deref()
            .ok_or("Explicit WSL owner required")?;
        let snapshot = self
            .wsl_snapshot
            .as_deref()
            .ok_or("Explicit WSL helper required")?;
        let runtime = self
            .wsl_runtime_dir
            .as_deref()
            .ok_or("Explicit WSL runtime required")?;
        let endpoint = self
            .wsl_endpoint
            .as_deref()
            .ok_or("Explicit WSL endpoint required")?;
        if !distro
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
            || distro.len() > 64
            || !distro
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_. ".contains(&c))
            || !user
                .as_bytes()
                .first()
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
            || user.len() > 64
            || !user
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
            || [snapshot, runtime, endpoint].iter().any(|p| {
                !p.starts_with('/')
                    || p.chars().any(char::is_control)
                    || p.split('/').any(|s| s == "..")
            })
        {
            return Err("Invalid WSL connection inputs");
        }
        let mut args = vec![
            "--distribution".into(),
            distro.into(),
            "--user".into(),
            user.into(),
            "--exec".into(),
            snapshot.into(),
            "--runtime-dir".into(),
            runtime.into(),
            "--endpoint".into(),
            endpoint.into(),
        ];
        if let Some(project) = project {
            args.push(format!("--project={project}"));
        }
        if !task.is_empty() {
            args.push(format!("--task-id={task}"));
        }
        Ok(args)
    }
    #[cfg(not(target_os = "windows"))]
    fn fetch_wsl(&self, project: Option<&str>, task: &str) -> Result<Snapshot, &'static str> {
        self.wsl_arguments(project, task)?;
        Err("Windows WSL owner transport unavailable on this OS")
    }
    #[cfg(target_os = "windows")]
    fn fetch_wsl(&self, project: Option<&str>, task: &str) -> Result<Snapshot, &'static str> {
        use tokio::io::AsyncReadExt;
        let args = self.wsl_arguments(project, task)?;
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Client unavailable")?
            .block_on(async {
                let executable = std::env::var_os("SystemRoot")
                    .map(PathBuf::from)
                    .ok_or("WSL executable unavailable")?
                    .join("System32/wsl.exe");
                let mut cmd = tokio::process::Command::new(executable);
                cmd.args(args)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .kill_on_drop(true);
                // Native desktop reconnect must not create a Windows console window.
                cmd.creation_flags(0x08000000);
                let mut child = cmd.spawn().map_err(|_| "WSL query unavailable")?;
                let result = tokio::time::timeout(std::time::Duration::from_secs(20), async {
                    let mut raw = Vec::new();
                    child
                        .stdout
                        .take()
                        .ok_or("WSL query unavailable")?
                        .take(4 * 1024 * 1024 + 1)
                        .read_to_end(&mut raw)
                        .await
                        .map_err(|_| "WSL query unavailable")?;
                    if raw.len() > 4 * 1024 * 1024 {
                        return Err("WSL response too large");
                    }
                    let status = child.wait().await.map_err(|_| "WSL query unavailable")?;
                    if !status.success() {
                        return Err(helper_failure(status.code()));
                    }
                    let data: Snapshot =
                        serde_json::from_slice(&raw).map_err(|_| "Invalid WSL snapshot")?;
                    data.validate()?;
                    if project.is_some_and(|p| !data.projects.iter().any(|v| v.name == p)) {
                        return Err("Permission denied");
                    }
                    Ok(data)
                })
                .await
                .unwrap_or(Err("WSL query timed out"));
                if result.is_err() {
                    let _ = child.kill().await;
                }
                result
            })
    }
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

async fn fetch(
    location: &Location,
    selected: Option<&str>,
    task: &str,
) -> acp_ipc::Result<Snapshot> {
    use acp_ipc::Error;
    use serde_json::json;
    let credential = load_credential(location, &location.credential_path(None)?)?;
    if credential.access != Access::Operator {
        return Err(Error::Unauthorized);
    }
    let mut client = Client::connect(location, &credential).await?;
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
        let mut client = Client::connect(location, &credential).await?;
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
            wsl_snapshot: Some("/opt/acp-ui-snapshot".into()),
            wsl_runtime_dir: Some("/tmp/private".into()),
            wsl_endpoint: Some("/tmp/private/control.sock".into()),
            ..Default::default()
        };
        assert!(base.is_live());
        let args = base
            .wsl_arguments(Some("alpha"), "task 'quoted' ; $(never-run)")
            .unwrap();
        assert_eq!(args[4], "--exec");
        assert_eq!(
            args.last().unwrap(),
            "--task-id=task 'quoted' ; $(never-run)"
        );
        assert!(
            !args
                .iter()
                .any(|v| ["bash", "sh", "-c"].contains(&v.as_str()))
        );
        for invalid in ["../other", "bad\nname", "--system"] {
            let mut bad = base.clone();
            bad.wsl_distribution = Some(invalid.into());
            assert!(bad.wsl_arguments(None, "").is_err());
        }
        assert!(
            Inputs {
                wsl_snapshot: Some("/opt/../secret".into()),
                ..base.clone()
            }
            .wsl_arguments(None, "")
            .is_err()
        );
    }
}
