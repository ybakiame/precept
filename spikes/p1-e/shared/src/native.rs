//! Trusted operator UI client using unchanged authenticated native IPC.
use super::*;
use acp_ipc::{Access, Client, Location, load_credential};
use clap::Parser;
use std::path::PathBuf;

#[derive(Clone, Parser, Default)]
pub struct Inputs {
    /// Explicit private runtime; omit both location flags for offline synthetic data.
    #[arg(long, requires = "endpoint")]
    pub runtime_dir: Option<PathBuf>,
    #[arg(long, requires = "runtime_dir")]
    pub endpoint: Option<String>,
}
impl Inputs {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
    pub fn fetch(&self) -> Result<Snapshot, &'static str> {
        let Some(runtime) = &self.runtime_dir else {
            return Ok(Snapshot::fixture());
        };
        let location = Location {
            runtime_dir: runtime.clone(),
            endpoint: self.endpoint.clone().ok_or("Explicit endpoint required")?,
        };
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Client unavailable")?
            .block_on(fetch(&location))
            .map_err(|_| "Local daemon unavailable or access refused")
    }
}
async fn fetch(location: &Location) -> acp_ipc::Result<Snapshot> {
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
    // One fresh bounded connection per project avoids the protocol's 32-request lease.
    let mut projects = Vec::new();
    for Name { name } in names.into_iter().take(64) {
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
        let handoff = match client
            .request("handoff.get", json!({"project":name,"task_id":"shared"}))
            .await
        {
            Ok(v) => Some(serde_json::from_value(v).map_err(|_| Error::Unavailable)?),
            Err(Error::NotFound) => None,
            Err(e) => return Err(e),
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
