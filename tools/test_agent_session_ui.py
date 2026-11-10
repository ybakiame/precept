"""Actual NSH native/PTY or ConPTY clients of one real daemon-owned fake ACP child."""

import argparse
import json
import os
from pathlib import Path
import time
from test_agent_session import SessionFixture
from test_local_service import ROOT, stop
from local_console_probe import (
    TerminalProbe,
    native_start,
    native_close,
    native_wait_marker,
    windows_api,
)
from window_capture import capture_owned_window


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--desktop", type=Path, required=True)
    p.add_argument("--tui", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    a = p.parse_args()
    out = a.out.resolve()
    assert out.is_relative_to(ROOT / "artifacts/local-p1-g")
    out.mkdir(parents=True, exist_ok=True, mode=0o700)
    with SessionFixture() as f:
        f.set_programs(["normal", "permission", "cancel"])
        cfg = json.loads(f.config.read_text())
        cfg["programs"][0]["id"] = "agent"
        f.config.write_text(json.dumps(cfg))
        f.start()
        flags = [
            *f.location,
            "--project",
            "alpha",
            "--sessions",
            "--appearance",
            "light",
        ]
        proc = window = terminal = None

        def launch():
            return native_start([str(a.desktop.resolve()), *flags])

        def key(sequence):
            if os.name == "nt":
                from native_window_input import NativeInput

                inp = NativeInput(proc, window)
                if sequence == "session-prompt":
                    for c in sequence:
                        inp.owned()
                        inp.user.SendMessageW(window, 0x102, ord(c), 0)
                else:
                    keys = {"Tab": [9], "Enter": [13]}.get(sequence)
                    if keys is None:
                        keys = [
                            0x11 if sequence.startswith("ctrl+") else 0x12,
                            ord(sequence.split("+")[1].upper()),
                        ]
                    inp.chord(*keys)
            else:
                return window.key(sequence)

        def wait(marker):
            if os.name == "nt":
                native_wait_marker(proc, marker)
            else:
                return window.wait(marker)

        def capture(name, width=1080, height=760):
            if os.name == "nt":
                assert windows_api().SetWindowPos(
                    window, None, 80, 80, width, height, 0x40
                )
                time.sleep(0.2)
                capture_owned_window(window, out / name)
            else:
                window.resize(width, height, frameless=True)
                time.sleep(0.15)
                window.capture(out / name, frameless=True)

        try:
            proc, window, sample = launch()
            wait("configured=3")
            key("alt+7")
            wait("confirm=true")
            time.sleep(0.9)
            assert (
                f.query("session.get") is None
            ), "review must not automatically launch"
            capture("confirm-1080.png")
            key("alt+8")
            v = f.wait("ready")
            ident = v["session_id"]
            wait("state=Some(Ready)")
            key("alt+j")
            time.sleep(0.15)
            key("session-prompt")
            time.sleep(0.1)
            key("alt+9")
            v = f.wait("completed")
            assert any("Synthetic ACP update" in e for e in v["events"])
            wait("state=Some(Completed)")
            capture("completed-1080.png")
            capture("completed-640.png", 640, 480)
            key("ctrl+l")
            wait("locale=ZhCn")
            capture("completed-zh-640.png", 640, 480)
            key("ctrl+l")
            wait("locale=En")
            terminal = TerminalProbe([str(a.tui.resolve()), *flags])
            terminal.wait("turn completed")
            # Close/reopen actual clients; the same daemon-owned child/session persists.
            native_close(window)
            stop(proc)
            proc = window = None
            assert (
                f.query("session.get")["session_id"] == ident
                and f.query("session.get")["process_alive"]
            )
            terminal.close()
            terminal = None
            proc, window, _ = launch()
            wait("state=Some(Completed)")
            terminal = TerminalProbe([str(a.tui.resolve()), *flags])
            terminal.wait("turn completed")
            # A second explicit prompt originates in the terminal, observed by the GUI.
            prior = f.query("session.get")["revision"]
            terminal.focus(b"\x1bj", "Single line, at most 4096 UTF-8 bytes")
            terminal.send(b"terminal-prompt")
            terminal.wait("terminal-prompt")
            terminal.send(b"\x1b9")
            deadline = time.monotonic() + 8
            while time.monotonic() < deadline:
                observed = f.query("session.get")
                if observed["revision"] > prior and observed["state"] == "completed":
                    break
                time.sleep(0.03)
            else:
                raise AssertionError("TUI prompt did not reach daemon completion")
            terminal.wait("turn completed")
            wait("revision=" + str(observed["revision"]) + " ")
            for width, height in [(160, 50), (80, 30)]:
                terminal.resize(width, height)
                terminal.settle()
                (out / f"session-{width}.txt").write_text(
                    "\n".join(terminal.screen.display) + "\n", encoding="utf-8"
                )
            terminal.resize(160, 50)
            # Cancellation through an actual GUI, with both clients observing the turn.
            f.end(v)
            v = f.launch("cancel")
            f.query(
                "session.prompt", session_id=v["session_id"], prompt="cancel fixture"
            )
            wait("state=Some(Running)")
            key("alt+q")
            v = f.wait("completed")
            assert v["stop_reason"] == "cancelled"
            f.end(v)
            v = f.launch("permission")
            f.query(
                "session.prompt",
                session_id=v["session_id"],
                prompt="permission fixture",
            )
            f.wait("waiting_permission")
            wait("state=Some(WaitingPermission)")
            terminal.wait("waiting for permission")
            # An unselected permission must never be approved by a shortcut.
            key("alt+y")
            assert f.query("session.get")["state"] == "waiting_permission"
            # Explicit review binds keyboard decisions to that one pending request.
            key("alt+i")
            time.sleep(0.1)
            key("alt+n")
            v = f.wait("completed")
            assert "Permission denied" in " ".join(v["events"])
            wait("state=Some(Completed)")
            f.query(
                "session.prompt",
                session_id=v["session_id"],
                prompt="allow synthetic operation",
            )
            f.wait("waiting_permission")
            wait("state=Some(WaitingPermission)")
            key("alt+i")
            time.sleep(0.1)
            capture("permission-reviewed-1080.png")
            key("alt+y")
            v = f.wait("completed")
            assert "Permission explicitly allowed once" in " ".join(v["events"])
            wait("state=Some(Completed)")
            # Stop deliberately from TUI; closing either client alone never stopped it.
            terminal.send(b"\x1bx")
            f.wait("stopped")
            terminal.wait("stopped")
            wait("state=Some(Stopped)")
            assert f.cli("health")["ready"] and f.db.read_bytes() == f.before
            stop(f.proc)
            terminal.wait("Session unavailable or access refused")
            wait("state=None")
            result = dict(
                status="PASS",
                agent="deterministic Rust fake ACP agent; NOT a real coding agent",
                native=sample,
                explicit_native_start_prompt_cancel=True,
                terminal_prompt_stop=True,
                two_clients_and_close_reopen=True,
                permission="both clients observed pending; actual GUI Deny and Allow once controls",
                daemon_loss_clears_state=True,
                database_unchanged=True,
                clipboard="NOT_RUN",
                real_agent="NOT_RUN by deterministic UI test; explicit opt-in smoke is separate",
            )
            (out / "session-ui-result.json").write_text(
                json.dumps(result, indent=2) + "\n"
            )
            print(json.dumps(result))
        finally:
            if terminal:
                terminal.close()
            if window:
                native_close(window)
            if proc:
                stop(proc)


if __name__ == "__main__":
    main()
