"""Explicit opt-in native ACP smoke with content-free evidence and private operator auth."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import time
from test_agent_session import SessionFixture
from test_local_service import ROOT

PROMPT = (
    "Reply with ACP_SMOKE_OK only. Do not read or write files, run tools, inspect "
    "environment, use MCP, authenticate, or call other agents."
)


def private_home(path):
    """Validate ownership/shape only. Never read the operator's credential/config content."""
    assert path.is_absolute() and path.resolve() == path and not path.is_symlink()
    assert (
        path != Path.home().resolve()
    ), "normal user HOME is not a disposable smoke home"
    assert (
        path / ".acp-owned-smoke-home"
    ).is_file(), "explicit disposable smoke home required"
    assert not path.is_relative_to(
        ROOT
    ), "credential state must stay outside repository"
    for item in [path, path / ".kimi-code", path / ".kimi-code/config.toml"]:
        metadata = item.lstat()
        assert not stat.S_ISLNK(metadata.st_mode), "auth state symlinks refused"
        assert metadata.st_uid == os.getuid() and metadata.st_mode & 0o077 == 0
    return path


def observed(f, terminal, timeout):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        v = f.query("session.get")
        if v["state"] in terminal:
            return v
        time.sleep(0.03)
    raise AssertionError(
        "real ACP observation deadline; raw output deliberately excluded"
    )


def token_received(events):
    # A genuine streamed token can cross several ACP chunks; do not persist text.
    prefix = "agent_message_chunk: "
    return "ACP_SMOKE_OK" in "".join(
        event[len(prefix) :] for event in events if event.startswith(prefix)
    )


