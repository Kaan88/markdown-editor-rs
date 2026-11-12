//! Literal find over the rope, walking its chunks without copying the buffer. Case-insensitive by
//! default with the viewer's folding (`mdview-core/src/find.rs`, `fold` and `chars_match`: simple
//! one-to-one lower-casing, U+00A0 read as a space, and the Turkish i forms kept apart so that `i`
//! matches `I` and `İ`, and `I` matches `i` and `ı`), or exact with `match_case`. A line break in the query (CRLF, LF or CR) matches any one line
//! break in the text. Matches never overlap and are reported in buffer order.

use crate::limits::FIND_WORK_PER_BYTE;
use ropey::{Rope, RopeSlice};
use std::iter::Peekable;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FindOptions {
    pub match_case: bool,
}

/// One element of a prepared query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
    /// A character, folded unless the query is case sensitive.
    Char(char),
    /// A line break (CRLF, LF or CR in the query), matching any one break in the text.
    Break,
}

/// How candidate start positions are found inside a chunk.
#[derive(Clone, Copy, Debug)]
enum Skip {
    /// The text byte is one of these two.
    Bytes2(u8, u8),
    /// The text byte is one of these three.
    Bytes3(u8, u8, u8),
    /// The text byte is this one.
    Byte(u8),
    /// The match starts with a non-ASCII character.
    NonAscii,
}

/// A prepared query.
#[derive(Clone, Debug)]
pub struct Query {
    text: String,
    options: FindOptions,
    items: Vec<Item>,
    skip: Skip,
}

