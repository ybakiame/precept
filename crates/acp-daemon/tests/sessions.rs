//! Rust replacements for essential retired session-driver security assertions.
//! Real native daemon and deterministic fake agent; no vendor, credentials or model.
mod support;
use acp_ipc::{Client, Credential, Error};
use serde_json::{Value, json};
use std::{
    fs, thread,
    time::{Duration, Instant},
};
use support::{Daemon, Fixture, exe, logical, private_bytes};

struct Session {
    f: Fixture,
    before: Vec<u8>,
}
impl Session {
    fn new(modes: &[&str], ttl: u64) -> Self {
        let mut f = Fixture::created(false);
        let b = f.daemon.enable();
        f.register(&b, "alpha");
        f.daemon
            .mutate(&b, "session-suggest", support::suggest())
            .unwrap();
        f.daemon
            .control("profile.write.disable", json!({}))
            .unwrap();
        f.daemon.kill();
        private_bytes(&f.temp.path().join("agent.toml"), toml::to_string(&json!({"version":1,"programs":modes.iter().map(|mode|json!({"id":mode,"executable":exe("acp-fake-agent"),"args":[mode,"--test-context"],"env":{}})).collect::<Vec<_>>()})).unwrap().as_bytes());
        f.daemon = Daemon::launch_with_ttl(f.temp.path(), "sessions", Some(&f.db), true, ttl);
        f.daemon.ready();
        let before = logical(&f.db);
        Self { f, before }
    }
    fn params(&self) -> Value {
        json!({"session_api_version":2,"project":"alpha"})
    }
    fn call(&self, method: &str, extra: Value) -> acp_ipc::Result<Value> {
        let mut p = self.params();
        p.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        self.f.daemon.control(method, p)
    }
    fn view(&self) -> Value {
        self.call("session.get", json!({})).unwrap()["session"].clone()
    }
    fn wait(&self, states: &[&str], reaped: bool) -> Value {
        let end = Instant::now() + Duration::from_secs(15);
        loop {
            let v = self.view();
            assert!(!v.to_string().contains("SENSITIVE_AUTH_TEST"));
            assert!(!v.to_string().contains("_UNTRUSTED_SHOULD_NOT_BE_LOGGED"));
            if states.iter().any(|s| v["state"] == *s) && (!reaped || v["process_alive"] == false) {
                return v;
            }
            assert!(Instant::now() < end, "synthetic session state deadline");
            thread::sleep(Duration::from_millis(15));
        }
    }
    fn start(&self, mode: &str, share: bool) -> Value {
        self.call(
            "session.start",
            json!({"executable":mode,"confirmed":true,"share_context":share}),
        )
        .unwrap();
        self.wait(&["ready", "failed", "auth_required"], false)
    }
    fn action(&self, method: &str, v: &Value, extra: Value) -> acp_ipc::Result<Value> {
        let mut p = extra;
        p["session_id"] = v["session_id"].clone();
        self.call(method, p)
    }
    fn stop(&self, v: &Value) {
        self.action("session.stop", v, json!({})).unwrap();
        self.wait(&["stopped", "failed", "auth_required"], true);
    }
    fn credential(&self) -> Credential {
        let paths: Vec<_> = fs::read_dir(&self.f.daemon.location.runtime_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("session-")
            })
            .collect();
        assert_eq!(paths.len(), 1);
        acp_ipc::load_credential(&self.f.daemon.location, &paths[0]).unwrap()
    }
    fn revoked(&self, peer: &mut Client, credential: &Credential) {
        assert_eq!(
            self.f
                .daemon
                .runtime
                .block_on(peer.request("context.get", json!({}))),
            Err(Error::Unauthorized)
        );
        assert!(matches!(
            self.f.daemon.client(credential),
            Err(Error::Unauthorized)
        ));
        assert_eq!(
            self.call("session.context", json!({})).unwrap()["active"],
            false
        );
        assert_eq!(logical(&self.f.db), self.before);
    }
}

#[test]
fn explicit_start_scope_generation_and_unshared_default() {
    let s = Session::new(&["normal"], 300);
    assert_eq!(
        s.call(
            "session.start",
            json!({"executable":"normal","confirmed":false})
        ),
        Err(Error::InvalidRequest)
    );
    assert_eq!(
        s.call(
            "session.start",
            json!({"executable":"normal","confirmed":true,"mcpServers":[]})
        ),
        Err(Error::InvalidRequest)
    );
    assert_eq!(
        s.call(
            "session.start",
            json!({"executable":"absent","confirmed":true})
        ),
        Err(Error::Unauthorized)
    );
    let v = s.start("normal", false);
    assert_eq!(v["state"], "ready");
    assert_eq!(
        s.call("session.context", json!({})).unwrap()["active"],
        false
    );
    assert_eq!(
        s.action(
            "session.prompt",
            &json!({"session_id":"old"}),
            json!({"prompt":"synthetic"})
        ),
        Err(Error::NotFound)
    );
    s.action("session.prompt", &v, json!({"prompt":"synthetic"}))
        .unwrap();
    let done = s.wait(&["completed"], false);
    assert!(done["activity"]["response_chunks"].as_u64().unwrap() > 0);
    s.stop(&v);
    let next = s.start("normal", false);
    assert_ne!(v["session_id"], next["session_id"]);
    assert_eq!(
        s.action("session.stop", &v, json!({})),
        Err(Error::NotFound)
    );
    s.stop(&next);
    assert_eq!(logical(&s.f.db), s.before);
}

