//! Replace: one match ([`replace_one`]) or every match ([`replace_all`]), each exactly one undo
//! step (`Kind::Other`) with one merged [`Change`](crate::Change), like the commands in
//! [`crate::edit`].
//!
//! The replacement text of a match comes from the template ([`expand`]); line breaks in it
//! become the document's `eol`. [`replace_all`] finds every match on the unchanged text first,
//! then replaces front to back in place, so the document is never built as one `String`; its
//! extra memory is the list of match ranges (16 bytes each) besides the undo records.

use crate::buffer::Buffer;
use crate::edit::{Cmd, Outcome, nothing};
use crate::eol::normalize_eols;
use crate::find::{self, Query};
use crate::history::{History, Kind};
use crate::selection::Selection;
use regex_automata::util::captures::Captures;
use regex_automata::{Anchored, Input, PatternID, meta};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};

/// One piece of a parsed template.
enum Part {
    Text(String),
    /// A capture group by index (0 is the whole match).
    Group(usize),
}

/// A template read once, so each match only copies pieces together.
struct Expander<'a> {
    regex: Option<&'a meta::Regex>,
    parts: Vec<Part>,
    /// Some part is a group other than 0, so the pattern must be run again on the matched text.
    needs_captures: bool,
    cache: Option<meta::Cache>,
    caps: Option<Captures>,
}

