//! S3 consumer-visible gaps, not renamed S2 cases or native UI evidence.
//! Build workspace binaries first. Every daemon/root/Profile is owned synthetic.
//! Direct SQLite changes below are explicitly contained fixture/fault injection;
//! every subsequent process request still passes production validation.
mod support;

use acp_core::operator::{Channel, Origin, Receipt, RecordVersion, Snapshot};
use acp_ipc::Error;
use acp_write::{Binding, Command, Expected, Request, Scope};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command as Process, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use support::*;

fn envelope(binding: &Binding, operation: &str, command: Command) -> Value {
    serde_json::to_value(Request {
        version: 2,
        binding: binding.clone(),
        operation_id: operation.into(),
        command,
    })
    .unwrap()
}

fn inspect(f: &Fixture, id: i64, revision: i64) -> Value {
    f.daemon
        .control(
            "profile.review.inspect",
            json!({"scope":{"kind":"global"},"id":id,"revision":revision}),
        )
        .unwrap()
}

fn confirm(id: i64, digest: &str, channel: Channel) -> Command {
    Command::ConfirmPreference {
        id,
        scope: Scope::Global,
        proposal_digest: digest.into(),
        channel,
        expected: Expected::Revision(1),
    }
}

/// A bounded, owned real CLI process; panic paths also terminate and reap it.
struct Cli(Option<Child>);
impl Cli {
    fn start(f: &Fixture, binding: &Binding, operation: &str, args: &[&str]) -> Self {
        let child = Process::new(exe("acp-query"))
            .arg("--runtime-dir")
            .arg(&f.daemon.location.runtime_dir)
            .arg("--endpoint")
            .arg(&f.daemon.location.endpoint)
            .args(args)
            .args(["--profile-id", &binding.profile_id])
            .args(["--generation", &binding.generation])
            .args(["--owner-epoch", &binding.owner_epoch.to_string()])
            .args(["--operation-id", operation])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        Self(Some(child))
    }
    fn finish(mut self) -> (bool, Value) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while self.0.as_mut().unwrap().try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "owned CLI response deadline");
            thread::sleep(Duration::from_millis(5));
        }
        let output = self.0.take().unwrap().wait_with_output().unwrap();
        assert!(
            output.stderr.is_empty(),
            "CLI diagnostics must be sanitized"
        );
        assert!(output.stdout.len() < 65536);
        (
            output.status.success(),
            serde_json::from_slice(&output.stdout).expect("bounded CLI JSON"),
        )
    }
}
impl Drop for Cli {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn s3_concurrent_cli_cas_and_identical_replay_commit_one_decision() {
    let f = Fixture::created(false);
    let binding = f.daemon.enable();
    for same_operation in [false, true] {
        let proposed = f
            .daemon
            .mutate(
                &binding,
                if same_operation {
                    "propose-replay"
                } else {
                    "propose-cas"
                },
                suggest(),
            )
            .unwrap();
        let id = proposed["affected"][0]["record_id"]
            .as_str()
            .unwrap()
            .parse::<i64>()
            .unwrap();
        let exact = inspect(&f, id, 1);
        let digest = exact["record"]["proposal_digest"].as_str().unwrap();
        let id_text = id.to_string();
        let args = [
            "preference-confirm",
            &id_text,
            "--scope",
            "global",
            "--expected-revision",
            "1",
            "--proposal-digest",
            digest,
            "--confirm",
        ];
        // Two independent CLI processes submit before either is waited on.
        let left_operation = if same_operation {
            "race-replay"
        } else {
            "race-left"
        };
        let left = Cli::start(&f, &binding, left_operation, &args);
        let right = Cli::start(
            &f,
            &binding,
            if same_operation {
                "race-replay"
            } else {
                "race-right"
            },
            &args,
        );
        let left = left.finish();
        let right = right.finish();
        if same_operation {
            assert!(left.0 && right.0);
            assert_eq!(left.1, right.1);
        } else {
            assert_ne!(left.0, right.0);
            assert_eq!(if left.0 { &right.1 } else { &left.1 }["error"], "Conflict");
        }
        let exact = inspect(&f, id, 2);
        assert_eq!(exact["record"]["status"], "confirmed");
        assert_eq!(exact["decisions"].as_array().unwrap().len(), 1);
        assert_eq!(exact["decisions"][0]["channel"], "cli");
        assert_eq!(exact["decisions"][0]["to_revision"], 2);
        assert_eq!(
            exact["decisions"][0]["receipt_commit_sequence"],
            exact["commit_sequence"]
        );
    }
}

#[test]
fn s3_session_review_denial_and_client_channels_never_supply_authority() {
    let f = Fixture::created(true);
    let binding = f.daemon.enable();
    f.register(&binding, "alpha");
    f.daemon.mutate(&binding, "proposal", suggest()).unwrap();
    let exact = inspect(&f, 1, 1);
    let digest = exact["record"]["proposal_digest"].as_str().unwrap();
    let params = json!({"session_api_version":acp_session::SESSION_API,"project":"alpha"});
    let mut start = params.clone();
    start["executable"] = json!("fake");
    start["confirmed"] = json!(true);
    start["share_context"] = json!(true);
    f.daemon.control("session.start", start).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let session = loop {
        let session = f.daemon.control("session.get", params.clone()).unwrap()["session"].clone();
        if session["state"] == "ready" {
            break session;
        }
        assert!(![json!("failed"), json!("auth_required")].contains(&session["state"]));
        assert!(Instant::now() < deadline, "owned fake session readiness");
        thread::sleep(Duration::from_millis(10));
    };
    let path = fs::read_dir(&f.daemon.location.runtime_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("session-")
        })
        .unwrap();
    let session_credential = acp_ipc::load_credential(&f.daemon.location, &path).unwrap();
    let mut peer = f.daemon.client(&session_credential).unwrap();
    let before = logical(&f.db);
    for (method, params) in [
        (
            "profile.review.queue",
            json!({"scope":{"kind":"global"},"after_id":0}),
        ),
        (
            "profile.review.inspect",
            json!({"scope":{"kind":"global"},"id":1,"revision":1}),
        ),
        (
            "profile.write.enable",
            json!({"profile_id":binding.profile_id,"generation":binding.generation,"confirm":true}),
        ),
    ] {
        assert_eq!(
            f.daemon.runtime.block_on(peer.request(method, params)),
            Err(Error::Unauthorized)
        );
    }
    for channel in [Channel::Cli, Channel::Desktop, Channel::Tui] {
        for credential in [
            &session_credential,
            &f.daemon.credential("operator.cap"),
            &f.daemon.credential("agent-alpha.cap"),
        ] {
            assert_eq!(
                f.daemon.call(
                    credential,
                    "profile.mutate",
                    envelope(&binding, "asserted-channel", confirm(1, digest, channel)),
                ),
                Err(Error::Unauthorized)
            );
        }
    }
    assert_eq!(logical(&f.db), before);
    let mut stop = params;
    stop["session_id"] = session["session_id"].clone();
    f.daemon.control("session.stop", stop).unwrap();
    f.daemon
        .control("profile.write.disable", json!({}))
        .unwrap();
    let before = logical(&f.db);
    assert_eq!(inspect(&f, 1, 1)["record"], exact["record"]);
    let queue = f
        .daemon
        .control(
            "profile.review.queue",
            json!({"scope":{"kind":"global"},"after_id":0}),
        )
        .unwrap();
    assert_eq!(queue["records"].as_array().unwrap().len(), 1);
    assert_eq!(logical(&f.db), before);
    assert!(
        !f.daemon
            .location
            .runtime_dir
            .join("operator-write.cap")
            .exists()
    );
}

