//! Copying a change across in a comparison: the lines of one side of a changed block replace the
//! lines of the other side's, as one undo step (`Kind::Other`) like the commands of
//! [`crate::lines`]. The ranges are line ranges as [`crate::diff`] reports them (the empty line
//! after a final line break is not counted).
//!
//! The target keeps its own conventions: the copied lines join with the target's line break
//! `eol`, and the break after the replaced block (or the lack of one at the end of the file) stays
//! as it was. A block copied into an empty place (the other side has nothing there) is a new set of
//! lines before the line the range names; a block copied from an empty place removes the target's
//! lines with their breaks.

use crate::buffer::Buffer;
use crate::diff::line_count;
use crate::edit::{Cmd, Outcome, nothing};
use crate::history::{History, Kind};
use crate::selection::Selection;
use std::ops::Range;

/// Lines taken from a buffer: their text joined with a line break, how many there are (none is
/// not the same as one empty line) and whether the last one had a break after it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lines {
    pub text: String,
    pub count: usize,
    pub ended: bool,
}

/// What [`replace_lines`] is given: the lines of the target to replace and what replaces them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Splice {
    pub target: Range<usize>,
    pub from: Lines,
}

fn has_break(buf: &Buffer, line: usize) -> bool {
    buf.line_range(line).end != buf.line_range_with_break(line).end
}

/// The lines `range` of `buf` (clamped to the lines the comparison sees), joined with `eol`.
pub fn take_lines(buf: &Buffer, range: Range<usize>, eol: &str) -> Lines {
    let n = line_count(buf);
    let end = range.end.min(n);
    if range.start >= end {
        return Lines::default();
    }
    let mut text = String::new();
    let mut piece = String::new();
    for l in range.start..end {
        if l > range.start {
            text.push_str(eol);
        }
        buf.line_text(l, &mut piece);
        text.push_str(&piece);
    }
    Lines {
        text,
        count: end - range.start,
        ended: has_break(buf, end - 1),
    }
}

