use acp_native_acceptance::process::OwnedProcess;
use acp_native_acceptance::worker::WorkerRpc;
use serde_json::{Value, json};
use std::{
    path::Path,
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
pub struct Desktop {
    /// Owned from the moment the GUI exists, so an initialization failure below still
    /// terminates and reaps it and stops its log thread.
    gui: OwnedProcess,
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
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        #[cfg(unix)]
        {
            command
                .env("GDK_BACKEND", "wayland")
                .env_remove("DISPLAY")
                .env("WAYLAND_DISPLAY", "wayland-0");
        }
        let gui = OwnedProcess::adopt(command.spawn().expect("owned native GUI launches"), "gui")
            .unwrap_or_else(|unfit| panic!("owned GUI adoption refused: {unfit:?}"));
        let gui_read_mode = gui.read_mode();
        let lines = gui.log();
        let powershell = if cfg!(windows) {
            "powershell.exe"
        } else {
            "/mnt/c/Program Files/PowerShell/7/pwsh.exe"
        };
        // A refused session must not leave the window running: unwinding drops `gui`,
        // which terminates and reaps exactly the process this driver launched.
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
        .unwrap_or_else(|reason| panic!("native window worker session unavailable: {reason}"));
        let response = worker
            .request(&json!({"action":"init","nonce":nonce,"pid":gui.pid(),"remote":cfg!(unix)}))
            .unwrap_or_else(|reason| panic!("native window init refused: {reason}"));
        assert_eq!(response["status"], "ok");
        let (stdin_mode, stdout_mode) = worker.io_modes();
        println!(
            "DESKTOP_IO_MODE stdin={stdin_mode} stdout={stdout_mode} gui_stderr={gui_read_mode}"
        );
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
            gui.pid()
        );
        Self {
            gui,
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
        self.after = self.lines.lock().unwrap().len();
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
        self.after = self.lines.lock().unwrap().len();
        #[cfg(unix)]
        let (geometry, already_sized) = {
            let geometry = format!("width={w} height={h}");
            let lines = self.lines.lock().unwrap();
            let already = lines
                .iter()
                .rev()
                .find(|s| s.starts_with("ACP_GTK backend="))
                .is_some_and(|s| s.contains(&geometry));
            (geometry, already)
        };
        self.control(json!({"action":"resize","width":w,"height":h}));
        #[cfg(unix)]
        if !already_sized {
            self.wait(&geometry);
        }
        thread::sleep(Duration::from_millis(400));
    }
    pub fn close(&mut self) {
        self.control(json!({"action":"close"}));
        let end = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(status) = self.gui.exited() {
                assert!(status.success(), "owned GUI did not exit cleanly");
                break;
            }
            assert!(Instant::now() < end);
            thread::sleep(Duration::from_millis(30));
        }
        let cleanup = self.worker.shutdown();
        // Terminating the owned GUI is the last step, and it is never assumed: both its
        // reap and its log-thread join are reported and asserted.
        let gui = self.gui.terminate();
        println!(
            "OWNED_DRIVER_CLOSED gui_pid={} gui_reaped={} gui_reader_joined={} worker_reaped={} reader_joined={} writer_joined={}",
            self.gui.pid(),
            gui.reaped,
            gui.reader_joined,
            cleanup.reaped,
            cleanup.reader_joined,
            cleanup.writer_joined
        );
        assert!(
            gui.reaped
                && gui.reader_joined
                && cleanup.reaped
                && cleanup.reader_joined
                && cleanup.writer_joined,
            "owned driver cleanup left an unreaped child or an unjoined IO thread"
        );
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        if let Ok(lines) = self.lines.lock() {
            let _ = std::fs::write(self.out.join("desktop-probes.log"), lines.join("\n"));
        }
        // `gui` and `worker` own their processes and threads; dropping them reaps and
        // joins, which is why no initialization failure below the spawn can leak either.
    }
}