#[test]
fn s3_stale_request_bindings_refuse_before_replay_and_strict_decode() {
    let f = Fixture::created(false);
    let binding = f.daemon.enable();
    let original = envelope(&binding, "committed", suggest());
    let write = f.daemon.credential("operator-write.cap");
    f.daemon
        .call(&write, "profile.mutate", original.clone())
        .unwrap();
    let before = logical(&f.db);
    for field in ["profile_id", "generation", "owner_epoch"] {
        let mut stale = original.clone();
        stale["binding"][field] = if field == "owner_epoch" {
            json!(binding.owner_epoch + 1)
        } else {
            let current = stale["binding"][field].as_str().unwrap();
            json!(if current == "a".repeat(32) {
                "b".repeat(32)
            } else {
                "a".repeat(32)
            })
        };
        assert_eq!(
            f.daemon.call(&write, "profile.mutate", stale),
            Err(Error::Unauthorized)
        );
    }
    for field in ["outer", "binding", "command"] {
        let mut unknown = original.clone();
        match field {
            "outer" => unknown["extra"] = json!(true),
            "binding" => unknown["binding"]["extra"] = json!(true),
            _ => unknown["command"]["extra"] = json!(true),
        }
        assert_eq!(
            f.daemon.call(&write, "profile.mutate", unknown),
            Err(Error::InvalidRequest)
        );
    }
    assert_eq!(
        f.daemon.control("profile.review.apply", json!({})),
        Err(Error::Unsupported)
    );
    assert_eq!(logical(&f.db), before);
}

