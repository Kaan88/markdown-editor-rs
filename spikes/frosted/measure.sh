#!/usr/bin/env bash
# First-frame time and private memory of the spike window, N runs per variant, interleaved so a
# busy moment on the machine hits every variant alike. Then the programmatic resize test.
#   ./measure.sh [runs]
# Times are from the process's own start of `main` (as the app's trace), not from CreateProcess.
EXE=${EXE:-/c/Users/kaank/Desktop/md-wt/target-frosted/release/frosted-spike.exe}
N=${1:-10}
OUT=$(dirname "$0")/logs/measure
mkdir -p "$OUT"
names=(opaque mica alt acrylic mica-extend)
args=("--opaque" "--blur --backdrop mica" "--blur --backdrop alt" "--blur --backdrop acrylic" "--extend --backdrop mica")
for run in $(seq 1 "$N"); do
  for i in "${!names[@]}"; do
    "$EXE" --dark --pos 240,170 --auto-exit-ms 1200 --log "$OUT/${names[$i]}-$run.txt" ${args[$i]} 2>/dev/null
  done
done
stat() { sort -n | awk '{a[NR]=$1; s+=$1} END {printf "median %.2f  min %.2f  max %.2f  mean %.2f  (n=%d)", a[int((NR+1)/2)], a[1], a[NR], s/NR, NR}'; }
for n in "${names[@]}"; do
  echo "== $n"
  echo -n "  first frame presented, ms:   "; grep -h "first frame (window shown" "$OUT/$n"-[0-9]*.txt | sed 's/^\[ *\([0-9.]*\) ms.*/\1/' | stat; echo
  echo -n "  + one DWM composition, ms:   "; grep -h "DwmFlush returned" "$OUT/$n"-[0-9]*.txt | sed 's/^\[ *\([0-9.]*\) ms.*/\1/' | stat; echo
  echo -n "  private at first frame, MiB: "; grep -h "memory at first frame" "$OUT/$n"-[0-9]*.txt | sed 's/.*private \([0-9.]*\) MiB.*/\1/' | stat; echo
  echo -n "  private after 1.2 s, MiB:    "; grep -h "memory at exit" "$OUT/$n"-[0-9]*.txt | sed 's/.*private \([0-9.]*\) MiB.*/\1/' | stat; echo
  echo -n "  working set after 1.2 s, MiB:"; grep -h "memory at exit" "$OUT/$n"-[0-9]*.txt | sed 's/.*working set \([0-9.]*\) MiB.*/\1/' | stat; echo
done
echo "== resize test (60 programmatic size changes, 3 runs each)"
for i in "${!names[@]}"; do
  for run in 1 2 3; do
    "$EXE" --dark --pos 240,170 --resize-test --auto-exit-ms 300 --log "$OUT/resize-${names[$i]}-$run.txt" ${args[$i]} 2>/dev/null
    echo "  ${names[$i]}: $(grep -h 'resize test' "$OUT/resize-${names[$i]}-$run.txt" | sed 's/.*resize test: //')"
  done
done
