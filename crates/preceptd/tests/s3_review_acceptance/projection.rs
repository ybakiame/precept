use super::support::{Fixture, exe, logical};
use precept_core::operator::{Channel, Origin};
use precept_write::{Command, Expected, Scope};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    process::{Child, Command as Process, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

/// Real MCP subprocess, with bounded replies and owned teardown. It talks only to
/// this test's real daemon and never opens the fixture database itself.
struct Mcp {
    child: Child,
    replies: mpsc::Receiver<Value>,
    reader: Option<thread::JoinHandle<()>>,
}
impl Mcp {
    fn start(f: &Fixture, project: &str) -> Self {
        let mut child = Process::new(exe("precept-mcp"))
            .arg("--runtime-dir")
            .arg(&f.daemon.location.runtime_dir)
            .arg("--endpoint")
            .arg(&f.daemon.location.endpoint)
            // Opened in the project's folder, as a client would.
            .current_dir(f.temp.path().join(project))
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

fn assert_projected_context(context: &Value, project: &str, private_markers: &[String]) {
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
            (
                row["key"].as_str().unwrap(),
                (row["value"].as_str().unwrap(), row["tags"].clone()),
            )
        })
        .collect();
    // Independent expected values, not equality to a second projection path.
    // Tags travel with the winning row, so agents can find relevant rules.
    let expected = if project == "alpha" {
        BTreeMap::from([
            ("fallback", ("global-fallback", json!(["s3-tag-3"]))),
            ("shared", ("alpha-choice", json!(["s3-tag-1"]))),
        ])
    } else {
        BTreeMap::from([
            ("fallback", ("global-fallback", json!(["s3-tag-3"]))),
            ("foreign", ("S3_BETA_PRIVATE_VALUE", json!(["s3-tag-2"]))),
            ("shared", ("global-choice", json!(["s3-tag-0"]))),
        ])
    };
    assert_eq!(actual, expected);
    let serialized = context.to_string();
    for private in private_markers {
        assert!(!serialized.contains(private), "operator-only marker leaked");
    }
    for private in [
        "S3_PENDING_VALUE",
        "S3_REJECTED_VALUE",
        "S3_DEPRECATED_VALUE",
        "proposal_digest",
        "receipt_commit_sequence",
        "s3-tag-4",
        "s3-tag-5",
        "s3-tag-6",
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
fn s3_only_confirmed_rows_and_their_tags_reach_real_ipc_or_mcp() {
    let f = Fixture::created();
    let binding = f.daemon.enable();
    f.register(&binding, "alpha");
    f.register(&binding, "beta");
    let mut private_markers = Vec::new();
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
        let tags = vec![format!("s3-tag-{index}")];
        let receipt = f
            .daemon
            .mutate(
                &binding,
                &format!("privacy-suggest-{index}"),
                Command::SuggestPreference {
                    rule: Default::default(),
                    scope: scope.clone(),
                    key: key.into(),
                    value: value.into(),
                    tags: tags.clone(),
                    origin: Origin::Agent,
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
        assert_eq!(preview["record"]["tags"], json!(tags));
        let digest = preview["record"]["proposal_digest"].as_str().unwrap();
        if status != "suggested" {
            let command = if status == "rejected" {
                Command::RejectPreference {
                    id,
                    scope: scope.clone(),
                    proposal_digest: digest.into(),
                    channel: Channel::Desktop,
                    expected: Expected::Revision(1),
                    note: None,
                }
            } else {
                Command::ConfirmPreference {
                    id,
                    scope: scope.clone(),
                    proposal_digest: digest.into(),
                    channel: Channel::Desktop,
                    expected: Expected::Revision(1),
                    note: None,
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
                        note: None,
                    },
                )
                .unwrap();
        }
        // Exact Operator review retains tags and provenance for all statuses.
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
        assert_eq!(exact["record"]["tags"], json!(tags));
        assert_eq!(exact["record"]["origin"], "agent");
        assert_eq!(exact["record"]["proposal_digest"], digest);
        assert!(exact["decisions"].as_array().unwrap().len() <= 2);
        private_markers.push(digest.into());
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
        assert_projected_context(&direct, project, &private_markers);
        let operator = f
            .daemon
            .control("context.get", json!({"project":project}))
            .unwrap();
        assert_projected_context(&operator, project, &private_markers);
        let mut mcp = Mcp::start(&f, project);
        assert_projected_context(&mcp.context(), project, &private_markers);
    }
    assert_eq!(logical(&f.db), before);
}
