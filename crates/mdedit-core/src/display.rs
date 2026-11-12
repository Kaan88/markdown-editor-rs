//! The text a line is drawn with, which differs from the buffer's: a tab becomes spaces to the
//! next stop, and a C0 control character (U+0000 to U+001F except tab, and U+007F) becomes its
//! control picture (U+2400 + c, U+2421 for DEL) so it is visible. The map between the two lets
//! the text view hit-test and draw carets and selections in buffer offsets.
//!
//! Long lines are split into chunks ([`ChunkIndex`]) so only the part on screen is shaped. A
//! chunk start carries its display column and how many wide and zero-width characters come before
//! it, so the view can estimate its x without shaping what is before it ([`width_of`]).

use crate::buffer::Buffer;
use crate::limits::{CHUNK_BYTES, CHUNK_LOOKAHEAD, WHOLE_LINE_BYTES};

/// One substitution: a source byte (a tab or a C0 control, always one byte) shown as `dlen`
/// display bytes. Offsets are relative to the line's `source_start` and to the display text.
#[derive(Clone, Copy, Debug)]
struct Sub {
    /// Display byte where the substitute starts.
    d: u32,
    /// Display bytes the substitute takes.
    dlen: u32,
    /// Source byte of the substituted character.
    s: u32,
}

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
    /// One entry per substitution, in order. Between entries the display and source bytes
    /// advance together.
    map: Vec<Sub>,
    /// Length in bytes of the source text `text` was built from.
    source_len: usize,
}

impl DisplayLine {
    pub fn new() -> DisplayLine {
        DisplayLine::default()
    }

    /// Fills `self` from `source` (no line break in it), which starts at buffer offset
    /// `source_start` and at display column `start_col` (tab stops are counted from column 0 of
    /// the line, so a chunk needs its true start column).
    pub fn build(&mut self, source: &str, source_start: usize, start_col: usize, tab_width: usize) {
        let tab = tab_width.max(1);
        self.text.clear();
        self.map.clear();
        self.source_start = source_start;
        self.start_col = start_col;
        self.source_len = source.len();
        let bytes = source.as_bytes();
        let mut col = start_col;
        let mut i = 0;
        while i < bytes.len() {
            // A run of plain characters is copied whole.
            let run = bytes[i..]
                .iter()
                .position(|&b| b < 0x20 || b == 0x7F)
                .unwrap_or(bytes.len() - i);
            if run > 0 {
                let piece = &source[i..i + run];
                self.text.push_str(piece);
                col += piece.bytes().filter(|&b| (b & 0xC0) != 0x80).count();
                i += run;
                continue;
            }
            let b = bytes[i];
            let d = self.text.len() as u32;
            if b == b'\t' {
                let n = tab - col % tab;
                for _ in 0..n {
                    self.text.push(' ');
                }
                self.map.push(Sub {
                    d,
                    dlen: n as u32,
                    s: i as u32,
                });
                col += n;
            } else {
                let picture = if b == 0x7F {
                    '\u{2421}'
                } else {
                    char::from_u32(0x2400 + b as u32).unwrap_or('\u{FFFD}')
                };
                self.text.push(picture);
                self.map.push(Sub {
                    d,
                    dlen: picture.len_utf8() as u32,
                    s: i as u32,
                });
                col += 1;
            }
            i += 1;
        }
        self.columns = col - start_col;
    }

    /// The display byte for a buffer offset inside this text (clamped to its ends).
    pub fn to_display(&self, source: usize) -> usize {
        let rel = source
            .saturating_sub(self.source_start)
            .min(self.source_len);
        let i = self.map.partition_point(|m| m.s as usize <= rel);
        if i == 0 {
            return rel.min(self.text.len());
        }
        let m = self.map[i - 1];
        if rel == m.s as usize {
            m.d as usize
        } else {
            (m.d + m.dlen) as usize + (rel - m.s as usize - 1)
        }
    }

