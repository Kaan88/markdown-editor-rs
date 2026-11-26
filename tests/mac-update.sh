#!/usr/bin/env bash
# The whole self-update on macOS, on a runner: an installed copy of the bundle is offered version
# 99.0.0 by a manifest served over HTTPS from this machine (a throwaway certificate the system is
# told to trust), installs it with `--install-update` (what the update bar's Update and Restart
# buttons do), and the new copy must be in place, the old one set aside, and the new one started.
#
#   tests/mac-update.sh <out-dir> <app-bundle>
set -euo pipefail
out="$1"
src="$2"
name="$(basename "$src" .app)"
upper="$(echo "$name" | tr '[:lower:]' '[:upper:]')"
rm -rf "$out"
mkdir -p "$out/apps" "$out/new" "$out/srv"
out="$(cd "$out" && pwd)"
mkdir -p "$out/settings"
settings="$out/settings/settings.ini"
port=8443
fail() { echo "FAIL $*"; exit 1; }

# The installed copy, in a folder this user can write (as /Applications is), and the newer copy:
# the same program with a marker, signed again (a changed bundle must be).
ditto "$src" "$out/apps/$name.app"
ditto "$src" "$out/new/$name.app"
echo "the update" > "$out/new/$name.app/Contents/Resources/update-marker"
# LaunchServices starts the new bundle with only --restarted. Its own Info.plist
# supplies the scratch path without changing the runner's launchd environment.
plist="$out/new/$name.app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :LSEnvironment dict' "$plist"
/usr/libexec/PlistBuddy -c "Add :LSEnvironment:MDEDIT_SETTINGS string \"$settings\"" "$plist"
codesign --force --sign - "$out/new/$name.app"
(cd "$out/new" && ditto -c -k --keepParent "$name.app" "$out/srv/$name-new.zip")
case "$(uname -m)" in arm64) key=macos-aarch64 ;; *) key=macos-x86_64 ;; esac
sha="$(shasum -a 256 "$out/srv/$name-new.zip" | cut -d' ' -f1)"
cat > "$out/srv/update.txt" <<EOF
manifest: 1
version: 99.0.0
notes: The update test.
$key.asset: $name-new.zip
$key.url: https://localhost:$port/$name-new.zip
$key.sha256: $sha
EOF

# A certificate for localhost the system trusts, and a server.
openssl=openssl
for p in /opt/homebrew/opt/openssl@3/bin/openssl /usr/local/opt/openssl@3/bin/openssl; do
    [ -x "$p" ] && openssl="$p" && break
done
"$openssl" req -x509 -newkey rsa:2048 -nodes -days 2 -keyout "$out/key.pem" -out "$out/cert.pem" \
    -subj /CN=localhost -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" \
    -addext "extendedKeyUsage=serverAuth" -addext "basicConstraints=critical,CA:true" 2>/dev/null
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain "$out/cert.pem"
cat > "$out/serve.py" <<'PY'
import http.server, ssl, sys
root, cert, key, port = sys.argv[1:5]
class Quiet(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=root, **k)
srv = http.server.ThreadingHTTPServer(("127.0.0.1", int(port)), Quiet)
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain(cert, key)
srv.socket = ctx.wrap_socket(srv.socket, server_side=True)
srv.serve_forever()
PY
python3 "$out/serve.py" "$out/srv" "$out/cert.pem" "$out/key.pem" "$port" > "$out/server.log" 2>&1 &
server=$!
pid=""
cleanup() {
    [ -z "$pid" ] || kill "$pid" 2>/dev/null || true
    kill "$server" 2>/dev/null || true
}
trap cleanup EXIT
for _ in $(seq 1 40); do curl -s -o /dev/null "https://localhost:$port/update.txt" && break; sleep 0.25; done
curl -sS "https://localhost:$port/update.txt" > /dev/null || fail "the system does not trust the test server"

# The installation, from the installed copy.
exe="$out/apps/$name.app/Contents/MacOS/$name"
code=0
env MDEDIT_SETTINGS="$settings" "${upper}_UPDATE_MANIFEST=https://localhost:$port/update.txt" "$exe" --install-update \
    > "$out/install.log" 2>&1 || code=$?
cat "$out/install.log"
[ "$code" = 0 ] || fail "--install-update exited with $code"
[ -f "$out/apps/$name.app/Contents/Resources/update-marker" ] || fail "the bundle was not replaced"
[ -d "$out/apps/$name.app.old" ] || fail "the old copy was not set aside"
[ ! -e "$out/apps/$name.app.new" ] || fail "the staged copy was left behind"
codesign --verify --deep --strict "$out/apps/$name.app" || fail "the new bundle's signature does not hold"
[ "$(/usr/libexec/PlistBuddy -c 'Print :LSEnvironment:MDEDIT_SETTINGS' "$out/apps/$name.app/Contents/Info.plist")" = "$settings" ] \
    || fail "the installed bundle lost the scratch settings path"
for f in "$out/apps/$name.app/Contents/MacOS/"*; do
    [ -x "$f" ] || fail "$f lost its executable bit"
done
echo "PASS the update replaced the bundle and set the old one aside"

# The restart: a new process of the installed bundle, started through LaunchServices.
for _ in $(seq 1 60); do
    pid="$(pgrep -n -f "^$exe( |\$)" || true)"
    [ -n "$pid" ] && [ -s "$out/settings/instance.port" ] && break
    sleep 0.25
done
[ -n "$pid" ] || fail "the new version was not started"
args="$(ps -o args= -p "$pid")"
[[ "$args" == *" --restarted" ]] || fail "the new version did not start with --restarted"
[ -s "$out/settings/instance.port" ] || fail "the restarted editor did not use scratch settings"
port="$(cat "$out/settings/instance.port")"
/usr/sbin/lsof -a -p "$pid" -iTCP:"$port" -sTCP:LISTEN >/dev/null \
    || fail "the restarted editor does not own the scratch instance port"
echo "PASS the new version restarted and registered beside its scratch settings"
