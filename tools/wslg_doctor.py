"""Read-only, redacted Wayland doctor; child-only probes never change a session."""

from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import platform
import re
import socket
import stat
import shutil
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
ENV_KEYS = (
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "DISPLAY",
    "GDK_BACKEND",
    "GSK_RENDERER",
    "XDG_SESSION_TYPE",
    "DBUS_SESSION_BUS_ADDRESS",
)


def socket_info(path):
    result = dict(exists=False, reachable=False)
    try:
        info = path.stat()
        result.update(
            exists=True,
            is_socket=stat.S_ISSOCK(info.st_mode),
            owned_by_current_uid=info.st_uid == os.getuid(),
            mode=oct(stat.S_IMODE(info.st_mode)),
        )
        if not result["is_socket"]:
            return result
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(1)
            client.connect(str(path))
            result["reachable"] = True
            pid, uid, _ = struct.unpack(
                "3i", client.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12)
            )
            result["peer_same_uid"] = uid == os.getuid()
            try:
                comm = Path(f"/proc/{pid}/comm").read_text().strip()
                result["peer_compositor"] = (
                    comm
                    if comm in ("niri", "weston", "Xwayland")
                    else "other-or-namespace-hidden"
                )
            except OSError:
                result["peer_compositor"] = "namespace-hidden"
    except (OSError, ValueError):
        pass
    return result


def environment():
    values = {k: dict(present=bool(os.environ.get(k))) for k in ENV_KEYS}
    for key in ("GDK_BACKEND", "GSK_RENDERER", "XDG_SESSION_TYPE"):
        value = os.environ.get(key, "")
        if re.fullmatch(r"[a-zA-Z0-9,_-]{1,64}", value):
            values[key]["value"] = value
    wayland = os.environ.get("WAYLAND_DISPLAY", "wayland-0")
    runtime = Path(os.environ.get("XDG_RUNTIME_DIR", "/nonexistent"))
    path = Path(wayland) if Path(wayland).is_absolute() else runtime / wayland
    resolved = path.resolve()
    category = (
        "WSLg Weston"
        if resolved.parent == Path("/mnt/wslg/runtime-dir")
        else "unidentified Wayland session"
    )
    info = socket_info(path)
    if info.get("peer_compositor") == "niri":
        category = "niri"
    values["WAYLAND_DISPLAY"]["socket_category"] = category
    display = os.environ.get("DISPLAY", "")
    x11 = (
        socket_info(Path("/tmp/.X11-unix") / ("X" + display.lstrip(":").split(".")[0]))
        if re.fullmatch(r":\d+(\.\d+)?", display)
        else dict(reachable=False)
    )
    return dict(
        environment=values,
        compositor=category,
        wayland_socket=info,
        x11_socket=x11,
        niri_socket_available=any(runtime.glob("niri.*.sock")),
    )


