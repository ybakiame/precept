//! Deterministic fixture only: independently launch the advertised stdio server and call it.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdout, Command, Stdio},
};
pub struct Mcp {
    child: Child,
    output: BufReader<ChildStdout>,
}
impl Mcp {
    pub fn new(servers: &Value) -> Self {
        let servers = servers.as_array().unwrap();
        assert_eq!(servers.len(), 1);
        let s = &servers[0];
        assert_eq!(s["name"], "acp-context");
        assert_eq!(s["env"], json!([]));
        let command = s["command"].as_str().unwrap();
        assert!(std::path::Path::new(command).is_absolute());
        let args = s["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect::<Vec<_>>();
        let mut child = Command::new(command)
            .args(args)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut m = Self { child, output };
        let v=m.request(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"synthetic-fixture","version":"1"}}}));
        assert!(v.get("result").is_some());
        writeln!(
            m.child.stdin.as_mut().unwrap(),
            "{}",
            json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .unwrap();
        let v = m.request(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
        let tools = v["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "get_effective_context");
        assert_eq!(tools[1]["name"], "get_handoff");
        m
    }
    fn request(&mut self, v: Value) -> Value {
        let input = self.child.stdin.as_mut().unwrap();
        writeln!(input, "{v}").unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
    pub fn call_both(&mut self) {
        for (id, name, args) in [
            (
                3,
                "get_effective_context",
                json!({"include_documents":true}),
            ),
            (4, "get_handoff", json!({"task_id":"shared"})),
        ] {
            let v=self.request(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}}));
            assert_eq!(
                v["result"]["isError"], false,
                "synthetic scoped MCP call failed"
            );
        }
    }
}
impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
