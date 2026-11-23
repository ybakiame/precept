mod operator;
mod skill;
mod transfer;
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
    /// Switch a confirmed rule back on.
    PreferenceEnable(operator::Switch),
    /// Switch a confirmed rule off: it stays reviewed but agents stop receiving it.
    PreferenceDisable(operator::Switch),
    HandoffPut(operator::Put),
    /// The precept-connect skill for agents in other harnesses, filled with
    /// this installation's MCP command and each shared project's hook
    /// commands. Printed as Markdown, or written once to <out>/SKILL.md.
    Skill {
        /// Only this project's hook commands.
        #[arg(long)]
        project: Option<String>,
        /// A directory to write SKILL.md into (created; never overwritten).
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
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
    /// Why each rule of KEY reaches a request (narrowed like an agent's
    /// selector) or not, with the reason.
    Explain {
        project: String,
        key: String,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        language: Option<String>,
        #[arg(long = "task-tag")]
        task_tags: Vec<String>,
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
        Query::PreferenceEnable(p) => return operator::switch(&args.location, p, true).await,
        Query::PreferenceDisable(p) => return operator::switch(&args.location, p, false).await,
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
        Query::Explain {
            project,
            key,
            agent,
            path,
            language,
            task_tags,
        } => {
            let mut selector = json!({});
            for (k, v) in [("agent_id", agent), ("path", path), ("language", language)] {
                if let Some(v) = v {
                    selector[k] = v.into();
                }
            }
            if !task_tags.is_empty() {
                selector["task_tags"] = task_tags.into();
            }
            (
                "context.explain",
                json!({"project":project,"key":key,"selector":selector}),
            )
        }
        _ => unreachable!("operator commands returned above"),
    };
    client.request(method, params).await
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    if let Query::Skill { project, out } = &args.command {
        let result = match skill::render(&args.location, project.as_deref()).await {
            Ok(text) => match out {
                Some(dir) => skill::write_new(dir, &text)
                    .map(|file| json!({"written": file}).to_string() + "\n"),
                None => Ok(text),
            },
            Err(e) => Err(e),
        };
        match result {
            Ok(text) => print!("{text}"),
            Err(error) => {
                println!("{}", json!({"error":error}));
                std::process::exit(3);
            }
        }
        return;
    }
    let operation_id = match &args.command {
        Query::ProjectRegister(p) => Some(&p.authority.operation_id),
        Query::PreferenceSuggest(p) => Some(&p.authority.operation_id),
        Query::PreferenceConfirm(p)
        | Query::PreferenceReject(p)
        | Query::PreferenceDeprecate(p)
        | Query::PreferenceDelete(p) => Some(&p.authority.operation_id),
        Query::PreferenceEnable(p) | Query::PreferenceDisable(p) => Some(&p.authority.operation_id),
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
