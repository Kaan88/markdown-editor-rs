#!/usr/bin/env python3
"""Compares two mdedit builds on the budgets of docs/PLAN.md, in real windows, runs interleaved.

    python3 tools/compare-builds.py EXE_A EXE_B RUNS FILE...    (Linux: under xvfb-run)

For every run and file, each exe (in alternating order) is started three times with --trace:
until it settles (first frame, idle memory), with --type-steps 300 (keystroke to pixels) and with
--scroll-steps 300 (scroll frame). Prints the median over the runs of each figure. Extra mdedit
arguments for every start can be given in MDEDIT_ARGS (split on spaces).
"""
import os
import re
import statistics
import subprocess
import sys

exes = sys.argv[1:3]
runs = int(sys.argv[3])
files = sys.argv[4:]
extra = os.environ.get("MDEDIT_ARGS", "").split()

PATTERNS = {
    "first frame ms": r"first frame after ([\d.]+) ms",
    "idle private MiB": r"memory idle: private ([\d.]+) MiB",
    "keystroke p50 ms": r"keystrokes .*? p50 ([\d.]+) ms",
    "keystroke p95 ms": r"keystrokes .*? p95 ([\d.]+) ms",
    "scroll p50 ms": r"scroll steps .*? p50 ([\d.]+) ms",
    "scroll p95 ms": r"scroll steps .*? p95 ([\d.]+) ms",
}


def start(exe, args, path):
    cmd = [exe, "--no-session", "--trace", "--size", "1280x800", *extra, *args, path]
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=180)
    return p.stderr


results = {}  # (exe index, file, figure) -> [values]
for run in range(runs):
    for path in files:
        order = [0, 1] if run % 2 == 0 else [1, 0]
        for i in order:
            big = os.path.getsize(path) > 10_000_000
            out = start(exes[i], ["--exit-after-ms", "6000" if big else "1500"], path)
            out += start(exes[i], ["--type-steps", "300"], path)
            out += start(exes[i], ["--scroll-steps", "300"], path)
            for name, pat in PATTERNS.items():
                m = re.search(pat, out)
                if m:
                    results.setdefault((i, path, name), []).append(float(m.group(1)))
    print(f"run {run + 1} of {runs} done", file=sys.stderr, flush=True)

names = [os.path.basename(e) + f" ({os.path.basename(os.path.dirname(os.path.abspath(e)))})" for e in exes]
print(f"Medians of {runs} interleaved runs. A = {names[0]}, B = {names[1]}")
for path in files:
    print(f"\n{os.path.basename(path)}")
    print(f"  {'':18} {'A':>9} {'B':>9} {'B-A':>8}   A range / B range")
    for name in PATTERNS:
        a = results.get((0, path, name), [])
        b = results.get((1, path, name), [])
        if not a or not b:
            print(f"  {name:18} missing (A {len(a)}, B {len(b)} values)")
            continue
        ma, mb = statistics.median(a), statistics.median(b)
        print(
            f"  {name:18} {ma:9.2f} {mb:9.2f} {mb - ma:+8.2f}   "
            f"{min(a):.2f}-{max(a):.2f} / {min(b):.2f}-{max(b):.2f}"
        )
