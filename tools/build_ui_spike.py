"""Independent optional UI build gates; every invoked failure propagates."""
import argparse
import json
import os
from pathlib import Path
import platform
import subprocess
import time

p=argparse.ArgumentParser();p.add_argument("candidate",choices=["gpui-kit","nsh","hybrid"]);args=p.parse_args()
candidate=args.candidate
manifest=Path("spikes/p1-e")/("gpui-kit" if candidate=="hybrid" else candidate)/"Cargo.toml"
out=Path("spikes/p1-e/artifacts");out.mkdir(parents=True,exist_ok=True)
result={"candidate":candidate,"os":platform.platform(),"commands":[],"status":"BLOCKED"}
def run(tail, deny=False):
    cmd=["cargo",*tail,"--manifest-path",str(manifest),"--locked"]
    if deny:cmd += ["--","-D","warnings"]
    started=time.monotonic();proc=subprocess.run(cmd)
    result["commands"].append({"argv":cmd,"exit":proc.returncode,"wall_seconds":round(time.monotonic()-started,3)})
    if proc.returncode:raise SystemExit(proc.returncode)
try:
    subprocess.run(["cargo","fmt","--manifest-path",str(manifest),"--","--check"],check=True)
    if candidate=="nsh":
        run(["clippy","--all-targets","--features","native,tui"],deny=True)
        run(["test","--lib"])
        run(["build","--features","native","--bin","acp-nsh-native"])
    else:
        run(["clippy","--all-targets","--features","hybrid" if candidate=="hybrid" else "ui-tests"],deny=True)
        if candidate=="gpui-kit":run(["test","--features","ui-tests","--bin","acp-gpui-spike"])
        run(["build",*(["--features","hybrid"] if candidate=="hybrid" else [])])
    result["status"]="BUILD_PASS"
finally:
    (out/(candidate+"-build-"+platform.system()+".json")).write_text(json.dumps(result,indent=2));print(json.dumps(result))
