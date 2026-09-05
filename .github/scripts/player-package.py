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
