"""Keep subprocess E2E output/status and publish bounded failure headlines to CI."""
import os
import re
import subprocess
import sys

command = (["cargo", "clippy", "--workspace", "--all-targets", "--all-features", "--locked", "--", "-D", "warnings"]
           if len(sys.argv) > 1 and sys.argv[1] == "lint" else
           [sys.executable, "tools/test_local_service.py", "-v"])
result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", errors="replace",
                        env={**os.environ, "PYTHONPATH": "src"}, timeout=600)
print(result.stdout, end="")
print(result.stderr, end="")
if result.returncode:
    selected = [line.strip()[:400] for line in (result.stdout + "\n" + result.stderr).splitlines()
                if re.search(r"(^error|FAILED|FAIL:|ERROR:|AssertionError|TimeoutError|OSError|PermissionError|ValueError|WinError|panicked|^\s*(left|right):)", line)]
    message = " | ".join(selected[:20]) or "Local service validation failed"
    message = message.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    print(f"::error title=Local service process validation::{message}")
raise SystemExit(result.returncode)
