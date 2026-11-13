//! Boundedness, correlation and cleanup regressions for the owned native-window
//! worker pipe. These exercise the harness only, against an owned fake worker. They
//! never drive a real window and never constitute native GUI acceptance.
use acp_native_acceptance::worker::{Cleanup, WorkerRpc};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};

const FAKE: &str = env!("CARGO_BIN_EXE_acp-fake-desktop-worker");
const GENEROUS: Duration = Duration::from_secs(20);
const TIGHT: Duration = Duration::from_millis(250);
/// A stalled call must finish inside this window, not merely "eventually".
const BOUNDED: Duration = Duration::from_secs(10);
/// Near the harness request ceiling, so a few writes can fill any OS pipe.
const BULK_TEXT: usize = 7_000;

fn worker(mode: &str, deadline: Duration) -> WorkerRpc {
    let mode = mode.to_owned();
    WorkerRpc::spawn(Path::new(FAKE), &[mode.as_str()])
        .expect("owned fake worker spawns")
        .with_deadline(deadline)
}

/// Every outcome is asserted, so an unjoined thread can never look like success.
fn clean_exit(worker: &mut WorkerRpc) -> Cleanup {
    let start = Instant::now();
    let done = worker.shutdown();
    assert!(
        done.reaped && done.reader_joined && done.writer_joined,
        "teardown reported incomplete cleanup: {done:?}"
    );
    assert!(
        start.elapsed() < BOUNDED,
        "teardown took {:?}",
        start.elapsed()
    );
    done
}

