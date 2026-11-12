//! The selection and caret movement, in buffer byte offsets. Movement steps over grapheme
//! clusters, never into one; words follow the find box's classes (letters, digits and `_`;
//! whitespace; anything else). Up and Down keep a goal display column, so moving through a short
//! line and back returns to the same column.
//!
//! Nothing here walks a line character by character from its start: columns come from the
//! rope's character index and a byte search for tabs, and words stop after
//! [`WORD_LIMIT`](crate::limits::WORD_LIMIT) bytes, so a line or a word of many megabytes costs a
//! search over bytes, not a step per character.

use crate::buffer::Buffer;
use crate::limits::WORD_LIMIT;
use std::ops::Range;

/// `anchor` is where the selection started, `caret` where it ends and the caret is drawn. Both
/// are grapheme boundaries. Equal means no selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Selection {
    pub anchor: usize,
    pub caret: usize,
    /// The display column Up and Down aim for; set by the first vertical move, cleared by any
    /// other motion or click.
    pub goal_col: Option<usize>,
}

impl Selection {
    pub fn at(offset: usize) -> Selection {
        Selection {
            anchor: offset,
            caret: offset,
            goal_col: None,
        }
    }

    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.caret
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    /// Smart Home: to the line's first non-blank character, or to column 0 when already there.
    Home,
    End,
    Up(usize),
    Down(usize),
    DocStart,
    DocEnd,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Other,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Space
    } else if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else {
        Class::Other
    }
}

/// The class of the grapheme starting at `offset` (its first `char` decides).
fn class_at(buf: &Buffer, offset: usize) -> Option<Class> {
    buf.char_at(offset).map(class)
}

/// The start of the word before `i`: skip whitespace, then one run of the same class. Stops
/// [`WORD_LIMIT`] bytes before `i`.
fn prev_word(buf: &Buffer, i: usize) -> usize {
    let floor = i.saturating_sub(WORD_LIMIT);
    let mut pos = i;
    while pos > floor {
        let p = buf.prev_grapheme(pos);
        if class_at(buf, p) != Some(Class::Space) {
            break;
        }
        pos = p;
    }
    if pos <= floor {
        return pos;
    }
    let first = buf.prev_grapheme(pos);
    let k = class_at(buf, first);
    while pos > floor {
        let p = buf.prev_grapheme(pos);
        if class_at(buf, p) != k {
            break;
        }
        pos = p;
    }
    pos
}

/// The end of the word after `i`: skip whitespace, then one run of the same class. Stops
/// [`WORD_LIMIT`] bytes after `i`.
fn next_word(buf: &Buffer, i: usize) -> usize {
    let len = buf.len_bytes();
    let ceil = i.saturating_add(WORD_LIMIT).min(len);
    let mut pos = i;
    while pos < ceil && class_at(buf, pos) == Some(Class::Space) {
        pos = buf.next_grapheme(pos);
    }
    if pos >= ceil {
        return pos;
    }
    let k = class_at(buf, pos);
    while pos < ceil && class_at(buf, pos) == k {
        pos = buf.next_grapheme(pos);
    }
    pos
}

/// The first position of the line that is not a space or tab, or the line's end when the line is
/// blank.
fn first_non_blank(buf: &Buffer, line: usize) -> usize {
    let r = buf.line_range(line);
    let mut at = r.start;
    for piece in buf.chunks(r.clone()) {
        match piece.bytes().position(|b| b != b' ' && b != b'\t') {
            Some(i) => return at + i,
            None => at += piece.len(),
        }
    }
    r.end
}

/// Characters between two byte offsets (char boundaries), from the rope's index.
fn chars_between(buf: &Buffer, a: usize, b: usize) -> usize {
    let rope = buf.rope();
    rope.byte_to_char(b) - rope.byte_to_char(a)
}

/// The tabs in `range`, in order.
fn tabs_in(buf: &Buffer, range: Range<usize>) -> impl Iterator<Item = usize> + '_ {
    let mut at = buf.floor_char(range.start);
    buf.chunks(range).flat_map(move |piece| {
        let base = at;
        at += piece.len();
        memchr::memchr_iter(b'\t', piece.as_bytes()).map(move |i| base + i)
    })
}

/// The display column of `offset` on its line.
fn display_col(buf: &Buffer, offset: usize, tab_width: usize) -> usize {
    let tab = tab_width.max(1);
    let start = buf.line_start(buf.line_of(offset));
    let (mut col, mut from) = (0, start);
    for t in tabs_in(buf, start..offset) {
        col += chars_between(buf, from, t);
        col += tab - col % tab;
        from = t + 1;
    }
    col + chars_between(buf, from, offset)
}

