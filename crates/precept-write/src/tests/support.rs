use super::*;
pub(super) use crate::{files, profile::connect};
pub(super) use precept_store::{SqliteStore, profile as schema};
pub(super) use rusqlite::{Connection, types::Value};
pub(super) use std::{
    fs,
    path::Path,
    sync::Barrier,
    time::{Duration, Instant},
};
pub(super) fn fresh() -> (tempfile::TempDir, Profile) {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    let p = Profile::create(&t.path().join("profile.sqlite3")).unwrap();
    (t, p)
}
pub(super) fn request(p: &Profile, id: &str, command: Command) -> Request {
    Request {
        version: 2,
        binding: p.binding(),
        operation_id: id.into(),
        command,
    }
}
pub(super) fn suggest(scope: Scope) -> Command {
    Command::SuggestPreference {
        rule: Default::default(),
        scope,
        key: "formatter".into(),
        value: "synthetic".into(),
        tags: vec!["fixture".into()],
        origin: precept_core::operator::Origin::Agent,
        expected: Expected::Absent,
    }
}
/// The exact digest of the synthetic proposal `suggest()` inserts.
pub(super) fn proposal(scope: &Scope) -> String {
    precept_core::operator::digest_hex(&precept_core::operator::proposal_digest(
        scope,
        match scope {
            Scope::Project { name } => Some(name.as_str()),
            Scope::Global => None,
        },
        "formatter",
        "synthetic",
        precept_core::operator::Origin::Agent,
        &["fixture".to_owned()],
        &Default::default(),
    ))
}
pub(super) fn project_scope(name: &str) -> Scope {
    Scope::Project { name: name.into() }
}
pub(super) fn register(p: &Profile, name: &str) {
    let root = p.path.parent().unwrap().join(name);
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap().to_str().unwrap().to_owned();
    p.writer()
        .unwrap()
        .execute(&request(
            p,
            &format!("register-{name}"),
            Command::RegisterProject {
                name: name.into(),
                root,
                expected: Expected::Absent,
            },
        ))
        .unwrap();
}
pub(super) fn ready() -> (tempfile::TempDir, Profile) {
    let (t, p) = fresh();
    register(&p, "alpha");
    register(&p, "beta");
    (t, p)
}
pub(super) fn inserted(p: &Profile, op: &str, scope: Scope) -> i64 {
    p.writer()
        .unwrap()
        .execute(&request(p, op, suggest(scope)))
        .unwrap()
        .affected[0]
        .record_id
        .parse()
        .unwrap()
}
pub(super) fn transition(id: i64, scope: Scope, revision: i64, action: usize) -> Command {
    let proposal_digest = proposal(&scope);
    let channel = precept_core::operator::Channel::Cli;
    match action {
        0 => Command::ConfirmPreference {
            id,
            scope,
            proposal_digest,
            channel,
            expected: Expected::Revision(revision),
            note: None,
        },
        1 => Command::RejectPreference {
            id,
            scope,
            proposal_digest,
            channel,
            expected: Expected::Revision(revision),
            note: None,
        },
        _ => Command::DeprecatePreference {
            id,
            scope,
            proposal_digest,
            channel,
            expected: Expected::Revision(revision),
            note: None,
        },
    }
}
pub(super) fn handoff(project: &str, revision: Expected) -> Command {
    Command::PutHandoff{project:project.into(),task_id:"shared".into(),objective:"synthetic objective".into(),completed:"".into(),next_action:"inspect".into(),validation_json:"[ {\"check\":\"unit\",\"status\":\"NOT_RUN\",\"extra\":{\"opaque\":[1,null,\"合成\"]}} ]".into(),snapshot:Snapshot{base_sha:String::new(),branch:String::new(),state:"unknown".into(),fingerprint:None},expected:revision}
}
pub(super) fn dump(db: &Connection) -> Vec<Vec<Vec<Value>>> {
    [
        "SELECT * FROM acp_profile",
        "SELECT * FROM projects ORDER BY name",
        "SELECT * FROM preferences ORDER BY id",
        "SELECT * FROM handoffs ORDER BY project,task_id",
        "SELECT * FROM acp_receipts ORDER BY commit_sequence",
        "SELECT * FROM acp_review_decisions ORDER BY id",
    ]
    .iter()
    .map(|q| {
        let mut s = db.prepare(q).unwrap();
        let n = s.column_count();
        s.query_map([], |r| {
            (0..n)
                .map(|i| r.get(i))
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
    })
    .collect()
}
pub(super) fn validated(p: &Profile) {
    SqliteStore::open(&p.path).unwrap();
}
