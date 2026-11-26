#!/usr/bin/env bash
# Run the shell harness against a fake window server/Swift driver. No native UI or keychain.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
temp="$(mktemp -d)"
trap 'rm -rf "$temp"' EXIT
mkdir -p "$temp/bin" "$temp/tests" "$temp/input.app/Contents/MacOS"
cp "$here/mac-window.sh" "$here/mac-typing.sh" "$here/mac-ui.swift" "$temp/tests/"
sed 's|/usr/libexec/PlistBuddy|PlistBuddy|g' "$here/mac-launch.sh" > "$temp/tests/mac-launch.sh"
printf '#!/bin/sh\nexit 0\n' > "$temp/input.app/Contents/MacOS/mdedit"
chmod +x "$temp/input.app/Contents/MacOS/mdedit"
touch "$temp/sample.md"
export PATH="$temp/bin:$PATH" RUNNER_ENVIRONMENT=github-hosted
export MOCK_LOG="$temp/calls" MOCK_DRIVER="$temp/driver"

cat > "$temp/bin/swiftc" <<'EOF'
#!/usr/bin/env bash
set -eu
while [[ "$1" != -o ]]; do shift; done
cp "$MOCK_DRIVER" "$2"
chmod +x "$2"
EOF
cat > "$temp/bin/open" <<'EOF'
#!/usr/bin/env bash
[[ "$1 $2" == "-n -g" && "$*" == *MDEDIT_SETTINGS=*/settings.ini* && "$*" == *"--no-session --behind"* ]] || exit 1
echo open >> "$MOCK_LOG"
EOF
cat > "$temp/driver" <<'EOF'
#!/usr/bin/env bash
set -eu
action="$1"
echo "$action" >> "$MOCK_LOG"
[[ "$action" == quit-title || ( "$2" == */mdedit.app/Contents/MacOS/mdedit && "$3" == com.kaan88.mdedit.uitest.* ) ]]
case "$action" in
    absent) [[ "$MOCK_MODE" != existing ]];;
    discover) [[ "$MOCK_MODE" != no-pid ]] && echo 27;;
    terminate) [[ "$4" == 27 ]];;
    window)
        [[ "$4" == 27 && "$MOCK_MODE" != no-window && -f "$5" ]] || exit 1
        [[ "$6" != dirty || -f "$MOCK_LOG.dirty" ]] || exit 1
        if [[ "$7" == open ]]; then
            [[ "$MOCK_MODE" != no-panel && -f "$MOCK_LOG.panel" ]] && echo 92
        else
            [[ ! -f "$MOCK_LOG.panel" ]] && echo 91
        fi;;
    capture)
        [[ "$4" == 27 ]] || exit 1
        if [[ "$7" == open ]]; then [[ "$8" == 92 && -f "$MOCK_LOG.panel" ]]; else [[ "$8" == 91 ]]; fi
        printf '%s' "$(cat "$MOCK_LOG.modal" 2>/dev/null || true)" > "${9}";;
    keys)
        [[ "$4" == 27 && ( "$8" == 91 || ( "$7" == open && "$8" == 92 ) ) ]] || exit 1
        if [[ "$7" == open && "$9" == 53 ]]; then rm "$MOCK_LOG.panel"; exit 0; fi
        case "$*" in
            *"14:opt"*) printf 'abc é' > "$5";;
            *"123:cmd"*) printf 'XabcY é' > "$5";;
            *"6:cmd"*) printf 'Xabc é' > "$5";;
            *"7:cmd"*) printf 'Xabc éXabc é' > "$5";;
            *"31:cmd"*) [[ "$MOCK_MODE" == no-panel ]] || touch "$MOCK_LOG.panel";;
            *"18:shift"*) touch "$MOCK_LOG.dirty";;
            *"12:cmd"*) [[ "$MOCK_MODE" == no-quit ]] || printf dim > "$MOCK_LOG.modal";;
            *" 53") rm -f "$MOCK_LOG.modal";;
        esac;;
    edit-menu) [[ "$4" == 27 && "$8" == 91 ]];;
    recent) [[ "$4" == 27 && "$8" == 91 && -f "$5" && "$MOCK_MODE" != no-recent ]];;
    quit-title)
        case "$3" in
            open) [[ "$(cat "$2")" == dim ]];;
            closed) [[ ! -s "$2" ]];;
        esac;;
    *) exit 1;;
esac
EOF
for tool in codesign PlistBuddy sleep; do
    printf '#!/bin/sh\nexit 0\n' > "$temp/bin/$tool"
