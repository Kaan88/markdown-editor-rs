#!/usr/bin/env bash
# Writes the update manifests for one or both platforms, re-reads them and checks every hash.
#
#   tools/write-manifest.sh --version 1.1.0 --notes 'One short sentence.' --out DIR \
#       [--windows DIR/mdedit-1.1.0-windows-x86_64.exe --windows-engine DIR/mdview-engine.exe \
#        --windows-ai DIR/mdedit-ai.exe] \
#       [--linux DIR/mdedit-1.1.0-linux-x86_64.AppImage]
#
# At least one platform. Two files are written to DIR, with the same platforms:
#
#   mdedit-update.txt    `manifest: 1`, read by every mdedit up to 1.0.6 (and by 1.1 on Linux).
#                        Those builds refuse any key they do not know, so it lists only the exe
#                        and the engine; an update from it brings no mdedit-ai, and 1.1 fetches
#                        that at its first launch (`updater::restore_engine`).
#   mdedit-update-2.txt  `manifest: 2`, read by mdedit 1.1 and later on Windows: the same lines
#                        and `windows-x86_64.mdedit-ai.url/sha256`, the AI engine beside the exe.
#
# The asset names are the file names given, the URLs are where GitHub serves the assets of tag
# v<version>; the grammar is mdshared-app's update.rs (https only, at most 8 KiB, notes at most
# 1024 bytes without control characters). Windows carries the engine keys (mdview-engine.exe and,
# in format 2, mdedit-ai.exe are files of their own); Linux gets none, both are in the AppImage.
# bins/mdedit/tests/release_manifest.rs reads each file with the build that reads it.
set -euo pipefail

slug=Kaan88/markdown-editor-rs
app=mdedit
version="" notes="" out="" win="" weng="" wai="" lin=""
while [ $# -gt 0 ]; do
    case "$1" in
        --version) version="$2"; shift 2 ;;
        --notes) notes="$2"; shift 2 ;;
        --out) out="$2"; shift 2 ;;
        --windows) win="$2"; shift 2 ;;
        --windows-engine) weng="$2"; shift 2 ;;
        --windows-ai) wai="$2"; shift 2 ;;
        --linux) lin="$2"; shift 2 ;;
        *) echo "unknown argument $1" >&2; exit 2 ;;
    esac
done
fail() { echo "write-manifest: $*" >&2; exit 1; }
[ -n "$version" ] && [ -n "$out" ] || fail "--version and --out are required"
[ -n "$win$lin" ] || fail "give --windows and/or --linux"
{ [ -n "$win" ] && [ -n "$weng" ] && [ -n "$wai" ]; } || { [ -z "$win$weng$wai" ] || fail "Windows needs --windows, --windows-engine and --windows-ai"; }
printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' || fail "bad version $version"
[ -n "$notes" ] || notes="A new version of $app."
if printf '%s' "$notes" | LC_ALL=C grep -q '[[:cntrl:]]'; then fail "notes: one line, no control characters"; fi
[ "$(printf '%s' "$notes" | wc -c)" -le 1024 ] || fail "notes: at most 1024 bytes"
mkdir -p "$out"

base="https://github.com/$slug/releases/download/v$version"
sha() { sha256sum "$1" | cut -d' ' -f1; }

# write FORMAT FILE: the manifest of that format.
write() {
    local format="$1" manifest="$2" entry platform file asset
    {
        echo "manifest: $format"
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
            if [ "$platform" = windows-x86_64 ]; then
                [ -f "$weng" ] || fail "$weng does not exist"
                [ "$(basename "$weng")" = mdview-engine.exe ] || fail "the engine must be named mdview-engine.exe"
                echo "$platform.engine.url: $base/$(basename "$weng")"
                echo "$platform.engine.sha256: $(sha "$weng")"
                if [ "$format" = 2 ]; then
                    [ -f "$wai" ] || fail "$wai does not exist"
                    [ "$(basename "$wai")" = mdedit-ai.exe ] || fail "the AI engine must be named mdedit-ai.exe"
                    echo "$platform.mdedit-ai.url: $base/$(basename "$wai")"
                    echo "$platform.mdedit-ai.sha256: $(sha "$wai")"
                fi
            fi
        done
    } > "$manifest"
}

# check FORMAT FILE: re-reads what was written: header, keys, URLs, and every hash against the
# file it names.
check() {
    local format="$1" manifest="$2" checked=0 entry platform file asset unknown allowed
    [ "$(head -1 "$manifest")" = "manifest: $format" ] || fail "$manifest: header wrong"
    [ "$(wc -c < "$manifest")" -le 8192 ] || fail "$manifest larger than 8 KiB"
    [ "$(wc -l < "$manifest")" -le 64 ] || fail "$manifest has more than 64 lines"
    get() { grep -m1 "^$1: " "$manifest" | sed "s/^$1: //"; }
    [ "$(get version)" = "$version" ] || fail "$manifest: version does not match"
    [ "$(get notes)" = "$notes" ] || fail "$manifest: notes do not match"
    for entry in "windows-x86_64:$win" "linux-x86_64:$lin"; do
        platform="${entry%%:*}" file="${entry#*:}"
        [ -n "$file" ] || continue
        asset="$(get "$platform.asset")"
        [ "$asset" = "$(basename "$file")" ] || fail "$manifest: $platform.asset does not match"
        [ "$(get "$platform.url")" = "$base/$asset" ] || fail "$manifest: $platform.url does not match"
        [ "$(get "$platform.sha256")" = "$(sha "$file")" ] || fail "$manifest: $platform.sha256 does not match $file"
        checked=$((checked + 1))
    done
    if [ -n "$win" ]; then
        [ "$(get windows-x86_64.engine.url)" = "$base/$(basename "$weng")" ] || fail "$manifest: windows-x86_64.engine.url does not match"
        [ "$(get windows-x86_64.engine.sha256)" = "$(sha "$weng")" ] || fail "$manifest: windows-x86_64.engine.sha256 does not match $weng"
        checked=$((checked + 1))
        if [ "$format" = 2 ]; then
            [ "$(get windows-x86_64.mdedit-ai.url)" = "$base/$(basename "$wai")" ] || fail "$manifest: windows-x86_64.mdedit-ai.url does not match"
            [ "$(get windows-x86_64.mdedit-ai.sha256)" = "$(sha "$wai")" ] || fail "$manifest: windows-x86_64.mdedit-ai.sha256 does not match $wai"
            checked=$((checked + 1))
        fi
    fi
    # Format 1 must hold nothing a build up to 1.0.6 does not know.
    allowed='manifest|version|notes|(windows|linux)-x86_64\.(asset|url|sha256)|windows-x86_64\.engine\.(url|sha256)'
    [ "$format" = 1 ] || allowed="$allowed|windows-x86_64\.mdedit-ai\.(url|sha256)"
    unknown="$(grep -Ev "^($allowed): " "$manifest" || true)"
    [ -z "$unknown" ] || fail "$manifest: unexpected lines: $unknown"
    echo "wrote $manifest ($(wc -c < "$manifest") bytes, $checked file(s) checked)"
}

write 1 "$out/$app-update.txt"
check 1 "$out/$app-update.txt"
write 2 "$out/$app-update-2.txt"
check 2 "$out/$app-update-2.txt"