impl<'a> Expander<'a> {
    fn new(query: &'a Query, template: &str) -> Expander<'a> {
        let Some(re) = query.regex() else {
            return Expander {
                regex: None,
                parts: vec![Part::Text(template.to_string())],
                needs_captures: false,
                cache: None,
                caps: None,
            };
        };
        let info = re.group_info();
        let groups = info.group_len(PatternID::ZERO);
        let mut parts = Vec::new();
        let mut text = String::new();
        let b = template.as_bytes();
        let mut i = 0;
        let flush = |text: &mut String, parts: &mut Vec<Part>| {
            if !text.is_empty() {
                parts.push(Part::Text(std::mem::take(text)));
            }
        };
        while i < b.len() {
            if b[i] != b'$' {
                let n = template[i..].chars().next().map_or(1, char::len_utf8);
                text.push_str(&template[i..i + n]);
                i += n;
                continue;
            }
            match b.get(i + 1) {
                Some(b'$') => {
                    text.push('$');
                    i += 2;
                }
                Some(d) if d.is_ascii_digit() => {
                    let mut n = usize::from(d - b'0');
                    i += 2;
                    if let Some(d2) = b.get(i).filter(|d| d.is_ascii_digit()) {
                        n = n * 10 + usize::from(d2 - b'0');
                        i += 1;
                    }
                    if n < groups {
                        flush(&mut text, &mut parts);
                        parts.push(Part::Group(n));
                    }
                }
                Some(b'{') => match template[i + 2..].find('}') {
                    Some(close) if close > 0 => {
                        let name = &template[i + 2..i + 2 + close];
                        let index = if name.bytes().all(|c| c.is_ascii_digit()) {
                            name.parse::<usize>().ok()
                        } else {
                            info.to_index(PatternID::ZERO, name)
                        };
                        if let Some(n) = index.filter(|&n| n < groups) {
                            flush(&mut text, &mut parts);
                            parts.push(Part::Group(n));
                        }
                        i += 3 + close;
                    }
                    _ => {
                        text.push('$');
                        i += 1;
                    }
                },
                _ => {
                    text.push('$');
                    i += 1;
                }
            }
        }
        flush(&mut text, &mut parts);
        let needs_captures = parts.iter().any(|p| matches!(p, Part::Group(n) if *n > 0));
        Expander {
            regex: Some(re),
            parts,
            needs_captures,
            cache: None,
            caps: None,
        }
    }

    /// Appends the expansion for the matched text `matched` to `out`.
    fn expand_into(&mut self, matched: &str, out: &mut String) {
        let mut found = false;
        if self.needs_captures
            && let Some(re) = self.regex
        {
            let cache = self.cache.get_or_insert_with(|| re.create_cache());
            let caps = self.caps.get_or_insert_with(|| re.create_captures());
            let input = Input::new(matched).anchored(Anchored::Yes);
            re.search_captures_with(cache, &input, caps);
            found = caps.is_match();
        }
        for part in &self.parts {
            match part {
                Part::Text(t) => out.push_str(t),
                Part::Group(0) => out.push_str(matched),
                Part::Group(n) => {
                    if found && let Some(span) = self.caps.as_ref().and_then(|c| c.get_group(*n)) {
                        out.push_str(&matched[span.range()]);
                    }
                }
            }
        }
    }
}

/// The replacement text for one match of `query` whose text is `matched`.
///
/// For a literal query it is `template` as it is. For a regex query the pattern is run again on
/// `matched` (anchored at its start, with `matched` as the whole haystack) and the template is
/// read as follows:
/// - `$0` is the whole match; `$1`..`$99` (one or two digits) are capture groups; `${1}` and
///   `${name}` are a group by number or by name (`(?P<name>..)`), which allows a digit right
///   after it;
/// - `$$` is a dollar sign;
/// - a group that does not exist, or did not take part in the match, is empty;
/// - a `$` followed by anything else (or a `${` without a `}`) is an ordinary dollar sign.
///
/// Line breaks are left as they are here; [`replace_one`] and [`replace_all`] turn them into the
/// document's own.
pub fn expand(query: &Query, matched: &str, template: &str) -> String {
    let mut out = String::new();
    Expander::new(query, template).expand_into(matched, &mut out);
    out
}

fn text_of(buf: &Buffer, range: Range<usize>) -> String {
    let mut s = String::new();
    buf.text(range, &mut s);
    s
}

/// Replaces `range` with the expansion of `template` when `range` is exactly a match of `query`
/// in the buffer as it is now (`find` is re-run on it, whole-word included); line breaks in the
/// result become `eol`. One undo step; the selection afterwards is a caret at the end of the
/// inserted text. Returns the selection unchanged with no change when `range` is not a match, and
/// also when the replacement is the very text already there (nothing to record).
pub fn replace_one(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    query: &Query,
    range: Range<usize>,
    template: &str,
    eol: &str,
) -> Outcome {
    if !find::is_match(buf.rope(), query, range.clone()) {
        return nothing(sel);
    }
    let matched = text_of(buf, range.clone());
    let mut out = String::new();
    Expander::new(query, template).expand_into(&matched, &mut out);
    let text = normalize_eols(&out, eol);
    if *text == *matched {
        return nothing(sel);
    }
    let caret = range.start + text.len();
    let mut cmd = Cmd::new(buf);
    cmd.replace(range, &text);
    cmd.finish(hist, Kind::Other, sel, Selection::at(caret))
}

/// Replaces every match of `query` (or, with `within`, every match lying wholly inside that
/// range: "in selection") by its expansion of `template`, as ONE undo step, and returns the
/// number of matches. The matches are found first on the unchanged text and then replaced front
/// to back in place.
///
/// A selection end inside a replaced match goes to the end of its replacement; the others move
/// with the text. A plain query counts matches from `within.start`; a regex works on the whole
/// lines `within` touches and keeps the matches inside it.
///
/// Changes nothing and returns count 0 when `cancel` is set (before or during the search), or
/// when the search hit its work limit (so a partial replacement never happens). When every
/// replacement equals its match the result has no change (and no history entry) but the count is
/// still that of the matches.
///
/// This is [`replace_all_in`] with one range (`within`) or the whole document.
#[allow(clippy::too_many_arguments)]
pub fn replace_all(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    query: &Query,
    template: &str,
    eol: &str,
    within: Option<Range<usize>>,
    cancel: &AtomicBool,
) -> (Outcome, usize) {
    let scope = match within {
        Some(w) => w,
        None => 0..buf.len_bytes(),
    };
    let (out, count, _) = replace_all_in(buf, hist, sel, query, template, eol, &[scope], cancel);
    (out, count)
}

/// Like [`replace_all`] over several ranges (the selections of a multi-caret or column
/// selection): every match lying wholly inside ONE of `within` is replaced, all as one undo
/// step, and the count is the number of matches replaced. The ranges are clamped, floored to
/// `char` boundaries, sorted, and ranges that overlap are merged into their union; empty ranges
/// are dropped, and ranges that only touch stay apart, so a match that spans two of them is not
/// one. No ranges (or only empty ones) means no match: nothing changes and the count is 0. Each
/// range is searched as [`replace_all`] searches its `within`, with its own work limit.
///
/// Also returns the ranges (clamped, merged, sorted, as above) mapped into the new text, so the
/// shell can keep them selected: a range's start moves with the text before it, its end is the
/// end of its replacement. `sel` maps as in [`replace_all`].
#[allow(clippy::too_many_arguments)]
pub fn replace_all_in(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    query: &Query,
    template: &str,
    eol: &str,
    within: &[Range<usize>],
    cancel: &AtomicBool,
) -> (Outcome, usize, Vec<Range<usize>>) {
    let scopes = merged_scopes(buf, within);
    if cancel.load(Ordering::Relaxed) {
        return (nothing(sel), 0, scopes);
    }
    let len = buf.len_bytes();
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for scope in &scopes {
        let found = find::walk_within(buf.rope(), query, scope.clone(), cancel, |r| {
            ranges.push(r);
            true
        });
        if found.is_err() {
            return (nothing(sel), 0, scopes);
        }
    }
    let count = ranges.len();
    // The points to carry through the edits: the selection ends and the scopes' ends, ascending
    // (a cursor walks them along the matches), each with where its mapped value goes.
    let mut points: Vec<(usize, usize)> = Vec::with_capacity(2 + 2 * scopes.len());
    points.push((sel.anchor.min(len), 0));
    points.push((sel.caret.min(len), 1));
    for (i, s) in scopes.iter().enumerate() {
        points.push((s.start, 2 + 2 * i));
        points.push((s.end, 3 + 2 * i));
    }
    points.sort_unstable();
    let mut mapped: Vec<usize> = vec![0; points.len()];
    let mut next = 0;
    let mut expander = Expander::new(query, template);
    let mut cmd = Cmd::new(buf);
    let (mut matched, mut out) = (String::new(), String::new());
    // Bytes the buffer has grown by so far: its offsets are the original ones plus this.
    let mut delta = 0isize;
    let shift = |x: usize, d: isize| (x as isize + d) as usize;
    for r in &ranges {
        let now = shift(r.start, delta)..shift(r.end, delta);
        cmd.buf.text(now.clone(), &mut matched);
        out.clear();
        expander.expand_into(&matched, &mut out);
        let text = normalize_eols(&out, eol);
        while next < points.len() && points[next].0 <= r.end {
            let x = points[next].0;
            mapped[points[next].1] = if x <= r.start {
                shift(x, delta)
            } else {
                now.start + text.len()
            };
            next += 1;
        }
        if *text != *matched {
            cmd.replace(now, &text);
        }
        delta += text.len() as isize - (r.end - r.start) as isize;
    }
    for &(x, slot) in &points[next..] {
        mapped[slot] = shift(x, delta);
    }
    let after = Selection {
        anchor: mapped[0],
        caret: mapped[1],
        goal_col: None,
    };
    let new_scopes = (0..scopes.len())
        .map(|i| mapped[2 + 2 * i]..mapped[3 + 2 * i])
        .collect();
    (cmd.finish(hist, Kind::Other, sel, after), count, new_scopes)
}

/// `within` clamped to the buffer, floored to `char` boundaries, empty ranges dropped, sorted,
/// overlapping ranges merged.
fn merged_scopes(buf: &Buffer, within: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut v: Vec<Range<usize>> = within
        .iter()
        .map(|w| {
            let start = buf.floor_char(w.start);
            start..buf.floor_char(w.end).max(start)
        })
        .filter(|r| !r.is_empty())
        .collect();
    v.sort_unstable_by_key(|r| (r.start, r.end));
    let mut out: Vec<Range<usize>> = Vec::with_capacity(v.len());
    for r in v {
        match out.last_mut() {
            Some(last) if r.start < last.end => last.end = last.end.max(r.end),
            _ => out.push(r),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{backspace, insert, redo, undo};
    use crate::eol::EolCounts;
    use crate::find::{FindOptions, find_all};
    use crate::testutil::{Rng, check_change, check_eols};

    const NO: fn() -> AtomicBool = || AtomicBool::new(false);

    fn opts(match_case: bool, whole_word: bool, regex: bool) -> FindOptions {
        FindOptions {
            match_case,
            whole_word,
            regex,
        }
    }

    fn lit(text: &str) -> Query {
        Query::new(text, opts(true, false, false)).unwrap()
    }

    fn re(text: &str) -> Query {
        Query::new(text, opts(true, false, true)).unwrap()
    }

    fn txt(buf: &Buffer) -> String {
        text_of(buf, 0..buf.len_bytes())
    }

    fn all(
        text: &str,
        q: &Query,
        template: &str,
        eol: &str,
        sel: Selection,
    ) -> (String, Outcome, usize) {
        let mut buf = Buffer::from_text(text);
        let mut hist = History::new();
        let (o, n) = replace_all(&mut buf, &mut hist, sel, q, template, eol, None, &NO());
        (txt(&buf), o, n)
    }

    // ---- expand

    #[test]
    fn expand_rules() {
        let q = re(r"(?P<a>\w)(\d)?");
        assert_eq!(expand(&q, "x1", "[$0|$1|$2|${a}|${2}]"), "[x1|x|1|x|1]");
        assert_eq!(expand(&q, "x", "[$1|$2]"), "[x|]");
        assert_eq!(expand(&q, "x1", "$$1 $$ $"), "$1 $ $");
        assert_eq!(expand(&q, "x1", "$9 ${nope} ${9} ${"), "   ${");
        assert_eq!(expand(&q, "x1", "${1}2 $12"), "x2 ");
        assert_eq!(expand(&q, "x1", "$-$a $é"), "$-$a $é");
        assert_eq!(expand(&q, "x1", "${}"), "${}");
        // The template is read as it is for a literal query.
        assert_eq!(expand(&lit("a"), "a", "$1 $0 $$"), "$1 $0 $$");
        // Groups come from re-running the pattern on the matched text.
        let q = re(r"(\w+)@(\w+)");
        assert_eq!(expand(&q, "me@host", "$2 at $1"), "host at me");
    }

    // ---- replace_one

    #[test]
    fn replace_one_literal_and_regex() {
        let mut buf = Buffer::from_text("one two one");
        let mut hist = History::new();
        let s = Selection::at(0);
        let o = replace_one(&mut buf, &mut hist, s, &lit("one"), 8..11, "1", "\n");
        assert_eq!(txt(&buf), "one two 1");
        assert_eq!(o.sel, Selection::at(9));
        check_change("one two one", "one two 1", o.change.as_ref().unwrap());
        // Not a match: nothing.
        let before = hist.steps();
        let o = replace_one(&mut buf, &mut hist, s, &lit("one"), 1..4, "1", "\n");
        assert!(o.change.is_none() && o.sel == s);
        let o = replace_one(&mut buf, &mut hist, s, &lit("one"), 0..2, "1", "\n");
        assert!(o.change.is_none());
        let o = replace_one(&mut buf, &mut hist, s, &lit("one"), 0..0, "1", "\n");
        assert!(o.change.is_none());
        let o = replace_one(&mut buf, &mut hist, s, &lit("one"), 0..99, "1", "\n");
        assert!(o.change.is_none());
        assert_eq!(hist.steps(), before);
        // The same text: nothing recorded.
        let o = replace_one(&mut buf, &mut hist, s, &lit("one"), 0..3, "one", "\n");
        assert!(o.change.is_none());
        // Regex with groups.
        let q = re(r"(\w+) (\w+)");
        let o = replace_one(&mut buf, &mut hist, s, &q, 0..7, "$2 $1", "\n");
        assert_eq!(txt(&buf), "two one 1");
        assert_eq!(o.sel, Selection::at(7));
        // An overlapping occurrence is still a match when asked for directly.
        let mut buf = Buffer::from_text("aaa");
        let o = replace_one(&mut buf, &mut hist, s, &lit("aa"), 1..3, "b", "\n");
        assert!(o.change.is_some());
        assert_eq!(txt(&buf), "ab");
    }

    #[test]
    fn replace_one_rechecks_whole_word_and_checks_the_line() {
        let mut buf = Buffer::from_text("cat concat cat");
        let mut hist = History::new();
        let q = Query::new("cat", opts(true, true, false)).unwrap();
        let s = Selection::at(0);
        assert!(
            replace_one(&mut buf, &mut hist, s, &q, 7..10, "x", "\n")
                .change
                .is_none()
        );
        assert!(
            replace_one(&mut buf, &mut hist, s, &q, 11..14, "x", "\n")
                .change
                .is_some()
        );
        assert_eq!(txt(&buf), "cat concat x");
        let q = Query::new(r"\w+", opts(true, true, true)).unwrap();
        assert!(
            replace_one(&mut buf, &mut hist, s, &q, 4..7, "x", "\n")
                .change
                .is_none()
        );
        assert!(
            replace_one(&mut buf, &mut hist, s, &q, 4..10, "x", "\n")
                .change
                .is_some()
        );
        // A regex match never spans lines.
        let mut buf = Buffer::from_text("ab\ncd");
        let q = re("b.c");
        assert!(
            replace_one(&mut buf, &mut hist, s, &q, 1..4, "x", "\n")
                .change
                .is_none()
        );
    }

    #[test]
    fn replace_one_normalizes_line_breaks() {
        let mut buf = Buffer::from_text("a-b\r\nc");
        let mut hist = History::new();
        let o = replace_one(
            &mut buf,
            &mut hist,
            Selection::at(0),
            &lit("-"),
            1..2,
            "\n\r\n\r",
            "\r\n",
        );
        assert_eq!(txt(&buf), "a\r\n\r\n\r\nb\r\nc");
        assert_eq!(o.sel, Selection::at(7));
        let mut counts = EolCounts::of("a-b\r\nc");
        check_eols(&mut counts, &txt(&buf), o.change.as_ref().unwrap());
    }

    // ---- replace_all

    #[test]
    fn replace_all_literal_and_regex() {
        let (t, o, n) = all("a b a b a", &lit("a"), "xy", "\n", Selection::at(0));
        assert_eq!((t.as_str(), n), ("xy b xy b xy", 3));
        check_change("a b a b a", &t, o.change.as_ref().unwrap());
        let (t, _, n) = all(
            "ab12 cd34",
            &re(r"([a-z]+)(\d+)"),
            "$2$1",
            "\n",
            Selection::at(0),
        );
        assert_eq!((t.as_str(), n), ("12ab 34cd", 2));
        let (t, _, _) = all(
            "ab",
            &re("(a)(x)?"),
            "[$2|$3|$$|${q}]",
            "\n",
            Selection::at(0),
        );
        assert_eq!(t, "[||$|]b");
        // Deleting.
        let (t, _, n) = all("a1b22", &re(r"\d+"), "", "\n", Selection::at(0));
        assert_eq!((t.as_str(), n), ("ab", 2));
        // No match: nothing.
        let (t, o, n) = all("abc", &lit("z"), "y", "\n", Selection::at(2));
        assert_eq!((t.as_str(), n), ("abc", 0));
        assert!(o.change.is_none() && o.sel == Selection::at(2));
        // Same text for every match: nothing, count of matches.
        let mut buf = Buffer::from_text("a a");
        let mut hist = History::new();
        let (o, n) = replace_all(
            &mut buf,
            &mut hist,
            Selection::at(1),
            &lit("a"),
            "a",
            "\n",
            None,
            &NO(),
        );
        assert_eq!(n, 2);
        assert!(o.change.is_none() && o.sel == Selection::at(1));
        assert!(!hist.can_undo());
        // Case-insensitive and whole word.
        let q = Query::new("cat", opts(false, true, false)).unwrap();
        let (t, _, n) = all("Cat concat CAT cat.", &q, "dog", "\n", Selection::at(0));
        assert_eq!((t.as_str(), n), ("dog concat dog dog.", 3));
    }

    #[test]
    fn replace_all_crlf_document_with_lf_template() {
        let text = "a-b\r\nc-d\r\ne";
        let (t, o, n) = all(text, &lit("-"), "\n", "\r\n", Selection::at(0));
        assert_eq!((t.as_str(), n), ("a\r\nb\r\nc\r\nd\r\ne", 2));
        let mut counts = EolCounts::of(text);
        check_eols(&mut counts, &t, o.change.as_ref().unwrap());
        check_change(text, &t, o.change.as_ref().unwrap());
        // Into an LF document with a CRLF template.
        let (t, _, _) = all("a-b\nc", &lit("-"), "\r\n", "\n", Selection::at(0));
        assert_eq!(t, "a\nb\nc");
    }

    #[test]
    fn replace_all_within() {
        let text = "ab ab ab ab";
        let mut buf = Buffer::from_text(text);
        let mut hist = History::new();
        let sel = Selection {
            anchor: 3,
            caret: 8,
            goal_col: None,
        };
        let (o, n) = replace_all(
            &mut buf,
            &mut hist,
            sel,
            &lit("ab"),
            "X",
            "\n",
            Some(3..8),
            &NO(),
        );
        assert_eq!((txt(&buf).as_str(), n), ("ab X X ab", 2));
        assert_eq!(
            o.sel,
            Selection {
                anchor: 3,
                caret: 6,
                goal_col: None
            }
        );
        // A match only partly inside is not touched.
        let mut buf = Buffer::from_text(text);
        let (_, n) = replace_all(
            &mut buf,
            &mut hist,
            sel,
            &lit("ab"),
            "X",
            "\n",
            Some(4..8),
            &NO(),
        );
        assert_eq!((txt(&buf).as_str(), n), ("ab ab X ab", 1));
        // Regex: `^` is still the line's start inside a selection.
        let mut buf = Buffer::from_text("ab\nab\nab");
        let (_, n) = replace_all(
            &mut buf,
            &mut hist,
            Selection::at(0),
            &re("^a"),
            "X",
            "\n",
            Some(1..6),
            &NO(),
        );
        assert_eq!((txt(&buf).as_str(), n), ("ab\nXb\nab", 1));
        // Whole word looks beyond the selection's ends.
        let mut buf = Buffer::from_text("xcat cat catx");
        let q = Query::new("cat", opts(true, true, false)).unwrap();
        let (_, n) = replace_all(
            &mut buf,
            &mut hist,
            Selection::at(0),
            &q,
            "d",
            "\n",
            Some(1..13),
            &NO(),
        );
        assert_eq!((txt(&buf).as_str(), n), ("xcat d catx", 1));
        // Empty and out-of-range scopes.
        let (_, n) = replace_all(
            &mut buf,
            &mut hist,
            Selection::at(0),
            &lit("x"),
            "d",
            "\n",
            Some(5..5),
            &NO(),
        );
        assert_eq!(n, 0);
        let (_, n) = replace_all(
            &mut buf,
            &mut hist,
            Selection::at(0),
            &lit("x"),
            "d",
            "\n",
            Some(50..90),
            &NO(),
        );
        assert_eq!(n, 0);
    }

    #[test]
    fn replace_all_maps_the_selection() {
        // "aa b aa": matches 0..2 and 5..7, replaced by "xxxx" (grows by 2 each).
        let text = "aa b aa";
        let q = lit("aa");
        let run = |a: usize, c: usize| {
            let (t, o, _) = all(
                text,
                &q,
                "xxxx",
                "\n",
                Selection {
                    anchor: a,
                    caret: c,
                    goal_col: None,
                },
            );
            assert_eq!(t, "xxxx b xxxx");
            (o.sel.anchor, o.sel.caret)
        };
        assert_eq!(run(0, 0), (0, 0));
        assert_eq!(run(1, 1), (4, 4)); // inside a match: its replacement's end
        assert_eq!(run(2, 2), (4, 4)); // at a match's end
        assert_eq!(run(3, 3), (5, 5)); // between
        assert_eq!(run(5, 5), (7, 7)); // at the second match's start
        assert_eq!(run(6, 6), (11, 11));
        assert_eq!(run(7, 7), (11, 11));
        assert_eq!(run(7, 1), (11, 4)); // backwards selection
        assert_eq!(run(1, 6), (4, 11));
        // Shrinking.
        let (t, o, _) = all(
            "aa b aa",
            &q,
            "",
            "\n",
            Selection {
                anchor: 6,
                caret: 1,
                goal_col: None,
            },
        );
        assert_eq!(t, " b ");
        assert_eq!((o.sel.anchor, o.sel.caret), (3, 0));
    }

    #[test]
    fn undo_and_redo_restore_text_and_selection() {
        let text = "ab\r\nab ab\rab\n";
        let mut buf = Buffer::from_text(text);
        let mut hist = History::new();
        let sel = Selection {
            anchor: 9,
            caret: 1,
            goal_col: None,
        };
        let mut counts = EolCounts::of(text);
        let (o, n) = replace_all(
            &mut buf,
            &mut hist,
            sel,
            &lit("ab"),
            "<\n$1>",
            "\r\n",
            None,
            &NO(),
        );
        assert_eq!(n, 4);
        let after = txt(&buf);
        assert_eq!(after, "<\r\n$1>\r\n<\r\n$1> <\r\n$1>\r<\r\n$1>\n");
        check_change(text, &after, o.change.as_ref().unwrap());
        check_eols(&mut counts, &after, o.change.as_ref().unwrap());
        assert_eq!(hist.steps(), (1, 0));
        let u = undo(&mut buf, &mut hist).unwrap();
        assert_eq!(txt(&buf), text);
        assert_eq!(u.sel, sel);
        check_change(&after, text, u.change.as_ref().unwrap());
        check_eols(&mut counts, text, u.change.as_ref().unwrap());
        let r = redo(&mut buf, &mut hist).unwrap();
        assert_eq!(txt(&buf), after);
        assert_eq!(r.sel, o.sel);
        check_eols(&mut counts, &after, r.change.as_ref().unwrap());
        // replace_one the same way.
        let mut buf = Buffer::from_text("x yy");
        let mut hist = History::new();
        let s = Selection {
            anchor: 4,
            caret: 0,
            goal_col: None,
        };
        let o = replace_one(&mut buf, &mut hist, s, &lit("yy"), 2..4, "zzz", "\n");
        assert_eq!(txt(&buf), "x zzz");
        let u = undo(&mut buf, &mut hist).unwrap();
        assert_eq!((txt(&buf).as_str(), u.sel), ("x yy", s));
        let r = redo(&mut buf, &mut hist).unwrap();
        assert_eq!((txt(&buf).as_str(), r.sel), ("x zzz", o.sel));
    }

    #[test]
    fn cancel_changes_nothing() {
        let mut buf = Buffer::from_text("a a a");
        let mut hist = History::new();
        let flag = AtomicBool::new(true);
        let (o, n) = replace_all(
            &mut buf,
            &mut hist,
            Selection::at(2),
            &lit("a"),
            "b",
            "\n",
            None,
            &flag,
        );
        assert_eq!(n, 0);
        assert!(o.change.is_none());
        assert_eq!(txt(&buf), "a a a");
        assert!(!hist.can_undo());
    }

    #[test]
    fn matches_found_before_replacing_do_not_see_the_replacements() {
        // Replacing "a" by "aa" must not loop or rescan its own output.
        let (t, _, n) = all("a a", &lit("a"), "aa", "\n", Selection::at(0));
        assert_eq!((t.as_str(), n), ("aa aa", 2));
        // Whole word was judged on the original neighbours.
        let q = Query::new("a", opts(true, true, false)).unwrap();
        let (t, _, n) = all("a a", &q, "b", "\n", Selection::at(0));
        assert_eq!((t.as_str(), n), ("b b", 2));
        // `$` at the end of each line, an anchored regex on a CRLF document.
        let (t, _, n) = all("a\r\nb\rc\nd", &re("."), "[$0]", "\n", Selection::at(0));
        assert_eq!((t.as_str(), n), ("[a]\r\n[b]\r[c]\n[d]", 4));
    }

    // ---- the scripted run against a plain-String model

    mod model {
        use regex_automata::meta;
        use regex_automata::{Anchored, Input};
        use std::ops::Range;

        pub fn norm(t: &str, eol: &str) -> String {
            let mut out = String::new();
            let mut it = t.chars().peekable();
            while let Some(c) = it.next() {
                match c {
                    '\r' => {
                        if it.peek() == Some(&'\n') {
                            it.next();
                        }
                        out.push_str(eol);
                    }
                    '\n' => out.push_str(eol),
                    c => out.push(c),
                }
            }
            out
        }

        fn word(c: char) -> bool {
            c.is_alphanumeric() || c == '_'
        }

        fn eq(q: char, c: char, match_case: bool) -> bool {
            if match_case {
                return q == c;
            }
            q.eq_ignore_ascii_case(&c) || matches!((q, c), ('i', 'İ') | ('I', 'ı'))
        }

        /// A plain match of `q` (no line breaks) at char index `s` of `chars`: its end index.
        fn at(chars: &[char], s: usize, q: &[char], mc: bool) -> Option<usize> {
            (s + q.len() <= chars.len()
                && q.iter()
                    .enumerate()
                    .all(|(k, &qc)| eq(qc, chars[s + k], mc)))
            .then_some(s + q.len())
        }

        fn bounded(chars: &[char], s: usize, e: usize) -> bool {
            !(s > 0 && word(chars[s - 1])) && !(e < chars.len() && word(chars[e]))
        }

        /// Plain matches in `text[scope]`, counting from the scope's start, as byte ranges.
        pub fn plain(
            text: &str,
            q: &str,
            mc: bool,
            ww: bool,
            scope: Range<usize>,
        ) -> Vec<Range<usize>> {
            let chars: Vec<(usize, char)> = text.char_indices().collect();
            let cs: Vec<char> = chars.iter().map(|x| x.1).collect();
            let q: Vec<char> = q.chars().collect();
            let byte = |i: usize| chars.get(i).map_or(text.len(), |x| x.0);
            let lo = chars
                .iter()
                .position(|x| x.0 >= scope.start)
                .unwrap_or(chars.len());
            let hi = chars
                .iter()
                .position(|x| x.0 >= scope.end)
                .unwrap_or(chars.len());
            let mut out = Vec::new();
            let mut s = lo;
            while s < hi {
                match at(&cs[..hi], s, &q, mc) {
                    Some(e) if !ww || bounded(&cs, s, e) => {
                        out.push(byte(s)..byte(e));
                        s = e;
                    }
                    _ => s += 1,
                }
            }
            out
        }

        pub fn plain_at(text: &str, q: &str, mc: bool, ww: bool, r: &Range<usize>) -> bool {
            if r.start >= r.end
                || r.end > text.len()
                || !text.is_char_boundary(r.start)
                || !text.is_char_boundary(r.end)
            {
                return false;
            }
            let cs: Vec<char> = text.chars().collect();
            let s = text[..r.start].chars().count();
            let e = s + text[r.clone()].chars().count();
            let q: Vec<char> = q.chars().collect();
            at(&cs, s, &q, mc) == Some(e) && (!ww || bounded(&cs, s, e))
        }

        pub fn lines(text: &str) -> Vec<(usize, &str)> {
            let mut out = Vec::new();
            let mut start = 0;
            for l in crate::testutil::lines_of(text) {
                let content = l.trim_end_matches(['\r', '\n']);
                // A lone "\r\n" or "\n" or "\r" ends the line; content has no break chars.
                out.push((start, content));
                start += l.len();
            }
            out
        }

        /// Regex matches wholly inside `scope`, and the groups of each (None: not taking part).
        pub fn regex(
            text: &str,
            re: &meta::Regex,
            scope: Range<usize>,
        ) -> Vec<(Range<usize>, Vec<Option<String>>)> {
            let mut out = Vec::new();
            for (start, line) in lines(text) {
                for caps in re.captures_iter(line) {
                    let m = caps.get_match().unwrap();
                    if m.is_empty() {
                        continue;
                    }
                    let r = start + m.start()..start + m.end();
                    if r.start >= scope.start && r.end <= scope.end {
                        let groups = (0..caps.group_len())
                            .map(|g| caps.get_group(g).map(|s| line[s.range()].to_string()))
                            .collect();
                        out.push((r, groups));
                    }
                }
            }
            out
        }

        pub fn regex_at(text: &str, re: &meta::Regex, r: &Range<usize>) -> bool {
            for (start, line) in lines(text) {
                if r.start >= start && r.start <= start + line.len() {
                    if r.end > start + line.len() || r.start == r.end {
                        return false;
                    }
                    let input = Input::new(line)
                        .span(r.start - start..line.len())
                        .anchored(Anchored::Yes);
                    return re.search(&input).is_some_and(|m| m.end() == r.end - start);
                }
            }
            false
        }

        /// CRLF is one grapheme in this test's alphabet; everything else is a char.
        pub fn snap(t: &str, p: usize) -> usize {
            if p > 0 && p < t.len() && t.as_bytes()[p - 1] == b'\r' && t.as_bytes()[p] == b'\n' {
                p - 1
            } else {
                p
            }
        }

        pub fn prev(t: &str, p: usize) -> usize {
            let q = t[..p].char_indices().next_back().map_or(0, |x| x.0);
            snap(t, q)
        }

        /// Where an original offset `x` goes when each `(range, replacement length)` is applied.
        pub fn map(x: usize, edits: &[(Range<usize>, usize)]) -> usize {
            let mut delta = 0isize;
            for (r, n) in edits {
                if x >= r.end {
                    delta += *n as isize - r.len() as isize;
                } else if x > r.start {
                    return (r.start as isize + delta) as usize + n;
                } else {
                    break;
                }
            }
            (x as isize + delta) as usize
        }
    }

    type Step = (String, Selection, String, Selection);

    const ALPHABET: [char; 22] = [
        'a', 'b', 'a', 'b', 'A', 'B', 'i', 'I', 'İ', 'ı', ' ', ' ', '_', '1', '漢', 'ç', '-', '\n',
        '\n', '\r', '.', 'x',
    ];
    const LIT_QUERIES: [&str; 7] = ["a", "ab", "aba", "i", "I", "B-", "漢"];
    const RE_QUERIES: [&str; 7] = [
        r"a+",
        r"(a)(b)?",
        r"[ab]c?",
        r"\w+",
        r"^.",
        r".$",
        r"(?i)(i)|(b)",
    ];
    const EOLS: [&str; 3] = ["\n", "\r\n", "\r"];
    // The templates and their meaning for a regex query with the groups given.
    const TEMPLATES: [&str; 7] = ["X", "[$0]", "$2-$1", "${1}${1}", "a$$b", "l1\nl2", "$7!"];
    fn expand_model(t: &str, groups: &[Option<String>], whole: &str) -> String {
        let g = |i: usize| -> String {
            if i == 0 {
                whole.to_string()
            } else {
                groups.get(i).cloned().flatten().unwrap_or_default()
            }
        };
        match t {
            "X" => "X".into(),
            "[$0]" => format!("[{}]", g(0)),
            "$2-$1" => format!("{}-{}", g(2), g(1)),
            "${1}${1}" => format!("{}{}", g(1), g(1)),
            "a$$b" => "a$b".into(),
            "l1\nl2" => "l1\nl2".into(),
            "$7!" => "!".into(),
            _ => unreachable!(),
        }
    }

    fn random_sel(rng: &mut Rng, t: &str) -> Selection {
        let pts: Vec<usize> = (0..=t.len())
            .filter(|&p| t.is_char_boundary(p) && model::snap(t, p) == p)
            .collect();
        let a = *rng.pick(&pts);
        let c = if rng.chance(40) { a } else { *rng.pick(&pts) };
        Selection {
            anchor: a,
            caret: c,
            goal_col: None,
        }
    }

    fn scripted(seed: u64, ops: usize) {
        let mut rng = Rng::new(seed);
        let original: String = (0..60 + rng.below(200))
            .map(|_| *rng.pick(&ALPHABET))
            .collect();
        let mut buf = Buffer::from_text(&original);
        let mut hist = History::new();
        let mut counts = EolCounts::of(&original);
        let mut s = Selection::at(0);
        let (mut undos, mut redos): (Vec<Step>, Vec<Step>) = (Vec::new(), Vec::new());
        let mut replaced = 0usize;
        for op in 0..ops {
            let before = txt(&buf);
            if rng.chance(30) {
                s = random_sel(&mut rng, &before);
            }
            let (lo, hi) = (s.anchor.min(s.caret), s.anchor.max(s.caret));
            let ctx0 = format!("seed {seed} op {op} sel {s:?} on {before:?}");
            // (what happened, expected text and selection (None: no change))
            let want: Option<(String, Selection)>;
            let out: Outcome;
            let kind = rng.below(100);
            if kind < 15 {
                // Undo or redo.
                if rng.chance(50) {
                    let got = undo(&mut buf, &mut hist);
                    match undos.pop() {
                        Some(st) => {
                            let o = got.expect(&ctx0);
                            assert_eq!(txt(&buf), st.0, "{ctx0}");
                            assert_eq!(o.sel, st.1, "{ctx0}");
                            check_change(&before, &st.0, o.change.as_ref().unwrap());
                            check_eols(&mut counts, &st.0, o.change.as_ref().unwrap());
                            s = o.sel;
                            redos.push(st);
                        }
                        None => assert!(got.is_none(), "{ctx0}"),
                    }
                } else {
                    let got = redo(&mut buf, &mut hist);
                    match redos.pop() {
                        Some(st) => {
                            let o = got.expect(&ctx0);
                            assert_eq!(txt(&buf), st.2, "{ctx0}");
                            assert_eq!(o.sel, st.3, "{ctx0}");
                            check_change(&before, &st.2, o.change.as_ref().unwrap());
                            check_eols(&mut counts, &st.2, o.change.as_ref().unwrap());
                            s = o.sel;
                            undos.push(st);
                        }
                        None => assert!(got.is_none(), "{ctx0}"),
                    }
                }
                assert_eq!(counts, EolCounts::of(&txt(&buf)), "{ctx0}");
                continue;
            } else if kind < 27 {
                let t: String = (0..1 + rng.below(4))
                    .map(|_| *rng.pick(&ALPHABET))
                    .collect();
                out = insert(&mut buf, &mut hist, s, &t);
                let mut m = before.clone();
                m.replace_range(lo..hi, &t);
                let p = model::snap(&m, lo + t.len());
                want = (m != before).then(|| (m, Selection::at(p)));
            } else if kind < 35 {
                out = backspace(&mut buf, &mut hist, s);
                let (from, to) = if lo != hi {
                    (lo, hi)
                } else {
                    (model::prev(&before, hi), hi)
                };
                let mut m = before.clone();
                m.replace_range(from..to, "");
                want = (m != before).then(|| {
                    let p = model::snap(&m, from);
                    (m, Selection::at(p))
                });
            } else {
                // A find: literal or regex, whole word, case.
                let regex = rng.chance(45);
                let (mc, ww) = (rng.chance(50), !regex && rng.chance(40));
                let qt = if regex {
                    *rng.pick(&RE_QUERIES)
                } else {
                    *rng.pick(&LIT_QUERIES)
                };
                let q = Query::new(qt, opts(mc, ww, regex)).unwrap();
                let eol = *rng.pick(&EOLS);
                let template = *rng.pick(&TEMPLATES);
                let rx = regex.then(|| {
                    meta::Regex::builder()
                        .syntax(regex_automata::util::syntax::Config::new().case_insensitive(!mc))
                        .build(qt)
                        .unwrap()
                });
                // The model's matches over `scope`, with their replacement.
                let model_matches = |scope: Range<usize>| -> Vec<(Range<usize>, String)> {
                    match &rx {
                        Some(re) => model::regex(&before, re, scope)
                            .into_iter()
                            .map(|(r, g)| {
                                let whole = before[r.clone()].to_string();
                                let e = model::norm(&expand_model(template, &g, &whole), eol);
                                (r, e)
                            })
                            .collect(),
                        None => model::plain(&before, qt, mc, ww, scope)
                            .into_iter()
                            .map(|r| (r, model::norm(template, eol)))
                            .collect(),
                    }
                };
                if kind < 60 {
                    // replace_one: a real match most of the time.
                    let all_m = model_matches(0..before.len());
                    let range = if !all_m.is_empty() && rng.chance(75) {
                        all_m[rng.below(all_m.len())].0.clone()
                    } else {
                        let a = random_sel(&mut rng, &before).caret;
                        let b = random_sel(&mut rng, &before).caret;
                        a.min(b)..a.max(b)
                    };
                    let ok = match &rx {
                        Some(re) => model::regex_at(&before, re, &range),
                        None => model::plain_at(&before, qt, mc, ww, &range),
                    };
                    out = replace_one(&mut buf, &mut hist, s, &q, range.clone(), template, eol);
                    // The expansion of this one match, with the groups of the anchored rerun.
                    let rep = match &rx {
                        Some(re) => {
                            let m = &before[range.clone()];
                            let mut caps = re.create_captures();
                            re.search_captures(&Input::new(m).anchored(Anchored::Yes), &mut caps);
                            let g: Vec<Option<String>> = (0..caps.group_len())
                                .map(|i| caps.get_group(i).map(|sp| m[sp.range()].to_string()))
                                .collect();
                            model::norm(&expand_model(template, &g, m), eol)
                        }
                        None => model::norm(template, eol),
                    };
                    want = if ok && before[range.clone()] != rep {
                        let mut m = before.clone();
                        m.replace_range(range.clone(), &rep);
                        let p = model::snap(&m, range.start + rep.len());
                        Some((m, Selection::at(p)))
                    } else {
                        None
                    };
                    if want.is_some() {
                        replaced += 1;
                    }
                } else {
                    let within = rng.chance(35).then_some(lo..hi);
                    let scope = within.clone().unwrap_or(0..before.len());
                    let ms = model_matches(scope);
                    let (o, n) =
                        replace_all(&mut buf, &mut hist, s, &q, template, eol, within, &NO());
                    out = o;
                    assert_eq!(n, ms.len(), "{ctx0} {qt:?} {template:?}");
                    let mut m = String::new();
                    let mut at = 0;
                    for (r, rep) in &ms {
                        m.push_str(&before[at..r.start]);
                        m.push_str(rep);
                        at = r.end;
                    }
                    m.push_str(&before[at..]);
                    let edits: Vec<(Range<usize>, usize)> =
                        ms.iter().map(|(r, e)| (r.clone(), e.len())).collect();
                    let sel = Selection {
                        anchor: model::snap(&m, model::map(s.anchor, &edits)),
                        caret: model::snap(&m, model::map(s.caret, &edits)),
                        goal_col: None,
                    };
                    want = (m != before).then_some((m, sel));
                    replaced += ms.len();
                }
            }
            let ctx = format!("{ctx0} kind {kind}");
            let after = txt(&buf);
            match want {
                None => {
                    assert_eq!(after, before, "{ctx}");
                    assert!(out.change.is_none() && out.sel == s, "{ctx}");
                }
                Some((text, sel)) => {
                    assert_eq!(after, text, "{ctx}");
                    assert_eq!(
                        (out.sel.anchor, out.sel.caret),
                        (sel.anchor, sel.caret),
                        "{ctx}"
                    );
                    let ch = out.change.as_ref().expect(&ctx);
                    check_change(&before, &after, ch);
                    check_eols(&mut counts, &after, ch);
                    hist.seal();
                    undos.push((before, s, after, out.sel));
                    redos.clear();
                    s = out.sel;
                }
            }
            assert_eq!(counts, EolCounts::of(&txt(&buf)), "{ctx}");
        }
        assert!(replaced > 0, "seed {seed}: nothing was ever replaced");
        let last = txt(&buf);
        while undo(&mut buf, &mut hist).is_some() {}
        assert_eq!(txt(&buf), original, "seed {seed}");
        while redo(&mut buf, &mut hist).is_some() {}
        assert_eq!(txt(&buf), last, "seed {seed}");
    }

    #[test]
    fn scripted_run_matches_the_reference_model() {
        for seed in 1..=6 {
            scripted(seed, 1500);
        }
    }

    #[test]
    fn find_all_agrees_with_the_replacement_count() {
        let text = "one two one\ntwo one";
        let q = lit("one");
        let n = find_all(&ropey::Rope::from_str(text), &q, 100, &NO())
            .ranges
            .len();
        let (_, _, k) = all(text, &q, "1", "\n", Selection::at(0));
        assert_eq!(n, k);
    }

    // ---- speed

    /// Prints the time of `replace_all` and of undoing it on the generated 100 MB file, for a
    /// query with about a million matches.
    /// `cargo run --release -q -p mdedit-core --example gen -- lines 100 target/data/100mb.txt`
    /// creates the file.
    #[test]
    #[ignore]
    fn timing_replace_all_on_100mb() {
        use std::time::Instant;
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/data/100mb.txt"
        ));
        let text = std::fs::read_to_string(path).expect("run the gen example first");
        let mut buf = Buffer::from_text(&text);
        let lines = buf.len_lines();
        drop(text);
        let mut hist = History::new();
        let no = NO();
        for (name, q, template) in [
            ("literal", lit("is"), "WAS"),
            ("regex is", re("is"), "WAS"),
            ("regex the", re(r"\bthe\b"), "<$0>"),
            ("regex groups", re(r"(\w+)\s+(\w+)\("), "$2 $1("),
        ] {
            let t = Instant::now();
            let found = find_all(buf.rope(), &q, usize::MAX, &no).ranges.len();
            let t_find = t.elapsed();
            let t = Instant::now();
            let (o, n) = replace_all(
                &mut buf,
                &mut hist,
                Selection::at(0),
                &q,
                template,
                "\n",
                None,
                &no,
            );
            let t_all = t.elapsed();
            println!(
                "{name}: {lines} lines, {found} matches (find_all {t_find:?}); replace_all {n} in {t_all:?}, changed {}",
                o.change.is_some()
            );
            let t = Instant::now();
            undo(&mut buf, &mut hist);
            let t_undo = t.elapsed();
            let t = Instant::now();
            redo(&mut buf, &mut hist);
            println!(
                "  undo {t_undo:?}, redo {:?}, history {} MB",
                t.elapsed(),
                hist.bytes() >> 20
            );
            // Back to the original text for the next query.
            undo(&mut buf, &mut hist);
            hist.clear();
        }
    }
}
