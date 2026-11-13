//! Literal find over the rope, walking its chunks without copying the buffer. Case-insensitive by
//! default with the viewer's folding (`mdview-core/src/find.rs`, `fold` and `chars_match`: simple
//! one-to-one lower-casing, U+00A0 read as a space, and the Turkish i forms kept apart so that `i`
//! matches `I` and `İ`, and `I` matches `i` and `ı`), or exact with `match_case`. A line break in the query (CRLF, LF or CR) matches any one line
//! break in the text. Matches never overlap and are reported in buffer order.
//!
//! Two options extend the literal search. `whole_word` keeps a match only when the characters
//! just before and after it are not word characters (the class word motions use, see
//! `selection::is_word_char`) or are the buffer's ends or line breaks; a rejected candidate does
//! not hide a later, overlapping one. `regex` reads the query as a regular expression
//! (`regex-automata`'s meta engine, linear time) applied to each line's text without its line
//! break: `^` and `$` are the line's ends, a match never spans lines, empty matches are not
//! reported, and a line over [`REGEX_LINE_MAX`] bytes is skipped. With `match_case` off a regex
//! matches case-insensitively with the engine's own Unicode simple folding; the literal path's
//! special Turkish i folding (and the NBSP-as-space rule) does not apply to it.

use crate::limits::{FIND_WORK_PER_BYTE, REGEX_LINE_MAX, REGEX_SIZE_LIMIT};
use crate::selection::is_word_char;
use regex_automata::meta;
use regex_automata::util::syntax;
use regex_automata::{Anchored, Input};
use ropey::{Rope, RopeSlice};
use std::iter::Peekable;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FindOptions {
    pub match_case: bool,
    /// A match must not touch a word character on either side.
    pub whole_word: bool,
    /// The query is a regular expression, applied per line.
    pub regex: bool,
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
    /// The compiled pattern when `options.regex` is set.
    regex: Option<meta::Regex>,
}

fn build_regex(text: &str, options: FindOptions) -> Result<meta::Regex, Box<meta::BuildError>> {
    meta::Regex::builder()
        .syntax(syntax::Config::new().case_insensitive(!options.match_case))
        .configure(meta::Config::new().nfa_size_limit(Some(REGEX_SIZE_LIMIT)))
        .build(text)
        .map_err(Box::new)
}

