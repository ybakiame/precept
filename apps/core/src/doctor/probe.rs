use super::presentation::{Check, Fix, Level, check};
use crate::{Model, Phase, i18n::Msg};

/// The harnesses with a one-switch session-start hook, in page order.
pub const HOOKS: [(&str, Msg); 2] = [
    ("claude", Msg::DoctorHookClaude),
    ("codex", Msg::DoctorHookCodex),
];
/// The clients with a one-switch MCP entry, in page order.
pub const MCP: [(&str, Msg); 2] = [
    ("claude", Msg::DoctorMcpClaude),
    ("codex", Msg::DoctorMcpCodex),
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
    } else if let Some(reason) = &info.write_restore_failure {
        let message = if reason == "identity_mismatch" {
            Msg::DoctorWriteRestoreIdentityMismatch
        } else {
            Msg::DoctorWriteRestoreFailed
        };
        check(
            Msg::DoctorWrites,
            Level::Attention,
            m.word(message),
            Some(Fix::Profile),
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
            } else if h.broken {
                check(
                    *title,
                    Level::Problem,
                    m.word(Msg::HookBroken),
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
                Some(e) if e.installed && e.broken => check(
                    *title,
                    Level::Problem,
                    m.word(Msg::HookBroken),
                    Some(Fix::Agents),
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

fn rules(m: &Model) -> Vec<Check> {
    let Some(p) = m.project() else {
        return vec![check(
            Msg::DoctorRules,
            Level::Unknown,
            m.word(Msg::DoctorNotRead),
            None,
        )];
    };
    let c = &p.context;
    let rules = &c.repository_rules;
    let status = c
        .contract_resolution
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let first = match status {
        "invalid_source" => {
            let files: Vec<&str> = c
                .contract_resolution
                .get("invalid_files")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .collect();
            check(
                Msg::DoctorRules,
                Level::Problem,
                m.word(Msg::RulesInvalid)
                    .replace("{files}", &files.join(", ")),
                None,
            )
        }
        "source_unavailable" => check(
            Msg::DoctorRules,
            Level::Problem,
            m.word(Msg::RulesUnreadable),
            None,
        ),
        _ if rules.is_empty() && c.pending_rules.is_empty() => check(
            Msg::DoctorRules,
            Level::Off,
            m.word(Msg::DoctorRulesNone),
            None,
        ),
        _ => {
            let status_of = |r: &crate::RepositoryRule| {
                r.checks
                    .get("status")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_owned()
            };
            let failing: Vec<&str> = rules
                .iter()
                .filter(|r| status_of(r) == "failing")
                .map(|r| r.key.as_str())
                .collect();
            if failing.is_empty() {
                check(
                    Msg::DoctorRules,
                    if c.pending_rules.is_empty() {
                        Level::Ok
                    } else {
                        Level::Attention
                    },
                    m.word(Msg::DoctorRulesOk)
                        .replace("{n}", &rules.len().to_string())
                        .replace(
                            "{v}",
                            &rules
                                .iter()
                                .filter(|r| status_of(r) == "verified")
                                .count()
                                .to_string(),
                        )
                        .replace("{p}", &c.pending_rules.len().to_string()),
                    None,
                )
            } else {
                check(
                    Msg::DoctorRules,
                    Level::Attention,
                    m.word(Msg::DoctorRulesFailing)
                        .replace("{f}", &failing.len().to_string())
                        .replace("{n}", &rules.len().to_string())
                        .replace(
                            "{keys}",
                            &crate::presentation::sanitize(&failing.join(", "), 160),
                        ),
                    None,
                )
            }
        }
    };
    // The gate: reported as set up, never as enforcement achieved.
    let guarded = rules
        .iter()
        .filter(|r| r.guards > 0 && r.enforcement.as_deref() == Some("hard"))
        .count();
    let gate_on = m
        .data
        .as_ref()
        .and_then(|d| d.directory.iter().find(|e| e.name == p.name))
        .is_some_and(|e| e.gate);
    let gate = if guarded == 0 {
        check(
            Msg::DoctorGate,
            Level::Off,
            m.word(Msg::DoctorGateNoGuards),
            None,
        )
    } else if !gate_on {
        check(
            Msg::DoctorGate,
            Level::Off,
            m.word(Msg::DoctorGateOff)
                .replace("{n}", &guarded.to_string()),
            Some(Fix::Agents),
        )
    } else {
        let current = for_project(m, m.selected_project.as_deref());
        match m.agents.hooks.as_ref().filter(|_| current) {
            None => check(
                Msg::DoctorGate,
                Level::Unknown,
                m.word(Msg::DoctorNotRead),
                Some(Fix::Agents),
            ),
            Some(h) => {
                let names: Vec<&str> = [("claude", Msg::TileClaude), ("codex", Msg::TileCodex)]
                    .iter()
                    .filter(|(v, _)| h.of(v, "pre_tool").is_some_and(|s| s.installed))
                    .map(|(_, t)| m.word(*t))
                    .collect();
                if names.is_empty() {
                    check(
                        Msg::DoctorGate,
                        Level::Attention,
                        m.word(Msg::DoctorGateNoHook),
                        Some(Fix::Agents),
                    )
                } else {
                    check(
                        Msg::DoctorGate,
                        Level::Ok,
                        m.word(Msg::DoctorGateOn)
                            .replace("{hooks}", &names.join(", ")),
                        None,
                    )
                }
            }
        }
    };
    vec![first, gate]
}

/// How each agent is connected, in one row: OK when at least one has the
/// recommended hooks + MCP; never guessed before both were read.
fn level(m: &Model) -> Check {
    use crate::mcp::{AGENTS, Level as Connection};
    let title = Msg::DoctorLevel;
    let unread = |m: &Model| {
        check(
            title,
            Level::Unknown,
            m.word(Msg::DoctorNotRead),
            Some(Fix::Agents),
        )
    };
    if !for_project(m, m.selected_project.as_deref()) {
        return unread(m);
    }
    let names = [Msg::TileClaude, Msg::TileCodex];
    let mut parts = vec![];
    let mut levels = vec![];
    for (agent, name) in AGENTS.iter().zip(names) {
        let Some(l) = m.agents.level(agent) else {
            return unread(m);
        };
        let word = match l {
            Connection::Full => Msg::LevelFull,
            Connection::Hooks => Msg::LevelHooks,
            Connection::Mcp => Msg::LevelMcp,
            Connection::None => Msg::LevelNone,
        };
        parts.push(format!("{}: {}", m.word(name), m.word(word)));
        levels.push(l);
    }
    if levels.contains(&Connection::Full) {
        check(title, Level::Ok, parts.join("; "), None)
    } else if levels.iter().all(|l| *l == Connection::None) {
        check(
            title,
            Level::Off,
            m.word(Msg::DoctorLevelNone),
            Some(Fix::Agents),
        )
    } else {
        check(title, Level::Attention, parts.join("; "), Some(Fix::Agents))
    }
}

/// Every check, in the order a fix usually has to happen.
pub fn checks(m: &Model) -> Vec<Check> {
    let mut out = vec![daemon(m)];
    out.extend(profile(m));
    out.extend(workspace(m));
    out.push(preferences(m));
    out.extend(rules(m));
    out.extend(mcp(m));
    out.extend(hooks(m));
    out.push(level(m));
    out.push(pointer(m));
    out.push(activity(m));
    out
}
