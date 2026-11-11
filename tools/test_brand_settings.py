"""Real owned-window and PTY/ConPTY theme/language switching; synthetic data only."""

import argparse
import ctypes
import json
import os
import time
from pathlib import Path

from local_console_probe import (
    TerminalProbe,
    native_start,
    native_close,
    windows_api,
    native_wait_marker,
)
from test_local_service import ROOT, stop
from window_capture import capture_owned_window


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--desktop", type=Path, required=True)
    p.add_argument("--tui", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--product", action="store_true")
    args = p.parse_args()
    out = args.out.resolve()
    assert any(
        out.is_relative_to(ROOT / folder)
        for folder in [
            "artifacts/local-p1-f",
            "artifacts/local-p1-g",
            "artifacts/local-p1-h",
            "artifacts/local-p1-i",
        ]
    )
    out.mkdir(parents=True, exist_ok=True, mode=0o700)
    flags = [] if args.product else ["--brand-preview", "c"]
    proc, window, _ = native_start([str(args.desktop.resolve()), *flags])
    try:
        if os.name == "nt":
            user = windows_api()
            user.keybd_event.argtypes = [
                ctypes.c_ubyte,
                ctypes.c_ubyte,
                ctypes.c_uint,
                ctypes.c_size_t,
            ]

            def send(sequence):
                modifier, key = {
                    "ctrl+l": (0x11, ord("L")),
                    "alt+0": (0x12, ord("0")),
                    "alt+b": (0x12, ord("B")),
                }[sequence]
                user.SetForegroundWindow(window)
                time.sleep(0.08)
                assert user.GetForegroundWindow() == window
                user.keybd_event(modifier, user.MapVirtualKeyW(modifier, 0), 0, 0)
                try:
                    assert user.GetForegroundWindow() == window
                    user.keybd_event(key, user.MapVirtualKeyW(key, 0), 0, 0)
                finally:
                    user.keybd_event(key, user.MapVirtualKeyW(key, 0), 2, 0)
                    user.keybd_event(modifier, user.MapVirtualKeyW(modifier, 0), 2, 0)

            def wait(marker):
                native_wait_marker(proc, marker)

        else:

            def send(sequence):
                window.key(sequence)

            def wait(marker):
                window.wait(marker)

        send("ctrl+l")
        wait("locale=ZhCn")
        send("alt+0")
        wait(
            "settings=true locale=ZhCn"
            if args.product
            else "page=Appearance locale=ZhCn"
        )
        send("alt+b")
        wait(
            "settings=true locale=ZhCn theme=Ledger"
            if args.product
            else "theme=Ledger page=Appearance locale=ZhCn"
        )
        time.sleep(0.2)
        if os.name == "nt":
            capture_owned_window(window, out / "settings-switched.png")
        else:
            window.capture(out / "settings-switched.png", frameless=True)
        send("ctrl+l")
        wait(
            "settings=true locale=En theme=Ledger"
            if args.product
            else "theme=Ledger page=Appearance locale=En"
        )
    finally:
        native_close(window)
        stop(proc)
    terminal = TerminalProbe([str(args.tui.resolve()), *flags])
    try:
        terminal.wait("Agent access")
        terminal.send(b"\x0c")
        terminal.wait("Daemon 状态" if args.product else "有效上下文")
        terminal.send(b"\x1b0")
        terminal.wait("主题集")
        terminal.send(b"\x1bb")
        terminal.wait("> Ledger")
        terminal.send(b"\x0c")
        terminal.wait("Theme collection")
        terminal.resize(80, 30)
        terminal.wait("Theme collection")
        terminal.settle()
        (out / "settings-interaction-80.txt").write_text(
            "\n".join(terminal.screen.display) + "\n", encoding="utf-8"
        )
    finally:
        terminal.close()
    result = dict(
        status="PASS",
        native_language_and_theme=True,
        terminal_language_and_theme=True,
        clipboard="NOT_RUN",
        synthetic_only=True,
    )
    (out / "settings-result.json").write_text(
        json.dumps(result, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(result))


if __name__ == "__main__":
    main()
