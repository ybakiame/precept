use crate::screen::Screen;
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
pub struct Tty {
    pub state: Arc<Mutex<Screen>>,
    pub raw: Arc<Mutex<Vec<u8>>>,
    pub os: platform::Terminal,
    out: std::path::PathBuf,
}
impl Tty {
    pub fn start(binary: &Path, args: &[String], out: &Path, focus: &Path) -> Self {
        let (mut os, mut reader) = platform::Terminal::start(binary, args, focus);
        let state = Arc::new(Mutex::new(Screen::new(160, 50)));
        let raw = Arc::new(Mutex::new(Vec::new()));
        let writer = os.writer();
        let s = state.clone();
        let r = raw.clone();
        thread::spawn(move || {
            let mut parser = vte::Parser::new();
            let mut bytes = [0; 65536];
            while let Ok(n) = reader.read(&mut bytes) {
                if n == 0 {
                    break;
                }
                let mut raw = r.lock().unwrap();
                assert!(
                    raw.len() + n < 16 * 1024 * 1024,
                    "bounded terminal capture exceeded"
                );
                raw.extend_from_slice(&bytes[..n]);
                drop(raw);
                let mut screen = s.lock().unwrap();
                parser.advance(&mut *screen, &bytes[..n]);
                let replies = std::mem::take(&mut screen.responses);
                drop(screen);
                for reply in replies {
                    let _ = writer.lock().unwrap().write_all(&reply);
                }
            }
        });
        os.resize(160, 50);
        Self {
            state,
            raw,
            os,
            out: out.to_owned(),
        }
    }
    pub fn send(&mut self, b: &[u8]) {
        self.os.write(b);
        thread::sleep(Duration::from_millis(80));
    }
    /// Human-paced text, not a bulk paste/event-queue stress test. Keep each
    /// UTF-8 scalar intact and give controlled inputs time to paint their value.
    /// No character or business action is retried; exact field checks still run.
    pub fn text(&mut self, text: &str) {
        assert!(text.chars().count() <= 1024, "bounded synthetic input");
        for character in text.chars() {
            let mut bytes = [0; 4];
            self.os.write(character.encode_utf8(&mut bytes).as_bytes());
            thread::sleep(Duration::from_millis(25));
        }
        thread::sleep(Duration::from_millis(80));
    }
    pub fn wait(&self, text: &str) {
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end {
            if self.state.lock().unwrap().text().contains(text) {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("terminal did not render expected synthetic state: {text}");
    }
    pub fn wait_gone(&self, text: &str) {
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end {
            if !self.state.lock().unwrap().text().contains(text) {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("terminal kept dismissed synthetic state: {text}");
    }
    pub fn capture(&self, path: &Path) {
        std::fs::write(path, self.state.lock().unwrap().text()).unwrap();
    }
    pub fn resize(&mut self, w: u16, h: u16) {
        self.state.lock().unwrap().resize(w as usize, h as usize);
        self.os.resize(w, h);
        thread::sleep(Duration::from_millis(300));
    }
    pub fn close(&mut self) {
        self.send(&[17]);
        self.os.wait_restored();
    }
    pub fn pid(&self) -> u32 {
        self.os.pid()
    }
}
#[cfg(unix)]
mod platform {
    use super::*;
    use std::{
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::process::CommandExt,
        },
        process::{Child, Command, Stdio},
    };
    pub struct Terminal {
        master: File,
        slave: File,
        child: Child,
        original: libc::termios,
        writer: Arc<Mutex<File>>,
    }
    impl Terminal {
        pub fn start(bin: &Path, args: &[String], focus: &Path) -> (Self, File) {
            unsafe {
                let (mut master, mut slave) = (0, 0);
                assert_eq!(
                    libc::openpty(
                        &mut master,
                        &mut slave,
                        std::ptr::null_mut(),
                        std::ptr::null(),
                        std::ptr::null()
                    ),
                    0
                );
                let master = File::from_raw_fd(master);
                let slave = File::from_raw_fd(slave);
                let mut original = std::mem::zeroed();
                assert_eq!(libc::tcgetattr(slave.as_raw_fd(), &mut original), 0);
                let mut cmd = Command::new(bin);
                cmd.args(args)
                    .env("TERM", "xterm-256color")
                    .env("COLORTERM", "truecolor")
                    .env_remove("ACP_UI_PROBE")
                    .env("ACP_NATIVE_FOCUS_FILE", focus)
                    .stdin(Stdio::from(slave.try_clone().unwrap()))
                    .stdout(Stdio::from(slave.try_clone().unwrap()))
                    .stderr(Stdio::from(slave.try_clone().unwrap()));
                let fd = slave.as_raw_fd();
                cmd.pre_exec(move || {
                    if libc::setsid() < 0 || libc::ioctl(fd, libc::TIOCSCTTY, 0) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
                let child = cmd.spawn().unwrap();
                eprintln!("OWNED_TTY_READY backend=PTY pid={}", child.id());
                let reader = master.try_clone().unwrap();
                (
                    Self {
                        writer: Arc::new(Mutex::new(master.try_clone().unwrap())),
                        master,
                        slave,
                        child,
                        original,
                    },
                    reader,
                )
            }
        }
        pub fn write(&mut self, b: &[u8]) {
            self.writer.lock().unwrap().write_all(b).unwrap();
        }
        pub fn pid(&self) -> u32 {
            self.child.id()
        }
        pub fn writer(&self) -> Arc<Mutex<File>> {
            self.writer.clone()
        }
        pub fn resize(&mut self, w: u16, h: u16) {
            unsafe {
                let size = libc::winsize {
                    ws_row: h,
                    ws_col: w,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };
                assert_eq!(
                    libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &size),
                    0
                );
            }
        }
        pub fn wait_restored(&mut self) {
            let end = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(code) = self.child.try_wait().unwrap() {
                    assert!(code.success());
                    break;
                }
                assert!(Instant::now() < end);
                thread::sleep(Duration::from_millis(20));
            }
            unsafe {
                let mut current: libc::termios = std::mem::zeroed();
                assert_eq!(libc::tcgetattr(self.slave.as_raw_fd(), &mut current), 0);
                assert_eq!(
                    current.c_lflag, self.original.c_lflag,
                    "terminal flags not restored"
                );
            }
        }
    }
    impl Drop for Terminal {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
#[cfg(windows)]
mod platform {
    use super::*;
    use std::os::windows::{ffi::OsStrExt, io::FromRawHandle};
    use windows_sys::Win32::{
        Foundation::*,
        System::{Console::*, Pipes::*, Threading::*},
    };
    pub struct Terminal {
        input: Arc<Mutex<File>>,
        process: HANDLE,
        console: HPCON,
    }
    impl Terminal {
        pub fn start(bin: &Path, args: &[String], focus: &Path) -> (Self, File) {
            unsafe {
                let (mut ir, mut iw, mut or, mut ow) = (
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
                assert_ne!(CreatePipe(&mut ir, &mut iw, std::ptr::null(), 0), 0);
                assert_ne!(CreatePipe(&mut or, &mut ow, std::ptr::null(), 0), 0);
                let mut console = 0;
                assert_eq!(
                    CreatePseudoConsole(COORD { X: 160, Y: 50 }, ir, ow, 0, &mut console),
                    0
                );
                let mut bytes = 0;
                InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut bytes);
                let mut buffer = vec![0usize; bytes.div_ceil(std::mem::size_of::<usize>())];
                let attributes = buffer.as_mut_ptr().cast();
                assert_ne!(
                    InitializeProcThreadAttributeList(attributes, 1, 0, &mut bytes),
                    0
                );
                assert_ne!(
                    UpdateProcThreadAttribute(
                        attributes,
                        0,
                        PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                        console as *const _,
                        std::mem::size_of::<HPCON>(),
                        std::ptr::null_mut(),
                        std::ptr::null()
                    ),
                    0
                );
                let quote = |s: &str| {
                    assert!(!s.contains('"') && !s.ends_with('\\'));
                    format!("\"{s}\"")
                };
                let command = std::iter::once(bin.to_str().unwrap())
                    .chain(args.iter().map(String::as_str))
                    .map(quote)
                    .collect::<Vec<_>>()
                    .join(" ");
                let mut line: Vec<u16> = command.encode_utf16().chain(Some(0)).collect();
                let app: Vec<u16> = bin.as_os_str().encode_wide().chain(Some(0)).collect();
                let mut startup = STARTUPINFOEXW::default();
                startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
                // Do not inherit this runner's redirected stdio (WSL interop /
                // PowerShell). Null handles are assigned by the pseudoconsole.
                startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
                startup.lpAttributeList = attributes;
                let mut info = PROCESS_INFORMATION::default();
                let mut environment: Vec<_> = std::env::vars_os()
                    .filter(|(key, _)| {
                        !key.to_string_lossy().eq_ignore_ascii_case("ACP_UI_PROBE")
                            && !key
                                .to_string_lossy()
                                .eq_ignore_ascii_case("ACP_NATIVE_FOCUS_FILE")
                    })
                    .collect();
                environment.push(("ACP_NATIVE_FOCUS_FILE".into(), focus.as_os_str().to_owned()));
                environment.sort_by_key(|(key, _)| key.to_string_lossy().to_uppercase());
                let mut environment_block = Vec::<u16>::new();
                for (key, value) in environment {
                    environment_block.extend(key.encode_wide());
                    environment_block.push(b'=' as u16);
                    environment_block.extend(value.encode_wide());
                    environment_block.push(0);
                }
                environment_block.push(0);
                let ok = CreateProcessW(
                    app.as_ptr(),
                    line.as_mut_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                    environment_block.as_ptr().cast(),
                    std::ptr::null(),
                    &startup.StartupInfo,
                    &mut info,
                );
                DeleteProcThreadAttributeList(attributes);
                CloseHandle(ir);
                CloseHandle(ow);
                assert_ne!(ok, 0, "ConPTY child launch failed");
                CloseHandle(info.hThread);
                eprintln!("OWNED_TTY_READY backend=ConPTY pid={}", info.dwProcessId);
                (
                    Self {
                        input: Arc::new(Mutex::new(File::from_raw_handle(iw))),
                        process: info.hProcess,
                        console,
                    },
                    File::from_raw_handle(or),
                )
            }
        }
        pub fn write(&mut self, b: &[u8]) {
            self.input.lock().unwrap().write_all(b).unwrap();
        }
        pub fn pid(&self) -> u32 {
            unsafe { GetProcessId(self.process) }
        }
        pub fn writer(&self) -> Arc<Mutex<File>> {
            self.input.clone()
        }
        pub fn resize(&mut self, w: u16, h: u16) {
            unsafe {
                assert_eq!(
                    ResizePseudoConsole(
                        self.console,
                        COORD {
                            X: w as i16,
                            Y: h as i16
                        }
                    ),
                    0
                );
            }
        }
        pub fn wait_restored(&mut self) {
            unsafe {
                assert_eq!(WaitForSingleObject(self.process, 5000), WAIT_OBJECT_0);
                let mut code = 999;
                assert_ne!(GetExitCodeProcess(self.process, &mut code), 0);
                assert_eq!(code, 0);
            }
        }
    }
    impl Drop for Terminal {
        fn drop(&mut self) {
            unsafe {
                TerminateProcess(self.process, 1);
                WaitForSingleObject(self.process, 5000);
                CloseHandle(self.process);
                ClosePseudoConsole(self.console);
            }
        }
    }
}

impl Drop for Tty {
    fn drop(&mut self) {
        if let Ok(s) = self.state.lock() {
            let _ = std::fs::write(self.out.join("last-terminal.txt"), s.text());
        }
        if let Ok(raw) = self.raw.lock() {
            let _ = std::fs::write(self.out.join("terminal.vt"), &*raw);
        }
    }
}
