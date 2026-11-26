#!/usr/bin/env bash
# Run one shot or test invocation with no access to the normal settings or install folder.
set -euo pipefail
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/local" "$scratch/config" "$scratch/data"
local_data="$scratch/local"
settings="$scratch/settings.ini"
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
        local_data="$(cygpath -w "$local_data")"
        settings="$(cygpath -w "$settings")"
        ;;
esac
export LOCALAPPDATA="$local_data" MDEDIT_SETTINGS="$settings"
export XDG_CONFIG_HOME="$scratch/config" XDG_DATA_HOME="$scratch/data"
unset MDV_ENGINE MDV_ENGINE_STALL MD_SISTER_MDVIEW MD_SISTER_MDEDIT APPIMAGE
"$@"