fn bulk_bytes() -> Vec<u8> {
    serde_json::to_vec(&json!({"action":"text","text":"x".repeat(BULK_TEXT)})).unwrap()
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
fn every_reply_names_the_request_and_ids_increase_monotonically() {
    let mut worker = worker("healthy", GENEROUS);
    for expected in 1..=3u64 {
        let reply: Value = worker
            .request(&json!({"action":"observe"}))
            .expect("correlated reply settles");
        assert_eq!(
            reply["rpc_id"].as_u64(),
            Some(expected),
            "correlation id must advance with each request"
        );
    }
    clean_exit(&mut worker);
}

#[test]
fn silent_worker_fails_inside_its_deadline_and_is_reaped() {
    let mut worker = worker("silent", TIGHT);
    let start = Instant::now();
    let reason = worker
        .request(&json!({"action":"key","key":"alt+shift+i"}))
        .expect_err("a silent worker must fail");
    assert!(reason.contains("reply deadline exceeded"), "{reason}");
    assert!(reason.contains("action=key"), "{reason}");
    assert!(worker.is_fatal());
    let later = worker
        .request(&json!({"action":"capture","name":"x.png"}))
        .expect_err("a faulted session stays unusable");
    assert!(later.contains("unusable"), "{later}");
    assert!(start.elapsed() < BOUNDED, "call took {:?}", start.elapsed());
    let done = worker.shutdown();
    assert!(
        done.reaped && done.reader_joined && done.writer_joined,
        "{done:?}"
    );
}

#[test]
fn stale_reply_for_an_older_request_cannot_settle_a_newer_one() {
    // Deterministic by construction: the fake emits the previous call's reply only
    // after this newer request has been written, so ordering alone proves the refusal.
    // No sleep, no inspection of internal queue state.
    let mut worker = worker("stale-after-next", GENEROUS);
    worker
        .request(&json!({"action":"observe"}))
        .expect("first request settles normally");
    let reason = worker
        .request(&json!({"action":"key","key":"alt+shift+i"}))
        .expect_err("the stale reply must be refused");
    assert!(reason.contains("correlation mismatch"), "{reason}");
    assert!(worker.is_fatal(), "session must fail closed: {reason}");
    let third = worker
        .request(&json!({"action":"capture","name":"x.png"}))
        .expect_err("a faulted session never adopts a late acknowledgement");
    assert!(third.contains("unusable"), "{third}");
    clean_exit(&mut worker);
}

#[test]
fn duplicate_reply_cannot_ack_the_next_action() {
    let mut worker = worker("duplicate", GENEROUS);
    worker
        .request(&json!({"action":"observe"}))
        .expect("first reply settles the request");
    let reason = worker
        .request(&json!({"action":"key","key":"alt+shift+i"}))
        .expect_err("a leftover reply must not be consumed as this action's ack");
    assert!(
        reason.contains("correlation mismatch") || reason.contains("unsolicited"),
        "unexpected refusal: {reason}"
    );
    assert!(worker.is_fatal());
    clean_exit(&mut worker);
}

#[test]
fn reply_naming_an_unknown_request_is_refused() {
    let mut worker = worker("unsolicited", GENEROUS);
    let reason = worker
        .request(&json!({"action":"observe"}))
        .expect_err("a reply to a request nobody sent must fail");
    assert!(reason.contains("correlation mismatch"), "{reason}");
    clean_exit(&mut worker);
}

#[test]
fn stalled_stdin_write_is_bounded_and_cancelable() {
    // The worker never reads stdin, so writes eventually fill the pipe. Each attempt
    // must still return inside its own deadline instead of blocking on a full pipe.
    let mut worker = worker("no-stdin-read", TIGHT);
    let payload = bulk_bytes();
    let mut refused = None;
    for attempt in 1..=200 {
        let start = Instant::now();
        let outcome = worker.write_probe(&payload, TIGHT);
        assert!(
            start.elapsed() < BOUNDED,
            "write attempt {attempt} blocked for {:?}",
            start.elapsed()
        );
        if let Err(reason) = outcome {
            refused = Some((attempt, reason));
            break;
        }
    }
    let (attempt, reason) = refused.expect("a never-reading worker must eventually refuse");
    assert!(
        reason.contains("write deadline exceeded") || reason.contains("write refused"),
        "unexpected write refusal on attempt {attempt}: {reason}"
    );
    let done = worker.shutdown();
    assert!(
        done.reaped && done.reader_joined && done.writer_joined,
        "a stalled write must still clean up: {done:?}"
    );
}

#[test]
fn held_stdout_from_an_unowned_child_still_joins_the_reader() {
    // The worker exits but leaves a child holding the pipe open, so EOF never arrives.
    // Teardown must not depend on it: both IO threads stop by flag and are joined.
    let mut worker = worker("hold-stdout", TIGHT);
    let start = Instant::now();
    let reason = worker
        .request(&json!({"action":"observe"}))
        .expect_err("no reply can arrive, so the call must fail");
    assert!(start.elapsed() < BOUNDED, "call took {:?}", start.elapsed());
    assert!(worker.is_fatal());
    let done = worker.shutdown();
    assert!(
        done.reader_joined && done.writer_joined,
        "a pipe held by an unowned child must not leak an IO thread: {done:?} cause: {reason}"
    );
}

#[test]
fn abnormal_worker_exit_is_refused_and_reaped() {
    let mut worker = worker("crash", GENEROUS);
    let reason = worker
        .request(&json!({"action":"observe"}))
        .expect_err("an abnormal exit yields no reply");
    assert!(reason.contains("worker eof"), "{reason}");
    let start = Instant::now();
    let done = worker.shutdown();
    assert!(done.reaped, "{done:?}");
    assert!(done.reader_joined && done.writer_joined, "{done:?}");
    assert!(start.elapsed() < BOUNDED);
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
    assert!(start.elapsed() < BOUNDED);
}

#[test]
fn late_reply_never_becomes_a_later_acknowledgement() {
    let mut worker = worker("late", TIGHT);
    let reason = worker
        .request(&json!({"action":"key","key":"alt+shift+i"}))
        .expect_err("a reply arriving after the deadline must not settle the request");
    assert!(reason.contains("reply deadline exceeded"), "{reason}");
    assert!(
        worker.is_fatal(),
        "a timed-out request must fault the session"
    );
    assert!(
        worker
            .request(&json!({"action":"capture","name":"x.png"}))
            .expect_err("session stays faulted")
            .contains("unusable")
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
            .expect("a refusal is still one well-formed correlated reply");
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
        .request(&json!({"action":"text","text":"x".repeat(9_000)}))
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
