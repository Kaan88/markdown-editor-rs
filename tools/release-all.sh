#!/usr/bin/env bash
# Builds every file of an mdedit release on Linux, the Windows and Linux files together, and with
# --publish creates the GitHub release. Everything is written to target/release-files-all/:
#
#   mdedit-<version>-linux-x86_64.AppImage   packaging/linux/build-appimage.sh
#   mdedit-<version>-windows-x86_64.exe      tools/release-cross.sh (cross-built, Wine for the installer)
#   mdedit-setup-<version>.exe               tools/release-cross.sh
#   mdedit-update.txt                        tools/write-manifest.sh, both platforms
#
#   tools/release-all.sh --notes 'One short sentence.'                        build and check only
#   tools/release-all.sh --notes '...' --notes-file FILE --publish            also publish
#   tools/release-all.sh --notes '...' --notes-file FILE --publish --dry-run  print the gh command
#
# --notes is the manifest's one line (what the in-app update bar shows); --notes-file holds the
# GitHub release text. It refuses a dirty tree (changes under docs/ and tools/ do not count, the
# files do not depend on them) and a HEAD that no remote branch contains. The release is created
# on the current commit as `v<version>`, marked latest, with all four assets.
set -euo pipefail

app=mdedit
slug=Kaan88/markdown-editor-rs
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
notes="A new version of $app." notes_file="" publish=0 dry=0
while [ $# -gt 0 ]; do
    case "$1" in
        --notes) notes="$2"; shift 2 ;;
        --notes-file) notes_file="$2"; shift 2 ;;
        --publish) publish=1; shift ;;
        --dry-run) dry=1; shift ;;
        *) echo "unknown argument $1" >&2; exit 2 ;;
    esac
done
cd "$root"
fail() { echo "release-all: $*" >&2; exit 1; }
if [ "$publish" = 1 ]; then
    [ -n "$notes_file" ] && [ -f "$notes_file" ] || fail "--publish needs --notes-file FILE (the release text)"
fi

version="$(awk '/^\[workspace\.package\]/{p=1;next} /^\[/{p=0} p&&/^version[ ]*=/{gsub(/[" ]/,"",$3);print $3;exit}' Cargo.toml)"
[ -n "$version" ] || fail "cannot read the version from Cargo.toml"

# A clean tree and a pushed HEAD.
dirty="$(git status --porcelain -- . ':(exclude)tools' ':(exclude)docs')"
[ -z "$dirty" ] || fail "working tree is dirty:
$dirty"
sha="$(git rev-parse HEAD)"
git fetch --quiet origin
[ -n "$(git branch -r --contains "$sha")" ] || fail "HEAD $sha is on no remote branch; push it first"

all="$root/target/release-files-all"
rm -rf "$all"
mkdir -p "$all"

packaging/linux/build-appimage.sh
tools/release-cross.sh --notes "$notes"
cp "target/dist-linux/$app-$version-linux-x86_64.AppImage" "target/dist-cross/$app-$version-windows-x86_64.exe" \
   "target/dist-cross/$app-setup-$version.exe" "$all/"
tools/write-manifest.sh --version "$version" --notes "$notes" --out "$all" \
    --windows "$all/$app-$version-windows-x86_64.exe" --linux "$all/$app-$version-linux-x86_64.AppImage"

echo
echo "release files for $app $version (commit $sha) in $all:"
( cd "$all" && for f in *; do printf '%12d  %s  %s\n' "$(stat -c %s "$f")" "$(sha256sum "$f" | cut -d' ' -f1)" "$f"; done )

if [ "$publish" = 0 ]; then
    echo "not published (no --publish)."
    exit 0
fi
cmd=(gh release create "v$version" "$all/$app-$version-linux-x86_64.AppImage" "$all/$app-$version-windows-x86_64.exe"
     "$all/$app-setup-$version.exe" "$all/$app-update.txt"
     --repo "$slug" --target "$sha" --title "$app $version" --notes-file "$notes_file" --latest)
if [ "$dry" = 1 ]; then
    printf 'dry run, would run:'; printf ' %q' "${cmd[@]}"; echo
    exit 0
fi
"${cmd[@]}"
echo "published https://github.com/$slug/releases/tag/v$version"
