#!/usr/bin/env bash
# The budgets with the backdrop setting off and on, against the build before the feature:
# first frame and private memory (small file), keystroke to pixels, scroll step.
#   measure.sh <mdedit-before.exe> <mdedit-after.exe> [runs]
# Run from the worktree root in Git Bash on Windows (release builds). Runs are interleaved so a
# busy moment on the machine hits every variant alike. Scratch settings, --no-session.
set -euo pipefail
before="$1"; after="$2"; n="${3:-15}"
export MDEDIT_SETTINGS="$(mktemp -d)/settings.ini"
out="$(mktemp -d)"
names=(before after-off after-mica after-acrylic)
exes=("$before" "$after" "$after" "$after")
prefs=("" "--pref backdrop=off" "--pref backdrop=mica" "--pref backdrop=acrylic")
small=fixtures/sample.rs
big=target/data/50k.rs
for run in $(seq 1 "$n"); do
    for i in "${!names[@]}"; do
        "${exes[$i]}" --no-session --dark --files --trace --exit-after-ms 500 ${prefs[$i]} "$small" 2>"$out/start-${names[$i]}-$run.txt" || true
    done
done
for run in 1 2 3; do
    for i in "${!names[@]}"; do
        "${exes[$i]}" --no-session --dark --files --trace --type-steps 300 --exit-after-ms 4000 ${prefs[$i]} "$small" 2>"$out/type-${names[$i]}-$run.txt" || true
        "${exes[$i]}" --no-session --dark --files --trace --scroll-steps 300 --exit-after-ms 4000 ${prefs[$i]} "$big" 2>"$out/scroll-${names[$i]}-$run.txt" || true
    done
done
stat() { sort -n | awk '{a[NR]=$1} END {printf "median %.1f  min %.1f  max %.1f  (n=%d)", a[int((NR+1)/2)], a[1], a[NR], NR}'; }
for name in "${names[@]}"; do
    echo "== $name"
    echo -n "  first frame, ms:      "; sed -n 's/.*first frame after \([0-9.]*\) ms.*/\1/p' "$out"/start-"$name"-*.txt | stat; echo
    echo -n "  private, MiB:         "; sed -n 's/.*memory first frame: private \([0-9.]*\) MiB.*/\1/p' "$out"/start-"$name"-*.txt | stat; echo
    echo -n "  private idle, MiB:    "; sed -n 's/.*memory idle: private \([0-9.]*\) MiB.*/\1/p' "$out"/start-"$name"-*.txt | stat; echo
    grep -h "backdrop" "$out"/start-"$name"-1.txt | sed 's/^/  /' || true
    grep -h "keystrokes at" "$out"/type-"$name"-*.txt | sed 's/^mdedit: /  /'
    grep -h "scroll steps at" "$out"/scroll-"$name"-*.txt | sed 's/^mdedit: /  /'
done
