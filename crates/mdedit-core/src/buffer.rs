//! The text: a `ropey::Rope` of UTF-8, with lines broken at LF, CRLF and CR (the `cr_lines`
//! feature). Positions are byte offsets on `char` boundaries. The one way to change a buffer is
//! [`Buffer::replace`], which bumps [`Buffer::revision`] and reports what it did as a [`Change`];
//! a reload makes a new buffer.

use crate::eol::EolCounts;
use ropey::Rope;
use std::ops::Range;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

/// A line and a column. `col` counts `char`s from the line start (a tab is one), as Notepad's
/// "Col" does; the status area adds one to each.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

/// What one edit did, for the shell's caches (layouts, colours, find, line-ending counts).
///
/// Every line before `first_line` is unchanged. Every line from `first_line + lines_before` in
/// the old text equals the line from `first_line + lines_after` in the new text. Between, the old
/// text had `lines_before` lines and the new has `lines_after`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// Start of the replaced range, after clamping and flooring to a `char` boundary. The bytes
    /// before it are unchanged.
    pub at: usize,
    /// Bytes removed from `at` (old coordinates).
    pub removed: usize,
    /// Bytes inserted at `at` (new coordinates). The bytes after `at + removed` in the old text
    /// are those after `at + inserted` in the new.
    pub inserted: usize,
    /// First line whose text or line break changed. A CR before `at` counts: an inserted LF
    /// joins it into a CRLF, and a removed LF after it splits one.
    pub first_line: usize,
    /// How many lines, from `first_line`, the edit touched in the old text (at least 1).
    pub lines_before: usize,
    /// How many lines, from `first_line`, they became in the new text (at least 1).
    pub lines_after: usize,
    /// Line-ending counts of a window of the old text, the replaced range plus a byte each side.
    pub eol_removed: EolCounts,
    /// The same window of the new text. `EolCounts::apply(eol_removed, eol_added)` takes the old
    /// text's counts to the new text's.
    pub eol_added: EolCounts,
}

impl Change {
    /// New line count minus old: `lines_after - lines_before`.
    pub fn line_delta(&self) -> isize {
        self.lines_after as isize - self.lines_before as isize
    }

    fn is_noop(&self) -> bool {
        self.removed == 0 && self.inserted == 0
    }

    /// Makes `self` the change of two edits applied one after the other, `later` being in the
    /// coordinates of the text `self` produced. The first line is the smaller; the line spans
    /// reach the later end of the two in the middle text and map back to the old and forward to
    /// the new. `removed` and `inserted` become the span from the smaller `at` to the furthest
    /// touched end in old and in new coordinates (bytes in between that neither edit touched are
    /// counted in both). The eol windows are summed.
    pub fn merge(&mut self, later: &Change) {
        if later.is_noop() {
            return;
        }
        if self.is_noop() {
            *self = later.clone();
            return;
        }
        let first = self.first_line.min(later.first_line);
        // The end, in the middle text, of what either edit touched.
        let mid_line =
            (self.first_line + self.lines_after).max(later.first_line + later.lines_before);
        let lines_before = (mid_line as isize - self.line_delta()) as usize - first;
        let lines_after = (mid_line as isize + later.line_delta()) as usize - first;
        let mid_end = (self.at + self.inserted).max(later.at + later.removed);
        let at = self.at.min(later.at);
        let old_end = (mid_end as isize - self.inserted as isize + self.removed as isize) as usize;
        let new_end =
            (mid_end as isize - later.removed as isize + later.inserted as isize) as usize;
        self.at = at;
        self.removed = old_end - at;
        self.inserted = new_end - at;
        self.first_line = first;
        self.lines_before = lines_before;
        self.lines_after = lines_after;
        self.eol_removed.add(&later.eol_removed);
        self.eol_added.add(&later.eol_added);
    }
}

