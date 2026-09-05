"""Give native player installers stable, platform-specific release names."""
from pathlib import Path
import shutil
import sys

platform = sys.argv[1]
bundle = Path("desktop/target/release/bundle")
if platform == "windows-x64":
    pattern, name = "nsis/*-setup.exe", f"phasecraft-player-{platform}-setup.exe"
elif platform.startswith("macos"):
    pattern, name = "dmg/*.dmg", f"phasecraft-player-{platform}.dmg"
else:
    pattern, name = "deb/*.deb", f"phasecraft-player-{platform}.deb"
files = list(bundle.glob(pattern))
assert len(files) == 1, files
out = Path("player-assets")
out.mkdir(exist_ok=True)
shutil.copyfile(files[0], out / name)

# Signed updater payloads use fixed names independent of the display version.
if platform == 'windows-x64':
    update_pattern, update_name = 'nsis/*-setup.exe', name
elif platform.startswith('macos'):
    update_pattern, update_name = 'macos/*.app.tar.gz', f'phasecraft-player-{platform}.app.tar.gz'
else:
    update_pattern, update_name = 'appimage/*.AppImage', f'phasecraft-player-{platform}.AppImage'
updates = list(bundle.glob(update_pattern))
assert len(updates) == 1, updates
shutil.copyfile(updates[0], out / update_name)
shutil.copyfile(str(updates[0]) + '.sig', out / (update_name + '.sig'))
