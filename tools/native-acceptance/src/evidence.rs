//! Bounded, content-free evidence. A begun cell is never silently promoted to PASS.
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub const CELLS: &[&str] = &[
    "profile-owned-flow",
    "empty-queue",
    "project-paging",
    "global-paging",
    "stale-page-disclosure",
    "exact-inspect-provenance",
    "confirm-peer-demotion",
    "reject",
    "history-bounded",
    "deprecate",
    "delete-wrong-id",
    "delete-blind-enter",
    "delete-escape-reopen",
    "delete-exact-id",
    "deleted-content-history",
    "keyboard-restoration",
    "selected-id-change",
    "scope-change",
    "project-change",
    "profile-epoch-change",
    "profile-generation-change",
    "target-change",
    "source-change",
    "source-field-change",
    "status-change",
    "stale-revision",
    "altered-digest",
    "revoke-inspect-apply",
    "disconnect",
    "delayed-read-navigation",
    "unknown-no-auto-replay",
    "manual-exact-replay",
    "replay-changed-bytes",
    "replay-stale-epoch",
    "en-light-dark-compact-large",
    "zh-light-dark-compact-large",
    "tab-enter-space-alt",
];
struct Ledger {
    out: PathBuf,
    start: Instant,
    value: Value,
    active: Option<usize>,
    stream: fs::File,
    event_count: u64,
    stream_bytes: usize,
    owner_event: Option<u64>,
}
thread_local! { static LEDGER: RefCell<Option<Ledger>> = const { RefCell::new(None) }; }
fn stamp(start: Instant) -> Value {
    let utc = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    json!({"monotonic_ms":start.elapsed().as_millis(),"utc_unix_ms":utc.as_millis()})
}
fn flush(l: &Ledger) {
    fs::write(
        l.out.join("ledger.json"),
        serde_json::to_vec_pretty(&l.value).unwrap(),
    )
    .unwrap();
}
pub fn init(out: &Path, surface: &str, sha: &str, binaries: Value) {
    let start = Instant::now();
    let cells: Vec<_> = CELLS
        .iter()
        .map(|id| json!({"id":id,"status":"NOT_RUN","mechanism":null,"event_range":null}))
        .collect();
    let value = json!({"schema":1,"source_sha":sha,"surface":surface,"binaries":binaries,"timeline_start":stamp(start),"cells":cells,"events":"events.jsonl","limits":{"events":32768,"per_cell_events":2048,"event_bytes":4096,"stream_bytes":8388608},"render_limit":"GUI probes observe owned surface state; PNG pixels require independent visual review. CLI-only/fault-injection observations are not native render claims."});
    LEDGER.with(|slot| {
        let stream = fs::OpenOptions::new()
            .append(true)
            .create_new(true)
            .open(out.join("events.jsonl"))
            .unwrap();
        let l = Ledger {
            out: out.into(),
            start,
            value,
            active: None,
            stream,
            event_count: 0,
            stream_bytes: 0,
            owner_event: None,
        };
        flush(&l);
        *slot.borrow_mut() = Some(l);
    });
}
pub fn begin(id: &str, mechanism: &str) {
    LEDGER.with(|slot| {
        if let Some(l) = slot.borrow_mut().as_mut() {
            assert!(l.active.is_none(), "nested evidence cell");
            let time = stamp(l.start);
            let index = l.value["cells"]
                .as_array()
                .unwrap()
                .iter()
                .position(|c| c["id"] == id)
                .expect("declared cell");
            let cell = &mut l.value["cells"][index];
            assert_eq!(
                cell["status"], "NOT_RUN",
                "a failed attempt cannot be overwritten"
            );
            cell["status"] = json!("FAIL");
            cell["mechanism"] = json!(mechanism);
            cell["started"] = time;
            cell["event_range"] = json!({"first":l.event_count,"end_exclusive":null});
            cell["owned_ui_event"] = json!(l.owner_event);
            l.active = Some(index);
            flush(l);
        }
    });
}
pub fn clear_owner() {
    LEDGER.with(|slot| {
        if let Some(l) = slot.borrow_mut().as_mut() {
            l.owner_event = None;
            if let Some(index) = l.active {
                l.value["cells"][index]["owned_ui_event"] = Value::Null;
            }
            flush(l);
        }
    });
}
pub fn finish(status: &str) {
    assert!(["PASS", "FAIL", "BLOCKED"].contains(&status));
    LEDGER.with(|slot| {
        if let Some(l) = slot.borrow_mut().as_mut()
            && let Some(index) = l.active.take()
        {
            let time = stamp(l.start);
            let cell = &mut l.value["cells"][index];
            cell["status"] = json!(status);
            cell["finished"] = time;
            cell["event_range"]["end_exclusive"] = json!(l.event_count);
            flush(l);
        }
    });
}
pub fn event(kind: &str, observation: Value) {
    LEDGER.with(|slot| {
        if let Some(l) = slot.borrow_mut().as_mut() {
            assert!(l.event_count < 32768, "bounded evidence exhausted");
            if let Some(index) = l.active {
                let first = l.value["cells"][index]["event_range"]["first"].as_u64().unwrap();
                assert!(l.event_count - first < 2048, "bounded cell evidence exhausted");
            }
            if kind == "owned-ui" {
                l.owner_event = Some(l.event_count);
                if let Some(index) = l.active {
                    l.value["cells"][index]["owned_ui_event"] = json!(l.event_count);
                }
            }
            let event = json!({"index":l.event_count,"cell":l.active.map(|index|CELLS[index]),"time":stamp(l.start),"kind":kind,"observation":observation});
            let bytes = serde_json::to_vec(&event).unwrap();
            assert!(bytes.len() <= 4096 && l.stream_bytes + bytes.len() < 8 * 1024 * 1024, "bounded event stream exhausted");
            l.stream.write_all(&bytes).unwrap();
            l.stream.write_all(b"\n").unwrap();
            l.stream.flush().unwrap();
            l.stream_bytes += bytes.len() + 1;
            l.event_count += 1;
        }
    });
}
pub fn snapshot() -> Value {
    LEDGER.with(|slot| {
        slot.borrow().as_ref().map_or(Value::Null, |l| {
            flush(l);
            l.value.clone()
        })
    })
}
