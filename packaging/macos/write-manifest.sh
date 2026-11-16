#!/usr/bin/env bash
# Writes <app>-update-macos.txt, the manifest the macOS builds read (they only tell of a new
# version and open the download page: docs/MACOS.md, decision 5), re-reads it and checks every
# hash. The grammar is mdshared-app's update.rs; the asset names are the file names given, the
# URLs where GitHub serves the assets of tag v<version>.
#
#   packaging/macos/write-manifest.sh APP SLUG VERSION NOTES-LINE OUT-DIR ARM64-ZIP X86_64-ZIP
set -euo pipefail
app="$1" slug="$2" version="$3" notes="$4" out="$5" arm="$6" x64="$7"
fail() { echo "macos write-manifest: $*" >&2; exit 1; }
printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' || fail "bad version $version"
if printf '%s' "$notes" | LC_ALL=C grep -q '[[:cntrl:]]'; then fail "notes: one line"; fi
[ "$(printf '%s' "$notes" | wc -c)" -le 1024 ] || fail "notes: at most 1024 bytes"
base="https://github.com/$slug/releases/download/v$version"
manifest="$out/$app-update-macos.txt"
sha() { shasum -a 256 "$1" 2>/dev/null | cut -d' ' -f1 || sha256sum "$1" | cut -d' ' -f1; }
mkdir -p "$out"
{
    echo "manifest: 1"
    echo "version: $version"
    [ -z "$notes" ] || echo "notes: $notes"
    for pair in "macos-aarch64 $arm" "macos-x86_64 $x64"; do
        set -- $pair
        [ -f "$2" ] || fail "$2 does not exist"
        name="$(basename "$2")"
        printf '%s' "$name" | grep -Eq '^[A-Za-z0-9._-]{1,128}$' || fail "bad asset name $name"
        echo "$1.asset: $name"
        echo "$1.url: $base/$name"
        echo "$1.sha256: $(sha "$2")"
    done
} > "$manifest"
[ "$(wc -c < "$manifest")" -le 8192 ] || fail "manifest larger than 8 KiB"
grep -q "^version: $version$" "$manifest" || fail "version does not match"
for pair in "macos-aarch64 $arm" "macos-x86_64 $x64"; do
    set -- $pair
    grep -qx "$1.sha256: $(sha "$2")" "$manifest" || fail "$1.sha256 does not match $2"
done
cat "$manifest"
