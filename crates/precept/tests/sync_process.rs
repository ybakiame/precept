use precept_ipc::{Access, Client, Location, load_credential};
use serde_json::{Value, json};
use std::{
    ffi::OsStr,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Fixture {
    _temp: tempfile::TempDir,
    home: PathBuf,
    profile: PathBuf,
    location: Location,
    daemon: Child,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let temp = tempfile::Builder::new().prefix(name).tempdir().unwrap();
        let home = temp.path().join("home");
        let profile_dir = home.join("profile");
        let runtime = home.join(".local/state/precept/runtime");
        let state = home.join(".local/state/precept/daemon-state");
        fs::create_dir_all(&home).unwrap();
        precept_ipc::transport::private_dir(&profile_dir, true).unwrap();
        for directory in [
            home.join(".local"),
            home.join(".local/state"),
            home.join(".local/state/precept"),
            state.clone(),
        ] {
            precept_ipc::transport::private_dir(&directory, true).unwrap();
        }
        let mut projects =
            precept_ipc::transport::new_private(&state.join("projects.json")).unwrap();
        projects
            .write_all(br#"{"format":1,"projects":[]}"#)
            .unwrap();
        let endpoint = if cfg!(windows) {
            format!(
                "sync-{}",
                temp.path().file_name().unwrap().to_string_lossy()
            )
        } else {
            runtime.join("control.sock").to_string_lossy().into_owned()
        };
        let location = Location {
            runtime_dir: runtime,
            endpoint,
        };
        let daemon = Command::new(product_exe("preceptd"))
            .arg("--runtime-dir")
            .arg(&location.runtime_dir)
            .arg("--endpoint")
            .arg(&location.endpoint)
            .arg("--state-dir")
            .arg(&state)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("build workspace binaries before sync process tests");
        let fixture = Self {
            _temp: temp,
            home,
            profile: profile_dir.join("context.sqlite3"),
            location,
            daemon,
        };
        fixture.ready();
        fixture.json(["profile", "create", fixture.profile.to_str().unwrap()]);
        fixture.enable_writes();
        fixture
    }

    fn ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(12);
        while Instant::now() < deadline {
            let path = self.location.runtime_dir.join("operator.cap");
            if path.exists()
                && let Ok(credential) = load_credential(&self.location, &path)
                && credential.access == Access::Operator
            {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                let connected = runtime.block_on(async {
                    let mut client = Client::connect(&self.location, &credential).await?;
                    client.request("daemon.health", json!({})).await
                });
                if connected.is_ok() {
                    return;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("synthetic daemon readiness deadline");
    }

    fn precept(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_precept"));
        command
            .arg("--runtime-dir")
            .arg(&self.location.runtime_dir)
            .arg("--endpoint")
            .arg(&self.location.endpoint)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home);
        command
    }

    fn output<I, S>(&self, arguments: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.precept().args(arguments).output().unwrap()
    }

    fn json<I, S>(&self, arguments: I) -> Value
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.output(arguments);
        assert!(
            output.status.success(),
            "status: {:?}\nstdout: {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn binding(&self) -> Value {
        self.json(["profile", "info"])["binding"].clone()
    }

    fn authority(&self) -> Vec<String> {
        let binding = self.binding();
        vec![
            "--profile-id".into(),
            binding["profile_id"].as_str().unwrap().into(),
            "--generation".into(),
            binding["generation"].as_str().unwrap().into(),
            "--owner-epoch".into(),
            binding["owner_epoch"].to_string(),
        ]
    }

    fn enable_writes(&self) {
        let binding = self.binding();
        self.json([
            "profile",
            "write",
            "enable",
            "--profile-id",
            binding["profile_id"].as_str().unwrap(),
            "--generation",
            binding["generation"].as_str().unwrap(),
            "--confirm",
        ]);
    }

    fn confirm_global(&self, key: &str, value: &str, prefix: &str) {
        let mut args = vec![
            "preference-suggest".to_string(),
            "--scope".into(),
            "global".into(),
            "--key".into(),
            key.into(),
            "--value".into(),
            value.into(),
            "--tag".into(),
            "sync-test".into(),
            "--operation-id".into(),
            format!("{prefix}-suggest"),
        ];
        args.extend(self.authority());
        let receipt = self.json(args);
        let id = receipt["affected"][0]["record_id"]
            .as_str()
            .unwrap()
            .to_string();
        let inspected = self.json([
            "review-inspect",
            &id,
            "--scope",
            "global",
            "--revision",
            "1",
        ]);
        let digest = inspected["record"]["proposal_digest"].as_str().unwrap();
        let mut args = vec![
            "preference-confirm".to_string(),
            id,
            "--scope".into(),
            "global".into(),
            "--expected-revision".into(),
            "1".into(),
            "--proposal-digest".into(),
            digest.into(),
            "--confirm".into(),
            "--operation-id".into(),
            format!("{prefix}-confirm"),
        ];
        args.extend(self.authority());
        self.json(args);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

fn product_exe(name: &str) -> PathBuf {
    let current = std::env::current_exe().unwrap();
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    if let Some(profile) = current.ancestors().find(|path| {
        path.file_name()
            .is_some_and(|name| matches!(name.to_str(), Some("debug" | "release")))
    }) {
        let candidate = profile.join(&file);
        if candidate.exists() {
            return candidate;
        }
    }
    if let Some(deps) = current.parent()
        && let Some(target) = deps.parent()
    {
        let candidate = target.join(&file);
        if candidate.exists() {
            return candidate;
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug")
        .join(file)
}

#[test]
fn cli_pairs_devices_and_replays_an_exact_passphrase_import() {
    let sender = Fixture::new("precept-sync-a-");
    let receiver = Fixture::new("precept-sync-b-");
    let sender_invitation = sender.home.join("sender-invitation.json");
    let receiver_invitation = receiver.home.join("receiver-invitation.json");
    let a = sender.json(["sync", "init", "--out", sender_invitation.to_str().unwrap()]);

    let wrong_fingerprint = "0".repeat(32);
    let refused = receiver.output([
        "sync",
        "pair",
        "--invitation",
        sender_invitation.to_str().unwrap(),
        "--expected-owner-fingerprint",
        a["invitation"]["owner_fingerprint"].as_str().unwrap(),
        "--expected-invitation-fingerprint",
        &wrong_fingerprint,
    ]);
    assert_eq!(refused.status.code(), Some(3));
    assert_eq!(
        serde_json::from_slice::<Value>(&refused.stdout).unwrap()["error"],
        "InvalidRequest"
    );

    let paired = receiver.json([
        "sync",
        "pair",
        "--invitation",
        sender_invitation.to_str().unwrap(),
        "--expected-owner-fingerprint",
        a["invitation"]["owner_fingerprint"].as_str().unwrap(),
        "--expected-invitation-fingerprint",
        a["invitation_fingerprint"].as_str().unwrap(),
    ]);
    receiver.json([
        "sync",
        "invitation",
        "--out",
        receiver_invitation.to_str().unwrap(),
    ]);
    let b: Value = serde_json::from_slice(&fs::read(&receiver_invitation).unwrap()).unwrap();
    assert_eq!(paired["device_id"], b["invitation"]["device_id"]);
    sender.json([
        "sync",
        "trust",
        "--invitation",
        receiver_invitation.to_str().unwrap(),
        "--expected-owner-fingerprint",
        b["invitation"]["owner_fingerprint"].as_str().unwrap(),
        "--expected-invitation-fingerprint",
        b["invitation_fingerprint"].as_str().unwrap(),
    ]);

    sender.confirm_global("cli-sync-key", "cli sync value", "sender");
    let passphrase = sender.home.join("passphrase.txt");
    let mut file = precept_ipc::transport::new_private(&passphrase).unwrap();
    file.write_all(b"correct horse battery staple\n").unwrap();
    drop(file);
    let bundle = sender.home.join("bundle.json");
    sender.json([
        "sync",
        "export",
        "--out",
        bundle.to_str().unwrap(),
        "--passphrase-file",
        passphrase.to_str().unwrap(),
    ]);
    let prepared = receiver.home.join("prepared.json");
    receiver.json([
        "sync",
        "preview",
        bundle.to_str().unwrap(),
        "--passphrase-file",
        passphrase.to_str().unwrap(),
        "--out",
        prepared.to_str().unwrap(),
    ]);
    let prepared_text = fs::read_to_string(&prepared).unwrap();
    assert!(!prepared_text.contains("correct horse battery staple"));
    assert_eq!(
        serde_json::from_str::<Value>(&prepared_text).unwrap()["format"],
        "precept-personal-sync-prepared"
    );

    let apply = || {
        receiver.json([
            "sync",
            "apply",
            prepared.to_str().unwrap(),
            "--operation-id",
            "cli-import-1",
            "--passphrase-file",
            passphrase.to_str().unwrap(),
        ])
    };
    let first = apply();
    assert_eq!(first["applied"], 1);
    assert_eq!(first["changed"], 1);
    assert_eq!(apply(), first);
    let changed_operation = receiver.output([
        "sync",
        "apply",
        prepared.to_str().unwrap(),
        "--operation-id",
        "cli-import-2",
        "--passphrase-file",
        passphrase.to_str().unwrap(),
    ]);
    assert_eq!(changed_operation.status.code(), Some(3));
    assert_eq!(
        serde_json::from_slice::<Value>(&changed_operation.stdout).unwrap()["error"],
        "Conflict"
    );
    let missing_abort = receiver.output(["sync", "abort", "missing-operation"]);
    assert_eq!(missing_abort.status.code(), Some(3));
    let failure = serde_json::from_slice::<Value>(&missing_abort.stdout).unwrap();
    assert_eq!(failure["operation_id"], "missing-operation");
    assert_eq!(failure["outcome_unknown"], false);

    let records = receiver.json(["preferences", "list", "--scope", "global"]);
    let matches: Vec<_> = records["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|record| record["key"] == "cli-sync-key")
        .collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["value"], "cli sync value");
    assert_eq!(matches[0]["status"], "confirmed");
}