fn saved_handoff(expected: Expected) -> Command {
    Command::PutHandoff {
        project: "alpha".into(),
        task_id: "capacity".into(),
        objective: "Synthetic capacity fixture".into(),
        completed: String::new(),
        next_action: "Inspect current evidence".into(),
        validation_json: r#"[{"check":"synthetic historical","status":"PASS"}]"#.into(),
        snapshot: Snapshot {
            base_sha: String::new(),
            branch: String::new(),
            state: "unknown".into(),
            fingerprint: None,
        },
        expected,
    }
}

/// Contained fixture injection, not 4094 production mutations or native evidence.
/// Build a complete structurally valid revision/receipt chain in one transaction.
fn seed_receipt_ceiling(f: &Fixture, binding: &Binding) -> (Request, Receipt) {
    let mut db = Connection::open(&f.db).unwrap();
    let tx = db.transaction().unwrap();
    let mut last = None;
    for sequence in 3..=4096 {
        let request = Request {
            version: 2,
            binding: binding.clone(),
            operation_id: format!("s3-capacity-{sequence}"),
            command: saved_handoff(Expected::Revision(sequence - 2)),
        };
        let receipt = Receipt {
            generation: binding.generation.clone(),
            operation_id: request.operation_id.clone(),
            commit_sequence: sequence,
            affected: vec![RecordVersion {
                kind: "handoff".into(),
                project: "alpha".into(),
                record_id: "capacity".into(),
                revision: sequence - 1,
                deleted: false,
            }],
        };
        tx.execute(
            "INSERT INTO acp_receipts VALUES(?1,?2,?3,?4,?5)",
            params![
                request.operation_id,
                binding.generation,
                Sha256::digest(serde_json::to_vec(&request).unwrap()).to_vec(),
                sequence,
                serde_json::to_string(&receipt).unwrap()
            ],
        )
        .unwrap();
        last = Some((request, receipt));
    }
    tx.execute_batch(
        "UPDATE handoffs SET revision=4095 WHERE project='alpha' AND task_id='capacity';
         UPDATE acp_profile SET commit_sequence=4096",
    )
    .unwrap();
    tx.commit().unwrap();
    // No validation seam is bypassed when admitting the injected fixture.
    let metadata = acp_store::SqliteStore::open(&f.db)
        .unwrap()
        .metadata()
        .unwrap();
    assert_eq!(metadata.commit_sequence, 4096);
    last.unwrap()
}

