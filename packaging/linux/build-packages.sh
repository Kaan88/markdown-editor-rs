#!/usr/bin/env bash
# Builds every Linux package of mdedit from any Linux PC with Docker:
#
#   packaging/linux/build-packages.sh
#
# Output in target/dist-linux/:
#   mdedit-<version>-linux-x86_64.AppImage   packaging/linux/build-appimage.sh (builds the binary too)
#   mdedit-<version>-linux-x86_64.tar.gz     make-tarball.sh
#   mdedit_<version>_amd64.deb               make-deb.sh (+ the lintian report, .lintian.txt)
#   mdedit-<version>-1.x86_64.rpm            make-rpm.sh
#   PKGBUILD                                 packaging/arch/make-pkgbuild.sh (sha256 of the tar.gz filled in)
#
# The binary is built once, in the old-glibc image of packaging/linux/Dockerfile; the tarball, deb
# and rpm are made from that same binary inside the same image (it carries dpkg-deb, rpmbuild and
# lintian), as the caller's user. Needs on the host what build-appimage.sh needs, and nothing
# else: docker, appimagetool, mksquashfs, curl, sha256sum. RUST_TOOLCHAIN picks the toolchain.
# The CI workflow runs this same script.
set -euo pipefail

app=mdedit
image=mdedit-linux-build
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

packaging/linux/build-appimage.sh

dist="$root/target/dist-linux"
epoch="$(git log -1 --format=%ct 2>/dev/null || date +%s)"
docker run --rm --user "$(id -u):$(id -g)" \
    -v "$root:/src:ro" -v "$dist:/out" \
    -e BIN_DIR=/out/bin -e OUT_DIR=/out -e SOURCE_DATE_EPOCH="$epoch" -e TMPDIR=/tmp \
    -w /src "$image" \
    bash -c 'set -e; for s in make-tarball make-deb make-rpm; do echo "== $s"; packaging/linux/$s.sh; done'

packaging/arch/make-pkgbuild.sh
echo
( cd "$dist" && ls -l *.AppImage *.tar.gz *.deb *.rpm PKGBUILD && sha256sum *.AppImage *.tar.gz *.deb *.rpm PKGBUILD )
