"""P1-K opt-in synthetic interoperability evaluation; never opens normal client state."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

from agent_control_plane.context import worktree_snapshot
from interop_trace import WARNINGS, digest
from test_local_service import ROOT, binary, collect, stop
from test_real_acp_smoke import observed, private_home
from test_real_context_smoke import approved_readonly_request
from test_session_context import ContextFixture

PROMPT = (
    "Resume the synthetic retry-policy investigation using only "
    "mcp__acp-context__get_effective_context with include_documents=true and "
    "mcp__acp-context__get_handoff with task_id=shared. Read each once. "
    "Do not run commands, access files, authenticate, use skills or other tools, "
    "or change anything. Inspect the returned current_repository and resume_warnings. "
    "If the handoff is stale and its recorded validation must remain historical, "
    "reply HANDOFF_STALE_CHECKS_HISTORICAL only; otherwise reply HANDOFF_UNKNOWN only."
)


class Scenario(ContextFixture):
    def __init__(self):
        super().__init__()
        (self.root / ".acp-interop-fixture").touch(mode=0o600)
        self.project_root = self.root / "fixture/alpha"
        self.readme = self.project_root / "README.md"
        self.readme.write_text(
            "Synthetic retry scheduler: inspect cancellation before resuming work.\n"
        )
        self.git("add", "AGENTS.md", "README.md")
        self.git("commit", "-qm", "synthetic baseline")
        self.base = self.git("rev-parse", "HEAD")
        self.branch = self.git("branch", "--show-current")
        self.readme.write_text(
            "Synthetic retry scheduler: pending cancellation review A.\n"
        )
        self.validation = [
            dict(
                check="historical-" + state.lower(),
                status=state,
                revision=self.base,
                environment="synthetic-source-client",
                origin="agent-reported",
                evidence="scripted scenario record, not a current verifier result",
            )
            for state in ["PASS", "FAIL", "SKIPPED", "NOT_RUN"]
        ]
        subprocess.run(
            [
                sys.executable,
                "-m",
                "agent_control_plane.cli",
                "--data-dir",
                str(self.db.parent),
                "handoff",
                "put",
                "--project",
                "alpha",
                "--task-id",
                "shared",
                "--objective",
                "Investigate bounded retries after cancellation",
                "--completed",
                "Source client recorded baseline investigation; checks remain historical",
                "--next-action",
                "Inspect changed cancellation behavior and rerun affected checks; no approval to edit",
                "--validation-json",
                json.dumps(self.validation),
            ],
            cwd=ROOT,
            check=True,
            capture_output=True,
        )
        self.before = self.db.read_bytes()

    def git(self, *args):
        env = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
        return subprocess.check_output(
            [
                "git",
                "-c",
                "user.name=ACP Synthetic",
                "-c",
                "user.email=fixture@invalid",
                "-c",
                "core.hooksPath=" + os.devnull,
                "-C",
                str(self.project_root),
                *args,
            ],
            env=env,
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()


class Codex:
    def __init__(self, fixture, executable):
        self.fixture = fixture
        home = fixture.root / "isolated-codex"
        home.mkdir(mode=0o700)
        self.trace = fixture.root / "codex-mcp-trace.jsonl"
        args = [
            str(ROOT / "tools/interop_trace.py"),
            "--synthetic-root",
            str(fixture.root),
            "--out",
            str(self.trace),
            "--",
            binary("acp-mcp"),
            *fixture.location,
            "--project",
            "alpha",
            "--capability-file",
            str(fixture.runtime / "agent-alpha.cap"),
        ]
        (home / "config.toml").write_text(
            "\n".join(
                [
                    'model = "local-validation-no-inference"',
                    'model_provider = "local_validation"',
                    "[model_providers.local_validation]",
                    'name = "No inference"',
                    'base_url = "http://127.0.0.1:9/v1"',
                    'wire_api = "responses"',
                    "[analytics]",
                    "enabled = false",
                    "[feedback]",
                    "enabled = false",
                    "[otel]",
                    'exporter = "none"',
                    'trace_exporter = "none"',
                    "[mcp_servers.acp_validation]",
                    "command = " + json.dumps(sys.executable),
                    "args = " + json.dumps(args),
                    "startup_timeout_sec = 15",
                ]
            )
            + "\n"
        )
        env = dict(
            HOME=str(home),
            CODEX_HOME=str(home),
            PATH=os.environ.get("PATH", "/usr/bin:/bin"),
        )
        self.proc = subprocess.Popen(
            [str(executable), "app-server", "--stdio"],
            cwd=fixture.root,
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
        )
        fixture.children.append(self.proc)
        self.lines = collect(self.proc.stdout)
        self.serial = 0
        self.rpc(
            "initialize",
            dict(
                clientInfo=dict(name="acp_interop", version="1"),
                capabilities=dict(experimentalApi=True),
            ),
        )
        self.proc.stdin.write(b'{"method":"initialized","params":{}}\n')
        self.proc.stdin.flush()
        self.thread = self.rpc(
            "thread/start",
            dict(
                cwd=str(fixture.root),
                ephemeral=True,
                sandbox="read-only",
                approvalPolicy="never",
            ),
        )["thread"]["id"]

    def call(self, name, args, success=True):
        reply = self.rpc(
            "mcpServer/tool/call",
            dict(
                threadId=self.thread, server="acp_validation", tool=name, arguments=args
            ),
        )
        if not success:
            assert "error" in reply or reply["result"].get("isError")
            return
        assert "error" not in reply and not reply["result"].get("isError")
        return json.loads(reply["result"]["content"][0]["text"])

    def rpc(self, method, params):
        self.serial += 1
        self.proc.stdin.write(
            json.dumps(dict(id=self.serial, method=method, params=params)).encode()
            + b"\n"
        )
        self.proc.stdin.flush()
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            line = self.lines.get(timeout=max(0.1, deadline - time.monotonic()))
            if line is None:
                raise RuntimeError("Sanitized Codex channel failure")
            value = json.loads(line)
            if value.get("id") == self.serial:
                if method in {"initialize", "thread/start", "mcpServerStatus/list"}:
                    assert "error" not in value, "Sanitized Codex setup rejection"
                    return value["result"]
                return value
        raise TimeoutError("Codex request deadline")


def check_handoff(f, client, required):
    value = client.call("get_handoff", dict(task_id="shared"))
    assert value == f.cli("handoff", "alpha", "shared")
    assert value["validation"] == f.validation
    flags = {WARNINGS.get(w, "unknown") for w in value["resume_warnings"]}
    assert set(required) <= flags
    return dict(
        result_sha256=digest(value),
        fixture_head=value["current_repository"]["head"],
        fixture_branch=value["current_repository"]["branch"],
        freshness=sorted(flags),
        historical_validation_unchanged=True,
    )


def codex_trial(f, executable):
    client = Codex(f, executable)
    inventory = client.rpc(
        "mcpServerStatus/list",
        dict(threadId=client.thread, serverName="acp_validation"),
    )
    tools = inventory["data"][0]["tools"]
    assert set(tools) == {"get_effective_context", "get_handoff"}
    context = client.call("get_effective_context", dict(include_documents=True))
    assert context == f.cli("context", "alpha", "--include-documents")
    contract = json.loads(
        (ROOT / "tests/fixtures/p1-preference-contract.v1.json").read_text()
    )
    assert all(
        marker not in json.dumps(context)
        for marker in contract["forbidden_alpha_strings"]
    )
    assert context["project"] == "alpha"
    trials = [check_handoff(f, client, ["dirty", "historical"])]
    assert "fingerprint" not in trials[0]["freshness"]
    before = worktree_snapshot(f.project_root, include_fingerprint=True)
    f.readme.write_text("Synthetic retry scheduler: pending cancellation review B.\n")
    after = worktree_snapshot(f.project_root, include_fingerprint=True)
    assert before["head"] == after["head"] and before["dirty"] == after["dirty"]
    assert before["worktree_fingerprint"] != after["worktree_fingerprint"]
    trials.append(check_handoff(f, client, ["fingerprint", "historical"]))
    f.git("switch", "-qc", "synthetic-review")
    trials.append(check_handoff(f, client, ["branch", "fingerprint", "historical"]))
    f.git("add", "README.md")
    f.git("commit", "-qm", "synthetic cancellation drift")
    trials.append(check_handoff(f, client, ["head", "branch", "historical"]))
    legacy = client.call("get_handoff", dict(task_id="null-fingerprint"))
    assert "missing_fingerprint" in {WARNINGS.get(w) for w in legacy["resume_warnings"]}
    attacks = [
        ("get_effective_context", {"project": "beta"}),
        ("get_handoff", {"task_id": "shared", "project": "beta"}),
        ("get_handoff", {"task_id": "beta-only"}),
        ("get_handoff", {"task_id": ["shared"]}),
        ("preference_confirm", {"id": 1}),
        ("session.start", {}),
        ("project.list", {}),
    ]
    for name, args in attacks:
        client.call(name, args, success=False)
    stop(client.proc)
    events = [json.loads(line) for line in client.trace.read_text().splitlines()]
    for trial in trials:
        assert any(
            e.get("result_sha256") == trial["result_sha256"]
            and e["tool"] == "get_handoff"
            and e["outcome"] == "success"
            for e in events
        )
    assert any(
        e.get("result_sha256") == digest(context)
        and e["tool"] == "get_effective_context"
        for e in events
    )
    assert any(e.get("exact_two_tools") for e in events)
    assert f.db.read_bytes() == f.before
    f.readme.write_text("Synthetic retry scheduler: pending cancellation review C.\n")
    return dict(
        status="PASS",
        mode="direct MCP via installed app-server; no model turn",
        provider_model="NOT_RUN: closed-loopback no-inference provider",
        tools=sorted(tools),
        handoff_trials=trials,
        denials=len(attacks),
        server_trace=events,
        fixture_revision=f.base,
        fixture_branch=f.branch,
        database_unchanged=True,
    )


def kimi_trial(f, home, executable, save):
    private_home(home)
    f.config.write_text(
        json.dumps(
            dict(
                version=1,
                programs=[
                    dict(
                        id="real-kimi",
                        executable=str(executable),
                        args=["acp"],
                        env=dict(
                            HOME=str(home), PATH="/usr/bin:/bin", TMPDIR=str(f.root)
                        ),
                    )
                ],
            )
        )
    )
    f.config.chmod(0o600)
    # The fixture daemon was started with a passive program configuration. Reload
    # only by restarting this owned synthetic daemon, never a user service.
    stop(f.proc)
    f.start()
    expected = f.cli("handoff", "alpha", "shared")
    assert {"head", "branch", "fingerprint", "historical"} <= {
        WARNINGS.get(w) for w in expected["resume_warnings"]
    }
    fingerprint = worktree_snapshot(f.project_root, include_fingerprint=True)
    result = dict(
        status="NOT_RUN",
        prompt_submissions=0,
        provider_model="Kimi Code/kimi-for-coding",
    )
    save(result)
    try:
        f.query(
            "session.start", executable="real-kimi", confirmed=True, share_context=True
        )
        view = observed(f, ["ready", "failed", "auth_required"], 20)
        assert view["state"] == "ready" and view["agent"] == "Kimi Code CLI 2.5.2"
        result["prompt_submissions"] = 1
        save(result)
        f.query("session.prompt", session_id=view["session_id"], prompt=PROMPT)
        deadline = time.monotonic() + 125
        approved = 0
        while time.monotonic() < deadline:
            view = f.query("session.get")
            for pending in view["permissions"]:
                operation = json.loads(pending["operation"])
                assert approved_readonly_request(
                    operation,
                    opt_in=True,
                    agent="kimi",
                    remaining=2 - approved,
                    allow_once=pending["allow_once"],
                ), "Unapproved vendor permission"
                f.query(
                    "session.permission",
                    session_id=view["session_id"],
                    request_id=pending["request_id"],
                    option_id=pending["allow_once"],
                )
                approved += 1
            if view["state"] in ["completed", "failed", "auth_required"]:
                break
            time.sleep(0.04)
        audit = f.query("session.context")
        text = "".join(
            e.removeprefix("agent_message_chunk: ")
            for e in view["events"]
            if e.startswith("agent_message_chunk: ")
        )
        result.update(
            state=view["state"],
            stop_reason=view["stop_reason"],
            audit=audit,
            model_stale_marker="HANDOFF_STALE_CHECKS_HISTORICAL" in text,
            historical_validation_unchanged=f.cli("handoff", "alpha", "shared")[
                "validation"
            ]
            == f.validation,
            worktree_unchanged=worktree_snapshot(
                f.project_root, include_fingerprint=True
            )
            == fingerprint,
            stale_flags=sorted(
                WARNINGS.get(w, "unknown") for w in expected["resume_warnings"]
            ),
            permission_approvals=approved,
        )
        save(result)
        assert view["state"] == "completed" and view["stop_reason"] == "end_turn"
        assert audit["context_calls"] == audit["handoff_calls"] == 1
        assert (
            result["model_stale_marker"]
            and result["worktree_unchanged"]
            and result["historical_validation_unchanged"]
        )
        saved = json.loads(f.lease_file().read_text())
        with f.lease_peer(saved) as peer:
            f.end(view)
            result["established_peer_denied"] = (
                peer.request("context.get", {})["error"] == "Unauthorized"
            )
        result["reconnect_denied"] = f.rejected(saved)
        assert result["established_peer_denied"] and result["reconnect_denied"]
        result["status"] = "PASS"
    except Exception:
        result["status"] = "FAIL"
        result["reason"] = (
            "Sanitized Kimi resume validation failure; no automatic retry"
        )
    finally:
        try:
            latest = f.query("session.get")
            if latest["process_alive"] and latest["state"] not in [
                "failed",
                "auth_required",
            ]:
                f.query("session.stop", session_id=latest["session_id"], success=False)
            deadline = time.monotonic() + 3
            while latest["process_alive"] and time.monotonic() < deadline:
                time.sleep(0.03)
                latest = f.query("session.get")
            result["direct_child_reaped"] = not latest["process_alive"]
        except Exception:
            result["direct_child_reaped"] = False
        result["lease_removed"] = not list(f.runtime.glob("session-*.cap"))
        result["database_unchanged"] = f.db.read_bytes() == f.before
        if not all(
            result[k]
            for k in ["direct_child_reaped", "lease_removed", "database_unchanged"]
        ):
            result["status"] = "FAIL"
        save(result)
    return result


def opencode_trial(executable):
    """No model/auth: isolated native CLI discovery and managed ACP lifecycle."""
    result = dict(
        executable_sha256=hashlib.sha256(executable.read_bytes()).hexdigest(),
        provider_model="NOT_RUN: no model prompt or credentials",
    )
    with ContextFixture() as f:
        (f.root / ".acp-interop-fixture").touch(mode=0o600)
        home = f.root / "private-opencode"
        home.mkdir(mode=0o700)
        config = home / ".config/opencode/opencode.json"
        config.parent.mkdir(parents=True, mode=0o700)
        trace = f.root / "opencode-trace.jsonl"
        command = [
            sys.executable,
            str(ROOT / "tools/interop_trace.py"),
            "--synthetic-root",
            str(f.root),
            "--out",
            str(trace),
            "--",
            binary("acp-mcp"),
            *f.location,
            "--project",
            "alpha",
            "--capability-file",
            str(f.runtime / "agent-alpha.cap"),
        ]
        minimal = dict(
            update="disable",
            share="disabled",
            snapshots=False,
            permissions=[dict(action="*", resource="*", effect="deny")],
        )
        config.write_text(
            json.dumps(
                minimal
                | dict(
                    mcp=dict(
                        servers=dict(
                            acp_validation=dict(
                                type="local", command=command, codemode=False
                            )
                        )
                    )
                )
            )
        )
        env = dict(HOME=str(home), PATH="/usr/bin:/bin", TMPDIR=str(f.root))
        result["version"] = subprocess.check_output(
            [str(executable), "--version"], env=env, text=True, timeout=10
        ).strip()
        f.start()
        documents = subprocess.run(
            [str(executable), "debug", "config", "--standalone"],
            cwd=f.root,
            env=env,
            capture_output=True,
            text=True,
            timeout=25,
        )
        declared = False
        if documents.returncode == 0:
            for doc in json.loads(documents.stdout):
                declared |= isinstance(doc, dict) and "acp_validation" in doc.get(
                    "info", {}
                ).get("mcp", {}).get("servers", {})
        listing = subprocess.run(
            [str(executable), "mcp", "list", "--standalone"],
            cwd=f.root,
            env=env,
            capture_output=True,
            text=True,
            timeout=25,
        )
        events = (
            [json.loads(line) for line in trace.read_text().splitlines()]
            if trace.exists()
            else []
        )
        result["direct"] = dict(
            config_declared=declared,
            cli_exit=listing.returncode,
            discovery=(
                "PASS"
                if any(e.get("exact_two_tools") for e in events)
                else "NOT_VERIFIED"
            ),
            no_servers_reported="No MCP servers configured" in listing.stdout,
            server_trace=events,
            actual_calls="NOT_RUN",
            codemode=False,
        )
        assert not any(e["method"] == "tools/call" for e in events)
        config.write_text(json.dumps(minimal))
        f.config.write_text(
            json.dumps(
                dict(
                    version=1,
                    programs=[
                        dict(
                            id="real-opencode",
                            executable=str(executable),
                            args=["acp"],
                            env=env,
                        )
                    ],
                )
            )
        )
        f.config.chmod(0o600)
        stop(f.proc)
        f.start()
        f.query(
            "session.start",
            executable="real-opencode",
            confirmed=True,
            share_context=True,
        )
        view = observed(f, ["ready", "auth_required", "failed"], 25)
        result["acp"] = dict(
            state=view["state"],
            identity=view["agent"],
            actual_calls="BLOCKED: default Code Mode execute is not granted",
        )
        if view["state"] == "ready":
            saved = json.loads(f.lease_file().read_text())
            audit = f.query("session.context")
            with f.lease_peer(saved) as peer:
                f.end(view)
                result["acp"]["established_peer_denied"] = (
                    peer.request("context.get", {})["error"] == "Unauthorized"
                )
            result["acp"].update(
                audit=audit,
                reconnect_denied=f.rejected(saved),
                lease_removed=not list(f.runtime.glob("session-*.cap")),
                direct_child_reaped=not f.query("session.get")["process_alive"],
            )
        result["database_unchanged"] = f.db.read_bytes() == f.before
        assert result["database_unchanged"]
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--kimi-home", type=Path)
    parser.add_argument("--kimi-executable", type=Path)
    parser.add_argument("--opencode-executable", type=Path)
    parser.add_argument("--allow-kimi-subscription-turn", action="store_true")
    parser.add_argument("--security-report", type=Path)
    args = parser.parse_args()
    assert os.name != "nt", "Installed clients selected on Linux/WSL only"
    assert bool(args.kimi_home) == args.allow_kimi_subscription_turn
    assert not subprocess.check_output(
        ["git", "status", "--porcelain"], cwd=ROOT
    ).strip()
    sha = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    if args.kimi_home:
        assert (
            args.kimi_executable
            and args.kimi_executable.is_absolute()
            and args.kimi_executable.is_file()
        )
        gates = json.loads(args.security_report.read_text())
        assert gates["tested_git_sha"] == sha and not gates["worktree_dirty"]
        assert {"context-process", "lease-unit", "root-clippy"} <= {
            g["name"] for g in gates["gates"] if g["status"] == "PASS"
        }
    out = args.out.resolve()
    assert out.is_relative_to(ROOT / "artifacts/local-p1-k")
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    report = dict(
        tested_git_sha=sha,
        worktree_dirty=False,
        codex=dict(status="NOT_RUN"),
        kimi=dict(status="NOT_RUN"),
    )

    report["entrypoints"] = {}
    selected = {
        "codex": Path(shutil.which("codex")),
        "daemon": Path(binary("acp-daemon")),
        "mcp": Path(binary("acp-mcp")),
    }
    if args.kimi_home:
        selected["kimi"] = args.kimi_executable
    for label, path in selected.items():
        item = {"sha256": hashlib.sha256(path.resolve().read_bytes()).hexdigest()}
        if label in {"codex", "kimi"}:
            item["version"] = subprocess.check_output(
                [str(path), "--version"],
                env=dict(
                    HOME=str(out),
                    CODEX_HOME=str(out / "inventory-codex"),
                    PATH="/usr/bin:/bin",
                ),
                text=True,
                timeout=15,
            ).strip()
        report["entrypoints"][label] = item

    def save_kimi(result):
        report["kimi"] = result
        (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")

    save_kimi(report["kimi"])
    try:
        report["stage"] = "synthetic fixture setup"
        with Scenario() as f:
            report["stage"] = "synthetic daemon startup"
            f.start()
            executable = shutil.which("codex")
            assert executable, "Selected installed Codex unavailable"
            report["stage"] = "Codex direct MCP trial"
            report["codex"] = codex_trial(f, Path(executable).resolve())
            if args.kimi_home:
                report["stage"] = "Kimi managed ACP resume"
                report["kimi"] = kimi_trial(
                    f,
                    args.kimi_home,
                    args.kimi_executable.resolve(),
                    save_kimi,
                )
            report["database_unchanged"] = f.db.read_bytes() == f.before
        if args.opencode_executable:
            assert (
                args.opencode_executable.is_absolute()
                and args.opencode_executable.is_file()
            )
            report["stage"] = "OpenCode no-model discovery/lifecycle"
            report["opencode"] = opencode_trial(args.opencode_executable.resolve())
    except Exception:
        report["evaluation_failure"] = (
            "Sanitized synthetic interoperability failure; original report retained"
        )
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))
    return int(
        "evaluation_failure" in report
        or any(report[c]["status"] == "FAIL" for c in ["codex", "kimi"])
    )


if __name__ == "__main__":
    raise SystemExit(main())
