//! Doctor: one page of checks over what the app already observed (daemon,
//! Profile, the selected workspace, its integrations and agent activity). Each
//! check says what it saw, never more: a value not read yet is `Unknown`, not
//! a pass. Pure; the page renders it and the report copies it as text with no
//! paths, preference values or credentials.
use crate::i18n::Msg;
use crate::{Model, Phase};

/// How a check came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Working as intended.
    Ok,
    /// Works, but something the operator likely wants to look at.
    Attention,
    /// Broken: agents miss context because of it.
    Problem,
    /// Deliberately not set up (a switch that is off).
    Off,
    /// Not observed (yet): never shown as a pass.
    Unknown,
}

/// Where a check's fix lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fix {
    Connection,
    Profile,
    Workspaces,
    Agents,
    Review,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub title: Msg,
    pub level: Level,
    /// Localized, sanitized detail.
    pub detail: String,
    pub fix: Option<Fix>,
}

fn check(title: Msg, level: Level, detail: impl Into<String>, fix: Option<Fix>) -> Check {
    Check {
        title,
        level,
        detail: detail.into(),
        fix,
    }
}

/// The harnesses with a one-switch session-start hook, in page order.
pub const HOOKS: [(&str, Msg); 3] = [
    ("claude", Msg::DoctorHookClaude),
    ("codex", Msg::DoctorHookCodex),
    ("antigravity", Msg::DoctorHookAntigravity),
];
/// The clients with a one-switch MCP entry, in page order.
pub const MCP: [(&str, Msg); 3] = [
    ("claude", Msg::DoctorMcpClaude),
    ("codex", Msg::DoctorMcpCodex),
    ("antigravity", Msg::DoctorMcpAntigravity),
];

fn daemon(m: &Model) -> Check {
    if !m.live {
        return check(
            Msg::DoctorDaemon,
            Level::Unknown,
            m.word(Msg::DoctorDemo),
            Some(Fix::Connection),
        );
    }
    match (&m.phase, &m.data) {
        (Phase::Ready, Some(d)) => {
            let mut detail = m
                .word(Msg::DoctorDaemonOk)
                .replace("{api}", &d.health.api_version.to_string());
            if let Some(ms) = m.last_duration_ms {
                detail.push_str(&format!(" · {ms} ms"));
            }
            check(Msg::DoctorDaemon, Level::Ok, detail, None)
        }
        (Phase::Loading, _) => check(
            Msg::DoctorDaemon,
            Level::Unknown,
            m.word(Msg::Checking),
            None,
        ),
        _ => check(
            Msg::DoctorDaemon,
            Level::Problem,
            m.word(Msg::DoctorDaemonDown),
            Some(Fix::Connection),
        ),
    }
}

fn profile(m: &Model) -> Vec<Check> {
    let Some(info) = m.profile_info.as_ref() else {
        return vec![check(
            Msg::DoctorProfile,
            Level::Unknown,
            m.word(Msg::DoctorNotRead),
            Some(Fix::Profile),
        )];
    };
    if !info.selected {
        return vec![check(
            Msg::DoctorProfile,
            Level::Problem,
            m.word(Msg::DoctorProfileNone),
            Some(Fix::Profile),
        )];
    }
    let selected = if info.remembered {
        check(
            Msg::DoctorProfile,
            Level::Ok,
            m.word(Msg::DoctorProfileOk),
            None,
        )
    } else {
        check(
            Msg::DoctorProfile,
            Level::Attention,
            m.word(Msg::DoctorProfileRunOnly),
            Some(Fix::Profile),
        )
    };
    let writes = if info.write_enabled {
        check(
            Msg::DoctorWrites,
            Level::Ok,
            m.word(Msg::DoctorWritesOn),
            None,
        )
    } else {
        check(
            Msg::DoctorWrites,
            Level::Off,
            m.word(Msg::DoctorWritesOff),
            Some(Fix::Profile),
        )
    };
    vec![selected, writes]
}

