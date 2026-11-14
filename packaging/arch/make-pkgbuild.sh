#!/usr/bin/env bash
# Writes the release's PKGBUILD: packaging/arch/PKGBUILD with the version of Cargo.toml and the real
# SHA-256 of the release tarball in place of SKIP.
#
#   packaging/arch/make-pkgbuild.sh [TARBALL [OUT]]
#
# TARBALL defaults to target/dist-linux/mdedit-<version>-linux-x86_64.tar.gz, OUT to
# target/dist-linux/PKGBUILD. Validate with: makepkg --printsrcinfo (in the folder of OUT).
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
version="$(awk '/^\[workspace\.package\]/{p=1;next} /^\[/{p=0} p&&/^version[ ]*=/{gsub(/[" ]/,"",$3);print $3;exit}' "$root/Cargo.toml")"
[ -n "$version" ] || { echo "cannot read the version from Cargo.toml" >&2; exit 1; }
tar_gz="${1:-$root/target/dist-linux/mdedit-$version-linux-x86_64.tar.gz}"
out="${2:-$root/target/dist-linux/PKGBUILD}"
[ -f "$tar_gz" ] || { echo "no $tar_gz" >&2; exit 1; }
sum="$(sha256sum "$tar_gz" | cut -d' ' -f1)"
mkdir -p "$(dirname "$out")"
sed -e "s/^pkgver=.*/pkgver=$version/" -e "s/^sha256sums=.*/sha256sums=('$sum')/" \
    "$root/packaging/arch/PKGBUILD" > "$out"
grep -q "^pkgver=$version\$" "$out" && grep -q "^sha256sums=('$sum')\$" "$out" || { echo "PKGBUILD substitution failed" >&2; exit 1; }
echo "wrote $out (pkgver $version, sha256 $sum)"
