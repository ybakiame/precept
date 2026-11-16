//! Independent daemon-process fault injection over the actual native listener.
//! Only the libtest executable contains this checkpoint; product builds do not.
use super::*;
use std::{fs, path::Path, process::Stdio, sync::OnceLock, thread, time::Instant};

// Reuse the existing synthetic fixture/client and owned-child cleanup. Some
// helpers are used only by the integration binaries, not this test module.
#[allow(dead_code)]
#[path = "../tests/support/mod.rs"]
mod support;
use support::{Daemon, exe};
#[allow(dead_code)]
#[path = "../tests/support/mcp.rs"]
mod mcp;

static CHECKPOINT: OnceLock<PathBuf> = OnceLock::new();

pub(super) fn before_context_finish() {
    if let Some(base) = CHECKPOINT.get() {
        checkpoint(base, "context-armed");
    }
}
fn checkpoint(base: &Path, marker: &str) {
    if fs::remove_file(base.join(marker)).is_err() {
        return;
    }
    fs::write(base.join("reached-pending"), std::process::id().to_string()).unwrap();
    fs::rename(base.join("reached-pending"), base.join("reached")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(4);
    while !base.join("release").exists() {
        assert!(Instant::now() < deadline, "checkpoint release deadline");
        thread::sleep(Duration::from_millis(5));
    }
}

pub(super) fn install_checkpoint(profiles: &profiles::Profiles) {
    if let Some(base) = CHECKPOINT.get() {
        let base = base.clone();
        *profiles.before_begin.lock().unwrap() = Some(Arc::new(move || {
            // Arming happens only after the fixture setup has committed. The
            // existing hook runs between initial and final admission checks.
            checkpoint(&base, "armed");
        }));
    }
}

fn location(base: &Path) -> Location {
    let runtime_dir = base.join("run");
    Location {
        endpoint: if cfg!(windows) {
            format!(
                "s3-checkpoint-{}",
                base.file_name().unwrap().to_string_lossy()
            )
        } else {
            runtime_dir
                .join("control.sock")
                .to_string_lossy()
                .into_owned()
        },
        runtime_dir,
    }
}

#[test]
#[ignore = "owned subprocess entry; exercised by s3_independent_daemon_admission_checkpoint"]
fn checkpoint_daemon_child() {
    let base = PathBuf::from(std::env::var_os("ACP_TEST_CHECKPOINT_ROOT").unwrap());
    CHECKPOINT.set(base.clone()).unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime
        .block_on(run(Args {
            location: location(&base),
            database: None,
            projects: vec!["alpha".into()],
            roots: vec![format!("alpha={}", base.join("alpha").to_str().unwrap())],
            agent_config: None,
            context_mcp: None,
            context_lease_seconds: 300,
        }))
        .unwrap();
}

#[test]
fn s3_independent_daemon_admission_checkpoint() {
    for action in ["confirm", "reject", "deprecate", "delete"] {
        let temp = tempfile::Builder::new()
            .prefix("acp-s3-checkpoint-")
            .tempdir()
            .unwrap();
        let base = temp.path().canonicalize().unwrap();
        let root = base.join("alpha");
        fs::create_dir(&root).unwrap();
        acp_ipc::transport::private_dir(&base.join("state"), true).unwrap();
        let db = base.join("state/profile.sqlite3");
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "process_tests::checkpoint_daemon_child",
            ])
            .env("ACP_TEST_CHECKPOINT_ROOT", &base)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        assert_ne!(child.id(), std::process::id());
        // This guard always terminates/reaps the exact owned child, including
        // assertion failures, before the private temporary directory is dropped.
        let mut daemon = support::Daemon {
            child,
            location: location(&base),
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap(),
        };
        daemon.ready();
        daemon
            .control("profile.create", json!({"path":db}))
            .unwrap();
        let binding = daemon.enable();
        let scope = json!({"kind":"project","name":"alpha"});
        for (operation, command) in [
            (
                "register",
                json!({"command":"register_project","name":"alpha","root":root,"expected":{"kind":"absent"}}),
            ),
            (
                "suggest",
                json!({"command":"suggest_preference","scope":scope,"key":"synthetic","value":"synthetic","source":"synthetic","origin":"agent_reported","expected":{"kind":"absent"}}),
            ),
        ] {
            daemon.call(&daemon.credential("operator-write.cap"), "profile.mutate", json!({"version":2,"binding":binding,"operation_id":operation,"command":command})).unwrap();
        }
        let inspect = |revision| {
            daemon
                .control(
                    "profile.review.inspect",
                    json!({"scope":scope,"id":1,"revision":revision}),
                )
                .unwrap()
        };
        let digest = inspect(1)["record"]["proposal_digest"].clone();
        let request = |operation: &str, action: &str, revision| json!({"version":2,"binding":binding,"operation_id":operation,"command":{"command":format!("{action}_preference"),"scope":scope,"id":1,"proposal_digest":digest,"channel":"cli","expected":{"kind":"revision","revision":revision}}});
        let credential = daemon.credential("operator-write.cap");
        let revision = if action == "deprecate" {
            daemon
                .call(
                    &credential,
                    "profile.mutate",
                    request("setup-confirm", "confirm", 1),
                )
                .unwrap();
            2
        } else {
            1
        };
        let before = support::logical(&db);
        let before_bytes = fs::read(&db).unwrap();
        let before_inspect = inspect(revision);
        fs::write(base.join("armed"), []).unwrap();
        let mut client = daemon.client(&credential).unwrap();
        daemon.runtime.block_on(async {
            let mutation = client.request("profile.mutate", request("drift", action, revision));
            let move_root = async {
                let deadline = Instant::now() + Duration::from_secs(3);
                while !base.join("reached").exists() {
                    assert!(
                        Instant::now() < deadline,
                        "independent daemon checkpoint deadline"
                    );
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                assert_eq!(
                    fs::read_to_string(base.join("reached")).unwrap(),
                    daemon.child.id().to_string()
                );
                fs::rename(&root, base.join("displaced")).unwrap();
                fs::write(base.join("release"), []).unwrap();
            };
            let (result, ()) = tokio::join!(mutation, move_root);
            assert_eq!(result, Err(Error::UnsafeRuntime));
        });
        assert_eq!(
            support::logical(&db),
            before,
            "no record, receipt or decision change"
        );
        assert_eq!(fs::read(&db).unwrap(), before_bytes);
        fs::rename(base.join("displaced"), &root).unwrap();
        assert_eq!(inspect(revision), before_inspect);
        daemon.control("profile.write.disable", json!({})).unwrap();
    }
}

