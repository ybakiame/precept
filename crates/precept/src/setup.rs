use clap::Args;
use precept_ipc::{Access, Client, Error, Location, Result, load_credential, operator::control};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

#[derive(Args)]
pub struct Options {
    /// Exact Profile path to create (absent) or select (present).
    #[arg(long)]
    pub profile: Option<PathBuf>,
    /// Exact checkout to check and register; never scanned.
    #[arg(long)]
    pub project: Option<PathBuf>,
    /// Explicit project name; defaults to the daemon's suggestion.
    #[arg(long)]
    pub name: Option<String>,
    /// Remember an explicitly created/selected Profile at daemon startup.
    #[arg(long)]
    pub remember: Option<bool>,
    /// Let this project's agents read context through MCP.
    #[arg(long)]
    pub share: Option<bool>,
    /// Let agents create suggestions and their own session/memory records.
    #[arg(long)]
    pub propose: Option<bool>,
    /// Preview and install the one MCP entry plus session-start hooks.
    #[arg(long)]
    pub integrate: Option<bool>,
    /// Turn on the cooperative pre-tool gate and preview its hooks.
    #[arg(long)]
    pub gate: Option<bool>,
    /// Print the exact plan without creating, selecting or changing anything.
    #[arg(long)]
    pub plan: bool,
}

struct State {
    enabled_here: bool,
    keep_writes: bool,
}

pub async fn run(location: &Location, options: &Options) -> Result<Value> {
    let mut state = State {
        enabled_here: false,
        keep_writes: false,
    };
    let mut result = flow(location, options, &mut state).await;
    if state.enabled_here && !(result.is_ok() && state.keep_writes) {
        let _ = control(location, "profile.write.disable", json!({})).await;
        if let Ok(value) = &mut result {
            let info = control(location, "profile.info", json!({})).await?;
            value["write_enabled"] = json!(info["write_enabled"]);
        }
    }
    result
}