#[test]
fn malformed_initialization_auth_and_metadata_are_sanitized_and_reaped() {
    let modes = [
        "version",
        "timeout",
        "auth-malformed",
        "auth-duplicate",
        "auth-bad-id",
        "auth-bad-type",
        "auth-huge-list",
        "auth-bad-capability",
        "auth-rejected",
        "metadata-wrong-scope",
        "metadata-overflow",
    ];
    for chunk in modes.chunks(8) {
        let s = Session::new(chunk, 300);
        for mode in chunk {
            s.start(mode, false);
            let v = s.wait(&["failed"], true);
            assert!(
                v["failure"].is_string(),
                "missing sanitized failure for {mode}"
            );
            assert!(v["permissions"].as_array().unwrap().is_empty());
        }
        assert_eq!(logical(&s.f.db), s.before);
    }
}

#[test]
fn auth_required_is_observation_and_unsupported_reverse_auth_is_never_executed() {
    let s = Session::new(
        &[
            "auth",
            "auth-midturn",
            "auth-unknown-type",
            "auth-unsupported",
            "metadata-early",
        ],
        300,
    );
    s.start("auth", false);
    let auth = s.wait(&["auth_required"], true);
    assert_eq!(auth["authentication"]["status"], "required");
    assert!(auth["failure"].is_null());
    let v = s.start("auth-midturn", true);
    assert_eq!(v["state"], "ready");
    let c = s.credential();
    let mut peer = s.f.daemon.client(&c).unwrap();
    s.action("session.prompt", &v, json!({"prompt":"synthetic"}))
        .unwrap();
    s.wait(&["auth_required"], true);
    s.revoked(&mut peer, &c);
    for mode in ["auth-unknown-type", "auth-unsupported", "metadata-early"] {
        let v = s.start(mode, false);
        assert_eq!(v["state"], "ready");
        if mode == "auth-unknown-type" {
            assert_eq!(v["authentication"]["methods"], json!(["unsupported"]));
        }
        s.action("session.prompt", &v, json!({"prompt":"synthetic"}))
            .unwrap();
        s.wait(&["completed"], false);
        s.stop(&v);
    }
    assert_eq!(logical(&s.f.db), s.before);
}

#[test]
fn permission_is_specific_allow_once_or_default_deny_without_persistent_approval() {
    let s = Session::new(&["permission", "malformed-permission"], 300);
    for allow in [false, true] {
        let v = s.start("permission", false);
        s.action("session.prompt", &v, json!({"prompt":"synthetic"}))
            .unwrap();
        let waiting = s.wait(&["waiting_permission"], false);
        let request = &waiting["permissions"][0];
        assert_eq!(request["allow_once"], "once");
        assert_eq!(
            s.action(
                "session.permission",
                &v,
                json!({"request_id":request["request_id"],"option_id":"always"})
            ),
            Err(Error::InvalidRequest)
        );
        assert_eq!(
            s.action(
                "session.permission",
                &v,
                json!({"request_id":"stale","option_id":"once"})
            ),
            Err(Error::InvalidRequest)
        );
        let mut choice = json!({"request_id":request["request_id"]});
        if allow {
            choice["option_id"] = json!("once");
        }
        s.action("session.permission", &v, choice.clone()).unwrap();
        let done = s.wait(&["completed"], false);
        assert!(done["permissions"].as_array().unwrap().is_empty());
        assert!(done["events"].to_string().contains(if allow {
            "explicitly allowed once"
        } else {
            "Permission denied"
        }));
        assert_eq!(
            s.action("session.permission", &v, choice),
            Err(Error::InvalidRequest)
        );
        s.stop(&v);
    }
    let v = s.start("malformed-permission", false);
    s.action("session.prompt", &v, json!({"prompt":"synthetic"}))
        .unwrap();
    let denied = s.wait(&["completed"], false);
    assert!(denied["permissions"].as_array().unwrap().is_empty());
    assert!(denied["events"].to_string().contains("Permission denied"));
    s.stop(&v);
    assert_eq!(logical(&s.f.db), s.before);
}

