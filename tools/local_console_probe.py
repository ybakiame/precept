"""Local OS probes. Live project data never enters screenshots or diagnostics."""
from __future__ import annotations
import codecs
import json
import os
from pathlib import Path
import platform
import queue
import re
import select
import shutil
import struct
import subprocess
import time
from test_local_service import collect, stop, ROOT


def exe(name):
    return ROOT / "spikes/p1-e/nsh/target/release" / (name + (".exe" if os.name == "nt" else ""))


class TerminalProbe:
    def __init__(self, command):
        if os.name == "nt":
            self._windows_init(command)
            return
        import fcntl
        import pty
        import termios
        import pyte
        self.master, slave = pty.openpty()
        self.original_termios = termios.tcgetattr(slave)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 50, 160, 0, 0))
        self.screen = pyte.Screen(160, 50)
        self.stream = pyte.Stream(self.screen)
        self.decode = codecs.getincrementaldecoder("utf-8")("replace")
        env = {**os.environ, "TERM": "xterm-256color", "COLORTERM": "truecolor"}
        env.pop("ACP_UI_PROBE", None)
        self.started = time.monotonic()
        self.proc = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave, env=env, start_new_session=True)
        os.close(slave)

    def wait(self, expected, timeout=20):
        if os.name == "nt":
            return self._windows_wait(expected, timeout)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if select.select([self.master], [], [], .03)[0]:
                try:
                    self.stream.feed(self.decode.decode(os.read(self.master, 65536)))
                except OSError:
                    raise AssertionError("terminal closed before expected state") from None
            visible = "\n".join(self.screen.display)
            assert "ACP_NSH_" not in visible, "readiness marker leaked into terminal"
            if expected in visible:
                return time.monotonic() - self.started
            assert self.proc.poll() is None, "terminal exited before expected state"
        # Deliberately never print the screen: live context can be private.
        raise AssertionError("terminal did not reach expected state")

    def send(self, raw):
        if os.name == "nt":
            self.pty.write(raw.decode("utf-8"))
        else:
            os.write(self.master, raw)

    def focus(self, key, label):
        """Wait for the real terminal cursor to reach the requested input."""
        self.send(key)
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            if os.name == "nt":
                self.stream.feed(self.pty.read(blocking=False))
            elif select.select([self.master], [], [], .02)[0]:
                self.stream.feed(self.decode.decode(os.read(self.master, 65536)))
            rows = [i for i, line in enumerate(self.screen.display) if label in line]
            if rows and self.screen.cursor.y == rows[0] + 1:
                return
            time.sleep(.005)
        raise AssertionError("terminal input did not receive focus")

    def resize(self, columns, rows):
        if os.name == "nt":
            self.pty.set_size(columns, rows)
        else:
            import fcntl, termios
            fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        self.screen.resize(lines=rows, columns=columns)

    def close(self):
        if self.proc.poll() is None:
            self.send(b"\x11")  # Ctrl+Q; plain q remains editable input.
            self.proc.wait(timeout=8)
        assert self.proc.returncode == 0, "terminal did not close cleanly"
        if os.name != "nt":
            import termios
            restored = termios.tcgetattr(self.master)
            assert restored == self.original_termios, "terminal attributes were not restored"
            os.close(self.master)

    def _windows_init(self, command):
        import pyte
        from winpty import PTY, Backend
        # Use the low-level ConPTY binding. PtyProcess adds a TCP listener;
        # this direct pipe reader deliberately never opens one.
        self.pty = PTY(160, 50, backend=Backend.ConPTY)
        env = {**os.environ, "TERM": "xterm-256color", "COLORTERM": "truecolor"}
        env.pop("ACP_UI_PROBE", None)
        block = "\0".join(k + "=" + v for k, v in env.items()) + "\0"
        self.started = time.monotonic()
        assert self.pty.spawn(str(command[0]), cmdline=" " + subprocess.list2cmdline(command[1:]), cwd=os.getcwd(), env=block)
        self.screen = pyte.Screen(160, 50)
        self.stream = pyte.Stream(self.screen)
        pty = self.pty
        class Process:
            pid = pty.pid
            returncode = None
            def poll(self):
                if not pty.isalive(): self.returncode = pty.get_exitstatus()
                return self.returncode
            def wait(self, timeout):
                deadline = time.monotonic() + timeout
                while self.poll() is None and time.monotonic() < deadline:
                    # Drain ConPTY before wait; blocked output can keep it alive.
                    pty.read(blocking=False)
                    time.sleep(.01)
                if self.poll() is None:
                    import psutil
                    psutil.Process(self.pid).kill()
                    raise AssertionError("ConPTY child failed to close")
                return self.returncode
        self.proc = Process()

    def _windows_wait(self, expected, timeout):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            raw = self.pty.read(blocking=False)
            if raw:
                self.stream.feed(raw)
            visible = "\n".join(self.screen.display)
            assert "ACP_NSH_" not in visible, "readiness marker leaked into ConPTY"
            if expected in visible:
                return time.monotonic() - self.started
            assert self.proc.poll() is None, "ConPTY app exited before expected state"
            time.sleep(.01)
        raise AssertionError("ConPTY screen did not reach expected state")


def tui_available():
    try:
        import pyte  # noqa: F401
        if os.name == "nt":
            import winpty  # noqa: F401
    except ImportError:
        return "pyte/ConPTY test dependency unavailable"
    if not exe("acp-nsh-tui").is_file():
        return "release TUI executable unavailable"
    return None