def cancel_turn(f, session_id):
    try:
        f.query("session.cancel", session_id=session_id)
        return True
    except AssertionError:
        return False  # An already completed turn cannot be cancelled; never retry the prompt.


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--opt-in", action="store_true")
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--authenticated-home", type=Path)
    parser.add_argument("--allow-credential-use", action="store_true")
    parser.add_argument("--allow-model-call", action="store_true")
    parser.add_argument("--cancel-after-send", action="store_true")
    args = parser.parse_args()
    assert args.opt_in, "installed real agent requires operator opt-in"
    assert (
        os.name != "nt"
    ), "this explicit smoke selects the installed Linux Kimi executable"
    assert args.executable.is_absolute() and args.executable.is_file()
    assert not args.cancel_after_send or args.allow_model_call
    home = None
    if args.authenticated_home:
        assert args.allow_credential_use, "private auth needs separate owner opt-in"
        home = private_home(args.authenticated_home)
    else:
        assert not args.allow_credential_use, "explicit private home required"
    out = args.out.resolve()
    assert any(
        out.is_relative_to(ROOT / folder)
        for folder in ["artifacts/local-p1-g", "artifacts/local-p1-h"]
    )
    out.mkdir(parents=True, exist_ok=True, mode=0o700)
    out.chmod(0o700)
    assert not (
        out / "real-acp-result.json"
    ).exists(), "retain each original run separately"
    result = dict(
        agent="Kimi Code CLI",
        installed_version=args.version,
        tested_git_sha=subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip(),
        worktree_dirty=bool(
            subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT)
        ),
        host="Linux/WSL native daemon",
        invocation="absolute installed executable + argv [acp]; cleared environment; explicit disposable HOME",
        executable_sha256=hashlib.sha256(args.executable.read_bytes()).hexdigest(),
        daemon_sha256=hashlib.sha256(
            (ROOT / "target/debug/acp-daemon").read_bytes()
        ).hexdigest(),
        owner_opt_in=True,
        credential_use="explicit private HOME" if home else "NOT_RUN: empty HOME",
        model_call_opt_in=args.allow_model_call,
        client_capabilities={},
        context_mcp="NOT_RUN: mcpServers empty",
        initialize="NOT_RUN",
        session_new="NOT_RUN",
        prompt="NOT_RUN",
        remote_model_turn="NOT_RUN",
        cancel="NOT_RUN",
        status="NOT_RUN",
    )
    with SessionFixture() as f:
        if home is None:
            home = f.root / "isolated-home"
            home.mkdir(mode=0o700)
        f.config.write_text(
            json.dumps(
                dict(
                    version=1,
                    programs=[
                        dict(
                            id="real-kimi",
                            executable=str(args.executable.resolve()),
                            args=["acp"],
                            env={
                                "HOME": str(home),
                                "PATH": "/usr/bin:/bin",
                                "TMPDIR": str(f.root),
                            },
                        )
                    ],
                )
            )
        )
        f.start()
        f.query("session.start", executable="real-kimi", confirmed=True)
        v = observed(f, ["ready", "auth_required", "failed"], 16)
        identity = v["agent"] == "Kimi Code CLI " + args.version
        result["identity_verified"] = identity
        result["initialize"] = "PASS" if identity else "FAIL"
        result["protocol_version"] = 1 if v["agent"] else None
        result["session_api_version"] = v["session_api_version"]
        result["auth_method_types"] = v["authentication"]["methods"]
        result["capabilities"] = dict(
            v["agent_features"], load_session=v["load_session_advertised"]
        )
        result["session_new"] = (
            "AUTH_REQUIRED" if v["state"] == "auth_required" else v["state"]
        )
        token_seen = False
        if v["state"] == "ready" and args.allow_model_call and identity:
            result["remote_model_turn"] = "ATTEMPTED"
            f.query("session.prompt", session_id=v["session_id"], prompt=PROMPT)
            deadline = time.monotonic() + 125
            cancellation_sent = False
            if args.cancel_after_send:
                cancellation_sent = cancel_turn(f, v["session_id"])
            while time.monotonic() < deadline:
                v = f.query("session.get")
                token_seen |= token_received(v["events"])
                if v["permissions"] or v["activity"]["tool_events"]:
                    result["status"] = "FAIL"
                    result["reason"] = (
                        "unexpected tool activity; no permission granted; turn cancelled"
                    )
                    cancel_turn(f, v["session_id"])
                    break
                if v["state"] in ["completed", "failed", "auth_required"]:
                    break
                time.sleep(0.03)
            result["prompt"] = v["state"]
            result["stop_reason"] = v["stop_reason"]
            result["activity"] = v["activity"]
            result["synthetic_token_seen"] = token_seen
            if args.cancel_after_send:
                result["cancel"] = (
                    "PASS"
                    if cancellation_sent and v["stop_reason"] == "cancelled"
                    else "NOT_RUN: agent completed before cancellation or did not confirm cancel"
                )
                if result["status"] != "FAIL":
                    result["status"] = (
                        "PASS" if result["cancel"] == "PASS" else "PARTIAL"
                    )
            elif result["status"] != "FAIL":
                result["status"] = (
                    "PASS"
                    if v["state"] == "completed"
                    and v["stop_reason"] == "end_turn"
                    and v["activity"]["response_chunks"] > 0
                    and token_seen
                    else "FAIL"
                )
        else:
            result["status"] = "PARTIAL" if identity else "FAIL"
        # Deliberate stop; no automatic prompts, authentication or retry.
        latest = f.query("session.get")
        if latest["process_alive"] and latest["state"] not in [
            "failed",
            "auth_required",
        ]:
            latest = f.end(latest)
        deadline = time.monotonic() + 3
        while latest["process_alive"] and time.monotonic() < deadline:
            time.sleep(0.03)
            latest = f.query("session.get")
        result["direct_child_reaped"] = not latest["process_alive"]
        result["database_unchanged"] = f.db.read_bytes() == f.before
        if not result["direct_child_reaped"] or not result["database_unchanged"]:
            result["status"] = "FAIL"
    (out / "real-acp-result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    return int(result["status"] == "FAIL")


if __name__ == "__main__":
    raise SystemExit(main())
