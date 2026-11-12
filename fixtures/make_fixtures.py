"""Writes the fixture files with exact bytes: encodings, line endings and control characters that
an editor must not change. Run from anywhere: python fixtures/make_fixtures.py

The large files are not here (they are never committed); tests/shots.sh makes them under
target/data.
"""

import os

HERE = os.path.dirname(os.path.abspath(__file__))

SAMPLE_RS = '''//! A small ring buffer, used as a sample file for the editor's screenshots.

use std::collections::HashMap;
use std::fmt;

/// How many items a ring holds unless told otherwise.
pub const DEFAULT_CAPACITY: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct Ring<T> {
    items: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> Ring<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "a ring needs room for one item");
        Ring {
            items: vec![None; capacity],
            head: 0,
            len: 0,
        }
    }

    /// Adds an item, dropping the oldest when the ring is full.
    pub fn push(&mut self, item: T) -> Option<T> {
        let cap = self.items.len();
        let slot = (self.head + self.len) % cap;
        let old = self.items[slot].replace(item);
        if self.len == cap {
            self.head = (self.head + 1) % cap;
            old
        } else {
            self.len += 1;
            None
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let item = self.items[self.head].take();
        self.head = (self.head + 1) % self.items.len();
        self.len -= 1;
        item
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        (0..self.len).filter_map(move |i| {
            let at = (self.head + i) % self.items.len();
            self.items[at].as_ref()
        })
    }
}

impl<T: fmt::Display + Clone> fmt::Display for Ring<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.iter().map(|x| x.to_string()).collect();
        write!(f, "[{}]", parts.join(", "))
    }
}

/// Counts words, case-insensitively, keeping the `limit` most recent in a ring.
pub fn word_counts(text: &str, limit: usize) -> (HashMap<String, u32>, Ring<String>) {
    let mut counts = HashMap::new();
    let mut recent = Ring::new(limit.max(1));
    for word in text.split_whitespace() {
        let w = word
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();
        if w.is_empty() {
            continue;
        }
        *counts.entry(w.clone()).or_insert(0) += 1;
        recent.push(w);
    }
    (counts, recent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_pop() {
        let mut r = Ring::new(3);
        assert_eq!(r.push(1), None);
        assert_eq!(r.push(2), None);
        assert_eq!(r.push(3), None);
        assert_eq!(r.push(4), Some(1));
        assert_eq!(r.pop(), Some(2));
        assert_eq!(r.len(), 2);
        assert_eq!(r.to_string(), "[3, 4]");
    }

    #[test]
    fn counts_words() {
        let (counts, recent) = word_counts("The cat; the hat. THE end!", 2);
        assert_eq!(counts["the"], 3);
        assert_eq!(recent.iter().cloned().collect::<Vec<_>>(), ["hat", "end"]);
        let raw = r#"quoted "text" stays"#;
        assert!(raw.contains('"') && 0x1F_u8 == 31 && 2.5e3 > 1.0);
    }
}
'''

SAMPLE_MD = '''# mdedit sample

A Markdown file for the editor's screenshots. The editor shows the *source*, coloured, not the
rendered page; the **viewer** renders it.

## Lists

- First item with `inline code`
- Second item with a [link](https://example.com/docs "Title")
  - Nested item
- [x] A done task
- [ ] An open task

1. One
2. Two
3. Three

## Code

```rust
fn main() {
    println!("Hello, {}!", "world");
}
```

> A quote that runs over more than one line, to show how a long line looks when it does not
> wrap: the editor scrolls horizontally instead.

| Column | Value |
|--------|------:|
| alpha  |     1 |
| beta   |    22 |

Türkçe karakterler: ğüşıöç ĞÜŞİÖÇ, and an emoji: 🙂.

---

Footnote reference[^1].

[^1]: The footnote text.
'''

