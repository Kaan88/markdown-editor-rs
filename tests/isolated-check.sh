#!/usr/bin/env bash
# Exercise the same isolation boundary used by shots and cargo test.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
export LOCALAPPDATA=owner-app-data MDEDIT_SETTINGS=owner-settings
export MDV_ENGINE=owner-engine MD_SISTER_MDVIEW=owner-viewer
probe() {
    bash "$here/isolated.sh" bash -eu -c '
        local_data="$LOCALAPPDATA"
        settings="$MDEDIT_SETTINGS"
        case "$(uname -s)" in
            MINGW*|MSYS*|CYGWIN*)
                [[ "$local_data" == *\\local && "$settings" == *\\settings.ini ]]
                local_data="$(cygpath -u "$local_data")"
                settings="$(cygpath -u "$settings")"
                ;;
        esac
        [[ "$local_data" == "$(dirname "$settings")/local" ]]
        [[ -d "$local_data" && ! -e "$settings" ]]
        [[ -z "${MDV_ENGINE+x}${MD_SISTER_MDVIEW+x}" ]]
        echo "$settings"
    '
}
a="$(probe)"
b="$(probe)"
[[ "$a" != "$b" && ! -d "$(dirname "$a")" && ! -d "$(dirname "$b")" ]]
echo "Each invocation has fresh scratch paths, no inherited overrides, and cleans up"
