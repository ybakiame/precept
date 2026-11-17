//! Linux (WSL2 + WSLg) packaged process smoke for the S1-C tar.gz. Not shipped;
//! never opens user state. Desktop runs on native Wayland through WSLg's Weston.
//! Unix-only; other platforms build a stub so the harness crate still compiles.
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../desktop.rs"]
mod desktop;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../screen.rs"]
mod screen;
#[cfg(unix)]
#[allow(dead_code)]
#[path = "../tty.rs"]
mod tty;
#[cfg(unix)]
#[path = "linux_portable_smoke/unix.rs"]
mod unix;

#[cfg(unix)]
fn main() {
    unix::main()
}

#[cfg(not(unix))]
fn main() {
    panic!("Linux packaged acceptance only");
}
