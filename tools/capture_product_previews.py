"""Actual five-page synthetic console previews. Only owned window pixels, private output."""

import argparse
import hashlib
import json
import os
import time
from pathlib import Path
from local_console_probe import TerminalProbe, native_start, native_close, windows_api
from test_local_service import ROOT, stop
from window_capture import capture_owned_window
from check_caption_alignment import caption_alignment


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--desktop", type=Path, required=True)
    p.add_argument("--tui", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--locale", choices=["en", "zh-CN"], default="en")
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
    results = []
    for page in [
        "dashboard",
        "projects",
        "preferences",
        "handoff",
        "diagnostics",
        "settings",
        "connection",
    ]:
        flags = [
            "--locale",
            args.locale,
            "--appearance",
            "light",
            "--task-id",
            "shared",
        ]
        flags += (
            ["--settings"]
            if page == "settings"
            else (
                ["--connection-settings"]
                if page == "connection"
                else ["--section", page]
            )
        )
        proc, window, sample = native_start([str(args.desktop.resolve()), *flags])
        try:
            for width, height in [(1080, 760), (640, 480)]:
                dest = out / f"{page}-{width}.png"
                if os.name == "nt":
                    assert windows_api().SetWindowPos(
                        window, None, 80, 80, width, height, 0x40
                    )
                    time.sleep(0.25)
                    capture_owned_window(window, dest)
                else:
                    window.wait("decorated=false")
                    window.resize(width, height, frameless=True)
                    time.sleep(0.15)
                    window.capture(dest, frameless=True)
                caption_alignment(dest)
            results.append(dict(page=page, native=sample, caption_alignment="PASS"))
        finally:
            native_close(window)
            stop(proc)
        terminal = TerminalProbe([str(args.tui.resolve()), *flags])
        try:
            terminal.wait("OFFLINE_SYNTHETIC")
            for width, height in [(160, 50), (80, 30)]:
                terminal.resize(width, height)
                terminal.wait("OFFLINE_SYNTHETIC")
                terminal.settle()
                (out / f"{page}-{width}.txt").write_text(
                    "\n".join(terminal.screen.display) + "\n", encoding="utf-8"
                )
            results[-1]["terminal"] = "PASS"
        finally:
            terminal.close()
    (out / "previews.json").write_text(
        json.dumps(
            dict(
                status="PASS",
                locale=args.locale,
                scenarios=results,
                binaries={
                    str(p.name): hashlib.sha256(p.read_bytes()).hexdigest()
                    for p in [args.desktop, args.tui]
                },
            ),
            indent=2,
        )
        + "\n"
    )
    print(
        "PASS: actual five pages, appearance and connection at two native/terminal sizes"
    )


if __name__ == "__main__":
    main()
