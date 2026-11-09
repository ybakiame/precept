"""Release NSH terminal: actual PTY paint/input, 10k dataset and measurements."""
import argparse
import json
import os
from pathlib import Path
import time
from local_console_probe import TerminalProbe, tui_available
from test_local_service import Fixture


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    reason = tui_available()
    if reason:
        print("NOT_RUN: " + reason)
        return
    import psutil
    binary = str(args.binary.resolve())
    samples = []
    with Fixture() as f:
        for _ in range(3):
            t = TerminalProbe([binary, *f.location, "--project", "alpha", "--task-id", "shared"])
            try:
                ready = t.wait("Project: alpha")
                t.wait("LIVE_READ_ONLY")
                t.wait("Handoff shared")
                samples.append(dict(startup_to_screen_seconds=round(ready, 4), rss_bytes=psutil.Process(t.proc.pid).memory_info().rss))
                t.send(b"\t\t\r")
                t.wait("Project: beta")
                t.focus(b"\x0b", "Task ID")
                # Plain q must be accepted as task text, never quit the app.
                t.send(b"quality-task\r")
                t.wait("Empty — no handoff for task quality-task")
            finally:
                t.close()
            assert f.cli("health")["ready"]
        assert f.db.read_bytes() == f.before
    t = TerminalProbe([binary, "--synthetic-rows", "10000"])
    try:
        t.wait("OFFLINE_SYNTHETIC")
        t.wait("10000 confirmed rows")
        started = time.monotonic()
        t.focus(b"\x06", "Filter confirmed preferences")
        t.send(b"event-09999")
        t.wait("1 confirmed rows")
        t.wait("event-09999")
        latency = time.monotonic() - started
        assert latency < 2, "filter-to-painted-screen latency exceeded local acceptance bound"
        # Only synthetic terminal text may be retained.
        args.out.mkdir(parents=True, exist_ok=True, mode=0o700)
        (args.out / "tui-10k-synthetic.txt").write_text("\n".join(t.screen.display) + "\n", encoding="utf-8")
    finally:
        t.close()
    result = dict(status="PASS", backend="NSH/ratatui real ConPTY" if os.name == "nt" else "NSH/ratatui real PTY", profile="release", sample_count=len(samples), samples=samples,
                  synthetic_rows=10000, filter_to_painted_screen_seconds=round(latency, 4), binary_bytes=args.binary.stat().st_size,
                  keyboard="Tab selection; Ctrl+K exact lookup including q; Ctrl+F filter; Ctrl+Q clean close", daemon_outlives_ui=True)
    (args.out / "tui-result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
