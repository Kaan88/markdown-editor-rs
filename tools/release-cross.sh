#!/usr/bin/env bash
# Builds the Windows release files of mdedit on Linux: the same files tools\release.ps1 makes.
#
#   tools/release-cross.sh [--notes 'One short sentence.'] [--no-installer]
#
# Output in target/dist-cross/:
#   mdedit-<version>-windows-x86_64.exe   the executable
#   mdview-engine.exe                     the preview pane's engine (formulas, diagrams), built from
#                                         the shared toolkit at the pinned revision (packaging/engine-source.sh);
#                                         the updater installs both
#   mdedit-setup-<version>.exe            the Inno Setup installer (ISCC.exe run under Wine)
#   mdedit-update.txt                     the manifest, Windows entry only (release-all.sh writes
#                                         the one with both platforms)
#
# The exe is cross-compiled in packaging/windows-cross/Dockerfile with cargo-xwin, which downloads
# the MSVC C runtime and the Windows SDK from Microsoft on its first run (cached under
# $MD_BUILD_CACHE/xwin). USING THEM MEANS ACCEPTING MICROSOFT'S LICENCE
# (https://go.microsoft.com/fwlink/?LinkId=2086102). cargo-xwin 0.23.1 has no accept flag or
# variable of its own: it accepts for you when it downloads, so running this script is that
# acceptance.
#
# Caches (default ~/.cache/md-build, override with MD_BUILD_CACHE), all safe to delete:
#   mdedit/cargo        cargo registry and git checkouts
#   mdedit/target-win   cargo's target directory for the Windows build
#   xwin                the downloaded CRT and SDK
#   innosetup           a copy of the Inno Setup folder (from $INNO_SETUP_DIR, default the one on the
#                       mounted Windows drive, "/mnt/C/Program Files/Inno Setup 7")
#   wine-prefix         the WINEPREFIX ISCC.exe runs in
# Needs on the host: docker, wine (for the installer), sha256sum.
set -euo pipefail

app=mdedit
image=mdedit-windows-cross
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cache="${MD_BUILD_CACHE:-$HOME/.cache/md-build}"
notes="A new version of $app."
installer=1
while [ $# -gt 0 ]; do
    case "$1" in
        --notes) notes="$2"; shift 2 ;;
        --no-installer) installer=0; shift ;;
        *) echo "unknown argument $1" >&2; exit 2 ;;
    esac
done
cd "$root"

version="$(awk '/^\[workspace\.package\]/{p=1;next} /^\[/{p=0} p&&/^version[ ]*=/{gsub(/[" ]/,"",$3);print $3;exit}' Cargo.toml)"
[ -n "$version" ] || { echo "cannot read the version from Cargo.toml" >&2; exit 1; }
dist="$root/target/dist-cross"
mkdir -p "$cache/$app/cargo" "$cache/$app/target-win" "$cache/xwin" "$dist"

# 1. The executable.
docker build -q -t "$image" packaging/windows-cross >/dev/null
docker run --rm --user "$(id -u):$(id -g)" \
    -v "$root:/src:ro" \
    -v "$cache/$app/cargo:/cache/cargo" \
    -v "$cache/$app/target-win:/cache/target" \
    -v "$cache/xwin:/cache/xwin" \
    -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target -e XWIN_CACHE_DIR=/cache/xwin \
    -e MD_LLVM_RC=/usr/bin/llvm-rc -e CARGO_TERM_COLOR=never \
    -w /src "$image" \
    bash -c 'set -e; export PATH=/opt/cargo/bin:$PATH
        cargo xwin build --release --locked --target x86_64-pc-windows-msvc -p mdedit -p mdedit-ai
        packaging/engine-source.sh /cache/target/engine-src
        RUSTFLAGS="-C target-feature=+crt-static" cargo xwin build --release --locked --target x86_64-pc-windows-msvc \
            -p mdshared-md-engine --bin mdview-engine --manifest-path /cache/target/engine-src/Cargo.toml --target-dir /cache/target/engine'
