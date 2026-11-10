"""Real Linux NSH native Wayland acceptance; synthetic-only windows and data."""

from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time
from local_console_probe import native_available
from test_local_service import Fixture, stop
from wayland_probe import WaylandWindow, control
from wslg_doctor import environment


def verify(binary, out):
    out.mkdir(parents=True, exist_ok=True, mode=0o700)
    out.chmod(0o700)
    reason = native_available()
    if reason:
        print("BLOCKED: " + reason)
        return 2
    metadata = environment()
    # niri requires a scoped compositor capture/input adapter; do not route it
    # through a Weston/RAIL window and mislabel the session as niri.
    if metadata["compositor"] != "WSLg Weston":
        print(
            "BLOCKED: niri/non-WSLg compositor needs an authorized window-scoped input/capture adapter"
        )
        return 2
    import psutil

    samples = []
    with Fixture() as f:
        for i in range(3):
            w = WaylandWindow(
                [
                    str(binary.resolve()),
                    *f.location,
                    "--project",
                    "alpha",
                    "--task-id",
                    "shared",
                ],
                backend="auto" if i == 0 else "wayland",
            )
            try:
                w.state(selected=0)
                sample = {
                    **w.sample,
                    "selection": "auto" if i == 0 else "wayland",
                    "rss_bytes": psutil.Process(w.proc.pid).memory_info().rss,
                }
                start = w.key("ctrl+p")
                w.state(selected=1, after=start)
                w.key("ctrl+k")
                w.key("quality-task")
                start = w.key("Enter")
                w.state(selected=1, after=start)
                w.wait("queried_chars=12", after=start)
                start = w.key("ctrl+f")
                w.key("nomatch")
                w.state(selected=1, rows=0, after=start)
                w.key("Tab")
                w.key("Escape")
                w.resize(640, 480, frameless=True)
                w.resize(1080, 760, frameless=True)
                if i == 0:
                    stop(f.proc)
                    start = w.key("ctrl+r")
                    w.wait("phase=Unavailable", after=start)
                    f.start()
                    start = w.key("ctrl+r")
                    w.state(selected=1, rows=0, after=start)
                    # Replaced credentials must fail closed without restoring the
                    # previous context. Restore only the owned test capability.
                    cap = f.runtime / "operator.cap"
                    raw = cap.read_bytes()
                    cap.write_bytes(b"0" * len(raw))
                    try:
                        start = w.key("ctrl+r")
                        w.wait("phase=Unavailable", after=start)
                    finally:
                        cap.write_bytes(raw)
                    start = w.key("ctrl+r")
                    w.state(selected=1, rows=0, after=start)
                samples.append(sample)
            finally:
                w.drain()
                (out / f"wayland-synthetic-{i}.log").write_text("".join(w.lines))
                try:
                    w.close()
                finally:
                    w.close(hard=True)
            assert f.cli("health")["ready"]
        assert (
            f.db.read_bytes() == f.before
        ), "disposable database changed during UI reads"
    w = WaylandWindow(
        [str(binary.resolve()), "--synthetic-rows", "10000"], backend="wayland"
    )
    try:
        w.state(rows=10000)
        start = w.key("alt+3")
        w.wait("section=Preferences", after=start)
        time.sleep(0.2)
        control(w.nonce, "click", position=[365, 246], frameless=True)
        w.wait("page=1")
        start = w.key("alt+Right")
        w.wait("page=2", after=start)
        control(w.nonce, "click", position=[1015, 320], frameless=True)
        w.wait("detail=true")
        time.sleep(0.2)
        w.capture(out / "wayland-detail.png", frameless=True)
        start = w.key("ctrl+d")
        w.wait("dark=false", after=start)
        w.capture(out / "wayland-light.png", frameless=True)
        start = w.key("ctrl+shift+h")
        w.wait("contrast=true", after=start)
        w.capture(out / "wayland-contrast.png", frameless=True)
        rss = psutil.Process(w.proc.pid).memory_info().rss
        started = time.monotonic()
        start = w.key("ctrl+f")
        w.key("event-09999")
        w.state(rows=1, after=start)
        model_seconds = time.monotonic() - started
        assert (
            model_seconds < 2
        ), "10k Wayland filter/model acknowledgement exceeded local two-second bound"
        # Model acknowledgement is NOT a frame timestamp. Actual opaque window
        # capture follows separately and is visually reviewed locally.
        time.sleep(0.25)
        capture = w.capture(out / "wayland-10k-content.png", frameless=True)
        captured_seconds = time.monotonic() - started
        w.resize(640, 480, frameless=True)
        w.capture(out / "wayland-640-content.png", frameless=True)
        workload = dict(
            rows=10000,
            filtered_rows=1,
            model_ack_seconds=round(model_seconds, 4),
            capture_completed_seconds=round(captured_seconds, 4),
            capture=capture,
            paging_detail_themes="PASS: actual pointer next page/detail, Alt+Right, light and contrast",
            input="real Win32 keys -> WSLg RAIL -> Wayland keyboard -> GTK",
            rss_before_filter=rss,
            rss_after_filter=psutil.Process(w.proc.pid).memory_info().rss,
        )
    finally:
        w.drain()
        (out / "wayland-10k.log").write_text("".join(w.lines))
        try:
            w.close()
        finally:
            w.close(hard=True)
    # A missing Wayland socket must never silently become a working X11 window.
    env = {
        **os.environ,
        "GDK_BACKEND": "wayland",
        "WAYLAND_DISPLAY": "/nonexistent/acp-e3-wayland-test",
    }
    p = subprocess.run(
        [str(binary.resolve()), "--synthetic-rows", "1"],
        env=env,
        capture_output=True,
        timeout=10,
    )
    assert p.returncode != 0, "failed Wayland unexpectedly fell back to another backend"
    # Automatic GTK selection may fall back to X11 when its Wayland socket is
    # broken. The acceptance adapter must reject that *actual* backend.
    try:
        unexpected = WaylandWindow(
            [str(binary.resolve()), "--synthetic-rows", "1"],
            extra_env={"WAYLAND_DISPLAY": "/nonexistent/acp-e3-wayland-test"},
        )
    except AssertionError as error:
        assert "actual GTK backend is X11" in str(
            error
        ), "unexpected fallback failure classification"
    else:
        unexpected.close(hard=True)
        raise AssertionError("automatic X11 fallback was accepted as Wayland")
    cairo = WaylandWindow(
        [str(binary.resolve()), "--synthetic-rows", "1"],
        backend="wayland",
        extra_env={"GSK_RENDERER": "cairo"},
    )
    try:
        assert cairo.sample["renderer"] == "GskCairoRenderer"
        cairo_observation = dict(
            status="PASS",
            backend=cairo.sample["backend"],
            renderer=cairo.sample["renderer"],
            hardware_acceleration=False,
        )
    finally:
        try:
            cairo.close()
        finally:
            cairo.close(hard=True)
    result = dict(
        status="PASS",
        acceptance="RUN_PASS_WAYLAND_WSLG",
        niri="NOT_RUN: owner chose not to start niri this run",
        backend="GdkWaylandDisplay",
        compositor=metadata["compositor"],
        profile="release",
        sample_count=len(samples),
        samples=samples,
        rendered_10k=workload,
        resize="real border drag; exact GTK allocations 640x480 and 1080x760",
        keyboard="project/task/filter/Tab/Escape/reconnect via actual Wayland input",
        independent_lifetimes=True,
        database_unchanged=True,
        missing_wayland_no_fallback=True,
        automatic_x11_fallback_rejected=True,
        per_child_cairo_diagnostic=cairo_observation,
        binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        binary_bytes=binary.stat().st_size,
        acceleration="NOT_VERIFIED: GskGLRenderer is not proof of hardware acceleration",
        painted_screen="target HWND PrintWindow captures (no desktop pixels); model/GTK paint markers are separate observations",
    )
    (out / "wayland-result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--binary", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    args = p.parse_args()
    return verify(args.binary, args.out)


if __name__ == "__main__":
    raise SystemExit(main())