async fn flow(location: &Location, options: &Options, state: &mut State) -> Result<Value> {
    let info = control(location, "profile.info", json!({})).await?;
    let profile_path = if let Some(path) = &options.profile {
        Some(path.clone())
    } else if info["selected"] != true {
        let suggested = info["suggested_path"].as_str().unwrap_or("").to_owned();
        Some(PathBuf::from(prompt(
            "Profile path (a new file will be created only after preview)",
            (!suggested.is_empty()).then_some(suggested.as_str()),
        )?))
    } else {
        None
    };
    let profile_action = profile_action(&info, profile_path.as_deref())?;
    let remember = if profile_action == "keep" {
        false
    } else {
        options
            .remember
            .unwrap_or_else(|| confirm("Remember this Profile for future daemon startups?", false))
    };

    let project_root = if let Some(root) = &options.project {
        root.clone()
    } else {
        let cwd = std::env::current_dir().map_err(|_| Error::Unavailable)?;
        PathBuf::from(prompt(
            "Existing Git checkout to register",
            cwd.to_str().filter(|_| !cwd.as_os_str().is_empty()),
        )?)
    };
    let checked = control(location, "project.check", json!({"root":project_root})).await?;
    if checked["ok"] != true {
        let reason = checked["reason"].as_str().unwrap_or("refused");
        eprintln!(
            "setup: project check refused ({reason}); choose an eligible checkout inside your home"
        );
        return Err(Error::UnsafeRuntime);
    }
    let root = checked["root"].as_str().unwrap_or_default();
    let suggested_name = checked["name"].as_str().unwrap_or_default();
    let name = match &options.name {
        Some(name) if precept_ipc::valid_project(name) => name.clone(),
        Some(_) => return Err(Error::InvalidRequest),
        None if !suggested_name.is_empty() => suggested_name.to_owned(),
        None => prompt("Project name", None)?,
    };
    if !precept_ipc::valid_project(&name) {
        return Err(Error::InvalidRequest);
    }
    let projects = control(location, "project.list", json!({})).await?;
    let project_action = project_action(&projects, root, &name)?;
    let current = projects
        .as_array()
        .and_then(|list| list.iter().find(|project| project["name"] == name));
    let needs_write = matches!(project_action.as_str(), "add" | "register");
    let write_action = if needs_write && info["write_enabled"] != true {
        "enable"
    } else {
        "none"
    };
    let share = options
        .share
        .unwrap_or_else(|| confirm("Share read access with this project's agents (MCP)?", true));
    let propose = options.propose.unwrap_or_else(|| {
        confirm(
            "Let agents suggest rules and keep their own sessions/memory?",
            false,
        )
    });
    let integrate = options.integrate.unwrap_or_else(|| {
        confirm(
            "Install the previewed MCP entry and session-start hooks for Claude Code/Codex?",
            true,
        )
    });
    let gate = options.gate.unwrap_or_else(|| {
        confirm(
            "Turn on the cooperative pre-tool gate and preview its hooks (not a sandbox)?",
            false,
        )
    });
    let plan = json!({
        "status":"planned",
        "product_version":env!("CARGO_PKG_VERSION"),
        "profile":{"action":profile_action,"path":profile_path,"remember":remember},
        "project":{"name":name,"root":root,"action":project_action},
        "write":{"action":write_action,"scope":"selected Profile, current daemon run only"},
        "access":{"share":share,"propose":propose},
        "integrations":{"mcp_and_session_start":integrate,"cooperative_gate":gate},
        "enforcement_claimed":false,
        "verification":"context.get after the separately confirmed changes"
    });
    if options.plan {
        return Ok(plan);
    }
    eprintln!(
        "Setup preview:\n{}",
        serde_json::to_string_pretty(&plan).map_err(|_| Error::Unavailable)?
    );

    if profile_action != "keep"
        && confirm(
            &format!(
                "Apply Profile {} for the exact path shown above?",
                profile_action
            ),
            false,
        )
    {
        control(
            location,
            if profile_action == "create" {
                "profile.create"
            } else {
                "profile.select"
            },
            json!({"path":profile_path,"remember":remember}),
        )
        .await?;
    } else if profile_action != "keep" {
        return Err(Error::InvalidRequest);
    }
    let mut info = control(location, "profile.info", json!({})).await?;
    if info["selected"] != true {
        return Err(Error::Unavailable);
    }
    if write_action == "enable"
        && confirm(
            "Enable Profile writes for this daemon run so the project can be added/registered?",
            false,
        )
    {
        let binding = &info["binding"];
        control(
            location,
            "profile.write.enable",
            json!({
                "profile_id":binding["profile_id"],
                "generation":binding["generation"],
                "confirm":true
            }),
        )
        .await?;
        state.enabled_here = true;
        info = control(location, "profile.info", json!({})).await?;
    } else if write_action == "enable" {
        return Err(Error::InvalidRequest);
    }
    if project_action != "keep"
        && confirm(
            &format!(
                "Apply project {} for '{}' at the exact checkout shown above?",
                project_action, name
            ),
            false,
        )
    {
        let method = if project_action == "add" {
            "project.add"
        } else {
            "project.register"
        };
        let mut params = json!({"name":name});
        if project_action == "add" {
            params["root"] = json!(root);
        }
        precept_ipc::operator::project_write(location, method, params).await?;
    } else if project_action != "keep" {
        return Err(Error::InvalidRequest);
    }

    let mut changes = Vec::new();
    if share
        && current.is_none_or(|project| project["agents"] != true)
        && confirm("Share agent read access for this project?", false)
    {
        control(
            location,
            "project.agents",
            json!({"name":name,"enabled":true}),
        )
        .await?;
        changes.push("agent_read");
    }
    if propose
        && current.is_none_or(|project| project["propose"] != true)
        && confirm(
            "Allow agent suggestions/session records (still inactive until review)?",
            false,
        )
    {
        control(
            location,
            "project.propose",
            json!({"name":name,"enabled":true}),
        )
        .await?;
        changes.push("agent_propose");
    }
    if gate
        && confirm(
            "Turn on the cooperative gate for this project (not an enforcement guarantee)?",
            false,
        )
    {
        control(
            location,
            "project.gate",
            json!({"name":name,"enabled":true}),
        )
        .await?;
        changes.push("cooperative_gate");
    }
    if integrate {
        changes.extend(configure_mcp(location, &name).await?);
        changes.extend(configure_hooks(location, &name, "hooks", "session_start").await?);
    }
    if gate {
        changes.extend(configure_hooks(location, &name, "gate_hooks", "pre_tool").await?);
    }
    let context = operator_request(
        location,
        "context.get",
        json!({"project":name,"include_documents":false}),
    )
    .await?;
    let projects = control(location, "project.list", json!({})).await?;
    let project = projects
        .as_array()
        .and_then(|list| list.iter().find(|project| project["name"] == name))
        .ok_or(Error::NotFound)?;
    if project["state"] != "ready" {
        return Err(Error::Unavailable);
    }
    if state.enabled_here {
        state.keep_writes = !confirm_or_default(
            "Disable the write grant this setup enabled? Suggestions need writes enabled again per run.",
            true,
        );
    }
    Ok(json!({
        "status":"ready",
        "project":name,
        "preferences":context["preferences"].as_array().map_or(0, Vec::len),
        "repository_rules":context["repository_rules"].as_array().map_or(0, Vec::len),
        "contract_status":context["contract_resolution"]["status"],
        "agent_read":project["agents"],
        "agent_propose":project["propose"],
        "cooperative_gate":project["gate"],
        "enforcement_claimed":false,
        "suggestions_require_write_enable":propose,
        "changes":changes,
        "write_enabled":info["write_enabled"]
    }))
}

