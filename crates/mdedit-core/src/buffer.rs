//! The text: a `ropey::Rope` of UTF-8, with lines broken at LF, CRLF and CR (the `cr_lines`
//! feature). Positions are byte offsets on `char` boundaries. Phase 1 never changes a buffer; a
//! reload replaces it. Phase 2 adds `replace`, which bumps [`Buffer::revision`].

use ropey::Rope;
use std::ops::Range;

/// A line and a column. `col` counts `char`s from the line start (a tab is one), as Notepad's
/// "Col" does; the status area adds one to each.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Buffer {
    rope: Rope,
    revision: u64,
}

impl Buffer {
    pub fn from_rope(rope: Rope) -> Buffer {
        Buffer { rope, revision: 0 }
    }

    pub fn from_text(text: &str) -> Buffer {
        Buffer::from_rope(Rope::from_str(text))
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// A copy-on-write snapshot for a worker thread (O(1); shares the rope's chunks).
    pub fn snapshot(&self) -> Rope {
        self.rope.clone()
    }

    /// Changes with every edit. Results computed on a worker carry the revision they were
    /// computed for, and the shell drops those that are stale.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn len_bytes(&self) -> usize {
        todo!()
    }

    /// Number of lines. Text ending in a line break has an empty last line after it; an empty
    /// buffer has one line.
    pub fn len_lines(&self) -> usize {
        todo!()
    }

    /// Byte offset where `line` starts. `line == len_lines()` gives `len_bytes()`.
    pub fn line_start(&self, line: usize) -> usize {
        let _ = line;
        todo!()
    }

    /// The line's text without its line break.
    pub fn line_range(&self, line: usize) -> Range<usize> {
        let _ = line;
        todo!()
    }

    /// The line's text with its line break (LF, CRLF, CR or none).
    pub fn line_range_with_break(&self, line: usize) -> Range<usize> {
        let _ = line;
        todo!()
    }

    /// The line holding byte `offset`. An offset inside a line break belongs to that line.
    pub fn line_of(&self, offset: usize) -> usize {
        let _ = offset;
        todo!()
    }

    /// Replaces `out` with the line's text, line break excluded.
    pub fn line_text(&self, line: usize, out: &mut String) {
        let _ = (line, out);
        todo!()
    }

    /// Replaces `out` with the text of `range` (char boundaries), copied from the rope's chunks.
    pub fn text(&self, range: Range<usize>, out: &mut String) {
        let _ = (range, out);
        todo!()
    }

    /// The `&str` pieces of `range` in order, without copying.
    pub fn chunks(&self, range: Range<usize>) -> impl Iterator<Item = &str> + '_ {
        let _ = range;
        std::iter::from_fn(|| -> Option<&str> { todo!() })
    }

    pub fn char_at(&self, offset: usize) -> Option<char> {
        let _ = offset;
        todo!()
    }

    pub fn is_char_boundary(&self, offset: usize) -> bool {
        let _ = offset;
        todo!()
    }

    /// The grapheme boundary before `offset` (0 at the start). A CRLF is one grapheme.
    pub fn prev_grapheme(&self, offset: usize) -> usize {
        let _ = offset;
        todo!()
    }

    /// The grapheme boundary after `offset` (`len_bytes()` at the end).
    pub fn next_grapheme(&self, offset: usize) -> usize {
        let _ = offset;
        todo!()
    }

    /// The nearest grapheme boundary at or before `offset`.
    pub fn snap_to_grapheme(&self, offset: usize) -> usize {
        let _ = offset;
        todo!()
    }

    pub fn position(&self, offset: usize) -> Position {
        let _ = offset;
        todo!()
    }

    /// The byte offset of a position, clamped to the buffer, and to the line's end (break
    /// excluded) for a column past it.
    pub fn offset(&self, pos: Position) -> usize {
        let _ = pos;
        todo!()
    }
}
