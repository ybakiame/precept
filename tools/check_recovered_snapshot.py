"""Read-only full-record Python oracle for test-runner-owned recovery fixtures."""
import argparse
from pathlib import Path
import tempfile
from check_migration_compat import snapshot

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--synthetic-only", action="store_true", required=True)
parser.add_argument("root", type=Path)
args = parser.parse_args()
root = args.root.resolve(strict=True)
assert not args.root.is_symlink()
assert root.parent == Path(tempfile.gettempdir()).resolve()
assert root.name.startswith("acp-synthetic-recovery-")
paths = [root / "fixture/state/context.sqlite3", root / "backup.sqlite3", root / "output.sqlite3"]
if (root / "restored.sqlite3").exists():
    paths.append(root / "restored.sqlite3")
for path in paths:
    current = root
    for part in path.relative_to(root).parts:
        current /= part
        assert not current.is_symlink()
        assert not getattr(current, "is_junction", lambda: False)()
    assert path.resolve(strict=True).is_relative_to(root)
original = snapshot(paths[0])
assert original[0] == 0
assert snapshot(paths[1]) == original
assert snapshot(paths[2]) == (1, original[1])
if len(paths) == 4:
    assert snapshot(paths[3]) == original
print("PASS: Python full snapshot/evidence equality after fresh-process recovery")
