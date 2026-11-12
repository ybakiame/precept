mod operator;
use acp_ipc::{Access, Client, Error, Location, load_credential};
use clap::{Parser, Subcommand};
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
    HandoffRecord {
        project: String,
        task: String,
    },
    ProjectRegister(operator::Register),
    PreferenceSuggest(operator::Suggest),
    PreferenceConfirm(operator::Review),
    PreferenceReject(operator::Review),
    PreferenceDeprecate(operator::Review),
    PreferenceDelete(operator::Review),
    HandoffPut(operator::Put),
    Health,
    /// Explicit native operator launch. Sharing is off unless separately selected.
    SessionStart {
        project: String,
        executable: String,
        #[arg(long)]
        confirm_start: bool,
        #[arg(long)]
        share_context: bool,
    },
    SessionContext {
        project: String,
    },
    SessionStop {
        project: String,
        session_id: String,
    },
    Projects,
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
async fn run(args: Args) -> acp_ipc::Result<serde_json::Value> {
    match args.command {
        Query::Profile { command } => return operator::profile(&args.location, command).await,
        Query::Preferences { command } => {
            return operator::preferences(&args.location, command).await;
        }
        Query::HandoffRecord { project, task } => {
            return acp_ipc::operator::control(
                &args.location,
                "profile.handoff.get",
                json!({"project":project,"task_id":task}),
            )
            .await;
        }
        Query::ProjectRegister(p) => return operator::register(&args.location, p).await,
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
        Query::SessionStart {
            project,
            executable,
            confirm_start,
            share_context,
        } => (
            "session.start",
            json!({"session_api_version":acp_session::SESSION_API,"project":project,"executable":executable,"confirmed":confirm_start,"share_context":share_context}),
        ),
        Query::SessionContext { project } => (
            "session.context",
            json!({"session_api_version":acp_session::SESSION_API,"project":project}),
        ),
        Query::SessionStop {
            project,
            session_id,
        } => (
            "session.stop",
            json!({"session_api_version":acp_session::SESSION_API,"project":project,"session_id":session_id}),
        ),
        Query::Health => ("daemon.health", json!({})),
        Query::Projects => ("project.list", json!({})),
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