    /// The buffer offset for a display byte. A byte inside a substitution (the third space of a
    /// tab) maps to the substituted character's start, or past it when `round_up` is set (a
    /// click on the right half of a tab).
    pub fn to_source(&self, display: usize, round_up: bool) -> usize {
        let d = display.min(self.text.len());
        let i = self.map.partition_point(|m| m.d as usize <= d);
        let rel = if i == 0 {
            d
        } else {
            let m = self.map[i - 1];
            let (md, mlen, ms) = (m.d as usize, m.dlen as usize, m.s as usize);
            if d < md + mlen {
                if round_up && d > md { ms + 1 } else { ms }
            } else {
                ms + 1 + (d - md - mlen)
            }
        };
        self.source_start + rel
    }

    /// The display column of a buffer offset inside this text.
    pub fn column_of(&self, source: usize) -> usize {
        let mut d = self.to_display(source);
        while !self.text.is_char_boundary(d) {
            d -= 1;
        }
        self.start_col + self.text[..d].chars().count()
    }

    /// The buffer offset at display column `col` (or the last one before it, for a column inside
    /// a tab), clamped to this text.
    pub fn offset_at_column(&self, col: usize) -> usize {
        let n = col.saturating_sub(self.start_col);
        let d = self
            .text
            .char_indices()
            .nth(n)
            .map_or(self.text.len(), |(i, _)| i);
        self.to_source(d, false)
    }
}

/// How wide a character is drawn, as a class: the view turns it into px with the measured
/// advance of a narrow and of a wide glyph. Combining marks, joiners and variation selectors are
/// zero-width; East Asian wide and fullwidth characters (Han, kana, Hangul, most emoji) are wide.
/// Anything else, tabs and control characters included, is narrow (a tab's extra columns are
/// counted separately).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    Zero,
    Narrow,
    Wide,
}

pub fn width_of(c: char) -> Width {
    if c.is_ascii() {
        return Width::Narrow;
    }
    match unicode_width::UnicodeWidthChar::width(c) {
        Some(0) => Width::Zero,
        Some(2) => Width::Wide,
        _ => Width::Narrow,
    }
}

/// Display columns counted along a line (a tab to its stop, every other character 1) and, of
/// the characters counted, how many are wide and how many zero-width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cols {
    pub col: usize,
    pub wide: usize,
    pub zero: usize,
}

impl Cols {
    /// The counts after `text` (no line break in it). ASCII without tabs costs a length check.
    pub fn after(mut self, text: &str, tab_width: usize) -> Cols {
        let tab = tab_width.max(1);
        let bytes = text.as_bytes();
        if bytes.is_ascii() {
            let mut from = 0;
            for t in memchr::memchr_iter(b'\t', bytes) {
                self.col += t - from;
                self.col += tab - self.col % tab;
                from = t + 1;
            }
            self.col += bytes.len() - from;
            return self;
        }
        for c in text.chars() {
            if c == '\t' {
                self.col += tab - self.col % tab;
                continue;
            }
            self.col += 1;
            match width_of(c) {
                Width::Zero => self.zero += 1,
                Width::Wide => self.wide += 1,
                Width::Narrow => {}
            }
        }
        self
    }
}

/// Where a chunk of a long line starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkStart {
    /// Buffer offset.
    pub offset: usize,
    /// Display column.
    pub col: usize,
    /// Wide characters on the line before the chunk.
    pub wide: usize,
    /// Zero-width characters on the line before the chunk.
    pub zero: usize,
}

impl ChunkStart {
    pub fn cols(&self) -> Cols {
        Cols {
            col: self.col,
            wide: self.wide,
            zero: self.zero,
        }
    }
}

/// The chunk starts of one line, found from the line's start as far as they are needed. Each
/// chunk is about [`CHUNK_BYTES`](crate::limits::CHUNK_BYTES) of buffer text and ends after the
/// first ASCII space or tab within [`CHUNK_LOOKAHEAD`](crate::limits::CHUNK_LOOKAHEAD) bytes past
/// that, else at the next grapheme boundary; never inside a grapheme. A line of at most
/// [`WHOLE_LINE_BYTES`](crate::limits::WHOLE_LINE_BYTES) is one chunk.
///
/// Finding a chunk costs a pass over its own bytes (to count columns), so the index costs what
/// the part of the line it covers costs: the view extends it to the chunks on screen, a caret
/// or a click extends it to its offset. The shell keeps a few indexes per document revision.
#[derive(Clone, Debug)]
pub struct ChunkIndex {
    starts: Vec<ChunkStart>,
    /// The line's end (line break excluded).
    end: usize,
    /// Every chunk start of the line is in `starts`.
    complete: bool,
}

