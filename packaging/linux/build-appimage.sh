#!/usr/bin/env bash
# Builds the Linux release of mdedit as an AppImage, from any Linux PC with Docker.
#
#   packaging/linux/build-appimage.sh
#
# Output: target/dist-linux/mdedit-<version>-linux-x86_64.AppImage, and the binary in
# target/dist-linux/bin/. RUST_TOOLCHAIN (default stable) picks the toolchain the image installs.
#
# The binary is built inside packaging/linux/Dockerfile (Ubuntu 20.04, glibc 2.31) from the
# repository mounted read-only, so it runs on distributions at least that old. Caches (kept between
# runs, safe to delete) live under $MD_BUILD_CACHE (default ~/.cache/md-build):
#   mdedit/cargo         cargo registry and git checkouts
#   mdedit/target-linux  cargo's target directory
#   runtime-x86_64-<tag> the AppImage runtime appimagetool embeds (downloaded once, hash checked)
# Needs on the host: docker, appimagetool, mksquashfs (used by appimagetool), curl, sha256sum.
set -euo pipefail

app=mdedit
image=mdedit-linux-build
runtime_tag=20251108
runtime_sha256=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cache="${MD_BUILD_CACHE:-$HOME/.cache/md-build}"
cd "$root"

version="$(awk '/^\[workspace\.package\]/{p=1;next} /^\[/{p=0} p&&/^version[ ]*=/{gsub(/[" ]/,"",$3);print $3;exit}' Cargo.toml)"
[ -n "$version" ] || { echo "cannot read the version from Cargo.toml" >&2; exit 1; }

mkdir -p "$cache/$app/cargo" "$cache/$app/target-linux"

# 1. The build image (a no-op when nothing changed).
docker build -q -t "$image" --build-arg "RUST_TOOLCHAIN=${RUST_TOOLCHAIN:-stable}" packaging/linux >/dev/null

# 2. The release binary, built as the caller's user from the read-only repository.
docker run --rm --user "$(id -u):$(id -g)" \
    -v "$root:/src:ro" \
    -v "$cache/$app/cargo:/cache/cargo" \
    -v "$cache/$app/target-linux:/cache/target" \
    -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR=/cache/target -e CARGO_TERM_COLOR=never \
    -w /src "$image" \
    bash -c 'PATH=/opt/cargo/bin:$PATH cargo build --release --locked -p mdedit'
bin="$cache/$app/target-linux/release/$app"
[ -x "$bin" ] || { echo "build left no $bin" >&2; exit 1; }

# 3. The glibc the binary asks for must be old enough.
need="$(objdump -T "$bin" | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -V | tail -1)"
echo "highest GLIBC symbol version needed: $need"
if [ "$(printf '%s\n2.31\n' "$need" | sort -V | tail -1)" != "2.31" ]; then
    echo "needs glibc $need, newer than 2.31" >&2
    exit 1
fi

# 4. The AppDir. The binary is also left in target/dist-linux/bin for build-packages.sh.
dist="$root/target/dist-linux"
mkdir -p "$dist/bin"
install -m 755 "$bin" "$dist/bin/$app"
appdir="$dist/AppDir"
rm -rf "$appdir"
mkdir -p "$appdir/usr/bin"
install -m 755 "$bin" "$appdir/usr/bin/$app"
install -m 644 assets/mdedit.png "$appdir/$app.png"
install -m 644 assets/mdedit.png "$appdir/.DirIcon"
install -m 644 packaging/linux/$app.desktop "$appdir/$app.desktop"
ln -s "usr/bin/$app" "$appdir/AppRun"

# 5. The runtime appimagetool embeds, fetched once and checked, so appimagetool never downloads.
runtime="$cache/runtime-x86_64-$runtime_tag"
if [ ! -f "$runtime" ]; then
    curl -fsSL -o "$runtime.part" \
        "https://github.com/AppImage/type2-runtime/releases/download/$runtime_tag/runtime-x86_64"
    mv "$runtime.part" "$runtime"
fi
echo "$runtime_sha256  $runtime" | sha256sum -c --quiet - \
    || { echo "AppImage runtime $runtime does not match its pinned SHA-256" >&2; exit 1; }

# 6. Pack. SOURCE_DATE_EPOCH (the commit's time) makes the squashfs stamps the same every run.
out="$dist/$app-$version-linux-x86_64.AppImage"
rm -f "$out"
epoch="$(git log -1 --format=%ct 2>/dev/null || date +%s)"
find "$appdir" -exec touch -h -d "@$epoch" {} +
ARCH=x86_64 SOURCE_DATE_EPOCH="$epoch" appimagetool --no-appstream --runtime-file "$runtime" \
    "$appdir" "$out" >/dev/null
chmod 755 "$out"
ls -l "$out"
sha256sum "$out"