#[test]
fn protocol_failures_revoke_established_peers_and_do_not_retain_untrusted_diagnostics() {
    let modes = [
        "malformed",
        "huge",
        "wrong-id",
        "wrong-session",
        "duplicate",
        "duplicate-response",
        "mcp-crash",
        "mcp-unsupported-notification",
    ];
    let s = Session::new(&modes, 300);
    for mode in modes {
        let v = s.start(mode, true);
        assert_eq!(v["state"], "ready");
        let c = s.credential();
        let mut peer = s.f.daemon.client(&c).unwrap();
        s.action("session.prompt", &v, json!({"prompt":"synthetic"}))
            .unwrap();
        s.wait(&["failed"], true);
        s.revoked(&mut peer, &c);
    }
}

#[test]
fn cancel_deadline_and_root_drift_revoke_without_blocking_explicit_stop() {
    #[allow(unused_mut)] // Windows injects drift into its owned fixture DB.
    let mut s = Session::new(&["mcp-cancel", "cancel-timeout", "mcp-passive"], 300);
    for mode in ["mcp-cancel", "cancel-timeout"] {
        let v = s.start(mode, true);
        assert_eq!(v["state"], "ready");
        let c = s.credential();
        let mut peer = s.f.daemon.client(&c).unwrap();
        s.action("session.prompt", &v, json!({"prompt":"synthetic"}))
            .unwrap();
        s.wait(&["running"], false);
        s.action("session.cancel", &v, json!({})).unwrap();
        // Assert revocation at Cancel acknowledgement, while this established
        // connection is still within the normal two-second idle frame deadline.
        s.revoked(&mut peer, &c);
        if mode == "cancel-timeout" {
            s.wait(&["failed"], true);
        } else {
            s.wait(&["completed"], false);
            s.stop(&v);
        }
    }
    let v = s.start("mcp-passive", true);
    let c = s.credential();
    let mut peer = s.f.daemon.client(&c).unwrap();
    let root = s.f.temp.path().join("alpha");
    #[cfg(unix)]
    fs::rename(&root, s.f.temp.path().join("old-alpha")).unwrap();
    #[cfg(windows)]
    {
        // Windows may pin a live child's cwd. Inject the equivalent unavailable
        // registered-root condition in this synthetic DB instead of changing OS
        // sharing/permissions or weakening the assertion.
        rusqlite::Connection::open(&s.f.db)
            .unwrap()
            .execute(
                "UPDATE projects SET root=?1 WHERE name='alpha'",
                [root.join("unavailable").to_str().unwrap()],
            )
            .unwrap();
        s.before = logical(&s.f.db);
    }
    // Full validation now refuses the missing registered root before the later
    // per-project binding check; that early failure must also revoke context.
    assert!(
        s.action("session.prompt", &v, json!({"prompt":"synthetic"}))
            .is_err()
    );
    s.revoked(&mut peer, &c);
    s.stop(&v);
}

#[test]
fn expired_permission_is_denied_and_cannot_be_approved_afterwards() {
    let s = Session::new(&["permission"], 300);
    let v = s.start("permission", false);
    s.action("session.prompt", &v, json!({"prompt":"synthetic"}))
        .unwrap();
    let waiting = s.wait(&["waiting_permission"], false);
    let request = waiting["permissions"][0]["request_id"].clone();
    // This is the real production expiry, not a relaxed test clock or bypass.
    thread::sleep(Duration::from_secs(31));
    let done = s.wait(&["completed"], false);
    assert!(done["permissions"].as_array().unwrap().is_empty());
    assert!(done["events"].to_string().contains("Permission denied"));
    assert_eq!(
        s.action(
            "session.permission",
            &v,
            json!({"request_id":request,"option_id":"once"})
        ),
        Err(Error::InvalidRequest)
    );
    s.stop(&v);
    assert_eq!(logical(&s.f.db), s.before);
}

#[test]
fn expiry_and_hard_restart_refuse_cached_session_credentials_and_old_generations() {
    let mut s = Session::new(&["mcp-passive"], 1);
    let v = s.start("mcp-passive", true);
    let c = s.credential();
    let mut peer = s.f.daemon.client(&c).unwrap();
    assert!(
        s.f.daemon
            .runtime
            .block_on(peer.request("context.get", json!({})))
            .is_ok()
    );
    thread::sleep(Duration::from_millis(1100));
    s.revoked(&mut peer, &c);
    s.stop(&v);
    let old = s.start("mcp-passive", true);
    let c = s.credential();
    s.f.daemon.kill();
    s.f.daemon = Daemon::launch_with_ttl(s.f.temp.path(), "sessions", Some(&s.f.db), true, 1);
    s.f.daemon.ready();
    assert!(matches!(s.f.daemon.client(&c), Err(Error::Unauthorized)));
    assert_eq!(
        s.action("session.stop", &old, json!({})),
        Err(Error::NotFound)
    );
    assert!(
        !fs::read_dir(&s.f.daemon.location.runtime_dir)
            .unwrap()
            .any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("session-"))
    );
    assert_eq!(logical(&s.f.db), s.before);
}
