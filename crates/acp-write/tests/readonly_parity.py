"""Private Rust-test child: read-only comparison on this run's owned fixtures.

Bounded stdin is supplied by the unit harness, never an operator profile selector.
No Python Store, profile defaults, output records or exception details.
"""
from contextlib import closing
import json
from pathlib import Path
import sqlite3
import sys
import tempfile


def main():
    payload = json.loads(sys.stdin.buffer.read(8193))
    root = Path(payload["root"])
    canonical = root.resolve(strict=True)
    if (canonical != root or not canonical.parent.samefile(Path(tempfile.gettempdir()))
            or not canonical.name.startswith("acp-m1-synthetic-")):
        raise ValueError("SyntheticGuard")

    def read(role):
        path = canonical / role / "profile.sqlite3"
        if path.is_symlink() or path.resolve(strict=True) != path:
            raise ValueError("SyntheticGuard")
        with closing(sqlite3.connect(path.as_uri() + "?mode=ro", uri=True)) as db:
            db.execute("PRAGMA query_only=ON")
            db.execute("BEGIN")
            version = db.execute("PRAGMA user_version").fetchone()[0]
            names = {r[1] for r in db.execute("PRAGMA table_info(handoffs)")}
            fingerprint = "worktree_fingerprint" if "worktree_fingerprint" in names else "NULL"
            if db.execute("PRAGMA integrity_check").fetchall() != [("ok",)]:
                raise ValueError("Integrity")
            if db.execute("PRAGMA foreign_key_check").fetchall():
                raise ValueError("ForeignKey")
            data = [
                db.execute("SELECT name,root,created_at FROM projects ORDER BY name").fetchall(),
                db.execute("SELECT id,scope,project,pref_key,value,status,source,created_at,updated_at FROM preferences ORDER BY id").fetchall(),
                db.execute(f"SELECT project,task_id,objective,base_sha,branch,worktree_state,{fingerprint},completed,next_action,validation_json,updated_at FROM handoffs ORDER BY project,task_id").fetchall(),
                db.execute("SELECT name,seq FROM sqlite_sequence ORDER BY name").fetchall(),
            ]
            if role == "candidate":
                identity = db.execute("SELECT profile_id,generation,owner_epoch,commit_sequence FROM acp_profile").fetchall()
                if identity != [(payload["binding"]["profile_id"], payload["binding"]["generation"], 1, 0)]:
                    raise ValueError("Identity")
                if version != 2 or db.execute("SELECT count(*) FROM acp_receipts").fetchone()[0] != 0:
                    raise ValueError("Metadata")
                if db.execute("SELECT count(*) FROM acp_revisions WHERE revision<>1 OR deleted<>0").fetchone()[0] != 0:
                    raise ValueError("Revision")
            elif version not in (0, 1):
                raise ValueError("SourceVersion")
            return data

    source = read("source")
    if read("backup") != source or read("candidate") != source:
        raise ValueError("LegacyMismatch")
    # Exact strings include raw opaque JSON whitespace/non-ASCII and historical IDs.
    print("PASS")


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("SyntheticReadonlyVerificationFailed", file=sys.stderr)
        raise SystemExit(1)