built="$cache/$app/target-win/x86_64-pc-windows-msvc/release/$app.exe"
engine="$cache/$app/target-win/engine/x86_64-pc-windows-msvc/release/mdview-engine.exe"
[ -f "$built" ] || { echo "build left no $built" >&2; exit 1; }
[ -f "$engine" ] || { echo "build left no $engine" >&2; exit 1; }
ai="$cache/$app/target-win/x86_64-pc-windows-msvc/release/mdedit-ai.exe"
[ -f "$ai" ] || { echo "build left no $ai" >&2; exit 1; }

asset="$app-$version-windows-x86_64.exe"
rm -f "$dist/$asset" "$dist/mdview-engine.exe" "$dist/mdedit-ai.exe" "$dist/$app-update-2.txt" "$dist/$app-setup-$version.exe" "$dist/$app-update.txt"
cp "$built" "$dist/$asset"
cp "$engine" "$dist/mdview-engine.exe"
cp "$ai" "$dist/mdedit-ai.exe"

# 2. The installer: ISCC.exe under Wine, on the same .iss the PowerShell script uses. The script
# names its inputs relative to itself (..\..\target\release\mdedit.exe), so a scratch tree with the
# same layout holds the cross-built exe where the script looks for it.
if [ "$installer" = 1 ]; then
    iscc_dir="$cache/innosetup"
    if [ ! -f "$iscc_dir/ISCC.exe" ]; then
        src="${INNO_SETUP_DIR:-/mnt/C/Program Files/Inno Setup 7}"
        [ -f "$src/ISCC.exe" ] || { echo "no Inno Setup at $src (set INNO_SETUP_DIR)" >&2; exit 1; }
        mkdir -p "$iscc_dir"
        cp -r "$src/." "$iscc_dir/"
    fi
    stage="$root/target/cross-installer"
    rm -rf "$stage"
    mkdir -p "$stage/packaging/windows" "$stage/assets" "$stage/target/release"
    cp packaging/windows/mdedit.iss "$stage/packaging/windows/"
    cp assets/mdedit.ico "$stage/assets/"
    cp "$built" "$stage/target/release/mdedit.exe"
    cp "$engine" "$stage/target/release/mdview-engine.exe"
    cp "$ai" "$stage/target/release/mdedit-ai.exe"
    export WINEPREFIX="$cache/wine-prefix" WINEARCH=win64 WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml="
    wine "$iscc_dir/ISCC.exe" "/DAppVersion=$version" "$(winepath -w "$stage/packaging/windows/mdedit.iss" 2>/dev/null)" >/dev/null
    cp "$stage/target/installer/$app-setup-$version.exe" "$dist/"
    # ISCC under Wine leaves Inno Setup's stock icons in the setup beside ours (Wine's resource
    # update does not delete them). Take them out, so the file has the same icons as one built
    # on Windows. Needs python3 with pefile; without it the setup is left as built.
    if python3 -c 'import pefile' 2>/dev/null; then
        python3 "$root/tools/fix-setup-icons.py" "$dist/$app-setup-$version.exe" "$dist/$app-setup-$version.exe.fixed" \
            && mv -f "$dist/$app-setup-$version.exe.fixed" "$dist/$app-setup-$version.exe"
    else
        echo "python3 pefile not found: the setup keeps Inno Setup's stock icons" >&2
    fi
fi

# 3. The manifest, Windows entry only.
tools/write-manifest.sh --version "$version" --notes "$notes" --out "$dist" --windows "$dist/$asset" --windows-engine "$dist/mdview-engine.exe" --windows-ai "$dist/mdedit-ai.exe"

( cd "$dist" && ls -l "$asset" mdview-engine.exe "$app-update.txt" $( [ "$installer" = 1 ] && echo "$app-setup-$version.exe" ) && sha256sum "$asset" $( [ "$installer" = 1 ] && echo "$app-setup-$version.exe" ) )
