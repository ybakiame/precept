"""Explicit opt-in real ACP context turn, gated by clean same-source security tests."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
from test_local_service import ROOT
from test_real_acp_smoke import private_home, observed
from test_session_context import ContextFixture

PROMPT = (
    "In this disposable synthetic project, call mcp__acp-context__get_effective_context "
    "with include_documents=true, then mcp__acp-context__get_handoff with task_id=shared. "
    "Do not invoke other tools, read or write files, run commands, inspect environment, "
    "log in or call other agents. After both results reply ACP_CONTEXT_OK only."
)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for flag in ["opt-in", "allow-credential-use", "allow-model-call"]:
        p.add_argument("--" + flag, action="store_true")
    p.add_argument("--authenticated-home", type=Path, required=True)
    p.add_argument("--executable", type=Path, required=True)
    p.add_argument("--version", required=True)
    p.add_argument("--security-report", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    a = p.parse_args()
    assert (
        a.opt_in and a.allow_credential_use and a.allow_model_call
    ), "Separate operator credential/network/model opt-in required"
    assert os.name != "nt", "This real opt-in selects the installed Linux agent"
    home = private_home(a.authenticated_home)
    assert a.executable.is_absolute() and a.executable.is_file()
    sha = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    assert not subprocess.check_output(
        ["git", "status", "--porcelain"], cwd=ROOT, text=True
    ).strip(), "Clean reviewed source required"
    security = json.loads(a.security_report.read_text())
    assert security["tested_git_sha"] == sha and not security["worktree_dirty"]
    passed = {g["name"] for g in security["gates"] if g["status"] == "PASS"}
    assert {
        "context-process",
        "lease-unit",
        "root-clippy",
    } <= passed, "Negative security gates must pass before real agent"
    out = a.out.resolve()
    assert out.is_relative_to(ROOT / "artifacts/local-p1-i")
    out.mkdir(parents=True, exist_ok=True, mode=0o700)
    result_file = out / "real-context-result.json"
    assert not result_file.exists(), "Preserve every original run"
    result = dict(
        status="NOT_RUN",
        tested_git_sha=sha,
        worktree_dirty=False,
        host="Linux/WSL native daemon",
        agent="Kimi Code CLI",
        installed_version=a.version,
        owner_opt_in=True,
        credential_use="operator-prepared disposable private HOME",
        application_prompt_submissions=0,
        provider_http_requests="NOT_INSTRUMENTED",
        invocation="installed absolute executable + fixed argv [acp]; cleared env; explicit private HOME",
        executable_sha256=hashlib.sha256(a.executable.read_bytes()).hexdigest(),
        daemon_sha256=hashlib.sha256(
            (ROOT / "target/debug/acp-daemon").read_bytes()
        ).hexdigest(),
        mcp_sha256=hashlib.sha256(
            (ROOT / "target/debug/acp-mcp").read_bytes()
        ).hexdigest(),
        protocol_version=1,
        session_api_version=2,
        credential_format=2,
        context_calls=0,
        handoff_calls=0,
    )
    with ContextFixture() as f:
        f.config.write_text(
            json.dumps(
                dict(
                    version=1,
                    programs=[
                        dict(
                            id="real-kimi",
                            executable=str(a.executable.resolve()),
                            args=["acp"],
                            env=dict(
                                HOME=str(home), PATH="/usr/bin:/bin", TMPDIR=str(f.root)
                            ),
                        )
                    ],
                )
            )
        )
        try:
            f.start()
            f.query(
                "session.start",
                executable="real-kimi",
                confirmed=True,
                share_context=True,
            )
            v = observed(f, ["ready", "auth_required", "failed"], 18)
            identity = v["agent"] == "Kimi Code CLI " + a.version
            result.update(
                identity_verified=identity,
                session_new=v["state"],
                capabilities=v["agent_features"],
                auth_method_types=v["authentication"]["methods"],
            )
            if v["state"] == "ready" and identity:
                before = f.query("session.context")
                assert (
                    before["active"]
                    and before["context_calls"] == 0
                    and before["handoff_calls"] == 0
                )
                f.query("session.prompt", session_id=v["session_id"], prompt=PROMPT)
                result["application_prompt_submissions"] = 1
                deadline = time.monotonic() + 125
                while time.monotonic() < deadline:
                    v = f.query("session.get")
                    if v["permissions"]:
                        result.update(
                            status="NOT_RUN",
                            reason="Vendor requires manual permission; no permission automatically approved",
                        )
                        break
                    if v["state"] in ["completed", "failed", "auth_required"]:
                        break
                    time.sleep(0.04)
                audit = f.query("session.context")
                result.update(
                    context_calls=audit["context_calls"],
                    handoff_calls=audit["handoff_calls"],
                    stop_reason=v["stop_reason"],
                    activity=v["activity"],
                )
                if "reason" not in result:
                    result["status"] = (
                        "PASS"
                        if v["state"] == "completed"
                        and v["stop_reason"] == "end_turn"
                        and audit["context_calls"] >= 1
                        and audit["handoff_calls"] >= 1
                        and v["activity"]["response_chunks"] > 0
                        else "FAIL"
                    )
                if result["status"] == "PASS":
                    # Open a new peer immediately before revoke, with no additional data reads.
                    saved = json.loads(f.lease_file().read_text())
                    with f.lease_peer(saved) as peer:
                        f.end(v)
                        result["established_peer_denied_after_stop"] = (
                            peer.request("context.get", {})["error"] == "Unauthorized"
                        )
                    result["saved_secret_denied_after_stop"] = f.rejected(saved)
                    if (
                        not result["established_peer_denied_after_stop"]
                        or not result["saved_secret_denied_after_stop"]
                    ):
                        result["status"] = "FAIL"
            else:
                result.update(
                    status="NOT_RUN",
                    reason="AUTH_REQUIRED or vendor stdio MCP session unavailable; no prompt sent",
                )
        except Exception:
            result.update(
                status="FAIL",
                reason="Sanitized local real-context validation failure; no raw vendor output retained",
            )
        finally:
            if f.proc and f.proc.poll() is None:
                latest = f.query("session.get")
                if latest and latest["process_alive"]:
                    f.end(latest)
                latest = f.query("session.get")
                result["direct_child_reaped"] = (
                    not latest or not latest["process_alive"]
                )
                result["lease_file_removed"] = not list(f.runtime.glob("session-*.cap"))
                result["database_unchanged"] = f.db.read_bytes() == f.before
                if not all(
                    result[k]
                    for k in [
                        "direct_child_reaped",
                        "lease_file_removed",
                        "database_unchanged",
                    ]
                ):
                    result["status"] = "FAIL"
    result_file.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    return int(result["status"] == "FAIL")


if __name__ == "__main__":
    raise SystemExit(main())