/// What replaces a range: text, or a rope whose chunks the buffer then shares.
enum Insert<'a> {
    Str(&'a str),
    Rope(&'a Rope),
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
        self.rope.len_bytes()
    }

    /// Number of lines. Text ending in a line break has an empty last line after it; an empty
    /// buffer has one line.
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Replaces `range` with `text` and bumps [`Buffer::revision`]. The range is clamped to the
    /// buffer and its ends moved back to `char` boundaries. An empty range with empty text
    /// changes nothing: the revision stays and the [`Change`] has no bytes. Costs O(log n) plus
    /// the size of the edit, however large the buffer.
    pub fn replace(&mut self, range: Range<usize>, text: &str) -> Change {
        self.replace_with(range, Insert::Str(text))
    }

    /// [`Buffer::replace`] with a rope: the new text shares the rope's chunks, so restoring a
    /// large removed text (undo) copies nothing.
    pub(crate) fn replace_rope(&mut self, range: Range<usize>, text: &Rope) -> Change {
        self.replace_with(range, Insert::Rope(text))
    }

    fn replace_with(&mut self, range: Range<usize>, ins: Insert<'_>) -> Change {
        let len = self.rope.len_bytes();
        let at = self.floor_char(range.start);
        let end = self.floor_char(range.end).max(at);
        let removed = end - at;
        let inserted = match &ins {
            Insert::Str(s) => s.len(),
            Insert::Rope(r) => r.len_bytes(),
        };
        let mut first_line = self.rope.byte_to_line(at);
        if removed == 0 && inserted == 0 {
            return Change {
                at,
                first_line,
                lines_before: 1,
                lines_after: 1,
                ..Change::default()
            };
        }
        if at > 0 && self.rope.byte(at - 1) == b'\r' {
            first_line = self.rope.byte_to_line(at - 1);
        }
        let lines_before_total = self.rope.len_lines();
        let last_line = self.rope.byte_to_line(end);
        // The window for the line-ending counts: the range and a `char` each side, so a pair
        // that the edit joins or splits is inside it.
        let win_start = if at == 0 { 0 } else { self.floor_char(at - 1) };
        let win_end = end + self.char_at(end).map_or(0, char::len_utf8);
        let eol_removed = self.eol_window(win_start..win_end);

        let (c0, c1) = (self.rope.byte_to_char(at), self.rope.byte_to_char(end));
        if c1 > c0 {
            self.rope.remove(c0..c1);
        }
        match ins {
            Insert::Str(s) => {
                if !s.is_empty() {
                    self.rope.insert(c0, s);
                }
            }
            Insert::Rope(r) => {
                if r.len_bytes() > 0 {
                    let tail = self.rope.split_off(c0);
                    self.rope.append(r.clone());
                    self.rope.append(tail);
                }
            }
        }
        self.revision += 1;
        let eol_added = self.eol_window(win_start..win_end - removed + inserted);
        debug_assert!(win_end <= len);
        let lines_before = last_line - first_line + 1;
        let lines_after = lines_before + self.rope.len_lines() - lines_before_total;
        Change {
            at,
            removed,
            inserted,
            first_line,
            lines_before,
            lines_after,
            eol_removed,
            eol_added,
        }
    }

    fn eol_window(&self, range: Range<usize>) -> EolCounts {
        EolCounts::of_chunks(self.rope.byte_slice(range).chunks())
    }

    /// `offset` clamped to the buffer and moved back to the start of the `char` holding it.
    pub(crate) fn floor_char(&self, offset: usize) -> usize {
        let len = self.rope.len_bytes();
        if offset >= len {
            return len;
        }
        // Most offsets are boundaries; a continuation byte is the only case that needs the walk.
        if (self.rope.byte(offset) & 0xC0) != 0x80 {
            return offset;
        }
        self.rope.char_to_byte(self.rope.byte_to_char(offset))
    }

    /// Byte offset where `line` starts. `line == len_lines()` gives `len_bytes()`.
    pub fn line_start(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            self.rope.len_bytes()
        } else {
            self.rope.line_to_byte(line)
        }
    }

    /// The line's text without its line break.
    pub fn line_range(&self, line: usize) -> Range<usize> {
        let r = self.line_range_with_break(line);
        let n = self.break_len(&r);
        r.start..r.end - n
    }

    /// The line's text with its line break (LF, CRLF, CR or none).
    pub fn line_range_with_break(&self, line: usize) -> Range<usize> {
        let start = self.line_start(line);
        let end = self.line_start(line.saturating_add(1));
        start..end
    }

    /// Length of the line break that ends `r` (a whole line, break included): 0, 1 or 2.
    fn break_len(&self, r: &Range<usize>) -> usize {
        if r.end == r.start {
            return 0;
        }
        match self.rope.byte(r.end - 1) {
            b'\n' => {
                if r.end - 1 > r.start && self.rope.byte(r.end - 2) == b'\r' {
                    2
                } else {
                    1
                }
            }
            b'\r' => 1,
            _ => 0,
        }
    }

    /// The line holding byte `offset`. An offset inside a line break belongs to that line.
    pub fn line_of(&self, offset: usize) -> usize {
        self.rope.byte_to_line(offset.min(self.rope.len_bytes()))
    }

    /// Replaces `out` with the line's text, line break excluded.
    pub fn line_text(&self, line: usize, out: &mut String) {
        self.text(self.line_range(line), out);
    }

    /// Replaces `out` with the text of `range` (char boundaries), copied from the rope's chunks.
    pub fn text(&self, range: Range<usize>, out: &mut String) {
        out.clear();
        for piece in self.chunks(range) {
            out.push_str(piece);
        }
    }

    /// The `&str` pieces of `range` in order, without copying.
    pub fn chunks(&self, range: Range<usize>) -> impl Iterator<Item = &str> + '_ {
        let start = self.floor_char(range.start);
        let end = self.floor_char(range.end).max(start);
        self.rope.byte_slice(start..end).chunks()
    }

    pub fn char_at(&self, offset: usize) -> Option<char> {
        if offset >= self.rope.len_bytes() {
            return None;
        }
        self.rope.get_char(self.rope.byte_to_char(offset))
    }

    /// True when `offset` (clamped to the buffer) is the start of a `char`.
    pub fn is_char_boundary(&self, offset: usize) -> bool {
        self.floor_char(offset) == offset.min(self.rope.len_bytes())
    }

    /// The grapheme boundary before `offset` (0 at the start). A CRLF is one grapheme.
    pub fn prev_grapheme(&self, offset: usize) -> usize {
        let offset = self.floor_char(offset);
        if offset == 0 {
            return 0;
        }
        let len = self.rope.len_bytes();
        let (chunk, chunk_start, _, _) = self.rope.chunk_at_byte(offset);
        // Two ASCII bytes that are not CR LF always have a boundary between them.
        let local = offset - chunk_start;
        if local >= 2 {
            let b = chunk.as_bytes();
            let (o, p) = (b[local - 2], b[local - 1]);
            if p < 0x80 && o < 0x80 && !(o == b'\r' && p == b'\n') {
                return offset - 1;
            }
        }
        let mut chunk = chunk;
        let mut chunk_start = chunk_start;
        let mut cursor = GraphemeCursor::new(offset, len, true);
        loop {
            match cursor.prev_boundary(chunk, chunk_start) {
                Ok(Some(n)) => return n,
                Ok(None) => return 0,
                Err(GraphemeIncomplete::PrevChunk) => {
                    let (c, s, _, _) = self.rope.chunk_at_byte(chunk_start - 1);
                    chunk = c;
                    chunk_start = s;
                }
                Err(GraphemeIncomplete::PreContext(n)) => {
                    let (ctx, start, _, _) = self.rope.chunk_at_byte(n - 1);
                    cursor.provide_context(&ctx[..n - start], start);
                }
                Err(_) => return self.floor_char(offset - 1),
            }
        }
    }

    /// The grapheme boundary after `offset` (`len_bytes()` at the end).
    pub fn next_grapheme(&self, offset: usize) -> usize {
        let offset = self.floor_char(offset);
        let len = self.rope.len_bytes();
        if offset >= len {
            return len;
        }
        let (chunk, chunk_start, _, _) = self.rope.chunk_at_byte(offset);
        let local = offset - chunk_start;
        let b = chunk.as_bytes();
        if local + 1 < b.len() && b[local] < 0x80 && b[local + 1] < 0x80 {
            if !(b[local] == b'\r' && b[local + 1] == b'\n') {
                return offset + 1;
            }
            return offset + 2;
        }
        let mut chunk = chunk;
        let mut chunk_start = chunk_start;
        let mut cursor = GraphemeCursor::new(offset, len, true);
        loop {
            match cursor.next_boundary(chunk, chunk_start) {
                Ok(Some(n)) => return n,
                Ok(None) => return len,
                Err(GraphemeIncomplete::NextChunk) => {
                    chunk_start += chunk.len();
                    chunk = self.rope.chunk_at_byte(chunk_start).0;
                }
                Err(GraphemeIncomplete::PreContext(n)) => {
                    let (ctx, start, _, _) = self.rope.chunk_at_byte(n - 1);
                    cursor.provide_context(&ctx[..n - start], start);
                }
                Err(_) => return len,
            }
        }
    }

    /// The nearest grapheme boundary at or before `offset`.
    pub fn snap_to_grapheme(&self, offset: usize) -> usize {
        let offset = self.floor_char(offset);
        if self.is_grapheme_boundary(offset) {
            offset
        } else {
            self.prev_grapheme(offset)
        }
    }

    /// True when `offset` (a `char` boundary) is a grapheme boundary.
    pub(crate) fn is_grapheme_boundary(&self, offset: usize) -> bool {
        let len = self.rope.len_bytes();
        if offset == 0 || offset >= len {
            return offset <= len;
        }
        let (chunk, chunk_start, _, _) = self.rope.chunk_at_byte(offset);
        let local = offset - chunk_start;
        let b = chunk.as_bytes();
        if local >= 1 && b[local] < 0x80 && b[local - 1] < 0x80 {
            return !(b[local - 1] == b'\r' && b[local] == b'\n');
        }
        let mut cursor = GraphemeCursor::new(offset, len, true);
        loop {
            match cursor.is_boundary(chunk, chunk_start) {
                Ok(v) => return v,
                Err(GraphemeIncomplete::PreContext(n)) => {
                    let (ctx, start, _, _) = self.rope.chunk_at_byte(n - 1);
                    cursor.provide_context(&ctx[..n - start], start);
                }
                Err(_) => return true,
            }
        }
    }

    pub fn position(&self, offset: usize) -> Position {
        let o = self.floor_char(offset);
        let line = self.rope.byte_to_line(o);
        let col = self.rope.byte_to_char(o) - self.rope.line_to_char(line);
        Position { line, col }
    }

    /// The byte offset of a position, clamped to the buffer, and to the line's end (break
    /// excluded) for a column past it.
    pub fn offset(&self, pos: Position) -> usize {
        let line = pos.line.min(self.rope.len_lines() - 1);
        let r = self.line_range(line);
        let start_char = self.rope.byte_to_char(r.start);
        let end_char = self.rope.byte_to_char(r.end);
        let c = start_char + pos.col.min(end_char - start_char);
        self.rope.char_to_byte(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::RopeBuilder;

    fn b(t: &str) -> Buffer {
        Buffer::from_text(t)
    }

    #[test]
    fn lines_lf_crlf_cr() {
        for (text, br) in [
            ("ab\ncd\nef", "\n"),
            ("ab\r\ncd\r\nef", "\r\n"),
            ("ab\rcd\ref", "\r"),
        ] {
            let buf = b(text);
            let n = br.len();
            assert_eq!(buf.len_lines(), 3, "{text:?}");
            assert_eq!(buf.line_range(0), 0..2);
            assert_eq!(buf.line_range_with_break(0), 0..2 + n);
            assert_eq!(buf.line_range(1), 2 + n..4 + n);
            assert_eq!(buf.line_range_with_break(1), 2 + n..4 + 2 * n);
            assert_eq!(buf.line_range(2), 4 + 2 * n..6 + 2 * n);
            assert_eq!(buf.line_range_with_break(2), 4 + 2 * n..6 + 2 * n);
            assert_eq!(buf.line_start(3), buf.len_bytes());
        }
    }

    #[test]
    fn trailing_break_and_empty() {
        for text in ["ab\n", "ab\r\n", "ab\r"] {
            let buf = b(text);
            assert_eq!(buf.len_lines(), 2, "{text:?}");
            assert_eq!(buf.line_range(0), 0..2);
            assert_eq!(buf.line_range_with_break(0), 0..text.len());
            assert_eq!(buf.line_range(1), text.len()..text.len());
            assert_eq!(buf.line_of(text.len()), 1);
        }
        let e = b("");
        assert_eq!(e.len_lines(), 1);
        assert_eq!(e.len_bytes(), 0);
        assert_eq!(e.line_range(0), 0..0);
        assert_eq!(e.line_of(0), 0);
        assert_eq!(e.position(5), Position { line: 0, col: 0 });
        assert_eq!(e.offset(Position { line: 9, col: 9 }), 0);
        let blank = b("\n\n");
        assert_eq!(blank.len_lines(), 3);
        assert_eq!(blank.line_range(0), 0..0);
        assert_eq!(blank.line_range_with_break(1), 1..2);
        // Lines past the end are empty at the end.
        assert_eq!(blank.line_range(10), 2..2);
        assert_eq!(blank.line_start(10), 2);
    }

    #[test]
    fn other_separators_do_not_break_lines() {
        let buf = b("a\u{0B}b\u{0C}c\u{85}d\u{2028}e");
        assert_eq!(buf.len_lines(), 1);
    }

    #[test]
    fn line_of_inside_crlf() {
        let buf = b("ab\r\ncd");
        assert_eq!(buf.line_of(0), 0);
        assert_eq!(buf.line_of(2), 0);
        assert_eq!(buf.line_of(3), 0); // between CR and LF
        assert_eq!(buf.line_of(4), 1);
        assert_eq!(buf.line_of(6), 1);
        assert_eq!(buf.line_of(600), 1);
    }

    fn big_rope() -> (String, Buffer) {
        let mut text = String::new();
        let mut i = 0;
        while text.len() < 100_000 {
            text.push_str(&format!("line {i} ğüşiöç 漢字 e\u{301}\n"));
            i += 1;
        }
        let mut builder = RopeBuilder::new();
        let mut start = 0;
        while start < text.len() {
            let mut end = (start + 997).min(text.len());
            while !text.is_char_boundary(end) {
                end += 1;
            }
            builder.append(&text[start..end]);
            start = end;
        }
        let rope = builder.finish();
        assert!(rope.chunks().count() > 50);
        (text, Buffer::from_rope(rope))
    }

    #[test]
    fn text_and_chunks_across_rope_chunks() {
        let (text, buf) = big_rope();
        assert_eq!(buf.len_bytes(), text.len());
        let mut out = String::from("junk");
        buf.text(0..buf.len_bytes(), &mut out);
        assert_eq!(out, text);
        let joined: String = buf.chunks(0..buf.len_bytes()).collect();
        assert_eq!(joined, text);
        assert!(buf.chunks(0..buf.len_bytes()).count() > 50);
        for (a, z) in [(5000, 5003), (1000, 3500), (0, 1), (99_000, 100_000)] {
            let (mut a, mut z) = (a, z);
            while !text.is_char_boundary(a) {
                a += 1;
            }
            while !text.is_char_boundary(z) {
                z += 1;
            }
            let j: String = buf.chunks(a..z).collect();
            assert_eq!(j, &text[a..z]);
            buf.text(a..z, &mut out);
            assert_eq!(out, &text[a..z]);
        }
        assert_eq!(buf.chunks(10..10).count(), 0);
        // Lines match str::lines.
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(buf.len_lines(), lines.len() + 1);
        let mut line = String::new();
        for i in [0usize, 1, 17, 500, lines.len() - 1] {
            buf.line_text(i, &mut line);
            assert_eq!(line, lines[i]);
        }
        buf.line_text(lines.len(), &mut line);
        assert_eq!(line, "");
    }

    #[test]
    fn offsets_inside_a_char_and_past_the_end() {
        let buf = b("a\u{11F}b"); // ğ is 2 bytes
        assert_eq!(buf.char_at(0), Some('a'));
        assert_eq!(buf.char_at(1), Some('ğ'));
        assert_eq!(buf.char_at(2), Some('ğ'));
        assert_eq!(buf.char_at(3), Some('b'));
        assert_eq!(buf.char_at(4), None);
        assert!(buf.is_char_boundary(1));
        assert!(!buf.is_char_boundary(2));
        assert!(buf.is_char_boundary(4));
        assert!(buf.is_char_boundary(400));
        assert_eq!(buf.snap_to_grapheme(2), 1);
        assert_eq!(buf.snap_to_grapheme(400), 4);
        assert_eq!(buf.next_grapheme(2), 3);
        assert_eq!(buf.prev_grapheme(2), 0);
        assert_eq!(buf.position(2), Position { line: 0, col: 1 });
        let joined: String = buf.chunks(2..400).collect();
        assert_eq!(joined, "ğb");
        let (from, to) = (3, 2);
        let joined: String = buf.chunks(from..to).collect();
        assert_eq!(joined, "");
        assert_eq!(buf.revision(), 0);
    }

    #[test]
    fn position_and_offset_round_trip() {
        let (text, buf) = big_rope();
        let mut o = 0;
        let mut n = 0;
        while o < text.len() {
            let p = buf.position(o);
            assert_eq!(buf.offset(p), o, "offset {o}");
            // Compare with a direct computation.
            let line = text[..o].matches('\n').count();
            let ls = text[..o].rfind('\n').map_or(0, |i| i + 1);
            assert_eq!(p.line, line);
            assert_eq!(p.col, text[ls..o].chars().count());
            o += 1;
            while o < text.len() && !text.is_char_boundary(o) {
                o += 1;
            }
            n += 1;
            o += 13;
            while o < text.len() && !text.is_char_boundary(o) {
                o += 1;
            }
        }
        assert!(n > 100);
        // Tab counts one column.
        let t = b("\tx\ty");
        assert_eq!(t.position(2), Position { line: 0, col: 2 });
        assert_eq!(t.offset(Position { line: 0, col: 3 }), 3);
    }

    #[test]
    fn offset_clamps() {
        let buf = b("abc\ndef\r\nghi");
        assert_eq!(buf.offset(Position { line: 0, col: 99 }), 3);
        assert_eq!(buf.offset(Position { line: 1, col: 99 }), 7);
        assert_eq!(buf.offset(Position { line: 2, col: 99 }), 12);
        assert_eq!(buf.offset(Position { line: 99, col: 1 }), 10);
        assert_eq!(buf.offset(Position { line: 99, col: 99 }), 12);
        assert_eq!(buf.offset(Position { line: 1, col: 0 }), 4);
        assert_eq!(buf.position(1000), Position { line: 2, col: 3 });
        // A position in the middle of a CRLF reads as past the end of the line.
        assert_eq!(buf.position(8), Position { line: 1, col: 4 });
    }

    #[test]
    fn graphemes() {
        // e + combining acute, ZWJ family, a flag pair, CRLF.
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let flag = "\u{1F1F9}\u{1F1F7}";
        let text = format!("xe\u{301}y{family}{flag}\r\nz");
        let buf = b(&text);
        let mut bounds = vec![0];
        let mut o = 0;
        while o < buf.len_bytes() {
            o = buf.next_grapheme(o);
            bounds.push(o);
        }
        let mut expect = vec![0];
        let mut pos = 0;
        for part in ["x", "e\u{301}", "y", family, flag, "\r\n", "z"] {
            pos += part.len();
            expect.push(pos);
        }
        assert_eq!(bounds, expect);
        // Backwards gives the same boundaries.
        let mut back = vec![buf.len_bytes()];
        let mut o = buf.len_bytes();
        while o > 0 {
            o = buf.prev_grapheme(o);
            back.push(o);
        }
        back.reverse();
        assert_eq!(back, expect);
        // Snap from inside a cluster.
        let e_start = 1;
        assert_eq!(buf.snap_to_grapheme(e_start + 1), e_start);
        assert_eq!(buf.snap_to_grapheme(e_start + 2), e_start);
        assert_eq!(buf.snap_to_grapheme(e_start + 3), e_start + 3);
        let crlf = expect[5];
        assert_eq!(buf.snap_to_grapheme(crlf + 1), crlf);
        assert_eq!(buf.prev_grapheme(crlf + 2), crlf);
        assert_eq!(buf.next_grapheme(crlf), crlf + 2);
        let flag_start = expect[4];
        assert_eq!(buf.snap_to_grapheme(flag_start + 4), flag_start);
        assert_eq!(buf.next_grapheme(0), 1);
        assert_eq!(buf.prev_grapheme(0), 0);
        assert_eq!(buf.next_grapheme(buf.len_bytes()), buf.len_bytes());
    }

    #[test]
    fn graphemes_across_rope_chunks() {
        // A text of combining sequences, regional-indicator pairs and CRLFs, split at awkward
        // places into many rope chunks.
        let mut text = String::new();
        let mut parts: Vec<String> = Vec::new();
        let mut i = 0usize;
        while text.len() < 30_000 {
            let part = match i % 5 {
                0 => "e\u{301}\u{302}".to_string(),
                1 => "\u{1F1F9}\u{1F1F7}".to_string(),
                2 => "\r\n".to_string(),
                3 => "\u{1F468}\u{200D}\u{1F469}".to_string(),
                _ => "a".to_string(),
            };
            text.push_str(&part);
            parts.push(part);
            i += 1;
        }
        let mut builder = RopeBuilder::new();
        let mut start = 0;
        let mut step = 1;
        while start < text.len() {
            let mut end = (start + 600 + step % 7).min(text.len());
            while !text.is_char_boundary(end) {
                end += 1;
            }
            builder.append(&text[start..end]);
            start = end;
            step += 1;
        }
        let buf = Buffer::from_rope(builder.finish());
        assert!(buf.rope().chunks().count() > 20);
        let mut expect = vec![0];
        let mut pos = 0;
        for p in &parts {
            pos += p.len();
            expect.push(pos);
        }
        let mut got = vec![0];
        let mut o = 0;
        while o < buf.len_bytes() {
            o = buf.next_grapheme(o);
            got.push(o);
        }
        assert_eq!(got, expect);
        let mut back = vec![buf.len_bytes()];
        let mut o = buf.len_bytes();
        while o > 0 {
            o = buf.prev_grapheme(o);
            back.push(o);
        }
        back.reverse();
        assert_eq!(back, expect);
        // snap_to_grapheme from every char boundary.
        let set: std::collections::HashSet<usize> = expect.iter().copied().collect();
        for (o, _) in text.char_indices() {
            let s = buf.snap_to_grapheme(o);
            assert!(set.contains(&s) && s <= o);
            assert!(!expect.iter().any(|&e| e > s && e <= o), "offset {o}");
        }
    }

    // ---- replace

    use crate::testutil::{Rng, check_change, check_eols};

    fn floor(s: &str, mut i: usize) -> usize {
        i = i.min(s.len());
        while !s.is_char_boundary(i) {
            i -= 1;
        }
        i
    }

    /// The same edit on a plain string: the floored range.
    fn model_replace(s: &mut String, range: Range<usize>, text: &str) -> (usize, usize) {
        let at = floor(s, range.start);
        let end = floor(s, range.end).max(at);
        s.replace_range(at..end, text);
        (at, end)
    }

    const PIECES: [&str; 9] = ["a", "\r", "\n", "\r\n", "\r", "\n", "é", "x", "漢"];

    fn random_text(rng: &mut Rng, max_pieces: usize) -> String {
        (0..rng.below(max_pieces + 1))
            .map(|_| *rng.pick(&PIECES))
            .collect()
    }

    /// An offset biased to land next to or inside line breaks and multi-byte characters.
    fn random_offset(rng: &mut Rng, s: &str) -> usize {
        let breaks: Vec<usize> = s
            .char_indices()
            .filter(|&(_, c)| c == '\r' || c == '\n')
            .map(|(i, _)| i)
            .collect();
        if !breaks.is_empty() && rng.chance(60) {
            let i = *rng.pick(&breaks);
            return (i + rng.below(4)).saturating_sub(1).min(s.len() + 1);
        }
        rng.below(s.len() + 3)
    }

    #[test]
    fn replace_basics() {
        let mut buf = b("hello\nworld");
        let c = buf.replace(0..5, "HELLO!");
        assert_eq!(buf.rope().to_string(), "HELLO!\nworld");
        assert_eq!(buf.revision(), 1);
        assert_eq!((c.at, c.removed, c.inserted), (0, 5, 6));
        assert_eq!((c.first_line, c.lines_before, c.lines_after), (0, 1, 1));
        assert_eq!(c.line_delta(), 0);

        let c = buf.replace(6..7, "");
        assert_eq!(buf.rope().to_string(), "HELLO!world");
        assert_eq!((c.first_line, c.lines_before, c.lines_after), (0, 2, 1));
        assert_eq!(c.line_delta(), -1);
        assert_eq!(c.eol_removed.lf, 1);

        // Clamped and floored; an offset inside a char moves to the char's start.
        let mut buf = b("a\u{11F}b");
        let c = buf.replace(2..400, "");
        assert_eq!(buf.rope().to_string(), "a");
        assert_eq!((c.at, c.removed), (1, 3));
        let c = buf.replace(9..9, "z");
        assert_eq!(buf.rope().to_string(), "az");
        assert_eq!((c.at, c.inserted), (1, 1));

        // Nothing: no revision.
        let before = buf.revision();
        let c = buf.replace(1..1, "");
        assert_eq!(buf.revision(), before);
        assert_eq!(
            (c.removed, c.inserted, c.lines_before, c.lines_after),
            (0, 0, 1, 1)
        );
        let (hi, lo) = (5, 2);
        let c = buf.replace(hi..lo, ""); // inverted range: empty at its start
        assert_eq!(buf.revision(), before);
        assert_eq!(c.at, 2.min(buf.len_bytes()));

        // Empty buffer.
        let mut e = b("");
        let c = e.replace(0..0, "ab\r\ncd");
        assert_eq!(e.len_lines(), 2);
        assert_eq!((c.first_line, c.lines_before, c.lines_after), (0, 1, 2));
        e.replace(0..e.len_bytes(), "");
        assert_eq!(e.len_bytes(), 0);
        assert_eq!(e.len_lines(), 1);
    }

    #[test]
    fn replace_joining_and_splitting_crlf() {
        // An LF after a CR joins them: the CR's line changes.
        let mut buf = b("a\rb");
        let c = buf.replace(2..2, "\n");
        assert_eq!(buf.rope().to_string(), "a\r\nb");
        assert_eq!(c.first_line, 0);
        assert_eq!(buf.len_lines(), 2);
        // Removing the LF of a CRLF splits... nothing, but the CR's line changes.
        let mut buf = b("a\r\nb");
        let c = buf.replace(2..3, "");
        assert_eq!(buf.rope().to_string(), "a\rb");
        assert_eq!(c.first_line, 0);
        // Text inserted between the CR and the LF of a CRLF.
        let mut buf = b("a\r\nb");
        let c = buf.replace(2..2, "x");
        assert_eq!(buf.rope().to_string(), "a\rx\nb");
        assert_eq!((c.first_line, c.lines_before, c.lines_after), (0, 1, 2));
        assert_eq!(buf.len_lines(), 3);
    }

    #[test]
    fn replace_random_against_a_string() {
        let mut rng = Rng::new(7);
        for round in 0..300 {
            let mut model = random_text(&mut rng, 24);
            let mut buf = b(&model);
            let mut counts = EolCounts::of(&model);
            for step in 0..200 {
                let before = model.clone();
                let r1 = random_offset(&mut rng, &model);
                let r2 = if rng.chance(30) {
                    r1
                } else {
                    random_offset(&mut rng, &model)
                };
                let range = if rng.chance(90) {
                    r1.min(r2)..r1.max(r2)
                } else {
                    r1..r2
                };
                let text = random_text(&mut rng, 4);
                let (at, end) = model_replace(&mut model, range.clone(), &text);
                let rev = buf.revision();
                let c = if rng.chance(25) {
                    buf.replace_rope(range.clone(), &Rope::from_str(&text))
                } else {
                    buf.replace(range.clone(), &text)
                };
                let ctx = format!("round {round} step {step}: {before:?} {range:?} {text:?}");
                assert_eq!(buf.rope().to_string(), model, "{ctx}");
                assert_eq!(
                    (c.at, c.removed, c.inserted),
                    (at, end - at, text.len()),
                    "{ctx}"
                );
                assert_eq!(
                    buf.revision(),
                    rev + u64::from(end > at || !text.is_empty())
                );
                assert_eq!(buf.len_lines(), lines_len(&model));
                check_change(&before, &model, &c);
                check_eols(&mut counts, &model, &c);
            }
        }
    }

    fn lines_len(s: &str) -> usize {
        crate::testutil::lines_of(s).len()
    }

    #[test]
    fn replace_with_a_large_rope_joins_crlf_at_the_seams() {
        // Many chunks so that the seams are real chunk boundaries.
        let big: String = "ab\r\n".repeat(5000);
        let (cr_first, lf_first) = (format!("{big}\r"), format!("\n{big}"));
        let mut buf = b("x\ry");
        let c = buf.replace_rope(2..2, &Rope::from_str(&lf_first));
        let want = format!("x\r{lf_first}y");
        assert_eq!(buf.rope().to_string(), want);
        assert_eq!(c.first_line, 0);
        let mut buf = b("x\ny");
        buf.replace_rope(1..1, &Rope::from_str(&cr_first));
        assert_eq!(buf.rope().to_string(), format!("x{cr_first}\ny"));
        assert_eq!(buf.len_lines(), lines_len(&buf.rope().to_string()));
        assert_eq!(buf.len_lines(), 5000 + 2);
    }

    #[test]
    fn merge_of_two_changes_is_one_change() {
        let mut rng = Rng::new(11);
        for round in 0..400 {
            let mut model = random_text(&mut rng, 30);
            let mut buf = b(&model);
            let start = model.clone();
            let start_counts = EolCounts::of(&model);
            let mut merged: Option<Change> = None;
            // One to four edits, merged one by one.
            for _ in 0..1 + rng.below(4) {
                let r1 = random_offset(&mut rng, &model);
                let r2 = random_offset(&mut rng, &model);
                let text = random_text(&mut rng, 4);
                let range = r1.min(r2)..r1.max(r2);
                model_replace(&mut model, range.clone(), &text);
                let c = buf.replace(range, &text);
                match merged.as_mut() {
                    Some(m) => m.merge(&c),
                    None => merged = Some(c),
                }
            }
            let m = merged.unwrap();
            assert_eq!(buf.rope().to_string(), model, "round {round}");
            check_change(&start, &model, &m);
            let mut counts = start_counts;
            check_eols(&mut counts, &model, &m);
        }
    }

    #[test]
    fn merge_with_a_no_change() {
        let mut buf = b("abc\ndef");
        let mut m = buf.replace(0..1, "xy");
        let none = buf.replace(3..3, "");
        let snapshot = m.clone();
        m.merge(&none);
        assert_eq!(m, snapshot);
        let mut n = none.clone();
        n.merge(&snapshot);
        assert_eq!(n, snapshot);
    }

    #[test]
    fn replace_in_a_large_buffer_keeps_chunks_shared() {
        let (text, mut buf) = big_rope();
        let snap = buf.snapshot();
        let c = buf.replace(50_000..50_010, "Z");
        assert!(c.removed >= 9 && c.removed <= 12);
        // The snapshot is unchanged.
        assert_eq!(snap.to_string(), text);
        assert_ne!(buf.rope().to_string(), text);
    }
}
