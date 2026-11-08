"""SQLite storage for developer preferences, project registration and task handoffs.

This store only accepts explicit CLI writes. No agent-side MCP mutation tool exists.
"""

from __future__ import annotations

import json
import os
import sqlite3
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def state_dir() -> Path:
    if override := os.environ.get("AGENT_CP_HOME"):
        return Path(override).expanduser().resolve()
    if os.name == "nt" and os.environ.get("LOCALAPPDATA"):
        return Path(os.environ["LOCALAPPDATA"]) / "agent-control-plane"
    return Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local" / "state")) / "agent-control-plane"


class Store:
    def __init__(self, data_dir: Path | None = None):
        self.data_dir = (data_dir or state_dir()).expanduser().resolve()
        self.data_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
        if os.name != "nt":
            os.chmod(self.data_dir, 0o700)
        self.path = self.data_dir / "context.sqlite3"
        self.db = sqlite3.connect(str(self.path), timeout=5)
        self.db.row_factory = sqlite3.Row
        self.db.execute("PRAGMA busy_timeout=5000")
        self.db.execute("PRAGMA foreign_keys=ON")
        self.db.execute("PRAGMA journal_mode=WAL")
        self._init_schema()
        if os.name != "nt":
            os.chmod(self.path, 0o600)

    def close(self) -> None:
        self.db.close()

    def __enter__(self) -> "Store":
        return self

    def __exit__(self, *_args: Any) -> None:
        self.close()

    def _init_schema(self) -> None:
        with self.db:
            self.db.executescript("""
                CREATE TABLE IF NOT EXISTS projects (
                    name TEXT PRIMARY KEY,
                    root TEXT NOT NULL UNIQUE,
                    created_at TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS preferences (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    scope TEXT NOT NULL CHECK (scope IN ('global','project')),
                    project TEXT REFERENCES projects(name) ON DELETE CASCADE,
                    pref_key TEXT NOT NULL,
                    value TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'suggested'
                      CHECK (status IN ('suggested','confirmed','rejected','deprecated')),
                    source TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    CHECK ((scope = 'global' AND project IS NULL) OR
                           (scope = 'project' AND project IS NOT NULL))
                );
                CREATE INDEX IF NOT EXISTS prefs_lookup
                  ON preferences(scope, project, status, pref_key);
                CREATE TABLE IF NOT EXISTS handoffs (
                    project TEXT NOT NULL REFERENCES projects(name) ON DELETE CASCADE,
                    task_id TEXT NOT NULL,
                    objective TEXT NOT NULL,
                    base_sha TEXT NOT NULL,
                    branch TEXT NOT NULL,
                    worktree_state TEXT NOT NULL,
                    worktree_fingerprint TEXT,
                    completed TEXT NOT NULL,
                    next_action TEXT NOT NULL,
                    validation_json TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    PRIMARY KEY (project, task_id)
                );
            """)
            # Existing P0 installations already have the handoffs table.
            # Add the nullable field in place without deleting task history.
            cols = {row[1] for row in self.db.execute("PRAGMA table_info(handoffs)")}
            if "worktree_fingerprint" not in cols:
                self.db.execute("ALTER TABLE handoffs ADD COLUMN worktree_fingerprint TEXT")

    def register_project(self, name: str, root: str) -> dict:
        if not name or any(x in name for x in ("/", "\\", "\n", "\x00")):
            raise ValueError("Project name must be a nonempty simple name")
        resolved = Path(root).expanduser().resolve(strict=True)
        if not resolved.is_dir():
            raise ValueError("Project path must be a directory")
        if not ((resolved / ".git").exists() or (resolved / "AGENTS.md").is_file()):
            raise ValueError("Project requires a .git checkout or AGENTS.md")
        now = utc_now()
        with self.db:
            self.db.execute("INSERT INTO projects(name,root,created_at) VALUES(?,?,?)", (name, str(resolved), now))
        return {"name": name, "root": str(resolved)}

    def project(self, name: str) -> dict:
        r = self.db.execute("SELECT name,root,created_at FROM projects WHERE name=?", (name,)).fetchone()
        if not r:
            raise ValueError("Unknown project: " + name)
        return dict(r)

    def projects(self) -> list[dict]:
        return [dict(x) for x in self.db.execute("SELECT name,root,created_at FROM projects ORDER BY name")]

    def suggest_preference(self, key: str, value: str, source: str, project: str | None = None) -> int:
        if not all((key.strip(), value.strip(), source.strip())):
            raise ValueError("key, value and source cannot be empty")
        if len(key) > 120 or len(value) > 4096 or len(source) > 512:
            raise ValueError("Preference exceeds allowed length")
        if project:
            self.project(project)
        now = utc_now()
        with self.db:
            cur = self.db.execute(
                "INSERT INTO preferences(scope,project,pref_key,value,status,source,created_at,updated_at) "
                "VALUES(?,?,?,?,?,?,?,?)",
                ("project" if project else "global", project, key, value, "suggested", source, now, now),
            )
        return int(cur.lastrowid)

    def change_preference(self, pref_id: int, status: str) -> None:
        if status not in ("confirmed", "rejected", "deprecated"):
            raise ValueError("Unsupported preference status")
        with self.db:
            cur = self.db.execute(
                "UPDATE preferences SET status=?,updated_at=? WHERE id=? "
                "AND (status='suggested' OR (status='confirmed' AND ?='deprecated'))",
                (status, utc_now(), pref_id, status),
            )
            if cur.rowcount != 1:
                raise ValueError("Preference not found or invalid status transition")
            # A confirmed preference with the same scope/key supersedes previous confirmed versions.
            if status == "confirmed":
                row = self.db.execute("SELECT scope,project,pref_key FROM preferences WHERE id=?", (pref_id,)).fetchone()
                self.db.execute(
                    "UPDATE preferences SET status='deprecated',updated_at=? "
                    "WHERE id<>? AND status='confirmed' AND scope=? AND project IS ? AND pref_key=?",
                    (utc_now(), pref_id, row["scope"], row["project"], row["pref_key"]),
                )

    def delete_preference(self, pref_id: int) -> None:
        with self.db:
            cur = self.db.execute("DELETE FROM preferences WHERE id=?", (pref_id,))
            if cur.rowcount != 1:
                raise ValueError("Preference not found")

    def preferences(self, project: str | None = None, *, all_statuses: bool = False) -> list[dict]:
        if project:
            self.project(project)
        sql = "SELECT * FROM preferences WHERE (scope='global' OR (scope='project' AND project=?))"
        if not all_statuses:
            sql += " AND status='confirmed'"
        sql += " ORDER BY pref_key,CASE scope WHEN 'project' THEN 0 ELSE 1 END,id DESC"
        return [dict(x) for x in self.db.execute(sql, (project,))]

    def resolve_preferences(self, project: str) -> list[dict]:
        """Project-scoped confirmed values mask confirmed global values by key."""
        effective: dict[str, dict] = {}
        for item in self.preferences(project):
            effective.setdefault(item["pref_key"], item)
        return list(effective.values())

    def save_handoff(self, project: str, task_id: str, objective: str, next_action: str,
                     base_sha: str, branch: str, worktree_state: str, completed: str,
                     validation: list[dict], worktree_fingerprint: str | None = None) -> None:
        self.project(project)
        if not all((task_id.strip(), objective.strip(), next_action.strip())):
            raise ValueError("task-id, objective and next-action are required")
        if len(task_id) > 120 or len(objective) > 4000 or len(completed) > 8000 or len(next_action) > 4000:
            raise ValueError("Handoff exceeds allowed length")
        if not isinstance(validation, list) or any(
            not isinstance(x, dict) or x.get("status") not in ("PASS", "FAIL", "SKIPPED", "NOT_RUN")
            or not isinstance(x.get("check"), str) for x in validation
        ):
            raise ValueError("Validation must be an array of {check:string,status:PASS|FAIL|SKIPPED|NOT_RUN}")
        encoded = json.dumps(validation, ensure_ascii=False)
        if len(encoded) > 16000:
            raise ValueError("Validation detail exceeds limit")
        with self.db:
            self.db.execute(
                "INSERT INTO handoffs(project,task_id,objective,base_sha,branch,worktree_state,worktree_fingerprint,completed,next_action,validation_json,updated_at) "
                "VALUES(?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(project,task_id) DO UPDATE SET "
                "objective=excluded.objective,base_sha=excluded.base_sha,branch=excluded.branch,"
                "worktree_state=excluded.worktree_state,worktree_fingerprint=excluded.worktree_fingerprint,"
                "completed=excluded.completed,next_action=excluded.next_action,"
                "validation_json=excluded.validation_json,updated_at=excluded.updated_at",
                (project, task_id, objective, base_sha, branch, worktree_state, worktree_fingerprint,
                 completed, next_action, encoded, utc_now()),
            )

    def handoff(self, project: str, task_id: str) -> dict:
        self.project(project)
        row = self.db.execute(
            "SELECT * FROM handoffs WHERE project=? AND task_id=?", (project, task_id)
        ).fetchone()
        if not row:
            raise ValueError("Handoff not found")
        result = dict(row)
        result["validation"] = json.loads(result.pop("validation_json"))
        return result
