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


READONLY_REQUESTS = {
    "mcp__acp-context__get_effective_context",
    "mcp__acp-context__get_handoff",
}


def approved_readonly_request(operation, *, opt_in, agent, remaining, allow_once):
    """Approve only two owner-authorized tools, never broad vendor permission grants.

    Kimi 2.5.2's ACP permission adapter omits arguments; its exact tool title and
    action content are vendor claims. Daemon/MCP enforce scope independently.
    See MoonshotAI/kimi-code packages/acp-server/src/approval.ts.
    """
    if not isinstance(operation, dict):
        return False
    title = operation.get("title")
    return bool(
        opt_in
        and agent == "kimi"
        and remaining > 0
        and allow_once
        and isinstance(title, str)
        and title in READONLY_REQUESTS
        and set(operation) == {"toolCallId", "title", "content"}
        and operation["content"]
        == [
            {
                "type": "content",
                "content": {
                    "type": "text",
                    "text": "Requesting approval to Approve " + title,
                },
            }
        ]
    )


def credential_mode_allowed(agent, anonymous, allow_credentials):
    return (agent == "opencode" and anonymous and not allow_credentials) or (
        not anonymous and allow_credentials
    )


def opencode_model_allowed(model, service, anonymous):
    if service == "deepseek":
        return not anonymous and model == "deepseek/deepseek-flash"
    return service == "free" and model in {
        "opencode/glm-5.3-flash-free",
        "opencode/deepseek-v4.1-flash-free",
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for flag in ["opt-in", "allow-credential-use", "allow-model-call"]:
        p.add_argument("--" + flag, action="store_true")
    p.add_argument("--authenticated-home", "--private-home", type=Path, required=True)
    p.add_argument(
        "--anonymous",
        action="store_true",
        help="OpenCode free-model test with no credential file or inherited environment",
    )
    p.add_argument("--executable", type=Path, required=True)
    p.add_argument("--version", required=True)
    p.add_argument("--agent", choices=["kimi", "opencode"], default="kimi")
    p.add_argument("--opencode-service", choices=["free", "deepseek"], default="free")
    p.add_argument("--allow-readonly-mcp-permissions", action="store_true")
    p.add_argument("--security-report", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument(
        "--initialize-only",
        action="store_true",
        help="create and stop the MCP session without submitting any prompt",
    )
    a = p.parse_args()
    assert a.opt_in, "Separate operator real-agent opt-in required"
    assert credential_mode_allowed(
        a.agent, a.anonymous, a.allow_credential_use
    ), "Select explicit credential use or anonymous OpenCode, never both"
    assert a.initialize_only or a.allow_model_call, "Separate model opt-in required"
    assert os.name != "nt", "This real opt-in selects the installed Linux agent"
    if a.agent == "kimi":
        home = private_home(a.authenticated_home)
    else:
        home = a.authenticated_home
        assert home.is_absolute() and home.resolve() == home and home != Path.home()
        assert not home.is_relative_to(ROOT)
        auth_file = home / ".local/share/opencode/auth.json"
        if a.anonymous:
            assert (
                not auth_file.exists() and not auth_file.is_symlink()
            ), "Anonymous HOME must not contain authentication state"
        for item in [home, home / ".acp-owned-smoke-home"] + (
            [] if a.anonymous else [auth_file]
        ):
            st = item.lstat()
            assert (
                not item.is_symlink()
                and st.st_uid == os.getuid()
                and st.st_mode & 0o077 == 0
            )
    if a.agent == "opencode":
        config = json.loads((home / ".config/opencode/opencode.json").read_text())
        if a.anonymous:
            assert set(config) <= {
                "model",
                "agents",
                "permissions",
                "update",
                "share",
                "snapshots",
                "lsp",
                "formatter",
            }, "Anonymous config may not contain provider credentials or extensions"
            assert all(set(agent) == {"model"} for agent in config["agents"].values())
        model = config["model"]
        assert opencode_model_allowed(model, a.opencode_service, a.anonymous)
        if a.opencode_service == "deepseek":
            assert (
                config["providers"]["deepseek"]["settings"]["baseURL"]
                == "https://api.deepseek.com/v1"
            ), "Official credential must only target the official endpoint"
        assert all(
            config["agents"][name]["model"] == model
            for name in ["build", "title", "summary", "compaction"]
        )
    else:
        model = "kimi-for-coding"
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
        agent="Kimi Code CLI" if a.agent == "kimi" else "OpenCode",
        readonly_permission_opt_in=a.allow_readonly_mcp_permissions,
        approved_readonly_calls=0,
        installed_version=a.version,
        configured_model=model,
        model_service=a.opencode_service if a.agent == "opencode" else "Kimi Code",
        owner_opt_in=True,
        credential_use=(
            "NONE: explicit anonymous HOME; no inherited environment"
            if a.anonymous
            else "operator-prepared disposable private HOME"
        ),
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

    def save():
        pending = result_file.with_suffix(".pending")
        pending.write_text(json.dumps(result, indent=2) + "\n")
        pending.replace(result_file)

    save()
    with ContextFixture() as f:
        f.config.write_text(
            json.dumps(
                dict(
                    version=1,
                    programs=[
                        dict(
                            id="real-" + a.agent,
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
                executable="real-" + a.agent,
                confirmed=True,
                share_context=True,
            )
            v = observed(f, ["ready", "auth_required", "failed"], 18)
            identity = (
                v["agent"]
                == ("Kimi Code CLI " if a.agent == "kimi" else "OpenCode ") + a.version
            )
            result.update(
                identity_verified=identity,
                session_new=v["state"],
                capabilities=v["agent_features"],
                auth_method_types=v["authentication"]["methods"],
            )
            save()
            if v["state"] == "ready" and identity and a.initialize_only:
                result.update(
                    status="PASS", model_turn="NOT_RUN: explicit initialize-only probe"
                )
            elif v["state"] == "ready" and identity:
                before = f.query("session.context")
                assert (
                    before["active"]
                    and before["context_calls"] == 0
                    and before["handoff_calls"] == 0
                )
                result["application_prompt_submissions"] = 1
                result["prompt_submission"] = (
                    "ABOUT_TO_SEND; never automatically retried"
                )
                save()
                f.query(
                    "session.prompt",
                    session_id=v["session_id"],
                    prompt=(
                        PROMPT
                        if a.agent == "kimi"
                        else PROMPT.replace("mcp__acp-context__", "acp_context_")
                    ),
                )
                result["prompt_submission"] = "ACKNOWLEDGED"
                save()
                deadline = time.monotonic() + 125
                while time.monotonic() < deadline:
                    v = f.query("session.get")
                    if v["permissions"]:
                        # Operator-approved read-only tool requests only. These strings
                        # describe vendor claims, not trusted project or authorization facts.
                        permitted = True
                        for pending in v["permissions"]:
                            operation = json.loads(pending["operation"])
                            title = operation.get("title")
                            raw = operation.get("rawInput")
                            approved = approved_readonly_request(
                                operation,
                                opt_in=a.allow_readonly_mcp_permissions,
                                agent=a.agent,
                                remaining=2 - result["approved_readonly_calls"],
                                allow_once=pending["allow_once"],
                            )
                            if not approved:
                                result["permission_shape"] = {
                                    "operation_keys": sorted(operation),
                                    "known_title": isinstance(title, str)
                                    and title in READONLY_REQUESTS,
                                    "arguments_exposed": raw is not None,
                                }
                                permitted = False
                                break
                            f.query(
                                "session.permission",
                                session_id=v["session_id"],
                                request_id=pending["request_id"],
                                option_id=pending["allow_once"],
                            )
                            result["approved_readonly_calls"] += 1
                            save()
                        if permitted:
                            continue
                        result.update(
                            status="NOT_RUN",
                            reason="Vendor permission outside separately authorized read-only tool requests; turn stopped",
                        )
                        break
                    if v["state"] in ["completed", "failed", "auth_required"]:
                        break
                    time.sleep(0.04)
                result["final_session_state"] = v["state"]
                result["supervisor_failure"] = v["failure"]
                result["protocol_rejections"] = [
                    event for event in v["events"] if event.startswith("ACP rejected: ")
                ]
                save()
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
            try:
                latest = (
                    f.query("session.get") if f.proc and f.proc.poll() is None else None
                )
                if latest and latest["process_alive"]:
                    # Failed/auth-required children are already being reaped; do not enqueue
                    # Stop onto their closed control receiver or assume a Stopped transition.
                    if latest["state"] not in ["failed", "auth_required"]:
                        f.query(
                            "session.stop",
                            session_id=latest["session_id"],
                            success=False,
                        )
                    deadline = time.monotonic() + 3
                    while latest["process_alive"] and time.monotonic() < deadline:
                        time.sleep(0.03)
                        latest = f.query("session.get")
                result["direct_child_reaped"] = bool(
                    latest and not latest["process_alive"]
                )
            except Exception:
                result["direct_child_reaped"] = False
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
            save()
    result_file.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    return int(result["status"] == "FAIL")


if __name__ == "__main__":
    raise SystemExit(main())
