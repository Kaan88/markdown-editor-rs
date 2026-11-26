# Sourced by the native UI tests. Only the freshly copied bundle may be addressed.
mac_fail() { echo "FAIL $*" >&2; exit 1; }

launch() {
    local source="$1" tool_dir bundle plist
    shift
    tool_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    [[ "$source" == *.app && -d "$source" ]] || mac_fail "expected a built .app"
    scratch="$(mktemp -d)"
    scratch="$(cd "$scratch" && pwd -P)"
    pid=""
    owned=""
    trap stop EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    bundle="$scratch/mdedit.app"
    ditto "$source" "$bundle"
    bundle_id="com.kaan88.mdedit.uitest.$(uuidgen | tr '[:upper:]' '[:lower:]')"
    plist="$bundle/Contents/Info.plist"
    /usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $bundle_id" "$plist"
    exe="$bundle/Contents/MacOS/mdedit"
    [[ -x "$exe" ]] || mac_fail "scratch executable is missing"
    codesign --force --sign - "$bundle"
    mac_tool="$scratch/mac-ui"
    swiftc -parse-as-library "$tool_dir/mac-ui.swift" -o "$mac_tool"
    "$mac_tool" absent "$exe" "$bundle_id"
    mkdir "$scratch/local"
    document="${!#}"
    [[ "$document" == /* && -f "$document" ]] || mac_fail "expected an absolute scratch document"
    open -n -g \
        --env "MDEDIT_SETTINGS=$scratch/settings.ini" --env "LOCALAPPDATA=$scratch/local" \
        --stdout "$out/app.log" --stderr "$out/app.log" \
        "$bundle" --args --no-session --behind "$@"
    for _ in $(seq 1 40); do
        pid="$("$mac_tool" discover "$exe" "$bundle_id")" && break
        sleep 0.25
    done
    [[ "$pid" =~ ^[1-9][0-9]*$ ]] || mac_fail "no unique scratch process started"
    owned="$pid"
    wid="$(wait_ui window clean main)" || mac_fail "the scratch document window did not appear"
    [[ "$wid" =~ ^[1-9][0-9]*$ ]] || mac_fail "no verified scratch window"
    echo "Scratch PID $pid, window $wid"
}

ui() { "$mac_tool" "$1" "$exe" "$bundle_id" "$pid" "$document" "${@:2}"; }

wait_ui() {
    local result
    for _ in $(seq 1 40); do
        if result="$(ui "$@" 2>"$scratch/verify.log")"; then
            printf '%s' "$result"
            return 0
        fi
        sleep 0.25
    done
    cat "$scratch/verify.log" >&2
    return 1
}

stop() {
    local status=$?
    if [[ -n "$owned" ]]; then
        if ! "$mac_tool" terminate "$exe" "$bundle_id" "$owned"; then
            echo "Scratch process could not be safely terminated; leaving its bundle in place" >&2
            exit 1
        fi
    fi
    rm -rf "$scratch"
    exit "$status"
}