fn workspace(m: &Model) -> Vec<Check> {
    let Some(name) = m.selected_project.as_deref() else {
        return vec![check(
            Msg::DoctorWorkspace,
            Level::Unknown,
            m.word(Msg::DoctorNoWorkspace),
            Some(Fix::Workspaces),
        )];
    };
    let entry = m
        .data
        .as_ref()
        .and_then(|d| d.directory.iter().find(|e| e.name == name));
    let Some(entry) = entry else {
        return vec![check(
            Msg::DoctorWorkspace,
            Level::Unknown,
            m.word(Msg::DoctorNotRead),
            Some(Fix::Workspaces),
        )];
    };
    let state = match entry.state.as_str() {
        "ready" => check(
            Msg::DoctorWorkspace,
            Level::Ok,
            m.word(Msg::ProjectReady),
            None,
        ),
        "unregistered" => check(
            Msg::DoctorWorkspace,
            Level::Problem,
            m.word(Msg::DoctorUnregistered),
            Some(Fix::Workspaces),
        ),
        _ => check(
            Msg::DoctorWorkspace,
            Level::Problem,
            m.word(Msg::DoctorUnavailable),
            Some(Fix::Workspaces),
        ),
    };
    let shared = if entry.agents {
        check(
            Msg::DoctorShared,
            Level::Ok,
            m.word(Msg::DoctorSharedOn),
            None,
        )
    } else {
        check(
            Msg::DoctorShared,
            Level::Problem,
            m.word(Msg::DoctorSharedOff),
            Some(Fix::Agents),
        )
    };
    let writes = m.profile_info.as_ref().map(|i| i.write_enabled);
    let suggest = match (entry.propose, writes) {
        (false, _) => check(
            Msg::DoctorSuggest,
            Level::Off,
            m.word(Msg::DoctorSuggestOff),
            Some(Fix::Agents),
        ),
        (true, Some(false)) => check(
            Msg::DoctorSuggest,
            Level::Attention,
            m.word(Msg::DoctorSuggestWritesOff),
            Some(Fix::Profile),
        ),
        (true, _) => check(
            Msg::DoctorSuggest,
            Level::Ok,
            m.word(Msg::DoctorSuggestOn),
            None,
        ),
    };
    vec![state, shared, suggest]
}

/// Whether the integration reads on the Agents state belong to `project`.
fn for_project(m: &Model, project: Option<&str>) -> bool {
    project.is_some() && m.agents.project.as_deref() == project
}

fn hooks(m: &Model) -> Vec<Check> {
    let current = for_project(m, m.selected_project.as_deref());
    HOOKS
        .iter()
        .map(|(vendor, title)| {
            let unknown = |m: &Model| {
                check(
                    *title,
                    Level::Unknown,
                    m.word(Msg::DoctorNotRead),
                    Some(Fix::Agents),
                )
            };
            let Some(hooks) = m.agents.hooks.as_ref().filter(|_| current) else {
                return unknown(m);
            };
            if !hooks.available {
                return check(*title, Level::Problem, m.word(Msg::HookUnavailable), None);
            }
            let Some(h) = hooks.get(vendor) else {
                return unknown(m);
            };
            if h.malformed {
                check(
                    *title,
                    Level::Problem,
                    m.word(Msg::HookMalformed).replace("{file}", &h.file),
                    None,
                )
            } else if !h.installed {
                check(
                    *title,
                    Level::Off,
                    m.word(Msg::DoctorHookOff),
                    Some(Fix::Agents),
                )
            } else if !h.current {
                check(
                    *title,
                    Level::Attention,
                    m.word(Msg::HookOutdated),
                    Some(Fix::Agents),
                )
            } else {
                let key = if *vendor == "codex" {
                    Msg::HookOnCodex
                } else {
                    Msg::HookOn
                };
                check(
                    *title,
                    Level::Ok,
                    m.word(key).replace("{file}", &h.file),
                    None,
                )
            }
        })
        .collect()
}

