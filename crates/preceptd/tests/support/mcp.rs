//! Owned, bounded synthetic MCP stdio client.
use super::{Daemon, exe};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command as Process, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};
pub struct Mcp {
    child: Child,
    replies: mpsc::Receiver<Value>,
    reader: Option<thread::JoinHandle<()>>,
}
impl Mcp {
    /// `precept-mcp` started in `project`'s root, as a client opened in that
    /// folder would; a project without a working agent capability starts it
    /// at `/`, outside every project.
    pub fn start(daemon: &Daemon, project: &str) -> Self {
        // On its own thread and runtime: callers may be inside one already.
        let location = daemon.location.clone();
        let project = project.to_owned();
        let root = thread::spawn(move || {
            let credential = precept_ipc::load_credential(
                &location,
                &location.runtime_dir.join(format!("agent-{project}.cap")),
            )
            .ok()?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok()?;
            let answer = runtime
                .block_on(async {
                    precept_ipc::Client::connect(&location, &credential)
                        .await?
                        .request("project.root", json!({}))
                        .await
                })
                .ok()?;
            answer["root"].as_str().map(std::path::PathBuf::from)
        })
        .join()
        .unwrap()
        .unwrap_or_else(|| "/".into());
        Self::start_in(daemon, &root)
    }
    /// `precept-mcp` started in `dir`.
    pub fn start_in(daemon: &Daemon, dir: &std::path::Path) -> Self {
        let mut child = Process::new(exe("precept-mcp"))
            .arg("--runtime-dir")
            .arg(&daemon.location.runtime_dir)
            .arg("--endpoint")
            .arg(&daemon.location.endpoint)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let output = child.stdout.take().unwrap();
        let (tx, replies) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else { break };
                assert!(line.len() < 128 * 1024);
                if tx.send(serde_json::from_str(&line).unwrap()).is_err() {
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
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let mut params = params;
        params["_meta"] = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28"});
        writeln!(
            self.child.stdin.as_mut().unwrap(),
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})
        )
        .unwrap();
        self.child.stdin.as_mut().unwrap().flush().unwrap();
        self.replies
            .recv_timeout(Duration::from_secs(12))
            .expect("owned MCP deadline")
    }
    /// Sends `message` as is; waits for a reply only when it has an id.
    #[allow(dead_code)]
    pub fn send(&mut self, message: Value) -> Option<Value> {
        writeln!(self.child.stdin.as_mut().unwrap(), "{message}").unwrap();
        self.child.stdin.as_mut().unwrap().flush().unwrap();
        message.get("id").map(|_| {
            self.replies
                .recv_timeout(Duration::from_secs(12))
                .expect("owned MCP deadline")
        })
    }
    pub fn context(&mut self, args: Value) -> Value {
        let reply = self.request(
            "tools/call",
            json!({"name":"get_effective_context","arguments":args}),
        );
        assert_eq!(reply["result"]["isError"], false, "MCP context refused");
        serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }
}
impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(r) = self.reader.take() {
            let _ = r.join();
        }
    }
}
