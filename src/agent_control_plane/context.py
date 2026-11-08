"""Live project metadata and carefully scoped effective-context resolution."""

from __future__ import annotations

import hashlib
import os
import stat
import subprocess
from pathlib import Path
from typing import Any

from .store import Store

# Canonical project docs are data, never executable instructions from the service.
DOCUMENTS = ("AGENTS.md", "README.md", "docs/index.md", "docs/README.md", "docs/adr/README.md")
MAX_DOC_BYTES = 16384


def run_git(root: Path, *args: str) -> str | None:
    try:
        cmd = subprocess.run(
            ["git", "-C", str(root), *args], capture_output=True, text=True,
            timeout=4, check=False, encoding="utf-8", errors="replace",
            env=None,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    return cmd.stdout.strip() if cmd.returncode == 0 else None



# A bounded content fingerprint for resuming dirty worktrees.
# Persist only the digest, never tracked diff, untracked file data or paths.
MAX_FINGERPRINT_BYTES = 16 * 1024 * 1024
MAX_UNTRACKED_FILES = 500


def worktree_fingerprint(root: Path) -> str | None:
    """Detect changes to tracked diffs and bounded untracked files.

    Ignored files, submodule internals and external environment remain
    outside this best-effort snapshot. Unavailable means NOT verified.
    """
    try:
        status = subprocess.run(
            ["git", "-C", str(root), "status", "--porcelain=v1", "-z",
             "--untracked-files=all"],
            capture_output=True, check=False, timeout=6,
        )
        diff = subprocess.run(
            ["git", "-C", str(root), "diff", "--binary", "--no-ext-diff",
             "--no-textconv", "HEAD", "--"],
            capture_output=True, check=False, timeout=6,
        )
        if status.returncode != 0 or diff.returncode != 0:
            return None
        total = len(status.stdout) + len(diff.stdout)
        if total > MAX_FINGERPRINT_BYTES:
            return None
        digest = hashlib.sha256()
        digest.update(b"acp-worktree-v1\0")
        digest.update(status.stdout)
        digest.update(b"tracked-diff\0")
        digest.update(diff.stdout)
        untracked = [item[3:] for item in status.stdout.split(b"\0")
                     if item.startswith(b"?? ")]
        if len(untracked) > MAX_UNTRACKED_FILES:
            return None
        project_root = root.resolve(strict=True)
        for raw in sorted(untracked):
            file = root / os.fsdecode(raw)
            # Do not follow symlinked parent directories outside this project.
            file.parent.resolve(strict=True).relative_to(project_root)
            info = file.lstat()
            digest.update(b"untracked\0")
            digest.update(raw)
            digest.update(str(info.st_mode).encode("ascii"))
            if stat.S_ISLNK(info.st_mode):
                digest.update(os.fsencode(os.readlink(file)))
            elif stat.S_ISREG(info.st_mode):
                if info.st_size > MAX_FINGERPRINT_BYTES - total:
                    return None
                flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
                fd = os.open(file, flags)
                with os.fdopen(fd, "rb") as stream:
                    while chunk := stream.read(65536):
                        total += len(chunk)
                        if total > MAX_FINGERPRINT_BYTES:
                            return None
                        digest.update(chunk)
            else:
                return None
        return digest.hexdigest()
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return None


def worktree_snapshot(root: Path, *, include_fingerprint: bool = False) -> dict[str, Any]:
    sha = run_git(root, "rev-parse", "HEAD")
    branch = run_git(root, "branch", "--show-current")
    state = run_git(root, "status", "--porcelain=v1", "--untracked-files=normal")
    # Presence of sensitive filenames is possible; return counts, not file names.
    result = {"head": sha, "branch": branch, "dirty": None if state is None else bool(state),
              "change_count": None if state is None else len(state.splitlines()) if state else 0}
    if include_fingerprint:
        result["worktree_fingerprint"] = worktree_fingerprint(root) if sha else None
    return result


def document_metadata(root: Path, *, include_content: bool) -> list[dict]:
    result = []
    for relative in DOCUMENTS:
        path = root / relative
        if not path.is_file():
            continue
        # Do not follow symlinks outside the registered project root.
        try:
            resolved = path.resolve(strict=True)
            resolved.relative_to(root)
            size = resolved.stat().st_size
            if size > MAX_DOC_BYTES:
                result.append({"path": relative, "status": "too_large", "bytes": size})
                continue
            raw = resolved.read_bytes()
            entry: dict[str, Any] = {"path": relative, "status": "available", "bytes": len(raw),
                                      "sha256": hashlib.sha256(raw).hexdigest()}
            if include_content:
                entry["content"] = raw.decode("utf-8", errors="replace")
            result.append(entry)
        except (OSError, ValueError):
            result.append({"path": relative, "status": "unavailable_or_outside_root"})
    return result


def effective_context(store: Store, project: str, *, include_documents: bool = False) -> dict:
    item = store.project(project)
    root = Path(item["root"])
    return {
        "project": project,
        "authority": "Current repository files and accepted ADRs outrank preferences and task handoffs; verify conflicts.",
        "preferences": [{"key": x["pref_key"], "value": x["value"], "scope": x["scope"],
                          "source": x["source"], "id": x["id"], "status": x["status"]}
                         for x in store.resolve_preferences(project)],
        "documents": document_metadata(root, include_content=include_documents),
        "repository": worktree_snapshot(root),
        "warnings": ["Only confirmed preferences are included.",
                     "Documents are live references, not a complete project audit.",
                     "This context does not authorize filesystem edits or external actions."],
    }


def inspect_handoff(store: Store, project: str, task_id: str) -> dict:
    obj = store.handoff(project, task_id)
    root = Path(store.project(project)["root"])
    current = worktree_snapshot(root, include_fingerprint=True)
    obj["current_repository"] = current
    obj["resume_warnings"] = []
    if not current["head"] or current["head"] != obj["base_sha"]:
        obj["resume_warnings"].append("HEAD changed or cannot be verified; re-evaluate all prior checks")
    if not current["branch"] or current["branch"] != obj["branch"]:
        obj["resume_warnings"].append("Branch differs from handoff; inspect worktree before proceeding")
    if current["dirty"] is None or current["dirty"]:
        obj["resume_warnings"].append("Current worktree has changes or cannot be inspected; preserve user edits")
    saved_fp = obj.get("worktree_fingerprint")
    current_fp = current.get("worktree_fingerprint")
    if not saved_fp:
        obj["resume_warnings"].append("Handoff has no worktree fingerprint; uncommitted changes cannot be compared")
    elif not current_fp:
        obj["resume_warnings"].append("Worktree fingerprint unavailable; inspect differences manually")
    elif saved_fp != current_fp:
        obj["resume_warnings"].append("Worktree fingerprint changed since handoff; inspect uncommitted edits")
    obj["resume_warnings"].append("Earlier validation is scoped to its recorded revision and environment")
    return obj
