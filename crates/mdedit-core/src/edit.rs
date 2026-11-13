//! The editing commands. Each takes the buffer, the history and the selection, makes its
//! replacements, records them as one undo step and returns the new selection with one merged
//! [`Change`] (`None` when nothing happened, with the selection unchanged). Every returned
//! selection end is on a grapheme boundary of the new text.
//!
//! The commands never copy more than the edit: a removed text over
//! [`UNDO_SHARE_BYTES`](crate::limits::UNDO_SHARE_BYTES) goes to the history as a rope slice
//! that shares the buffer's chunks.

use crate::buffer::{Buffer, Change};
use crate::history::{History, Kind, Record, Text};
use crate::limits::TAB_WIDTH;
use crate::selection::{self, Motion, Selection};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// The result of a command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub sel: Selection,
    /// `None`: nothing happened, and `sel` is the selection that was given.
    pub change: Option<Change>,
}

/// The replacements of one command, collected so they become one undo step.
pub(crate) struct Cmd<'a> {
    pub(crate) buf: &'a mut Buffer,
    records: Vec<Record>,
    change: Option<Change>,
}

impl<'a> Cmd<'a> {
    pub(crate) fn new(buf: &'a mut Buffer) -> Cmd<'a> {
        Cmd {
            buf,
            records: Vec::new(),
            change: None,
        }
    }

    /// Replaces `range` (clamped, floored to `char` boundaries) with `text`.
    pub(crate) fn replace(&mut self, range: Range<usize>, text: &str) {
        let at = self.buf.floor_char(range.start);
        let end = self.buf.floor_char(range.end).max(at);
        if at == end && text.is_empty() {
            return;
        }
        let removed = if at == end {
            Text::empty()
        } else {
            Text::of(self.buf.rope(), at..end)
        };
        let c = self.buf.replace(at..end, text);
        self.records.push(Record {
            at,
            removed,
            inserted: text.len(),
            ws: text.chars().next().is_some_and(char::is_whitespace),
        });
        match self.change.as_mut() {
            Some(m) => m.merge(&c),
            None => self.change = Some(c),
        }
    }

    /// Ends the command: records the step and snaps the selection ends to grapheme boundaries.
    pub(crate) fn finish(
        self,
        hist: &mut History,
        kind: Kind,
        before: Selection,
        after: Selection,
    ) -> Outcome {
        let Some(change) = self.change else {
            return Outcome {
                sel: before,
                change: None,
            };
        };
        let sel = Selection {
            anchor: self.buf.snap_to_grapheme(after.anchor),
            caret: self.buf.snap_to_grapheme(after.caret),
            goal_col: after.goal_col,
        };
        hist.commit(kind, before, sel, self.records);
        Outcome {
            sel,
            change: Some(change),
        }
    }
}

pub(crate) fn nothing(sel: Selection) -> Outcome {
    Outcome { sel, change: None }
}

/// The selection's range clamped to the buffer, ends floored to `char` boundaries.
pub(crate) fn clamped(buf: &Buffer, sel: Selection) -> Range<usize> {
    let r = sel.range();
    let start = buf.floor_char(r.start);
    start..buf.floor_char(r.end).max(start)
}

/// `text` is one grapheme and not a line break.
fn is_typed_char(text: &str) -> bool {
    let mut g = text.graphemes(true);
    match (g.next(), g.next()) {
        (Some(first), None) => !first.starts_with(['\r', '\n']),
        _ => false,
    }
}

/// Replaces the selection with `text`, the caret after it. One typed character with no
/// selection is `Kind::Typing` and merges with the characters typed before it; anything else is
/// its own step.
pub fn insert(buf: &mut Buffer, hist: &mut History, sel: Selection, text: &str) -> Outcome {
    let range = clamped(buf, sel);
    if range.is_empty() && text.is_empty() {
        return nothing(sel);
    }
    let kind = if sel.is_empty() && is_typed_char(text) {
        Kind::Typing
    } else {
        Kind::Other
    };
    let at = range.start;
    let mut cmd = Cmd::new(buf);
    cmd.replace(range, text);
    cmd.finish(hist, kind, sel, Selection::at(at + text.len()))
}

/// Replaces the selection with a line break `eol`, then, with `auto_indent`, the spaces and tabs
/// the line holding the selection's start begins with (those before the start only). Its own
/// step.
pub fn newline(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    eol: &str,
    auto_indent: bool,
) -> Outcome {
    let range = clamped(buf, sel);
    let mut text = String::from(eol);
    if auto_indent {
        let line_start = buf.line_start(buf.line_of(range.start));
        'scan: for piece in buf.chunks(line_start..range.start) {
            for c in piece.chars() {
                if c != ' ' && c != '\t' {
                    break 'scan;
                }
                text.push(c);
            }
        }
    }
    let at = range.start;
    let mut cmd = Cmd::new(buf);
    cmd.replace(range, &text);
    cmd.finish(hist, Kind::Other, sel, Selection::at(at + text.len()))
}

/// Removes `range` and puts the caret at its start.
fn remove(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    range: Range<usize>,
    kind: Kind,
) -> Outcome {
    let at = range.start;
    let mut cmd = Cmd::new(buf);
    cmd.replace(range, "");
    cmd.finish(hist, kind, sel, Selection::at(at))
}

/// Removes the selection, or the grapheme before the caret (a CRLF is one).
pub fn backspace(buf: &mut Buffer, hist: &mut History, sel: Selection) -> Outcome {
    if !sel.is_empty() {
        let r = clamped(buf, sel);
        return remove(buf, hist, sel, r, Kind::Other);
    }
    let caret = buf.floor_char(sel.caret);
    let prev = buf.prev_grapheme(caret);
    if prev == caret {
        return nothing(sel);
    }
    remove(buf, hist, sel, prev..caret, Kind::Backspace)
}

/// Removes the selection, or the grapheme after the caret (a CRLF is one).
pub fn delete(buf: &mut Buffer, hist: &mut History, sel: Selection) -> Outcome {
    if !sel.is_empty() {
        let r = clamped(buf, sel);
        return remove(buf, hist, sel, r, Kind::Other);
    }
    let caret = buf.floor_char(sel.caret);
    let next = buf.next_grapheme(caret);
    if next == caret {
        return nothing(sel);
    }
    remove(buf, hist, sel, caret..next, Kind::Delete)
}

fn delete_word(buf: &mut Buffer, hist: &mut History, sel: Selection, motion: Motion) -> Outcome {
    if !sel.is_empty() {
        let r = clamped(buf, sel);
        return remove(buf, hist, sel, r, Kind::Other);
    }
    let caret = buf.floor_char(sel.caret);
    let to = selection::apply(buf, Selection::at(caret), motion, false, TAB_WIDTH).caret;
    if to == caret {
        return nothing(sel);
    }
    remove(buf, hist, sel, to.min(caret)..to.max(caret), Kind::Other)
}