fn mcp(m: &Model) -> Vec<Check> {
    let current = for_project(m, m.selected_project.as_deref());
    MCP.iter()
        .map(|(client, title)| {
            let Some(entries) = m.agents.mcp.as_ref().filter(|_| current) else {
                return check(
                    *title,
                    Level::Unknown,
                    m.word(Msg::DoctorNotRead),
                    Some(Fix::Agents),
                );
            };
            if !entries.available {
                return check(*title, Level::Problem, m.word(Msg::McpUnavailable), None);
            }
            match entries.get(client) {
                Some(e) if e.malformed => check(
                    *title,
                    Level::Problem,
                    m.word(Msg::HookMalformed).replace("{file}", &e.file),
                    None,
                ),
                Some(e) if e.installed && !e.current => check(
                    *title,
                    Level::Attention,
                    m.word(Msg::HookOutdated),
                    Some(Fix::Agents),
                ),
                Some(e) if e.installed => check(
                    *title,
                    Level::Ok,
                    m.word(Msg::McpOn).replace("{file}", &e.file),
                    None,
                ),
                Some(_) => check(
                    *title,
                    Level::Off,
                    m.word(Msg::DoctorMcpOff),
                    Some(Fix::Agents),
                ),
                None => check(
                    *title,
                    Level::Unknown,
                    m.word(Msg::DoctorNotRead),
                    Some(Fix::Agents),
                ),
            }
        })
        .collect()
}

fn pointer(m: &Model) -> Check {
    let title = Msg::TileAgentsMd;
    let Some(p) = m
        .agents
        .pointer
        .as_ref()
        .filter(|_| for_project(m, m.selected_project.as_deref()))
    else {
        return check(
            title,
            Level::Unknown,
            m.word(Msg::DoctorNotRead),
            Some(Fix::Agents),
        );
    };
    match (p.malformed, p.state.as_deref()) {
        (true, _) => check(
            title,
            Level::Problem,
            m.word(Msg::PointerMalformed),
            Some(Fix::Agents),
        ),
        (_, Some("current")) => check(title, Level::Ok, m.word(Msg::PointerTileOn), None),
        (_, Some("outdated")) => check(
            title,
            Level::Attention,
            m.word(Msg::PointerTileOutdated),
            Some(Fix::Agents),
        ),
        _ => check(
            title,
            Level::Off,
            m.word(Msg::DoctorPointerOff),
            Some(Fix::Agents),
        ),
    }
}

fn activity(m: &Model) -> Check {
    let title = Msg::DoctorActivity;
    let a = &m.agents.activity;
    let Some(page) = a
        .page
        .as_ref()
        .filter(|_| a.project.is_some() && a.project == m.selected_project)
    else {
        return check(
            title,
            Level::Unknown,
            m.word(Msg::DoctorNotRead),
            Some(Fix::Agents),
        );
    };
    // Reads only: a refused suggestion is the suggestions row's business.
    let refused = page
        .events
        .iter()
        .filter(|e| e.kind == "refused" || (e.kind != "suggested" && e.outcome != "ok"))
        .count();
    if let Some(last) = page.last_read() {
        let client = last
            .client
            .clone()
            .unwrap_or_else(|| m.word(Msg::DoctorUnnamedClient).into());
        let mut detail = m
            .word(Msg::DoctorLastRead)
            .replace("{time}", &crate::activity::clock(last.at_ms))
            .replace("{client}", &client);
        if refused > 0 {
            detail.push_str(" · ");
            detail.push_str(
                &m.word(Msg::DoctorRefused)
                    .replace("{n}", &refused.to_string()),
            );
            return check(title, Level::Attention, detail, Some(Fix::Agents));
        }
        return check(title, Level::Ok, detail, None);
    }
    if refused > 0 {
        return check(
            title,
            Level::Problem,
            m.word(Msg::DoctorRefused)
                .replace("{n}", &refused.to_string()),
            Some(Fix::Agents),
        );
    }
    if page.events.iter().any(|e| e.kind == "started") {
        return check(
            title,
            Level::Attention,
            m.word(Msg::DoctorStartedNoRead),
            Some(Fix::Agents),
        );
    }
    check(
        title,
        Level::Off,
        m.word(Msg::DoctorNoActivity),
        Some(Fix::Agents),
    )
}