SAMPLE_TOML = '''[package]
name = "sample"
version = "0.3.1"
edition = "2024"
description = "A manifest for the editor's screenshots"
license = "MIT OR Apache-2.0"
publish = false

[features]
default = ["std"]
std = []
fast = ["dep:memchr"]

[dependencies]
ropey = { version = "=1.6.1", default-features = false, features = ["simd", "cr_lines"] }
memchr = { version = "2.7", optional = true }
unicode-segmentation = "1.12"

[dev-dependencies]
criterion = "0.5"

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
# Keep panics unwinding so a worker thread can fail alone.
panic = "unwind"

[[bench]]
name = "load"
harness = false
'''

SAMPLE_PY = '''"""A small sample for the editor's screenshots: reads a CSV and prints column totals."""

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
'''

TURKISH = (
    "Türkçe örnek metin\n"
    "Pijamalı hasta yağız şoföre çabucak güvendi.\n"
    "ĞÜŞİÖÇ ğüşıöç: büyük ve küçük harfler.\n"
    "İstanbul, İzmir, Iğdır, Muş.\n"
)

CJK = (
    "中文：我能吞下玻璃而不伤身体。\n"
    "日本語：私はガラスを食べられます。それは私を傷つけません。\n"
    "한국어: 나는 유리를 먹을 수 있어요. 그래도 아프지 않아요.\n"
    "Mixed 漢字 and ASCII on one line, かな and カナ.\n"
)

UTF16_TEXT = "Türkçe UTF-16 metin: ğüşıöç ĞÜŞİÖÇ\nEmoji outside the BMP: 🙂 🚀 𝄞\nSon satır.\n"


def write(name, data):
    with open(os.path.join(HERE, name), "wb") as f:
        f.write(data)


def main():
    write("sample.rs", SAMPLE_RS.encode("utf-8"))
    write("sample.md", SAMPLE_MD.encode("utf-8"))
    write("sample.toml", SAMPLE_TOML.encode("utf-8"))
    write("sample.py", SAMPLE_PY.encode("utf-8"))
    write("utf8-bom.txt", b"\xef\xbb\xbf" + "UTF-8 with a byte order mark.\nŞeker ve çay.\n".encode("utf-8"))
    write("utf16le.txt", b"\xff\xfe" + UTF16_TEXT.encode("utf-16-le"))
    write("utf16be.txt", b"\xfe\xff" + UTF16_TEXT.encode("utf-16-be"))
    write("cp1254.txt", ("Windows-1254 metin, BOM yok.\n" + TURKISH).encode("cp1254"))
    write("crlf.txt", b"Windows line endings.\r\nSecond line.\r\nThird line.\r\n")
    write("lf.txt", b"Unix line endings.\nSecond line.\nThird line.\n")
    write("cr.txt", b"Classic Mac line endings.\rSecond line.\rThird line.\r")
    write(
        "mixed.txt",
        b"Mixed line endings: this line ends in CRLF.\r\n"
        b"This one in LF.\n"
        b"This one in CR.\r"
        b"CRLF again.\r\n"
        b"No break at the end.",
    )
    write(
        "tabs-controls.txt",
        b"\tOne tab at the start.\n"
        b"a\tb\tc\td: tabs to stops of four.\n"
        b"\t\tTwo tabs.\n"
        b"NUL here: [\x00], ESC here: [\x1b], DEL here: [\x7f].\n"
        b"Bell [\x07] and form feed [\x0c] are controls too.\n",
    )
    write("turkish.txt", TURKISH.encode("utf-8"))
    write("cjk.txt", CJK.encode("utf-8"))
    write("empty.txt", b"")
    # UTF-16LE with a BOM, then a high surrogate followed by an ASCII letter (unpaired), then a
    # lone low surrogate.
    invalid = b"\xff\xfe" + "Before ".encode("utf-16-le") + b"\x3d\xd8" + "A after a lone high surrogate\n".encode("utf-16-le")
    invalid += b"\x00\xde" + " lone low surrogate\n".encode("utf-16-le")
    write("invalid-utf16.txt", invalid)


if __name__ == "__main__":
    main()