impl Query {
    /// `None` for an empty query.
    pub fn new(text: &str, options: FindOptions) -> Option<Query> {
        let mut items = Vec::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    items.push(Item::Break);
                }
                '\n' => items.push(Item::Break),
                _ if options.match_case => items.push(Item::Char(c)),
                _ => items.push(Item::Char(fold(c))),
            }
        }
        let skip = skip_for(*items.first()?, options.match_case);
        Some(Query {
            text: text.to_string(),
            options,
            items,
            skip,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn options(&self) -> FindOptions {
        self.options
    }
}

/// The candidate filter for a query that starts with `first`, when one is safe.
fn skip_for(first: Item, match_case: bool) -> Skip {
    let c = match first {
        Item::Break => return Skip::Bytes2(b'\n', b'\r'),
        Item::Char(c) => c,
    };
    if match_case {
        let mut buf = [0u8; 4];
        return Skip::Byte(c.encode_utf8(&mut buf).as_bytes()[0]);
    }
    // A space also matches U+00A0, whose UTF-8 starts with 0xC2.
    if c == ' ' {
        return Skip::Bytes2(b' ', 0xC2);
    }
    if c.is_ascii() {
        // Letters with non-ASCII case partners also start with the partner's first byte: the
        // Turkish dotted and dotless i (C4 B0, C4 B1), the Kelvin sign (E2 84 AA) and the long s
        // (C5 BF).
        return match c {
            'i' | 'I' => Skip::Bytes3(b'i', b'I', 0xC4),
            'k' | 'K' => Skip::Bytes3(b'k', b'K', 0xE2),
            's' | 'S' => Skip::Bytes3(b's', b'S', 0xC5),
            _ => Skip::Bytes2(c as u8, c.to_ascii_uppercase() as u8),
        };
    }
    // A non-ASCII folded letter matches only non-ASCII text, except the dotless and dotted i,
    // which also match ASCII `I` and `i`.
    match c {
        'ı' => Skip::Bytes3(0xC4, b'I', b'I'),
        'İ' => Skip::Bytes3(0xC4, b'i', b'i'),
        _ => Skip::NonAscii,
    }
}

/// Simple case folding, leaving the four Turkish i forms distinct so `chars_match` can compare
/// them. A non-breaking space reads as a space, since nobody can tell them apart when typing a
/// query.
fn fold(c: char) -> char {
    if c.is_ascii() {
        return if c == 'I' {
            'I'
        } else {
            c.to_ascii_lowercase()
        };
    }
    match c {
        'İ' | 'ı' => c,
        'ς' => 'σ',
        'ſ' => 's',
        '\u{A0}' => ' ',
        _ => {
            let mut lower = c.to_lowercase();
            match (lower.next(), lower.next()) {
                (Some(l), None) => l,
                _ => c,
            }
        }
    }
}

/// Two letters match when they are case partners under either root or Turkish casing: I/i in
/// English, I/ı and İ/i in Turkish. The relation is deliberately not transitive, so `ı` does not
/// find `i`.
fn chars_match(a: char, b: char) -> bool {
    a == b
        || matches!(
            (a, b),
            ('i', 'I') | ('I', 'i') | ('i', 'İ') | ('İ', 'i') | ('I', 'ı') | ('ı', 'I')
        )
}

fn utf8_len(b: u8) -> usize {
    match b {
        0..=0xBF => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// The result of trying to match the query at one position.
enum Outcome {
    /// Matched; this many bytes of text.
    Match(usize),
    /// Did not match; this many characters were compared.
    Fail(usize),
    /// The text ran out first (only when it was not known to be the end of the buffer).
    Exhausted,
}

/// Matches `query` against the characters of `it`. `last` says `it` runs to the end of the
/// buffer, so running out of text is a failure rather than a request for more.
fn match_iter<I: Iterator<Item = char>>(
    it: &mut Peekable<I>,
    query: &Query,
    last: bool,
) -> Outcome {
    let mut bytes = 0;
    for (n, item) in query.items.iter().enumerate() {
        let Some(c) = it.next() else {
            return if last {
                Outcome::Fail(n)
            } else {
                Outcome::Exhausted
            };
        };
        match *item {
            Item::Break => match c {
                '\n' => bytes += 1,
                '\r' => match it.peek() {
                    Some('\n') => {
                        it.next();
                        bytes += 2;
                    }
                    Some(_) => bytes += 1,
                    None => {
                        if last {
                            bytes += 1;
                        } else {
                            return Outcome::Exhausted;
                        }
                    }
                },
                _ => return Outcome::Fail(n + 1),
            },
            Item::Char(q) => {
                let ok = if query.options.match_case {
                    c == q
                } else {
                    chars_match(q, fold(c))
                };
                if !ok {
                    return Outcome::Fail(n + 1);
                }
                bytes += c.len_utf8();
            }
        }
    }
    Outcome::Match(bytes)
}

/// Why a walk stopped before the end of the text.
enum Stop {
    Cancelled,
    /// `FIND_WORK_PER_BYTE` was spent.
    Budget,
}

/// The first candidate start at or after `from` in `s`.
fn next_candidate(skip: Skip, s: &str, from: usize) -> Option<usize> {
    let rest = &s.as_bytes()[from..];
    match skip {
        Skip::Bytes2(a, b) => memchr::memchr2(a, b, rest).map(|i| from + i),
        Skip::Byte(a) => memchr::memchr(a, rest).map(|i| from + i),
        Skip::NonAscii => rest.iter().position(|&b| b >= 0xC0).map(|i| from + i),
        Skip::Bytes3(a, b, c) => memchr::memchr3(a, b, c, rest).map(|i| from + i),
    }
}

const CANCEL_STEP: usize = 64 * 1024;

/// Walks the chunks of `slice` from byte `start` (a char boundary), calling `found` with each
/// non-overlapping match in order until it returns false.
fn walk(
    slice: RopeSlice<'_>,
    query: &Query,
    start: usize,
    cancel: &AtomicBool,
    mut found: impl FnMut(Range<usize>) -> bool,
) -> Result<(), Stop> {
    let len = slice.len_bytes();
    let budget = len.saturating_mul(FIND_WORK_PER_BYTE);
    let mut work = 0usize;
    let mut checked = start;
    if cancel.load(Ordering::Relaxed) {
        return Err(Stop::Cancelled);
    }
    let mut pos = start;
    while pos < len {
        let (chunks, chunk_start, _, _) = slice.chunks_at_byte(pos);
        let mut cs = chunk_start;
        let mut restart = false;
        for chunk in chunks {
            if cs >= checked + CANCEL_STEP {
                checked = cs;
                if cancel.load(Ordering::Relaxed) {
                    return Err(Stop::Cancelled);
                }
            }
            let mut local = pos.saturating_sub(cs);
            while let Some(i) = next_candidate(query.skip, chunk, local) {
                let abs = cs + i;
                let mut it = chunk[i..].chars().peekable();
                let outcome = match match_iter(&mut it, query, false) {
                    Outcome::Exhausted => {
                        let mut rest = slice.byte_slice(abs..).chars().peekable();
                        match_iter(&mut rest, query, true)
                    }
                    other => other,
                };
                match outcome {
                    Outcome::Match(n) => {
                        let end = abs + n;
                        if !found(abs..end) {
                            return Ok(());
                        }
                        pos = end;
                        if end > cs + chunk.len() {
                            restart = true;
                            break;
                        }
                        local = end - cs;
                    }
                    Outcome::Fail(compared) => {
                        work += compared;
                        if work > budget {
                            return Err(Stop::Budget);
                        }
                        local = i + utf8_len(chunk.as_bytes()[i]);
                    }
                    Outcome::Exhausted => unreachable!("the last iterator never runs out"),
                }
            }
            if restart {
                break;
            }
            cs += chunk.len();
            pos = cs;
        }
        if !restart {
            break;
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Matches {
    /// Byte ranges in buffer order, at most `cap` of them.
    pub ranges: Vec<Range<usize>>,
    /// A limit stopped the search: `cap` matches, or `FIND_WORK_PER_BYTE` spent. The real count
    /// may be higher.
    pub capped: bool,
    /// The `cancel` flag stopped the search; `ranges` is what was found before.
    pub cancelled: bool,
}

/// Every match from the start, up to `cap`. Checks `cancel` at least every 64 KiB.
pub fn find_all(rope: &Rope, query: &Query, cap: usize, cancel: &AtomicBool) -> Matches {
    let mut out = Matches::default();
    let result = walk(rope.slice(..), query, 0, cancel, |r| {
        if out.ranges.len() >= cap {
            out.capped = true;
            return false;
        }
        out.ranges.push(r);
        true
    });
    match result {
        Ok(()) => {}
        Err(Stop::Cancelled) => out.cancelled = true,
        Err(Stop::Budget) => out.capped = true,
    }
    out
}

/// The first match starting at or after `from` (forward), or the last one ending at or before
/// `from` (backward), wrapping around the buffer's end when `wrap` is set. For F3 past the last
/// counted match of a capped search.
///
/// Backward, the buffer is searched in windows that end at `from` and double in size, so the
/// cost depends on the distance to the match. A window starts at a fresh position, so where two
/// occurrences overlap the one found can differ from the one `find_all` lists.
pub fn find_next(
    rope: &Rope,
    query: &Query,
    from: usize,
    forward: bool,
    wrap: bool,
    cancel: &AtomicBool,
) -> Option<Range<usize>> {
    let len = rope.len_bytes();
    let from = floor_char(rope, from.min(len));
    if forward {
        let mut hit = None;
        let _ = walk(rope.slice(..), query, from, cancel, |r| {
            hit = Some(r);
            false
        });
        if hit.is_some() || !wrap || from == 0 {
            return hit;
        }
        // From the top, up to the starting point: a match must start before `from`.
        let _ = walk(rope.slice(..), query, 0, cancel, |r| {
            if r.start < from {
                hit = Some(r);
            }
            false
        });
        hit
    } else {
        if let Some(r) = last_in(rope, query, 0, from, cancel) {
            return Some(r);
        }
        if wrap && from < len {
            return last_in(rope, query, from, len, cancel);
        }
        None
    }
}

/// The last match inside `lo..hi`, found by searching windows that end at `hi`.
fn last_in(
    rope: &Rope,
    query: &Query,
    lo: usize,
    hi: usize,
    cancel: &AtomicBool,
) -> Option<Range<usize>> {
    let mut window = CANCEL_STEP;
    loop {
        let w = if hi - lo <= window {
            lo
        } else {
            window_start(rope, hi - window)
        };
        let slice = rope.byte_slice(w..hi);
        let mut last = None;
        let result = walk(slice, query, 0, cancel, |r| {
            last = Some(r);
            true
        });
        if let Some(r) = last {
            return Some(r.start + w..r.end + w);
        }
        if result.is_err() || w == lo {
            return None;
        }
        window = window.saturating_mul(2);
    }
}

/// `offset` moved back to a char boundary, and before the LF of a CRLF.
fn window_start(rope: &Rope, offset: usize) -> usize {
    let mut o = floor_char(rope, offset);
    if o > 0 && rope.byte(o) == b'\n' && rope.byte(o - 1) == b'\r' {
        o -= 1;
    }
    o
}

fn floor_char(rope: &Rope, offset: usize) -> usize {
    if offset >= rope.len_bytes() {
        return rope.len_bytes();
    }
    rope.char_to_byte(rope.byte_to_char(offset))
}

/// Index of the first match in `ranges` that starts at or after `offset` (the one F3 goes to
/// from the caret), or `ranges.len()`.
pub fn first_at_or_after(ranges: &[Range<usize>], offset: usize) -> usize {
    ranges.partition_point(|r| r.start < offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::RopeBuilder;

    const NO: fn() -> AtomicBool = || AtomicBool::new(false);

    fn q(text: &str) -> Query {
        Query::new(text, FindOptions::default()).unwrap()
    }

    fn qc(text: &str) -> Query {
        Query::new(text, FindOptions { match_case: true }).unwrap()
    }

    fn all(text: &str, query: &Query) -> Vec<Range<usize>> {
        find_all(&Rope::from_str(text), query, 10_000, &NO()).ranges
    }

    fn texts(text: &str, query: &Query) -> Vec<String> {
        all(text, query)
            .into_iter()
            .map(|r| text[r].to_string())
            .collect()
    }

    #[test]
    fn empty_query_is_none() {
        assert!(Query::new("", FindOptions::default()).is_none());
        let query = q("Ab");
        assert_eq!(query.text(), "Ab");
        assert!(!query.options().match_case);
    }

    #[test]
    fn case_folding() {
        assert_eq!(texts("Hello HELLO hello", &q("hello")).len(), 3);
        assert_eq!(texts("Hello HELLO hello", &qc("hello")), ["hello"]);
        assert_eq!(texts("Hello HELLO hello", &qc("HELLO")), ["HELLO"]);
        // Greek final sigma, accented capitals.
        assert_eq!(texts("ΟΔΟΣ οδος ΟΔΟς", &q("οδοσ")).len(), 3);
        assert_eq!(texts("ÇAĞ çağ", &q("çağ")).len(), 2);
    }

    #[test]
    fn turkish_i_forms() {
        // i finds i, I and İ; I finds I, i and ı; ı finds ı and I; İ finds İ and i. `ı` does
        // not find `i`: the relation is not transitive.
        assert_eq!(texts("i I İ ı", &q("i")), ["i", "I", "İ"]);
        assert_eq!(texts("i I İ ı", &q("I")), ["i", "I", "ı"]);
        assert_eq!(texts("i I İ ı", &q("ı")), ["I", "ı"]);
        assert_eq!(texts("i I İ ı", &q("İ")), ["i", "İ"]);
        assert_eq!(
            texts("ISPARTA ısparta İstanbul", &q("ısparta")),
            ["ISPARTA", "ısparta"]
        );
        // Match case compares exactly.
        assert_eq!(texts("i I İ ı", &qc("i")), ["i"]);
        assert_eq!(texts("i I İ ı", &qc("İ")), ["İ"]);
        assert_eq!(texts("i I İ ı", &qc("ı")), ["ı"]);
    }

    #[test]
    fn kelvin_long_s_and_nbsp() {
        assert_eq!(texts("k K \u{212A}", &q("k")).len(), 3);
        assert_eq!(texts("k K \u{212A}", &qc("k")), ["k"]);
        assert_eq!(texts("s S \u{17F}", &q("s")).len(), 3);
        assert_eq!(texts("ſ", &q("S")).len(), 1);
        // A non-breaking space reads as a space, both ways.
        assert_eq!(texts("a\u{A0}b a b", &q("a b")).len(), 2);
        assert_eq!(texts("a\u{A0}b a b", &q("a\u{A0}b")).len(), 2);
        assert_eq!(texts("a\u{A0}b a b", &qc("a b")), ["a b"]);
        assert_eq!(texts("a\u{A0}b a b", &qc("a\u{A0}b")), ["a\u{A0}b"]);
        // A query starting with a space also finds NBSP.
        assert_eq!(texts("x\u{A0}y z", &q(" ")).len(), 2);
    }

    #[test]
    fn non_overlapping() {
        assert_eq!(all("aaaa", &q("aa")), vec![0..2, 2..4]);
        assert_eq!(all("aaaaa", &q("aa")), vec![0..2, 2..4]);
        assert_eq!(all("abababa", &q("aba")), vec![0..3, 4..7]);
        assert_eq!(all("", &q("a")), Vec::<Range<usize>>::new());
        assert_eq!(all("a", &q("ab")), Vec::<Range<usize>>::new());
    }

    #[test]
    fn line_break_queries() {
        for text in ["one\ntwo", "one\r\ntwo", "one\rtwo"] {
            for query in ["one\ntwo", "one\r\ntwo", "one\rtwo"] {
                let found = all(text, &q(query));
                assert_eq!(found, vec![0..text.len()], "{text:?} / {query:?}");
                assert_eq!(all(text, &qc(query)), vec![0..text.len()]);
            }
        }
        // CRLF in the query is one break: it does not match two.
        assert!(all("one\n\ntwo", &q("one\r\ntwo")).is_empty());
        assert_eq!(all("a\n\nb", &q("\n\n")), vec![1..3]);
        assert_eq!(all("a\r\n\r\nb", &q("\n\n")), vec![1..5]);
        // A bare break query finds every break.
        assert_eq!(all("a\nb\r\nc\rd", &q("\n")), vec![1..2, 3..5, 6..7]);
        // CR at the end of the text, and a lone CR before another char.
        assert_eq!(all("a\r", &q("\n")), vec![1..2]);
        assert_eq!(all("a\rb", &q("\r")), vec![1..2]);
        // A break does not match a letter.
        assert!(all("ab", &q("a\nb")).is_empty());
    }

    fn many_chunks(text: &str) -> Rope {
        let mut b = RopeBuilder::new();
        let mut start = 0;
        while start < text.len() {
            let mut end = (start + 500).min(text.len());
            while !text.is_char_boundary(end) {
                end += 1;
            }
            b.append(&text[start..end]);
            start = end;
        }
        let rope = b.finish();
        assert!(rope.chunks().count() > 3 || text.len() < 3000);
        rope
    }

    #[test]
    fn matches_cross_chunk_boundaries() {
        // A needle placed across every chunk boundary of a rope of many chunks.
        let filler = "x".repeat(40_000);
        let rope0 = many_chunks(&filler);
        let boundaries: Vec<usize> = {
            let mut v = Vec::new();
            let mut at = 0;
            for c in rope0.chunks() {
                at += c.len();
                v.push(at);
            }
            v.pop();
            v
        };
        assert!(boundaries.len() > 20);
        let mut text: Vec<u8> = filler.into_bytes();
        let mut expect = Vec::new();
        for (k, &b) in boundaries.iter().enumerate() {
            if k % 2 == 0 {
                text[b - 2..b + 2].copy_from_slice(b"NEED");
                expect.push(b - 2..b + 2);
            }
        }
        let text = String::from_utf8(text).unwrap();
        let rope = many_chunks(&text);
        // The chunking is the same for the same appended pieces.
        let found = find_all(&rope, &q("need"), 10_000, &NO());
        assert_eq!(found.ranges, expect);
        assert!(!found.capped && !found.cancelled);
        let found = find_all(&rope, &qc("NEED"), 10_000, &NO());
        assert_eq!(found.ranges, expect);
        assert!(
            find_all(&rope, &qc("need"), 10_000, &NO())
                .ranges
                .is_empty()
        );
    }

    #[test]
    fn multibyte_and_crlf_across_chunks() {
        let mut text = String::new();
        let mut expect = Vec::new();
        for i in 0..3000 {
            if i % 97 == 0 {
                let start = text.len();
                text.push_str("ĞÜŞ\r\nİıx");
                expect.push(start..text.len());
            } else {
                text.push_str("ab\r\n");
            }
        }
        let rope = many_chunks(&text);
        let found = find_all(&rope, &q("ğüş\niıx"), 10_000, &NO());
        assert_eq!(found.ranges, expect);
        let found = find_all(&rope, &q("ğüş\nİıx"), 10_000, &NO());
        assert_eq!(found.ranges, expect);
    }

    #[test]
    fn cap_and_cancel() {
        let text = "ab ".repeat(100);
        let rope = Rope::from_str(&text);
        let m = find_all(&rope, &q("ab"), 10, &NO());
        assert_eq!(m.ranges.len(), 10);
        assert!(m.capped && !m.cancelled);
        let m = find_all(&rope, &q("ab"), 100, &NO());
        assert_eq!(m.ranges.len(), 100);
        assert!(!m.capped);
        let m = find_all(&rope, &q("ab"), 99, &NO());
        assert!(m.capped);
        let m = find_all(&rope, &q("ab"), 0, &NO());
        assert!(m.ranges.is_empty() && m.capped);
        // Cancelled before the start.
        let flag = AtomicBool::new(true);
        let m = find_all(&rope, &q("ab"), 1000, &flag);
        assert!(m.cancelled && m.ranges.is_empty());
        let none = find_next(&rope, &q("ab"), 0, true, true, &flag);
        assert_eq!(none, None);
    }

    #[test]
    fn cancel_during_a_large_search() {
        // The flag is set up front: the search stops before its first chunk.
        let text = "abcdefghij".repeat(30_000);
        let rope = Rope::from_str(&text);
        let flag = AtomicBool::new(true);
        let m = find_all(&rope, &q("j"), 10_000_000, &flag);
        assert!(m.cancelled);
        assert!(m.ranges.len() < 30_000);
    }

    #[test]
    fn cancel_from_another_thread_stops_a_long_search() {
        // About 30 MB where every 'a' is a candidate that fails on the third character, so the
        // search takes long enough for the flag to be set while it runs.
        let text = "ab".repeat(15_000_000);
        let rope = Rope::from_str(&text);
        let query = q("abz");
        let flag = AtomicBool::new(false);
        let (m, elapsed) = std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(5));
                flag.store(true, Ordering::Relaxed);
            });
            let t = std::time::Instant::now();
            let m = find_all(&rope, &query, 10_000, &flag);
            (m, t.elapsed())
        });
        eprintln!("cancelled mid-search: {} after {elapsed:?}", m.cancelled);
        // Either the search was cut short, or it was too fast to catch; it never reports
        // matches that do not exist.
        assert!(m.ranges.is_empty());
        if m.cancelled {
            assert!(elapsed < std::time::Duration::from_secs(5));
        }
        // With the flag already set the answer is always `cancelled`.
        assert!(find_all(&rope, &query, 10_000, &flag).cancelled);
    }

    #[test]
    fn work_budget_caps_a_nearly_matching_text() {
        // Every position matches 63 of 64 query characters.
        let query_text = format!("{}b", "a".repeat(100));
        let text = "a".repeat(20_000);
        let rope = Rope::from_str(&text);
        let m = find_all(&rope, &q(&query_text), 10_000, &NO());
        assert!(m.capped && m.ranges.is_empty());
    }

    #[test]
    fn find_next_forward_and_backward() {
        let text = "ab cd ab cd ab";
        let rope = Rope::from_str(text);
        let query = q("ab");
        let n = |from, fwd, wrap| find_next(&rope, &query, from, fwd, wrap, &NO());
        assert_eq!(n(0, true, false), Some(0..2));
        assert_eq!(n(1, true, false), Some(6..8));
        assert_eq!(n(6, true, false), Some(6..8));
        assert_eq!(n(7, true, false), Some(12..14));
        assert_eq!(n(13, true, false), None);
        assert_eq!(n(13, true, true), Some(0..2));
        assert_eq!(n(14, true, true), Some(0..2));
        assert_eq!(n(1000, true, true), Some(0..2));
        // Wrap finds a match that starts before `from` only.
        assert_eq!(n(0, true, true), Some(0..2));
        // Backward: the last match ending at or before `from`.
        assert_eq!(n(14, false, false), Some(12..14));
        assert_eq!(n(13, false, false), Some(6..8));
        assert_eq!(n(8, false, false), Some(6..8));
        assert_eq!(n(7, false, false), Some(0..2));
        assert_eq!(n(2, false, false), Some(0..2));
        assert_eq!(n(1, false, false), None);
        assert_eq!(n(1, false, true), Some(12..14));
        assert_eq!(n(0, false, true), Some(12..14));
        assert_eq!(n(0, false, false), None);
        // No match at all.
        let none = q("zz");
        assert_eq!(find_next(&rope, &none, 3, true, true, &NO()), None);
        assert_eq!(find_next(&rope, &none, 3, false, true, &NO()), None);
        // Empty rope.
        assert_eq!(find_next(&Rope::new(), &query, 0, true, true, &NO()), None);
        assert_eq!(find_next(&Rope::new(), &query, 0, false, true, &NO()), None);
    }

    #[test]
    fn find_next_backward_in_a_large_buffer() {
        let mut text = "0123456789".repeat(100_000);
        text.replace_range(10..14, "NEED");
        text.replace_range(500_000..500_004, "NEED");
        let rope = Rope::from_str(&text);
        let query = q("need");
        let from = 900_000;
        let r = find_next(&rope, &query, from, false, false, &NO());
        assert_eq!(r, Some(500_000..500_004));
        let r = find_next(&rope, &query, 499_999, false, false, &NO());
        assert_eq!(r, Some(10..14));
        let r = find_next(&rope, &query, 5, false, true, &NO());
        assert_eq!(r, Some(500_000..500_004));
        let r = find_next(&rope, &query, 20, true, false, &NO());
        assert_eq!(r, Some(500_000..500_004));
        // A match that straddles a window start is still found.
        let r = find_next(&rope, &query, 500_004 + 65_536 - 2, false, false, &NO());
        assert_eq!(r, Some(500_000..500_004));
    }

    #[test]
    fn first_at_or_after_is_the_partition_point() {
        let r = vec![2..3, 5..6, 9..10];
        assert_eq!(first_at_or_after(&r, 0), 0);
        assert_eq!(first_at_or_after(&r, 2), 0);
        assert_eq!(first_at_or_after(&r, 3), 1);
        assert_eq!(first_at_or_after(&r, 9), 2);
        assert_eq!(first_at_or_after(&r, 10), 3);
        assert_eq!(first_at_or_after(&[], 1), 0);
    }

    // An independent, naive search over chars, for the property test.
    fn naive(text: &str, query: &str, match_case: bool) -> Vec<Range<usize>> {
        let mut items: Vec<Option<char>> = Vec::new();
        let qc: Vec<char> = query.chars().collect();
        let mut i = 0;
        while i < qc.len() {
            match qc[i] {
                '\r' => {
                    if qc.get(i + 1) == Some(&'\n') {
                        i += 1;
                    }
                    items.push(None);
                }
                '\n' => items.push(None),
                c => items.push(Some(if match_case { c } else { fold(c) })),
            }
            i += 1;
        }
        let chars: Vec<(usize, char)> = text.char_indices().collect();
        let mut out = Vec::new();
        let mut s = 0;
        while s < chars.len() {
            let mut j = s;
            let mut ok = true;
            for item in &items {
                if j >= chars.len() {
                    ok = false;
                    break;
                }
                let c = chars[j].1;
                match item {
                    None => match c {
                        '\n' => j += 1,
                        '\r' => {
                            j += 1;
                            if j < chars.len() && chars[j].1 == '\n' {
                                j += 1;
                            }
                        }
                        _ => {
                            ok = false;
                            break;
                        }
                    },
                    Some(qch) => {
                        let m = if match_case {
                            c == *qch
                        } else {
                            chars_match(*qch, fold(c))
                        };
                        if !m {
                            ok = false;
                            break;
                        }
                        j += 1;
                    }
                }
            }
            if ok {
                let end = if j < chars.len() {
                    chars[j].0
                } else {
                    text.len()
                };
                out.push(chars[s].0..end);
                s = j;
            } else {
                s += 1;
            }
        }
        out
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    #[test]
    fn property_find_all_equals_naive() {
        let ascii: Vec<char> = "abAB cdk\nS_.0\r\t".chars().collect();
        let turkish: Vec<char> = "iIıİğĞüÜşŞöÖçÇ kKsS\u{212A}\u{17F}\u{A0}\n\rσςΣ"
            .chars()
            .collect();
        let mut rng = Rng(0x2545F4914F6CDD1D);
        for round in 0..400 {
            let alphabet = if round % 2 == 0 { &ascii } else { &turkish };
            let tlen = 1 + rng.below(if round % 8 < 2 { 6000 } else { 80 });
            let text: String = (0..tlen)
                .map(|_| alphabet[rng.below(alphabet.len())])
                .collect();
            let qlen = 1 + rng.below(4);
            // Mostly queries cut out of the text itself, so there are matches.
            let query: String = if rng.below(3) > 0 {
                let chars: Vec<char> = text.chars().collect();
                let s = rng.below(chars.len());
                chars[s..(s + qlen).min(chars.len())].iter().collect()
            } else {
                (0..qlen)
                    .map(|_| alphabet[rng.below(alphabet.len())])
                    .collect()
            };
            let rope = many_chunks(&text);
            for match_case in [false, true] {
                let Some(query_obj) = Query::new(&query, FindOptions { match_case }) else {
                    continue;
                };
                let got = find_all(&rope, &query_obj, 1_000_000, &NO());
                let want = naive(&text, &query, match_case);
                assert_eq!(
                    got.ranges, want,
                    "round {round} match_case {match_case} query {query:?} text {text:?}"
                );
                assert!(!got.capped && !got.cancelled);
            }
        }
    }
}
