# Shared by the make-*.sh package scripts (sourced, not run). They run wherever tar, dpkg-deb and
# rpmbuild exist, normally inside the build image (packaging/linux/build-packages.sh).
#
# Inputs, as environment variables:
#   BIN_DIR            the directory holding the release binaries (default: target/dist-linux/bin)
#   OUT_DIR            where the packages are written (default: target/dist-linux)
#   SOURCE_DATE_EPOCH  the time stamped on every file (default: the commit's, else now)
app=mdedit
bins=(mdedit)
slug=Kaan88/markdown-editor-rs
summary="Light, fast text editor"
maintainer="Kaan88 <Kaan88@users.noreply.github.com>"
homepage="https://github.com/$slug"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
version="$(awk '/^\[workspace\.package\]/{p=1;next} /^\[/{p=0} p&&/^version[ ]*=/{gsub(/[" ]/,"",$3);print $3;exit}' "$root/Cargo.toml")"
[ -n "$version" ] || { echo "cannot read the version from Cargo.toml" >&2; exit 1; }
BIN_DIR="${BIN_DIR:-$root/target/dist-linux/bin}"
OUT_DIR="${OUT_DIR:-$root/target/dist-linux}"
epoch="${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct 2>/dev/null || date +%s)}"
mkdir -p "$OUT_DIR"
for b in "${bins[@]}"; do
    [ -x "$BIN_DIR/$b" ] || { echo "no $BIN_DIR/$b (build it first: packaging/linux/build-appimage.sh)" >&2; exit 1; }
done

# Installs the files every package carries under DEST, laid out as a /usr (or ~/.local) tree:
# bin/, share/applications/, share/icons/hicolor/256x256/apps/. With a second argument "strip" the
# binaries are stripped of their symbol table (deb and rpm; the AppImage and tarball keep the
# binary as built).
stage_tree() {
    local dest="$1" strip="${2:-}"
    install -d -m 755 "$dest/bin" "$dest/share/applications" "$dest/share/icons/hicolor/256x256/apps"
    for b in "${bins[@]}"; do
        install -m 755 "$BIN_DIR/$b" "$dest/bin/$b"
        # The release binary keeps a small symbol table; distribution packages carry none.
        [ "$strip" != strip ] || strip --strip-unneeded "$dest/bin/$b"
    done
    install -m 644 "$root/packaging/linux/$app.desktop" "$dest/share/applications/$app.desktop"
    install -m 644 "$root/assets/$app.png" "$dest/share/icons/hicolor/256x256/apps/$app.png"
}

# The glibc symbol version the binaries ask for at most, e.g. 2.30.
glibc_needed() {
    for b in "${bins[@]}"; do objdump -T "$BIN_DIR/$b"; done | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -V | tail -1
}

# Stamps every file under DIR with the epoch, so a package does not depend on when it was made.
stamp() { find "$1" -exec touch -h -d "@$epoch" {} +; }
