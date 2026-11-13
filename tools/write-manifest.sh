#!/usr/bin/env bash
# Writes mdedit-update.txt for one or both platforms, re-reads it and checks every hash.
#
#   tools/write-manifest.sh --version 0.4.1 --notes 'One short sentence.' --out DIR \
#       [--windows DIR/mdedit-0.4.1-windows-x86_64.exe] [--linux DIR/mdedit-0.4.1-linux-x86_64.AppImage]
#
# At least one platform. The asset names are the file names given, the URLs are where GitHub serves
# the assets of tag v<version>; the grammar is mdshared-app's update.rs (manifest: 1, https only,
# at most 8 KiB, notes at most 1024 bytes without control characters). Linux gets no engine keys.
# The manifest is written to DIR/mdedit-update.txt.
set -euo pipefail

slug=Kaan88/markdown-editor-rs
app=mdedit
version="" notes="" out="" win="" lin=""
while [ $# -gt 0 ]; do
    case "$1" in
        --version) version="$2"; shift 2 ;;
        --notes) notes="$2"; shift 2 ;;
        --out) out="$2"; shift 2 ;;
        --windows) win="$2"; shift 2 ;;
        --linux) lin="$2"; shift 2 ;;
        *) echo "unknown argument $1" >&2; exit 2 ;;
    esac
done
fail() { echo "write-manifest: $*" >&2; exit 1; }
[ -n "$version" ] && [ -n "$out" ] || fail "--version and --out are required"
[ -n "$win$lin" ] || fail "give --windows and/or --linux"
printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' || fail "bad version $version"
[ -n "$notes" ] || notes="A new version of $app."
if printf '%s' "$notes" | LC_ALL=C grep -q '[[:cntrl:]]'; then fail "notes: one line, no control characters"; fi
[ "$(printf '%s' "$notes" | wc -c)" -le 1024 ] || fail "notes: at most 1024 bytes"
mkdir -p "$out"

base="https://github.com/$slug/releases/download/v$version"
manifest="$out/$app-update.txt"
sha() { sha256sum "$1" | cut -d' ' -f1; }
{
    echo "manifest: 1"
    echo "version: $version"
    echo "notes: $notes"
    for entry in "windows-x86_64:$win" "linux-x86_64:$lin"; do
        platform="${entry%%:*}" file="${entry#*:}"
        [ -n "$file" ] || continue
        [ -f "$file" ] || fail "$file does not exist"
        asset="$(basename "$file")"
        printf '%s' "$asset" | grep -Eq '^[A-Za-z0-9._-]{1,128}$' || fail "bad asset name $asset"
        echo "$platform.asset: $asset"
        echo "$platform.url: $base/$asset"
        echo "$platform.sha256: $(sha "$file")"
    done
} > "$manifest"

# Re-read what was written: header, keys, URLs, and every hash against the file it names.
[ "$(head -1 "$manifest")" = "manifest: 1" ] || fail "header wrong"
[ "$(wc -c < "$manifest")" -le 8192 ] || fail "manifest larger than 8 KiB"
[ "$(wc -l < "$manifest")" -le 64 ] || fail "manifest has more than 64 lines"
get() { grep -m1 "^$1: " "$manifest" | sed "s/^$1: //"; }
[ "$(get version)" = "$version" ] || fail "version does not match"
[ "$(get notes)" = "$notes" ] || fail "notes do not match"
checked=0
for entry in "windows-x86_64:$win" "linux-x86_64:$lin"; do
    platform="${entry%%:*}" file="${entry#*:}"
    [ -n "$file" ] || continue
    asset="$(get "$platform.asset")"
    [ "$asset" = "$(basename "$file")" ] || fail "$platform.asset does not match"
    [ "$(get "$platform.url")" = "$base/$asset" ] || fail "$platform.url does not match"
    [ "$(get "$platform.sha256")" = "$(sha "$file")" ] || fail "$platform.sha256 does not match $file"
    checked=$((checked + 1))
done
unknown="$(grep -Ev '^(manifest|version|notes|(windows|linux)-x86_64\.(asset|url|sha256)): ' "$manifest" || true)"
[ -z "$unknown" ] || fail "unexpected lines: $unknown"
echo "wrote $manifest ($(wc -c < "$manifest") bytes, $checked platform(s) checked)"
