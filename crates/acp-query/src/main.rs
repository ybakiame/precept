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
    };
    client.request(method, params).await
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    match run(Args::parse()).await {
        Ok(value) => println!("{}", value),
        Err(error) => {
            println!("{}", json!({"error":error}));
            std::process::exit(3);
        }
    }
}
