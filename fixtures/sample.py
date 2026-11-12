"""A small sample for the editor's screenshots: reads a CSV and prints column totals."""

import csv
import sys
from collections import defaultdict
from pathlib import Path

LIMIT = 1_000


class Totals:
    def __init__(self, columns):
        self.columns = list(columns)
        self.sums = defaultdict(float)
        self.rows = 0

    def add(self, row):
        for name in self.columns:
            value = row.get(name, "").strip()
            try:
                self.sums[name] += float(value)
            except ValueError:
                continue
        self.rows += 1

    def __str__(self):
        parts = [f"{name}={self.sums[name]:.2f}" for name in self.columns]
        return f"{self.rows} rows: " + ", ".join(parts)


def main(argv):
    if len(argv) < 2:
        print("usage: totals.py FILE.csv", file=sys.stderr)
        return 2
    path = Path(argv[1])
    with path.open(newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        totals = Totals(reader.fieldnames or [])
        for i, row in enumerate(reader):
            if i >= LIMIT:
                break
            totals.add(row)
    print(totals)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