#[test]
fn n_independent_daemon_contract_source_changes_refuse_publication() {
    for (action, use_mcp) in ["edit", "delete", "rename", "replace", "add", "root"]
        .into_iter()
        .flat_map(|action| [false, true].map(move |bridge| (action, bridge)))
    {
        let temp = tempfile::Builder::new()
            .prefix("acp-n-race-")
            .tempdir()
            .unwrap();
        let base = temp.path().canonicalize().unwrap();
        let root = base.join("alpha");
        fs::create_dir(&root).unwrap();
        acp_ipc::transport::private_dir(&base.join("state"), true).unwrap();
        let db = base.join("state/profile.sqlite3");
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "process_tests::checkpoint_daemon_child",
            ])
            .env("ACP_TEST_CHECKPOINT_ROOT", &base)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        assert_ne!(child.id(), std::process::id());
        let mut daemon = support::Daemon {
            child,
            location: location(&base),
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap(),
        };
        daemon.ready();
        daemon
            .control("profile.create", json!({"path":db}))
            .unwrap();
        let binding = daemon.enable();
        let credential = daemon.credential("operator-write.cap");
        for (operation, command) in [
            (
                "register",
                json!({"command":"register_project","name":"alpha","root":root,"expected":{"kind":"absent"}}),
            ),
            (
                "suggest",
                json!({"command":"suggest_preference","scope":{"kind":"global"},"key":"formatter","value":"personal","source":"PRIVATE_RACE_SOURCE","origin":"agent_reported","expected":{"kind":"absent"}}),
            ),
        ] {
            daemon.call(&credential,"profile.mutate",json!({"version":2,"binding":binding,"operation_id":operation,"command":command})).unwrap();
        }
        let record = daemon
            .control(
                "profile.review.inspect",
                json!({"scope":{"kind":"global"},"id":1,"revision":1}),
            )
            .unwrap();
        daemon.call(&credential,"profile.mutate",json!({"version":2,"binding":binding,"operation_id":"confirm","command":{"command":"confirm_preference","scope":{"kind":"global"},"id":1,"proposal_digest":record["record"]["proposal_digest"],"channel":"cli","expected":{"kind":"revision","revision":1}}})).unwrap();
        let source = root.join("ACP.contracts.md");
        let contract = "```acp-contracts-v1\n{\"schema_version\":1,\"rules\":[{\"id\":\"fmt\",\"revision\":1,\"review\":\"accepted\",\"key\":\"formatter\",\"allowed_values\":[\"rustfmt\"]}]}\n```\n";
        if action != "add" {
            fs::write(&source, contract).unwrap();
        }
        let before = support::logical(&db);
        let before_bytes = fs::read(&db).unwrap();
        fs::write(base.join("context-armed"), []).unwrap();
        let agent = daemon.credential("agent-alpha.cap");
        let mut client = daemon.client(&agent).unwrap();
        daemon.runtime.block_on(async {
            let read = async {
                if use_mcp {
                    let mut mcp = mcp::Mcp::start(&daemon, "alpha");
                    let reply = tokio::task::spawn_blocking(move || {
                        mcp.request(
                            "tools/call",
                            json!({"name":"get_effective_context","arguments":{}}),
                        )
                    })
                    .await
                    .unwrap();
                    if reply["result"]["isError"] == true {
                        Err(Error::Unavailable)
                    } else {
                        Ok(serde_json::from_str(
                            reply["result"]["content"][0]["text"].as_str().unwrap(),
                        )
                        .unwrap())
                    }
                } else {
                    client.request("context.get", json!({})).await
                }
            };
            let change = async {
                let deadline = Instant::now() + Duration::from_secs(3);
                while !base.join("reached").exists() {
                    assert!(Instant::now() < deadline, "context checkpoint deadline");
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                assert_eq!(
                    fs::read_to_string(base.join("reached")).unwrap(),
                    daemon.child.id().to_string()
                );
                match action {
                    "edit" => fs::write(&source, contract.replace("rustfmt", "changed")).unwrap(),
                    "delete" => fs::remove_file(&source).unwrap(),
                    "rename" => fs::rename(&source, root.join("moved.md")).unwrap(),
                    "replace" => {
                        fs::write(
                            root.join("new.md"),
                            contract.replace("rustfmt", "replacement"),
                        )
                        .unwrap();
                        fs::remove_file(&source).unwrap();
                        fs::rename(root.join("new.md"), &source).unwrap();
                    }
                    "add" => fs::write(&source, contract).unwrap(),
                    "root" => fs::rename(&root, base.join("displaced")).unwrap(),
                    _ => unreachable!(),
                }
                fs::write(base.join("release"), []).unwrap();
            };
            let (result, ()) = tokio::join!(read, change);
            if action == "root" {
                assert_eq!(
                    result,
                    Err(if use_mcp {
                        Error::Unavailable
                    } else {
                        Error::UnsafeRuntime
                    })
                );
            } else {
                let result = result.unwrap();
                assert_eq!(result["contract_resolution"]["status"], "source_changed");
                assert_eq!(result["preferences"], json!([]));
                assert_eq!(result["effective_contracts"], json!([]));
                assert!(!result.to_string().contains("PRIVATE_RACE_SOURCE"));
            }
        });
        assert_eq!(support::logical(&db), before);
        assert_eq!(fs::read(&db).unwrap(), before_bytes);
        if action == "root" {
            fs::rename(base.join("displaced"), &root).unwrap();
        }
    }
}