/// Puts `from` in place of the lines `target` of `buf`. Nothing (no history entry) when both are
/// empty or the text would not change.
pub fn replace_lines(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    target: Range<usize>,
    from: &Lines,
    eol: &str,
) -> Outcome {
    let n = line_count(buf);
    let start_line = target.start.min(n);
    let end_line = target.end.min(n).max(start_line);
    let mut cmd = Cmd::new(buf);
    let at = if from.count == 0 {
        if start_line == end_line {
            return nothing(sel);
        }
        let mut start = cmd.buf.line_start(start_line);
        // The last line of a file with no break at its end: the break before it goes instead, so
        // the file still ends without one.
        if end_line == n && !has_break(cmd.buf, end_line - 1) && start_line > 0 {
            start = cmd.buf.line_range(start_line - 1).end;
        }
        let end = cmd.buf.line_range_with_break(end_line - 1).end;
        cmd.replace(start..end, "");
        start
    } else if start_line < end_line {
        let start = cmd.buf.line_start(start_line);
        let end = cmd.buf.line_range(end_line - 1).end;
        cmd.replace(start..end, &from.text);
        start
    } else {
        let pos = cmd.buf.line_start(start_line);
        let len = cmd.buf.len_bytes();
        let mut ins = String::new();
        let mut caret = pos;
        if pos == len && len == 0 {
            ins.push_str(&from.text);
            if from.ended {
                ins.push_str(eol);
            }
        } else if pos == len && start_line == n && !has_break(cmd.buf, n - 1) {
            // After a last line that has no break: it gets one.
            ins.push_str(eol);
            ins.push_str(&from.text);
            caret += eol.len();
        } else {
            ins.push_str(&from.text);
            ins.push_str(eol);
        }
        cmd.replace(pos..pos, &ins);
        caret
    };
    cmd.finish(hist, Kind::Other, sel, Selection::at(at))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::undo;

    /// Replaces lines `t` of `target` with lines `s` of `source`; returns the new text after
    /// checking that one undo gives the old one back.
    fn copy(target: &str, t: Range<usize>, source: &str, s: Range<usize>) -> String {
        let mut buf = Buffer::from_text(target);
        let from = take_lines(&Buffer::from_text(source), s, "\n");
        let mut hist = History::new();
        let out = replace_lines(&mut buf, &mut hist, Selection::at(0), t, &from, "\n");
        let new = buf.rope().to_string();
        if out.change.is_some() {
            assert!(undo(&mut buf, &mut hist).is_some());
            assert_eq!(buf.rope().to_string(), target);
            assert!(undo(&mut buf, &mut hist).is_none(), "one step");
        } else {
            assert_eq!(new, target);
        }
        new
    }

    #[test]
    fn lines_are_taken_with_the_breaks_of_the_target() {
        let b = Buffer::from_text("a\r\nb\r\nc");
        let l = take_lines(&b, 0..2, "\n");
        assert_eq!((l.text.as_str(), l.count, l.ended), ("a\nb", 2, true));
        let l = take_lines(&b, 1..3, "\r\n");
        assert_eq!((l.text.as_str(), l.count, l.ended), ("b\r\nc", 2, false));
        assert_eq!(take_lines(&b, 2..2, "\n"), Lines::default());
        // One empty line is not no line.
        let l = take_lines(&Buffer::from_text("a\n\nb\n"), 1..2, "\n");
        assert_eq!((l.text.as_str(), l.count), ("", 1));
        // The empty line after the last break is not a line.
        assert_eq!(take_lines(&Buffer::from_text("a\n"), 0..5, "\n").count, 1);
    }

    #[test]
    fn a_changed_block_is_replaced_and_its_break_stays() {
        assert_eq!(
            copy("a\nb\nc\n", 1..2, "a\nX\nY\nc\n", 1..3),
            "a\nX\nY\nc\n"
        );
        assert_eq!(copy("a\nb\nc\n", 0..2, "q\n", 0..1), "q\nc\n");
        // A last line with no break stays without one; with one it keeps it.
        assert_eq!(copy("a\nb", 1..2, "x\ny\n", 0..2), "a\nx\ny");
        assert_eq!(copy("a\nb\n", 1..2, "x\ny", 0..2), "a\nx\ny\n");
        // Another file's line breaks do not come along.
        let mut buf = Buffer::from_text("a\r\nb\r\n");
        let from = take_lines(&Buffer::from_text("x\ny\n"), 0..2, "\r\n");
        let mut hist = History::new();
        replace_lines(&mut buf, &mut hist, Selection::at(0), 1..2, &from, "\r\n");
        assert_eq!(buf.rope().to_string(), "a\r\nx\r\ny\r\n");
    }

    #[test]
    fn lines_the_target_lacks_are_inserted() {
        assert_eq!(copy("a\nc\n", 1..1, "a\nb\nc\n", 1..2), "a\nb\nc\n");
        assert_eq!(copy("a\nc\n", 0..0, "x\ny\n", 0..2), "x\ny\na\nc\n");
        // At the end, after a line with a break, and after one without.
        assert_eq!(copy("a\n", 1..1, "a\nb\n", 1..2), "a\nb\n");
        assert_eq!(copy("a", 1..1, "a\nb\n", 1..2), "a\nb");
        assert_eq!(copy("a", 1..1, "a\nb", 1..2), "a\nb");
        // Into an empty file.
        assert_eq!(copy("", 0..0, "x\ny\n", 0..2), "x\ny\n");
        assert_eq!(copy("", 0..0, "x", 0..1), "x");
        // An empty line.
        assert_eq!(copy("a\nb\n", 1..1, "a\n\nb\n", 1..2), "a\n\nb\n");
    }

    #[test]
    fn lines_the_source_lacks_are_removed() {
        assert_eq!(copy("a\nb\nc\n", 1..2, "a\nc\n", 1..1), "a\nc\n");
        assert_eq!(copy("a\nb\nc\n", 0..3, "", 0..0), "");
        assert_eq!(copy("a\nb\nc\n", 2..3, "a\nb\n", 2..2), "a\nb\n");
        // The file ends without a break and still does.
        assert_eq!(copy("a\nb\nc", 2..3, "a\nb\n", 2..2), "a\nb");
        assert_eq!(copy("a\nb\nc", 1..3, "a\n", 1..1), "a");
        assert_eq!(copy("a", 0..1, "", 0..0), "");
        // Nothing to remove.
        assert_eq!(copy("a\n", 1..1, "a\n", 1..1), "a\n");
    }
}
