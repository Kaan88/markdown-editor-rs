#!/usr/bin/env bash
# Release workflow, first step: reads the workspace version, refuses a tag that is not v<version>,
# and decides whether this run publishes. Writes version= and publish= to $GITHUB_OUTPUT.
# Publishes for a pushed tag, and for a manual run with the input publish=true; every other run
# (a manual run with the default, a push to the ci branch) is a dry build.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
version="$(awk '/^\[workspace\.package\]/{p=1;next} /^\[/{p=0} p&&/^version[ ]*=/{gsub(/[" ]/,"",$3);print $3;exit}' "$root/Cargo.toml")"
[ -n "$version" ] || { echo "cannot read the version from Cargo.toml" >&2; exit 1; }
publish=false
if [ "${GITHUB_REF_TYPE:-}" = tag ]; then
    [ "$GITHUB_REF_NAME" = "v$version" ] || {
        echo "::error::tag $GITHUB_REF_NAME does not match the workspace version: expected v$version" >&2
        exit 1
    }
    publish=true
elif [ "${INPUT_PUBLISH:-false}" = true ]; then
    publish=true
fi
echo "version $version, publish $publish"
{ echo "version=$version"; echo "publish=$publish"; } >> "${GITHUB_OUTPUT:-/dev/stdout}"