fn preferences(m: &Model) -> Check {
    let title = Msg::DoctorPreferences;
    let confirmed = m.selected_project.as_deref().and_then(|name| {
        m.data
            .as_ref()?
            .projects
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.context.preferences.len())
    });
    let (Some(c), Some(waiting)) = (confirmed, m.review_count) else {
        return check(title, Level::Unknown, m.word(Msg::DoctorNotRead), None);
    };
    let detail = m
        .word(Msg::DoctorPreferencesCount)
        .replace("{c}", &c.to_string())
        .replace("{n}", &waiting.to_string());
    match (c, waiting) {
        (0, 0) => check(
            title,
            Level::Off,
            m.word(Msg::DoctorNoPreferences),
            Some(Fix::Review),
        ),
        (_, 0) => check(title, Level::Ok, detail, None),
        _ => check(title, Level::Attention, detail, Some(Fix::Review)),
    }
}

/// Every check, in the order a fix usually has to happen.
pub fn checks(m: &Model) -> Vec<Check> {
    let mut out = vec![daemon(m)];
    out.extend(profile(m));
    out.extend(workspace(m));
    out.push(preferences(m));
    out.extend(mcp(m));
    out.extend(hooks(m));
    out.push(pointer(m));
    out.push(activity(m));
    out
}

/// The number of problems and of checks worth a look.
pub fn tally(checks: &[Check]) -> (usize, usize) {
    let n = |l: Level| checks.iter().filter(|c| c.level == l).count();
    (n(Level::Problem), n(Level::Attention))
}

pub fn level_word(level: Level) -> Msg {
    match level {
        Level::Ok => Msg::DoctorLevelOk,
        Level::Attention => Msg::DoctorLevelAttention,
        Level::Problem => Msg::DoctorLevelProblem,
        Level::Off => Msg::DoctorLevelOff,
        Level::Unknown => Msg::DoctorLevelUnknown,
    }
}

