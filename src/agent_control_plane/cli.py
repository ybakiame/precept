"""Local operator CLI. MCP interface intentionally has no write operations."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .context import effective_context, inspect_handoff, worktree_snapshot
from .mcp import serve_stdio
from .store import Store


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="acpctl", description="Local-first Agent Control Plane P0")
    p.add_argument("--data-dir", type=Path, help="State directory (default AGENT_CP_HOME / XDG state)")
    commands = p.add_subparsers(dest="entity", required=True)

    project = commands.add_parser("project", help="Register and list local project checkouts")
    project_sub = project.add_subparsers(dest="action", required=True)
    add = project_sub.add_parser("add")
    add.add_argument("name")
    add.add_argument("path")
    project_sub.add_parser("list")

    pref = commands.add_parser("preference", help="Reviewable developer preferences")
    pref_sub = pref.add_subparsers(dest="action", required=True)
    suggestion = pref_sub.add_parser("suggest")
    suggestion.add_argument("--key", required=True)
    suggestion.add_argument("--value", required=True)
    suggestion.add_argument("--source", required=True, help="Reason or evidence; do not include secrets")
    suggestion.add_argument("--project", help="Omit for a global preference")
    for action in ("confirm", "reject", "deprecate", "delete"):
        sub = pref_sub.add_parser(action)
        sub.add_argument("id", type=int)
    listing = pref_sub.add_parser("list")
    listing.add_argument("--project")
    listing.add_argument("--all", action="store_true", help="Include suggestions, rejected, deprecated")

    context = commands.add_parser("context", help="Render effective context")
    context.add_argument("project")
    context.add_argument("--include-documents", action="store_true", help="Explicitly include allowlisted contract file contents")

    handoff = commands.add_parser("handoff", help="Snapshot and inspect unfinished tasks")
    handoff_sub = handoff.add_subparsers(dest="action", required=True)
    put = handoff_sub.add_parser("put")
    put.add_argument("--project", required=True)
    put.add_argument("--task-id", required=True)
    put.add_argument("--objective", required=True)
    put.add_argument("--next-action", required=True)
    put.add_argument("--completed", default="")
    put.add_argument("--validation-json", default="[]", help="Array of {check,status}; status=PASS|FAIL|SKIPPED|NOT_RUN")
    get = handoff_sub.add_parser("get")
    get.add_argument("--project", required=True)
    get.add_argument("--task-id", required=True)

    mcp = commands.add_parser("mcp", help="Run project-scoped, read-only MCP stdio server")
    mcp.add_argument("--project", required=True, help="Only this project's data is accessible to the MCP process")
    return p


def run(args: argparse.Namespace) -> object:
    with Store(args.data_dir) as store:
        if args.entity == "project":
            if args.action == "add":
                return store.register_project(args.name, args.path)
            return store.projects()
        if args.entity == "preference":
            if args.action == "suggest":
                return {"id": store.suggest_preference(args.key, args.value, args.source, args.project),
                        "status": "suggested", "next": "Use preference confirm ID after user review"}
            if args.action == "list":
                return store.preferences(args.project, all_statuses=args.all)
            if args.action == "delete":
                store.delete_preference(args.id)
            else:
                status = {"confirm": "confirmed", "reject": "rejected", "deprecate": "deprecated"}[args.action]
                store.change_preference(args.id, status)
            return {"id": args.id, "action": args.action}
        if args.entity == "context":
            return effective_context(store, args.project, include_documents=args.include_documents)
        if args.entity == "handoff":
            if args.action == "get":
                return inspect_handoff(store, args.project, args.task_id)
            project = store.project(args.project)
            snap = worktree_snapshot(Path(project["root"]))
            if not snap["head"]:
                raise ValueError("Cannot determine git HEAD; handoff requires a git repository")
            validation = json.loads(args.validation_json)
            store.save_handoff(args.project, args.task_id, args.objective, args.next_action,
                               snap["head"], snap["branch"] or "", "dirty" if snap["dirty"] else "clean",
                               args.completed, validation)
            return {"project": args.project, "task_id": args.task_id, "saved_at_head": snap["head"],
                    "warning": "Capture does not prove validation; prior results require independent evidence"}
        if args.entity == "mcp":
            serve_stdio(store, args.project)
            return None
        raise ValueError("Unknown command")


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        result = run(args)
    except (ValueError, OSError, json.JSONDecodeError) as exc:
        print("error: " + str(exc), file=sys.stderr)
        return 2
    if result is not None:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
