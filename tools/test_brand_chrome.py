"""B0 custom title-bar interaction on owned synthetic windows only."""

import argparse
import json
import os
from pathlib import Path

from local_console_probe import native_start, native_close
from test_local_service import ROOT, stop


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--binary", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--product", action="store_true")
    args = p.parse_args()
    out = args.out.resolve()
    assert any(
        out.is_relative_to(ROOT / folder)
        for folder in ["artifacts/local-p1-f", "artifacts/local-p1-g"]
    )
    out.mkdir(parents=True, exist_ok=True, mode=0o700)
    proc, window, _ = native_start(
        [
            str(args.binary.resolve()),
            *([] if args.product else ["--brand-preview", "a"]),
        ]
    )
    try:
        if os.name == "nt":
            import ctypes
            from ctypes import wintypes as W
            from window_chrome_probe import exercise_custom_chrome

            user = ctypes.WinDLL("user32", use_last_error=True)
            user.GetWindowLongW.argtypes = [W.HWND, ctypes.c_int]
            assert (
                not user.GetWindowLongW(window, -16) & 0x00C00000
            ), "system caption remains"
            result = exercise_custom_chrome(window)
            result["system_caption"] = False
        else:
            from wayland_probe import control

            window.wait("decorated=false")
            result = control(window.nonce, "chrome")
            result["backend"] = "GdkWaylandDisplay"
            result["gtk_decorated"] = False
        assert proc.wait(timeout=10) == 0
        (out / "chrome-result.json").write_text(
            json.dumps(result, indent=2) + "\n", encoding="utf-8"
        )
        print(json.dumps(result))
    finally:
        if proc.poll() is None:
            native_close(window)
            stop(proc)


if __name__ == "__main__":
    main()
