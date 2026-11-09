"""Explicit opt-in, non-destructive acceptance against one local checkout.

The checkout is input data only. No repo scripts/hooks/builds/commands from
AGENTS.md are executed. Only fresh private ACP state/runtime may be written.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from test_local_service import Fixture, Bridge, Raw, binary, stop, ROOT
from codex_mcp_smoke import smoke


def git(root, *args):
    env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
    env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull, GIT_OPTIONAL_LOCKS="0", GIT_TERMINAL_PROMPT="0")
    # A temporary output file bounds retained memory and never belongs to the repo.
    with tempfile.TemporaryFile() as output:
        p = subprocess.run(["git", "-c", "core.fsmonitor=false", "-c", "core.untrackedCache=false",
                            "-c", "core.hooksPath=" + os.devnull, "-C", str(root), *args],
                           env=env, stdout=output, stderr=subprocess.DEVNULL, timeout=8)
        assert p.returncode == 0, "read-only Git inspection failed"
        assert output.tell() <= 65536, "Git metadata exceeds bounded acceptance limit"
        output.seek(0)
        return output.read()


def inspect(root):
    if (root / ".git").exists():
        top = Path(os.fsdecode(git(root, "rev-parse", "--show-toplevel")).strip()).resolve()
        assert top == root, "selected path must be the checkout root"
        head = git(root, "rev-parse", "HEAD").strip()
        branch = git(root, "branch", "--show-current").strip()
        keys = (
            git(root, "config", "--includes", "--name-only", "--list")
            .decode("utf-8", "replace")
            .lower()
            .splitlines()
        )
        scan_permitted = not (root / ".gitmodules").exists() and not any(
            k.startswith("submodule.")
            or k.startswith("filter.")
            and (k.endswith(".clean") or k.endswith(".process"))
            for k in keys
        )
        # Refuse external programs while still testing scoped metadata/UI reads.
        # None is Unknown, never an empty/clean status observation.
        status = git(root, "status", "--porcelain=v1", "-z", "--untracked-files=normal") if scan_permitted else None
        git_dir = Path(os.fsdecode(git(root, "rev-parse", "--absolute-git-dir")).strip())
    else:
        head = branch = status = b""
        git_dir = root / ".git"
    # Metadata only: no raw file contents/diffs, recursive traversal or atime checks.
    paths = [root, root / ".git", *[git_dir / n for n in ["HEAD", "index", "config", "packed-refs"]],
             *[root / n for n in ["AGENTS.md", "README.md", "docs/index.md", "docs/README.md", "docs/adr/README.md"]]]
    metadata = []
    for p in paths:
        try:
            s = p.lstat()
            metadata.append((str(p), s.st_mode, s.st_size, s.st_mtime_ns, s.st_ino))
        except FileNotFoundError:
            metadata.append((str(p), None))
    return dict(head=head.hex(), branch=branch.hex(), status=status.hex() if status is not None else None, metadata=metadata)


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


class Selected(Fixture):
    def __init__(self, root, label):
        # Do not call the synthetic Fixture initializer or any default Store.
        self.tmp = tempfile.TemporaryDirectory(prefix="acpe2-", dir="/tmp" if os.name != "nt" else None)
        self.root = Path(self.tmp.name).resolve()
        assert not self.root.is_relative_to(root) and not self.root.is_relative_to(ROOT)
        self.runtime = self.root / "run"
        self.db = self.root / "state/context.sqlite3"
        self.endpoint = "e2-" + self.root.name if os.name == "nt" else str(self.runtime / "control.sock")
        self.location = ["--runtime-dir", str(self.runtime), "--endpoint", self.endpoint]
        self.children, self.errors, self.allow, self.environment, self.proc = [], [], (label,), None, None
        self.label = label
        self.cli_write("project", "add", label, str(root))
        second = self.root / "synthetic-project"
        second.mkdir()
        (second / "AGENTS.md").write_text("Synthetic isolation fixture only.\n", encoding="utf-8")
        self.cli_write("project", "add", "beta", str(second))
        # Only this owned synthetic checkout gets init/add/commit. The external
        # selected root is never passed to a Git mutation or handoff capture.
        git_base = ["git", "-c", "core.hooksPath=" + os.devnull, "-c", "user.name=Synthetic", "-c", "user.email=synthetic@example.invalid"]
        for command in [[*git_base, "init", "-q", str(second)], [*git_base, "-C", str(second), "add", "AGENTS.md"],
                        [*git_base, "-C", str(second), "commit", "-qm", "synthetic-isolation"]]:
            subprocess.run(command, capture_output=True, check=True, timeout=10)
        self.cli_write("handoff", "put", "--project", "beta", "--task-id", "beta-only", "--objective", "SYNTHETIC_OTHER_PROJECT", "--next-action", "inspect synthetic fixture")
        # Unconfirmed suggestions in disposable state must remain invisible.
        self.cli_write("preference", "suggest", "--project", label, "--key", "synthetic.suggested", "--value", "SYNTHETIC_NOT_CONFIRMED", "--source", "disposable-test")
        self.cli_write("preference", "suggest", "--project", "beta", "--key", "synthetic.other", "--value", "SYNTHETIC_OTHER_PROJECT", "--source", "disposable-test")
        self.before = self.db.read_bytes()
        try:
            self.start()
        except BaseException:
            self.close()
            raise

    def cli_write(self, *args):
        p = subprocess.run([sys.executable, "-m", "agent_control_plane.cli", "--data-dir", str(self.root / "state"), *args],
                           cwd=ROOT, env={**os.environ, "PYTHONPATH": str(ROOT / "src")}, capture_output=True, timeout=15)
        assert p.returncode == 0, "disposable operator CLI failed"


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out", required=True, type=Path)
    args = p.parse_args()
    supplied = os.environ.get("ACP_LOCAL_PROJECT_ROOT")
    if not supplied:
        print("NOT_RUN: ACP_LOCAL_PROJECT_ROOT not explicitly set")
        return
    # Validate before the first external project read.
    path = Path(supplied)
    assert path.is_absolute() and path.is_dir(), "explicit absolute existing checkout required"
    root = path.resolve(strict=True)
    assert root != ROOT and not root.is_relative_to(ROOT), "external checkout required"
    assert (root / ".git").exists() or (root / "AGENTS.md").is_file(), "checkout marker required"
    snapshot_binary = ROOT / "spikes/p1-e/shared/target/debug" / ("acp-ui-snapshot" + (".exe" if os.name == "nt" else ""))
    if not all(Path(binary(name)).is_file() for name in ["acp-daemon", "acp-query", "acp-mcp"]) or not snapshot_binary.is_file():
        print("NOT_RUN: build service and native snapshot binaries before selected-project acceptance")
        return
    label = os.environ.get("ACP_LOCAL_PROJECT_LABEL", "selected-local")
    assert re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,63}", label) and label != "beta", "invalid explicit project label"
    out = args.out.resolve()
    assert not out.is_relative_to(root), "diagnostics must stay outside external checkout"
    out.mkdir(parents=True, exist_ok=True, mode=0o700)
    before = inspect(root)
    results = {}
    try:
        with Selected(root, label) as f:
            context = f.cli("context", label)
            assert context["project"] == label and context["preferences"] == []
            assert all("content" not in d for d in context["documents"])
            assert f.cli("projects") == [{"name": label}]
            assert f.cli("context", "beta", success=False)["error"] == "Unauthorized"
            assert f.cli("handoff", label, "missing", success=False)["error"] == "NotFound"
            results["operator_queries"] = "PASS: health/projects/context; confirmed preferences and handoffs Empty"
            for method, params, expected in [
                ("context.get", {"project": "beta"}, "Unauthorized"),
                ("context.get", {"project": label, "extra": True}, "InvalidRequest"),
                ("handoff.get", {"project": label, "task_id": ["missing"]}, "InvalidRequest"),
            ]:
                with Raw(f) as peer:
                    assert peer.request(method, params)["error"] == expected
            bridge = Bridge(f, label)
            def rpc(method, params, ident=1):
                return bridge.request(dict(jsonrpc="2.0", id=ident, method=method, params=params))
            rpc("initialize", dict(protocolVersion="2025-11-25", clientInfo=dict(name="local-e2", version="1"), capabilities={}))
            bridge.request(dict(jsonrpc="2.0", method="notifications/initialized"))
            tools = rpc("tools/list", {}, 2)["result"]["tools"]
            assert {t["name"] for t in tools} == {"get_effective_context", "get_handoff"}
            value = rpc("tools/call", dict(name="get_effective_context", arguments={}), 3)["result"]
            assert json.loads(value["content"][0]["text"]) == context
            for name, arguments in [("get_effective_context", dict(project="beta")), ("get_handoff", dict(task_id="beta-only"))]:
                assert rpc("tools/call", dict(name=name, arguments=arguments), 4)["result"]["isError"]
            results["mcp_bridge"] = "PASS: exact operator parity, two schemas, project override and foreign task denied"
            snapshot = snapshot_binary
            command = [str(snapshot), *f.location, "--project", label, "--task-id", "missing"]
            def ui(success=True, command=command):
                r = subprocess.run(command, capture_output=True, timeout=20)
                assert (r.returncode == 0) == success, "UI client unexpected outcome"
                if success:
                    data = json.loads(r.stdout)
                    assert len(data["projects"]) == 1
                    assert data["projects"][0]["context"]["project"] == label
                    assert data["projects"][0]["context"]["preferences"] == []
                    assert data["projects"][0]["handoff"] is None
                    assert all(data["projects"][0]["context"][field] == context[field]
                               for field in ["project", "preferences", "repository", "documents", "warnings"])
            ui()
            ui(False, [str(snapshot), *f.location, "--project", "beta"])
            bad_location = [str(snapshot), "--runtime-dir", str(f.runtime), "--endpoint", "invalid" if os.name == "nt" else str(f.runtime / "missing.sock")]
            ui(False, bad_location)
            cap = f.runtime / "operator.cap"
            raw = cap.read_bytes()
            cap.unlink()
            ui(False)
            cap.write_bytes(raw)
            if os.name != "nt":
                cap.chmod(0o600)
            ui()
            assert f.db.read_bytes() == f.before
            results["native_ui_client"] = "PASS: actual scoped empty snapshot; invalid endpoint, removed capability, recovery"
            results["codex"] = smoke(f, label, context)
            # Actual desktop/TUI views use the same endpoint, never a synthetic fallback.
            if os.environ.get("ACP_WINDOWS_TEST_ROOT"):
                from windows_wsl_driver import views
                results["views"] = views(f, label, snapshot)
            else:
                from local_console_probe import live_views
                results["views"] = live_views(f, label, out)
            stop(f.proc)
            ui(False)
            f.start()
            ui()
            assert f.db.read_bytes() == f.before
            results["daemon_lifecycle"] = "PASS: stop/refusal/restart/reconnect; disposable DB unchanged"
    finally:
        after = inspect(root)
        unchanged = after == before
        results["external_checkout"] = dict(status="PASS" if unchanged else "FAIL", pre_digest=digest(before), post_digest=digest(after),
            metadata="root/checkout-marker, Git HEAD/index/config/packed-refs and five allowlisted document lstat records; no atime/content scan",
            git="HEAD/branch + bounded status compared locally; no values published" if before["status"] is not None else "HEAD/branch compared locally; status NOT_RUN/Unknown because external filters/submodules are refused")
        (out / "local-project-result.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
        assert unchanged, "external checkout changed during read-only acceptance; preserve changes and investigate"
    print("PASS: operator/IPC/MCP/UI/Codex scoped local acceptance; protected checkout pre/post identical")
    if results["codex"]["status"] == "NOT_RUN":
        print("NOT_RUN: installed Codex integration unavailable")
    views = results["views"]
    if views.get("status") == "NOT_RUN" or any(isinstance(v, dict) and v.get("status") == "NOT_RUN" for v in views.values()):
        print("NOT_RUN: one or more selected-project desktop/TUI views unavailable")


if __name__ == "__main__":
    main()
