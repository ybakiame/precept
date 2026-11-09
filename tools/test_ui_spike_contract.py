"""Synthetic authenticated UI client: explicit task lookup, scope, denied states.

No user profile discovery. Build root binaries and shared/native snapshot first.
"""
import json
import os
from pathlib import Path
import subprocess
from test_local_service import Fixture, ROOT

model_bin = Path(os.environ.get("ACP_UI_SNAPSHOT", ROOT / "spikes/p1-e/shared/target/debug/acp-ui-snapshot"))
if os.name == "nt" and not model_bin.suffix:
    model_bin = model_bin.with_suffix(".exe")

def snapshot(f, *flags, success=True):
    p = subprocess.run([str(model_bin), *f.location, *flags], capture_output=True, text=True, encoding="utf-8", timeout=30)
    assert (p.returncode == 0) == success, "UI client outcome differs"
    return json.loads(p.stdout) if success else p.stderr

with Fixture() as f:
    before = f.db.read_bytes()
    for name in ["alpha", "beta"]:
        data = snapshot(f, "--project", name, "--task-id", "shared")
        assert [p["name"] for p in data["projects"]] == ["alpha", "beta"]
        project = next(p for p in data["projects"] if p["name"] == name)
        reference = f.cli("context", name)
        assert project["context"]["project"] == name
        assert project["context"]["preferences"] == [{k: v for k, v in p.items() if k != "id"} for p in reference["preferences"]]
        h = f.cli("handoff", name, "shared")
        assert project["handoff"]["validation"] == h["validation"]
        assert project["handoff"]["resume_warnings"] == h["resume_warnings"]
        assert project["handoff"]["current_repository"]["worktree_fingerprint"] is None
        other = next(p for p in data["projects"] if p["name"] != name)
        assert not other["context"]["preferences"] and other["handoff"] is None
    empty = snapshot(f, "--project", "alpha", "--task-id", "beta-only")
    assert empty["projects"][0]["handoff"] is None
    assert snapshot(f)["projects"][0]["handoff"] is None, "implicit task lookup"
    for name in ["unknown", "../beta"]:
        assert "Permission denied" in snapshot(f, "--project", name, success=False)
    assert "Invalid task ID" in snapshot(f, "--task-id", "a\nb", success=False)
    cap = f.runtime / "operator.cap"
    raw = cap.read_bytes()
    cap.unlink()
    snapshot(f, success=False)
    cap.write_bytes(raw)
    if os.name != "nt":
        cap.chmod(0o644)
        assert "Permission denied" in snapshot(f, success=False)
        cap.chmod(0o600)
    assert snapshot(f)["health"]["ready"]
    assert f.cli("health")["ready"], "client exit stopped daemon"
    assert f.db.read_bytes() == before
print("TEST_PASS: selected project only, exact confirmed rows/evidence, empty task, hostile scope, removed/broad credentials, reconnect, unchanged synthetic DB")
