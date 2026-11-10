"""Native Wayland probe transport. Never certify a window using X11 tools."""

from __future__ import annotations
import json
import os
from pathlib import Path
import queue
import re
import subprocess
import sys
import time
import uuid
from test_local_service import collect, stop, ROOT
from wslg_doctor import environment


def windows_python():
    value = os.environ.get("ACP_WINDOWS_PYTHON")
    return Path(value) if value else None


def control(nonce, action, **values):
    python = windows_python()
    assert (
        python and python.is_absolute() and python.is_file()
    ), "explicit ACP_WINDOWS_PYTHON required for WSLg OS input/capture"
    # Running a stdlib/test-only worker through stdin avoids shell execution and
    # does not require writing to a Windows checkout or forwarding credentials.
    source = (
        (ROOT / "tools/window_capture.py").read_text()
        + "\n"
        + (ROOT / "tools/window_chrome_probe.py").read_text()
        + "\n"
        + (ROOT / "tools/wslg_window.py").read_text()
    )
    deps = os.environ.get("ACP_WINDOWS_TEST_ROOT")
    if deps:
        path = Path(deps)
        assert (
            path / ".local-acp-test-root"
        ).is_file(), "owned Windows test dependency directory required"
        value = subprocess.run(
            ["wslpath", "-w", str(path / "artifacts/local-p1-e2/python")],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        source = "import sys\nsys.path.insert(0," + repr(value) + ")\n" + source
    if action == "capture":
        output = Path(values["output"]).resolve()
        assert any(
            output.is_relative_to(ROOT / folder)
            for folder in (
                "artifacts/local-p1-e3",
                "artifacts/local-p1-e4",
                "artifacts/local-p1-f",
            )
        ), "synthetic captures must stay in private local UI artifacts"
        output.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        values["output"] = subprocess.run(
            ["wslpath", "-w", str(output)], check=True, capture_output=True, text=True
        ).stdout.strip()
    p = subprocess.run(
        [str(python), "-", json.dumps(dict(nonce=nonce, action=action, **values))],
        input=source,
        capture_output=True,
        text=True,
        timeout=15,
    )
    assert p.returncode == 0, "WSLg owned-window operation failed: " + p.stderr[-400:]
    return json.loads(p.stdout)


class WaylandWindow:
    def __init__(self, command, *, backend="auto", extra_env=None):
        self.nonce = uuid.uuid4().hex[:16]
        self.lines = []
        self.started = time.monotonic()
        self.environment = environment()
        env = {**os.environ, "ACP_UI_PROBE": "1", "ACP_UI_PROBE_ID": self.nonce}
        if backend == "auto":
            env.pop("GDK_BACKEND", None)
        else:
            env["GDK_BACKEND"] = backend
        env.update(extra_env or {})
        self.proc = subprocess.Popen(
            command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env
        )
        self.queue = collect(self.proc.stderr)
        collect(self.proc.stdout)
        self.proc._acp_probe_lines = self.queue
        self.sample = {}
        try:
            if windows_python():
                control(self.nonce, "visible")
                self.sample["visible_seconds"] = round(
                    time.monotonic() - self.started, 4
                )
            self.wait("backend=GdkWaylandDisplay", timeout=30)
            self.sample.setdefault(
                "backend_verified_seconds", round(time.monotonic() - self.started, 4)
            )
            self.wait("mapped=true", timeout=30)
            self.sample.setdefault(
                "mapped_seconds", round(time.monotonic() - self.started, 4)
            )
            self.wait("ACP_NSH_LAYOUT_READY", timeout=30)
            self.sample.setdefault(
                "model_layout_seconds", round(time.monotonic() - self.started, 4)
            )
            self.wait("ACP_GTK_FRAME after_paint=", timeout=30)
            self.sample.setdefault(
                "gtk_after_paint_seconds", round(time.monotonic() - self.started, 4)
            )
            self.sample["backend"] = "GdkWaylandDisplay"
            self.sample["renderer"] = next(
                re.search(r"renderer=(\S+)", s)[1]
                for s in self.lines
                if re.search(r"renderer=\S+", s)
            )
        except BaseException:
            self.close(hard=True)
            raise

    def drain(self):
        while not self.queue.empty():
            value = self.queue.get_nowait()
            if value:
                line = value.decode(errors="replace")
                self.lines.append(line)
                elapsed = re.search(r"elapsed=([0-9.]+)", line)
                if elapsed:
                    elapsed = float(elapsed[1])
                    for marker, name in [
                        ("backend=GdkWaylandDisplay", "backend_verified_seconds"),
                        ("mapped=true", "mapped_seconds"),
                        ("ACP_NSH_LAYOUT_READY", "model_layout_seconds"),
                        ("ACP_GTK_FRAME", "gtk_after_paint_seconds"),
                        ("phase=Ready", "model_ready_seconds"),
                    ]:
                        if marker in line:
                            self.sample.setdefault(name, elapsed)

    def wait(self, marker, *, timeout=15, after=0):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.drain()
            if any("ACP_GTK backend=GdkX11Display" in line for line in self.lines):
                raise AssertionError(
                    "actual GTK backend is X11; native Wayland acceptance rejected"
                )
            if any(marker in line for line in self.lines[after:]):
                return
            assert (
                self.proc.poll() is None
            ), "native Wayland process exited before expected state"
            time.sleep(0.02)
        raise AssertionError("native Wayland marker timed out: " + marker)

    def key(self, key):
        self.drain()
        index = len(self.lines)
        control(self.nonce, "key", sequence=key)
        return index

    def state(self, *, selected=0, rows=None, after=0, phase="Ready"):
        suffix = f"phase={phase} selected={selected}"
        if rows is not None:
            suffix += f" rows={rows}"
        self.wait(suffix, after=after)

    def capture(self, output, *, frameless=False):
        return control(self.nonce, "capture", output=str(output), frameless=frameless)

    def resize(self, width, height, *, frameless=False):
        self.drain()
        index = len(self.lines)
        before = next((s for s in reversed(self.lines) if "ACP_GTK backend=" in s), "")
        if f"width={width} height={height}" in before:
            return
        control(self.nonce, "resize", size=[width, height], frameless=frameless)
        # WSLg native frame includes GTK decorations; assert a fresh actual
        # allocation instead of equating outer OS bounds to GTK content size.
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            self.drain()
            if any(
                "ACP_GTK backend=" in s and f"width={width} height={height}" in s
                for s in self.lines[index:]
            ):
                return
            time.sleep(0.03)
        raise AssertionError("Wayland configure did not change GTK allocation")

    def close(self, hard=False):
        if self.proc.poll() is None:
            if hard:
                stop(self.proc, hard=True)
            else:
                control(self.nonce, "close")
                self.proc.wait(timeout=10)
                assert self.proc.returncode == 0, "Wayland window did not close cleanly"
