"""Describe the exact tested native executables; publish after their assets."""
import hashlib
import json
from pathlib import Path
import sys

root = Path(sys.argv[1])
commit = sys.argv[2]
assert len(commit) == 40 and all(c in "0123456789abcdef" for c in commit)
targets = {}
for platform in ("linux-x64", "windows-x64", "macos-arm64", "macos-x64"):
    suffix = "exe" if platform.startswith("windows") else "bin"
    path = root / f"phasecraft-update-{platform}.{suffix}"
    data = path.read_bytes()
    assert data
    targets[platform] = dict(asset=path.name, sha256=hashlib.sha256(data).hexdigest(), size=len(data))
(root / "update.json").write_text(json.dumps(dict(schema=1, commit=commit, targets=targets), indent=2) + "\n")