#[test]
fn s3_contained_4096_fixture_process_capacity_conflict_and_exact_replay() {
    let f = Fixture::created(false);
    let binding = f.daemon.enable();
    f.register(&binding, "alpha");
    f.daemon
        .mutate(
            &binding,
            "historical-handoff",
            saved_handoff(Expected::Absent),
        )
        .unwrap();
    let (request, receipt) = seed_receipt_ceiling(&f, &binding);
    let write = f.daemon.credential("operator-write.cap");
    let before = logical(&f.db);
    let replay = f
        .daemon
        .call(
            &write,
            "profile.mutate",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    assert_eq!(replay, serde_json::to_value(receipt).unwrap());
    let mut changed = request.clone();
    if let Command::PutHandoff {
        validation_json, ..
    } = &mut changed.command
    {
        validation_json.push(' ');
    }
    assert_eq!(
        f.daemon.call(
            &write,
            "profile.mutate",
            serde_json::to_value(changed).unwrap()
        ),
        Err(Error::IdempotencyConflict)
    );
    let denied = Cli::start(
        &f,
        &binding,
        "capacity-refused",
        &[
            "preference-suggest",
            "--scope",
            "global",
            "--key",
            "synthetic",
            "--value",
            "synthetic",
            "--source",
            "synthetic",
            "--origin",
            "agent_reported",
        ],
    )
    .finish();
    assert!(!denied.0);
    assert_eq!(denied.1["error"], "Capacity");
    assert_eq!(denied.1["operation_id"], "capacity-refused");
    assert_eq!(denied.1["outcome_unknown"], false);
    let saved = f
        .daemon
        .control(
            "handoff.get",
            json!({"project":"alpha","task_id":"capacity"}),
        )
        .unwrap();
    assert_eq!(saved["validation"][0]["status"], "PASS");
    assert!(
        saved["resume_warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| {
                warning
                    .as_str()
                    .unwrap()
                    .contains("recorded revision and environment")
            })
    );
    assert_eq!(logical(&f.db), before);
    f.daemon
        .control("profile.write.disable", json!({}))
        .unwrap();
    let next = f.daemon.enable();
    assert_eq!(next.owner_epoch, binding.owner_epoch + 1);
    let before = logical(&f.db);
    assert_eq!(
        f.daemon.call(
            &f.daemon.credential("operator-write.cap"),
            "profile.mutate",
            serde_json::to_value(request).unwrap(),
        ),
        Err(Error::Unauthorized)
    );
    assert_eq!(logical(&f.db), before);
}

#[cfg(unix)]
fn directory_link(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}
#[cfg(windows)]
fn directory_link(target: &Path, link: &Path) {
    // Directory junctions need no symlink privilege; both ends are owned fixtures.
    let output = Process::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "owned fixture junction creation refused"
    );
}

#[test]
fn s3_symlink_or_junction_swapped_root_and_foreign_registration_refuse() {
    let f = Fixture::created(false);
    let binding = f.daemon.enable();
    f.register(&binding, "alpha");
    let alpha = f.temp.path().join("alpha");
    let beta = f.temp.path().join("beta");
    let before = logical(&f.db);
    for (name, root) in [
        ("alpha", beta.canonicalize().unwrap()),
        ("outside", beta.canonicalize().unwrap()),
    ] {
        assert_eq!(
            f.daemon.mutate(
                &binding,
                "foreign-register",
                Command::RegisterProject {
                    name: name.into(),
                    root: root.to_str().unwrap().into(),
                    expected: Expected::Absent,
                },
            ),
            Err(Error::Unauthorized)
        );
    }
    fs::rename(&alpha, f.temp.path().join("alpha-original")).unwrap();
    directory_link(&beta, &alpha);
    assert_eq!(alpha.canonicalize().unwrap(), beta.canonicalize().unwrap());
    assert_eq!(
        f.daemon.control(
            "profile.review.queue",
            json!({"scope":{"kind":"project","name":"alpha"},"after_id":0}),
        ),
        Err(Error::UnsafeRuntime)
    );
    let mut proposal = suggest();
    if let Command::SuggestPreference { scope, .. } = &mut proposal {
        *scope = Scope::Project {
            name: "alpha".into(),
        };
    }
    assert_eq!(
        f.daemon.mutate(&binding, "swapped-root", proposal),
        Err(Error::UnsafeRuntime)
    );
    assert_eq!(logical(&f.db), before);
}

