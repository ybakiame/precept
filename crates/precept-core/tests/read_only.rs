use precept_core::*;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::PathBuf};

struct Memory {
    roots: BTreeMap<String, PathBuf>,
    prefs: Vec<Preference>,
    snapshot: Worktree,
    handoff: Option<Handoff>,
}
impl ProjectRegistry for Memory {
    fn project(&self, name: &str) -> Result<Project> {
        Ok(Project {
            name: name.into(),
            root: self.roots.get(name).ok_or(Error::UnknownProject)?.clone(),
        })
    }
}
impl PreferenceRepository for Memory {
    fn preferences(&self, _: &str) -> Result<Vec<Preference>> {
        Ok(self.prefs.clone())
    }
}
// Deliberately returns an unfiltered record: the resolver must check scope itself.
impl HandoffRepository for Memory {
    fn handoff(&self, _: &str, _: &str) -> Result<Handoff> {
        self.handoff.clone().ok_or(Error::HandoffNotFound)
    }
}
impl WorktreeInspector for Memory {
    fn inspect(&self, _: &Project, _: bool) -> Result<Worktree> {
        Ok(self.snapshot.clone())
    }
}
impl Memory {
    fn resolver(&self, project: &str) -> Result<ContextResolver<'_>> {
        ContextResolver::new(project, self, self, self, self, &documents::FileDocuments)
    }
}
fn fixture() -> (tempfile::TempDir, Memory, Value) {
    let data: Value = serde_json::from_str(include_str!("preference-resolution.v1.json")).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut roots = BTreeMap::new();
    for (name, marker) in data["projects"].as_object().unwrap() {
        let root = temp.path().join(name);
        fs::create_dir(&root).unwrap();
        fs::write(root.join("AGENTS.md"), marker.as_str().unwrap()).unwrap();
        roots.insert(name.clone(), root);
    }
    let prefs = data["preferences"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            serde_json::from_value(
                json!({"id":i+1,"scope":if p["scope"]=="global" {"global"} else {"project"},
            "project":if p["scope"]=="global" {Value::Null} else {p["scope"].clone()},
            "key":p["key"],"value":p["value"],"status":p["status"],"tags":["fixture"]}),
            )
            .unwrap()
        })
        .collect();
    (
        temp,
        Memory {
            roots,
            prefs,
            snapshot: Worktree::default(),
            handoff: None,
        },
        data,
    )
}
#[test]
fn shared_v1_preferences_documents_and_forbidden_markers() {
    let (_temp, mem, data) = fixture();
    assert_eq!(data["contract"], "precept.p1.preference-context.v1");
    for name in ["alpha", "beta"] {
        let resolver = mem.resolver(name).unwrap();
        let context = resolver.context(true).unwrap();
        let effective: BTreeMap<_, _> = context["preferences"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["key"].as_str().unwrap(),
                    json!({"value":p["value"],"scope":p["scope"]}),
                )
            })
            .collect();
        assert_eq!(
            serde_json::to_value(effective).unwrap(),
            data["expected_effective"][name]
        );
        assert_eq!(context["documents"][0]["content"], data["projects"][name]);
        if name == "alpha" {
            for marker in data["forbidden_alpha_strings"].as_array().unwrap() {
                assert!(!context.to_string().contains(marker.as_str().unwrap()));
            }
        }
    }
}

