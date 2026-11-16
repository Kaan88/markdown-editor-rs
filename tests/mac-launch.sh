# Sourced by the mac-*.sh tests. launch <app-bundle-or-exe> [args...]: starts the application
# the way the Mac starts one, through LaunchServices (`open`), so it is a regular app and comes to
# the front; a bare executable is started directly. Sets $pid. stop: ends it.
launch() {
    local target="$1"
    shift
    if [[ "$target" == *.app ]]; then
        local exe
        exe="$(cd "$target/Contents/MacOS" && pwd)/$(defaults read "$(cd "$target" && pwd)/Contents/Info" CFBundleExecutable)"
        open -n "$target" --stdout "$out/app.log" --stderr "$out/app.log" --args "$@"
        pid=""
        for _ in $(seq 1 40); do
            pid="$(pgrep -n -f "^$exe" || true)"
            [ -n "$pid" ] && break
            sleep 0.25
        done
    else
        "$target" "$@" > "$out/app.log" 2>&1 &
        pid=$!
    fi
    echo "started $target as ${pid:-nothing}"
}

# The window server's id of the process's first normal window, once it has one (20 s at most).
window_id() {
    local wid=""
    for _ in $(seq 1 40); do
        wid=$(swift -e 'import CoreGraphics; let pid = Int32(CommandLine.arguments[1])!; let l = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]; for w in l where (w[kCGWindowOwnerPID as String] as? Int32) == pid && (w[kCGWindowLayer as String] as? Int) == 0 { print(w[kCGWindowNumber as String]!); break }' "$pid" 2>/dev/null)
        [ -n "$wid" ] && break
        sleep 0.5
    done
    echo "$wid"
}

stop() {
    kill "$pid" 2>/dev/null
    for _ in $(seq 1 20); do kill -0 "$pid" 2>/dev/null || break; sleep 0.25; done
    echo "--- app log"
    cat "$out/app.log" 2>/dev/null
}