/// Real MCP subprocess, with bounded replies and owned teardown. It talks only to
/// this test's real daemon and never opens the fixture database itself.
struct Mcp {
    child: Child,
    replies: mpsc::Receiver<Value>,
    reader: Option<thread::JoinHandle<()>>,
}
impl Mcp {
    fn start(f: &Fixture, project: &str) -> Self {
        let mut child = Process::new(exe("acp-mcp"))
            .arg("--runtime-dir")
            .arg(&f.daemon.location.runtime_dir)
            .arg("--endpoint")
            .arg(&f.daemon.location.endpoint)
            .args(["--project", project, "--capability-file"])
            .arg(
                f.daemon
                    .location
                    .runtime_dir
                    .join(format!("agent-{project}.cap")),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, replies) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                assert!(line.len() < 128 * 1024, "bounded MCP fixture reply");
                let value = serde_json::from_str(&line).unwrap();
                if sender.send(value).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            replies,
            reader: Some(reader),
        }
    }
    fn context(&mut self) -> Value {
        let input = self.child.stdin.as_mut().unwrap();
        let request = json!({
            "jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{
                "_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"},
                "name":"get_effective_context","arguments":{}
            }
        });
        writeln!(input, "{request}").unwrap();
        input.flush().unwrap();
        let response = self
            .replies
            .recv_timeout(Duration::from_secs(12))
            .expect("owned MCP response deadline");
        assert_eq!(response["id"], 1);
        assert_eq!(response["result"]["isError"], false);
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }
}
impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn assert_projected_context(context: &Value, project: &str, private_sources: &[String]) {
    assert_eq!(context["project"], project);
    let rows = context["preferences"].as_array().unwrap();
    let actual: BTreeMap<_, _> = rows
        .iter()
        .map(|row| {
            assert_eq!(row["status"], "confirmed");
            for private in [
                "source",
                "origin",
                "proposal_digest",
                "decisions",
                "history",
            ] {
                assert!(row.get(private).is_none(), "private operator field leaked");
            }
            (row["key"].as_str().unwrap(), row["value"].as_str().unwrap())
        })
        .collect();
    // Independent expected values, not equality to a second projection path.
    let expected = if project == "alpha" {
        BTreeMap::from([("fallback", "global-fallback"), ("shared", "alpha-choice")])
    } else {
        BTreeMap::from([
            ("fallback", "global-fallback"),
            ("foreign", "S3_BETA_PRIVATE_VALUE"),
            ("shared", "global-choice"),
        ])
    };
    assert_eq!(actual, expected);
    let serialized = context.to_string();
    for private in private_sources {
        assert!(
            !serialized.contains(private),
            "private source marker leaked"
        );
    }
    for private in [
        "S3_PENDING_VALUE",
        "S3_REJECTED_VALUE",
        "S3_DEPRECATED_VALUE",
        "proposal_digest",
        "receipt_commit_sequence",
        "agent_reported",
    ] {
        assert!(
            !serialized.contains(private),
            "operator-only review data leaked"
        );
    }
    if project == "alpha" {
        assert!(!serialized.contains("S3_BETA_PRIVATE_VALUE"));
    } else {
        assert!(!serialized.contains("alpha-choice"));
    }
}

#[test]
fn s3_confirmed_private_sources_never_leak_through_real_ipc_or_mcp() {
    let f = Fixture::created(false);
    let binding = f.daemon.enable();
    f.register(&binding, "alpha");
    f.register(&binding, "beta");
    let mut private_sources = Vec::new();
    for (index, (project, key, value, status)) in [
        (None, "shared", "global-choice", "confirmed"),
        (Some("alpha"), "shared", "alpha-choice", "confirmed"),
        (
            Some("beta"),
            "foreign",
            "S3_BETA_PRIVATE_VALUE",
            "confirmed",
        ),
        (None, "fallback", "global-fallback", "confirmed"),
        (None, "pending", "S3_PENDING_VALUE", "suggested"),
        (None, "rejected", "S3_REJECTED_VALUE", "rejected"),
        (None, "deprecated", "S3_DEPRECATED_VALUE", "deprecated"),
    ]
    .into_iter()
    .enumerate()
    {
        let scope = project.map_or(Scope::Global, |name| Scope::Project { name: name.into() });
        let source = format!("S3_PRIVATE_SOURCE_{index}_NEVER_AGENT_VISIBLE");
        let receipt = f
            .daemon
            .mutate(
                &binding,
                &format!("privacy-suggest-{index}"),
                Command::SuggestPreference {
                    scope: scope.clone(),
                    key: key.into(),
                    value: value.into(),
                    source: source.clone(),
                    origin: Origin::AgentReported,
                    expected: Expected::Absent,
                },
            )
            .unwrap();
        let id = receipt["affected"][0]["record_id"]
            .as_str()
            .unwrap()
            .parse::<i64>()
            .unwrap();
        let preview = f
            .daemon
            .control("profile.preferences.get", json!({"scope":scope,"id":id}))
            .unwrap();
        assert_eq!(preview["record"]["source"], source);
        let digest = preview["record"]["proposal_digest"].as_str().unwrap();
        if status != "suggested" {
            let command = if status == "rejected" {
                Command::RejectPreference {
                    id,
                    scope: scope.clone(),
                    proposal_digest: digest.into(),
                    channel: Channel::Desktop,
                    expected: Expected::Revision(1),
                }
            } else {
                Command::ConfirmPreference {
                    id,
                    scope: scope.clone(),
                    proposal_digest: digest.into(),
                    channel: Channel::Tui,
                    expected: Expected::Revision(1),
                }
            };
            f.daemon
                .mutate(&binding, &format!("privacy-review-{index}"), command)
                .unwrap();
        }
        if status == "deprecated" {
            f.daemon
                .mutate(
                    &binding,
                    &format!("privacy-deprecate-{index}"),
                    Command::DeprecatePreference {
                        id,
                        scope: scope.clone(),
                        proposal_digest: digest.into(),
                        channel: Channel::Cli,
                        expected: Expected::Revision(2),
                    },
                )
                .unwrap();
        }
        // Exact Operator review retains source and provenance for all statuses.
        let revision = match status {
            "suggested" => 1,
            "deprecated" => 3,
            _ => 2,
        };
        let exact = f
            .daemon
            .control(
                "profile.review.inspect",
                json!({"scope":scope,"id":id,"revision":revision}),
            )
            .unwrap();
        assert_eq!(exact["record"]["source"], source);
        assert_eq!(exact["record"]["origin"], "agent_reported");
        assert_eq!(exact["record"]["proposal_digest"], digest);
        assert!(exact["decisions"].as_array().unwrap().len() <= 2);
        private_sources.push(source);
        private_sources.push(digest.into());
    }
    let before = logical(&f.db);
    for project in ["alpha", "beta"] {
        let direct = f
            .daemon
            .call(
                &f.daemon.credential(&format!("agent-{project}.cap")),
                "context.get",
                json!({}),
            )
            .unwrap();
        assert_projected_context(&direct, project, &private_sources);
        let operator = f
            .daemon
            .control("context.get", json!({"project":project}))
            .unwrap();
        assert_projected_context(&operator, project, &private_sources);
        let mut mcp = Mcp::start(&f, project);
        assert_projected_context(&mcp.context(), project, &private_sources);
    }
    assert_eq!(logical(&f.db), before);
}