done
printf '#!/bin/sh\nprintf "mock-uuid\\n"\n' > "$temp/bin/uuidgen"
cat > "$temp/bin/ditto" <<'EOF'
#!/bin/sh
cp -R "$1" "$2"
EOF
chmod +x "$temp/bin/"*

window() { bash "$temp/tests/mac-window.sh" "$temp/$MOCK_MODE" "$temp/input.app" "$temp/sample.md"; }
for mode in existing no-pid no-window good; do
    export MOCK_MODE="$mode"
    : > "$MOCK_LOG"
    if [[ "$mode" == good ]]; then
        window
        [[ -f "$temp/good/window.png" ]]
        grep -qx capture "$MOCK_LOG"
    else
        if window; then echo "unsafe launch accepted ($mode)" >&2; exit 1; fi
        [[ ! -e "$temp/$mode/window.png" ]]
        ! grep -Eq '^(keys|capture)$' "$MOCK_LOG"
    fi
    if [[ "$mode" == no-window || "$mode" == good ]]; then
        grep -qx terminate "$MOCK_LOG"
    else
        ! grep -qx terminate "$MOCK_LOG"
    fi
    if [[ "$mode" == existing ]]; then ! grep -qx open "$MOCK_LOG"; fi
done
for mode in no-panel no-recent no-quit typing-good; do
    export MOCK_MODE="$mode"
    : > "$MOCK_LOG"
    rm -f "$MOCK_LOG".{panel,dirty,modal}
    if bash "$temp/tests/mac-typing.sh" "$temp/$mode" "$temp/input.app" > "$temp/typing.log" 2>&1; then
        [[ "$mode" == typing-good ]]
        grep -Fq 'confirmation appeared and was cancelled' "$temp/typing.log"
    else
        case "$mode" in
            no-panel)
                grep -Fq 'the owned Open panel did not appear' "$temp/typing.log"
                [[ ! -e "$temp/$mode/open-panel.png" ]];;
            no-recent)
                grep -qx recent "$MOCK_LOG"
                ! grep -qx quit-title "$MOCK_LOG";;
            no-quit) [[ "$(grep -cx quit-title "$MOCK_LOG")" == 2 ]];;
            *) cat "$temp/typing.log"; exit 1;;
        esac
    fi
done
echo "Mac harness rejects unsafe targets and missing panel, recent bookmark, or Quit confirmation"

# Fail the trust command itself: the EXIT trap must already be armed and must
# delete exactly the generated fingerprint. Outside hosted CI, touch nothing.
sed 's|/usr/libexec/PlistBuddy|PlistBuddy|g; s|/opt/homebrew/opt/openssl@3/bin/openssl /usr/local/opt/openssl@3/bin/openssl|/nonexistent-openssl|' \
    "$here/mac-update.sh" > "$temp/tests/mac-update.sh"
cat > "$temp/bin/ditto" <<'EOF'
#!/bin/sh
if [ "$1" = -c ]; then printf zip > "$5"; else cp -R "$1" "$2"; fi
EOF
cat > "$temp/bin/openssl" <<'EOF'
#!/usr/bin/env bash
if [[ "$1" == x509 ]]; then
    echo 'SHA1 Fingerprint=00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33'
else
    while [[ "$1" != -out ]]; do shift; done
    touch "$2"
fi
EOF
cat > "$temp/bin/sudo" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >> "$MOCK_LOG"
[ "$2" != add-trusted-cert ]
EOF
printf '#!/bin/sh\necho Darwin\n' > "$temp/bin/uname"
printf '#!/bin/sh\necho mock-sha256\n' > "$temp/bin/shasum"
chmod +x "$temp/bin/"*
mkdir -p "$temp/input.app/Contents/Resources"
: > "$MOCK_LOG"
if RUNNER_ENVIRONMENT=self-hosted bash "$temp/tests/mac-update.sh" "$temp/blocked" "$temp/input.app"; then
    echo "update test accepted a self-hosted runner" >&2; exit 1
fi
[[ ! -e "$temp/blocked" && ! -s "$MOCK_LOG" ]]
if bash "$temp/tests/mac-update.sh" "$temp/update" "$temp/input.app"; then
    echo "update test accepted a failed trust command" >&2; exit 1
fi
grep -q '^security add-trusted-cert ' "$MOCK_LOG"
grep -q '^security remove-trusted-cert ' "$MOCK_LOG"
grep -Fxq 'security delete-certificate -Z 00112233445566778899AABBCCDDEEFF00112233 /Library/Keychains/System.keychain' "$MOCK_LOG"
echo "Mac updater blocks unhosted runs and cleans up by fingerprint even when adding trust fails"
