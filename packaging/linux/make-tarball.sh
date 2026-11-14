#!/usr/bin/env bash
# The generic Linux tarball: mdedit-<version>-linux-x86_64.tar.gz, one folder holding bin/,
# share/ (desktop entry, icon), LICENSE, README.md, install.sh and uninstall.sh.
#   packaging/linux/make-tarball.sh        (see lib.sh for BIN_DIR, OUT_DIR, SOURCE_DATE_EPOCH)
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

name="$app-$version-linux-x86_64"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
tree="$work/$name"
stage_tree "$tree"
install -m 644 "$root/LICENSE" "$tree/LICENSE"
install -m 644 "$root/README.md" "$tree/README.md"
install -m 755 "$root/packaging/linux/tarball-install.sh" "$tree/install.sh"
install -m 755 "$root/packaging/linux/tarball-uninstall.sh" "$tree/uninstall.sh"
stamp "$tree"
out="$OUT_DIR/$name.tar.gz"
rm -f "$out"
tar -C "$work" --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$epoch" -cf - "$name" | gzip -9n > "$out"
ls -l "$out"
