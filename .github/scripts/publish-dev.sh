#!/usr/bin/env bash
set -euo pipefail

# Serialized by the workflow. An old rerun must not roll dev backwards.
main_sha=$(gh api "repos/$GH_REPO/git/ref/heads/main" --jq .object.sha)
if [[ "$main_sha" != "$GITHUB_SHA" ]]; then
  echo "Skipping superseded commit $GITHUB_SHA; main is $main_sha."
  exit 0
fi

assets=(
  release-assets/phasecraft-linux-x64.tar.gz
  release-assets/phasecraft-windows-x64.zip
  release-assets/phasecraft-macos-arm64.tar.gz
  release-assets/phasecraft-macos-x64.tar.gz
  release-assets/phasecraft-update-linux-x64.bin
  release-assets/phasecraft-update-windows-x64.exe
  release-assets/phasecraft-update-macos-arm64.bin
  release-assets/phasecraft-update-macos-x64.bin
)
for asset in "${assets[@]}"; do
  test -s "$asset"
done
(cd release-assets && sha256sum phasecraft-* > SHA256SUMS.txt)
assets+=(release-assets/SHA256SUMS.txt)

python3 .github/scripts/update-manifest.py release-assets "$GITHUB_SHA"

cat > release-notes.md <<EOF
Rolling development build from main. These downloads are replaced after all
native builds and tests pass. Expect unfinished features and changing behavior.

Commit: $GITHUB_SHA
Build: https://github.com/$GH_REPO/actions/runs/$GITHUB_RUN_ID

Windows: download phasecraft-windows-x64.zip, extract it, and open PowerShell
in the extracted folder. Run:

    .\phasecraft.exe new my-set
    .\phasecraft.exe validate my-set
    .\phasecraft.exe play my-set --dry-run --bars 4

After this first manual download, use .\phasecraft.exe update to install future
builds in place (update --check compares commits). Private-repo access uses
GitHub CLI login or GH_TOKEN/GITHUB_TOKEN; see the README.

See the included README for MIDI routing into Ableton. No Rust installation
is needed to run the executable. SHA256SUMS.txt contains archive checksums.
EOF

# Only the intentionally mutable dev tag is moved; version tags are untouched.
git tag --force dev "$GITHUB_SHA"
git push origin refs/tags/dev --force
if gh release view dev > /dev/null 2>&1; then
  gh release upload dev "${assets[@]}" --clobber
  gh release edit dev --title "Development build" --prerelease --latest=false \
    --notes-file release-notes.md
else
  gh release create dev "${assets[@]}" --verify-tag --title "Development build" \
    --prerelease --latest=false --notes-file release-notes.md
fi

# Publish the commit/checksum manifest last. A concurrent reader fails closed on mixed assets.
gh release upload dev release-assets/update.json --clobber
