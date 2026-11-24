#[path = "ownership/admission.rs"]
mod admission;
#[path = "ownership/concurrency.rs"]
mod concurrency;
#[path = "ownership/lifecycle.rs"]
mod lifecycle;
#[path = "ownership/recovery.rs"]
mod recovery;
#[path = "ownership/revocation.rs"]
mod revocation;
mod support;

use precept_core::operator::Channel;
use precept_ipc::{Access, Error};
use precept_write::{Command, Expected, Request, Scope, Snapshot};
use serde_json::{Value, json};
use std::{
    fs, thread,
    time::{Duration, Instant},
};
use support::*;
fn handoff(project: &str, expected: Expected) -> Command {
    Command::PutHandoff {
        project: project.into(),
        task_id: "shared".into(),
        objective: "synthetic objective".into(),
        completed: String::new(),
        next_action: "inspect".into(),
        validation_json: "[]".into(),
        snapshot: Snapshot {
            base_sha: String::new(),
            branch: String::new(),
            state: "unknown".into(),
            fingerprint: None,
        },
        expected,
    }
}
fn request(b: &precept_write::Binding, op: &str, command: Command) -> Value {
    serde_json::to_value(Request {
        version: 2,
        binding: b.clone(),
        operation_id: op.into(),
        command,
    })
    .unwrap()
}
fn denied(f: &Fixture, method: &str, params: Value, error: Error) {
    let before = logical(&f.db);
    assert_eq!(f.daemon.control(method, params), Err(error));
    assert_eq!(logical(&f.db), before);
}
/// The digest a real operator preview returns; never a guessed constant.
fn preview_digest(f: &Fixture, scope: &Scope, id: i64) -> String {
    f.daemon
        .control("profile.preferences.get", json!({"scope":scope,"id":id}))
        .unwrap()["record"]["proposal_digest"]
        .as_str()
        .unwrap()
        .to_owned()
}
