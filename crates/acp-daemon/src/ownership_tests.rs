//! Deterministic drain scheduling over real framing/dispatch and real SQLite.
//! The admission barrier exists only in cfg(test), never in a product daemon.
use super::*;
use std::sync::Barrier;

async fn open(
    service: &Service,
    credential: &acp_ipc::Credential,
) -> (tokio::io::DuplexStream, Value) {
    let (server, mut client) = tokio::io::duplex(8192);
    let permit = Arc::new(Arc::new(Semaphore::new(1)).acquire_owned().await.unwrap());
    tokio::spawn(peer(Box::new(server), service.clone(), permit));
    let token = serde_json::to_value(credential).unwrap()["token"].clone();
    write_frame(
        &mut client,
        &json!({"api_version":acp_ipc::API_VERSION,"credential":token}),
    )
    .await
    .unwrap();
    let ack = read_frame(&mut client).await.unwrap();
    (client, ack)
}
async fn send(stream: &mut tokio::io::DuplexStream, id: &str, method: &str, params: Value) {
    write_frame(
        stream,
        &Request {
            api_version: acp_ipc::API_VERSION,
            id: id.into(),
            method: method.into(),
            params,
        },
    )
    .await
    .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revoke_closes_admissions_drains_and_preserves_admitted_committed_response() {
    let t = tempfile::tempdir().unwrap();
    let state = t.path().join("state");
    acp_ipc::transport::private_dir(&state, true).unwrap();
    let db = state.join("profile.sqlite3");
    acp_write::Profile::create(&db).unwrap();
    let location = Location {
        runtime_dir: t.path().join("run"),
        endpoint: "unused".into(),
    };
    let mut runtime = RuntimeLease::acquire(&location).unwrap();
    let credentials = Arc::new(runtime.credentials(&[]).unwrap());
    let grants = acp_ipc::peer::Peers::default();
    let state = test_state(&t.path().join("daemon-state"), &[], Some(&db));
    let profiles = Arc::new(
        profiles::Profiles::new(
            location.clone(),
            state::StateDir::open(&state).unwrap(),
            grants.writes.clone(),
        )
        .unwrap(),
    );
    let service = Service {
        profiles: profiles.clone(),
        lease: Arc::new(Mutex::new(runtime)),
        credentials: Arc::new(RwLock::new(credentials.to_vec())),
        mcp_executable: None,
        grants,
        activity: crate::activity::Activity::default(),
    };
    profiles.select_remembered();
    let (mut operator, ack) = open(&service, &credentials[0]).await;
    assert_eq!(ack["ready"], true);
    send(&mut operator, "info", "profile.info", json!({})).await;
    let info = read_frame(&mut operator).await.unwrap()["result"].clone();
    send(&mut operator,"enable","profile.write.enable",json!({"profile_id":info["binding"]["profile_id"],"generation":info["binding"]["generation"],"confirm":true})).await;
    let enabled = read_frame(&mut operator).await.unwrap()["result"].clone();
    let c = acp_ipc::load_credential(&location, &location.runtime_dir.join("operator-write.cap"))
        .unwrap();
    let (mut writer, ack) = open(&service, &c).await;
    assert_eq!(ack["ready"], true);
    let (reached, observed) = std::sync::mpsc::channel();
    let release = Arc::new(Barrier::new(2));
    let barrier = release.clone();
    *profiles.before_begin.lock().unwrap() = Some(Arc::new(move || {
        reached.send(()).unwrap();
        barrier.wait();
    }));
    let mutation = json!({"version":2,"binding":enabled["binding"],"operation_id":"admitted","command":{"command":"suggest_preference","scope":{"kind":"global"},"key":"synthetic","value":"synthetic","tags":["synthetic"],"origin":"agent","expected":{"kind":"absent"}}});
    send(&mut writer, "write", "profile.mutate", mutation.clone()).await;
    tokio::task::spawn_blocking(move || observed.recv_timeout(Duration::from_secs(2)).unwrap())
        .await
        .unwrap();
    send(&mut operator, "disable", "profile.write.disable", json!({})).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let (_, ack) = open(&service, &c).await;
        if ack["error"] == "Unauthorized" {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    let mut disabled = Box::pin(read_frame(&mut operator));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut disabled)
            .await
            .is_err()
    );
    release.wait();
    let committed = read_frame(&mut writer).await.unwrap();
    assert_eq!(committed["error"], Value::Null);
    assert_eq!(committed["result"]["operation_id"], "admitted");
    assert_eq!(disabled.await.unwrap()["result"]["write_enabled"], false);
    assert!(!location.runtime_dir.join("operator-write.cap").exists());
    send(&mut writer, "after", "profile.mutate", mutation).await;
    assert_eq!(
        read_frame(&mut writer).await.unwrap()["error"],
        "Unauthorized"
    );
    let metadata = acp_store::SqliteStore::open(&db)
        .unwrap()
        .metadata()
        .unwrap();
    assert_eq!(metadata.commit_sequence, 1);
    assert_eq!(metadata.next_preference_id, 2);
    // Disable has drained the transaction and released the lease.
    acp_write::owner::Selected::select(&db)
        .unwrap()
        .acquire()
        .unwrap();
}

