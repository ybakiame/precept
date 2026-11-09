"""WSL-hosted test driver for a Windows-owner read-only WSL connection.

Requires explicit Windows snapshot binary, distro and Linux user. The Windows
client invokes wsl.exe --exec, with no shell, listener or credential transfer.
Raw snapshots remain only in child pipes/memory. Real project opt-in is handled
by test_local_project.py, never guessed here.
"""
import argparse
import json
import subprocess
from pathlib import Path
from test_local_service import Fixture


def connection(binary, distro, user, helper, fixture, project="alpha", task="shared"):
    return [str(binary), "--wsl-distribution", distro, "--wsl-user", user,
            "--wsl-snapshot", str(helper), "--wsl-runtime-dir", str(fixture.runtime),
            "--wsl-endpoint", fixture.endpoint, "--project", project, "--task-id", task]


def check_snapshot(command, fixture, project, success=True):
    process = subprocess.run(command, capture_output=True, timeout=30)
    assert (process.returncode == 0) == success, "Windows owner/WSL query outcome differs"
    if success:
        snapshot = json.loads(process.stdout)
        item = next(p for p in snapshot["projects"] if p["name"] == project)
        reference = fixture.cli("context", project)
        assert item["context"]["project"] == project
        assert item["context"]["repository"] == reference["repository"]
        assert item["context"]["preferences"] == [{k:v for k,v in p.items() if k != "id"} for p in reference["preferences"]]
        assert all(p["status"] == "confirmed" for p in item["context"]["preferences"])
    return process


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--windows-snapshot", required=True, type=Path)
    p.add_argument("--distribution", required=True)
    p.add_argument("--user", required=True)
    p.add_argument("--helper", required=True, type=Path)
    p.add_argument("--out", required=True, type=Path)
    args = p.parse_args()
    assert args.windows_snapshot.is_absolute() and args.windows_snapshot.is_file()
    assert args.helper.is_absolute() and args.helper.is_file()
    with Fixture(allow=("alpha",)) as f:
        before = f.db.read_bytes()
        command = connection(args.windows_snapshot, args.distribution, args.user, args.helper, f)
        check_snapshot(command, f, "alpha")
        check_snapshot(connection(args.windows_snapshot, args.distribution, args.user, args.helper, f, "beta"), f, "beta", False)
        check_snapshot(connection(args.windows_snapshot, args.distribution, args.user, args.helper, f, "../beta"), f, "../beta", False)
        cap = f.runtime / "operator.cap"
        raw = cap.read_bytes()
        cap.unlink()
        check_snapshot(command, f, "alpha", False)
        cap.write_bytes(raw); cap.chmod(0o600)
        check_snapshot(command, f, "alpha")
        from test_local_service import stop
        stop(f.proc)
        check_snapshot(command, f, "alpha", False)
        f.start()
        check_snapshot(command, f, "alpha")
        assert f.cli("health")["ready"] and f.db.read_bytes() == before
    result = dict(status="PASS", host_client="Windows MSVC executable", transport="explicit wsl.exe --distribution/--user/--exec -> WSL helper -> authenticated Unix IPC",
                  checks=["exact context parity", "foreign project denied", "hostile project denied", "removed credentials denied", "daemon stop/refusal/restart", "manual query reconnect", "unchanged disposable DB", "independent process lifetime"],
                  credentials="remain inside WSL private runtime; no token argv/environment/Windows copies", data="synthetic only")
    args.out.mkdir(parents=True, exist_ok=True, mode=0o700)
    (args.out / "windows-wsl-result.json").write_text(json.dumps(result, indent=2) + "\n")
    print("PASS: Windows owner -> WSL authenticated read-only IPC; isolation/credential/lifecycle regression checks")


if __name__ == "__main__":
    main()
