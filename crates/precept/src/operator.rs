use clap::{Args, Subcommand};
use precept_ipc::{Error, Location, Result, operator::*};
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
            ("project", Some(name)) if precept_ipc::valid_project(name) => {
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
        /// Select this Profile again whenever the daemon starts.
        #[arg(long)]
        remember: bool,
    },
    Select {
        path: PathBuf,
        /// Select this Profile again whenever the daemon starts.
        #[arg(long)]
        remember: bool,
    },
    Info,
    /// Stop selecting a remembered Profile at start.
    ForgetStartup,
    /// Write every preference of the selected Profile to a new private file
    /// (versioned JSON; never overwrites). Handoffs are not included.
    Export {
        #[arg(long)]
        out: PathBuf,
        /// Also carry the served projects' memory (cautions, lessons,
        /// attempts).
        #[arg(long)]
        with_memory: bool,
        /// With --with-memory, carry this machine's environment notes too.
        #[arg(long, requires = "with_memory")]
        with_environment: bool,
    },
    /// Suggest every confirmed or pending preference of an export into the
    /// selected Profile (writes must be on). Each arrives as a suggestion to
    /// confirm again; a repeat with the same prefix replays, adding nothing.
    Import {
        file: PathBuf,
        /// Operation-id prefix (letters, digits, - and _); keep it to retry.
        #[arg(long)]
        operation_prefix: String,
    },
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
    /// Lowercase hex digest of the exact proposal the operator inspected.
    #[arg(long)]
    pub proposal_digest: String,
    #[arg(long)]
    pub confirm: bool,
    /// A note kept with the decision (confirm, reject, deprecate; at most 500
    /// characters).
    #[arg(long)]
    pub note: Option<String>,
}
#[derive(Args)]
pub struct ReviewQueueArgs {
    #[command(flatten)]
    pub scope: ScopeArgs,
    #[arg(long, default_value = "0")]
    pub after_id: i64,
}
#[derive(Args)]
pub struct ReviewInspectArgs {
    pub id: i64,
    #[command(flatten)]
    pub scope: ScopeArgs,
    #[arg(long)]
    pub revision: i64,
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
    /// One to ten tags (repeat the flag); normalized to lowercase.
    #[arg(long = "tag", required = true)]
    pub tags: Vec<String>,
    /// Who the proposal comes from: the operator, or an agent's suggestion the
    /// operator records. An asserted label; it verifies nothing by itself.
    #[arg(long, default_value = "operator", value_parser = ["operator", "agent"])]
    pub origin: String,
    /// Only for these agents (repeat); empty applies to every agent.
    #[arg(long = "agent")]
    pub agents: Vec<String>,
    /// Only for these languages (repeat).
    #[arg(long = "language")]
    pub languages: Vec<String>,
    /// Only for these repository paths (repeat; a trailing / covers a directory).
    #[arg(long = "path")]
    pub paths: Vec<String>,
    /// Only for tasks tagged with one of these (repeat).
    #[arg(long = "task-tag")]
    pub task_tags: Vec<String>,
    /// -100..100; higher wins among rules of one key and scope.
    #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
    pub priority: i64,
    #[arg(long, default_value = "advisory", value_parser = ["advisory", "verify", "hard"])]
    pub enforcement: String,
}
/// Switch a confirmed rule on or off (no new review; revision-checked).
#[derive(Args)]
pub struct Switch {
    pub id: i64,
    #[command(flatten)]
    pub scope: ScopeArgs,
    #[command(flatten)]
    pub authority: Authority,
    #[arg(long)]
    pub expected_revision: i64,
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
        Profile::Export {
            out,
            with_memory,
            with_environment,
        } => {
            use crate::transfer::MemoryChoice;
            let memory = match (with_memory, with_environment) {
                (false, _) => MemoryChoice::None,
                (true, false) => MemoryChoice::Notes,
                (true, true) => MemoryChoice::WithEnvironment,
            };
            let export = crate::transfer::collect(location, memory).await?;
            crate::transfer::write_new(&out, &export)?;
            return Ok(
                json!({"exported":export.preferences.len(),"memory":export.memory.len(),"format":crate::transfer::FORMAT,"version":crate::transfer::VERSION}),
            );
        }
        Profile::Import {
            file,
            operation_prefix,
        } => {
            let export = crate::transfer::read(&file)?;
            return crate::transfer::import(location, &export, &operation_prefix).await;
        }
        Profile::Create { path, remember } => {
            ("profile.create", json!({"path":path,"remember":remember}))
        }
        Profile::Select { path, remember } => {
            ("profile.select", json!({"path":path,"remember":remember}))
        }
        Profile::Info => ("profile.info", json!({})),
        Profile::ForgetStartup => ("profile.startup.forget", json!({})),
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
    precept_ipc::operator::control(location, method, params).await
}
/// The daemon's project set. `check` is a read; `add` and `remove` need the
/// current Profile's write grant (`profile write enable`).
#[derive(Subcommand)]
pub enum Project {
    /// Whether a directory may become a project, and its suggested name.
    Check { root: PathBuf },
    /// Add a project (a Git repository root inside your home).
    Add { name: String, root: PathBuf },
    /// Record a listed project in the selected Profile (one that is listed as
    /// not in this Profile, e.g. after creating a new Profile).
    Register { name: String },
    /// Remove a project from the set; its Profile records are kept.
    Remove { name: String },
    /// Let external agents (MCP) read this project's context again.
    Share { name: String },
    /// Stop external agents (MCP) from reading this project's context.
    Unshare { name: String },
    /// Whether this project's agents may suggest rules (`suggest_rule`); each
    /// suggestion still waits for your review. Off by default.
    Suggestions {
        name: String,
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },
    /// How an MCP client starts precept-mcp for this project (paths, no token).
    Mcp { name: String },
    /// The project's AGENTS.md pointer: its status and the exact block, or with
    /// --add / --remove a write guarded by the digest the status reported.
    Pointer {
        name: String,
        #[arg(long, conflicts_with = "remove")]
        add: bool,
        #[arg(long)]
        remove: bool,
        /// `sha256` from the status (omit when the file does not exist).
        #[arg(long)]
        expected_sha256: Option<String>,
    },
}
pub async fn project(location: &Location, command: Project) -> Result<Value> {
    match command {
        Project::Check { root } => {
            precept_ipc::operator::control(location, "project.check", json!({"root":root})).await
        }
        Project::Add { name, root } => {
            precept_ipc::operator::project_write(
                location,
                "project.add",
                json!({"name":name,"root":root}),
            )
            .await
        }
        Project::Register { name } => {
            precept_ipc::operator::project_write(location, "project.register", json!({"name":name}))
                .await
        }
        Project::Remove { name } => {
            precept_ipc::operator::project_write(location, "project.remove", json!({"name":name}))
                .await
        }
        Project::Share { name } => {
            precept_ipc::operator::control(
                location,
                "project.agents",
                json!({"name":name,"enabled":true}),
            )
            .await
        }
        Project::Suggestions { name, state } => {
            precept_ipc::operator::control(
                location,
                "project.propose",
                json!({"name":name,"enabled":state == "on"}),
            )
            .await
        }
        Project::Unshare { name } => {
            precept_ipc::operator::control(
                location,
                "project.agents",
                json!({"name":name,"enabled":false}),
            )
            .await
        }
        Project::Pointer {
            name,
            add,
            remove,
            expected_sha256,
        } => {
            if !add && !remove {
                return precept_ipc::operator::control(
                    location,
                    "integration.pointer",
                    json!({"project":name}),
                )
                .await;
            }
            precept_ipc::operator::control(
                location,
                "integration.pointer.apply",
                json!({"project":name,"enabled":add,"expected_sha256":expected_sha256}),
            )
            .await
        }
        Project::Mcp { name } => {
            precept_ipc::operator::control(location, "project.mcp", json!({"name":name})).await
        }
    }
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
    precept_ipc::operator::control(location, method, params).await
}
pub async fn review_queue(location: &Location, p: ReviewQueueArgs) -> Result<Value> {
    let params = json!({"scope":p.scope.scope()?,"after_id":p.after_id});
    precept_ipc::operator::control(location, "profile.review.queue", params).await
}
pub async fn review_inspect(location: &Location, p: ReviewInspectArgs) -> Result<Value> {
    let params = json!({"scope":p.scope.scope()?,"id":p.id,"revision":p.revision});
    precept_ipc::operator::control(location, "profile.review.inspect", params).await
}
async fn submit(location: &Location, a: Authority, command: Command) -> Result<Value> {
    precept_ipc::operator::mutate(
        location,
        &Mutation {
            version: 2,
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
    let origin = Origin::parse(&p.origin).ok_or(Error::InvalidRequest)?;
    let tags = precept_ipc::operator::canonical_tags(&p.tags).ok_or(Error::InvalidRequest)?;
    let rule = RuleOptions {
        applies_to: AppliesTo {
            agents: p.agents,
            languages: p.languages,
            paths: p.paths,
            task_tags: p.task_tags,
        }
        .canonical()
        .ok_or(Error::InvalidRequest)?,
        priority: p.priority,
        enforcement: Enforcement::parse(&p.enforcement).ok_or(Error::InvalidRequest)?,
    };
    if !rule.valid() {
        return Err(Error::InvalidRequest);
    }
    submit(
        location,
        p.authority,
        Command::SuggestPreference {
            rule,
            scope,
            key: p.key,
            value: p.value,
            tags,
            origin,
            expected: Expected::Absent,
        },
    )
    .await
}
pub async fn switch(location: &Location, p: Switch, enabled: bool) -> Result<Value> {
    if p.id <= 0 || p.expected_revision <= 0 {
        return Err(Error::InvalidRequest);
    }
    let command = Command::SetPreferenceEnabled {
        id: p.id,
        scope: p.scope.scope()?,
        enabled,
        channel: Channel::Cli,
        expected: Expected::Revision(p.expected_revision),
    };
    submit(location, p.authority, command).await
}
pub async fn review(location: &Location, p: Review, action: &str) -> Result<Value> {
    if !p.confirm || p.id <= 0 || p.expected_revision <= 0 {
        return Err(Error::InvalidRequest);
    }
    let scope = p.scope.scope()?;
    let expected = Expected::Revision(p.expected_revision);
    let id = p.id;
    let proposal_digest = p.proposal_digest;
    let channel = Channel::Cli;
    let note = canonical_note(p.note.as_deref()).ok_or(Error::InvalidRequest)?;
    let command = match action {
        "confirm" => Command::ConfirmPreference {
            id,
            scope,
            proposal_digest,
            channel,
            expected,
            note: note.clone(),
        },
        "reject" => Command::RejectPreference {
            id,
            scope,
            proposal_digest,
            channel,
            expected,
            note: note.clone(),
        },
        "deprecate" => Command::DeprecatePreference {
            id,
            scope,
            proposal_digest,
            channel,
            expected,
            note: note.clone(),
        },
        "delete" if note.is_some() => return Err(Error::InvalidRequest),
        "delete" => Command::DeletePreference {
            id,
            scope,
            proposal_digest,
            channel,
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
