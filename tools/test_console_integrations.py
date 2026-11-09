"""Fresh synthetic local Codex and optional Windows owner/WSL regressions."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
from test_local_service import Fixture, ROOT
from codex_mcp_smoke import smoke
from windows_wsl_driver import views


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    results = {}
    with Fixture() as f:
        results["codex"] = smoke(f, "alpha", f.cli("context", "alpha"))
        helper = ROOT / "spikes/p1-e/shared/target/debug/acp-ui-snapshot"
        results["windows_wsl_views"] = (
            views(f, "alpha", helper)
            if os.name != "nt"
            else dict(
                status="NOT_RUN", reason="WSL-side explicit owner driver required"
            )
        )
        assert f.db.read_bytes() == f.before
    args.out.mkdir(parents=True, exist_ok=True, mode=0o700)
    (args.out / "integrations.json").write_text(json.dumps(results, indent=2) + "\n")
    print(json.dumps(results))
    if any(v["status"] == "NOT_RUN" for v in results.values()):
        print("NOT_RUN: one or more explicit optional integration clients unavailable")


if __name__ == "__main__":
    main()
