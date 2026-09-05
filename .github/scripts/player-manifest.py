"""Publish the signed Player feed last; build metadata carries the full commit."""
from pathlib import Path
import json
import re
import sys
root, commit = Path(sys.argv[1]), sys.argv[2]
assert re.fullmatch('[0-9a-f]{40}', commit)
platforms = {}
for target, name in {
    'windows-x86_64': 'windows-x64-setup.exe',
    'darwin-aarch64': 'macos-arm64.app.tar.gz',
    'darwin-x86_64': 'macos-x64.app.tar.gz',
    'linux-x86_64': 'linux-x64.AppImage',
}.items():
    asset = 'phasecraft-player-' + name
    assert (root / asset).stat().st_size > 0
    signature = (root / (asset + '.sig')).read_text().strip()
    assert signature
    platforms[target] = {
        'signature': signature,
        'url': 'https://github.com/sleepunit-agents/phasecraft/releases/download/dev/' + asset,
    }
(root / 'player-update.json').write_text(json.dumps({
    'version': '0.1.0+' + commit,
    'notes': 'Development build ' + commit,
    'platforms': platforms,
}, indent=2) + '\n')