async fn operator_request(location: &Location, method: &str, params: Value) -> Result<Value> {
    let credential = load_credential(location, &location.credential_path(None)?)?;
    if credential.access != Access::Operator {
        return Err(Error::Unauthorized);
    }
    let mut client = Client::connect(location, &credential).await?;
    client.request(method, params).await
}

fn profile_action(info: &Value, path: Option<&Path>) -> Result<&'static str> {
    if info["selected"] == true {
        return if path.is_some() {
            Err(Error::RestartRequired)
        } else {
            Ok("keep")
        };
    }
    match path {
        None => Err(Error::InvalidRequest),
        Some(path) if path.exists() => Ok("select"),
        Some(_) => Ok("create"),
    }
}

fn project_action(projects: &Value, root: &str, name: &str) -> Result<String> {
    let list = projects.as_array().ok_or(Error::Unavailable)?;
    if let Some(project) = list.iter().find(|project| project["root"] == root) {
        if project["name"] != name {
            return Err(Error::Conflict);
        }
        return Ok(if project["state"] == "ready" {
            "keep".into()
        } else {
            "register".into()
        });
    }
    if list.iter().any(|project| project["name"] == name) {
        return Err(Error::Conflict);
    }
    Ok("add".into())
}

async fn configure_mcp(location: &Location, project: &str) -> Result<Vec<&'static str>> {
    let status = control(location, "integration.mcp", json!({})).await?;
    if status["available"] != true {
        eprintln!("setup: precept-mcp is unavailable; MCP configuration was not changed");
        return Ok(vec![]);
    }
    let launch = control(location, "project.mcp", json!({"name":project})).await?;
    let mut changed = Vec::new();
    for client in status["clients"].as_array().into_iter().flatten() {
        if client["current"] == true {
            continue;
        }
        eprintln!(
            "MCP preview for {} in {}:\ncommand: {}\nargs: {}",
            client["client"], client["file"], launch["executable"], launch["args"]
        );
        if confirm("Apply this exact MCP entry?", false) {
            control(
                location,
                "integration.mcp.apply",
                json!({
                    "client":client["client"],"enabled":true,
                    "expected_sha256":client["sha256"]
                }),
            )
            .await?;
            changed.push("mcp_entry");
        }
    }
    Ok(changed)
}

