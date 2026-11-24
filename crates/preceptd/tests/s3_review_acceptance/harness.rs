use super::support::{Fixture, exe};
use precept_write::{Binding, Command, Request};
use serde_json::{Value, json};
use std::{
    process::{Child, Command as Process, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(super) fn envelope(binding: &Binding, operation: &str, command: Command) -> Value {
    serde_json::to_value(Request {
        version: 2,
        binding: binding.clone(),
        operation_id: operation.into(),
        command,
    })
    .unwrap()
}

pub(super) fn inspect(f: &Fixture, id: i64, revision: i64) -> Value {
    f.daemon
        .control(
            "profile.review.inspect",
            json!({"scope":{"kind":"global"},"id":id,"revision":revision}),
        )
        .unwrap()
}

/// A bounded, owned real CLI process; panic paths also terminate and reap it.
pub(super) struct Cli(Option<Child>);
impl Cli {
    pub(super) fn start(f: &Fixture, binding: &Binding, operation: &str, args: &[&str]) -> Self {
        let child = Process::new(exe("precept"))
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
    pub(super) fn finish(mut self) -> (bool, Value) {
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
