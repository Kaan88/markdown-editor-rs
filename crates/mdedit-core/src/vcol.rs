//! Visual columns, for the column (block) selection: a tab counts to its next stop, a wide
//! (East Asian wide or fullwidth) character 2, a combining or other zero-width character 0 and
//! every other character 1. The caret motions keep their own count (a character is one column,
//! see `selection::display_col`); only the column selection measures like this, so a rectangle
//! on screen holds the same columns on every line.

use crate::buffer::Buffer;
use crate::display::{Width, width_of};

fn width(c: char, col: usize, tab_width: usize) -> usize {
    if c == '\t' {
        let tab = tab_width.max(1);
        return tab - col % tab;
    }
    match width_of(c) {
        Width::Zero => 0,
        Width::Narrow => 1,
        Width::Wide => 2,
    }
}

/// The visual column of `offset` on its line (an offset in the line break counts as the line's
/// end).
pub fn visual_col(buf: &Buffer, offset: usize, tab_width: usize) -> usize {
    let line = buf.line_of(offset);
    let range = buf.line_range(line);
    let end = offset.clamp(range.start, range.end);
    let mut col = 0;
    for chunk in buf.chunks(range.start..end) {
        for c in chunk.chars() {
            col += width(c, col, tab_width);
        }
    }
    col
}

/// The offset on `line` at visual column `goal`: the character holding it, or the line's end
/// when the line is shorter. A column inside a wide character or a tab gives its start.
pub fn offset_at_visual_col(buf: &Buffer, line: usize, goal: usize, tab_width: usize) -> usize {
    let range = buf.line_range(line);
    let (mut col, mut offset) = (0, range.start);
    for chunk in buf.chunks(range.clone()) {
        for c in chunk.chars() {
            let w = width(c, col, tab_width);
            // A zero-width character belongs to the one before it: the column after that is
            // after both.
            let attached = w == 0 && offset > range.start;
            if !attached && (col >= goal || col + w > goal) {
                return buf.snap_to_grapheme(offset);
            }
            col += w;
            offset += c.len_utf8();
        }
    }
    range.end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_columns_are_offsets() {
        let buf = Buffer::from_text("abc\ndef");
        assert_eq!(visual_col(&buf, 2, 4), 2);
        assert_eq!(visual_col(&buf, 5, 4), 1);
        assert_eq!(offset_at_visual_col(&buf, 1, 2, 4), 6);
        // Past the end: the line's end, before the break.
        assert_eq!(offset_at_visual_col(&buf, 0, 9, 4), 3);
        assert_eq!(offset_at_visual_col(&buf, 1, 9, 4), 7);
    }

    #[test]
    fn tabs_go_to_their_stops() {
        let buf = Buffer::from_text("a\tb");
        assert_eq!(visual_col(&buf, 2, 4), 4);
        assert_eq!(visual_col(&buf, 3, 4), 5);
        // Columns 1 to 3 are inside the tab: its start; column 4 is the "b".
        for goal in 1..4 {
            assert_eq!(offset_at_visual_col(&buf, 0, goal, 4), 1, "goal {goal}");
        }
        assert_eq!(offset_at_visual_col(&buf, 0, 4, 4), 2);
        assert_eq!(offset_at_visual_col(&buf, 0, 5, 4), 3);
    }

    #[test]
    fn wide_characters_count_two() {
        let buf = Buffer::from_text("漢字ab");
        assert_eq!(visual_col(&buf, 3, 4), 2);
        assert_eq!(visual_col(&buf, 6, 4), 4);
        assert_eq!(visual_col(&buf, 8, 4), 6);
        assert_eq!(offset_at_visual_col(&buf, 0, 2, 4), 3);
        // Column 1 is inside the first character; column 3 inside the second.
        assert_eq!(offset_at_visual_col(&buf, 0, 1, 4), 0);
        assert_eq!(offset_at_visual_col(&buf, 0, 3, 4), 3);
        assert_eq!(offset_at_visual_col(&buf, 0, 4, 4), 6);
        assert_eq!(offset_at_visual_col(&buf, 0, 99, 4), 8);
    }

    #[test]
    fn combining_marks_take_no_column() {
        // "e" and a combining acute accent, then "x".
        let buf = Buffer::from_text("e\u{301}x");
        assert_eq!(visual_col(&buf, 3, 4), 1);
        assert_eq!(visual_col(&buf, 4, 4), 2);
        // Column 1 is after the whole grapheme, never between "e" and its mark.
        assert_eq!(offset_at_visual_col(&buf, 0, 1, 4), 3);
        assert_eq!(offset_at_visual_col(&buf, 0, 0, 4), 0);
        assert_eq!(offset_at_visual_col(&buf, 0, 2, 4), 4);
    }

    #[test]
    fn a_line_break_is_the_line_end() {
        let buf = Buffer::from_text("ab\r\ncd");
        assert_eq!(visual_col(&buf, 3, 4), 2);
        assert_eq!(offset_at_visual_col(&buf, 0, 5, 4), 2);
    }
}