#[test]
fn effective_context_carries_tags_for_project_and_global_fallbacks() {
    let (_temp, mut mem, _) = fixture();
    mem.prefs = [
        (1, Scope::Global, None, "global", "shared-global"),
        (2, Scope::Global, None, "override", "global-fallback"),
        (
            3,
            Scope::Project,
            Some("alpha"),
            "override",
            "project-value",
        ),
    ]
    .into_iter()
    .map(|(id, scope, project, key, value)| Preference {
        id,
        scope,
        project: project.map(str::to_owned),
        key: key.into(),
        value: value.into(),
        status: Status::Confirmed,
        tags: vec![format!("tag-{id}")],
    })
    .collect();
    for project in ["alpha", "beta"] {
        for include_documents in [false, true] {
            let context = mem
                .resolver(project)
                .unwrap()
                .context(include_documents)
                .unwrap();
            let rows = context["preferences"].as_array().unwrap();
            let effective: BTreeMap<_, _> = rows
                .iter()
                .map(|row| (row["key"].as_str().unwrap(), row["value"].as_str().unwrap()))
                .collect();
            assert_eq!(effective["global"], "shared-global");
            assert_eq!(
                effective["override"],
                if project == "alpha" {
                    "project-value"
                } else {
                    "global-fallback"
                }
            );
            // Tags travel with each effective row and never change which wins.
            let tags: BTreeMap<_, _> = rows
                .iter()
                .map(|row| (row["key"].as_str().unwrap(), row["tags"].clone()))
                .collect();
            assert_eq!(tags["global"], json!(["tag-1"]));
            assert_eq!(
                tags["override"],
                if project == "alpha" {
                    json!(["tag-3"])
                } else {
                    json!(["tag-2"])
                }
            );
            assert!(rows.iter().all(|row| row.get("source").is_none()));
        }
    }
}
#[test]
fn shared_v1_denials_and_malformed_arguments() {
    let (_temp, mem, data) = fixture();
    let resolver = mem.resolver("alpha").unwrap();
    for call in data["must_reject_tool_calls"].as_array().unwrap() {
        assert!(
            resolver
                .read(call["name"].as_str().unwrap(), &call["arguments"])
                .is_err()
        );
    }
    for args in [
        Value::Null,
        json!([]),
        json!({"project_id":"beta"}),
        json!({"include_documents":1}),
    ] {
        assert!(resolver.read("get_effective_context", &args).is_err());
    }
    for args in [
        json!({}),
        json!({"task_id":1}),
        json!({"task_id":"t","project_id":"beta"}),
    ] {
        assert!(resolver.read("get_handoff", &args).is_err());
    }
    assert!(resolver.context(false).is_ok());
    assert!(matches!(mem.resolver("absent"), Err(Error::UnknownProject)));
}
#[test]
fn newest_confirmed_within_scope_wins_and_invalid_scope_never_leaks() {
    let (_temp, mut mem, _) = fixture();
    let mut newer = mem.prefs[0].clone();
    newer.id = 100;
    newer.value = "new-global".into();
    mem.prefs.push(newer.clone());
    newer.id = 101;
    newer.project = Some("beta".into()); // Malformed global record from an adapter.
    newer.value = "INVALID_SCOPE".into();
    mem.prefs.push(newer);
    let result = mem.resolver("alpha").unwrap().context(false).unwrap();
    assert!(result.to_string().contains("new-global"));
    assert!(!result.to_string().contains("INVALID_SCOPE"));
}
fn handoff() -> Handoff {
    serde_json::from_value(json!({"project":"alpha","task_id":"task","objective":"synthetic",
        "base_sha":"base","branch":"main","worktree_state":"clean","worktree_fingerprint":"saved",
        "completed":"","next_action":"inspect","validation":[{"check":"unit","status":"PASS"}],"updated_at":"synthetic"})).unwrap()
}
#[test]
fn handoff_checks_both_project_and_task_even_with_faulty_adapter() {
    let (_temp, mut mem, _) = fixture();
    mem.handoff = Some(handoff());
    assert_eq!(
        mem.resolver("beta").unwrap().handoff("task"),
        Err(Error::HandoffNotFound)
    );
    assert_eq!(
        mem.resolver("alpha").unwrap().handoff("other"),
        Err(Error::HandoffNotFound)
    );
    mem.handoff = None;
    assert_eq!(
        mem.resolver("alpha").unwrap().handoff("task"),
        Err(Error::HandoffNotFound)
    );
}
#[test]
fn handoff_unavailable_changed_matching_and_legacy_evidence() {
    let (_temp, mut mem, _) = fixture();
    mem.handoff = Some(handoff());
    let result = mem.resolver("alpha").unwrap().handoff("task").unwrap();
    assert_eq!(result["resume_warnings"].as_array().unwrap().len(), 5);
    assert!(result["current_repository"]["worktree_fingerprint"].is_null());
    mem.snapshot = Worktree {
        head: Some("base".into()),
        branch: Some("main".into()),
        dirty: Some(false),
        change_count: Some(0),
        worktree_fingerprint: Some("saved".into()),
    };
    let result = mem.resolver("alpha").unwrap().handoff("task").unwrap();
    assert_eq!(result["resume_warnings"].as_array().unwrap().len(), 1);
    mem.snapshot.worktree_fingerprint = Some("changed".into());
    assert!(
        mem.resolver("alpha")
            .unwrap()
            .handoff("task")
            .unwrap()
            .to_string()
            .contains("fingerprint changed")
    );
    mem.handoff.as_mut().unwrap().worktree_fingerprint = None;
    let result = mem.resolver("alpha").unwrap().handoff("task").unwrap();
    assert!(result.to_string().contains("no worktree fingerprint"));
    assert_eq!(result["validation"][0]["status"], "PASS");
}
#[test]
fn validation_dto_rejects_unknown_status() {
    assert!(serde_json::from_value::<Validation>(json!({"check":"x","status":"SUCCESS"})).is_err());
}
#[test]
fn documents_are_live_allowlisted_bounded_and_opt_in() {
    let (_temp, mem, _) = fixture();
    let root = &mem.roots["alpha"];
    fs::write(root.join("unlisted.txt"), "DO_NOT_READ").unwrap();
    fs::write(root.join("README.md"), vec![b'a'; 16384]).unwrap();
    let resolver = mem.resolver("alpha").unwrap();
    let result = resolver.context(false).unwrap();
    assert!(result["documents"][0].get("content").is_none());
    assert_eq!(result["documents"][1]["status"], "available");
    assert!(!result.to_string().contains("DO_NOT_READ"));
    fs::write(root.join("README.md"), vec![b'a'; 16385]).unwrap();
    fs::write(root.join("AGENTS.md"), [0xff, b'x']).unwrap();
    let result = resolver.context(true).unwrap();
    assert_eq!(result["documents"][1]["status"], "too_large");
    assert_eq!(result["documents"][0]["content"], "\u{fffd}x");
}
#[cfg(unix)]
#[test]
fn documents_deny_escaping_file_and_parent_symlinks() {
    let (_temp, mem, _) = fixture();
    let root = &mem.roots["alpha"];
    let outside = &mem.roots["beta"];
    fs::create_dir(outside.join("docs")).unwrap();
    fs::write(outside.join("docs/index.md"), "PRIVATE").unwrap();
    std::os::unix::fs::symlink(outside.join("AGENTS.md"), root.join("README.md")).unwrap();
    std::os::unix::fs::symlink(outside.join("docs"), root.join("docs")).unwrap();
    let result = mem.resolver("alpha").unwrap().context(true).unwrap();
    for doc in result["documents"].as_array().unwrap().iter().skip(1) {
        assert_eq!(doc["status"], "unavailable_or_outside_root");
    }
    assert!(!result.to_string().contains("BETA_PROJECT_CONTRACT"));
    assert!(!result.to_string().contains("PRIVATE"));
}