/// The checks as plain text to paste into an issue or a chat: the project
/// name and what each check saw; never paths, values, commands or tokens.
pub fn report(m: &Model, checks: &[Check]) -> String {
    let mut out = format!(
        "Precept doctor · {}\n",
        m.selected_project
            .as_deref()
            .map(|p| crate::presentation::sanitize(p, 64))
            .unwrap_or_else(|| "-".into())
    );
    for c in checks {
        out.push_str(&format!(
            "[{}] {}: {}\n",
            m.word(level_word(c.level)),
            m.word(c.title),
            crate::presentation::sanitize(&c.detail, 240)
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::{HookState, Hooks, Pointer};

    fn ready() -> Model {
        let mut m = Model::loading();
        let mut data = crate::Snapshot::fixture();
        data.directory = vec![crate::ProjectEntry {
            name: "alpha".into(),
            root: "/home/u/alpha".into(),
            state: "ready".into(),
            agents: true,
            propose: false,
        }];
        m.data = Some(std::sync::Arc::new(data));
        m.phase = Phase::Ready;
        m.live = true;
        m.selected_project = Some("alpha".into());
        m
    }
    fn find(cs: &[Check], title: Msg) -> &Check {
        cs.iter().find(|c| c.title == title).unwrap()
    }

    #[test]
    fn unread_is_never_a_pass_and_each_state_has_its_fix() {
        let mut m = ready();
        let cs = checks(&m);
        assert_eq!(find(&cs, Msg::DoctorDaemon).level, Level::Ok);
        assert_eq!(find(&cs, Msg::DoctorWorkspace).level, Level::Ok);
        for title in [
            Msg::DoctorProfile,
            Msg::DoctorHookClaude,
            Msg::DoctorMcpClaude,
            Msg::TileAgentsMd,
            Msg::DoctorActivity,
            Msg::DoctorPreferences,
        ] {
            assert_eq!(find(&cs, title).level, Level::Unknown, "{title:?}");
        }

        *m.profile_info = Some(crate::operator::ProfileInfo {
            selected: true,
            write_enabled: false,
            remembered: false,
            ..Default::default()
        });
        m.agents.project = Some("alpha".into());
        m.agents.hooks = Some(Hooks {
            available: true,
            hooks: vec![
                HookState {
                    vendor: "claude".into(),
                    file: "~/.claude/settings.json".into(),
                    installed: true,
                    current: true,
                    ..Default::default()
                },
                HookState {
                    vendor: "codex".into(),
                    installed: true,
                    current: false,
                    ..Default::default()
                },
                HookState {
                    vendor: "antigravity".into(),
                    ..Default::default()
                },
            ],
        });
        m.agents.pointer = Some(Pointer {
            state: Some("absent".into()),
            ..Default::default()
        });
        m.review_count = Some(2);
        m.agents.mcp = Some(crate::mcp::McpEntries {
            available: true,
            clients: vec![
                crate::mcp::McpEntry {
                    client: "claude".into(),
                    file: "~/.claude.json".into(),
                    installed: true,
                    current: true,
                    ..Default::default()
                },
                crate::mcp::McpEntry {
                    client: "codex".into(),
                    ..Default::default()
                },
            ],
        });
        let cs = checks(&m);
        assert_eq!(find(&cs, Msg::DoctorMcpClaude).level, Level::Ok);
        assert_eq!(find(&cs, Msg::DoctorMcpCodex).level, Level::Off);
        assert_eq!(find(&cs, Msg::DoctorMcpAntigravity).level, Level::Unknown);
        let profile = find(&cs, Msg::DoctorProfile);
        assert_eq!(
            (profile.level, profile.fix),
            (Level::Attention, Some(Fix::Profile))
        );
        assert_eq!(find(&cs, Msg::DoctorWrites).level, Level::Off);
        assert_eq!(find(&cs, Msg::DoctorHookClaude).level, Level::Ok);
        assert_eq!(find(&cs, Msg::DoctorHookCodex).level, Level::Attention);
        assert_eq!(find(&cs, Msg::DoctorHookAntigravity).level, Level::Off);
        assert_eq!(find(&cs, Msg::TileAgentsMd).level, Level::Off);
        let prefs = find(&cs, Msg::DoctorPreferences);
        assert_eq!(
            (prefs.level, prefs.fix),
            (Level::Attention, Some(Fix::Review))
        );

        // Another project's integration reads are not this project's.
        m.selected_project = Some("beta".into());
        let cs = checks(&m);
        assert_eq!(find(&cs, Msg::DoctorHookClaude).level, Level::Unknown);
        assert_eq!(find(&cs, Msg::DoctorWorkspace).level, Level::Unknown);
    }

    #[test]
    fn workspace_sharing_and_activity_name_the_break() {
        let mut m = ready();
        let mut data = (**m.data.as_ref().unwrap()).clone();
        data.directory[0].state = "unregistered".into();
        data.directory[0].agents = false;
        m.data = Some(std::sync::Arc::new(data));
        m.agents.activity.project = Some("alpha".into());
        m.agents.activity.page = Some(crate::activity::Page::default());
        let cs = checks(&m);
        let ws = find(&cs, Msg::DoctorWorkspace);
        assert_eq!((ws.level, ws.fix), (Level::Problem, Some(Fix::Workspaces)));
        assert_eq!(find(&cs, Msg::DoctorShared).level, Level::Problem);
        assert_eq!(find(&cs, Msg::DoctorActivity).level, Level::Off);
        let (problems, _) = tally(&cs);
        assert_eq!(problems, 2);

        let event = |kind: &str, outcome: &str, at_ms| crate::activity::Event {
            id: at_ms,
            at_ms,
            kind: kind.into(),
            via: "mcp".into(),
            project: Some("alpha".into()),
            client: Some("antigravity hook".into()),
            outcome: outcome.into(),
            duration_ms: 1,
            counts: None,
        };
        m.agents.activity.page = Some(crate::activity::Page {
            daemon_started_ms: 0,
            events: vec![event("started", "ok", 2)],
            more: false,
        });
        assert_eq!(
            find(&checks(&m), Msg::DoctorActivity).level,
            Level::Attention
        );
        m.agents.activity.page = Some(crate::activity::Page {
            daemon_started_ms: 0,
            events: vec![event("context", "ok", 3_600_000)],
            more: false,
        });
        let a = find(&checks(&m), Msg::DoctorActivity).clone();
        assert_eq!(a.level, Level::Ok);
        assert!(a.detail.contains("01:00:00 UTC"));
        assert!(a.detail.contains("antigravity hook"));
    }

    #[test]
    fn the_report_carries_no_paths() {
        let m = ready();
        let text = report(&m, &checks(&m));
        assert!(text.starts_with("Precept doctor · alpha\n"));
        assert!(text.contains("Daemon"));
        assert!(!text.contains("/home/u"));
    }
}
