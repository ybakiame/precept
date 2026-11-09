"""Clean target + warm no-op timings; cached downloads are explicitly not cold.

Uses a new temporary target per app, then removes only that owned target.
System libraries/toolchain must already be installed. No daemon/data access.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

p=argparse.ArgumentParser();p.add_argument("--out",type=Path,required=True);args=p.parse_args()
args.out.mkdir(parents=True,exist_ok=True)
results=[]
for candidate in ["gpui-kit","nsh"]:
    manifest=Path("spikes/p1-e")/candidate/"Cargo.toml"
    with tempfile.TemporaryDirectory(prefix="compile-"+candidate+"-",dir=args.out) as target:
        env={**os.environ,"CARGO_TARGET_DIR":str(Path(target).resolve()),"CARGO_BUILD_JOBS":"2"}
        cmd=["cargo","build","--manifest-path",str(manifest),"--locked"]
        if candidate=="nsh":cmd += ["--features","native","--bin","acp-nsh-native"]
        row={"candidate":candidate,"profile":"dev debug=0","jobs":2,"registry":"warm cached downloads; OS file cache not flushed","command":cmd}
        for kind in ["clean_target","warm_noop"]:
            with (args.out/(candidate+"-compile-"+kind+".log")).open("w") as log:
                started=time.monotonic();proc=subprocess.run(cmd,env=env,stdout=log,stderr=subprocess.STDOUT)
            row[kind]={"seconds":round(time.monotonic()-started,3),"exit":proc.returncode}
            if proc.returncode:break
        results.append(row)
        (args.out/"compile-measurements.json").write_text(json.dumps(results,indent=2))
        print(json.dumps(row),flush=True)
        if proc.returncode:raise SystemExit(proc.returncode)