/// Removes the selection, or from the start of the word before the caret to the caret.
pub fn delete_word_left(buf: &mut Buffer, hist: &mut History, sel: Selection) -> Outcome {
    delete_word(buf, hist, sel, Motion::WordLeft)
}

/// Removes the selection, or from the caret to the end of the word after it.
pub fn delete_word_right(buf: &mut Buffer, hist: &mut History, sel: Selection) -> Outcome {
    delete_word(buf, hist, sel, Motion::WordRight)
}

/// The lines the selection touches: from the one holding its start to the one holding its end,
/// except that a line the selection only ends at column 0 of is not touched (a selection of whole
/// lines, ending at the start of the next, touches those lines only).
pub(crate) fn touched_lines(buf: &Buffer, r: &Range<usize>) -> (usize, usize) {
    let first = buf.line_of(r.start);
    let mut last = buf.line_of(r.end);
    if last > first && r.end == buf.line_start(last) {
        last -= 1;
    }
    (first, last)
}

/// With a selection over several lines, inserts a tab at the start of each touched line that is
/// not empty; the selection keeps covering the same text, and the tabs on its lines. Otherwise
/// the same as [`insert`] of a tab. `tab_width` is the editor's setting; a tab is always `\t`.
pub fn indent(buf: &mut Buffer, hist: &mut History, sel: Selection, _tab_width: usize) -> Outcome {
    let r = clamped(buf, sel);
    if buf.line_of(r.start) == buf.line_of(r.end) {
        return insert(buf, hist, sel, "\t");
    }
    let (first, last) = touched_lines(buf, &r);
    let starts: Vec<usize> = (first..=last)
        .filter(|&l| !buf.line_range(l).is_empty())
        .map(|l| buf.line_start(l))
        .collect();
    let mut cmd = Cmd::new(buf);
    for &s in starts.iter().rev() {
        cmd.replace(s..s, "\t");
    }
    // A position moves right by the tabs inserted before it; one at a line start stays in front
    // of that line's tab.
    let map = |p: usize| p + starts.partition_point(|&s| s < p);
    let after = Selection {
        anchor: map(sel.anchor),
        caret: map(sel.caret),
        goal_col: None,
    };
    cmd.finish(hist, Kind::Other, sel, after)
}

/// How many bytes of leading whitespace `outdent` removes from `line`: one tab, or else up to
/// `tab_width` spaces.
fn outdent_len(buf: &Buffer, line: usize, tab_width: usize) -> usize {
    let r = buf.line_range(line);
    let limit = r.end.min(r.start + tab_width.max(1));
    let mut bytes = buf.chunks(r.start..limit).flat_map(str::bytes);
    match bytes.next() {
        Some(b'\t') => 1,
        Some(b' ') => 1 + bytes.take_while(|&b| b == b' ').count(),
        _ => 0,
    }
}

/// Removes one leading tab, or else up to `tab_width` leading spaces, from every touched line
/// (the caret's when there is no selection). A selection end inside the removed whitespace goes
/// to the line start; the others move with their text. `change` is `None` when no line had
/// whitespace to remove.
pub fn outdent(buf: &mut Buffer, hist: &mut History, sel: Selection, tab_width: usize) -> Outcome {
    let r = clamped(buf, sel);
    let (first, last) = touched_lines(buf, &r);
    let removals: Vec<(usize, usize)> = (first..=last)
        .filter_map(|l| {
            let n = outdent_len(buf, l, tab_width);
            (n > 0).then(|| (buf.line_start(l), n))
        })
        .collect();
    if removals.is_empty() {
        return nothing(sel);
    }
    let mut cmd = Cmd::new(buf);
    for &(s, n) in removals.iter().rev() {
        cmd.replace(s..s + n, "");
    }
    let map = |p: usize| {
        let cut: usize = removals
            .iter()
            .take_while(|&&(s, _)| s < p)
            .map(|&(s, n)| n.min(p - s))
            .sum();
        p - cut
    };
    let after = Selection {
        anchor: map(sel.anchor),
        caret: map(sel.caret),
        goal_col: None,
    };
    cmd.finish(hist, Kind::Other, sel, after)
}

/// Applies the records of a step the other way, last first, returning the flipped records (what
/// the replacements took out of the buffer, ready for the opposite move) and the merged change.
fn flip(buf: &mut Buffer, records: Vec<Record>) -> (Vec<Record>, Change) {
    let mut flipped = Vec::with_capacity(records.len());
    let mut change: Option<Change> = None;
    for rec in records.into_iter().rev() {
        let at = buf.floor_char(rec.at);
        let end = buf.floor_char(rec.at + rec.inserted).max(at);
        let taken = Text::of(buf.rope(), at..end);
        let c = match &rec.removed {
            Text::Small(s) => buf.replace(at..end, s),
            Text::Large(rope) => buf.replace_rope(at..end, rope),
        };
        flipped.push(Record {
            at,
            inserted: rec.removed.len(),
            removed: taken,
            ws: false,
        });
        match change.as_mut() {
            Some(m) => m.merge(&c),
            None => change = Some(c),
        }
    }
    (flipped, change.unwrap_or_default())
}

fn snapped(buf: &Buffer, sel: Selection) -> Selection {
    Selection {
        anchor: buf.snap_to_grapheme(sel.anchor),
        caret: buf.snap_to_grapheme(sel.caret),
        goal_col: sel.goal_col,
    }
}

/// Undoes the newest step: the selection comes back as it was before the step. `None` when
/// there is nothing to undo.
pub fn undo(buf: &mut Buffer, hist: &mut History) -> Option<Outcome> {
    let mut step = hist.pop_undo()?;
    let records = std::mem::take(&mut step.records);
    let (flipped, change) = flip(buf, records);
    step.set_records(flipped);
    let sel = snapped(buf, step.sel_before);
    hist.push_redo(step);
    Some(Outcome {
        sel,
        change: Some(change),
    })
}