/// The offset on `line` at display column `goal`: the character holding it, or the line's end
/// when the line is shorter. Inside a tab it is the tab's start, as `DisplayLine` gives.
fn offset_at_display_col(buf: &Buffer, line: usize, goal: usize, tab_width: usize) -> usize {
    let tab = tab_width.max(1);
    let r = buf.line_range(line);
    // The goal falls in the run of tab-free characters starting at `from`, at column `col`.
    let in_run = |from: usize, col: usize| {
        let rope = buf.rope();
        let c = rope.byte_to_char(from) + (goal - col);
        buf.snap_to_grapheme(rope.char_to_byte(c))
    };
    let (mut col, mut from) = (0, r.start);
    for t in tabs_in(buf, r.clone()) {
        let n = chars_between(buf, from, t);
        if col + n > goal {
            return in_run(from, col);
        }
        col += n;
        let w = tab - col % tab;
        if col >= goal || col + w > goal {
            return buf.snap_to_grapheme(t);
        }
        col += w;
        from = t + 1;
    }
    if col + chars_between(buf, from, r.end) > goal {
        in_run(from, col)
    } else {
        r.end
    }
}

/// The selection after a motion. With `extend` the anchor stays; without it, Left and Right on a
/// non-empty selection collapse it to its start or end instead of moving.
pub fn apply(
    buf: &Buffer,
    sel: Selection,
    motion: Motion,
    extend: bool,
    tab_width: usize,
) -> Selection {
    let len = buf.len_bytes();
    let caret = sel.caret.min(len);
    let mut goal = None;
    let to = match motion {
        Motion::Left | Motion::Right if !extend && !sel.is_empty() => {
            let r = sel.range();
            let at = if motion == Motion::Left {
                r.start
            } else {
                r.end
            };
            return Selection::at(at.min(len));
        }
        Motion::Left => buf.prev_grapheme(caret),
        Motion::Right => buf.next_grapheme(caret),
        Motion::WordLeft => prev_word(buf, caret),
        Motion::WordRight => next_word(buf, caret),
        Motion::Home => {
            let line = buf.line_of(caret);
            let start = buf.line_start(line);
            let fnb = first_non_blank(buf, line);
            if caret == fnb { start } else { fnb }
        }
        Motion::End => buf.line_range(buf.line_of(caret)).end,
        Motion::Up(n) | Motion::Down(n) => {
            let up = matches!(motion, Motion::Up(_));
            let line = buf.line_of(caret);
            let want = sel
                .goal_col
                .unwrap_or_else(|| display_col(buf, caret, tab_width));
            goal = Some(want);
            let last = buf.len_lines() - 1;
            if up && line == 0 {
                buf.line_start(0)
            } else if !up && line == last {
                buf.line_range(last).end
            } else {
                let target = if up {
                    line.saturating_sub(n)
                } else {
                    (line + n).min(last)
                };
                offset_at_display_col(buf, target, want, tab_width)
            }
        }
        Motion::DocStart => 0,
        Motion::DocEnd => len,
    };
    Selection {
        anchor: if extend { sel.anchor } else { to },
        caret: to,
        goal_col: goal,
    }
}

/// The word, run of whitespace, or single other grapheme at `offset` (double click). At a line's
/// end it is the empty range there. A run reaches at most [`WORD_LIMIT`] bytes each way from
/// `offset`.
pub fn word_at(buf: &Buffer, offset: usize) -> Range<usize> {
    let at = buf.snap_to_grapheme(offset);
    let line = buf.line_of(at);
    let r = buf.line_range(line);
    if at >= r.end {
        return at..at;
    }
    let k = class_at(buf, at);
    if k == Some(Class::Other) {
        return at..buf.next_grapheme(at);
    }
    let floor = r.start.max(at.saturating_sub(WORD_LIMIT));
    let ceil = r.end.min(at.saturating_add(WORD_LIMIT));
    let mut start = at;
    while start > floor {
        let p = buf.prev_grapheme(start);
        if class_at(buf, p) != k {
            break;
        }
        start = p;
    }
    let mut end = at;
    while end < ceil && class_at(buf, end) == k {
        end = buf.next_grapheme(end);
    }
    start..end
}

