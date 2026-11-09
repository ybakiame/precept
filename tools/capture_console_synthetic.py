"""Private before/after captures of explicit binaries, using only synthetic data."""

import argparse
import json
import os
import time
from pathlib import Path
from local_console_probe import TerminalProbe, native_start, native_close, windows_api
from test_local_service import stop
from window_capture import capture_owned_window


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--desktop", type=Path, required=True)
    p.add_argument("--tui", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    args = p.parse_args()
    args.out.mkdir(parents=True, exist_ok=True, mode=0o700)
    proc, window, _ = native_start(
        [str(args.desktop.resolve()), "--synthetic-rows", "10000"]
    )
    try:
        for width, height in [(1080, 760), (640, 480)]:
            if os.name == "nt":
                user = windows_api()
                assert user.SetWindowPos(window, None, 80, 80, width, height, 0x40)
                time.sleep(0.25)
                capture_owned_window(
                    window, args.out / f"windows-{width}.png", inset=(16, 48, 16, 16)
                )
            else:
                window.resize(width, height)
                window.capture(args.out / f"wslg-{width}.png")
    finally:
        if os.name == "nt":
            native_close(window)
        else:
            window.close(hard=True)
        stop(proc, hard=True)
    terminal = TerminalProbe([str(args.tui.resolve()), "--synthetic-rows", "10000"])
    try:
        terminal.wait("OFFLINE_SYNTHETIC")
        terminal.wait("10000 confirmed")
        for width, height in [(160, 50), (80, 30)]:
            terminal.resize(width, height)
            time.sleep(0.1)
            terminal.wait("OFFLINE_SYNTHETIC")
            (args.out / f"terminal-{width}.txt").write_text(
                "\n".join(terminal.screen.display) + "\n", encoding="utf-8"
            )
    finally:
        terminal.close()
    print(
        json.dumps(
            dict(
                status="PASS",
                synthetic_only=True,
                capture="owned window PrintWindow; own PTY/ConPTY screen",
            )
        )
    )


if __name__ == "__main__":
    main()