impl Query {
    /// `None` for an empty query, and for a regex that does not compile.
    pub fn new(text: &str, options: FindOptions) -> Option<Query> {
        if text.is_empty() {
            return None;
        }
        if options.regex {
            let regex = build_regex(text, options).ok()?;
            return Some(Query {
                text: text.to_string(),
                options,
                items: Vec::new(),
                skip: Skip::NonAscii,
                regex: Some(regex),
            });
        }
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
            regex: None,
        })
    }

    /// Whether `text` is a usable query under `options`: `Ok` for a literal (even an empty one)
    /// and for a regex that compiles, else a short one-line reason for the UI.
    pub fn check(text: &str, options: FindOptions) -> Result<(), String> {
        if !options.regex || text.is_empty() {
            return Ok(());
        }
        match build_regex(text, options) {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.size_limit().is_some() {
                    return Err("the pattern is too large".to_string());
                }
                // The parser's report is several lines; its last one is the reason.
                let full =
                    std::error::Error::source(&*e).map_or_else(|| e.to_string(), |s| s.to_string());
                let line = full
                    .lines()
                    .rev()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .unwrap_or("invalid pattern");
                let line = line.strip_prefix("error: ").unwrap_or(line);
                Err(line.to_string())
            }
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The compiled pattern of a regex query.
    pub(crate) fn regex(&self) -> Option<&meta::Regex> {
        self.regex.as_ref()
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
pub(crate) enum Stop {
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

/// The characters just outside `s..e` of `rope` are not word characters (or there are none).
fn word_bounded(rope: &Rope, s: usize, e: usize) -> bool {
    if s > 0 && is_word_char(rope.char(rope.byte_to_char(s) - 1)) {
        return false;
    }
    let after = rope.byte_to_char(e);
    !(after < rope.len_chars() && is_word_char(rope.char(after)))
}

/// The same for `s..e` of one line's text.
fn line_bounded(line: &str, s: usize, e: usize) -> bool {
    !line[..s].chars().next_back().is_some_and(is_word_char)
        && !line[e..].chars().next().is_some_and(is_word_char)
}

/// Walks the chunks of `slice` (which starts at byte `base` of `full`) from byte `start` of the
/// slice (a char boundary), calling `found` with each non-overlapping match in order until it
/// returns false. Whole-word checks look at `full`, so a slice that cuts a word is judged by
/// what is beyond its ends.
fn walk(
    slice: RopeSlice<'_>,
    base: usize,
    full: &Rope,
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
                        if query.options.whole_word && !word_bounded(full, base + abs, base + end) {
                            // Not a whole word: the next candidate may overlap this one.
                            work += query.items.len();
                            if work > budget {
                                return Err(Stop::Budget);
                            }
                            local = i + utf8_len(chunk.as_bytes()[i]);
                            continue;
                        }
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

/// The matches of the regex in one line's text, in order, until `f` returns false (`Ok(false)`).
/// Empty matches are skipped. A candidate rejected by whole-word does not hide a later one that
/// overlaps it; such rescans are charged against a budget so a pattern cannot make a long line
/// quadratic (`Err(Stop::Budget)`).
fn line_matches(
    re: &meta::Regex,
    cache: &mut meta::Cache,
    line: &str,
    whole_word: bool,
    mut f: impl FnMut(Range<usize>) -> bool,
) -> Result<bool, Stop> {
    let mut budget = line.len().saturating_mul(FIND_WORK_PER_BYTE);
    let mut pos = 0;
    while pos <= line.len() {
        let input = Input::new(line).span(pos..line.len());
        let Some(m) = re.search_with(cache, &input) else {
            break;
        };
        let (s, e) = (m.start(), m.end());
        if s == e {
            match line[e..].chars().next() {
                Some(c) => pos = e + c.len_utf8(),
                None => break,
            }
            continue;
        }
        if whole_word && !line_bounded(line, s, e) {
            let cost = e - s + 1;
            if cost > budget {
                return Err(Stop::Budget);
            }
            budget -= cost;
            pos = s + line[s..].chars().next().map_or(1, char::len_utf8);
            continue;
        }
        if !f(s..e) {
            return Ok(false);
        }
        pos = e;
    }
    Ok(true)
}

/// A line's text without its break.
fn strip_break(line: RopeSlice<'_>) -> RopeSlice<'_> {
    let mut t = line.len_bytes();
    if t > 0 {
        match line.byte(t - 1) {
            b'\n' => {
                t -= 1;
                if t > 0 && line.byte(t - 1) == b'\r' {
                    t -= 1;
                }
            }
            b'\r' => t -= 1,
            _ => {}
        }
    }
    line.byte_slice(..t)
}

/// The text of `slice` as one `&str`: borrowed when one chunk holds it, else copied into `buf`.
fn contiguous<'a>(slice: RopeSlice<'a>, buf: &'a mut String) -> &'a str {
    match slice.as_str() {
        Some(s) => s,
        None => {
            buf.clear();
            for c in slice.chunks() {
                buf.push_str(c);
            }
            buf.as_str()
        }
    }
}

/// The regex counterpart of [`walk`]: the lines from the one holding `from` are searched one at
/// a time, and the matches that start at or after `from` and end at or before `to` are passed to
/// `found` in order until it returns false. `cancel` is checked at the start and every 64 KiB
/// of lines walked.
fn walk_regex(
    rope: &Rope,
    query: &Query,
    from: usize,
    to: usize,
    cancel: &AtomicBool,
    mut found: impl FnMut(Range<usize>) -> bool,
) -> Result<(), Stop> {
    let re = query.regex.as_ref().expect("a regex query");
    if cancel.load(Ordering::Relaxed) {
        return Err(Stop::Cancelled);
    }
    let to = to.min(rope.len_bytes());
    if from >= to {
        return Ok(());
    }
    let mut cache = re.create_cache();
    let mut buf = String::new();
    let first = rope.byte_to_line(from);
    let mut at = rope.line_to_byte(first);
    let mut checked = at;
    let mut capped = false;
    for line in rope.lines_at(first) {
        if at >= to {
            break;
        }
        let n = line.len_bytes();
        if at >= checked + CANCEL_STEP {
            checked = at;
            if cancel.load(Ordering::Relaxed) {
                return Err(Stop::Cancelled);
            }
        }
        let text = strip_break(line);
        if text.len_bytes() <= REGEX_LINE_MAX {
            let s = contiguous(text, &mut buf);
            let base = at;
            let more = line_matches(re, &mut cache, s, query.options.whole_word, |r| {
                let (rs, rend) = (base + r.start, base + r.end);
                rs < from || (rs < to && rend <= to && found(rs..rend))
            });
            match more {
                Ok(true) => {}
                Ok(false) => return Ok(()),
                // Only the pathological line is lost; the walk goes on.
                Err(_) => capped = true,
            }
        }
        at += n;
    }
    if capped { Err(Stop::Budget) } else { Ok(()) }
}

/// The last match inside `lo..hi` (starting at or after `lo`, ending at or before `hi`), found
/// by walking the lines backward from the one holding `hi`.
fn last_regex(
    rope: &Rope,
    query: &Query,
    lo: usize,
    hi: usize,
    cancel: &AtomicBool,
) -> Option<Range<usize>> {
    let re = query.regex.as_ref().expect("a regex query");
    if hi == 0 || lo >= hi || cancel.load(Ordering::Relaxed) {
        return None;
    }
    let mut line = rope.byte_to_line(hi);
    if rope.line_to_byte(line) >= hi {
        line = line.checked_sub(1)?;
    }
    let lowest = rope.byte_to_line(lo);
    let mut cache = re.create_cache();
    let mut buf = String::new();
    let mut walked = 0usize;
    loop {
        let slice = rope.line(line);
        let at = rope.line_to_byte(line);
        let text = strip_break(slice);
        if text.len_bytes() <= REGEX_LINE_MAX {
            let s = contiguous(text, &mut buf);
            let mut last = None;
            let _ = line_matches(re, &mut cache, s, query.options.whole_word, |r| {
                let (rs, rend) = (at + r.start, at + r.end);
                if rs >= lo && rend <= hi {
                    last = Some(rs..rend);
                }
                rend <= hi
            });
            if last.is_some() {
                return last;
            }
        }
        if line == lowest {
            return None;
        }
        line -= 1;
        walked += slice.len_bytes();
        if walked >= CANCEL_STEP {
            walked = 0;
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
        }
    }
}

/// Walks from byte `start` to the end with whichever engine the query uses.
fn walk_any(
    rope: &Rope,
    query: &Query,
    start: usize,
    cancel: &AtomicBool,
    found: impl FnMut(Range<usize>) -> bool,
) -> Result<(), Stop> {
    if query.regex.is_some() {
        walk_regex(rope, query, start, rope.len_bytes(), cancel, found)
    } else {
        walk(rope.slice(..), 0, rope, query, start, cancel, found)
    }
}

/// Every match that lies wholly inside `within`, in order. A plain search starts counting at
/// `within.start` (so overlapping candidates pair up from there); a regex search works on the
/// whole lines that `within` touches.
pub(crate) fn walk_within(
    rope: &Rope,
    query: &Query,
    within: Range<usize>,
    cancel: &AtomicBool,
    mut found: impl FnMut(Range<usize>) -> bool,
) -> Result<(), Stop> {
    if query.regex.is_some() {
        return walk_regex(rope, query, within.start, within.end, cancel, found);
    }
    let slice = rope.byte_slice(within.clone());
    walk(slice, within.start, rope, query, 0, cancel, |r| {
        found(r.start + within.start..r.end + within.start)
    })
}

/// Whether `range` is exactly a match of `query` in `rope`: the query, tried at `range.start`,
/// matches up to `range.end` (and passes the whole-word test, when set). This accepts a match
/// that [`find_all`] skipped for overlapping an earlier one. For re-checking a match before
/// replacing it.
pub(crate) fn is_match(rope: &Rope, query: &Query, range: Range<usize>) -> bool {
    let len = rope.len_bytes();
    if range.start >= range.end || range.end > len {
        return false;
    }
    let boundary = |b: usize| b == len || (rope.byte(b) & 0xC0) != 0x80;
    if !boundary(range.start) || !boundary(range.end) {
        return false;
    }
    if let Some(re) = &query.regex {
        let line = rope.byte_to_line(range.start);
        let at = rope.line_to_byte(line);
        let text = strip_break(rope.line(line));
        if range.end > at + text.len_bytes() || text.len_bytes() > REGEX_LINE_MAX {
            return false;
        }
        let mut buf = String::new();
        let s = contiguous(text, &mut buf);
        let mut cache = re.create_cache();
        let local = range.start - at..range.end - at;
        let input = Input::new(s)
            .span(local.start..s.len())
            .anchored(Anchored::Yes);
        return re
            .search_with(&mut cache, &input)
            .is_some_and(|m| m.end() == local.end)
            && (!query.options.whole_word || line_bounded(s, local.start, local.end));
    }
    let mut it = rope.byte_slice(range.start..).chars().peekable();
    match match_iter(&mut it, query, true) {
        Outcome::Match(n) if n == range.len() => {
            !query.options.whole_word || word_bounded(rope, range.start, range.end)
        }
        _ => false,
    }
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
    let result = walk_any(rope, query, 0, cancel, |r| {
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
        let _ = walk_any(rope, query, from, cancel, |r| {
            hit = Some(r);
            false
        });
        if hit.is_some() || !wrap || from == 0 {
            return hit;
        }
        // From the top, up to the starting point: a match must start before `from`.
        let _ = walk_any(rope, query, 0, cancel, |r| {
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
    if query.regex.is_some() {
        return last_regex(rope, query, lo, hi, cancel);
    }
    let mut window = CANCEL_STEP;
    loop {
        let w = if hi - lo <= window {
            lo
        } else {
            window_start(rope, hi - window)
        };
        let slice = rope.byte_slice(w..hi);
        let mut last = None;
        let result = walk(slice, w, rope, query, 0, cancel, |r| {
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
        Query::new(
            text,
            FindOptions {
                match_case: true,
                ..FindOptions::default()
            },
        )
        .unwrap()
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
    fn naive(text: &str, query: &str, match_case: bool, whole_word: bool) -> Vec<Range<usize>> {
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
            let word = |c: char| c.is_alphanumeric() || c == '_';
            if ok
                && whole_word
                && ((s > 0 && word(chars[s - 1].1)) || (j < chars.len() && word(chars[j].1)))
            {
                ok = false;
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
            for (match_case, whole_word) in
                [(false, false), (true, false), (false, true), (true, true)]
            {
                let options = FindOptions {
                    match_case,
                    whole_word,
                    regex: false,
                };
                let Some(query_obj) = Query::new(&query, options) else {
                    continue;
                };
                let got = find_all(&rope, &query_obj, 1_000_000, &NO());
                let want = naive(&text, &query, match_case, whole_word);
                assert_eq!(
                    got.ranges, want,
                    "round {round} {options:?} query {query:?} text {text:?}"
                );
                assert!(!got.capped && !got.cancelled);
            }
        }
    }

    // ---- whole word

    fn ww(text: &str, match_case: bool) -> Query {
        Query::new(
            text,
            FindOptions {
                match_case,
                whole_word: true,
                regex: false,
            },
        )
        .unwrap()
    }

    #[test]
    fn whole_word_ascii() {
        let text = "cat concat cat. cats _cat cat_ 9cat cat";
        let want = vec![0..3, 11..14, 36..39];
        assert_eq!(all(text, &ww("cat", true)), want);
        assert_eq!(all(&text.to_uppercase(), &ww("cat", false)), want);
        assert!(all(&text.to_uppercase(), &ww("cat", true)).is_empty());
        // A query with its own non-word characters is judged on its outer neighbours only.
        assert_eq!(
            all("a-b a-bc xa-b a-b", &ww("a-b", true)),
            vec![0..3, 14..17]
        );
        assert_eq!(all("(cat)", &ww("cat", true)), vec![1..4]);
    }

    #[test]
    fn whole_word_buffer_and_line_ends() {
        for text in ["cat", "cat\ncat", "cat\r\ncat\rcat\n", "\ncat\n", "cat "] {
            let n = text.matches("cat").count();
            assert_eq!(all(text, &ww("cat", true)).len(), n, "{text:?}");
            assert_eq!(all(text, &ww("CAT", false)).len(), n, "{text:?}");
        }
        assert_eq!(all("a\ncat\nb", &ww("cat", true)), vec![2..5]);
        // A query that is a line break: neighbours are letters.
        assert_eq!(all("a\nb", &ww("\n", true)).len(), 0);
        assert_eq!(all("a \n b", &ww("\n", true)).len(), 1);
    }

    #[test]
    fn whole_word_turkish_and_cjk_neighbours() {
        // Letters of any script are word characters, so these are rejected...
        for text in [
            "çcat", "catç", "漢cat", "cat漢", "İcat", "catı", "ğcatş", "cat٣", "٣cat",
        ] {
            assert!(all(text, &ww("cat", true)).is_empty(), "{text:?}");
        }
        // ...and spaces, punctuation (also CJK punctuation) are not.
        for text in [
            "ç cat",
            "cat 漢",
            "。cat。",
            "「cat」",
            "…cat…",
            "—cat—",
            "\u{A0}cat\u{A0}",
        ] {
            assert_eq!(all(text, &ww("cat", true)).len(), 1, "{text:?}");
        }
        // The Turkish i forms inside a whole-word search.
        assert_eq!(
            texts("ISPARTA ısparta İstanbul ısparta1", &ww("ısparta", false)),
            ["ISPARTA", "ısparta"]
        );
        assert_eq!(texts("i I İ ı", &ww("i", false)), ["i", "I", "İ"]);
        assert_eq!(texts("i I İ ı", &ww("i", true)), ["i"]);
        assert_eq!(
            texts("漢字 漢字漢 漢字", &ww("漢字", true)),
            ["漢字", "漢字"]
        );
        assert_eq!(texts("ÇAĞ çağ çağı", &ww("çağ", false)), ["ÇAĞ", "çağ"]);
    }

    #[test]
    fn whole_word_overlapping_candidates() {
        // Each candidate of "aa" in "aaa" touches a letter.
        assert!(all("aaa", &ww("aa", true)).is_empty());
        assert_eq!(all("aaa aa", &ww("aa", true)), vec![4..6]);
        // The first candidate (1..4) is rejected, and the one overlapping it (3..6) is valid.
        assert_eq!(all("ba-a-a", &ww("a-a", true)), vec![3..6]);
        assert_eq!(all("ba-a-a-a", &ww("a-a", true)), vec![3..6]);
        assert_eq!(all("a-a-a b", &ww("a-a", true)), vec![0..3]);
        assert_eq!(all("a-a-a", &ww("a-a", false)), vec![0..3]);
        // Across chunk boundaries of a rope.
        let mut text = String::new();
        let mut want = Vec::new();
        for i in 0..2000 {
            if i % 3 == 0 {
                want.push(text.len() + 1..text.len() + 4);
                text.push_str(" cat ");
            } else {
                text.push_str(" xcat");
            }
        }
        let rope = many_chunks(&text);
        assert_eq!(
            find_all(&rope, &ww("cat", true), 100_000, &NO()).ranges,
            want
        );
    }

    #[test]
    fn whole_word_find_next_backward_and_forward() {
        let text = "cat concat cat catx";
        let rope = Rope::from_str(text);
        let q = ww("cat", true);
        let n = |from, fwd| find_next(&rope, &q, from, fwd, true, &NO());
        assert_eq!(n(1, true), Some(11..14));
        assert_eq!(n(14, false), Some(11..14));
        assert_eq!(n(11, false), Some(0..3));
        assert_eq!(n(3, true), Some(11..14));
        // A window that cuts a word still sees the letters beyond its ends.
        let big = format!("{}cat{}", "x ".repeat(200_000), "y".repeat(10));
        let rope = Rope::from_str(&big);
        assert_eq!(find_next(&rope, &q, big.len(), false, false, &NO()), None);
        let big = format!("{}cat{}", "x ".repeat(200_000), " y");
        let rope = Rope::from_str(&big);
        assert_eq!(
            find_next(&rope, &q, big.len(), false, false, &NO()),
            Some(400_000..400_003)
        );
        // A match ending exactly at `from` with a word character after it is not a whole word.
        let rope = Rope::from_str("a cat dog");
        assert_eq!(
            find_next(&rope, &ww("ca", true), 4, false, false, &NO()),
            None
        );
    }

    // ---- regex

    fn rx(text: &str, match_case: bool) -> Query {
        Query::new(
            text,
            FindOptions {
                match_case,
                whole_word: false,
                regex: true,
            },
        )
        .unwrap()
    }

    fn rxw(text: &str) -> Query {
        Query::new(
            text,
            FindOptions {
                match_case: true,
                whole_word: true,
                regex: true,
            },
        )
        .unwrap()
    }

    #[test]
    fn regex_anchors_are_per_line_for_every_line_ending() {
        for eol in ["\n", "\r\n", "\r"] {
            let text = format!("ab{eol}cd{eol}{eol}ab");
            assert_eq!(texts(&text, &rx("^.", true)), ["a", "c", "a"], "{eol:?}");
            assert_eq!(texts(&text, &rx(".$", true)), ["b", "d", "b"], "{eol:?}");
            assert_eq!(texts(&text, &rx("^ab$", true)).len(), 2, "{eol:?}");
            assert_eq!(texts(&text, &rx("^cd$", true)), ["cd"], "{eol:?}");
        }
        // The line break is not part of any match; a pattern cannot match it.
        assert!(all("a\nb", &rx("a.b", true)).is_empty());
        assert!(all("a\r\nb", &rx(r"a\sb", true)).is_empty());
        assert!(all("a\nb", &rx(r"a\nb", true)).is_empty());
        assert!(all("a\rb", &rx("a.b", true)).is_empty());
        // Offsets are bytes of the whole buffer.
        assert_eq!(all("é\r\nab\nab", &rx("^ab", true)), vec![4..6, 7..9]);
    }

    #[test]
    fn regex_classes_alternation_and_unicode() {
        assert_eq!(texts("a1 b22 c333", &rx(r"\d+", true)), ["1", "22", "333"]);
        assert_eq!(
            texts("abcabc xbca", &rx(r"[a-c]+", true)),
            ["abcabc", "bca"]
        );
        assert_eq!(texts("cat dog bird", &rx("cat|dog", true)), ["cat", "dog"]);
        assert_eq!(
            texts("çağ 漢字 x1", &rx(r"\p{L}+", true)),
            ["çağ", "漢字", "x"]
        );
        assert_eq!(texts("a.b a+b", &rx(r"a\.b", true)), ["a.b"]);
        assert_eq!(texts("foo(1) foo(22)", &rx(r"foo\((\d+)\)", true)).len(), 2);
        assert_eq!(texts("aaa", &rx("a{2}", true)), ["aa"]);
        assert_eq!(texts("tab\there", &rx(r"\t", true)), ["\t"]);
        assert_eq!(all("日本語 日本", &rx("本", true)), vec![3..6, 13..16]);
    }

    #[test]
    fn regex_case_folding() {
        assert_eq!(texts("Hello HELLO hello", &rx("hello", false)).len(), 3);
        assert_eq!(texts("Hello HELLO hello", &rx("hello", true)), ["hello"]);
        assert_eq!(texts("Hello HELLO hello", &rx("HELLO", true)), ["HELLO"]);
        assert_eq!(texts("ÇAĞ çağ", &rx("çağ", false)).len(), 2);
        assert_eq!(texts("ΟΔΟΣ οδος", &rx("οδοσ", false)).len(), 2);
        assert_eq!(texts("k K \u{212A}", &rx("k", false)).len(), 3);
        // The literal path's Turkish i folding does not apply: İ and ı are not case partners of
        // i and I here.
        assert!(all("İ ı", &rx("i", false)).is_empty());
        assert!(all("İ ı", &rx("I", false)).is_empty());
        assert_eq!(texts("i I", &rx("i", false)), ["i", "I"]);
        // An inline flag still wins over the option.
        assert_eq!(texts("A a", &rx("(?-i)a", false)), ["a"]);
    }

    #[test]
    fn regex_invalid_and_check() {
        let on = FindOptions {
            regex: true,
            ..FindOptions::default()
        };
        for bad in [
            "(",
            "[a-",
            "a{2,1}",
            "*a",
            "(?P<n>a)(?P<n>b)",
            r"\p{Nope}",
            "(a{1000}){1000}",
        ] {
            assert!(Query::new(bad, on).is_none(), "{bad:?}");
            let why = Query::check(bad, on).unwrap_err();
            assert!(!why.is_empty() && !why.contains('\n'), "{bad:?}: {why:?}");
        }
        eprintln!(
            "{:?} / {:?}",
            Query::check("(", on),
            Query::check("[a-", on)
        );
        for good in ["a", r"(\w+)@(\w+)", "^$", "a|b"] {
            assert!(Query::new(good, on).is_some(), "{good:?}");
            assert_eq!(Query::check(good, on), Ok(()));
        }
        // Literal queries and the empty query are never invalid; the same text is fine as a
        // literal.
        assert_eq!(Query::check("(", FindOptions::default()), Ok(()));
        assert!(Query::new("(", FindOptions::default()).is_some());
        assert_eq!(Query::check("", on), Ok(()));
        assert!(Query::new("", on).is_none());
        let q = rx("a+", true);
        assert_eq!(q.text(), "a+");
        assert!(q.options().regex);
    }

    #[test]
    fn regex_empty_matches_are_not_reported() {
        assert_eq!(all("baab", &rx("a*", true)), vec![1..3]);
        assert!(all("abc", &rx("x*", true)).is_empty());
        assert!(all("abc", &rx("^", true)).is_empty());
        assert!(all("abc\ndef", &rx("$", true)).is_empty());
        assert!(all("", &rx("a*", true)).is_empty());
        assert_eq!(all("aa b aaa", &rx(r"a*\b", true)).len(), 2);
        assert_eq!(
            all("a\n\nb", &rx(r"^\s*$", true)),
            Vec::<Range<usize>>::new()
        );
        // An empty alternative does not stop a later, non-empty one from matching elsewhere.
        assert_eq!(all("xay", &rx("a?", true)), vec![1..2]);
        // Non-overlapping, left to right.
        assert_eq!(all("aaaa", &rx("aa", true)), vec![0..2, 2..4]);
        assert_eq!(all("abab", &rx("aba?", true)), vec![0..3]);
    }

    #[test]
    fn regex_whole_word() {
        assert_eq!(texts("12a 34 x5", &rxw(r"\d+")), ["34"]);
        assert_eq!(texts("xaaa aaa", &rxw("a+")), ["aaa"]);
        assert_eq!(texts("cat concat (cat) cats", &rxw("cat")), ["cat", "cat"]);
        assert_eq!(texts("ça cat", &rxw("a")), Vec::<String>::new());
        assert_eq!(texts("日本 日本語", &rxw("日本")), ["日本"]);
        // At line ends and buffer ends.
        assert_eq!(all("cat\ncat", &rxw("cat")), vec![0..3, 4..7]);
        // A candidate rejected for its neighbours does not hide an overlapping valid one.
        assert_eq!(all("ba-a-a", &rxw("a-a")), vec![3..6]);
        // A long line of rejected candidates costs a bounded amount of work.
        let line = "a".repeat(100_000);
        let t = std::time::Instant::now();
        let m = find_all(&Rope::from_str(&format!("x{line}")), &rxw("a+"), 10, &NO());
        assert!(m.ranges.is_empty());
        assert!(
            t.elapsed() < std::time::Duration::from_secs(20),
            "{:?}",
            t.elapsed()
        );
    }

    #[test]
    fn regex_line_over_the_limit_is_skipped() {
        let long = "a".repeat(REGEX_LINE_MAX + 1);
        let text = format!("a\n{long}\na\n{long}");
        let found = all(&text, &rx("a", true));
        assert_eq!(found, vec![0..1, 2 + long.len() + 1..2 + long.len() + 2]);
        // A line of exactly the limit is searched.
        let edge = "b".repeat(REGEX_LINE_MAX - 1);
        let text = format!("{edge}a\nx");
        assert_eq!(
            all(&text, &rx("a$", true)),
            vec![REGEX_LINE_MAX - 1..REGEX_LINE_MAX]
        );
        // Too long for a regex, but plain find is not limited.
        let text = format!("{long}z");
        assert_eq!(all(&text, &q("z")).len(), 1);
        assert!(all(&text, &rx("z", true)).is_empty());
        // find_next skips it the same way, forward and backward.
        let rope = Rope::from_str(&format!("{long}\nz\n{long}z"));
        let z = rx("z", true);
        assert_eq!(
            find_next(&rope, &z, 0, true, false, &NO()),
            Some(long.len() + 1..long.len() + 2)
        );
        assert_eq!(
            find_next(&rope, &z, rope.len_bytes(), false, false, &NO()),
            Some(long.len() + 1..long.len() + 2)
        );
    }

    #[test]
    fn regex_cap_and_cancel() {
        let text = "ab\n".repeat(100);
        let rope = Rope::from_str(&text);
        let r = rx("a", true);
        let m = find_all(&rope, &r, 10, &NO());
        assert_eq!(m.ranges.len(), 10);
        assert!(m.capped && !m.cancelled);
        let m = find_all(&rope, &r, 100, &NO());
        assert_eq!(m.ranges.len(), 100);
        assert!(!m.capped);
        assert!(find_all(&rope, &r, 99, &NO()).capped);
        let m = find_all(&rope, &r, 0, &NO());
        assert!(m.ranges.is_empty() && m.capped);
        let flag = AtomicBool::new(true);
        let m = find_all(&rope, &r, 1000, &flag);
        assert!(m.cancelled && m.ranges.is_empty());
        assert_eq!(find_next(&rope, &r, 0, true, true, &flag), None);
        assert_eq!(find_next(&rope, &r, 100, false, true, &flag), None);
    }

    #[test]
    fn regex_cancel_from_another_thread_stops_a_long_search() {
        let text = "abcdefghij\n".repeat(2_000_000);
        let rope = Rope::from_str(&text);
        let query = rx("(a|b)+z", true);
        let flag = AtomicBool::new(false);
        let (m, elapsed) = std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(5));
                flag.store(true, Ordering::Relaxed);
            });
            let t = std::time::Instant::now();
            (find_all(&rope, &query, 10_000, &flag), t.elapsed())
        });
        eprintln!(
            "regex cancelled mid-search: {} after {elapsed:?}",
            m.cancelled
        );
        assert!(m.ranges.is_empty());
        if m.cancelled {
            assert!(elapsed < std::time::Duration::from_secs(5));
        }
        assert!(find_all(&rope, &query, 10_000, &flag).cancelled);
    }

    /// A text of many chunks and lines with every kind of line ending.
    fn mixed_lines(rng: &mut Rng, lines: usize) -> String {
        let words = [
            "cat", "Cat", "dog", "aab", "ab", "x1", "çağ", "漢字", "", "  ", "a-a",
        ];
        let mut t = String::new();
        for _ in 0..lines {
            for _ in 0..rng.below(4) {
                t.push_str(words[rng.below(words.len())]);
                t.push(' ');
            }
            t.push_str(["\n", "\r\n", "\r"][rng.below(3)]);
        }
        t.push_str("tail cat");
        t
    }

    #[test]
    fn regex_find_next_agrees_with_find_all() {
        let mut rng = Rng(0x9E3779B97F4A7C15);
        for round in 0..30 {
            let n = 5 + rng.below(400);
            let text = mixed_lines(&mut rng, n);
            let rope = many_chunks(&text);
            let pat = ["a+", r"^c\w*", r"\w+$", "[ab]+b", r"(?i)cat", r"\d", "x"][round % 7];
            for whole_word in [false, true] {
                let query = Query::new(
                    pat,
                    FindOptions {
                        match_case: true,
                        whole_word,
                        regex: true,
                    },
                )
                .unwrap();
                let list = find_all(&rope, &query, 1_000_000, &NO()).ranges;
                let step = 1 + text.len() / 150;
                let mut from = 0;
                while from <= text.len() {
                    let from_c = rope.char_to_byte(rope.byte_to_char(from));
                    let fwd = list.iter().find(|r| r.start >= from_c).cloned();
                    let wrapped = fwd
                        .clone()
                        .or_else(|| list.first().filter(|r| r.start < from_c).cloned());
                    assert_eq!(
                        find_next(&rope, &query, from, true, false, &NO()),
                        fwd,
                        "{pat} fwd {from}"
                    );
                    assert_eq!(
                        find_next(&rope, &query, from, true, true, &NO()),
                        wrapped,
                        "{pat} wrap {from}"
                    );
                    let back = list.iter().rev().find(|r| r.end <= from_c).cloned();
                    let back_wrapped = back
                        .clone()
                        .or_else(|| list.iter().rev().find(|r| r.start >= from_c).cloned());
                    assert_eq!(
                        find_next(&rope, &query, from, false, false, &NO()),
                        back,
                        "{pat} back {from}"
                    );
                    assert_eq!(
                        find_next(&rope, &query, from, false, true, &NO()),
                        back_wrapped,
                        "{pat} backwrap {from}"
                    );
                    from += step;
                }
            }
        }
    }

    #[test]
    fn regex_find_all_equals_a_per_line_reference() {
        // The reference: the regex engine run on each line split by plain string code.
        let mut rng = Rng(77);
        for round in 0..40 {
            let n = 3 + rng.below(300);
            let text = mixed_lines(&mut rng, n);
            let rope = many_chunks(&text);
            let pat = [r"[a-c]+", r"\w+\s", r"^\S", r"\S$", "(cat|dog)", r"x\d?"][round % 6];
            let query = rx(pat, round % 2 == 0);
            let re = meta::Regex::builder()
                .syntax(syntax::Config::new().case_insensitive(round % 2 != 0))
                .build(pat)
                .unwrap();
            let mut want = Vec::new();
            let mut at = 0;
            for l in crate::testutil::lines_of(&text) {
                let content = l.trim_end_matches(['\r', '\n']);
                for m in re.find_iter(content) {
                    if !m.is_empty() {
                        want.push(at + m.start()..at + m.end());
                    }
                }
                at += l.len();
            }
            let got = find_all(&rope, &query, 1_000_000, &NO());
            assert_eq!(got.ranges, want, "{pat} round {round}");
            assert!(!got.capped && !got.cancelled);
        }
    }
}