#[test]
fn s3_contained_identity_corruption_never_rebinds_an_existing_writer() {
    for field in ["profile_id", "generation"] {
        let f = Fixture::created(false);
        let binding = f.daemon.enable();
        let original = if field == "profile_id" {
            &binding.profile_id
        } else {
            &binding.generation
        };
        let replacement = if original == &"a".repeat(32) {
            "b".repeat(32)
        } else {
            "a".repeat(32)
        };
        // Explicit contained fault injection into an empty, owned synthetic
        // Profile: structurally valid identity drift, not normal writer usage.
        let db = Connection::open(&f.db).unwrap();
        db.execute(&format!("UPDATE acp_profile SET {field}=?1"), [replacement])
            .unwrap();
        drop(db);
        acp_store::SqliteStore::open(&f.db).unwrap();
        let before = logical(&f.db);
        assert_eq!(
            f.daemon.control("profile.info", json!({})),
            Err(Error::OwnerMismatch)
        );
        assert_eq!(
            f.daemon.mutate(&binding, "stale-identity", suggest()),
            Err(Error::OwnerMismatch)
        );
        assert_eq!(logical(&f.db), before);
    }
}

#[test]
fn s3_intervening_commit_changes_page_watermark_without_snapshot_claim() {
    let f = Fixture::created(false);
    let binding = f.daemon.enable();
    for index in 0..13 {
        f.daemon
            .mutate(&binding, &format!("watermark-{index}"), suggest())
            .unwrap();
    }
    let first = f
        .daemon
        .control(
            "profile.review.queue",
            json!({"scope":{"kind":"global"},"after_id":0}),
        )
        .unwrap();
    assert_eq!(first["records"].as_array().unwrap().len(), 12);
    assert_eq!(first["next_after_id"], 12);
    f.daemon.mutate(&binding, "intervening", suggest()).unwrap();
    let second = f
        .daemon
        .control(
            "profile.review.queue",
            json!({"scope":{"kind":"global"},"after_id":first["next_after_id"]}),
        )
        .unwrap();
    let ids: Vec<_> = second["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| record["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, [13, 14]);
    assert_eq!(
        second["commit_sequence"].as_i64().unwrap(),
        first["commit_sequence"].as_i64().unwrap() + 1
    );
    assert!(second["next_after_id"].is_null());
}
