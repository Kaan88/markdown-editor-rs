#!/usr/bin/env bash
# The RPM package: mdedit-<version>-1.x86_64.rpm (Fedora, openSUSE, RHEL and the like), built with
# rpmbuild from packaging/linux/mdedit.spec. rpmbuild rather than fpm: it is the reference tool,
# is one apt/dnf package, derives the Requires from the binary by itself and needs no gem.
#   packaging/linux/make-rpm.sh            (see lib.sh for BIN_DIR, OUT_DIR, SOURCE_DATE_EPOCH)
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
command -v rpmbuild >/dev/null || { echo "make-rpm: rpmbuild not found" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
stage_tree "$work/stage" strip
stamp "$work/stage"
mkdir -p "$work/top"
rpmbuild -bb "$root/packaging/linux/$app.spec" \
    --define "_topdir $work/top" \
    --define "_rpmdir $work/out" \
    --define "app_version $version" \
    --define "stage_dir $work/stage" \
    --define "license_file $root/LICENSE" \
    --define "readme_file $root/README.md" \
    --define "changelog_date $(LC_ALL=C date -u -d "@$epoch" '+%a %b %d %Y')" \
    --define "_buildhost reproducible" \
    --define "use_source_date_epoch_as_buildtime 1" \
    --define "clamp_mtime_to_source_date_epoch 1" \
    --define "_binary_payload w9.gzdio" \
    --target x86_64-linux >/dev/null
rpm_file="$(find "$work/out" -name '*.rpm' | head -1)"
[ -n "$rpm_file" ] || { echo "make-rpm: rpmbuild left no rpm" >&2; exit 1; }
out="$OUT_DIR/$(basename "$rpm_file")"
cp "$rpm_file" "$out"
ls -l "$out"
rpm -qp --requires "$out"
