#!/usr/bin/env bash
# The Debian package: mdedit_<version>_amd64.deb (works on Debian 11+, Ubuntu 20.04+ and their
# derivatives). Needs dpkg-deb; lintian is run when it is installed.
#   packaging/linux/make-deb.sh            (see lib.sh for BIN_DIR, OUT_DIR, SOURCE_DATE_EPOCH)
#
# Depends are the packages that provide every shared library the binaries are linked against
# (read from the ELF headers and mapped below; a library not in the map stops the script, so a
# new link-time dependency cannot slip through unnoticed) plus the libraries the windowing code
# loads through fontconfig/X11 at run time. libcurl, Wayland and the desktop portal are loaded at
# run time when present, so they are only recommended.
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

declare -A pkg_of=(
    [libc.so.6]=libc6 [libm.so.6]=libc6 [libdl.so.2]=libc6 [libpthread.so.0]=libc6
    [librt.so.1]=libc6 [libgcc_s.so.1]=libgcc-s1 [libfontconfig.so.1]=libfontconfig1
    [libfreetype.so.6]=libfreetype6 [libX11.so.6]=libx11-6 [libX11-xcb.so.1]=libx11-xcb1
    [libxcb.so.1]=libxcb1 [libxkbcommon.so.0]=libxkbcommon0 [libxkbcommon-x11.so.0]=libxkbcommon-x11-0
)
glibc="$(glibc_needed)"
declare -A deps=([libfreetype6]=1 [libxcb1]=1)
for b in "${bins[@]}"; do
    for so in $(objdump -p "$BIN_DIR/$b" | awk '/NEEDED/{print $2}'); do
        p="${pkg_of[$so]:-}"
        [ -n "$p" ] || { echo "make-deb: $b needs $so, which is not in the map of make-deb.sh" >&2; exit 1; }
        deps[$p]=1
    done
done
unset 'deps[libc6]'
depends="libc6 (>= $glibc)"
for p in $(printf '%s\n' "${!deps[@]}" | sort); do depends="$depends, $p"; done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
pkg="$work/pkg"
stage_tree "$pkg/usr" strip
install -d -m 755 "$pkg/usr/share/doc/$app" "$pkg/DEBIAN"
cat > "$pkg/usr/share/doc/$app/copyright" <<COPYRIGHT
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: $app
Source: $homepage

Files: *
Copyright: 2026 Kaan
License: MIT

License: MIT
$(sed -e '1,2d' -e 's/^$/./' -e 's/^/ /' "$root/LICENSE")
COPYRIGHT
chmod 644 "$pkg/usr/share/doc/$app/copyright"
{
    echo "$app ($version-1) unstable; urgency=medium"
    echo
    echo "  * Release $version."
    echo "    $homepage/releases/tag/v$version"
    echo
    echo " -- $maintainer  $(date -u -d "@$epoch" '+%a, %d %b %Y %H:%M:%S +0000')"
} | gzip -9n > "$pkg/usr/share/doc/$app/changelog.Debian.gz"
chmod 644 "$pkg/usr/share/doc/$app/changelog.Debian.gz"

size_kb="$(du -sk --apparent-size "$pkg/usr" | cut -f1)"
cat > "$pkg/DEBIAN/control" <<CONTROL
Package: $app
Version: $version-1
Architecture: amd64
Maintainer: $maintainer
Installed-Size: $size_kb
Depends: $depends
Recommends: libcurl4, libwayland-client0, xdg-desktop-portal
Section: editors
Priority: optional
Homepage: $homepage
Description: $summary
 $app is a light, fast native text editor with syntax colouring, find and
 replace, split view and tabs. This package installs the program, its
 desktop entry and its icon. It does not update itself; use the package
 manager, or the AppImage if you want the built-in updater.
CONTROL
for script in postinst postrm; do
    cat > "$pkg/DEBIAN/$script" <<'SCRIPT'
#!/bin/sh
set -e
case "$1" in
    configure|triggered|remove|purge)
        if command -v update-desktop-database >/dev/null 2>&1; then
            update-desktop-database -q /usr/share/applications || true
        fi
        if command -v gtk-update-icon-cache >/dev/null 2>&1 && [ -f /usr/share/icons/hicolor/index.theme ]; then
            gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
        fi
        ;;
esac
exit 0
SCRIPT
    chmod 755 "$pkg/DEBIAN/$script"
done
# md5sums, as dpkg-buildpackage makes them.
( cd "$pkg" && find usr -type f -print0 | sort -z | xargs -0 md5sum > DEBIAN/md5sums )
chmod 644 "$pkg/DEBIAN/md5sums" "$pkg/DEBIAN/control"
stamp "$pkg"

out="$OUT_DIR/${app}_${version}_amd64.deb"
rm -f "$out"
dpkg-deb --root-owner-group -Zxz -b "$pkg" "$out" >/dev/null
ls -l "$out"
dpkg-deb -I "$out" | sed -n '/Depends/p;/Recommends/p'
if command -v lintian >/dev/null 2>&1; then
    echo "lintian:"
    lintian --no-tag-display-limit "$out" | tee "$OUT_DIR/${app}_${version}_amd64.lintian.txt" || true
fi
