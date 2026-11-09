"""Assemble an unsigned development archive on this host; never publish/install."""
import argparse
import hashlib
import json
import platform
import tarfile
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out", type=Path, default=ROOT / "artifacts/local-p1-e2/package")
    args = p.parse_args()
    suffix = ".exe" if platform.system() == "Windows" else ""
    files = [ROOT / "spikes/p1-e/nsh/target/release" / (name + suffix) for name in ["acp-nsh-native", "acp-nsh-tui"]]
    if not all(f.is_file() for f in files):
        print("NOT_RUN: both host release executables required")
        return
    args.out.mkdir(parents=True, exist_ok=True)
    notes = args.out / "README.txt"
    notes.write_text('''Agent Control Plane — unsigned development binaries
Read-only NSH desktop and TUI. No installer, autostart or daemon startup.
Run without flags for OFFLINE_SYNTHETIC; Ctrl+Q closes the TUI.
For native live reads, explicitly start acp-daemon separately, then supply
--runtime-dir <private-runtime> --endpoint <socket-path-or-pipe-label>.
Optional --project <allowlisted-label> --task-id <exact-task-id>.
Windows owner -> WSL: supply all five --wsl-distribution, --wsl-user,
--wsl-snapshot, --wsl-runtime-dir, --wsl-endpoint options explicitly.
The existing WSL query helper runs via wsl.exe --exec; no shell/TCP/token copy.
Do not pass both native and WSL location flags. WSL daemon is never started.
Windows: Windows 10/11 x64 with MSVC/UCRT runtime and ConPTY for TUI.
Linux: GTK4 >=4.14, GStreamer 1.x, Fontconfig and an X11/Wayland desktop;
a VT/truecolor terminal for TUI. macOS requires its own local build/verification.
Keep credentials and runtime outside checkouts; use only an operator-authorized DB.
See docs/p1/local-codex-e2.md for lifecycle, permissions and local validation.
''', encoding="utf-8")
    manifest = dict(kind="unsigned development archive", os=platform.system(), architecture=platform.machine(),
                    files={f.name: dict(bytes=f.stat().st_size, sha256=hashlib.sha256(f.read_bytes()).hexdigest()) for f in files})
    metadata = args.out / "manifest.json"
    metadata.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    basename = f"acp-nsh-dev-{platform.system().lower()}-{platform.machine().lower()}"
    if platform.system() == "Windows":
        archive = args.out / (basename + ".zip")
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
            for path in [*files, notes, metadata]:
                bundle.write(path, path.name)
    else:
        archive = args.out / (basename + ".tar.gz")
        with tarfile.open(archive, "w:gz") as bundle:
            for path in [*files, notes, metadata]:
                bundle.add(path, arcname=path.name, recursive=False)
    print("PASS: host-only unsigned desktop/TUI development archive created locally")


if __name__ == "__main__":
    main()