async fn configure_hooks(
    location: &Location,
    project: &str,
    field: &str,
    event: &str,
) -> Result<Vec<&'static str>> {
    let status = control(
        location,
        "integration.hooks",
        json!({"project":project,"event":event}),
    )
    .await?;
    if status["available"] != true {
        eprintln!("setup: precept-hook is unavailable; {event} hooks were not changed");
        return Ok(vec![]);
    }
    let mut changed = Vec::new();
    for hook in status[field].as_array().into_iter().flatten() {
        if hook["current"] == true {
            continue;
        }
        eprintln!(
            "{} preview for {} in {}:\n{}",
            event, hook["vendor"], hook["file"], hook["command"]
        );
        if confirm("Apply this exact hook entry?", false) {
            control(
                location,
                "integration.hooks.apply",
                json!({
                    "project":project,"vendor":hook["vendor"],"event":event,
                    "enabled":true,"expected_sha256":hook["sha256"]
                }),
            )
            .await?;
            changed.push("hook_entry");
        }
    }
    Ok(changed)
}

fn prompt(label: &str, default: Option<&str>) -> Result<String> {
    eprint!("{label}");
    if let Some(default) = default {
        eprint!(" [{default}]");
    }
    eprint!(": ");
    std::io::stderr().flush().map_err(|_| Error::Unavailable)?;
    let mut line = String::new();
    let read = std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|_| Error::Unavailable)?;
    if read == 0 {
        return Err(Error::InvalidRequest);
    }
    let answer = line.trim();
    if answer.is_empty() {
        return default.map(str::to_owned).ok_or(Error::InvalidRequest);
    }
    Ok(answer.to_owned())
}

fn confirm(label: &str, default: bool) -> bool {
    loop {
        let hint = if default { "Y/n" } else { "y/N" };
        let Ok(answer) = prompt(&format!("{label} [{hint}]"), None) else {
            return false;
        };
        match answer.to_ascii_lowercase().as_str() {
            "y" | "yes" => return true,
            "n" | "no" => return false,
            "" => return default,
            _ => eprintln!("setup: answer yes or no"),
        }
    }
}

// EOF while revoking setup's grant must keep the safe default, not retain writes.
fn confirm_or_default(label: &str, default: bool) -> bool {
    loop {
        let hint = if default { "Y/n" } else { "y/N" };
        match prompt(&format!("{label} [{hint}]"), None) {
            Ok(answer) => match answer.to_ascii_lowercase().as_str() {
                "y" | "yes" => return true,
                "n" | "no" => return false,
                "" => return default,
                _ => eprintln!("setup: answer yes or no"),
            },
            Err(_) => return default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_actions_never_adopt_or_overwrite_implicitly() {
        let absent = json!({"selected":false});
        let selected = json!({"selected":true});
        assert_eq!(
            profile_action(&absent, Some(Path::new("new"))).unwrap(),
            "create"
        );
        assert_eq!(
            profile_action(&absent, Some(Path::new("."))).unwrap(),
            "select"
        );
        assert_eq!(profile_action(&selected, None).unwrap(), "keep");
        assert_eq!(
            profile_action(&selected, Some(Path::new("other"))),
            Err(Error::RestartRequired)
        );
        assert_eq!(profile_action(&absent, None), Err(Error::InvalidRequest));
    }

    #[test]
    fn project_actions_distinguish_add_register_keep_and_conflict() {
        let projects = json!([
            {"name":"alpha","root":"/home/u/a","state":"ready"},
            {"name":"beta","root":"/home/u/b","state":"unregistered"}
        ]);
        assert_eq!(
            project_action(&projects, "/home/u/a", "alpha").unwrap(),
            "keep"
        );
        assert_eq!(
            project_action(&projects, "/home/u/b", "beta").unwrap(),
            "register"
        );
        assert_eq!(
            project_action(&projects, "/home/u/c", "gamma").unwrap(),
            "add"
        );
        assert_eq!(
            project_action(&projects, "/home/u/a", "other"),
            Err(Error::Conflict)
        );
        assert_eq!(
            project_action(&projects, "/home/u/c", "alpha"),
            Err(Error::Conflict)
        );
    }
}