impl ChunkIndex {
    pub fn new(buf: &Buffer, line: usize) -> ChunkIndex {
        let range = buf.line_range(line);
        ChunkIndex {
            starts: vec![ChunkStart {
                offset: range.start,
                ..ChunkStart::default()
            }],
            end: range.end,
            complete: range.len() <= WHOLE_LINE_BYTES,
        }
    }

    pub fn starts(&self) -> &[ChunkStart] {
        &self.starts
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Adds chunk starts while `more(last start)` holds and the line goes on.
    pub fn extend_while(
        &mut self,
        buf: &Buffer,
        tab_width: usize,
        mut more: impl FnMut(&ChunkStart) -> bool,
    ) {
        while !self.complete && more(&self.starts[self.starts.len() - 1]) {
            self.push_next(buf, tab_width);
        }
    }

    /// Extends the index until it holds the chunk that `offset` is in.
    pub fn extend_to(&mut self, buf: &Buffer, tab_width: usize, offset: usize) {
        self.extend_while(buf, tab_width, |last| last.offset <= offset);
    }

    fn push_next(&mut self, buf: &Buffer, tab_width: usize) {
        let last = self.starts[self.starts.len() - 1];
        let start = last.offset;
        let target = start + CHUNK_BYTES;
        if target >= self.end {
            self.complete = true;
            return;
        }
        // The first space or tab at or after `target`, within the lookahead.
        let scan_from = buf.floor_char(target);
        let scan_to = (target + CHUNK_LOOKAHEAD).min(self.end);
        let mut wanted = target;
        let mut at = scan_from;
        'scan: for piece in buf.chunks(scan_from..scan_to) {
            if let Some(i) = memchr::memchr2(b' ', b'\t', piece.as_bytes()) {
                wanted = at + i + 1;
                break 'scan;
            }
            at += piece.len();
        }
        // A boundary after the space may fall inside a grapheme (a combining mark after it).
        let floor = buf.floor_char(wanted);
        let next = if floor == wanted && buf.is_grapheme_boundary(wanted) {
            wanted
        } else {
            buf.next_grapheme(floor)
        };
        if next >= self.end {
            self.complete = true;
            return;
        }
        let mut cols = last.cols();
        for piece in buf.chunks(start..next) {
            cols = cols.after(piece, tab_width);
        }
        self.starts.push(ChunkStart {
            offset: next,
            col: cols.col,
            wide: cols.wide,
            zero: cols.zero,
        });
    }
}

/// Every chunk start of `line` (the whole [`ChunkIndex`]); one pass over the line.
pub fn chunk_starts(buf: &Buffer, line: usize, tab_width: usize) -> Vec<ChunkStart> {
    let mut index = ChunkIndex::new(buf, line);
    index.extend_while(buf, tab_width, |_| true);
    index.starts
}

/// The end column, shared by [`columns`] and the selection's column counts.
pub(crate) fn end_column(text: &str, start_col: usize, tab_width: usize) -> usize {
    Cols {
        col: start_col,
        ..Cols::default()
    }
    .after(text, tab_width)
    .col
}

