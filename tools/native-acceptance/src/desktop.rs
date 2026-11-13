use acp_native_acceptance::worker::WorkerRpc;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
pub struct Desktop {
    gui: Child,
    worker: WorkerRpc,
    pub lines: Arc<Mutex<Vec<String>>>,
    after: usize,
    out: std::path::PathBuf,
}
impl Desktop {
    pub fn start(binary: &Path, args: &[String], nonce: &str, script: &str, out: &Path) -> Self {
        let mut command = Command::new(binary);
        command
            .args(args)
            .env("ACP_UI_PROBE", "1")
            .env("ACP_UI_PROBE_ID", nonce)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            command
                .env("GDK_BACKEND", "wayland")
                .env_remove("DISPLAY")
                .env("WAYLAND_DISPLAY", "wayland-0");
        }
        let mut child = command.spawn().unwrap();
        let stderr = child.stderr.take().unwrap();
        let lines = Arc::new(Mutex::new(Vec::new()));
        let log = lines.clone();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if line.starts_with("ACP_") {
                    let mut log = log.lock().unwrap();
                    assert!(log.len() < 50000);
                    log.push(line);
                }
            }
        });
        let powershell = if cfg!(windows) {
            "powershell.exe"
        } else {
            "/mnt/c/Program Files/PowerShell/7/pwsh.exe"
        };
        let mut worker = WorkerRpc::spawn(
            Path::new(powershell),
            &[
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                script,
            ],
        )
        .unwrap();
        let response = worker
            .request(&json!({"action":"init","nonce":nonce,"pid":child.id(),"remote":cfg!(unix)}))
            .unwrap_or_else(|reason| panic!("native window init refused: {reason}"));
        assert_eq!(response["status"], "ok");
        let (stdin_mode, stdout_mode) = worker.io_modes();
        println!("DESKTOP_IO_MODE stdin={stdin_mode} stdout={stdout_mode}");
        // Every later reply that carries a nonce must name this owned window.
        worker.expect_nonce(nonce);
        let end = Instant::now() + Duration::from_secs(30);
        loop {
            let r = worker
                .request(&json!({"action":"observe"}))
                .unwrap_or_else(|reason| panic!("bounded native window observe failure: {reason}"));
            if r["status"] == "ok" {
                break;
            }
            assert!(
                Instant::now() < end,
                "owned native window did not appear: {}",
                r["reason"]
            );
            thread::sleep(Duration::from_millis(150));
        }
        println!(
            "OWNED_WINDOW_READY nonce={nonce} pid={} foreground protection enabled",
            child.id()
        );
        Self {
            gui: child,
            worker,
            lines,
            after: 0,
            out: out.to_owned(),
        }
    }
    /// A bounded worker fault is a failed acceptance run, never a silent continue:
    /// the session is already unusable and no later action may adopt a stale reply.
    fn call(&mut self, p: &Value) -> Value {
        self.worker
            .request(p)
            .unwrap_or_else(|reason| panic!("bounded native window worker failure: {reason}"))
    }
    /// Foreground readiness is bounded overall; each request is bounded inside
    /// `WorkerRpc`. Neither may be replaced by an unbounded blocked pipe read.
    fn control(&mut self, p: Value) -> Value {
        let deadline = Instant::now() + Duration::from_secs(900);
        let mut announced = false;
        loop {
            let r = self.call(&p);
            if r["status"] == "ok" {
                if r["input_layout"].is_object() {
                    self.lines
                        .lock()
                        .unwrap()
                        .push(format!("ACP_INPUT_LAYOUT {}", r["input_layout"]));
                }
                return r;
            }
            if r["status"] == "foreground_required" {
                if !announced {
                    println!("FOREGROUND_REQUIRED nonce={}", r["nonce"]);
                    announced = true;
                }
                assert!(
                    Instant::now() < deadline,
                    "owned foreground readiness deadline"
                );
                thread::sleep(Duration::from_millis(250));
            } else {
                panic!("native window control refused: {}", r["reason"]);
            }
        }
    }
    pub fn key(&mut self, key: &str) {
        self.after = self.lines.lock().unwrap().len();
        self.control(json!({"action":"key","key":key}));
        thread::sleep(Duration::from_millis(100));
    }
    pub fn text(&mut self, text: &str) {
        self.control(json!({"action":"text","text":text}));
        thread::sleep(Duration::from_millis(100));
    }
    pub fn wait(&self, marker: &str) {
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end {
            if self.lines.lock().unwrap()[self.after..]
                .iter()
                .any(|s| s.contains(marker))
            {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("native GUI state marker missing: {marker}");
    }
    pub fn capture(&mut self, name: &str, out: &Path) {
        // State probes can precede the compositor's next paint. Captured pixels
        // still require visual inspection; this delay is not rendering proof.
        thread::sleep(Duration::from_millis(250));
        let r = self.control(json!({"action":"capture","name":format!("{name}.png")}));
        let file = r["file"].as_str().unwrap();
        let native = if cfg!(unix) {
            String::from_utf8(
                Command::new("wslpath")
                    .args(["-u", file])
                    .output()
                    .unwrap()
                    .stdout,
            )
            .unwrap()
            .trim()
            .to_owned()
        } else {
            file.to_owned()
        };
        std::fs::copy(native, out.join(format!("{name}.png"))).unwrap();
    }
    pub fn resize(&mut self, w: u16, h: u16) {
        self.control(json!({"action":"resize","width":w,"height":h}));
        thread::sleep(Duration::from_millis(400));
    }
    pub fn close(&mut self) {
        self.control(json!({"action":"close"}));
        let end = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(status) = self.gui.try_wait().unwrap() {
                assert!(status.success(), "owned GUI did not exit cleanly");
                break;
            }
            assert!(Instant::now() < end);
            thread::sleep(Duration::from_millis(30));
        }
        let cleanup = self.worker.shutdown();
        println!(
            "OWNED_DRIVER_CLOSED gui_pid={} worker_reaped={} reader_joined={} writer_joined={}",
            self.gui.id(),
            cleanup.reaped,
            cleanup.reader_joined,
            cleanup.writer_joined
        );
        assert!(
            cleanup.reaped && cleanup.reader_joined && cleanup.writer_joined,
            "owned driver cleanup left an unreaped worker or an unjoined IO thread"
        );
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        if let Ok(lines) = self.lines.lock() {
            let _ = std::fs::write(self.out.join("desktop-probes.log"), lines.join("\n"));
        }
        let _ = self.gui.kill();
        let _ = self.gui.wait();
    }
}
