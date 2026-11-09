"""Synthetic daemon -> unchanged authenticated IPC -> neutral UI model.

Build shared with --features native and root service bins first. No user DB lookup.
"""
import json
import os
from pathlib import Path
import subprocess
from test_local_service import Fixture, ROOT

model_bin = Path(os.environ.get("ACP_UI_SNAPSHOT", ROOT / "spikes/p1-e/shared/target/debug/acp-ui-snapshot"))
if os.name == "nt" and not model_bin.suffix:
    model_bin = model_bin.with_suffix(".exe")
with Fixture() as f:
    before = f.db.read_bytes()
    result = subprocess.run([str(model_bin), *f.location], capture_output=True, text=True, encoding="utf-8", timeout=30, check=True)
    snapshot = json.loads(result.stdout)
    assert snapshot["health"]["ready"]
    assert [p["name"] for p in snapshot["projects"]] == ["alpha", "beta"]
    for project in snapshot["projects"]:
        name = project["name"]
        reference = f.cli("context", name)
        assert project["context"]["project"] == name
        assert all(p["status"] == "confirmed" for p in project["context"]["preferences"])
        # Typed rows omit IDs, but preserve actual key/value/scope/source/status.
        assert project["context"]["preferences"] == [{k: v for k, v in p.items() if k != "id"} for p in reference["preferences"]]
        handoff = f.cli("handoff", name, "shared")
        assert project["handoff"]["validation"] == handoff["validation"]
        assert project["handoff"]["resume_warnings"] == handoff["resume_warnings"]
        assert project["handoff"]["current_repository"]["worktree_fingerprint"] is None
    assert "token" not in snapshot and not result.stderr
    assert f.cli("health")["ready"], "client exit stopped daemon"
    assert f.db.read_bytes() == before
    print("TEST_PASS: two allowlisted UI projects, exact confirmed rows/opaque evidence/warnings, unknown fingerprint, unchanged synthetic DB, daemon outlives client")