def version(command):
    try:
        p = subprocess.run(command, capture_output=True, text=True, timeout=10)
        return p.stdout.strip()[:512] if p.returncode == 0 else "NOT_RUN"
    except (OSError, subprocess.TimeoutExpired):
        return "NOT_RUN"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--probe", action="store_true")
    parser.add_argument("--environment-only", action="store_true")
    parser.add_argument(
        "--context",
        choices=[
            "codex-subprocess",
            "windows-exec",
            "interactive-terminal",
            "niri-terminal",
        ],
        default="codex-subprocess",
    )
    parser.add_argument("--wsl-distribution")
    parser.add_argument("--wsl-user")
    args = parser.parse_args()
    if platform.system() != "Linux":
        print("NOT_RUN: Linux Wayland doctor")
        return
    if bool(args.wsl_distribution) != bool(args.wsl_user):
        parser.error("explicit WSL distribution and user are required together")
    report = environment()
    report["launch_context"] = args.context
    report["interactive_terminal"] = (
        "NOT_RUN: not invoked from an interactive operator terminal"
    )
    report["niri_terminal"] = "NOT_RUN: no niri session selected"
    if args.wsl_distribution:
        launcher = shutil.which("wsl.exe")
        if not launcher:
            report["windows_exec"] = "NOT_RUN: wsl.exe unavailable"
        else:
            p = subprocess.run(
                [
                    launcher,
                    "--distribution",
                    args.wsl_distribution,
                    "--user",
                    args.wsl_user,
                    "--exec",
                    sys.executable,
                    str(Path(__file__).resolve()),
                    "--environment-only",
                    "--context",
                    "windows-exec",
                ],
                capture_output=True,
                text=True,
                timeout=20,
            )
            report["windows_exec"] = (
                json.loads(p.stdout) if p.returncode == 0 else dict(status="FAIL")
            )
    if not args.environment_only:
        report.update(
            kernel=platform.release(),
            wsl="microsoft" in platform.release().lower(),
            gtk=version(["pkg-config", "--modversion", "gtk4"]),
            niri=version(["niri", "--version"]),
            nsh_revision="87c7e8da85d2f15a56107baa6f0fabb419b508b3",
        )
        versions = Path("/mnt/wslg/versions.txt")
        if versions.is_file():
            report["wslg_versions"] = versions.read_text()[:2048]
        binary = ROOT / "spikes/p1-e/nsh/target/release/acp-nsh-native"
        if binary.is_file():
            linked = subprocess.run(
                ["ldd", str(binary)], capture_output=True, text=True, timeout=10
            ).stdout
            report["dynamic_libraries"] = [
                line.strip().split()[0]
                for line in linked.splitlines()
                if line.strip()
                and line.strip().startswith(
                    (
                        "libgtk",
                        "libgdk",
                        "libEGL",
                        "libGL",
                        "libwayland",
                        "libgstreamer",
                    )
                )
            ]
        report["client_egl_version"] = version(["pkg-config", "--modversion", "egl"])
        report["client_mesa_version"] = version(["pkg-config", "--modversion", "gbm"])
        os_version = Path("/etc/os-release").read_text()
        report["distro"] = dict(
            re.findall(r"^(ID|VERSION_ID|BUILD_ID)=(.*)$", os_version, re.M)
        )
        if args.probe:
            if not args.out:
                parser.error("--probe requires private --out")
            args.out.mkdir(parents=True, exist_ok=True, mode=0o700)
            args.out.chmod(0o700)
            probes = []
            for backend in ("auto", "wayland"):
                env = dict(os.environ)
                if backend == "auto":
                    env.pop("GDK_BACKEND", None)
                else:
                    env["GDK_BACKEND"] = "wayland"
                try:
                    p = subprocess.run(
                        [sys.executable, str(ROOT / "tools/gtk_wayland_sample.py")],
                        env=env,
                        capture_output=True,
                        text=True,
                        timeout=15,
                    )
                    (args.out / ("gtk-" + backend + ".log")).write_text(p.stderr)
                    probes.append(
                        dict(
                            selection=backend,
                            status="PASS" if p.returncode == 0 else "FAIL",
                            observation=(
                                json.loads(p.stdout) if p.returncode == 0 else None
                            ),
                        )
                    )
                except (OSError, subprocess.TimeoutExpired, ValueError):
                    probes.append(dict(selection=backend, status="FAIL"))
            report["gtk_positive_control"] = probes
    report["status"] = (
        "PASS" if report["wayland_socket"].get("reachable") else "BLOCKED"
    )
    if any(p["status"] == "FAIL" for p in report.get("gtk_positive_control", [])):
        report["status"] = "FAIL"
    if args.out:
        args.out.mkdir(parents=True, exist_ok=True, mode=0o700)
        args.out.chmod(0o700)
        (args.out / "doctor.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))
    return (
        1 if report["status"] == "FAIL" else (2 if report["status"] == "BLOCKED" else 0)
    )


if __name__ == "__main__":
    raise SystemExit(main())