#[test]
fn injected_metadata_and_explicit_denial_errors() {
    struct Docs;
    impl DocumentRepository for Docs {
        fn metadata(&self, project: &Project, include: bool) -> Result<Vec<Value>> {
            assert_eq!(project.name, "alpha");
            Ok(vec![
                json!({"path":"AGENTS.md", "status":"injected", "included":include}),
            ])
        }
    }
    let (_temp, mem, _) = fixture();
    let resolver = ContextResolver::new("alpha", &mem, &mem, &mem, &mem, &Docs).unwrap();
    assert_eq!(
        resolver.context(false).unwrap()["documents"][0]["status"],
        "injected"
    );
    assert_eq!(
        resolver.read("get_effective_context", &json!({"project":"beta"})),
        Err(Error::ProjectOverride)
    );
    assert_eq!(
        resolver.read("preference_confirm", &json!({"id":1})),
        Err(Error::UnknownTool)
    );
}

#[test]
fn handoff_preserves_validation_provenance_without_relaxing_status() {
    let (_temp, mut mem, _) = fixture();
    let mut source = serde_json::to_value(handoff()).unwrap();
    let validation = json!([
        {"check":"unit", "status":"PASS", "environment":"linux", "revision":"synthetic-revision",
         "evidence":{"log":"synthetic-log-ref", "id":"evidence-id", "updated_at":"recorded",
                     "steps":["unit", {"passed":true}], "optional":null}, "attempts":2},
        {"check":"legacy", "status":"NOT_RUN"}
    ]);
    source["validation"] = validation.clone();
    mem.handoff = Some(serde_json::from_value(source.clone()).unwrap());
    let result = mem.resolver("alpha").unwrap().handoff("task").unwrap();
    assert_eq!(result["validation"], validation);
    assert_eq!(
        serde_json::to_value(mem.handoff.as_ref().unwrap()).unwrap(),
        source
    );
    source["validation"][0]["status"] = json!("SUCCESS");
    assert!(serde_json::from_value::<Handoff>(source).is_err());
}

#[test]
fn fingerprint_is_requested_only_after_valid_handoff_scope_check() {
    use std::cell::RefCell;
    struct Spy(RefCell<Vec<bool>>);
    impl WorktreeInspector for Spy {
        fn inspect(&self, _: &Project, fingerprint: bool) -> Result<Worktree> {
            self.0.borrow_mut().push(fingerprint);
            Ok(Worktree::default())
        }
    }
    let (_temp, mut memory, _) = fixture();
    memory.handoff = Some(handoff());
    let spy = Spy(RefCell::new(Vec::new()));
    let resolver = ContextResolver::new(
        "alpha",
        &memory,
        &memory,
        &memory,
        &spy,
        &documents::FileDocuments,
    )
    .unwrap();
    resolver.context(false).unwrap();
    resolver.context(true).unwrap();
    assert_eq!(*spy.0.borrow(), [false, false]);
    assert!(resolver.handoff("foreign").is_err());
    assert_eq!(spy.0.borrow().len(), 2);
    resolver.handoff("task").unwrap();
    assert_eq!(*spy.0.borrow(), [false, false, true]);
}
