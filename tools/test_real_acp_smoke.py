"""Explicit opt-in native ACP smoke; isolated HOME/project, never a fixture success claim."""

import argparse, json, os, time
from pathlib import Path
from test_agent_session import SessionFixture
from test_local_service import ROOT


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--opt-in", action="store_true")
    p.add_argument("--executable", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--version", required=True)
    a = p.parse_args()
    assert a.opt_in, "real agent requires explicit operator opt-in"
    assert a.executable.is_absolute() and a.executable.is_file()
    out = a.out.resolve()
    assert out.is_relative_to(ROOT / "artifacts/local-p1-g")
    out.mkdir(parents=True, exist_ok=True, mode=0o700)
    assert (
        os.name != "nt"
    ), "this owner-approved smoke selects the installed Linux Kimi executable"
    with SessionFixture() as f:
        home = f.root / "isolated-home"
        home.mkdir(mode=0o700)
        f.config.write_text(
            json.dumps(
                dict(
                    version=1,
                    programs=[
                        dict(
                            id="real-kimi",
                            executable=str(a.executable.resolve()),
                            args=["acp"],
                            env={
                                "HOME": str(home),
                                "PATH": "/usr/bin:/bin",
                                "TMPDIR": str(home),
                            },
                        )
                    ],
                )
            )
        )
        f.start()
        v = f.query("session.start", executable="real-kimi", confirmed=True)
        deadline = time.monotonic() + 16
        while time.monotonic() < deadline:
            v = f.query("session.get")
            if v["state"] in ["ready", "failed"]:
                break
            time.sleep(0.05)
        result = dict(
            agent="Kimi Code CLI",
            installed_version=a.version,
            transport="native ACP stdio",
            owner_opt_in=True,
            temporary_home=True,
            project="disposable synthetic",
            protocol_version=1 if v["agent"] else None,
            initialize="PASS" if v["agent"] else "FAIL",
            session_new=v["state"],
            prompt="NOT_RUN",
            permission_policy="deny by default; no always choices",
            real_profile_copy="NOT_RUN: no real login/config copied or supplied",
            remote_model_turn="NOT_RUN",
        )
        if v["state"] == "failed":
            result["session_new"] = (
                "AUTH_REQUIRED"
                if v["failure"] == "Authentication required"
                else v["failure"]
            )
            result["status"] = "PARTIAL" if v["agent"] else "FAIL"
        elif v["state"] == "ready":
            f.query(
                "session.prompt",
                session_id=v["session_id"],
                prompt="Reply with ACP_SMOKE_OK only. Do not read or write files, run tools, inspect environment, or call other agents.",
            )
            deadline = time.monotonic() + 125
            while time.monotonic() < deadline:
                v = f.query("session.get")
                for permission in v["permissions"]:
                    f.query(
                        "session.permission",
                        session_id=v["session_id"],
                        request_id=permission["request_id"],
                        option_id=None,
                    )
                if v["state"] in ["completed", "failed"]:
                    break
                time.sleep(0.1)
            result["prompt"] = v["state"]
            result["remote_model_turn"] = "ATTEMPTED"
            result["status"] = "PASS" if v["state"] == "completed" else "FAIL"
            f.end(v)
        else:
            result["status"] = "FAIL"
        result["database_unchanged"] = f.db.read_bytes() == f.before
        (out / "real-acp-result.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result))


if __name__ == "__main__":
    main()