/// Checkpoint-controlled TOCTOU: real dispatch/framing/SQLite/filesystem, but an
/// in-process duplex peer and cfg(test) hook, NOT a native/process acceptance run.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn s3_root_drift_between_initial_check_and_final_admission_refuses_review() {
    for action in ["confirm", "reject", "deprecate", "delete"] {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("state");
        acp_ipc::transport::private_dir(&state, true).unwrap();
        let db = state.join("profile.sqlite3");
        acp_write::Profile::create(&db).unwrap();
        let root = temp.path().join("alpha");
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let displaced = temp.path().join("alpha-displaced");
        let location = Location {
            runtime_dir: temp.path().join("run"),
            endpoint: "unused".into(),
        };
        let mut runtime = RuntimeLease::acquire(&location).unwrap();
        let credentials = Arc::new(runtime.credentials(&["alpha".into()]).unwrap());
        let grants = acp_ipc::peer::Peers::default();
        let state = test_state(
            &temp.path().join("daemon-state"),
            &[("alpha", root.as_path())],
            Some(&db),
        );
        let profiles = Arc::new(
            profiles::Profiles::new(
                location.clone(),
                state::StateDir::open(&state).unwrap(),
                grants.writes.clone(),
            )
            .unwrap(),
        );
        let service = Service {
            profiles: profiles.clone(),
            lease: Arc::new(Mutex::new(runtime)),
            credentials: Arc::new(RwLock::new(credentials.to_vec())),
            mcp_executable: None,
            grants,
            activity: crate::activity::Activity::default(),
        };
        profiles.select_remembered();
        let operator = credentials
            .iter()
            .find(|credential| credential.access == Access::Operator)
            .unwrap();
        let (mut operator, ack) = open(&service, operator).await;
        assert_eq!(ack["ready"], true);
        send(&mut operator, "info", "profile.info", json!({})).await;
        let info = read_frame(&mut operator).await.unwrap()["result"].clone();
        send(
            &mut operator,
            "enable",
            "profile.write.enable",
            json!({
                "profile_id":info["binding"]["profile_id"],
                "generation":info["binding"]["generation"],
                "confirm":true
            }),
        )
        .await;
        let enabled = read_frame(&mut operator).await.unwrap()["result"].clone();
        let credential =
            acp_ipc::load_credential(&location, &location.runtime_dir.join("operator-write.cap"))
                .unwrap();
        let (mut writer, ack) = open(&service, &credential).await;
        assert_eq!(ack["ready"], true);
        let scope = json!({"kind":"project","name":"alpha"});
        for (operation, command) in [
            (
                "register",
                json!({"command":"register_project","name":"alpha","root":root,
                    "expected":{"kind":"absent"}}),
            ),
            (
                "suggest",
                json!({"command":"suggest_preference","scope":scope,"key":"synthetic",
                    "value":"synthetic","tags":["synthetic"],"origin":"agent",
                    "expected":{"kind":"absent"}}),
            ),
        ] {
            send(
                &mut writer,
                operation,
                "profile.mutate",
                json!({"version":2,"binding":enabled["binding"],
                    "operation_id":operation,"command":command}),
            )
            .await;
            assert_eq!(read_frame(&mut writer).await.unwrap()["error"], Value::Null);
        }
        send(
            &mut operator,
            "inspect",
            "profile.review.inspect",
            json!({"scope":scope,"id":1,"revision":1}),
        )
        .await;
        let inspected = read_frame(&mut operator).await.unwrap()["result"].clone();
        let digest = inspected["record"]["proposal_digest"].clone();
        let revision = if action == "deprecate" {
            send(
                &mut writer,
                "confirm",
                "profile.mutate",
                json!({"version":2,"binding":enabled["binding"],"operation_id":"confirm",
                    "command":{"command":"confirm_preference","scope":scope,"id":1,
                    "proposal_digest":digest,"channel":"cli",
                    "expected":{"kind":"revision","revision":1}}}),
            )
            .await;
            assert_eq!(read_frame(&mut writer).await.unwrap()["error"], Value::Null);
            2
        } else {
            1
        };
        let before = std::fs::read(&db).unwrap();
        let moved_root = root.clone();
        let moved_destination = displaced.clone();
        *profiles.before_begin.lock().unwrap() = Some(Arc::new(move || {
            // The initial admission/root check has already succeeded. The final
            // check must observe this change before starting the transaction.
            std::fs::rename(&moved_root, &moved_destination).unwrap();
        }));
        send(
            &mut writer,
            "drift",
            "profile.mutate",
            json!({"version":2,"binding":enabled["binding"],"operation_id":"drift",
                "command":{"command":format!("{action}_preference"),"scope":scope,"id":1,
                "proposal_digest":digest,"channel":"desktop",
                "expected":{"kind":"revision","revision":revision}}}),
        )
        .await;
        assert_eq!(
            read_frame(&mut writer).await.unwrap()["error"],
            "UnsafeRuntime"
        );
        assert!(displaced.is_dir(), "the precise admission hook must run");
        assert_eq!(std::fs::read(&db).unwrap(), before);
        *profiles.before_begin.lock().unwrap() = None;
        std::fs::rename(displaced, root).unwrap();
        send(
            &mut operator,
            "fresh-inspect",
            "profile.review.inspect",
            json!({"scope":scope,"id":1,"revision":revision}),
        )
        .await;
        let result = read_frame(&mut operator).await.unwrap();
        assert_eq!(result["error"], Value::Null);
        assert_eq!(result["result"]["record"]["revision"], revision);
        send(&mut operator, "disable", "profile.write.disable", json!({})).await;
        assert_eq!(
            read_frame(&mut operator).await.unwrap()["error"],
            Value::Null
        );
    }
}
