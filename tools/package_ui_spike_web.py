"""Assemble a static offline-only Rust NSH app with exact upstream renderer JS.

No daemon server/gateway, no credentials. Serving these static assets in tests
does not expose a daemon or proxy its native IPC.
"""
import argparse
from pathlib import Path
import shutil
import subprocess

PIN = "87c7e8da85d2f15a56107baa6f0fabb419b508b3"
parser = argparse.ArgumentParser()
parser.add_argument("--source", type=Path, required=True, help="pinned longbridge/nsh checkout")
parser.add_argument("--wasm", type=Path, required=True)
parser.add_argument("--out", type=Path, required=True)
args = parser.parse_args()
assert subprocess.check_output(["git", "-C", str(args.source), "rev-parse", "HEAD"], text=True).strip() == PIN
args.out.mkdir(parents=True, exist_ok=True)
upstream = args.source / "crates/nsh-platform-web/web"
shutil.copy2(upstream / "nsh.js", args.out / "nsh.js")
shutil.copytree(upstream / "browser", args.out / "browser", dirs_exist_ok=True)
shutil.copy2(args.source / "LICENSE-APACHE", args.out / "NSH-LICENSE-APACHE.txt")
shutil.copy2(args.wasm, args.out / "acp_nsh_spike.wasm")
web = Path(__file__).resolve().parents[1] / "spikes/p1-e/nsh/web"
for file in ["index.html", "start.js"]:
    shutil.copy2(web / file, args.out / file)
print("BUILD_PASS: offline static assets assembled; bytes", sum(p.stat().st_size for p in args.out.rglob("*") if p.is_file()))
