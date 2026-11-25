//! Daemon-owned projects and the remembered startup Profile, through real
//! daemon processes and IPC (docs/architecture.md).
#[allow(dead_code)] // Shared harness; this suite uses part of it.
mod support;
use precept_ipc::Error;
use precept_write::{Command, Expected};
use serde_json::{Value, json};
use std::{fs, path::Path};
use support::*;

/// A Git repository root inside the test's home (the fixture base).
fn repository(base: &Path, name: &str) -> String {
    let root = base.join(name);
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join("AGENTS.md"), "Synthetic contract.\n").unwrap();
    root.canonicalize().unwrap().to_str().unwrap().to_owned()
}
fn names(f: &Fixture) -> Vec<(String, String)> {
    f.daemon
        .control("project.list", json!({}))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["name"].as_str().unwrap().into(),
                p["state"].as_str().unwrap().into(),
            )
        })
        .collect()
}
fn remove(f: &Fixture, name: &str) -> precept_ipc::Result<Value> {
    f.daemon.call(
        &f.daemon.credential("operator-write.cap"),
        "project.remove",
        json!({"name":name}),
    )
}

#[test]
fn check_reads_one_path_and_answers_a_fixed_reason() {
    let f = Fixture::created();
    let base = f.temp.path();
    let gamma = repository(base, "gamma");
    fs::create_dir(base.join("plain")).unwrap();
    let check = |root: &str| {
        f.daemon
            .control("project.check", json!({"root":root}))
            .unwrap()
    };
    let ok = check(&gamma);
    assert_eq!(
        ok,
        json!({"ok":true,"root":gamma,"name":"gamma","known":false})
    );
    for (root, reason) in [
        (
            base.join("plain").to_str().unwrap().to_owned(),
            "not_git_root",
        ),
        (
            base.join("missing").to_str().unwrap().to_owned(),
            "not_found",
        ),
        ("relative/path".into(), "not_absolute"),
        (base.to_str().unwrap().to_owned(), "is_home"),
        // The daemon's own state directory and a listed project.
        (
            base.join("daemon-state").to_str().unwrap().to_owned(),
            "not_git_root",
        ),
        (
            base.join("alpha").to_str().unwrap().to_owned(),
            "not_git_root",
        ),
    ] {
        assert_eq!(check(&root), json!({"ok":false,"reason":reason}), "{root}");
    }
    fs::create_dir(base.join("alpha/.git")).unwrap();
    assert_eq!(
        check(base.join("alpha").to_str().unwrap()),
        json!({"ok":false,"reason":"overlaps_project"})
    );
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir(outside.path().join(".git")).unwrap();
        std::os::unix::fs::symlink(outside.path(), base.join("escape")).unwrap();
        assert_eq!(
            check(base.join("escape").to_str().unwrap()),
            json!({"ok":false,"reason":"outside_home"})
        );
    }
    #[cfg(unix)]
    assert_eq!(check("/proc"), json!({"ok":false,"reason":"system_path"}));
    // Agents never check paths.
    assert_eq!(
        f.daemon.call(
            &f.daemon.credential("agent-alpha.cap"),
            "project.check",
            json!({"root":gamma})
        ),
        Err(Error::Unauthorized)
    );
}

