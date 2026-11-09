"""Host-only release desktop probes using synthetic data and explicit IPC."""
import argparse
import ctypes
import json
import os
import platform
import subprocess
import time
from pathlib import Path
from local_console_probe import native_available, native_start, native_close, windows_api, native_wait_state
from test_local_service import Fixture, stop


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if platform.system() == "Linux":
        from test_wayland_console import verify
        return verify(args.binary, args.out)
    reason = native_available()
    if reason:
        print("NOT_RUN: " + reason)
        return
    try:
        import psutil
        import mss
        import mss.tools
    except ImportError:
        print("NOT_RUN: psutil/mss test dependencies unavailable")
        return
    args.out.mkdir(parents=True, exist_ok=True, mode=0o700)
    def content_rect(user, wid):
        # Native window bounds can include transparent system borders. Restrict
        # synthetic capture to the opaque client interior.
        from ctypes import wintypes
        user.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
        user.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
        bounds = wintypes.RECT(); origin = wintypes.POINT()
        assert user.GetClientRect(wid, ctypes.byref(bounds))
        assert user.ClientToScreen(wid, ctypes.byref(origin))
        return dict(left=origin.x+16, top=origin.y+32, width=bounds.right-32, height=bounds.bottom-48)
    samples = []
    with Fixture() as f:
        for _ in range(3):
            proc, wid, sample = native_start([str(args.binary.resolve()), *f.location, "--project", "alpha", "--task-id", "shared"])
            try:
                time.sleep(.3)
                sample["rss_bytes"] = psutil.Process(proc.pid).memory_info().rss
                samples.append(sample)
                if os.name == "nt":
                    from ctypes import wintypes
                    user = windows_api()
                    user.SetWindowPos(wid, None, 80, 80, 1080, 760, 0x40)
                    user.SetForegroundWindow(wid)
                    time.sleep(.15)
                    assert user.GetForegroundWindow() == wid, "synthetic window could not receive foreground focus"
                    user.keybd_event.argtypes = [ctypes.c_ubyte, ctypes.c_ubyte, ctypes.c_uint, ctypes.c_size_t]
                    def key(vk, up=False):
                        user.keybd_event(vk, 0, 2 if up else 0, 0)
                    # Windows NSH Tab traverses fields; explicit project shortcut
                    # supplies the same action on this native backend.
                    key(0x11); key(ord('P')); key(ord('P'), True); key(0x11, True)
                    native_wait_state(proc, selected=1)
                    key(0x11); key(ord('F')); key(ord('F'), True); key(0x11, True)
                    time.sleep(.08)
                    for c in "nomatch":
                        key(ord(c.upper())); key(ord(c.upper()), True)
                    # Commit composition if the operator has an IME active.
                    key(0x0d); key(0x0d, True)
                    native_wait_state(proc, selected=1, rows=0)
                    time.sleep(.3)
                    bounds = wintypes.RECT()
                    assert user.GetWindowRect(wid, ctypes.byref(bounds))
                    rect = content_rect(user, wid)
                # Capture this synthetic app's window only, never the whole desktop.
                with mss.MSS() as capture:
                    frame = capture.grab(rect)
                    assert len(set(frame.rgb[::3])) > 8, "blank native window"
                    if len(samples) == 1:
                        mss.tools.to_png(frame.rgb, frame.size, output=str(args.out / "desktop-synthetic.png"))
                if os.name == "nt":
                    user.SetWindowPos(wid, None, 80, 80, 640, 480, 0x40)
                native_close(wid)
                proc.wait(timeout=8)
            finally:
                stop(proc, hard=True)
            assert f.cli("health")["ready"]
        assert f.db.read_bytes() == f.before
    workload = dict(status="NOT_RUN", reason="native 10k probe currently Windows-only")
    if os.name == "nt":
        proc, wid, _ = native_start([str(args.binary.resolve()), "--synthetic-rows", "10000"])
        try:
            user.SetWindowPos(wid, None, 80, 80, 1080, 760, 0x40)
            user.SetForegroundWindow(wid)
            time.sleep(.15)
            assert user.GetForegroundWindow() == wid
            rss_before = psutil.Process(proc.pid).memory_info().rss
            started = time.monotonic()
            key(0x11); key(ord('F')); key(ord('F'), True); key(0x11, True)
            time.sleep(.08)
            user.SendMessageW.argtypes = [wintypes.HWND, ctypes.c_uint, wintypes.WPARAM, wintypes.LPARAM]
            user.SendMessageW.restype = ctypes.c_ssize_t
            for c in 'event-09999':
                # A deterministic character stream through the native window's
                # real text handler, independent of the operator's active IME.
                user.SendMessageW(wid, 0x0102, ord(c), 0)
            native_wait_state(proc, selected=0, rows=1)
            elapsed = time.monotonic() - started
            assert elapsed < 2, "10k native filter exceeded local input latency bound"
            time.sleep(.25)
            bounds = wintypes.RECT(); assert user.GetWindowRect(wid, ctypes.byref(bounds))
            with mss.MSS() as capture:
                frame = capture.grab(content_rect(user, wid))
                assert len(set(frame.rgb[::3])) > 8
                mss.tools.to_png(frame.rgb, frame.size, output=str(args.out / 'desktop-10k-synthetic.png'))
            workload = dict(status="PASS", rows=10000, filtered_rows=1, filter_to_rendered_state_seconds=round(elapsed, 4),
                            input_method="Ctrl+F focus, then Win32 WM_CHAR messages (IME-independent)",
                            rss_before_filter_bytes=rss_before, rss_after_filter_bytes=psutil.Process(proc.pid).memory_info().rss)
            native_close(wid); proc.wait(timeout=8)
        finally:
            stop(proc, hard=True)
    result = dict(status="PASS", os=platform.system(), display="local Windows desktop" if os.name == "nt" else "unsupported", profile="release", sample_count=len(samples), samples=samples,
                  binary_bytes=args.binary.stat().st_size, synthetic_only=True, daemon_outlives_window=True,
                  rendered_10k=workload,
                  keyboard="Windows Ctrl+P selected beta; Ctrl+F + text + Enter committed filter; rendered selection/count assertions" if os.name == "nt" else "unsupported",
                  accessibility="NOT_RUN: screen reader and IME require operator session")
    (args.out / "desktop-result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result))


if __name__ == "__main__":
    raise SystemExit(main())