/// Redoes the step last undone: the selection is the one after the step. `None` when there is
/// nothing to redo.
pub fn redo(buf: &mut Buffer, hist: &mut History) -> Option<Outcome> {
    let mut step = hist.pop_redo()?;
    let records = std::mem::take(&mut step.records);
    let (flipped, change) = flip(buf, records);
    step.set_records(flipped);
    let sel = snapped(buf, step.sel_after);
    hist.push_undo(step);
    Some(Outcome {
        sel,
        change: Some(change),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eol::EolCounts;
    use crate::testutil::{Rng, check_change, check_eols, lines_of};

    fn setup(text: &str) -> (Buffer, History) {
        (Buffer::from_text(text), History::new())
    }

    fn txt(buf: &Buffer) -> String {
        buf.rope().to_string()
    }

    fn sel(anchor: usize, caret: usize) -> Selection {
        Selection {
            anchor,
            caret,
            goal_col: None,
        }
    }

    // ---- insert

    #[test]
    fn insert_in_an_empty_buffer_and_at_both_ends() {
        let (mut buf, mut hist) = setup("");
        let o = insert(&mut buf, &mut hist, Selection::at(0), "a");
        assert_eq!(txt(&buf), "a");
        assert_eq!(o.sel, Selection::at(1));
        assert_eq!(buf.revision(), 1);
        let o = insert(&mut buf, &mut hist, Selection::at(0), "<");
        assert_eq!(txt(&buf), "<a");
        assert_eq!(o.sel, Selection::at(1));
        let o = insert(&mut buf, &mut hist, Selection::at(2), ">");
        assert_eq!(txt(&buf), "<a>");
        assert_eq!(o.sel, Selection::at(3));
        // Nothing to insert, nothing selected.
        let rev = buf.revision();
        let o = insert(&mut buf, &mut hist, sel(1, 1), "");
        assert_eq!(
            o,
            Outcome {
                sel: sel(1, 1),
                change: None
            }
        );
        assert_eq!(buf.revision(), rev);
    }

    #[test]
    fn insert_replaces_the_selection_in_either_direction() {
        for (a, c) in [(1, 4), (4, 1)] {
            let (mut buf, mut hist) = setup("abcdef");
            let o = insert(&mut buf, &mut hist, sel(a, c), "XY");
            assert_eq!(txt(&buf), "aXYef");
            assert_eq!(o.sel, Selection::at(3));
            let ch = o.change.unwrap();
            assert_eq!((ch.at, ch.removed, ch.inserted), (1, 3, 2));
            let u = undo(&mut buf, &mut hist).unwrap();
            assert_eq!(txt(&buf), "abcdef");
            assert_eq!(u.sel, sel(a, c));
        }
        // Empty text deletes the selection.
        let (mut buf, mut hist) = setup("abcdef");
        let o = insert(&mut buf, &mut hist, sel(5, 2), "");
        assert_eq!(txt(&buf), "abf");
        assert_eq!(o.sel, Selection::at(2));
    }

    #[test]
    fn insert_text_with_other_alphabets() {
        let (mut buf, mut hist) = setup("İstanbul ığ 漢字");
        let o = insert(&mut buf, &mut hist, Selection::at("İstanbul".len()), "ç");
        assert_eq!(txt(&buf), "İstanbulç ığ 漢字");
        assert_eq!(o.sel.caret, "İstanbulç".len());
        let end = buf.len_bytes();
        let o = insert(&mut buf, &mut hist, Selection::at(end), "か");
        assert_eq!(txt(&buf), "İstanbulç ığ 漢字か");
        assert_eq!(o.sel.caret, buf.len_bytes());
        // A caret inside a char is floored to its start.
        let o = insert(&mut buf, &mut hist, Selection::at(1), "_");
        assert_eq!(txt(&buf), "_İstanbulç ığ 漢字か");
        assert_eq!(o.sel.caret, 1);
    }

    #[test]
    fn caret_after_inserted_text_that_joins_a_cluster_is_snapped() {
        // "e" typed before a combining acute makes the cluster "e\u{301}": the caret cannot sit
        // inside it, so it goes to the cluster's start.
        let (mut buf, mut hist) = setup("\u{301}");
        let o = insert(&mut buf, &mut hist, Selection::at(0), "e");
        assert_eq!(txt(&buf), "e\u{301}");
        assert_eq!(o.sel, Selection::at(0));
        // CR typed before an LF: the caret goes before the new CRLF.
        let (mut buf, mut hist) = setup("\n");
        let o = insert(&mut buf, &mut hist, Selection::at(0), "\r");
        assert_eq!(txt(&buf), "\r\n");
        assert_eq!(o.sel, Selection::at(0));
    }

    // ---- backspace and delete

    #[test]
    fn backspace_and_delete_treat_crlf_as_one() {
        let (mut buf, mut hist) = setup("a\r\nb");
        let o = backspace(&mut buf, &mut hist, Selection::at(3));
        assert_eq!(txt(&buf), "ab");
        assert_eq!(o.sel, Selection::at(1));
        assert_eq!(o.change.unwrap().removed, 2);
        let (mut buf, mut hist) = setup("a\r\nb");
        let o = delete(&mut buf, &mut hist, Selection::at(1));
        assert_eq!(txt(&buf), "ab");
        assert_eq!(o.sel, Selection::at(1));
        // A lone CR and a lone LF are one byte each.
        let (mut buf, mut hist) = setup("a\rb\nc");
        backspace(&mut buf, &mut hist, Selection::at(2));
        assert_eq!(txt(&buf), "ab\nc");
        delete(&mut buf, &mut hist, Selection::at(2));
        assert_eq!(txt(&buf), "abc");
    }

    #[test]
    fn backspace_and_delete_remove_whole_clusters() {
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let text = format!("xe\u{301}{family}y");
        let (mut buf, mut hist) = setup(&text);
        let end_family = 1 + 3 + family.len();
        let o = backspace(&mut buf, &mut hist, Selection::at(end_family));
        assert_eq!(txt(&buf), "xe\u{301}y");
        assert_eq!(o.sel, Selection::at(4));
        let o = backspace(&mut buf, &mut hist, Selection::at(4));
        assert_eq!(txt(&buf), "xy");
        assert_eq!(o.sel, Selection::at(1));
        let (mut buf, mut hist) = setup(&text);
        let o = delete(&mut buf, &mut hist, Selection::at(1));
        assert_eq!(txt(&buf), format!("x{family}y"));
        assert_eq!(o.sel, Selection::at(1));
        let o = delete(&mut buf, &mut hist, Selection::at(1));
        assert_eq!(txt(&buf), "xy");
        assert_eq!(o.sel, Selection::at(1));
        // Regional indicators pair up: a flag goes in one.
        let (mut buf, mut hist) = setup("\u{1F1F9}\u{1F1F7}\u{1F1E9}\u{1F1EA}");
        delete(&mut buf, &mut hist, Selection::at(0));
        assert_eq!(txt(&buf), "\u{1F1E9}\u{1F1EA}");
    }

    #[test]
    fn backspace_and_delete_with_a_selection_remove_it() {
        for (a, c) in [(2, 5), (5, 2)] {
            let (mut buf, mut hist) = setup("abcdefg");
            let o = backspace(&mut buf, &mut hist, sel(a, c));
            assert_eq!(txt(&buf), "abfg");
            assert_eq!(o.sel, Selection::at(2));
            let (mut buf, mut hist) = setup("abcdefg");
            let o = delete(&mut buf, &mut hist, sel(a, c));
            assert_eq!(txt(&buf), "abfg");
            assert_eq!(o.sel, Selection::at(2));
        }
    }

    #[test]
    fn commands_at_the_ends_and_in_an_empty_buffer_do_nothing() {
        let (mut buf, mut hist) = setup("");
        for o in [
            backspace(&mut buf, &mut hist, Selection::at(0)),
            delete(&mut buf, &mut hist, Selection::at(0)),
            delete_word_left(&mut buf, &mut hist, Selection::at(0)),
            delete_word_right(&mut buf, &mut hist, Selection::at(0)),
            outdent(&mut buf, &mut hist, Selection::at(0), 4),
        ] {
            assert_eq!(
                o,
                Outcome {
                    sel: Selection::at(0),
                    change: None
                }
            );
        }
        assert!(undo(&mut buf, &mut hist).is_none());
        assert!(redo(&mut buf, &mut hist).is_none());
        assert_eq!(buf.revision(), 0);
        let (mut buf, mut hist) = setup("ab");
        assert!(
            backspace(&mut buf, &mut hist, Selection::at(0))
                .change
                .is_none()
        );
        assert!(
            delete(&mut buf, &mut hist, Selection::at(2))
                .change
                .is_none()
        );
        assert!(
            delete_word_left(&mut buf, &mut hist, Selection::at(0))
                .change
                .is_none()
        );
        assert!(
            delete_word_right(&mut buf, &mut hist, Selection::at(2))
                .change
                .is_none()
        );
        assert!(!hist.can_undo());
        assert!(!hist.is_modified());
    }

    #[test]
    fn indent_in_an_empty_buffer() {
        let (mut buf, mut hist) = setup("");
        let o = indent(&mut buf, &mut hist, Selection::at(0), 4);
        assert_eq!(txt(&buf), "\t");
        assert_eq!(o.sel, Selection::at(1));
    }

    // ---- newline

    #[test]
    fn newline_with_and_without_auto_indent() {
        let (mut buf, mut hist) = setup("  \tfoo");
        let o = newline(&mut buf, &mut hist, Selection::at(6), "\n", true);
        assert_eq!(txt(&buf), "  \tfoo\n  \t");
        assert_eq!(o.sel, Selection::at(10));
        // Only the indent before the caret is copied.
        let (mut buf, mut hist) = setup("  \tfoo");
        let o = newline(&mut buf, &mut hist, Selection::at(2), "\r\n", true);
        assert_eq!(txt(&buf), "  \r\n  \tfoo");
        assert_eq!(o.sel, Selection::at(6));
        let (mut buf, mut hist) = setup("  foo");
        let o = newline(&mut buf, &mut hist, Selection::at(5), "\r", false);
        assert_eq!(txt(&buf), "  foo\r");
        assert_eq!(o.sel, Selection::at(6));
        // The line of the selection's start, on a later line, replacing the selection.
        let (mut buf, mut hist) = setup("a\r\n  b cde");
        let o = newline(&mut buf, &mut hist, sel(9, 6), "\n", true);
        assert_eq!(txt(&buf), "a\r\n  b\n  e");
        assert_eq!(o.sel, Selection::at(9));
        // Start of the buffer, end of the buffer.
        let (mut buf, mut hist) = setup("x");
        newline(&mut buf, &mut hist, Selection::at(0), "\n", true);
        assert_eq!(txt(&buf), "\nx");
        let o = newline(&mut buf, &mut hist, Selection::at(2), "\n", true);
        assert_eq!(txt(&buf), "\nx\n");
        assert_eq!(o.sel, Selection::at(3));
    }

    #[test]
    fn newline_is_its_own_undo_step() {
        let (mut buf, mut hist) = setup("");
        for c in ["a", "b"] {
            let at = buf.len_bytes();
            insert(&mut buf, &mut hist, Selection::at(at), c);
        }
        let at = buf.len_bytes();
        newline(&mut buf, &mut hist, Selection::at(at), "\n", false);
        let at = buf.len_bytes();
        insert(&mut buf, &mut hist, Selection::at(at), "c");
        assert_eq!(hist.steps().0, 3);
        undo(&mut buf, &mut hist);
        undo(&mut buf, &mut hist);
        assert_eq!(txt(&buf), "ab");
    }

    // ---- words

    #[test]
    fn delete_words() {
        let (mut buf, mut hist) = setup("foo bar  baz");
        let o = delete_word_left(&mut buf, &mut hist, Selection::at(12));
        assert_eq!(txt(&buf), "foo bar  ");
        assert_eq!(o.sel, Selection::at(9));
        let o = delete_word_left(&mut buf, &mut hist, o.sel);
        assert_eq!(txt(&buf), "foo ");
        assert_eq!(o.sel, Selection::at(4));
        let o = delete_word_right(&mut buf, &mut hist, Selection::at(0));
        assert_eq!(txt(&buf), " ");
        assert_eq!(o.sel, Selection::at(0));
        // With a selection, the selection goes.
        let (mut buf, mut hist) = setup("foo bar baz");
        let o = delete_word_right(&mut buf, &mut hist, sel(6, 2));
        assert_eq!(txt(&buf), "for baz");
        assert_eq!(o.sel, Selection::at(2));
    }

    #[test]
    fn delete_words_in_turkish_and_cjk() {
        let (mut buf, mut hist) = setup("çalışma dünya");
        let n = buf.len_bytes();
        delete_word_left(&mut buf, &mut hist, Selection::at(n));
        assert_eq!(txt(&buf), "çalışma ");
        let n = buf.len_bytes();
        delete_word_left(&mut buf, &mut hist, Selection::at(n));
        assert_eq!(txt(&buf), "");
        let (mut buf, mut hist) = setup("漢字、かな");
        delete_word_right(&mut buf, &mut hist, Selection::at(0));
        assert_eq!(txt(&buf), "、かな");
        let o = delete_word_right(&mut buf, &mut hist, Selection::at(0));
        assert_eq!(txt(&buf), "かな");
        assert_eq!(o.sel, Selection::at(0));
        // A line break is whitespace: it goes with the word before it.
        let (mut buf, mut hist) = setup("ab\r\ncd");
        let o = delete_word_left(&mut buf, &mut hist, Selection::at(4));
        assert_eq!(txt(&buf), "cd");
        assert_eq!(o.sel, Selection::at(0));
    }

    // ---- indent and outdent

    #[test]
    fn indent_a_selection_over_lines() {
        let (mut buf, mut hist) = setup("aa\nbb\n\ncc\ndd");
        // From inside "aa" to the start of "dd": lines 0, 1, 2 (empty, skipped), 3.
        let o = indent(&mut buf, &mut hist, sel(1, 10), 4);
        assert_eq!(txt(&buf), "\taa\n\tbb\n\n\tcc\ndd");
        assert_eq!(o.sel, sel(2, 13));
        let ch = o.change.unwrap();
        assert_eq!((ch.first_line, ch.lines_before, ch.lines_after), (0, 4, 4));
        // One undo step.
        let u = undo(&mut buf, &mut hist).unwrap();
        assert_eq!(txt(&buf), "aa\nbb\n\ncc\ndd");
        assert_eq!(u.sel, sel(1, 10));
        assert!(!hist.can_undo());
        // Backwards keeps the order.
        let o = indent(&mut buf, &mut hist, sel(10, 1), 4);
        assert_eq!(o.sel, sel(13, 2));
    }

    #[test]
    fn indent_keeps_a_selection_that_starts_at_a_line_start_covering_the_tab() {
        let (mut buf, mut hist) = setup("aa\nbb\ncc");
        let o = indent(&mut buf, &mut hist, sel(3, 8), 4);
        assert_eq!(txt(&buf), "aa\n\tbb\n\tcc");
        assert_eq!(o.sel, sel(3, 10));
        // Ending at column 0 of the next line leaves that line alone.
        let (mut buf, mut hist) = setup("aa\nbb\ncc");
        let o = indent(&mut buf, &mut hist, sel(0, 6), 4);
        assert_eq!(txt(&buf), "\taa\n\tbb\ncc");
        assert_eq!(o.sel, sel(0, 8));
        // A whole line with its break selected is indented, not replaced.
        let (mut buf, mut hist) = setup("aa\nbb");
        indent(&mut buf, &mut hist, sel(0, 3), 4);
        assert_eq!(txt(&buf), "\taa\nbb");
        // Only empty lines: nothing.
        let (mut buf, mut hist) = setup("\n\n\n");
        let o = indent(&mut buf, &mut hist, sel(0, 2), 4);
        assert!(o.change.is_none());
        assert_eq!(o.sel, sel(0, 2));
    }

    #[test]
    fn indent_within_one_line_is_an_insert_of_a_tab() {
        let (mut buf, mut hist) = setup("abcd");
        let o = indent(&mut buf, &mut hist, sel(1, 3), 4);
        assert_eq!(txt(&buf), "a\td");
        assert_eq!(o.sel, Selection::at(2));
        let o = indent(&mut buf, &mut hist, Selection::at(0), 4);
        assert_eq!(txt(&buf), "\ta\td");
        assert_eq!(o.sel, Selection::at(1));
    }

    #[test]
    fn outdent_tabs_and_spaces() {
        let (mut buf, mut hist) = setup("\ta\n    b\n  c\n d\ne");
        let o = outdent(&mut buf, &mut hist, sel(0, 16), 4);
        assert_eq!(txt(&buf), "a\nb\nc\nd\ne");
        // The end was at column 0 of "e": untouched, and it moved back by 1 + 4 + 2 + 1.
        assert_eq!(o.sel, sel(0, 8));
        let ch = o.change.unwrap();
        assert_eq!((ch.first_line, ch.lines_before, ch.lines_after), (0, 4, 4));
        undo(&mut buf, &mut hist);
        assert_eq!(txt(&buf), "\ta\n    b\n  c\n d\ne");
        // At most tab_width spaces; a leading tab goes alone.
        let (mut buf, mut hist) = setup("      x\n\t  y");
        outdent(&mut buf, &mut hist, sel(0, 12), 4);
        assert_eq!(txt(&buf), "  x\n  y");
    }

    #[test]
    fn outdent_the_caret_line_and_ends_inside_the_whitespace() {
        let (mut buf, mut hist) = setup("ab\n    cd");
        let o = outdent(&mut buf, &mut hist, Selection::at(9), 4);
        assert_eq!(txt(&buf), "ab\ncd");
        assert_eq!(o.sel, Selection::at(5));
        // An end inside the removed spaces goes to the line start; the other end moves.
        let (mut buf, mut hist) = setup("ab\n    cd");
        let o = outdent(&mut buf, &mut hist, sel(5, 7), 2);
        assert_eq!(txt(&buf), "ab\n  cd");
        assert_eq!(o.sel, sel(3, 5));
        let (mut buf, mut hist) = setup("ab\n    cd");
        let o = outdent(&mut buf, &mut hist, Selection::at(4), 4);
        assert_eq!(txt(&buf), "ab\ncd");
        assert_eq!(o.sel, Selection::at(3));
        // Nothing to remove: no change.
        let (mut buf, mut hist) = setup("ab\ncd");
        let o = outdent(&mut buf, &mut hist, sel(0, 5), 4);
        assert!(o.change.is_none());
        assert_eq!(o.sel, sel(0, 5));
        assert!(!hist.can_undo());
    }

    // ---- the scripted edit run: a reference model on a plain String

    mod reference {
        use super::*;

        pub fn bounds(t: &str) -> Vec<usize> {
            let mut v: Vec<usize> = t.grapheme_indices(true).map(|(i, _)| i).collect();
            v.push(t.len());
            v
        }

        fn snap(t: &str, p: usize) -> usize {
            *bounds(t).iter().rev().find(|&&b| b <= p).unwrap()
        }

        fn prev_b(t: &str, p: usize) -> usize {
            bounds(t)
                .iter()
                .rev()
                .find(|&&b| b < p)
                .copied()
                .unwrap_or(0)
        }

        fn next_b(t: &str, p: usize) -> usize {
            bounds(t)
                .iter()
                .find(|&&b| b > p)
                .copied()
                .unwrap_or(t.len())
        }

        #[derive(PartialEq, Clone, Copy)]
        enum Class {
            Space,
            Word,
            Other,
        }

        fn class_at(t: &str, p: usize) -> Option<Class> {
            t[p..].chars().next().map(|c| {
                if c.is_whitespace() {
                    Class::Space
                } else if c.is_alphanumeric() || c == '_' {
                    Class::Word
                } else {
                    Class::Other
                }
            })
        }

        fn prev_word(t: &str, i: usize) -> usize {
            let mut pos = i;
            while pos > 0 {
                let p = prev_b(t, pos);
                if class_at(t, p) != Some(Class::Space) {
                    break;
                }
                pos = p;
            }
            if pos == 0 {
                return 0;
            }
            let k = class_at(t, prev_b(t, pos));
            while pos > 0 {
                let p = prev_b(t, pos);
                if class_at(t, p) != k {
                    break;
                }
                pos = p;
            }
            pos
        }

        fn next_word(t: &str, i: usize) -> usize {
            let mut pos = i;
            while pos < t.len() && class_at(t, pos) == Some(Class::Space) {
                pos = next_b(t, pos);
            }
            if pos >= t.len() {
                return pos;
            }
            let k = class_at(t, pos);
            while pos < t.len() && class_at(t, pos) == k {
                pos = next_b(t, pos);
            }
            pos
        }

        fn line_starts(t: &str) -> Vec<usize> {
            let mut at = 0;
            lines_of(t)
                .iter()
                .map(|l| {
                    let s = at;
                    at += l.len();
                    s
                })
                .collect()
        }

        fn line_idx(starts: &[usize], p: usize) -> usize {
            starts.iter().rposition(|&s| s <= p).unwrap()
        }

        fn is_break_char(c: char) -> bool {
            c == '\r' || c == '\n'
        }

        /// The text and the selection, with the commands written the simple way. Each command
        /// returns whether it did anything.
        pub struct Model {
            pub text: String,
            pub anchor: usize,
            pub caret: usize,
        }

        impl Model {
            fn range(&self) -> (usize, usize) {
                (self.anchor.min(self.caret), self.anchor.max(self.caret))
            }

            fn place(&mut self, at: usize) {
                let p = snap(&self.text, at);
                self.anchor = p;
                self.caret = p;
            }

            pub fn insert(&mut self, text: &str) -> bool {
                let (a, b) = self.range();
                if a == b && text.is_empty() {
                    return false;
                }
                self.text.replace_range(a..b, text);
                self.place(a + text.len());
                true
            }

            pub fn newline(&mut self, eol: &str, auto: bool) -> bool {
                let (a, _) = self.range();
                let mut s = eol.to_string();
                if auto {
                    let ls = self.text[..a].rfind(is_break_char).map_or(0, |i| i + 1);
                    s.extend(
                        self.text[ls..a]
                            .chars()
                            .take_while(|&c| c == ' ' || c == '\t'),
                    );
                }
                self.insert(&s)
            }

            fn remove(&mut self, a: usize, b: usize) -> bool {
                self.text.replace_range(a..b, "");
                self.place(a);
                true
            }

            pub fn backspace(&mut self) -> bool {
                let (a, b) = self.range();
                if a != b {
                    return self.remove(a, b);
                }
                let p = prev_b(&self.text, self.caret);
                p != self.caret && self.remove(p, self.caret)
            }

            pub fn delete(&mut self) -> bool {
                let (a, b) = self.range();
                if a != b {
                    return self.remove(a, b);
                }
                let n = next_b(&self.text, self.caret);
                n != self.caret && self.remove(self.caret, n)
            }

            pub fn delete_word(&mut self, left: bool) -> bool {
                let (a, b) = self.range();
                if a != b {
                    return self.remove(a, b);
                }
                let to = if left {
                    prev_word(&self.text, self.caret)
                } else {
                    next_word(&self.text, self.caret)
                };
                to != self.caret && self.remove(to.min(self.caret), to.max(self.caret))
            }

            /// Rewrites lines `first..=last` with `f`, which gives the new text of a line, the
            /// bytes it lost from its start and the bytes it gained there, and maps the
            /// selection ends through the rewrite line by line.
            fn per_line(
                &mut self,
                first: usize,
                last: usize,
                f: impl Fn(&str) -> (String, usize, usize),
            ) -> bool {
                let old = lines_of(&self.text);
                let starts = line_starts(&self.text);
                let mut new_text = String::new();
                let mut new_starts = Vec::new();
                let mut cut = Vec::new();
                for (i, l) in old.iter().enumerate() {
                    new_starts.push(new_text.len());
                    if i >= first && i <= last {
                        let (s, removed, added) = f(l);
                        cut.push((removed, added));
                        new_text.push_str(&s);
                    } else {
                        cut.push((0, 0));
                        new_text.push_str(l);
                    }
                }
                if cut.iter().all(|&c| c == (0, 0)) {
                    return false;
                }
                let map = |p: usize| {
                    let li = line_idx(&starts, p);
                    let col = p - starts[li];
                    let (removed, added) = cut[li];
                    // A tab added at a line start stays behind a position at column 0.
                    new_starts[li] + col.saturating_sub(removed) + if col > 0 { added } else { 0 }
                };
                let (anchor, caret) = (map(self.anchor), map(self.caret));
                self.text = new_text;
                self.anchor = snap(&self.text, anchor);
                self.caret = snap(&self.text, caret);
                true
            }

            fn touched(&self) -> (usize, usize) {
                let starts = line_starts(&self.text);
                let (a, b) = self.range();
                let (fl, mut ll) = (line_idx(&starts, a), line_idx(&starts, b));
                if ll > fl && b == starts[ll] {
                    ll -= 1;
                }
                (fl, ll)
            }

            pub fn indent(&mut self) -> bool {
                let starts = line_starts(&self.text);
                let (a, b) = self.range();
                if line_idx(&starts, a) == line_idx(&starts, b) {
                    return self.insert("\t");
                }
                let (fl, ll) = self.touched();
                self.per_line(fl, ll, |l| {
                    if l.trim_end_matches(is_break_char).is_empty() {
                        (l.to_string(), 0, 0)
                    } else {
                        (format!("\t{l}"), 0, 1)
                    }
                })
            }

            pub fn outdent(&mut self, tab_width: usize) -> bool {
                let (fl, ll) = self.touched();
                self.per_line(fl, ll, |l| {
                    let n = if l.starts_with('\t') {
                        1
                    } else {
                        l.chars().take(tab_width).take_while(|&c| c == ' ').count()
                    };
                    (l[n..].to_string(), n, 0)
                })
            }
        }
    }

    use reference::{Model, bounds};

    const START: &str = "Merhaba dünya!\r\nİstanbul ışık ğüşöç\n\tindented line\rçalışma\r\n  two spaces  \n\
        漢字かな交じり文\n👨‍👩‍👧 family e\u{301}\n\n   \nlast line, no break";

    const CHARS: [&str; 18] = [
        "a",
        "b",
        " ",
        "é",
        "İ",
        "ı",
        "漢",
        "\t",
        "\u{301}",
        "👨‍👩‍👧",
        "\r",
        "\n",
        "e",
        "1",
        "_",
        ".",
        "\u{200D}",
        "ş",
    ];
    const STRINGS: [&str; 8] = [
        "foo bar",
        "\r\n",
        "hello\nworld\r",
        "  x",
        "ğüş iöç",
        "a\r\nb\r\nc",
        "日本語 テキスト",
        "\t\ttabbed\n\t",
    ];
    const EOLS: [&str; 3] = ["\n", "\r\n", "\r"];

    /// Runs random commands on a `Buffer` with a `History` and on the reference model, checking
    /// after each one. With `track` it also follows the history positions: the states reachable
    /// by undo and redo, each with its text, a cursor among them, and the save point.
    struct Run {
        buf: Buffer,
        hist: History,
        rng: Rng,
        sel: Selection,
        counts: EolCounts,
        track: bool,
        timeline: Vec<(u64, String)>,
        cursor: usize,
        saved: Option<u64>,
        fresh: u64,
        /// An explicit seal, save, undo or redo came last: the next edit must start a step.
        sealed: bool,
        first_sel: Option<Selection>,
        edits: usize,
    }

    impl Run {
        fn new(seed: u64, track: bool, copies: usize) -> Run {
            let text = START.repeat(copies);
            Run {
                buf: Buffer::from_text(&text),
                hist: History::new(),
                rng: Rng::new(seed),
                sel: Selection::at(0),
                counts: EolCounts::of(&text),
                track,
                timeline: vec![(0, text)],
                cursor: 0,
                saved: Some(0),
                fresh: 1,
                sealed: false,
                first_sel: None,
                edits: 0,
            }
        }

        fn text(&self) -> String {
            txt(&self.buf)
        }

        /// A random selection on grapheme boundaries, near the old one or anywhere.
        fn random_sel(&mut self) -> Selection {
            let text = self.text();
            let b = bounds(&text);
            let i = if self.rng.chance(50) {
                let c = b
                    .iter()
                    .position(|&x| x >= self.sel.caret)
                    .unwrap_or(b.len() - 1);
                (c + self.rng.below(7)).saturating_sub(3).min(b.len() - 1)
            } else {
                self.rng.below(b.len())
            };
            let j = match self.rng.below(40) {
                0..=15 => i,
                16..=38 => (i + self.rng.below(9)).saturating_sub(4).min(b.len() - 1),
                _ => self.rng.below(b.len()),
            };
            Selection {
                anchor: b[j],
                caret: b[i],
                goal_col: None,
            }
        }

        fn random_text(&mut self) -> String {
            match self.rng.below(10) {
                0..=5 => (*self.rng.pick(&CHARS)).to_string(),
                6..=8 => (*self.rng.pick(&STRINGS)).to_string(),
                _ => (0..self.rng.below(250))
                    .map(|_| *self.rng.pick(&CHARS))
                    .collect(),
            }
        }

        /// One random command, checked against the model.
        fn step(&mut self, with_history_ops: bool) {
            if self.rng.chance(15) {
                self.sel = self.random_sel();
            }
            let sel = self.sel;
            let before = self.text();
            let rev = self.buf.revision();
            let steps = self.hist.steps();
            let mut m = Model {
                text: before.clone(),
                anchor: sel.anchor,
                caret: sel.caret,
            };
            let mut rand_text = self.random_text();
            if before.len() < 1200 {
                rand_text = (0..100 + self.rng.below(300))
                    .map(|_| *self.rng.pick(&CHARS))
                    .collect();
            }
            let (buf, hist) = (&mut self.buf, &mut self.hist);
            let pick = self.rng.below(if with_history_ops { 112 } else { 100 });
            let (name, out, did) = match pick {
                0..=29 => {
                    let t = (*self.rng.pick(&CHARS)).to_string();
                    ("insert char", insert(buf, hist, sel, &t), m.insert(&t))
                }
                30..=37 => {
                    let t = rand_text;
                    ("insert text", insert(buf, hist, sel, &t), m.insert(&t))
                }
                38..=45 => {
                    let eol = *self.rng.pick(&EOLS);
                    let auto = self.rng.chance(70);
                    (
                        "newline",
                        newline(buf, hist, sel, eol, auto),
                        m.newline(eol, auto),
                    )
                }
                46..=63 => ("backspace", backspace(buf, hist, sel), m.backspace()),
                64..=77 => ("delete", delete(buf, hist, sel), m.delete()),
                78..=80 => (
                    "word left",
                    delete_word_left(buf, hist, sel),
                    m.delete_word(true),
                ),
                81..=83 => (
                    "word right",
                    delete_word_right(buf, hist, sel),
                    m.delete_word(false),
                ),
                84..=89 => ("indent", indent(buf, hist, sel, 4), m.indent()),
                90..=95 => {
                    let w = 1 + self.rng.below(8);
                    ("outdent", outdent(buf, hist, sel, w), m.outdent(w))
                }
                96..=99 => {
                    self.hist.seal();
                    self.sealed = true;
                    self.check_state();
                    return;
                }
                100..=103 => {
                    self.history_move(true);
                    return;
                }
                104..=107 => {
                    self.history_move(false);
                    return;
                }
                108..=109 => {
                    self.hist.mark_saved();
                    self.saved = Some(self.timeline[self.cursor].0);
                    self.sealed = true;
                    self.check_state();
                    return;
                }
                _ => {
                    let back = self.rng.chance(50);
                    self.history_move(back);
                    return;
                }
            };
            self.edits += 1;
            let after = self.text();
            let ctx = format!("op {} {name} sel {sel:?} on {before:?}", self.edits);
            assert_eq!(after, m.text, "{ctx}");
            assert_eq!(out.change.is_some(), did, "{ctx}");
            if did {
                let c = out.change.as_ref().unwrap();
                assert_eq!(out.sel.goal_col, None, "{ctx}");
                assert_eq!(
                    (out.sel.anchor, out.sel.caret),
                    (m.anchor, m.caret),
                    "{ctx}"
                );
                let b = bounds(&after);
                assert!(
                    b.contains(&out.sel.anchor) && b.contains(&out.sel.caret),
                    "{ctx}"
                );
                check_change(&before, &after, c);
                check_eols(&mut self.counts, &after, c);
                assert!(self.buf.revision() > rev, "{ctx}");
                self.first_sel.get_or_insert(sel);
                self.sel = out.sel;
                self.track_edit(steps, after.clone(), &ctx);
            } else {
                assert_eq!(out.sel, sel, "{ctx}");
                assert_eq!(self.buf.revision(), rev, "{ctx}");
                assert_eq!(self.hist.steps(), steps, "{ctx}");
            }
            assert_eq!(self.buf.len_bytes(), after.len(), "{ctx}");
            assert_eq!(self.buf.len_lines(), lines_of(&after).len(), "{ctx}");
            assert_eq!(self.counts, EolCounts::of(&after), "{ctx}");
            self.check_state();
        }

        /// After an edit: the history gained a step or merged into the newest.
        fn track_edit(&mut self, before: (usize, usize), after: String, ctx: &str) {
            let now = self.hist.steps();
            assert_eq!(now.1, 0, "redo steps survive an edit: {ctx}");
            if !self.track {
                return;
            }
            self.fresh += 1;
            let id = self.fresh;
            if now.0 == before.0 {
                assert!(
                    !self.sealed,
                    "merged after a seal, save, undo or redo: {ctx}"
                );
                assert!(before.0 > 0, "{ctx}");
                self.timeline[self.cursor] = (id, after);
            } else {
                assert_eq!(now.0, before.0 + 1, "{ctx}");
                self.timeline.truncate(self.cursor + 1);
                self.timeline.push((id, after));
                self.cursor += 1;
            }
            self.sealed = false;
        }

        fn check_state(&self) {
            assert_eq!(self.hist.can_undo(), self.hist.steps().0 > 0);
            assert_eq!(self.hist.can_redo(), self.hist.steps().1 > 0);
            if self.track {
                assert_eq!(self.hist.steps().0, self.cursor);
                assert_eq!(self.hist.steps().1, self.timeline.len() - 1 - self.cursor);
                let modified = self.saved != Some(self.timeline[self.cursor].0);
                assert_eq!(self.hist.is_modified(), modified, "cursor {}", self.cursor);
            }
        }

        fn history_move(&mut self, undo_it: bool) {
            let before = self.text();
            let out = if undo_it {
                undo(&mut self.buf, &mut self.hist)
            } else {
                redo(&mut self.buf, &mut self.hist)
            };
            if !self.track {
                return;
            }
            let can = if undo_it {
                self.cursor > 0
            } else {
                self.cursor + 1 < self.timeline.len()
            };
            assert_eq!(out.is_some(), can);
            let Some(out) = out else { return };
            self.cursor = if undo_it {
                self.cursor - 1
            } else {
                self.cursor + 1
            };
            let after = self.text();
            assert_eq!(after, self.timeline[self.cursor].1, "undo/redo text");
            let c = out.change.as_ref().unwrap();
            check_change(&before, &after, c);
            check_eols(&mut self.counts, &after, c);
            let b = bounds(&after);
            assert!(b.contains(&out.sel.anchor) && b.contains(&out.sel.caret));
            self.sel = out.sel;
            self.sealed = true;
            self.check_state();
        }
    }

    #[test]
    fn scripted_edit_run_matches_the_reference_model() {
        for seed in [1u64, 2, 3] {
            let mut run = Run::new(seed, false, 6);
            let original = run.text();
            let n = if seed == 1 { 20_000 } else { 4_000 };
            for _ in 0..n {
                run.step(false);
            }
            assert!(run.edits >= n - n / 20, "seed {seed}");
            let last = run.text();
            let first_sel = run.first_sel.expect("some edit changed the text");
            // Undo everything.
            let mut last_out = None;
            while let Some(out) = undo(&mut run.buf, &mut run.hist) {
                last_out = Some(out);
            }
            assert_eq!(run.text(), original, "seed {seed}");
            assert!(!run.hist.is_modified(), "seed {seed}");
            assert_eq!(last_out.unwrap().sel, first_sel);
            assert!(!run.hist.can_undo() && run.hist.can_redo());
            // Redo everything.
            while redo(&mut run.buf, &mut run.hist).is_some() {}
            assert_eq!(run.text(), last, "seed {seed}");
            assert!(run.hist.is_modified());
            assert!(!run.hist.can_redo());
        }
    }

    #[test]
    fn scripted_run_with_undo_redo_and_save_points() {
        for seed in [11u64, 12] {
            let mut run = Run::new(seed, true, 6);
            let n = if seed == 11 { 20_000 } else { 5_000 };
            for _ in 0..n {
                run.step(true);
            }
            assert!(run.edits > n / 2, "seed {seed}");
            // Back and forth over what is left.
            while run.cursor > 0 {
                run.history_move(true);
            }
            while run.cursor + 1 < run.timeline.len() {
                run.history_move(false);
            }
        }
    }

    #[test]
    fn scripted_run_on_a_larger_text_keeps_large_removals_as_ropes() {
        // About 20 KB, so some selections are over the 4 KiB kept as shared rope slices.
        let mut run = Run::new(21, true, 90);
        let mut large = false;
        for _ in 0..4_000 {
            let before = run.hist.bytes();
            run.step(true);
            large |= run.hist.bytes() > before + crate::limits::UNDO_SHARE_BYTES;
        }
        assert!(large, "no removal over the sharing size happened");
        while run.cursor > 0 {
            run.history_move(true);
        }
        while run.cursor + 1 < run.timeline.len() {
            run.history_move(false);
        }
    }

    // ---- speed

    /// Prints the time per edit of 10,000 single-character inserts and 10,000 backspaces at
    /// random positions of a 100 MB buffer.
    #[test]
    #[ignore]
    fn timing_edits_in_a_100_mb_buffer() {
        use std::time::{Duration, Instant};
        let mut text = String::with_capacity(101 << 20);
        let mut i = 0;
        while text.len() < 100 << 20 {
            text.push_str(&format!(
                "line {i} ğüşiöç 漢字 e\u{301} some more text to fill\r\n"
            ));
            i += 1;
        }
        let mut buf = Buffer::from_text(&text);
        drop(text);
        let mut hist = History::new();
        let mut rng = Rng::new(99);
        let n = 10_000;
        let len = buf.len_bytes();
        let mut ins = Vec::with_capacity(n);
        let mut del = Vec::with_capacity(n);
        for _ in 0..n {
            let at = buf.snap_to_grapheme(rng.below(len));
            let t = Instant::now();
            let o = insert(&mut buf, &mut hist, Selection::at(at), "x");
            ins.push(t.elapsed());
            assert!(o.change.is_some());
        }
        for _ in 0..n {
            let at = buf.snap_to_grapheme(rng.below(len)).max(1);
            let t = Instant::now();
            let o = backspace(&mut buf, &mut hist, Selection::at(at));
            del.push(t.elapsed());
            assert!(o.change.is_some());
        }
        let stat = |v: &mut Vec<Duration>| {
            let mean = v.iter().sum::<Duration>() / v.len() as u32;
            v.sort();
            (
                mean,
                v[v.len() / 2],
                v[v.len() * 99 / 100],
                *v.last().unwrap(),
            )
        };
        println!("100 MB buffer, {n} edits of each kind at random positions:");
        println!("  insert    (mean, median, p99, max) {:?}", stat(&mut ins));
        println!("  backspace (mean, median, p99, max) {:?}", stat(&mut del));
        println!(
            "  undo steps {}, history bytes {}",
            hist.steps().0,
            hist.bytes()
        );
    }
}
