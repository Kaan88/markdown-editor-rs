//! The text a line is drawn with, which differs from the buffer's: a tab becomes spaces to the
//! next stop, and a C0 control character (U+0000 to U+001F except tab, and U+007F) becomes its
//! control picture (U+2400 + c, U+2421 for DEL) so it is visible. The map between the two lets
//! the text view hit-test and draw carets and selections in buffer offsets.
//!
//! Long lines are split into chunks ([`chunk_starts`]) so only the part on screen is shaped.

use crate::buffer::Buffer;

/// One line, or one chunk of a long line, as drawn. Reused between lines to keep its allocations.
#[derive(Clone, Debug, Default)]
pub struct DisplayLine {
    /// The text to shape.
    pub text: String,
    /// The display column the text starts at (0 for a whole line; a chunk's start column).
    pub start_col: usize,
    /// Display columns the text covers, a tab counting to its stop.
    pub columns: usize,
    /// The buffer offset `text` starts at.
    pub source_start: usize,
    /// One entry per substitution, in order: (display byte, source byte) of its start, both
    /// relative to the starts above. Between entries the two advance together.
    map: Vec<(u32, u32)>,
}

impl DisplayLine {
    pub fn new() -> DisplayLine {
        DisplayLine::default()
    }

    /// Fills `self` from `source` (no line break in it), which starts at buffer offset
    /// `source_start` and at display column `start_col` (tab stops are counted from column 0 of
    /// the line, so a chunk needs its true start column).
    pub fn build(&mut self, source: &str, source_start: usize, start_col: usize, tab_width: usize) {
        let _ = (source, source_start, start_col, tab_width);
        todo!()
    }

    /// The display byte for a buffer offset inside this text (clamped to its ends).
    pub fn to_display(&self, source: usize) -> usize {
        let _ = source;
        todo!()
    }

    /// The buffer offset for a display byte. A byte inside a substitution (the third space of a
    /// tab) maps to the substituted character's start, or past it when `round_up` is set (a
    /// click on the right half of a tab).
    pub fn to_source(&self, display: usize, round_up: bool) -> usize {
        let _ = (display, round_up);
        todo!()
    }

    /// The display column of a buffer offset inside this text.
    pub fn column_of(&self, source: usize) -> usize {
        let _ = source;
        todo!()
    }

    /// The buffer offset at display column `col` (or the last one before it, for a column inside
    /// a tab), clamped to this text.
    pub fn offset_at_column(&self, col: usize) -> usize {
        let _ = col;
        todo!()
    }
}

/// Where a chunk of a long line starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkStart {
    /// Buffer offset.
    pub offset: usize,
    /// Display column.
    pub col: usize,
}

/// The chunk starts of `line`, the first at the line's start. Each chunk is about
/// [`CHUNK_BYTES`](crate::limits::CHUNK_BYTES) of buffer text and ends after the first ASCII
/// space or tab within [`CHUNK_LOOKAHEAD`](crate::limits::CHUNK_LOOKAHEAD) bytes past that, else
/// at the next grapheme boundary; never inside a grapheme. A line of at most
/// [`WHOLE_LINE_BYTES`](crate::limits::WHOLE_LINE_BYTES) is one chunk. Costs one pass over the
/// line's bytes (to count columns); the shell caches the result per line and revision.
pub fn chunk_starts(buf: &Buffer, line: usize, tab_width: usize) -> Vec<ChunkStart> {
    let _ = (buf, line, tab_width);
    todo!()
}

/// Display columns of `text` starting at column `start_col` (tabs to stops).
pub fn columns(text: &str, start_col: usize, tab_width: usize) -> usize {
    let _ = (text, start_col, tab_width);
    todo!()
}
