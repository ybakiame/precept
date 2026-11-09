"""Preserve Cargo's status/output and publish bounded sanitized failure headlines.

This makes CI diagnosis possible through check annotations without downloading
credentialed log archives. It runs only the synthetic recovery lint/test target.
"""
import argparse
import re
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("check", choices=["lint", "test"])
args = parser.parse_args()
command = (["cargo", "clippy", "--workspace", "--all-targets", "--all-features", "--locked", "--", "-D", "warnings"]
           if args.check == "lint" else
           ["cargo", "test", "--locked", "-p", "acp-rehearsal", "--features", "recovery-tests", "--test", "process_recovery", "--", "--nocapture"])
result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", errors="replace")
print(result.stdout, end="")
print(result.stderr, end="")
if result.returncode:
    lines = (result.stdout + "\n" + result.stderr).splitlines()
    selected = [line.strip()[:400] for line in lines
                if re.search(r"(^error|FAILED|panicked|^\s*(left|right):|^worker refused|^assertion)", line)]
    message = " | ".join(selected[:20]) or "Cargo failed; inspect full job log"
    message = message.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    print(f"::error title=Synthetic recovery {args.check} failure::{message}")
raise SystemExit(result.returncode)
