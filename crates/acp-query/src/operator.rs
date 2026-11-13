use acp_ipc::{Error, Location, Result, operator::*};
use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io::Read, path::PathBuf};

#[derive(Args)]
pub struct Authority {
    #[arg(long)]
    pub profile_id: String,
    #[arg(long)]
    pub generation: String,
    #[arg(long)]
    pub owner_epoch: i64,
    /// Preserve this ID and all inputs for an explicit same-epoch retry.
    #[arg(long)]
    pub operation_id: String,
}
#[derive(Args)]
pub struct ScopeArgs {
    #[arg(long, value_parser=["global","project"])]
    pub scope: String,
    #[arg(long)]
    pub project: Option<String>,
}
impl ScopeArgs {
    fn scope(&self) -> Result<Scope> {
        match (self.scope.as_str(), &self.project) {
            ("global", None) => Ok(Scope::Global),
            ("project", Some(name)) if acp_ipc::valid_project(name) => {
                Ok(Scope::Project { name: name.clone() })
            }
            _ => Err(Error::InvalidRequest),
        }
    }
}
#[derive(Subcommand)]
pub enum Profile {
    Create {
        path: PathBuf,
    },
    Select {
        path: PathBuf,
    },
    Info,
    Write {
        #[command(subcommand)]
        command: Write,
    },
}
#[derive(Subcommand)]
pub enum Write {
    Enable {
        #[arg(long)]
        profile_id: String,
        #[arg(long)]
        generation: String,
        #[arg(long)]
        confirm: bool,
    },
    Disable,
}
#[derive(Subcommand)]
pub enum Preferences {
    List {
        #[command(flatten)]
        scope: ScopeArgs,
        #[arg(long, default_value = "0")]
        after_id: i64,
    },
    Get {
        id: i64,
        #[command(flatten)]
        scope: ScopeArgs,
    },
}
#[derive(Args)]
pub struct Review {
    pub id: i64,
    #[command(flatten)]
    pub scope: ScopeArgs,
    #[command(flatten)]
    pub authority: Authority,
    #[arg(long)]
    pub expected_revision: i64,
    #[arg(long)]
    pub confirm: bool,
}
#[derive(Args)]
pub struct Suggest {
    #[command(flatten)]
    pub scope: ScopeArgs,
    #[command(flatten)]
    pub authority: Authority,
    #[arg(long)]
    pub key: String,
    #[arg(long)]
    pub value: String,
    #[arg(long)]
    pub source: String,
}
#[derive(Args)]
pub struct Register {
    pub name: String,
    pub root: String,
    #[command(flatten)]
    pub authority: Authority,
}
#[derive(Args)]
pub struct Put {
    pub project: String,
    pub task: String,
    #[command(flatten)]
    pub authority: Authority,
    #[arg(
        long,
        required_unless_present = "expected_revision",
        conflicts_with = "expected_revision"
    )]
    pub expected_absent: bool,
    #[arg(long)]
    pub expected_revision: Option<i64>,
    #[arg(long)]
    pub input: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HandoffInput {
    objective: String,
    completed: String,
    next_action: String,
    validation_json: String,
    snapshot: Snapshot,
}
pub async fn profile(location: &Location, command: Profile) -> Result<Value> {
    let (method, params) = match command {
        Profile::Create { path } => ("profile.create", json!({"path":path})),
        Profile::Select { path } => ("profile.select", json!({"path":path})),
        Profile::Info => ("profile.info", json!({})),
        Profile::Write {
            command:
                Write::Enable {
                    profile_id,
                    generation,
                    confirm,
                },
        } => (
            "profile.write.enable",
            json!({"profile_id":profile_id,"generation":generation,"confirm":confirm}),
        ),
        Profile::Write {
            command: Write::Disable,
        } => ("profile.write.disable", json!({})),
    };
    acp_ipc::operator::control(location, method, params).await
}
pub async fn preferences(location: &Location, command: Preferences) -> Result<Value> {
    let (method, params) = match command {
        Preferences::List { scope, after_id } => (
            "profile.preferences.list",
            json!({"scope":scope.scope()?,"after_id":after_id}),
        ),
        Preferences::Get { id, scope } => (
            "profile.preferences.get",
            json!({"scope":scope.scope()?,"id":id}),
        ),
    };
    acp_ipc::operator::control(location, method, params).await
}
async fn submit(location: &Location, a: Authority, command: Command) -> Result<Value> {
    acp_ipc::operator::mutate(
        location,
        &Mutation {
            version: 1,
            binding: Binding {
                profile_id: a.profile_id,
                generation: a.generation,
                owner_epoch: a.owner_epoch,
            },
            operation_id: a.operation_id,
            command,
        },
    )
    .await
}
pub async fn register(location: &Location, p: Register) -> Result<Value> {
    submit(
        location,
        p.authority,
        Command::RegisterProject {
            name: p.name,
            root: p.root,
            expected: Expected::Absent,
        },
    )
    .await
}
pub async fn suggest(location: &Location, p: Suggest) -> Result<Value> {
    let scope = p.scope.scope()?;
    submit(
        location,
        p.authority,
        Command::SuggestPreference {
            scope,
            key: p.key,
            value: p.value,
            source: p.source,
            expected: Expected::Absent,
        },
    )
    .await
}
pub async fn review(location: &Location, p: Review, action: &str) -> Result<Value> {
    if !p.confirm || p.id <= 0 || p.expected_revision <= 0 {
        return Err(Error::InvalidRequest);
    }
    let scope = p.scope.scope()?;
    let expected = Expected::Revision(p.expected_revision);
    let id = p.id;
    let command = match action {
        "confirm" => Command::ConfirmPreference {
            id,
            scope,
            expected,
        },
        "reject" => Command::RejectPreference {
            id,
            scope,
            expected,
        },
        "deprecate" => Command::DeprecatePreference {
            id,
            scope,
            expected,
        },
        "delete" => Command::DeletePreference {
            id,
            scope,
            expected,
        },
        _ => return Err(Error::InvalidRequest),
    };
    submit(location, p.authority, command).await
}
pub async fn put(location: &Location, p: Put) -> Result<Value> {
    // Bounded typed input; never a command envelope or arbitrary JSON patch.
    let file = std::fs::File::open(&p.input).map_err(|_| Error::InvalidRequest)?;
    if !file
        .metadata()
        .map_err(|_| Error::InvalidRequest)?
        .is_file()
    {
        return Err(Error::InvalidRequest);
    }
    let mut raw = Vec::new();
    file.take(128 * 1024 + 1)
        .read_to_end(&mut raw)
        .map_err(|_| Error::InvalidRequest)?;
    if raw.len() > 128 * 1024 {
        return Err(Error::TooLarge);
    }
    let input: HandoffInput = serde_json::from_slice(&raw).map_err(|_| Error::InvalidRequest)?;
    let expected = if p.expected_absent {
        Expected::Absent
    } else {
        Expected::Revision(p.expected_revision.ok_or(Error::InvalidRequest)?)
    };
    submit(
        location,
        p.authority,
        Command::PutHandoff {
            project: p.project,
            task_id: p.task,
            objective: input.objective,
            completed: input.completed,
            next_action: input.next_action,
            validation_json: input.validation_json,
            snapshot: input.snapshot,
            expected,
        },
    )
    .await
}
