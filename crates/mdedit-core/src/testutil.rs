//! Helpers shared by the tests of `buffer`, `history` and `edit`: a small deterministic random
//! generator, and checks written with plain `str` code so they do not depend on the rope.

use crate::buffer::Change;
use crate::eol::EolCounts;

/// xorshift64*: deterministic, no dependencies.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..n` (`n > 0`).
    pub fn below(&mut self, n: usize) -> usize {
        ((self.next() >> 11) % n as u64) as usize
    }

    pub fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// The lines of `text` as the buffer sees them, each with its break: LF, CRLF and CR end a line,
/// and the text after the last break (possibly empty) is the last line.
pub fn lines_of(text: &str) -> Vec<&str> {
    let b = text.as_bytes();
    let mut lines = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < b.len() {
        match b[i] {
            b'\n' => {
                i += 1;
                lines.push(&text[start..i]);
                start = i;
            }
            b'\r' => {
                i += if b.get(i + 1) == Some(&b'\n') { 2 } else { 1 };
                lines.push(&text[start..i]);
                start = i;
            }
            _ => i += 1,
        }
    }
    lines.push(&text[start..]);
    lines
}

/// Checks that `c` describes the edit from `before` to `after` as its documentation says.
pub fn check_change(before: &str, after: &str, c: &Change) {
    let ctx = format!("{before:?} -> {after:?}, {c:?}");
    assert!(
        before.is_char_boundary(c.at) && after.is_char_boundary(c.at),
        "{ctx}"
    );
    assert!(
        c.at + c.removed <= before.len() && c.at + c.inserted <= after.len(),
        "{ctx}"
    );
    assert_eq!(&before[..c.at], &after[..c.at], "{ctx}");
    assert_eq!(
        &before[c.at + c.removed..],
        &after[c.at + c.inserted..],
        "{ctx}"
    );

    let (old, new) = (lines_of(before), lines_of(after));
    assert!(c.lines_before >= 1 && c.lines_after >= 1, "{ctx}");
    assert_eq!(
        c.lines_after as isize - c.lines_before as isize,
        new.len() as isize - old.len() as isize,
        "{ctx}"
    );
    assert_eq!(
        c.line_delta(),
        new.len() as isize - old.len() as isize,
        "{ctx}"
    );
    let f = c.first_line;
    assert!(
        f + c.lines_before <= old.len() && f + c.lines_after <= new.len(),
        "{ctx}"
    );
    assert_eq!(old[..f], new[..f], "{ctx}");
    assert_eq!(old[f + c.lines_before..], new[f + c.lines_after..], "{ctx}");
}

/// Checks that `counts` (of `before`) updated with the change's windows are those of `after`.
pub fn check_eols(counts: &mut EolCounts, after: &str, c: &Change) {
    counts.apply(&c.eol_removed, &c.eol_added);
    assert_eq!(*counts, EolCounts::of(after), "{c:?} -> {after:?}");
}
