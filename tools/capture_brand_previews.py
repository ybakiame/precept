"""B0 synthetic-only owned NSH previews; private captures, never desktop pixels."""

import argparse
import hashlib
import json
import os
import time
from pathlib import Path

from local_console_probe import TerminalProbe, native_close, native_start, windows_api
from test_local_service import ROOT, stop
from window_capture import capture_owned_window
from check_caption_alignment import caption_alignment


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--desktop", type=Path, required=True)
    parser.add_argument("--tui", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--locale", choices=["en", "zh-CN"], default="en")
    parser.add_argument(
        "--appearance", choices=["system", "light", "dark"], default="dark"
    )
    parser.add_argument("--themes", default="abc")
    parser.add_argument("--pages", default="dashboard,handoff,appearance")
    args = parser.parse_args()
    out = args.out.resolve()
    assert out.is_relative_to(
        ROOT / "artifacts/local-p1-f"
    ), "private P1-F output required"
    out.mkdir(parents=True, exist_ok=True, mode=0o700)
    results = []
    assert args.themes and all(c in "abc" for c in args.themes)
    pages = args.pages.split(",")
    assert pages and all(p in ("dashboard", "handoff", "appearance") for p in pages)
    for direction in args.themes:
        for page in pages:
            flags = [
                "--brand-preview",
                direction,
                "--preview-page",
                page,
                "--locale",
                args.locale,
                "--appearance",
                args.appearance,
            ]
            proc, window, sample = native_start([str(args.desktop.resolve()), *flags])
            try:
                for width, height in [(1080, 760), (640, 480)]:
                    path = out / f"{direction}-{page}-{width}.png"
                    if os.name == "nt":
                        user = windows_api()
                        assert user.SetWindowPos(
                            window, None, 80, 80, width, height, 0x40
                        )
                        time.sleep(0.3)
                        capture_owned_window(window, path)
                    else:
                        window.wait("decorated=false")
                        window.resize(width, height, frameless=True)
                        time.sleep(0.15)
                        window.capture(path, frameless=True)
                    alignment = caption_alignment(path)
                results.append(
                    dict(
                        direction=direction,
                        page=page,
                        desktop="PASS",
                        observed=sample,
                        caption_alignment=alignment,
                    )
                )
            finally:
                native_close(window)
                stop(proc)
            terminal = TerminalProbe([str(args.tui.resolve()), *flags])
            try:
                terminal.wait("OFFLINE_SYNTHETIC")
                expected = {
                    "en": {
                        "dashboard": "Confirmed preferences",
                        "handoff": "Current repository uncertainty",
                        "appearance": "Theme collection",
                    },
                    "zh-CN": {
                        "dashboard": "已确认偏好",
                        "handoff": "当前仓库的不确定项",
                        "appearance": "主题集",
                    },
                }[args.locale][page]
                terminal.wait(expected)
                for width, height in [(160, 50), (80, 30)]:
                    terminal.resize(width, height)
                    time.sleep(0.15)
                    terminal.wait("OFFLINE_SYNTHETIC")
                    terminal.wait(
                        {
                            "en": {
                                "handoff": "Handoff",
                                "dashboard": "Dashboard",
                                "appearance": "Theme collection",
                            },
                            "zh-CN": {
                                "handoff": "交接",
                                "dashboard": "概览",
                                "appearance": "主题集",
                            },
                        }[args.locale][page]
                    )
                    (out / f"{direction}-{page}-{width}.txt").write_text(
                        "\n".join(terminal.screen.display) + "\n", encoding="utf-8"
                    )
                results[-1]["terminal"] = "PASS"
            finally:
                terminal.close()
    report = dict(
        status="PASS",
        stage="B0_PREVIEW_ONLY",
        synthetic_only=True,
        locale=args.locale,
        appearance=args.appearance,
        host=(
            "Windows native + ConPTY"
            if os.name == "nt"
            else "WSLg native Wayland + PTY"
        ),
        capture="owned HWND PrintWindow; no desktop pixels",
        samples=results,
        binaries={
            p.name: hashlib.sha256(p.read_bytes()).hexdigest()
            for p in [args.desktop, args.tui]
        },
    )
    (out / "report.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8"
    )
    print(
        json.dumps(
            {k: report[k] for k in ("status", "stage", "host", "synthetic_only")}
        )
    )


if __name__ == "__main__":
    main()
