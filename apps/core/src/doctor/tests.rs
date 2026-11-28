use super::*;
use crate::{
    Model, Phase,
    i18n::Msg,
    mcp::{HookState, Hooks, Pointer},
};

fn ready() -> Model {
    let mut m = Model::loading();
    let mut data = crate::Snapshot::fixture();
    data.directory = vec![crate::ProjectEntry {
        name: "alpha".into(),
        label: None,
        root: "/home/u/alpha".into(),
        state: "ready".into(),
        agents: true,
        propose: false,
        gate: false,
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
fn connection_level_needs_both_reads_and_names_each_agent() {
    use crate::mcp::{McpEntries, McpEntry};
    let mut m = ready();
    assert_eq!(find(&checks(&m), Msg::DoctorLevel).level, Level::Unknown);
    m.agents.project = Some("alpha".into());
    let hook = |vendor: &str, installed| HookState {
        vendor: vendor.into(),
        installed,
        current: installed,
        ..Default::default()
    };
    m.agents.hooks = Some(Hooks {
        available: true,
        hooks: vec![hook("claude", true), hook("codex", false)],
        ..Default::default()
    });
    // Hooks read, MCP entries not yet: still not guessed.
    assert_eq!(find(&checks(&m), Msg::DoctorLevel).level, Level::Unknown);
    let entry = |client: &str, installed| McpEntry {
        client: client.into(),
        installed,
        ..Default::default()
    };
    m.agents.mcp = Some(McpEntries {
        available: true,
        clients: vec![entry("claude", true), entry("codex", true)],
    });
    let c = find(&checks(&m), Msg::DoctorLevel).clone();
    assert_eq!(c.level, Level::Ok);
    assert_eq!(
        c.detail,
        "Claude Code: Hooks + MCP · recommended; Codex: MCP only · only when the agent calls it"
    );
    m.agents.mcp = Some(McpEntries {
        available: true,
        clients: vec![entry("claude", false), entry("codex", false)],
    });
    assert_eq!(find(&checks(&m), Msg::DoctorLevel).level, Level::Attention);
}

#[test]
fn repository_rules_and_the_gate_report_what_was_observed() {
    let mut m = ready();
    let cs = checks(&m);
    // The fixture: one verified rule, one uncommitted file, no guards.
    let r = find(&cs, Msg::DoctorRules);
    assert_eq!(r.level, Level::Attention);
    assert!(r.detail.contains("1 applying"), "{}", r.detail);
    assert!(r.detail.contains("1 not committed"));
    assert_eq!(find(&cs, Msg::DoctorGate).level, Level::Off);
    // A guarded hard rule with a failing check, the gate on, no hooks read.
    let data = std::sync::Arc::make_mut(m.data.as_mut().unwrap());
    let ctx = &mut data
        .projects
        .iter_mut()
        .find(|p| p.name == "alpha")
        .unwrap()
        .context;
    ctx.pending_rules.clear();
    ctx.repository_rules[0].enforcement = Some("hard".into());
    ctx.repository_rules[0].guards = 1;
    ctx.repository_rules[0].checks = serde_json::json!({"status":"failing","passed":0,"total":1});
    data.directory[0].gate = true;
    let cs = checks(&m);
    let r = find(&cs, Msg::DoctorRules);
    assert_eq!(r.level, Level::Attention);
    assert!(r.detail.contains("formatter"));
    assert_eq!(find(&cs, Msg::DoctorGate).level, Level::Unknown);
    m.agents.project = Some("alpha".into());
    m.agents.hooks = Some(Hooks {
        available: true,
        hooks: vec![],
        read_hooks: vec![],
        gate_hooks: vec![HookState {
            vendor: "codex".into(),
            installed: true,
            current: true,
            ..Default::default()
        }],
        compact_hooks: vec![],
        gate: true,
    });
    let g = find(&checks(&m), Msg::DoctorGate).clone();
    assert_eq!(g.level, Level::Ok, "{}", g.detail);
    assert!(g.detail.contains("Codex"));
    // An invalid rule file is a problem, named.
    let data = std::sync::Arc::make_mut(m.data.as_mut().unwrap());
    data.projects
        .iter_mut()
        .find(|p| p.name == "alpha")
        .unwrap()
        .context
        .contract_resolution = serde_json::json!({"status":"invalid_source",
                "invalid_files":[".precept/rules/x.md"]});
    let r = find(&checks(&m), Msg::DoctorRules).clone();
    assert_eq!(r.level, Level::Problem);
    assert!(r.detail.contains(".precept/rules/x.md"));
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
        ],
        ..Default::default()
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
    let profile = find(&cs, Msg::DoctorProfile);
    assert_eq!(
        (profile.level, profile.fix),
        (Level::Attention, Some(Fix::Profile))
    );
    assert_eq!(find(&cs, Msg::DoctorWrites).level, Level::Off);
    assert_eq!(find(&cs, Msg::DoctorHookClaude).level, Level::Ok);
    assert_eq!(find(&cs, Msg::DoctorHookCodex).level, Level::Attention);
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
        client: Some("qoder hook".into()),
        outcome: outcome.into(),
        duration_ms: 1,
        counts: None,
        session: None,
        rule: None,
    };
    m.agents.activity.page = Some(crate::activity::Page {
        daemon_started_ms: 0,
        events: vec![event("started", "ok", 2)],
        more: false,
        chain: None,
    });
    assert_eq!(
        find(&checks(&m), Msg::DoctorActivity).level,
        Level::Attention
    );
    m.agents.activity.page = Some(crate::activity::Page {
        daemon_started_ms: 0,
        events: vec![event("context", "ok", 3_600_000)],
        more: false,
        chain: None,
    });
    let a = find(&checks(&m), Msg::DoctorActivity).clone();
    assert_eq!(a.level, Level::Ok);
    assert!(a.detail.contains("01:00:00 UTC"));
    assert!(a.detail.contains("qoder hook"));
}

#[test]
fn profile_identity_mismatch_is_explained_as_read_only() {
    let mut m = ready();
    *m.profile_info = Some(crate::operator::ProfileInfo {
        selected: true,
        write_enabled: false,
        remembered: true,
        write_restore_failure: Some("identity_mismatch".into()),
        ..Default::default()
    });
    let check = find(&checks(&m), Msg::DoctorWrites).clone();
    assert_eq!(check.level, Level::Attention);
    assert!(check.detail.contains("identity no longer matches"));
}

#[test]
fn the_report_carries_no_paths() {
    let m = ready();
    let text = report(&m, &checks(&m));
    assert!(text.starts_with("Precept doctor · alpha\n"));
    assert!(text.contains("Daemon"));
    assert!(!text.contains("/home/u"));
}
