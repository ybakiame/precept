mod operator;
use clap::{Parser, Subcommand};
use precept_ipc::{Access, Client, Error, Location, load_credential};
use serde_json::json;
#[derive(Parser)]
struct Args {
    #[command(flatten)]
    location: Location,
    #[command(subcommand)]
    command: Query,
}
#[derive(Subcommand)]
enum Query {
    Profile {
        #[command(subcommand)]
        command: operator::Profile,
    },
    Preferences {
        #[command(subcommand)]
        command: operator::Preferences,
    },
    Project {
        #[command(subcommand)]
        command: operator::Project,
    },
    HandoffRecord {
        project: String,
        task: String,
    },
    /// Pending suggested preferences only, with a commit-sequence watermark.
    ReviewQueue(operator::ReviewQueueArgs),
    /// Exact record, bounded decision history and projected peer demotions.
    ReviewInspect(operator::ReviewInspectArgs),
    ProjectRegister(operator::Register),
    PreferenceSuggest(operator::Suggest),
    PreferenceConfirm(operator::Review),
    PreferenceReject(operator::Review),
    PreferenceDeprecate(operator::Review),
    PreferenceDelete(operator::Review),
    HandoffPut(operator::Put),
    Health,
    Projects,
    /// Recent agent activity (in memory since the daemon started; newest
    /// first): MCP starts, reads and refusals, by count only.
    Activity {
        /// Only this project (refused handshakes, which carry none, included).
        #[arg(long)]
        project: Option<String>,
        /// Only events older than this id.
        #[arg(long)]
        before: Option<u64>,
    },
    Context {
        project: String,
        #[arg(long)]
        include_documents: bool,
    },
    Handoff {
        project: String,
        task: String,
    },
}
async fn run(args: Args) -> precept_ipc::Result<serde_json::Value> {
    match args.command {
        Query::Profile { command } => return operator::profile(&args.location, command).await,
        Query::Project { command } => return operator::project(&args.location, command).await,
        Query::Preferences { command } => {
            return operator::preferences(&args.location, command).await;
        }
        Query::HandoffRecord { project, task } => {
            return precept_ipc::operator::control(
                &args.location,
                "profile.handoff.get",
                json!({"project":project,"task_id":task}),
            )
            .await;
        }
        Query::ProjectRegister(p) => return operator::register(&args.location, p).await,
        Query::ReviewQueue(p) => return operator::review_queue(&args.location, p).await,
        Query::ReviewInspect(p) => return operator::review_inspect(&args.location, p).await,
        Query::PreferenceSuggest(p) => return operator::suggest(&args.location, p).await,
        Query::PreferenceConfirm(p) => return operator::review(&args.location, p, "confirm").await,
        Query::PreferenceReject(p) => return operator::review(&args.location, p, "reject").await,
        Query::PreferenceDeprecate(p) => {
            return operator::review(&args.location, p, "deprecate").await;
        }
        Query::PreferenceDelete(p) => return operator::review(&args.location, p, "delete").await,
        Query::HandoffPut(p) => return operator::put(&args.location, p).await,
        _ => (),
    }
    let cap = load_credential(&args.location, &args.location.credential_path(None)?)?;
    if cap.access != Access::Operator {
        return Err(Error::Unauthorized);
    }
    let mut client = Client::connect(&args.location, &cap).await?;
    let (method, params) = match args.command {
        Query::Health => ("daemon.health", json!({})),
        Query::Projects => ("project.list", json!({})),
        Query::Activity { project, before } => {
            let mut p = json!({});
            if let Some(project) = project {
                p["project"] = project.into();
            }
            if let Some(before) = before {
                p["before"] = before.into();
            }
            ("activity.list", p)
        }
        Query::Context {
            project,
            include_documents,
        } => (
            "context.get",
            json!({"project":project,"include_documents":include_documents}),
        ),
        Query::Handoff { project, task } => {
            ("handoff.get", json!({"project":project,"task_id":task}))
        }
        _ => unreachable!("operator commands returned above"),
    };
    client.request(method, params).await
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    let operation_id = match &args.command {
        Query::ProjectRegister(p) => Some(&p.authority.operation_id),
        Query::PreferenceSuggest(p) => Some(&p.authority.operation_id),
        Query::PreferenceConfirm(p)
        | Query::PreferenceReject(p)
        | Query::PreferenceDeprecate(p)
        | Query::PreferenceDelete(p) => Some(&p.authority.operation_id),
        Query::HandoffPut(p) => Some(&p.authority.operation_id),
        _ => None,
    }
    .filter(|id| {
        !id.is_empty()
            && id.len() <= 64
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    })
    .cloned();
    match run(args).await {
        Ok(value) => println!("{}", value),
        Err(error) => {
            let mut value = json!({"error":error});
            if let Some(id) = operation_id {
                value["operation_id"] = id.into();
                value["outcome_unknown"] = (error == Error::OutcomeUnknown).into();
            }
            println!("{}", value);
            std::process::exit(3);
        }
    }
}