/// Display columns of `text` starting at column `start_col` (tabs to stops).
pub fn columns(text: &str, start_col: usize, tab_width: usize) -> usize {
    end_column(text, start_col, tab_width) - start_col
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_segmentation::UnicodeSegmentation;

    fn built(src: &str, start: usize, start_col: usize) -> DisplayLine {
        let mut d = DisplayLine::new();
        d.build(src, start, start_col, 4);
        d
    }

    #[test]
    fn tabs_at_several_start_columns() {
        let d = built("a\tb", 10, 0);
        assert_eq!(d.text, "a   b");
        assert_eq!(d.columns, 5);
        let d = built("\tb", 0, 0);
        assert_eq!(d.text, "    b");
        let d = built("\tb", 0, 1);
        assert_eq!(d.text, "   b");
        assert_eq!(d.columns, 4);
        let d = built("\tb", 0, 3);
        assert_eq!(d.text, " b");
        let d = built("ab\t\tc", 0, 0);
        assert_eq!(d.text, "ab      c");
        let d = built("\t", 0, 7);
        assert_eq!(d.text, " ");
        // Tab width other than 4, and zero.
        let mut d = DisplayLine::new();
        d.build("a\tb", 0, 0, 8);
        assert_eq!(d.text, "a       b");
        d.build("a\tb", 0, 0, 0);
        assert_eq!(d.text, "a b");
        // Reuse clears the old content.
        d.build("xyz", 5, 2, 4);
        assert_eq!(d.text, "xyz");
        assert_eq!((d.start_col, d.columns, d.source_start), (2, 3, 5));
    }

    #[test]
    fn control_pictures() {
        let d = built("a\u{0}b\u{1B}c\u{7F}d\u{1}", 0, 0);
        assert_eq!(d.text, "a\u{2400}b\u{241B}c\u{2421}d\u{2401}");
        assert_eq!(d.columns, 8);
        // Multi-byte text and pictures round trip through the map.
        let src = "ğ\u{0}漢\t\u{7F}e\u{301}";
        let d = built(src, 100, 0);
        for (i, _) in src.char_indices() {
            let disp = d.to_display(100 + i);
            assert_eq!(d.to_source(disp, false), 100 + i, "byte {i}");
        }
        assert_eq!(d.to_display(100 + src.len()), d.text.len());
        assert_eq!(d.to_source(d.text.len(), false), 100 + src.len());
    }

    #[test]
    fn to_display_and_to_source() {
        let src = "ab\tcd\u{1}e";
        let d = built(src, 50, 0);
        // a b [2 spaces] c d [picture, 3 bytes] e
        assert_eq!(d.text, "ab  cd\u{2401}e");
        assert_eq!(d.to_display(50), 0);
        assert_eq!(d.to_display(52), 2); // the tab starts
        assert_eq!(d.to_display(53), 4); // after the tab
        assert_eq!(d.to_display(55), 6); // the control starts
        assert_eq!(d.to_display(56), 9); // after it
        assert_eq!(d.to_display(57), 10);
        assert_eq!(d.to_display(0), 0); // clamped below
        assert_eq!(d.to_display(999), 10); // clamped above
        // Inside the tab and the picture.
        assert_eq!(d.to_source(2, false), 52);
        assert_eq!(d.to_source(3, false), 52);
        assert_eq!(d.to_source(3, true), 53);
        assert_eq!(d.to_source(2, true), 52);
        assert_eq!(d.to_source(4, false), 53);
        assert_eq!(d.to_source(7, false), 55);
        assert_eq!(d.to_source(7, true), 56);
        assert_eq!(d.to_source(9, false), 56);
        assert_eq!(d.to_source(999, false), 57);
    }

    #[test]
    fn columns_and_offset_at_column() {
        let src = "ab\tcd";
        let d = built(src, 10, 0);
        assert_eq!(d.column_of(10), 0);
        assert_eq!(d.column_of(12), 2);
        assert_eq!(d.column_of(13), 4);
        assert_eq!(d.column_of(15), 6);
        // Inside the tab: the last offset before the column.
        assert_eq!(d.offset_at_column(0), 10);
        assert_eq!(d.offset_at_column(2), 12);
        assert_eq!(d.offset_at_column(3), 12);
        assert_eq!(d.offset_at_column(4), 13);
        assert_eq!(d.offset_at_column(6), 15);
        assert_eq!(d.offset_at_column(60), 15);
        // A chunk that starts at column 5.
        let d = built("x\ty", 200, 5);
        assert_eq!(d.text, "x  y");
        assert_eq!(d.column_of(200), 5);
        assert_eq!(d.column_of(201), 6);
        assert_eq!(d.column_of(202), 8);
        assert_eq!(d.offset_at_column(1), 200);
        assert_eq!(d.offset_at_column(7), 201);
        assert_eq!(d.offset_at_column(8), 202);
        // CJK and combining marks count one column each (an estimate).
        let d = built("漢e\u{301}", 0, 0);
        assert_eq!(d.columns, 3);
    }

    #[test]
    fn columns_function() {
        assert_eq!(columns("abc", 0, 4), 3);
        assert_eq!(columns("a\tb", 0, 4), 5);
        assert_eq!(columns("\t", 3, 4), 1);
        assert_eq!(columns("\t", 4, 4), 4);
        assert_eq!(columns("ğ漢\u{0}", 0, 4), 3);
        assert_eq!(columns("", 9, 4), 0);
        assert_eq!(columns("\t", 0, 0), 1);
    }

    #[test]
    fn chunks_of_short_lines() {
        let buf = Buffer::from_text(&format!("{}\nabc", "x".repeat(4096)));
        let at = |offset, col| ChunkStart {
            offset,
            col,
            ..ChunkStart::default()
        };
        let c = chunk_starts(&buf, 0, 4);
        assert_eq!(c, vec![at(0, 0)]);
        let c = chunk_starts(&buf, 1, 4);
        assert_eq!(c, vec![at(4097, 0)]);
        let buf = Buffer::from_text(&format!("{}\nabc", "x".repeat(4097)));
        let c = chunk_starts(&buf, 0, 4);
        assert!(c.len() > 1);
        assert_eq!(c[0], at(0, 0));
        assert_eq!(c[1], at(1024, 1024));
        // Past the last line.
        assert_eq!(chunk_starts(&buf, 5, 4).len(), 1);
    }

    #[test]
    fn chunks_prefer_spaces_and_tabs() {
        // A space 100 bytes past the target: the chunk ends after it.
        let mut s = "x".repeat(1124);
        s.push(' ');
        s.push_str(&"y".repeat(5000));
        let buf = Buffer::from_text(&s);
        let c = chunk_starts(&buf, 0, 4);
        assert_eq!(c[1].offset, 1125);
        // A tab: the column after it is a tab stop.
        let mut s = "x".repeat(1100);
        s.push('\t');
        s.push_str(&"y".repeat(5000));
        let buf = Buffer::from_text(&s);
        let c = chunk_starts(&buf, 0, 4);
        assert_eq!(c[1].offset, 1101);
        assert_eq!(c[1].col, 1104);
        // No space within the lookahead: the chunk ends at the target.
        let mut s = "x".repeat(1600);
        s.push(' ');
        s.push_str(&"y".repeat(5000));
        let buf = Buffer::from_text(&s);
        let c = chunk_starts(&buf, 0, 4);
        assert_eq!(c[1].offset, 1024);
    }

    #[test]
    fn chunk_does_not_split_a_grapheme() {
        // A combining mark right after a space and a ZWJ sequence over the target.
        let mut s = "x".repeat(1023);
        s.push_str("e\u{301}\u{302}");
        s.push_str(&"y".repeat(6000));
        let buf = Buffer::from_text(&s);
        let c = chunk_starts(&buf, 0, 4);
        // The target 1024 is inside e + marks (1023..1028): the chunk ends after them.
        assert_eq!(c[1].offset, 1028);
        let mut s = "x".repeat(1100);
        s.push(' ');
        s.push('\u{301}');
        s.push_str(&"y".repeat(6000));
        let buf = Buffer::from_text(&s);
        let c = chunk_starts(&buf, 0, 4);
        assert_eq!(c[1].offset, 1103); // the space with its mark
    }

    #[test]
    fn one_megabyte_line_without_spaces() {
        // Multi-byte characters, combining marks, emoji and tabs, but no ASCII space.
        let unit = "ğüşi漢字e\u{301}\u{1F468}\u{200D}\u{1F469}\u{1F1F9}\u{1F1F7}ab";
        let mut s = String::new();
        let mut n = 0;
        while s.len() < 1_000_000 {
            s.push_str(unit);
            if n % 37 == 0 {
                s.push('\t');
            }
            n += 1;
        }
        let buf = Buffer::from_text(&s);
        let starts = chunk_starts(&buf, 0, 4);
        assert!(
            starts.len() > 600 && starts.len() < 1000,
            "{}",
            starts.len()
        );
        let boundaries: std::collections::HashSet<usize> = s
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain(std::iter::once(s.len()))
            .collect();
        for (k, c) in starts.iter().enumerate() {
            assert!(boundaries.contains(&c.offset), "chunk {k} at {}", c.offset);
            assert_eq!(c.col, columns(&s[..c.offset], 0, 4), "chunk {k}");
            let before = &s[..c.offset];
            let wide = before.chars().filter(|&ch| width_of(ch) == Width::Wide);
            let zero = before.chars().filter(|&ch| width_of(ch) == Width::Zero);
            assert_eq!((c.wide, c.zero), (wide.count(), zero.count()), "chunk {k}");
            if k > 0 {
                let size = c.offset - starts[k - 1].offset;
                // About 1 KB, up to the lookahead past it (a tab ends the chunk early in the
                // lookahead), plus a grapheme.
                assert!(
                    (CHUNK_BYTES..CHUNK_BYTES + CHUNK_LOOKAHEAD + 32).contains(&size),
                    "size {size}"
                );
            }
        }
        assert_eq!(starts[0], ChunkStart::default());
    }

    #[test]
    fn widths() {
        assert_eq!(width_of('a'), Width::Narrow);
        assert_eq!(width_of('\t'), Width::Narrow);
        assert_eq!(width_of('ğ'), Width::Narrow);
        assert_eq!(width_of('漢'), Width::Wide);
        assert_eq!(width_of('あ'), Width::Wide);
        assert_eq!(width_of('\u{AC00}'), Width::Wide);
        assert_eq!(width_of('\u{1F600}'), Width::Wide);
        assert_eq!(width_of('\u{301}'), Width::Zero);
        assert_eq!(width_of('\u{200D}'), Width::Zero);
        let c = Cols::default().after("a\t漢e\u{301}\u{1F600}", 4);
        assert_eq!(
            c,
            Cols {
                col: 8,
                wide: 2,
                zero: 1
            }
        );
        // ASCII with tabs takes the short path and agrees with the long one.
        let ascii = "ab\tc\t\tdefgh\t";
        let mut long = Cols::default();
        for ch in ascii.chars() {
            long = long.after(&ch.to_string(), 4);
        }
        assert_eq!(Cols::default().after(ascii, 4), long);
        assert_eq!(columns(ascii, 0, 4), long.col);
    }

    #[test]
    fn index_extends_only_as_far_as_asked() {
        let s = "word ".repeat(200_000); // 1 MB, one line
        let buf = Buffer::from_text(&s);
        let mut index = ChunkIndex::new(&buf, 0);
        assert!(!index.is_complete());
        assert_eq!(index.starts().len(), 1);
        index.extend_to(&buf, 4, 5000);
        let n = index.starts().len();
        assert!((5..=7).contains(&n), "{n}");
        assert!(index.starts()[n - 1].offset > 5000);
        // Asking again for less adds nothing; the full walk agrees with the partial one.
        index.extend_to(&buf, 4, 100);
        assert_eq!(index.starts().len(), n);
        let all = chunk_starts(&buf, 0, 4);
        assert_eq!(&all[..n], index.starts());
        index.extend_while(&buf, 4, |_| true);
        assert!(index.is_complete());
        assert_eq!(index.starts(), &all[..]);
    }

    #[test]
    fn chunks_over_many_rope_chunks_and_a_second_line() {
        let mut s = String::from("first\n");
        let start = s.len();
        for i in 0..2000 {
            s.push_str(&format!("w{i} "));
        }
        s.push_str("\nlast");
        let buf = Buffer::from_text(&s);
        let c = chunk_starts(&buf, 1, 4);
        assert_eq!(c[0].offset, start);
        assert!(c.len() > 3);
        for chunk in &c[1..] {
            assert_eq!(s.as_bytes()[chunk.offset - 1], b' ');
            assert_eq!(chunk.col, chunk.offset - start);
        }
    }
}