/// The line holding `offset` with its line break (triple click).
pub fn line_at(buf: &Buffer, offset: usize) -> Range<usize> {
    buf.line_range_with_break(buf.line_of(offset))
}

pub fn select_all(buf: &Buffer) -> Selection {
    Selection {
        anchor: 0,
        caret: buf.len_bytes(),
        goal_col: None,
    }
}

/// A drag selection: from the press's anchor range to the pointer's offset, growing by words
/// (after a double click) or lines (after a triple click) as the press did.
pub fn drag_to(buf: &Buffer, origin: Range<usize>, unit: Unit, offset: usize) -> Selection {
    let offset = buf.snap_to_grapheme(offset);
    let unit_range = |o: usize| match unit {
        Unit::Grapheme => o..o,
        Unit::Word => word_at(buf, o),
        Unit::Line => line_at(buf, o),
    };
    // A pointer inside the origin keeps the origin selected.
    if offset >= origin.start && offset <= origin.end {
        return Selection {
            anchor: origin.start,
            caret: origin.end,
            goal_col: None,
        };
    }
    if offset < origin.start {
        Selection {
            anchor: origin.end,
            caret: unit_range(offset).start,
            goal_col: None,
        }
    } else {
        Selection {
            anchor: origin.start,
            caret: unit_range(offset).end,
            goal_col: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    Grapheme,
    Word,
    Line,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::DisplayLine;

    const TAB: usize = 4;

    fn b(t: &str) -> Buffer {
        Buffer::from_text(t)
    }

    fn mv(buf: &Buffer, sel: Selection, m: Motion, extend: bool) -> Selection {
        apply(buf, sel, m, extend, TAB)
    }

    #[test]
    fn left_right_graphemes() {
        let buf = b("ae\u{301}\r\nz");
        let s = Selection::at(0);
        let s = mv(&buf, s, Motion::Right, false);
        assert_eq!((s.anchor, s.caret), (1, 1));
        let s = mv(&buf, s, Motion::Right, false);
        assert_eq!(s.caret, 4); // over e + acute
        let s = mv(&buf, s, Motion::Right, false);
        assert_eq!(s.caret, 6); // over CRLF
        let s = mv(&buf, s, Motion::Right, false);
        assert_eq!(s.caret, 7);
        let s = mv(&buf, s, Motion::Right, false);
        assert_eq!(s.caret, 7); // at the end
        let s = mv(&buf, s, Motion::Left, false);
        assert_eq!(s.caret, 6);
        let s = mv(&buf, s, Motion::Left, false);
        assert_eq!(s.caret, 4);
        let s = mv(&buf, Selection::at(0), Motion::Left, false);
        assert_eq!(s.caret, 0);
    }

    #[test]
    fn extend_and_collapse() {
        let buf = b("hello world");
        let s = mv(&buf, Selection::at(2), Motion::Right, true);
        assert_eq!((s.anchor, s.caret), (2, 3));
        let s = mv(&buf, s, Motion::Right, true);
        assert_eq!((s.anchor, s.caret), (2, 4));
        // Collapse to the end for Right and to the start for Left, without moving further.
        let r = mv(&buf, s, Motion::Right, false);
        assert_eq!((r.anchor, r.caret), (4, 4));
        let l = mv(&buf, s, Motion::Left, false);
        assert_eq!((l.anchor, l.caret), (2, 2));
        // A backward selection collapses the same way.
        let back = Selection {
            anchor: 6,
            caret: 3,
            goal_col: None,
        };
        assert_eq!(mv(&buf, back, Motion::Left, false).caret, 3);
        assert_eq!(mv(&buf, back, Motion::Right, false).caret, 6);
        // With extend, Left moves the caret.
        let e = mv(&buf, back, Motion::Left, true);
        assert_eq!((e.anchor, e.caret), (6, 2));
        // Other motions do not collapse, they move from the caret.
        let e = mv(&buf, s, Motion::End, false);
        assert_eq!((e.anchor, e.caret), (11, 11));
    }

    #[test]
    fn word_motions() {
        let buf = b("foo  bar_1 + baz\nğışöç_x  end");
        let at = |m, from| mv(&buf, Selection::at(from), m, false).caret;
        assert_eq!(at(Motion::WordRight, 0), 3);
        assert_eq!(at(Motion::WordRight, 3), 10); // spaces, then "bar_1"
        assert_eq!(at(Motion::WordRight, 8), 10);
        assert_eq!(at(Motion::WordRight, 10), 12); // a space, then "+"
        assert_eq!(at(Motion::WordRight, 12), 16);
        // Over the line break, then the Turkish word with `_`.
        let nl = 16;
        assert_eq!(buf.rope().byte(nl), b'\n');
        let word_end = nl + 1 + "ğışöç_x".len();
        assert_eq!(at(Motion::WordRight, nl), word_end);
        assert_eq!(at(Motion::WordRight, word_end), buf.len_bytes());
        assert_eq!(at(Motion::WordLeft, buf.len_bytes()), word_end + 2);
        assert_eq!(at(Motion::WordLeft, word_end + 2), nl + 1);
        assert_eq!(at(Motion::WordLeft, nl + 1), 13);
        assert_eq!(at(Motion::WordLeft, 13), 11);
        assert_eq!(at(Motion::WordLeft, 11), 5);
        assert_eq!(at(Motion::WordLeft, 5), 0);
        assert_eq!(at(Motion::WordLeft, 0), 0);
        assert_eq!(at(Motion::WordRight, buf.len_bytes()), buf.len_bytes());
        // Extend keeps the anchor.
        let e = mv(&buf, Selection::at(0), Motion::WordRight, true);
        assert_eq!((e.anchor, e.caret), (0, 3));
    }

    #[test]
    fn home_end_smart() {
        let buf = b("  \tfoo bar\nbaz\n   \n");
        let at = |m, from| mv(&buf, Selection::at(from), m, false).caret;
        // Line 0: indent is 3 bytes.
        assert_eq!(at(Motion::Home, 7), 3);
        assert_eq!(at(Motion::Home, 3), 0);
        assert_eq!(at(Motion::Home, 0), 3);
        assert_eq!(at(Motion::Home, 1), 3);
        assert_eq!(at(Motion::End, 5), 10);
        assert_eq!(at(Motion::End, 10), 10);
        // A line without indent: Home stays at column 0.
        assert_eq!(at(Motion::Home, 12), 11);
        assert_eq!(at(Motion::Home, 11), 11);
        // A blank line of spaces: the first non-blank is the end.
        let blank_start = 15;
        assert_eq!(at(Motion::Home, blank_start + 1), blank_start + 3);
        assert_eq!(at(Motion::Home, blank_start + 3), blank_start);
        // The empty last line.
        assert_eq!(at(Motion::End, buf.len_bytes()), buf.len_bytes());
        assert_eq!(at(Motion::Home, buf.len_bytes()), buf.len_bytes());
        // End excludes the break of a CRLF line.
        let crlf = b("ab\r\ncd");
        assert_eq!(mv(&crlf, Selection::at(0), Motion::End, false).caret, 2);
        // Shift+Home selects to the target.
        let e = mv(&buf, Selection::at(7), Motion::Home, true);
        assert_eq!((e.anchor, e.caret), (7, 3));
    }

    #[test]
    fn doc_start_end() {
        let buf = b("ab\ncd");
        let s = mv(&buf, Selection::at(3), Motion::DocEnd, false);
        assert_eq!((s.anchor, s.caret), (5, 5));
        let s = mv(&buf, s, Motion::DocStart, true);
        assert_eq!((s.anchor, s.caret), (5, 0));
        let s = mv(&buf, Selection::at(3), Motion::DocStart, false);
        assert_eq!((s.anchor, s.caret), (0, 0));
    }

    #[test]
    fn up_down_with_goal_column() {
        let buf = b("abcdefgh\nab\n\tabcd\nabcdefgh\nlast");
        // Line starts: 0, 9, 12, 18, 27.
        let s = Selection::at(6); // col 6
        let s = mv(&buf, s, Motion::Down(1), false);
        assert_eq!(s.caret, 11); // end of the short line
        assert_eq!(s.goal_col, Some(6));
        // The tab line: columns 0..4 are the tab; 6 is the second letter.
        let s = mv(&buf, s, Motion::Down(1), false);
        assert_eq!(s.caret, 12 + 1 + 2);
        assert_eq!(s.goal_col, Some(6));
        let s = mv(&buf, s, Motion::Down(1), false);
        assert_eq!(s.caret, 18 + 6);
        // And back up through the short line to the first.
        let s = mv(&buf, s, Motion::Up(1), false);
        assert_eq!(s.caret, 12 + 1 + 2);
        let s = mv(&buf, s, Motion::Up(1), false);
        assert_eq!(s.caret, 11);
        let s = mv(&buf, s, Motion::Up(1), false);
        assert_eq!(s.caret, 6);
        // First line: Up goes to the start; goal kept.
        let s = mv(&buf, s, Motion::Up(1), false);
        assert_eq!(s.caret, 0);
        assert_eq!(s.goal_col, Some(6));
        // Any other motion clears the goal.
        let s = mv(&buf, s, Motion::Right, false);
        assert_eq!(s.goal_col, None);
        // Last line: Down goes to the end.
        let s = mv(&buf, Selection::at(28), Motion::Down(1), false);
        assert_eq!(s.caret, buf.len_bytes());
        // Many lines at once, clamped.
        let s = mv(&buf, Selection::at(2), Motion::Down(100), false);
        assert_eq!(s.caret, 27 + 2); // the last line, at the goal column
        let s = mv(&buf, Selection::at(2), Motion::Down(4), false);
        assert_eq!(s.caret, 27 + 2);
        let s = mv(&buf, Selection::at(29), Motion::Up(100), false);
        assert_eq!(s.caret, 2); // line 0 at the goal column
        // Extending with Down keeps the anchor.
        let s = mv(&buf, Selection::at(3), Motion::Down(1), true);
        assert_eq!((s.anchor, s.caret), (3, 11));
    }

    #[test]
    fn caret_stays_on_grapheme_boundaries_vertically() {
        let buf = b("abcdef\ne\u{301}\u{302}x\nghijkl");
        let s = mv(&buf, Selection::at(2), Motion::Down(1), false);
        // Column 2 on line 1 is after "e + 2 marks" (columns count chars, so col 2 is inside
        // the cluster): the caret snaps back to the cluster start.
        assert_eq!(s.caret, 7);
        // Column 1 is inside the same cluster (e + first mark).
        let s = mv(&buf, Selection::at(1), Motion::Down(1), false);
        assert_eq!(s.caret, 7);
        // Column 3 is the `x` after the cluster.
        let s = mv(&buf, Selection::at(3), Motion::Down(1), false);
        assert_eq!(s.caret, 12);
    }

    #[test]
    fn goal_column_agrees_with_display_line() {
        let text = "a\tb\u{1}\u{7F}c\td漢e";
        let buf = b(text);
        let mut d = DisplayLine::new();
        d.build(text, 0, 0, TAB);
        let mut o = 0;
        while o < text.len() {
            let col = display_col(&buf, o, TAB);
            assert_eq!(col, d.column_of(o), "offset {o}");
            for goal in 0..d.columns + 3 {
                assert_eq!(
                    offset_at_display_col(&buf, 0, goal, TAB),
                    d.offset_at_column(goal),
                    "goal {goal}"
                );
            }
            o = buf.next_grapheme(o);
        }
    }

    #[test]
    fn long_words_stop_at_the_limit() {
        let blob = "a".repeat(3 * WORD_LIMIT);
        let buf = b(&format!("x {blob} y"));
        let mid = 2 + blob.len() / 2;
        let w = word_at(&buf, mid);
        assert_eq!(w, mid - WORD_LIMIT..mid + WORD_LIMIT);
        let at = |m, from| mv(&buf, Selection::at(from), m, false).caret;
        assert_eq!(at(Motion::WordRight, 2), 2 + WORD_LIMIT);
        assert_eq!(
            at(Motion::WordLeft, 2 + blob.len()),
            2 + blob.len() - WORD_LIMIT
        );
        // A word shorter than the limit is whole.
        assert_eq!(at(Motion::WordRight, 2 + blob.len() - 10), 2 + blob.len());
        // Whitespace runs stop at the limit too.
        let spaces = b(&" ".repeat(2 * WORD_LIMIT));
        assert_eq!(
            mv(&spaces, Selection::at(0), Motion::WordRight, false).caret,
            WORD_LIMIT
        );
    }

    #[test]
    fn columns_on_long_lines_with_tabs() {
        // Columns from the rope's index agree with a walk over the characters.
        let mut s = String::new();
        for i in 0..3000 {
            s.push_str(if i % 7 == 0 { "\tğ漢" } else { "ab" });
        }
        let buf = b(&s);
        let mut d = DisplayLine::new();
        d.build(&s, 0, 0, TAB);
        let mut o = 0;
        let mut k = 0;
        while o < s.len() {
            if k % 97 == 0 {
                assert_eq!(display_col(&buf, o, TAB), d.column_of(o), "offset {o}");
            }
            o = buf.next_grapheme(o);
            k += 1;
        }
        for goal in (0..d.columns + 5).step_by(13) {
            assert_eq!(
                offset_at_display_col(&buf, 0, goal, TAB),
                d.offset_at_column(goal),
                "goal {goal}"
            );
        }
    }

    #[test]
    fn word_at_cases() {
        let buf = b("foo  bar_1+ğış\nnext");
        assert_eq!(word_at(&buf, 1), 0..3);
        assert_eq!(word_at(&buf, 0), 0..3);
        assert_eq!(word_at(&buf, 3), 3..5); // the spaces
        assert_eq!(word_at(&buf, 4), 3..5);
        assert_eq!(word_at(&buf, 5), 5..10);
        assert_eq!(word_at(&buf, 10), 10..11); // "+" alone
        assert_eq!(word_at(&buf, 12), 11..17); // inside ğış
        // At the end of a line: empty.
        let eol = 17;
        assert_eq!(buf.rope().byte(eol), b'\n');
        assert_eq!(word_at(&buf, eol), eol..eol);
        assert_eq!(word_at(&buf, 1000), buf.len_bytes()..buf.len_bytes());
        assert_eq!(word_at(&buf, eol + 1), 18..22);
        // Words stop at line breaks, also whitespace runs.
        let ws = b("a  \n  b");
        assert_eq!(word_at(&ws, 1), 1..3);
        assert_eq!(word_at(&ws, 3), 3..3);
        assert_eq!(word_at(&ws, 5), 4..6);
        // A cluster is one grapheme.
        let g = b("e\u{301}!\u{301}x");
        assert_eq!(word_at(&g, 0), 0..3);
        assert_eq!(word_at(&g, 3), 3..6);
        // CRLF end of line.
        let c = b("ab\r\ncd");
        assert_eq!(word_at(&c, 2), 2..2);
        assert_eq!(word_at(&c, 3), 2..2);
        assert_eq!(word_at(&b(""), 0), 0..0);
    }

    #[test]
    fn line_at_and_select_all() {
        let buf = b("ab\r\ncd\nef");
        assert_eq!(line_at(&buf, 1), 0..4);
        assert_eq!(line_at(&buf, 3), 0..4);
        assert_eq!(line_at(&buf, 4), 4..7);
        assert_eq!(line_at(&buf, 8), 7..9); // last line, no break
        assert_eq!(line_at(&buf, 99), 7..9);
        let t = b("ab\n");
        assert_eq!(line_at(&t, 3), 3..3);
        let s = select_all(&buf);
        assert_eq!((s.anchor, s.caret, s.goal_col), (0, 9, None));
        assert_eq!(s.range(), 0..9);
        let e = select_all(&b(""));
        assert!(e.is_empty());
    }

    #[test]
    fn drag_by_graphemes_words_and_lines() {
        let buf = b("one two three\nfour five\nsix");
        // Graphemes.
        let s = drag_to(&buf, 4..4, Unit::Grapheme, 9);
        assert_eq!((s.anchor, s.caret), (4, 9));
        let s = drag_to(&buf, 4..4, Unit::Grapheme, 1);
        assert_eq!((s.anchor, s.caret), (4, 1));
        // Words: origin is "two" (4..7).
        let s = drag_to(&buf, 4..7, Unit::Word, 10); // inside "three" (8..13)
        assert_eq!((s.anchor, s.caret), (4, 13));
        let s = drag_to(&buf, 4..7, Unit::Word, 1); // inside "one"
        assert_eq!((s.anchor, s.caret), (7, 0));
        let s = drag_to(&buf, 4..7, Unit::Word, 5);
        assert_eq!((s.anchor, s.caret), (4, 7));
        let s = drag_to(&buf, 4..7, Unit::Word, 7); // at the origin's edge
        assert_eq!((s.anchor, s.caret), (4, 7));
        let s = drag_to(&buf, 4..7, Unit::Word, 16); // inside "four"
        assert_eq!((s.anchor, s.caret), (4, 18));
        // Lines: origin is line 1 (14..24).
        let s = drag_to(&buf, 14..24, Unit::Line, 20);
        assert_eq!((s.anchor, s.caret), (14, 24));
        let s = drag_to(&buf, 14..24, Unit::Line, 2);
        assert_eq!((s.anchor, s.caret), (24, 0));
        let s = drag_to(&buf, 14..24, Unit::Line, 26);
        assert_eq!((s.anchor, s.caret), (14, 27));
        // Result is a Selection with no goal column and works with `range`.
        assert_eq!(s.range(), 14..27);
        assert_eq!(s.goal_col, None);
    }
}
