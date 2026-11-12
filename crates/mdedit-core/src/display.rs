//! The text a line is drawn with, which differs from the buffer's: a tab becomes spaces to the
//! next stop, and a C0 control character (U+0000 to U+001F except tab, and U+007F) becomes its
//! control picture (U+2400 + c, U+2421 for DEL) so it is visible. The map between the two lets
//! the text view hit-test and draw carets and selections in buffer offsets.
//!
//! Long lines are split into chunks ([`chunk_starts`]) so only the part on screen is shaped.

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
    let range = buf.line_range(line);
    let mut out = vec![ChunkStart {
        offset: range.start,
        col: 0,
    }];
    if range.len() <= WHOLE_LINE_BYTES {
        return out;
    }
    let mut start = range.start;
    let mut col = 0usize;
    loop {
        let target = start + CHUNK_BYTES;
        if target >= range.end {
            break;
        }
        // The first space or tab at or after `target`, within the lookahead.
        let scan_from = buf.floor_char(target);
        let scan_to = (target + CHUNK_LOOKAHEAD).min(range.end);
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
        if next >= range.end {
            break;
        }
        for piece in buf.chunks(start..next) {
            col = end_column(piece, col, tab_width);
        }
        out.push(ChunkStart { offset: next, col });
        start = next;
    }
    out
}

/// The end column, shared by [`columns`] and the chunk walk.
pub(crate) fn end_column(text: &str, start_col: usize, tab_width: usize) -> usize {
    let tab = tab_width.max(1);
    let mut col = start_col;
    for &b in text.as_bytes() {
        if b == b'\t' {
            col += tab - col % tab;
        } else if (b & 0xC0) != 0x80 {
            col += 1;
        }
    }
    col
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
        let c = chunk_starts(&buf, 0, 4);
        assert_eq!(c, vec![ChunkStart { offset: 0, col: 0 }]);
        let c = chunk_starts(&buf, 1, 4);
        assert_eq!(
            c,
            vec![ChunkStart {
                offset: 4097,
                col: 0
            }]
        );
        let buf = Buffer::from_text(&format!("{}\nabc", "x".repeat(4097)));
        let c = chunk_starts(&buf, 0, 4);
        assert!(c.len() > 1);
        assert_eq!(c[0], ChunkStart { offset: 0, col: 0 });
        assert_eq!(
            c[1],
            ChunkStart {
                offset: 1024,
                col: 1024
            }
        );
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
        assert_eq!(starts[0], ChunkStart { offset: 0, col: 0 });
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