def native_available():
    if platform.system() not in ["Linux", "Windows"]:
        return "local window probe unavailable on this OS"
    if platform.system() == "Linux":
        from wslg_doctor import environment
        if not environment()["wayland_socket"].get("reachable"):
            return "native Wayland socket unavailable; no X11 fallback"
        from wayland_probe import windows_python
        if not windows_python():
            return "native Wayland OS interaction probe needs explicit ACP_WINDOWS_PYTHON"
    if not exe("acp-nsh-native").is_file():
        return "release desktop executable unavailable"
    return None


def native_start(command):
    if os.name == "nt":
        return windows_native_start(command)
    from wayland_probe import WaylandWindow
    window = WaylandWindow(command)
    return window.proc, window, window.sample


def windows_api():
    import ctypes
    from ctypes import wintypes
    user = ctypes.WinDLL("user32", use_last_error=True)
    user.SetProcessDPIAware()
    user.IsWindowVisible.argtypes = [wintypes.HWND]
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.GetForegroundWindow.restype = wintypes.HWND
    user.SetForegroundWindow.argtypes = [wintypes.HWND]
    user.SetWindowPos.argtypes = [wintypes.HWND, wintypes.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_uint]
    user.PostMessageW.argtypes = [wintypes.HWND, ctypes.c_uint, wintypes.WPARAM, wintypes.LPARAM]
    user.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    return user


def windows_native_start(command):
    import ctypes
    from ctypes import wintypes
    user = windows_api()
    started = time.monotonic()
    proc = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env={**os.environ, "ACP_UI_PROBE": "1"})
    lines = collect(proc.stderr)
    collect(proc.stdout)
    deadline, visible, ready, wid = started + 45, None, False, None
    callback = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def visit(hwnd, _):
        nonlocal wid
        owner = wintypes.DWORD()
        user.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == proc.pid and user.IsWindowVisible(hwnd):
            wid = hwnd
        return True
    try:
        while time.monotonic() < deadline:
            assert proc.poll() is None, "Windows desktop exited before visible model"
            user.EnumWindows(callback(visit), 0)
            if wid and visible is None: visible = time.monotonic() - started
            while not lines.empty():
                line = lines.get_nowait()
                ready |= bool(line and b"ACP_NSH_LAYOUT_READY" in line)
            if wid and ready:
                proc._acp_probe_lines = lines
                return proc, wid, dict(visible_seconds=round(visible, 4), model_layout_seconds=round(time.monotonic()-started, 4))
            time.sleep(.02)
        raise AssertionError("Windows native window/model readiness deadline")
    except BaseException:
        stop(proc, hard=True)
        raise


def native_wait_marker(proc, marker, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            line = proc._acp_probe_lines.get(timeout=0.1)
        except queue.Empty:
            assert proc.poll() is None, "owned native app exited"
            continue
        if marker.encode() in (line or b""):
            return
    raise AssertionError("owned native marker timed out: " + marker)


def native_wait_state(proc, *, selected=None, rows=None, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            line = proc._acp_probe_lines.get(timeout=.1)
        except queue.Empty:
            assert proc.poll() is None, "native app exited before expected state"
            continue
        match = re.search(rb"ACP_NSH_STATE phase=Ready selected=(\d+) rows=(\d+)", line or b"")
        if match and (selected is None or int(match[1]) == selected) and (rows is None or int(match[2]) == rows):
            return
    raise AssertionError("native UI did not render the expected selection/filter state")


def native_close(wid):
    if os.name == "nt":
        assert windows_api().PostMessageW(wid, 0x0010, 0, 0), "WM_CLOSE failed"
    else:
        wid.close()


def live_views(fixture, label, out):
    results = {}
    reason = tui_available()
    if reason:
        results["tui"] = dict(status="NOT_RUN", reason=reason)
    else:
        terminal = TerminalProbe([str(exe("acp-nsh-tui")), *fixture.location, "--project", label, "--task-id", "missing"])
        try:
            terminal.wait("LIVE_READ_ONLY")
            terminal.wait("Project: " + label)
            terminal.wait("Empty — no handoff for task missing")
            # The preference empty state belongs to its own page in E4.
            terminal.focus(b"\x06", "Filter confirmed preferences")
            terminal.wait("Empty — no confirmed preferences match")
            terminal.send(b"\x1b1")  # Alt+1 returns to the Dashboard before project/reconnect checks.
            terminal.wait("Project: " + label)
            terminal.send(b"\x10")  # Ctrl+P selects the single allowlisted root.
            terminal.wait("Project: " + label)
            # Actual live disconnect/reconnect while the terminal remains mounted.
            stop(fixture.proc)
            terminal.send(b"\x12")  # Ctrl+R
            terminal.wait("Unavailable")
            fixture.start()
            terminal.send(b"\x12")
            terminal.wait("Project: " + label)
        finally:
            terminal.close()
        assert fixture.cli("health")["ready"]
        results["tui"] = dict(status="PASS", context="actual selected-project empty state in PTY", reconnect=True, daemon_outlives_terminal=True)
    reason = native_available()
    if reason:
        results["desktop"] = dict(status="NOT_RUN", reason=reason)
    else:
        proc, wid, sample = native_start([str(exe("acp-nsh-native")), *fixture.location, "--project", label, "--task-id", "missing"])
        # No real-project screenshot, screen text or clipboard capture.
        native_close(wid)
        try:
            proc.wait(timeout=8)
        finally:
            stop(proc, hard=True)
        assert fixture.cli("health")["ready"]
        results["desktop"] = dict(status="PASS", model="authenticated selected-project snapshot reached native layout", sample=sample,
                                  daemon_outlives_window=True, screenshot="NOT_RUN for real data privacy")
    return results
