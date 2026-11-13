//! Boundedness and cleanup regressions for the owned native-window worker pipe.
//! These exercise the harness only, against an owned fake worker. They never drive a
//! real window and never constitute native GUI acceptance.
use acp_native_acceptance::worker::{Cleanup, WorkerRpc};
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};

const FAKE: &str = env!("CARGO_BIN_EXE_acp-fake-desktop-worker");
const GENEROUS: Duration = Duration::from_secs(20);
const TIGHT: Duration = Duration::from_millis(250);

fn worker(mode: &str, deadline: Duration) -> WorkerRpc {
    let mode = mode.to_owned();
    WorkerRpc::spawn(Path::new(FAKE), &[mode.as_str()])
        .expect("owned fake worker spawns")
        .with_deadline(deadline)
}

fn clean_exit(worker: &mut WorkerRpc) -> Cleanup {
    let done = worker.shutdown();
    assert!(
        done.reaped && done.reader_joined,
        "owned worker or reader thread survived: {done:?}"
    );
    done
}

#[test]
fn healthy_reply_settles_each_request_once_and_joins_on_teardown() {
    let mut worker = worker("healthy", GENEROUS);
    let start = Instant::now();
    for action in ["init", "observe", "key", "capture"] {
        let reply = worker
            .request(&json!({"action":action}))
            .unwrap_or_else(|reason| panic!("{action} should settle: {reason}"));
        assert_eq!(reply["status"], "ok", "{action} reply");
    }
    assert!(!worker.is_fatal());
    assert!(start.elapsed() < Duration::from_secs(10));
    clean_exit(&mut worker);
}

#[test]
fn silent_worker_fails_inside_its_deadline_and_is_reaped() {
    let mut worker = worker("silent", TIGHT);
    let start = Instant::now();
    let reason = worker
        .request(&json!({"action":"key","key":"alt+shift+i"}))
        .expect_err("a silent worker must fail");
    assert!(reason.contains("deadline exceeded"), "{reason}");
    assert!(reason.contains("action=key"), "{reason}");
    assert!(worker.is_fatal());
    // The hung worker must not be reachable for a later action either.
    let later = worker
        .request(&json!({"action":"capture","name":"x.png"}))
        .expect_err("a faulted session stays unusable");
    assert!(later.contains("unusable"), "{later}");
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "bounded failure took {:?}",
        start.elapsed()
    );
    let done = worker.shutdown();
    assert!(done.reaped && done.reader_joined, "{done:?}");
}

#[test]
fn worker_eof_without_reply_is_refused() {
    let mut worker = worker("eof", GENEROUS);
    let reason = worker
        .request(&json!({"action":"observe"}))
        .expect_err("eof without a reply must fail");
    assert!(reason.contains("worker eof"), "{reason}");
    clean_exit(&mut worker);
}

#[test]
fn malformed_reply_is_refused_without_echoing_its_bytes() {
    let mut worker = worker("malformed", GENEROUS);
    let reason = worker
        .request(&json!({"action":"observe"}))
        .expect_err("a malformed line must fail");
    assert!(
        reason.contains("malformed json response bytes="),
        "{reason}"
    );
    assert!(
        !reason.contains("not-a-json"),
        "raw worker bytes leaked into evidence"
    );
    clean_exit(&mut worker);
}

#[test]
fn oversized_reply_is_refused_instead_of_buffered() {
    let mut worker = worker("oversized", GENEROUS);
    let reason = worker
        .request(&json!({"action":"observe"}))
        .expect_err("an unbounded reply must fail");
    assert!(reason.contains("oversized response line"), "{reason}");
    let start = Instant::now();
    clean_exit(&mut worker);
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[test]
fn duplicate_reply_cannot_ack_the_next_action() {
    let mut worker = worker("duplicate", GENEROUS);
    worker
        .request(&json!({"action":"observe"}))
        .expect("first reply settles the request");
    // The fake flushes both lines together; let the reader queue the extra one.
    std::thread::sleep(Duration::from_millis(300));
    let reason = worker
        .request(&json!({"action":"key","key":"alt+shift+i"}))
        .expect_err("a leftover reply must not be consumed as this action's ack");
    assert!(
        reason.contains("unsolicited worker response while idle"),
        "{reason}"
    );
    clean_exit(&mut worker);
}

#[test]
fn late_reply_never_becomes_a_later_acknowledgement() {
    let mut worker = worker("late", TIGHT);
    let reason = worker
        .request(&json!({"action":"key","key":"alt+shift+i"}))
        .expect_err("a reply arriving after the deadline must not settle the request");
    assert!(reason.contains("deadline exceeded"), "{reason}");
    assert!(
        worker
            .request(&json!({"action":"capture","name":"x.png"}))
            .expect_err("session stays faulted")
            .contains("unusable")
    );
    assert!(
        worker.is_fatal(),
        "a timed-out request must fault the session"
    );
    clean_exit(&mut worker);
}

#[test]
fn acknowledgement_missing_its_required_field_is_refused() {
    let mut worker = worker("shapeless", GENEROUS);
    let reason = worker
        .request(&json!({"action":"init","nonce":"0123456789abcdef"}))
        .expect_err("init without an owned capture directory must fail");
    assert!(
        reason.contains("init acknowledgement lacks images"),
        "{reason}"
    );
    clean_exit(&mut worker);
}

#[test]
fn reply_naming_another_window_is_refused() {
    let mut worker = worker("bad-nonce", GENEROUS);
    worker.expect_nonce("0123456789abcdef");
    let reason = worker
        .request(&json!({"action":"observe"}))
        .expect_err("a nonce we do not own must fail");
    assert!(reason.contains("mismatched response nonce"), "{reason}");
    clean_exit(&mut worker);
}

#[test]
fn explicit_refusal_is_a_result_and_keeps_the_session_usable() {
    let mut worker = worker("refuse", GENEROUS);
    for _ in 0..2 {
        let reply = worker
            .request(&json!({"action":"key","key":"alt+shift+i"}))
            .expect("a refusal is still one well-formed reply");
        assert_eq!(reply["status"], "refused");
        assert_eq!(reply["reason"], "synthetic refusal");
    }
    assert!(!worker.is_fatal());
    clean_exit(&mut worker);
}

#[test]
fn out_of_bounds_request_never_reaches_the_pipe() {
    let mut worker = worker("healthy", GENEROUS);
    let reason = worker
        .request(&json!({"action":"text","text":"x".repeat(9000)}))
        .expect_err("an oversized request line must be refused locally");
    assert!(reason.contains("request line out of bounds"), "{reason}");
    assert!(!worker.is_fatal(), "{:?}", worker.fatal_reason());
    worker
        .request(&json!({"action":"observe"}))
        .expect("the session survives a locally refused request");
    clean_exit(&mut worker);
}

#[test]
fn request_without_an_action_is_refused() {
    let mut worker = worker("healthy", GENEROUS);
    let reason = worker
        .request(&json!({"nonce":"0123456789abcdef"}))
        .expect_err("an actionless request must be refused");
    assert!(reason.contains("no action"), "{reason}");
    assert!(!worker.is_fatal());
    clean_exit(&mut worker);
}
