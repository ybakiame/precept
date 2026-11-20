//! S1 storage/resolver baseline measurement, not S2 daemon latency.
//! S2 adds selection/binding and root admission outside this sequence. This
//! fixture does not bypass or change those product checks or cache verification.
use super::*;
use precept_core::{ContextResolver, Worktree, WorktreeInspector, documents::FileDocuments};
use std::time::Instant;
#[path = "../tests/support/mod.rs"]
mod support;

struct UnavailableWorktree;
impl WorktreeInspector for UnavailableWorktree {
    fn inspect(&self, _: &Project, _: bool) -> precept_core::Result<Worktree> {
        Ok(Worktree::default())
    }
}

fn counted<T>(f: impl FnOnce() -> T) -> (T, usize) {
    profile::VERIFY_CALLS.with(|n| n.set(0));
    let result = f();
    (result, profile::VERIFY_CALLS.with(|n| n.get()))
}

fn context_query(path: &Path, session_bound: bool) -> serde_json::Value {
    // The actual open and resolver are used. Only Git is replaced with explicit
    // unavailable evidence; no shell, agent or external project is touched.
    let store = SqliteStore::open(path).unwrap();
    if session_bound {
        let root = store.read_project("alpha").unwrap().root;
        assert_eq!(root.canonicalize().unwrap(), root);
    }
    let resolver = ContextResolver::new(
        "alpha",
        &store,
        &store,
        &store,
        &UnavailableWorktree,
        &FileDocuments,
    )
    .unwrap();
    resolver.context(true).unwrap()
}

#[test]
fn s1_context_storage_sequence_counts_full_verification() {
    let t = tempfile::tempdir().unwrap();
    let path = support::fixture(&t.path().join("fixture"));
    for (session, expected) in [(false, 5), (true, 6)] {
        let (result, calls) = counted(|| context_query(&path, session));
        assert_eq!(calls, expected);
        assert_eq!(result["project"], "alpha");
        assert!(!result["preferences"].as_array().unwrap().is_empty());
        assert_eq!(result["repository"]["dirty"], serde_json::Value::Null);
    }
}

fn extend_history(path: &Path, target: i64) {
    let mut db = Connection::open(path).unwrap();
    let tx = db.transaction().unwrap();
    let metadata = profile::verify(&tx).unwrap();
    let mut revision: i64 = tx
        .query_row(
            "SELECT revision FROM handoffs WHERE project='alpha' AND task_id='shared'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    for sequence in metadata.commit_sequence + 1..=target {
        revision += 1;
        let receipt = profile::Receipt {
            generation: metadata.binding.generation.clone(),
            operation_id: format!("measurement-{sequence}"),
            commit_sequence: sequence,
            affected: vec![profile::RecordVersion {
                kind: "handoff".into(),
                project: "alpha".into(),
                record_id: "shared".into(),
                revision,
                deleted: false,
            }],
        };
        tx.execute(
            "INSERT INTO acp_receipts VALUES(?1,?2,zeroblob(32),?3,?4)",
            rusqlite::params![
                receipt.operation_id,
                receipt.generation,
                sequence,
                serde_json::to_string(&receipt).unwrap()
            ],
        )
        .unwrap();
    }
    tx.execute(
        "UPDATE handoffs SET revision=?1 WHERE project='alpha' AND task_id='shared'",
        [revision],
    )
    .unwrap();
    tx.execute("UPDATE acp_profile SET commit_sequence=?1", [target])
        .unwrap();
    profile::verify(&tx).unwrap();
    tx.commit().unwrap();
}

// Manual measurement, not a timing assertion or an ignored acceptance test.
// cargo test -p precept-store --release --lib synthetic_profile_read_cost -- --ignored --nocapture
#[test]
#[ignore = "opt-in synthetic latency measurement; no timing acceptance threshold"]
fn synthetic_profile_read_cost() {
    let t = tempfile::tempdir().unwrap();
    let path = support::fixture(&t.path().join("fixture"));
    for target in [None, Some(4095), Some(4096)] {
        if let Some(target) = target {
            extend_history(&path, target);
        }
        let store = SqliteStore::open(&path).unwrap();
        let receipts = store.metadata().unwrap().commit_sequence;
        let bytes = std::fs::metadata(&path).unwrap().len();
        for mode in ["verify", "open", "context", "session-context"] {
            let calls = match mode {
                "context" => 5,
                "session-context" => 6,
                _ => 1,
            };
            let mut samples = Vec::new();
            for i in 0..23 {
                let start = Instant::now();
                let (_, observed) = counted(|| match mode {
                    "verify" => {
                        std::hint::black_box(store.metadata().unwrap());
                    }
                    "open" => {
                        std::hint::black_box(SqliteStore::open(&path).unwrap());
                    }
                    _ => {
                        std::hint::black_box(context_query(&path, mode == "session-context"));
                    }
                });
                assert_eq!(observed, calls);
                if i >= 2 {
                    samples.push(start.elapsed().as_secs_f64() * 1000.0);
                }
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "synthetic receipts={receipts} bytes={bytes} mode={mode} verify_calls={calls} samples=21 median_ms={:.3} p95_ms={:.3} max_ms={:.3}",
                samples[10], samples[19], samples[20]
            );
        }
    }
}