fn listed_states(daemon: &Daemon) -> Vec<(String, String)> {
    daemon
        .control("project.list", json!({}))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|project| {
            (
                project["name"].as_str().unwrap().to_owned(),
                project["state"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn legacy_entries_without_root_identity_are_unavailable_without_repair() {
    let temp = tempfile::Builder::new()
        .prefix("precept-identity-")
        .tempdir()
        .unwrap();
    fs::create_dir(temp.path().join("alpha")).unwrap();
    fs::create_dir(temp.path().join("beta")).unwrap();
    let mut daemon = Daemon::launch(temp.path(), "legacy-a", None);
    daemon.ready();
    daemon.kill();
    let path = temp.path().join("daemon-state/projects.json");
    let mut state: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    for project in state["projects"].as_array_mut().unwrap() {
        project.as_object_mut().unwrap().remove("identity");
    }
    fs::remove_file(&path).unwrap();
    private_bytes(&path, state.to_string().as_bytes());
    let before = fs::read(&path).unwrap();
    let mut daemon = Daemon::relaunch(temp.path(), "legacy-b");
    daemon.ready();
    assert_eq!(
        listed_states(&daemon),
        vec![
            ("alpha".into(), "unavailable".into()),
            ("beta".into(), "unavailable".into())
        ]
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn a_replaced_root_is_unavailable_to_metadata_and_agent_reads_without_restart() {
    let f = Fixture::created();
    let binding = f.daemon.enable();
    f.register(&binding, "alpha");
    let root = f.temp.path().join("alpha");
    let moved = f.temp.path().join("alpha-old");
    fs::rename(&root, &moved).unwrap();
    fs::create_dir(&root).unwrap();
    assert!(listed_states(&f.daemon).contains(&("alpha".into(), "unavailable".into())));
    let agent = f.daemon.credential("agent-alpha.cap");
    assert_eq!(
        f.daemon.call(&agent, "project.root", json!({})),
        Err(Error::UnsafeRuntime)
    );
    let context = f.daemon.call(&agent, "context.get", json!({})).unwrap();
    assert_eq!(context["contract_resolution"]["status"], "source_changed");
    assert_eq!(context["repository_rules"], json!([]));
    fs::remove_dir(&root).unwrap();
    fs::rename(&moved, &root).unwrap();
    assert!(listed_states(&f.daemon).contains(&("alpha".into(), "ready".into())));
}

#[test]
fn a_replaced_root_is_unavailable_after_restart_even_at_the_same_path() {
    let temp = tempfile::Builder::new()
        .prefix("precept-identity-")
        .tempdir()
        .unwrap();
    fs::create_dir(temp.path().join("alpha")).unwrap();
    fs::create_dir(temp.path().join("beta")).unwrap();
    let mut daemon = Daemon::launch(temp.path(), "swap-a", None);
    daemon.ready();
    daemon.kill();
    fs::rename(temp.path().join("alpha"), temp.path().join("alpha-old")).unwrap();
    fs::create_dir(temp.path().join("alpha")).unwrap();
    let mut daemon = Daemon::relaunch(temp.path(), "swap-a");
    daemon.ready();
    assert_eq!(
        listed_states(&daemon),
        vec![
            ("beta".into(), "unregistered".into()),
            ("alpha".into(), "unavailable".into())
        ]
    );
}

fn add(f: &Fixture, name: &str, root: &str) -> precept_ipc::Result<Value> {
    f.daemon.call(
        &f.daemon.credential("operator-write.cap"),
        "project.add",
        json!({"name":name,"root":root}),
    )
}

#[test]
fn add_applies_at_once_persists_and_remove_revokes_without_touching_the_profile() {
    let mut f = Fixture::created();
    let base = f.temp.path().to_path_buf();
    let gamma = repository(&base, "gamma");
    // The operator read credential cannot add or remove; agents never can.
    for credential in ["operator.cap", "agent-alpha.cap"] {
        let c = f.daemon.credential(credential);
        for (method, params) in [
            ("project.add", json!({"name":"gamma","root":gamma})),
            ("project.remove", json!({"name":"alpha"})),
        ] {
            assert_eq!(
                f.daemon.call(&c, method, params),
                Err(Error::Unauthorized),
                "{credential}"
            );
        }
    }
    let b = f.daemon.enable();
    // Only the canonical root `project.check` returned is accepted.
    // (A `..` alias: Windows verbatim paths do not resolve it at all.)
    let alias = add(&f, "gamma", &format!("{gamma}/../gamma"));
    assert!(
        matches!(alias, Err(Error::InvalidRequest | Error::UnsafeRuntime)),
        "{alias:?}"
    );
    assert_eq!(
        add(&f, "plain", base.join("alpha").to_str().unwrap()),
        Err(Error::UnsafeRuntime)
    );
    assert_eq!(
        add(&f, "Alpha", &gamma),
        Err(Error::Conflict),
        "names collide case-insensitively"
    );
    let before = logical(&f.db);
    let added = add(&f, "gamma", &gamma).unwrap();
    assert_eq!(added["registered"], true);
    assert_ne!(logical(&f.db), before, "registered in the Profile");
    assert!(names(&f).contains(&("gamma".into(), "ready".into())));
    // Its agent credential exists at once and reads its context.
    let agent = f.daemon.credential("agent-gamma.cap");
    assert!(f.daemon.call(&agent, "context.get", json!({})).is_ok());
    let saved: Value =
        serde_json::from_str(&fs::read_to_string(base.join("daemon-state/projects.json")).unwrap())
            .unwrap();
    let saved_gamma = saved["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|project| project["name"] == "gamma")
        .unwrap();
    assert_eq!(saved_gamma["root"], json!(gamma));
    assert_eq!(saved_gamma["agents"], true);
    assert!(saved_gamma["identity"].is_object());
    assert_eq!(
        add(&f, "gamma", &gamma),
        Err(Error::Conflict),
        "already in the set"
    );
    // RegisterProject keeps its meaning: only a name already in the set.
    let delta = repository(&base, "delta");
    assert_eq!(
        f.daemon.mutate(
            &b,
            "register-delta",
            Command::RegisterProject {
                name: "delta".into(),
                root: delta,
                expected: Expected::Absent
            }
        ),
        Err(Error::Unauthorized)
    );
    let revoked_later = f.daemon.credential("agent-gamma.cap");
    // Persisted across a restart.
    f.daemon.kill();
    f.daemon = Daemon::relaunch(&base, "run-b");
    f.daemon.ready();
    f.daemon
        .control("profile.select", json!({"path":f.db}))
        .unwrap();
    assert!(names(&f).contains(&("gamma".into(), "ready".into())));
    // Remove: the set and the credential go; the Profile is not written.
    f.daemon.enable();
    let before = logical(&f.db);
    assert_eq!(remove(&f, "gamma").unwrap(), json!({"removed":"gamma"}));
    assert!(!names(&f).iter().any(|(n, _)| n == "gamma"));
    assert!(
        !f.daemon
            .location
            .runtime_dir
            .join("agent-gamma.cap")
            .exists()
    );
    // A token issued before the removal is refused on any new connection.
    assert_eq!(
        f.daemon.call(&revoked_later, "context.get", json!({})),
        Err(Error::Unauthorized)
    );
    assert_eq!(logical(&f.db), before, "remove never writes the Profile");
    assert_eq!(remove(&f, "gamma"), Err(Error::NotFound));
    // The Profile kept the record: the check names it, and adding it again
    // only rejoins the set (no second registration).
    let check = f
        .daemon
        .control("project.check", json!({"root":gamma}))
        .unwrap();
    assert_eq!(check["name"], "gamma");
    assert_eq!(check["known"], true);
    let again = add(&f, "gamma", &gamma).unwrap();
    assert_eq!(again["registered"], false);
    assert_eq!(
        logical(&f.db),
        before,
        "rejoining writes nothing to the Profile"
    );
    assert!(
        f.daemon
            .call(
                &f.daemon.credential("agent-gamma.cap"),
                "context.get",
                json!({})
            )
            .is_ok()
    );
}

#[test]
fn a_remembered_profile_restores_only_explicitly_enabled_writes_until_disabled_or_forgotten() {
    let mut f = Fixture::unselected();
    let base = f.temp.path().to_path_buf();
    let info = f
        .daemon
        .control("profile.create", json!({"path":f.db,"remember":true}))
        .unwrap();
    assert_eq!(info["remembered"], true);
    f.daemon.enable();
    f.daemon.kill();
    f.daemon = Daemon::relaunch(&base, "run-b");
    f.daemon.ready();
    let info = f.daemon.control("profile.info", json!({})).unwrap();
    assert_eq!(info["selected"], true);
    assert_eq!(info["remembered"], true);
    assert_eq!(info["write_enabled"], true);
    assert!(info["write_restore_failure"].is_null());

    f.daemon
        .control("profile.write.disable", json!({}))
        .unwrap();
    f.daemon.kill();
    f.daemon = Daemon::relaunch(&base, "run-c");
    f.daemon.ready();
    let info = f.daemon.control("profile.info", json!({})).unwrap();
    assert_eq!(info["selected"], true);
    assert_eq!(info["write_enabled"], false);

    f.daemon.enable();
    let info = f
        .daemon
        .control("profile.startup.forget", json!({}))
        .unwrap();
    assert_eq!(info["remembered"], false);
    assert_eq!(info["startup_profile"], false);
    f.daemon.kill();
    f.daemon = Daemon::relaunch(&base, "run-d");
    f.daemon.ready();
    let info = f.daemon.control("profile.info", json!({})).unwrap();
    assert_eq!(info["selected"], false);
    assert_eq!(info["write_enabled"], false);
    assert!(info["startup_failure"].is_null());
    // A remembered Profile that moved starts unselected and says why.
    f.daemon
        .control("profile.select", json!({"path":f.db,"remember":true}))
        .unwrap();
    f.daemon.kill();
    fs::rename(&f.db, base.join("state/moved.sqlite3")).unwrap();
    f.daemon = Daemon::relaunch(&base, "run-e");
    f.daemon.ready();
    let info = f.daemon.control("profile.info", json!({})).unwrap();
    assert_eq!(info["selected"], false);
    assert_eq!(info["startup_failure"], "startup_missing");
    assert!(!f.db.exists(), "never created again");
}

#[test]
fn damaged_state_refuses_to_start_and_is_left_as_is() {
    let f = Fixture::unselected();
    let base = f.temp.path().to_path_buf();
    drop(f.daemon);
    let projects = base.join("daemon-state/projects.json");
    fs::remove_file(&projects).unwrap();
    private_bytes(&projects, b"{\"format\":1,\"projects\":[],\"extra\":true}");
    let mut d = Daemon::relaunch(&base, "run-b");
    let status = d.child.wait().unwrap();
    assert_eq!(status.code(), Some(2));
    assert_eq!(
        fs::read(&projects).unwrap(),
        b"{\"format\":1,\"projects\":[],\"extra\":true}"
    );
}

#[test]
fn a_vanished_root_is_listed_unavailable_never_served_and_removable() {
    let mut f = Fixture::created();
    let base = f.temp.path().to_path_buf();
    let gamma = repository(&base, "gamma");
    f.daemon.enable();
    add(&f, "gamma", &gamma).unwrap();
    f.daemon.kill();
    fs::rename(base.join("gamma"), base.join("gamma-moved")).unwrap();
    f.daemon = Daemon::relaunch(&base, "run-b");
    f.daemon.ready();
    // The Profile still records gamma's old root: selection must not fail on it.
    f.daemon
        .control("profile.select", json!({"path":f.db}))
        .unwrap();
    assert!(names(&f).contains(&("gamma".into(), "unavailable".into())));
    assert!(
        !f.daemon
            .location
            .runtime_dir
            .join("agent-gamma.cap")
            .exists()
    );
    assert_eq!(
        f.daemon.control("context.get", json!({"project":"gamma"})),
        Err(Error::Unauthorized)
    );
    f.daemon.enable();
    remove(&f, "gamma").unwrap();
    assert!(!names(&f).iter().any(|(n, _)| n == "gamma"));
    let saved = fs::read_to_string(base.join("daemon-state/projects.json")).unwrap();
    assert!(!saved.contains("gamma"));
}

#[test]
fn the_project_set_is_listed_before_any_profile_is_selected() {
    let f = Fixture::unselected();
    // An operator connecting to a fresh daemon gets a successful read.
    let listed = f.daemon.control("project.list", json!({})).unwrap();
    assert_eq!(listed[0]["name"], "alpha");
    assert_eq!(listed[0]["state"], "unregistered");
    assert_eq!(listed.as_array().unwrap().len(), 2);
}

#[test]
fn a_listed_project_is_registered_in_a_new_profile_by_the_operator_only() {
    let f = Fixture::created();
    let register = |cap: &str, name: &str| {
        f.daemon.call(
            &f.daemon.credential(cap),
            "project.register",
            json!({"name":name}),
        )
    };
    // A fresh Profile knows none of the daemon's projects.
    assert!(names(&f).contains(&("alpha".into(), "unregistered".into())));
    assert_eq!(register("operator.cap", "alpha"), Err(Error::Unauthorized));
    f.daemon.enable();
    assert_eq!(
        register("operator-write.cap", "gamma"),
        Err(Error::NotFound)
    );
    let done = register("operator-write.cap", "alpha").unwrap();
    assert_eq!(done["registered"], "alpha");
    assert!(names(&f).contains(&("alpha".into(), "ready".into())));
    assert!(names(&f).contains(&("beta".into(), "unregistered".into())));
    // Once is enough; the Profile now names it.
    assert_eq!(
        register("operator-write.cap", "alpha"),
        Err(Error::Conflict)
    );
    assert!(
        f.daemon
            .control("context.get", json!({"project":"alpha"}))
            .is_ok()
    );
}

#[test]
fn an_unselected_daemon_suggests_a_profile_in_its_state_directory_and_never_creates_it() {
    let f = Fixture::unselected();
    let info = f.daemon.control("profile.info", json!({})).unwrap();
    let suggested = Path::new(info["suggested_path"].as_str().unwrap()).to_path_buf();
    assert_eq!(info["suggested_exists"], false);
    assert!(suggested.starts_with(f.temp.path().join("daemon-state").canonicalize().unwrap()));
    assert!(!suggested.exists(), "a suggestion is never created");
    // The operator's explicit create at the suggestion works and is selected.
    let created = f
        .daemon
        .control("profile.create", json!({"path":suggested,"remember":true}))
        .unwrap();
    assert_eq!(created["selected"], true);
    let info = f.daemon.control("profile.info", json!({})).unwrap();
    assert!(info.get("suggested_path").is_none());
    assert_eq!(info["remembered"], true);
}

#[test]
fn agent_access_is_switched_per_project_and_persists_without_touching_the_profile() {
    let mut f = Fixture::created();
    let base = f.temp.path().to_path_buf();
    let runtime = f.daemon.location.runtime_dir.clone();
    let listed = |f: &Fixture| f.daemon.control("project.list", json!({})).unwrap();
    assert_eq!(listed(&f)[0]["agents"], true, "shared by default");
    let agent = f.daemon.credential("agent-alpha.cap");
    assert!(f.daemon.call(&agent, "daemon.health", json!({})).is_ok());
    // The launch line: paths only, the exact capability file, never a token.
    let mcp = f
        .daemon
        .control("project.mcp", json!({"name":"alpha"}))
        .unwrap();
    let args: Vec<String> = mcp["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_owned())
        .collect();
    // One server for every project: no project in its arguments.
    assert_eq!(args.len(), 4);
    assert!(!args.iter().any(|a| a == "--project"));
    assert_eq!(mcp["project_args"][0], "--project");
    assert_eq!(mcp["project_args"][1], "alpha");
    assert!(
        mcp["project_args"][3]
            .as_str()
            .unwrap()
            .ends_with("agent-alpha.cap")
    );
    let token = fs::read_to_string(runtime.join("agent-alpha.cap")).unwrap();
    assert!(!mcp.to_string().contains(token.trim()));
    // Agents cannot switch or read the launch line.
    for (method, params) in [
        ("project.agents", json!({"name":"alpha","enabled":false})),
        ("project.mcp", json!({"name":"alpha"})),
    ] {
        assert_eq!(
            f.daemon.call(&agent, method, params),
            Err(Error::Unauthorized)
        );
    }
    // Off: the credential is withdrawn and a held token is refused at once.
    let before = logical(&f.db);
    f.daemon
        .control("project.agents", json!({"name":"alpha","enabled":false}))
        .unwrap();
    assert!(!runtime.join("agent-alpha.cap").exists());
    assert_eq!(
        f.daemon.call(&agent, "daemon.health", json!({})),
        Err(Error::Unauthorized)
    );
    assert_eq!(listed(&f)[0]["agents"], false);
    assert_eq!(
        f.daemon
            .control("project.mcp", json!({"name":"alpha"}))
            .unwrap()["shared"],
        false
    );
    assert_eq!(logical(&f.db), before, "switching never writes the Profile");
    // Persisted: a restart publishes no credential for it.
    f.daemon.kill();
    f.daemon = Daemon::relaunch(&base, "run-b");
    f.daemon.ready();
    let runtime = f.daemon.location.runtime_dir.clone();
    assert!(!runtime.join("agent-alpha.cap").exists());
    assert!(runtime.join("agent-beta.cap").exists() || listed(&f).as_array().unwrap().len() == 1);
    // On again: a fresh credential works.
    f.daemon
        .control("project.agents", json!({"name":"alpha","enabled":true}))
        .unwrap();
    f.daemon
        .control("profile.select", json!({"path":f.db}))
        .unwrap();
    let agent = f.daemon.credential("agent-alpha.cap");
    assert!(f.daemon.call(&agent, "daemon.health", json!({})).is_ok());
    assert_eq!(
        f.daemon
            .control("project.agents", json!({"name":"nope","enabled":true})),
        Err(Error::NotFound)
    );
}
