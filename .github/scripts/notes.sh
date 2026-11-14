#!/usr/bin/env bash
# Writes the two texts of a release into DIR: notes-line.txt (the manifest's one line, which the
# in-app update bar shows) and release-notes.md (the GitHub release text). From
# docs/release-notes/<version>.md when it exists (the first non-empty line, stripped of leading #
# and spaces, is the manifest line; the whole file is the release text), else a default line.
#   .github/scripts/notes.sh VERSION DIR
set -euo pipefail
version="${1:?version}" dir="${2:?dir}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
app="$(basename "$(ls "$root"/packaging/windows/*.iss | head -1)" .iss)"
mkdir -p "$dir"
file="$root/docs/release-notes/$version.md"
if [ -f "$file" ]; then
    line="$(grep -m1 -v '^[[:space:]]*$' "$file" | sed 's/^[#[:space:]]*//' | tr -d '\000-\037' | head -c 1000)"
    cp "$file" "$dir/release-notes.md"
else
    line=""
fi
[ -n "$line" ] || line="A new version of $app."
printf '%s\n' "$line" > "$dir/notes-line.txt"
[ -f "$dir/release-notes.md" ] || printf '%s %s\n' "$app" "$version" > "$dir/release-notes.md"
echo "notes line: $line"
