//! Line-editing commands: duplicate, move and delete lines, toggle line comments, change case,
//! trim trailing whitespace, tabs to spaces. They have the shape of the commands in
//! [`crate::edit`]: each is exactly one undo step (`Kind::Other`), returns the new selection and
//! one merged [`Change`](crate::Change), and returns the selection unchanged with no change (and
//! no history entry) when it would change no byte.
//!
//! "The lines of the selection" are those from the line holding the selection's start to the one
//! holding its end, except that a non-empty selection ending at column 0 of a line does not
//! include that line (the rule `indent` and `outdent` use).
//!
//! The two whole-document commands, [`trim_trailing_whitespace`] and [`tabs_to_spaces`], walk the
//! lines one at a time and replace in place, front to back; they never build the document as one
//! `String`. Their cost is one rope replacement (O(log n)) and one history record per changed
//! line (per tab, for tabs), plus the removed text kept for undo.

use crate::buffer::Buffer;
use crate::display::Cols;
use crate::edit::{Cmd, Outcome, clamped, nothing, touched_lines};
use crate::history::{History, Kind};
use crate::limits::LINE_BLOCK_BYTES;
use crate::selection::Selection;
use std::ops::Range;

fn sel_of(anchor: usize, caret: usize) -> Selection {
    Selection {
        anchor,
        caret,
        goal_col: None,
    }
}

/// The line has a line break after its text (the last line may not).
fn has_break(buf: &Buffer, line: usize) -> bool {
    buf.line_range(line).end != buf.line_range_with_break(line).end
}

fn text_of(buf: &Buffer, range: Range<usize>) -> String {
    let mut s = String::new();
    buf.text(range, &mut s);
    s
}

/// Inserts a copy of the lines of the selection directly below them. The selection (or caret)
/// ends up on the copy, at the same columns. A last line with no line break is joined to its copy
/// with `eol`.
pub fn duplicate_lines(buf: &mut Buffer, hist: &mut History, sel: Selection, eol: &str) -> Outcome {
    let r = clamped(buf, sel);
    let (first, last) = touched_lines(buf, &r);
    duplicate_range(buf, hist, sel, first, last, eol)
}

/// [`duplicate_lines`] for the lines `first..=last`; `sel` is moved by the copy's distance.
pub(crate) fn duplicate_range(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    first: usize,
    last: usize,
    eol: &str,
) -> Outcome {
    let start = buf.line_start(first);
    let end = buf.line_range_with_break(last).end;
    let mut text = String::new();
    let copy_start = if has_break(buf, last) {
        end
    } else {
        text.push_str(eol);
        end + eol.len()
    };
    for piece in buf.chunks(start..end) {
        text.push_str(piece);
    }
    let d = copy_start - start;
    let len = buf.len_bytes();
    let after = sel_of(sel.anchor.min(len) + d, sel.caret.min(len) + d);
    let mut cmd = Cmd::new(buf);
    cmd.replace(end..end, &text);
    cmd.finish(hist, Kind::Other, sel, after)
}

/// Moves the lines of the selection one line up or down, the selection moving with them. Nothing
/// at the top or bottom edge, or when the swap changes no byte.
///
/// Each line moves with its own line break. When the last line of the document is involved and
/// has no break, the document still ends without one: the line that was last keeps no break
/// and takes the break of the line that is now last (so no `eol` is ever needed; the parameter is
/// kept for the callers' symmetry with `duplicate_lines`).
pub fn move_lines(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    down: bool,
    _eol: &str,
) -> Outcome {
    let r = clamped(buf, sel);
    let (first, last) = touched_lines(buf, &r);
    move_range(buf, hist, sel, first, last, down)
}

/// [`move_lines`] for the lines `first..=last`; `sel` is carried with them.
pub(crate) fn move_range(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    first: usize,
    last: usize,
    down: bool,
) -> Outcome {
    let r = clamped(buf, sel);
    // The region: the block and the one line it swaps with.
    let (r0, r1) = if down {
        if last + 1 >= buf.len_lines() {
            return nothing(sel);
        }
        (first, last + 1)
    } else {
        if first == 0 {
            return nothing(sel);
        }
        (first - 1, last)
    };
    let n = r1 - r0 + 1;
    struct Ln {
        start: usize,
        content: String,
        brk: String,
    }
    let lines: Vec<Ln> = (r0..=r1)
        .map(|l| {
            let c = buf.line_range(l);
            let w = buf.line_range_with_break(l);
            Ln {
                start: w.start,
                content: text_of(buf, c.clone()),
                brk: text_of(buf, c.end..w.end),
            }
        })
        .collect();
    // `order[slot]` is the region index of the line that ends up in that slot.
    let order: Vec<usize> = if down {
        std::iter::once(n - 1).chain(0..n - 1).collect()
    } else {
        (1..n).chain(std::iter::once(0)).collect()
    };
    let mut brk: Vec<&str> = lines.iter().map(|l| l.brk.as_str()).collect();
    if brk[n - 1].is_empty() {
        let y = order[n - 1];
        brk[n - 1] = brk[y];
        brk[y] = "";
    }
    let mut new_text = String::new();
    let mut slot_start = Vec::with_capacity(n + 1);
    let mut item_start = vec![0; n];
    for &i in &order {
        slot_start.push(new_text.len());
        item_start[i] = new_text.len();
        new_text.push_str(&lines[i].content);
        new_text.push_str(brk[i]);
    }
    slot_start.push(new_text.len());
    let old: String = lines.iter().flat_map(|l| [&*l.content, &*l.brk]).collect();
    if old == new_text {
        return nothing(sel);
    }
    let base = lines[0].start;
    // The block's lines, as region indexes, and where it ends in the new text.
    let (b0, b1, end_slot) = if down { (0, n - 1, n) } else { (1, n, n - 1) };
    let new_block_end = base + slot_start[end_slot];
    // A selection ending at column 0 after the block ends after the block, wherever it goes.
    let trailing = !sel.is_empty() && buf.line_of(r.end) > last;
    let map = |p: usize| {
        let p = p.min(base + old.len());
        if trailing && p == r.end {
            return new_block_end;
        }
        let i = (b0..b1).rfind(|&i| lines[i].start <= p).unwrap_or(b0);
        base + item_start[i] + (p - lines[i].start).min(lines[i].content.len())
    };
    let after = sel_of(map(sel.anchor), map(sel.caret));
    let mut cmd = Cmd::new(buf);
    cmd.replace(base..base + old.len(), &new_text);
    cmd.finish(hist, Kind::Other, sel, after)
}

/// Removes the lines of the selection with their line breaks. When the block includes the last
/// line and that has no break, the break before the block goes too, so no empty last line is
/// left. The caret goes to the same column (clamped to the line, snapped to a grapheme boundary)
/// of the line that takes the block's place, the new last line when there is none after.
pub fn delete_lines(buf: &mut Buffer, hist: &mut History, sel: Selection) -> Outcome {
    let r = clamped(buf, sel);
    let (first, last) = touched_lines(buf, &r);
    delete_range(buf, hist, sel, first, last)
}

/// [`delete_lines`] for the lines `first..=last`; the caret's column is that of `sel.caret`.
pub(crate) fn delete_range(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    first: usize,
    last: usize,
) -> Outcome {
    let start = buf.line_start(first);
    let end = buf.line_range_with_break(last).end;
    let caret = buf.floor_char(sel.caret);
    let col = caret - buf.line_start(buf.line_of(caret));
    let in_line = |line: usize| {
        let c = buf.line_range(line);
        buf.floor_char(c.start + col.min(c.end - c.start))
    };
    let (cut, pos) = if has_break(buf, last) {
        // The line after the block exists (maybe empty); it moves up to `start`.
        (start..end, in_line(last + 1) - (end - start))
    } else if first > 0 {
        (buf.line_range(first - 1).end..end, in_line(first - 1))
    } else {
        (0..end, 0)
    };
    let mut cmd = Cmd::new(buf);
    cmd.replace(cut, "");
    cmd.finish(hist, Kind::Other, sel, Selection::at(pos))
}

/// What `toggle_comment` needs to know of a line.
struct LineInfo {
    start: usize,
    /// Bytes of leading spaces and tabs.
    indent: usize,
    blank: bool,
    /// After the indentation comes the prefix.
    commented: bool,
    /// ... and a space after that.
    space: bool,
}

fn line_info(buf: &Buffer, line: usize, prefix: &str) -> LineInfo {
    let r = buf.line_range(line);
    let mut indent = 0;
    let mut blank = true;
    'scan: for piece in buf.chunks(r.clone()) {
        for b in piece.bytes() {
            if b != b' ' && b != b'\t' {
                blank = false;
                break 'scan;
            }
            indent += 1;
        }
    }
    let mut info = LineInfo {
        start: r.start,
        indent,
        blank,
        commented: false,
        space: false,
    };
    if !blank {
        let from = r.start + indent;
        let to = (from + prefix.len() + 1).min(r.end);
        let head: Vec<u8> = buf.chunks(from..to).flat_map(str::bytes).collect();
        info.commented = head.starts_with(prefix.as_bytes());
        info.space = info.commented && head.get(prefix.len()) == Some(&b' ');
    }
    info
}

/// What a line comment toggle does to some lines: ascending (position, bytes removed) edits,
/// inserting being `removed == 0`, and how a selection end maps through them.
pub(crate) struct CommentPlan {
    edits: Vec<(usize, usize)>,
    stay: bool,
    inserted: String,
}

impl CommentPlan {
    /// Where the offset `p` of the text before the edits is after them.
    pub(crate) fn map(&self, p: usize) -> usize {
        let mut q = p;
        for &(pos, rem) in &self.edits {
            if rem == 0 {
                if p > pos || (p == pos && !self.stay) {
                    q += self.inserted.len();
                }
            } else if p >= pos + rem {
                q -= rem;
            } else if p > pos {
                q -= p - pos;
            }
        }
        q
    }

    /// Makes the edits, back to front.
    pub(crate) fn apply(&self, cmd: &mut Cmd<'_>) {
        for &(pos, rem) in self.edits.iter().rev() {
            cmd.replace(pos..pos + rem, if rem == 0 { &self.inserted } else { "" });
        }
    }
}

/// The toggle of `prefix` over `lines` (ascending, no repeats): see [`toggle_comment`]. When every
/// line is blank, `caret_lines` (inside `lines`) are the ones commented, each at the end of its
/// indentation, the caret after the comment. `None` when `prefix` or `lines` is empty.
pub(crate) fn comment_plan(
    buf: &Buffer,
    lines: &[usize],
    caret_lines: &[usize],
    prefix: &str,
) -> Option<CommentPlan> {
    if prefix.is_empty() || lines.is_empty() {
        return None;
    }
    let infos: Vec<LineInfo> = lines.iter().map(|&l| line_info(buf, l, prefix)).collect();
    let inserted = format!("{prefix} ");
    let mut edits: Vec<(usize, usize)> = Vec::new();
    let mut stay = true;
    let solid: Vec<&LineInfo> = infos.iter().filter(|i| !i.blank).collect();
    if solid.is_empty() {
        for &line in caret_lines {
            if let Ok(k) = lines.binary_search(&line) {
                edits.push((infos[k].start + infos[k].indent, 0));
            }
        }
        stay = false;
    } else if solid.iter().all(|i| i.commented) {
        for i in &solid {
            edits.push((i.start + i.indent, prefix.len() + usize::from(i.space)));
        }
    } else {
        let col = solid.iter().map(|i| i.indent).min().unwrap_or(0);
        edits.extend(solid.iter().map(|i| (i.start + col, 0)));
    }
    Some(CommentPlan {
        edits,
        stay,
        inserted,
    })
}

/// Line comments over the lines of the selection, blank lines ignored. When every other line
/// starts (after its indentation) with `prefix`, removes it and one space after it, if there is
/// one, from each. Otherwise inserts `prefix` and a space at the smallest indentation of those
/// lines (the same byte column on each). When all lines are blank, the caret's line (kept inside
/// the lines of the selection) is commented at the end of its indentation.
///
/// A selection end after an insertion point moves by the inserted length, one at the point stays
/// in front of it (except for the all-blank case, where the caret goes after the comment); an end
/// inside removed text goes to its start. `prefix` empty: nothing.
pub fn toggle_comment(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    prefix: &str,
) -> Outcome {
    let r = clamped(buf, sel);
    let (first, last) = touched_lines(buf, &r);
    let lines: Vec<usize> = (first..=last).collect();
    let caret = buf.floor_char(sel.caret);
    let caret_line = buf.line_of(caret).clamp(first, last);
    let Some(plan) = comment_plan(buf, &lines, &[caret_line], prefix) else {
        return nothing(sel);
    };
    let after = sel_of(plan.map(sel.anchor), plan.map(sel.caret));
    let mut cmd = Cmd::new(buf);
    plan.apply(&mut cmd);
    cmd.finish(hist, Kind::Other, sel, after)
}

/// Selections over this many bytes are converted piece by piece.
const CASE_CHUNK: usize = 1 << 20;

/// Replaces the selected text with its `to_uppercase()` (`upper`) or `to_lowercase()`: Rust's
/// Unicode rules, no locale. The selection then covers the result, in the same direction. Nothing
/// when the selection is empty or the text does not change.
///
/// A selection over 1 MiB is converted in pieces cut after a whitespace character (so the final
/// sigma rule of `to_lowercase` sees the same neighbours as it would in the whole text), at most
/// a few KiB past 1 MiB each, so the memory used besides the history stays bounded; it is still
/// one undo step.
pub fn change_case(buf: &mut Buffer, hist: &mut History, sel: Selection, upper: bool) -> Outcome {
    let r = clamped(buf, sel);
    if r.is_empty() {
        return nothing(sel);
    }
    let mut cmd = Cmd::new(buf);
    // Where the part not converted yet starts in the buffer as it is now, and how long it is.
    let mut now = r.start;
    let mut rest = r.end - r.start;
    let mut piece = String::new();
    while rest > 0 {
        let mut end = cmd.buf.floor_char(now + rest.min(CASE_CHUNK));
        if end < now + rest {
            let probe = cmd.buf.floor_char((end + 4096).min(now + rest));
            let tail = text_of(cmd.buf, end..probe);
            if let Some((i, c)) = tail.char_indices().find(|(_, c)| c.is_whitespace()) {
                end += i + c.len_utf8();
            }
        }
        if end <= now {
            end = now + rest;
        }
        cmd.buf.text(now..end, &mut piece);
        let converted = if upper {
            piece.to_uppercase()
        } else {
            piece.to_lowercase()
        };
        if converted != piece {
            cmd.replace(now..end, &converted);
        }
        rest -= piece.len();
        now += converted.len();
    }
    let after = if sel.anchor <= sel.caret {
        sel_of(r.start, now)
    } else {
        sel_of(now, r.start)
    };
    cmd.finish(hist, Kind::Other, sel, after)
}

/// Removes the spaces and tabs before the line break of every line of the document; the breaks
/// stay as they are. A selection end inside the removed run goes to its start, one after it moves
/// back by the run's length. The document is walked line by line, front to back, replacing in
/// place.
pub fn trim_trailing_whitespace(buf: &mut Buffer, hist: &mut History, sel: Selection) -> Outcome {
    let len = buf.len_bytes();
    let (mut a, mut c) = (sel.anchor.min(len), sel.caret.min(len));
    let mut cmd = Cmd::new(buf);
    trim_into(&mut cmd, &mut a, &mut c);
    cmd.finish(hist, Kind::Other, sel, sel_of(a, c))
}

/// The walk of [`trim_trailing_whitespace`] inside a command, so `prefs::apply_on_save` can make
/// it part of a bigger step. `a` and `c` are selection ends (offsets in the buffer as it is now)
/// and are moved as the text goes.
pub(crate) fn trim_into(cmd: &mut Cmd<'_>, a: &mut usize, c: &mut usize) {
    let (oa, oc) = (*a, *c);
    // Bytes removed so far: the buffer's offsets are the original ones minus this.
    let mut removed = 0;
    let mut total = cmd.buf.len_lines();
    let mut line = 0;
    while line < total {
        let range = cmd.buf.line_range(line);
        let rope = cmd.buf.rope();
        let mut from = range.end;
        while from > range.start && matches!(rope.byte(from - 1), b' ' | b'\t') {
            from -= 1;
        }
        if from == range.end {
            line += 1;
            continue;
        }
        let n = range.end - from;
        let s = from + removed;
        for end in [(&oa, &mut *a), (&oc, &mut *c)] {
            if *end.0 >= s + n {
                *end.1 -= n;
            } else if *end.0 > s {
                *end.1 -= *end.0 - s;
            }
        }
        cmd.replace(from..range.end, "");
        removed += n;
        // Taking spaces from between a CR and an LF makes them one CRLF: a line fewer.
        let now = cmd.buf.len_lines();
        line = line + 1 - (total - now);
        total = now;
    }
}

/// Tabs replaced per batch within one line, so a line of millions of tabs does not need a list
/// of them all.
const TAB_BATCH: usize = 1 << 16;

/// The spaces for one tab.
fn spaces(n: usize) -> String {
    " ".repeat(n)
}

/// Replaces every tab of the document with the spaces up to the next tab stop of `tab_width`
/// columns. Columns are counted as the rest of the crate counts them (`display::Cols`): from the
/// line start, a tab to its stop and every other character as one column, a wide character or a
/// combining mark included (the view scales those separately). A selection end moves with its
/// text. The document is walked line by line, front to back.
pub fn tabs_to_spaces(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    tab_width: usize,
) -> Outcome {
    let tab = tab_width.max(1);
    let len = buf.len_bytes();
    let (oa, oc) = (sel.anchor.min(len), sel.caret.min(len));
    let (mut a, mut c) = (oa, oc);
    let mut cmd = Cmd::new(buf);
    // How far the buffer's offsets are ahead of the original ones.
    let mut delta = 0usize;
    for line in 0..cmd.buf.len_lines() {
        let mut at = cmd.buf.line_start(line);
        let mut col = 0usize;
        loop {
            let end = cmd.buf.line_range(line).end;
            // (offset now, spaces) of the next tabs, and whether the scan stopped at the batch.
            let mut reps: Vec<(usize, usize)> = Vec::new();
            let mut pos = at;
            let mut full = false;
            'scan: for piece in cmd.buf.chunks(at..end) {
                let mut from = 0;
                for t in memchr::memchr_iter(b'\t', piece.as_bytes()) {
                    col = Cols {
                        col,
                        ..Cols::default()
                    }
                    .after(&piece[from..t], tab)
                    .col;
                    let n = tab - col % tab;
                    reps.push((pos + t, n));
                    col += n;
                    from = t + 1;
                    if reps.len() == TAB_BATCH {
                        full = true;
                        break 'scan;
                    }
                }
                col = Cols {
                    col,
                    ..Cols::default()
                }
                .after(&piece[from..], tab)
                .col;
                pos += piece.len();
            }
            let Some(&(last_at, _)) = reps.last() else {
                break;
            };
            let mut grow = 0;
            for &(p, n) in &reps {
                let orig = p - delta;
                if oa > orig {
                    a += n - 1;
                }
                if oc > orig {
                    c += n - 1;
                }
                grow += n - 1;
            }
            for &(p, n) in reps.iter().rev() {
                cmd.replace(p..p + 1, &spaces(n));
            }
            delta += grow;
            if !full {
                break;
            }
            at = last_at + 1 + grow;
        }
    }
    cmd.finish(hist, Kind::Other, sel, sel_of(a, c))
}

/// Moves marked lines (sorted line numbers, such as bookmarks) with an edit: a mark below the
/// lines the edit touched moves by the lines it added or removed, a mark on a touched line that
/// is no longer there goes to the last line they became, and marks that meet become one.
pub fn shift_marks(marks: &mut Vec<usize>, change: &crate::buffer::Change) {
    if marks.is_empty() || change.lines_before == change.lines_after {
        return;
    }
    let (first, before, after) = (change.first_line, change.lines_before, change.lines_after);
    for m in marks.iter_mut() {
        if *m >= first + before {
            *m = *m + after - before;
        } else if *m >= first + after {
            *m = first + after - 1;
        }
    }
    marks.dedup();
}

/// The next line break (LF, CRLF or CR) at or after `from`: its start and its length. Other
/// Unicode breaks (VT, FF, NEL, LS, PS) are not breaks here, as in the buffer's line model.
fn next_break(rope: &ropey::Rope, from: usize) -> Option<(usize, usize)> {
    let len = rope.len_bytes();
    let mut base = from;
    for piece in rope.byte_slice(from..).chunks() {
        if let Some(j) = memchr::memchr2(b'\n', b'\r', piece.as_bytes()) {
            let at = base + j;
            let crlf = rope.byte(at) == b'\r' && at + 1 < len && rope.byte(at + 1) == b'\n';
            return Some((at, if crlf { 2 } else { 1 }));
        }
        base += piece.len();
    }
    None
}

/// Makes every line break of the document `to` ("\r\n", "\n" or "\r"), as one undo step. A CRLF
/// pair is one break. Nothing when every break already is `to` (or `to` is none of the three).
/// Other Unicode line separators (VT, FF, NEL, LS, PS) are not breaks to this crate and stay.
///
/// A selection end before a break stays on its text, one inside a CRLF pair goes to the start of
/// its replacement, one after it moves by the difference in length. The document is walked front
/// to back, replacing break by break in place; it never becomes one `String`. The scan reads
/// only text ahead of the walk, which is still the original, so a replacement next to an
/// unconverted break (a CR made, then an LF) is never mistaken for a pair.
pub fn convert_line_endings(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    to: &str,
) -> Outcome {
    if !matches!(to, "\r\n" | "\n" | "\r") {
        return nothing(sel);
    }
    let len = buf.len_bytes();
    let (oa, oc) = (sel.anchor.min(len) as isize, sel.caret.min(len) as isize);
    let (mut a, mut c) = (oa, oc);
    let mut cmd = Cmd::new(buf);
    // How far the buffer's offsets are ahead of the original ones.
    let mut delta = 0isize;
    let mut pos = 0;
    while let Some((at, n)) = next_break(cmd.buf.rope(), pos) {
        if n == to.len() && cmd.buf.rope().byte(at) == to.as_bytes()[0] {
            pos = at + n;
            continue;
        }
        let orig = at as isize - delta;
        let d = to.len() as isize - n as isize;
        for (o, e) in [(oa, &mut a), (oc, &mut c)] {
            if o >= orig + n as isize {
                *e += d;
            } else if o > orig {
                *e = orig + delta;
            }
        }
        cmd.replace(at..at + n, to);
        delta += d;
        pos = at + to.len();
    }
    cmd.finish(hist, Kind::Other, sel, sel_of(a as usize, c as usize))
}

/// How the lines of a block are put in order by [`sort_lines`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortOrder {
    /// By the bytes of the line's text (UTF-8, so by code point), smallest first.
    Ascending,
    /// The reverse of `Ascending`; lines that compare equal keep their order.
    Descending,
    /// Ascending, comparing each char lowercased by its simple mapping (`char::to_lowercase`,
    /// its first char where Unicode gives more than one: "İ" is "i"), so "B" sorts after "a".
    /// Lines that compare equal keep their order.
    IgnoreCase,
}

/// The lines the whole-block line commands work on, as (first, last): the lines of the selection
/// ([`touched_lines`]), or every line of the document when the selection is empty and
/// `whole_if_empty` is set. A last line that is empty and has no break (the text after the
/// document's final line break) is left out when other lines remain, so a sort does not move the
/// final break.
fn block_lines(buf: &Buffer, sel: Selection, whole_if_empty: bool) -> (usize, usize) {
    let r = clamped(buf, sel);
    let (first, last) = if r.is_empty() && whole_if_empty {
        (0, buf.len_lines() - 1)
    } else {
        touched_lines(buf, &r)
    };
    block_range(buf, first, last)
}

/// The block of `first..=last` for the whole-block commands: a last line that is empty and has
/// no break is left out when other lines remain.
pub(crate) fn block_range(buf: &Buffer, first: usize, mut last: usize) -> (usize, usize) {
    if last > first && buf.line_range_with_break(last).is_empty() {
        last -= 1;
    }
    (first, last)
}

/// The block `first..=last` is over [`LINE_BLOCK_BYTES`].
pub(crate) fn range_too_large(buf: &Buffer, first: usize, last: usize) -> bool {
    buf.line_range_with_break(last).end - buf.line_start(first) > LINE_BLOCK_BYTES
}

/// Whether the block `sort_lines`, `unique_lines`, `reverse_lines` or `join_lines` would work on
/// is over [`LINE_BLOCK_BYTES`]: those commands then do nothing. They take the whole document
/// when the selection is empty (`whole_if_empty` true); pass false to ask about the caret's line
/// alone.
pub fn block_too_large(buf: &Buffer, sel: Selection, whole_if_empty: bool) -> bool {
    let (first, last) = block_lines(buf, sel, whole_if_empty);
    range_too_large(buf, first, last)
}

/// The lines `first..=last` as their texts (without breaks) and their breaks.
fn read_block(buf: &Buffer, first: usize, last: usize) -> (Vec<String>, Vec<String>) {
    let mut texts = Vec::with_capacity(last - first + 1);
    let mut breaks = Vec::with_capacity(last - first + 1);
    for l in first..=last {
        let c = buf.line_range(l);
        let w = buf.line_range_with_break(l);
        texts.push(text_of(buf, c.clone()));
        breaks.push(text_of(buf, c.end..w.end));
    }
    (texts, breaks)
}

/// Writes `texts` with `breaks` between them (the last text gets no break: the last line's break
/// stays where it is) over the lines `first..=last`, trimmed to the bytes that differ, and ends
/// the command with the selection over the lines' new text. Nothing when no byte differs.
fn put_block(
    mut cmd: Cmd<'_>,
    hist: &mut History,
    sel: Selection,
    first: usize,
    last: usize,
    texts: &[String],
    breaks: &[String],
) -> Outcome {
    let start = cmd.buf.line_start(first);
    let old_end = cmd.buf.line_range(last).end;
    let old = text_of(cmd.buf, start..old_end);
    let mut new = String::with_capacity(old.len());
    for (i, t) in texts.iter().enumerate() {
        new.push_str(t);
        if i + 1 < texts.len() {
            new.push_str(&breaks[i]);
        }
    }
    let (ob, nb) = (old.as_bytes(), new.as_bytes());
    let mut head = ob.iter().zip(nb).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(head) {
        head -= 1;
    }
    let max_tail = ob.len().min(nb.len()) - head;
    let mut tail = ob
        .iter()
        .rev()
        .zip(nb.iter().rev())
        .take(max_tail)
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(ob.len() - tail) || !new.is_char_boundary(nb.len() - tail) {
        tail -= 1;
    }
    let after = sel_of(start, start + new.len());
    cmd.replace(start + head..old_end - tail, &new[head..nb.len() - tail]);
    cmd.finish(hist, Kind::Other, sel, after)
}

/// Puts the lines of the selection in order, or every line of the document when nothing is
/// selected. Each line keeps its own line break where it stands (the lines move, the breaks stay
/// in their places), so the document's line endings, mixed or not, and whether it ends with a
/// break are as they were. A final empty line (after the document's last break) is not sorted.
/// The sort is stable. The selection then runs from the start of the first line to the end of
/// the last line's text. Nothing when the block is one line, already in order, or over
/// [`LINE_BLOCK_BYTES`].
pub fn sort_lines(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    order: SortOrder,
) -> Outcome {
    let (first, last) = block_lines(buf, sel, true);
    sort_range(buf, hist, sel, first, last, order)
}

/// [`sort_lines`] for the block `first..=last` (as [`block_lines`] gives it).
pub(crate) fn sort_range(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    first: usize,
    last: usize,
    order: SortOrder,
) -> Outcome {
    if first == last || range_too_large(buf, first, last) {
        return nothing(sel);
    }
    let (texts, breaks) = read_block(buf, first, last);
    let mut keyed: Vec<(String, usize)> = texts
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let key = match order {
                SortOrder::IgnoreCase => fold_case(t),
                _ => String::new(),
            };
            (key, i)
        })
        .collect();
    match order {
        SortOrder::Ascending => keyed.sort_by(|a, b| texts[a.1].cmp(&texts[b.1])),
        SortOrder::Descending => keyed.sort_by(|a, b| texts[b.1].cmp(&texts[a.1])),
        SortOrder::IgnoreCase => keyed.sort_by(|a, b| a.0.cmp(&b.0)),
    }
    let sorted: Vec<String> = keyed.iter().map(|&(_, i)| texts[i].clone()).collect();
    put_block(Cmd::new(buf), hist, sel, first, last, &sorted, &breaks)
}

/// `s` with every char lowercased by its simple mapping.
fn fold_case(s: &str) -> String {
    s.chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect()
}

/// Reverses the order of the lines of the selection, or of the whole document when nothing is
/// selected. Breaks stay in their places as for [`sort_lines`], as does the selection rule.
/// Nothing when the block is one line, its lines read the same reversed, or it is over
/// [`LINE_BLOCK_BYTES`].
pub fn reverse_lines(buf: &mut Buffer, hist: &mut History, sel: Selection) -> Outcome {
    let (first, last) = block_lines(buf, sel, true);
    reverse_range(buf, hist, sel, first, last)
}

/// [`reverse_lines`] for the block `first..=last`.
pub(crate) fn reverse_range(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    first: usize,
    last: usize,
) -> Outcome {
    if first == last || range_too_large(buf, first, last) {
        return nothing(sel);
    }
    let (mut texts, breaks) = read_block(buf, first, last);
    texts.reverse();
    put_block(Cmd::new(buf), hist, sel, first, last, &texts, &breaks)
}

/// Removes every line of the selection (the whole document when nothing is selected) that has
/// the same text as an earlier line of the block, keeping the first of each; the order of the
/// rest is kept. Texts are compared byte for byte, so "a" and "A", or "a" and "a " are different;
/// two empty lines are duplicates. A removed line goes with its own break, except that the last
/// line of the block hands its break (or the lack of one) to the last line that stays, so the
/// document still ends as it did. A final empty line (after the document's last break) is not
/// part of the block. The selection then covers the remaining lines as for [`sort_lines`].
/// Nothing when there is no duplicate, or the block is over [`LINE_BLOCK_BYTES`].
pub fn unique_lines(buf: &mut Buffer, hist: &mut History, sel: Selection) -> Outcome {
    let (first, last) = block_lines(buf, sel, true);
    unique_range(buf, hist, sel, first, last)
}

/// [`unique_lines`] for the block `first..=last`.
pub(crate) fn unique_range(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    first: usize,
    last: usize,
) -> Outcome {
    if first == last || range_too_large(buf, first, last) {
        return nothing(sel);
    }
    let (texts, breaks) = read_block(buf, first, last);
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let keep: Vec<usize> = (0..texts.len())
        .filter(|&i| seen.insert(texts[i].as_str()))
        .collect();
    if keep.len() == texts.len() {
        return nothing(sel);
    }
    let kept: Vec<String> = keep.iter().map(|&i| texts[i].clone()).collect();
    let mut kept_breaks: Vec<String> = keep.iter().map(|&i| breaks[i].clone()).collect();
    if let (Some(b), Some(end)) = (kept_breaks.last_mut(), breaks.last()) {
        b.clone_from(end);
    }
    // Fewer lines than before: the whole block is replaced.
    let start = buf.line_start(first);
    let old_end = buf.line_range(last).end;
    let mut new = String::new();
    for (i, t) in kept.iter().enumerate() {
        new.push_str(t);
        if i + 1 < kept.len() {
            new.push_str(&kept_breaks[i]);
        }
    }
    let after = sel_of(start, start + new.len());
    let mut cmd = Cmd::new(buf);
    cmd.replace(start..old_end, &new);
    cmd.finish(hist, Kind::Other, sel, after)
}

/// Joins lines. With a selection over several lines ([`touched_lines`]), joins all of them into
/// the first; with no selection, or one inside a single line, joins that line (the caret's) with
/// the next one. Each line break, with the leading spaces and tabs (any Unicode whitespace) of the
/// line after it, becomes one space, or nothing when that line is empty or only whitespace. The
/// first line's own text, trailing whitespace included, is kept as it is. The joined line ends
/// with the break of the last line joined. With an empty selection the caret goes to the first
/// join point (where the first space is, or would be); otherwise the selection covers the joined
/// line's text. Nothing when there is no next line, or the block is over [`LINE_BLOCK_BYTES`].
pub fn join_lines(buf: &mut Buffer, hist: &mut History, sel: Selection) -> Outcome {
    let r = clamped(buf, sel);
    let (first, mut last) = touched_lines(buf, &r);
    if first == last {
        last = first + 1;
    }
    join_range(buf, hist, sel, first, last, r.is_empty())
}

/// [`join_lines`] for the lines `first..=last` (`last` after `first`): with `caret` the caret
/// goes to the first join point, else the selection covers the joined line.
pub(crate) fn join_range(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    first: usize,
    last: usize,
    caret: bool,
) -> Outcome {
    if last >= buf.len_lines()
        || buf.line_range_with_break(last).end - buf.line_start(first) > LINE_BLOCK_BYTES
    {
        return nothing(sel);
    }
    let from = buf.line_range(first).end;
    let to = buf.line_range(last).end;
    let mut joined = String::new();
    let mut piece = String::new();
    for l in first + 1..=last {
        buf.line_text(l, &mut piece);
        let t = piece.trim_start();
        if !t.is_empty() {
            joined.push(' ');
            joined.push_str(t);
        }
    }
    let start = buf.line_start(first);
    let end = start + (from - start) + joined.len();
    let after = if caret {
        Selection::at(from)
    } else {
        sel_of(start, end)
    };
    let mut cmd = Cmd::new(buf);
    cmd.replace(from..to, &joined);
    cmd.finish(hist, Kind::Other, sel, after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{backspace, insert, redo, undo};
    use crate::eol::EolCounts;
    use crate::testutil::{Rng, check_change, check_eols, lines_of};
    use unicode_segmentation::UnicodeSegmentation;

    fn txt(buf: &Buffer) -> String {
        buf.rope().to_string()
    }

    fn sel(anchor: usize, caret: usize) -> Selection {
        sel_of(anchor, caret)
    }

    fn grapheme_bounds(t: &str) -> Vec<usize> {
        let mut v: Vec<usize> = t.grapheme_indices(true).map(|(i, _)| i).collect();
        v.push(t.len());
        v
    }

    /// Runs `f` on a fresh buffer with `text`; checks the change, that undo restores the text and
    /// the selection byte for byte and that redo reapplies; for no change that nothing was
    /// recorded. Returns the new text and the outcome.
    fn go(
        text: &str,
        s: Selection,
        f: impl FnOnce(&mut Buffer, &mut History, Selection) -> Outcome,
    ) -> (String, Outcome) {
        let mut buf = Buffer::from_text(text);
        let mut hist = History::new();
        let out = f(&mut buf, &mut hist, s);
        let after = txt(&buf);
        match &out.change {
            None => {
                assert_eq!(after, text);
                assert_eq!(out.sel, s);
                assert!(!hist.can_undo());
                assert_eq!(buf.revision(), 0);
            }
            Some(c) => {
                check_change(text, &after, c);
                assert_ne!(after, text);
                assert_eq!(hist.steps(), (1, 0), "one undo step");
                let b = grapheme_bounds(&after);
                assert!(b.contains(&out.sel.anchor) && b.contains(&out.sel.caret));
                let u = undo(&mut buf, &mut hist).unwrap();
                assert_eq!(txt(&buf), text);
                // Undo snaps to grapheme boundaries (a selection given inside a CRLF pair comes
                // back at a boundary beside it); any other selection comes back exactly.
                let want = sel(
                    buf.snap_to_grapheme(s.anchor),
                    buf.snap_to_grapheme(s.caret),
                );
                assert_eq!(u.sel, want);
                check_change(&after, text, u.change.as_ref().unwrap());
                assert!(!hist.can_undo() && !hist.is_modified());
                let r = redo(&mut buf, &mut hist).unwrap();
                assert_eq!(txt(&buf), after);
                assert_eq!(r.sel, out.sel);
                assert!(hist.is_modified());
            }
        }
        (after, out)
    }

    fn dup(text: &str, s: Selection, eol: &str) -> (String, Selection) {
        let (t, o) = go(text, s, |b, h, s| duplicate_lines(b, h, s, eol));
        (t, o.sel)
    }

    fn mv(text: &str, s: Selection, down: bool) -> (String, Selection) {
        let (t, o) = go(text, s, |b, h, s| move_lines(b, h, s, down, "\n"));
        (t, o.sel)
    }

    fn del(text: &str, s: Selection) -> (String, Selection) {
        let (t, o) = go(text, s, delete_lines);
        (t, o.sel)
    }

    fn cmt(text: &str, s: Selection, prefix: &str) -> (String, Selection) {
        let (t, o) = go(text, s, |b, h, s| toggle_comment(b, h, s, prefix));
        (t, o.sel)
    }

    fn case(text: &str, s: Selection, upper: bool) -> (String, Selection) {
        let (t, o) = go(text, s, |b, h, s| change_case(b, h, s, upper));
        (t, o.sel)
    }

    fn trim(text: &str, s: Selection) -> (String, Selection) {
        let (t, o) = go(text, s, trim_trailing_whitespace);
        (t, o.sel)
    }

    fn tabs(text: &str, s: Selection, w: usize) -> (String, Selection) {
        let (t, o) = go(text, s, |b, h, s| tabs_to_spaces(b, h, s, w));
        (t, o.sel)
    }

    // ---- duplicate_lines

    #[test]
    fn duplicate_plain_and_at_the_ends() {
        assert_eq!(
            dup("ab\ncd\nef", Selection::at(4), "\n"),
            ("ab\ncd\ncd\nef".into(), Selection::at(7))
        );
        // First line.
        assert_eq!(
            dup("ab\ncd", Selection::at(1), "\n"),
            ("ab\nab\ncd".into(), Selection::at(4))
        );
        // Last line without a break: joined with the given eol.
        assert_eq!(
            dup("ab\ncd", Selection::at(4), "\r\n"),
            ("ab\ncd\r\ncd".into(), Selection::at(8))
        );
        // Last line that is empty (text ends with a break).
        assert_eq!(
            dup("ab\n", Selection::at(3), "\n"),
            ("ab\n\n".into(), Selection::at(4))
        );
        // Empty buffer.
        assert_eq!(
            dup("", Selection::at(0), "\n"),
            ("\n".into(), Selection::at(1))
        );
    }

    #[test]
    fn duplicate_selections_and_line_endings() {
        // Ends at column 0 of the next line: that line is not copied. CRLF.
        assert_eq!(
            dup("a\r\nb\r\nc", sel(0, 3), "\n"),
            ("a\r\na\r\nb\r\nc".into(), sel(3, 6))
        );
        // Backwards, over two lines.
        assert_eq!(
            dup("ab\ncd\nef", sel(5, 1), "\n"),
            ("ab\ncd\nab\ncd\nef".into(), sel(11, 7))
        );
        // Mixed breaks keep their own.
        assert_eq!(
            dup("a\rb\r\nc\nd", Selection::at(5), "\n"),
            ("a\rb\r\nc\nc\nd".into(), Selection::at(7))
        );
        // Turkish and CJK, last line without a break.
        assert_eq!(
            dup("İstanbul\n漢字かな", Selection::at(13), "\n"),
            ("İstanbul\n漢字かな\n漢字かな".into(), Selection::at(26))
        );
    }

    // ---- move_lines

    #[test]
    fn move_up_and_down() {
        assert_eq!(
            mv("a\nb\nc", Selection::at(2), false),
            ("b\na\nc".into(), Selection::at(0))
        );
        // The last line has no break: it stays without one.
        assert_eq!(
            mv("a\nb\nc", Selection::at(2), true),
            ("a\nc\nb".into(), Selection::at(4))
        );
        assert_eq!(
            mv("a\nb\nc", Selection::at(4), false),
            ("a\nc\nb".into(), Selection::at(2))
        );
        // Edges.
        assert_eq!(mv("a\nb", Selection::at(0), false).1, Selection::at(0));
        assert_eq!(mv("a\nb", Selection::at(2), true).1, Selection::at(2));
        assert_eq!(mv("", Selection::at(0), true).0, "");
        assert_eq!(mv("", Selection::at(0), false).0, "");
        // Swapping equal lines changes no byte.
        let (_, o) = go("a\na\nb", Selection::at(0), |b, h, s| {
            move_lines(b, h, s, true, "\n")
        });
        assert!(o.change.is_none());
    }

    #[test]
    fn move_with_mixed_line_endings() {
        assert_eq!(
            mv("a\r\nb\nc\rd", Selection::at(3), false),
            ("b\na\r\nc\rd".into(), Selection::at(0))
        );
        assert_eq!(
            mv("a\r\nb\nc\rd", Selection::at(3), true),
            ("a\r\nc\rb\nd".into(), Selection::at(5))
        );
        // The last line has no break: the one that was last gets the break of the new last.
        assert_eq!(
            mv("a\r\nb\nc", Selection::at(4), true),
            ("a\r\nc\nb".into(), Selection::at(6))
        );
        assert_eq!(
            mv("a\r\nb\nc", Selection::at(5), false),
            ("a\r\nc\nb".into(), Selection::at(3))
        );
        // A trailing empty last line takes part.
        assert_eq!(mv("a\n", Selection::at(0), true).0, "\na");
    }

    #[test]
    fn move_selections_in_both_directions() {
        // Ends at column 0 of the next line: that line stays put and the end moves with the block.
        assert_eq!(
            mv("a\nb\nc\nd", sel(0, 4), true),
            ("c\na\nb\nd".into(), sel(2, 6))
        );
        assert_eq!(
            mv("a\nb\nc\nd", sel(4, 0), true),
            ("c\na\nb\nd".into(), sel(6, 2))
        );
        // A block reaching the last line, moving up.
        assert_eq!(
            mv("a\nb\nc", sel(2, 5), false),
            ("b\nc\na".into(), sel(0, 3))
        );
        assert_eq!(
            mv("a\nb\nc", sel(5, 2), false),
            ("b\nc\na".into(), sel(3, 0))
        );
        // The block moving up over a CRLF line.
        assert_eq!(
            mv("x\r\ny\nz\n", sel(3, 6), false),
            ("y\nz\nx\r\n".into(), sel(0, 3))
        );
    }

    #[test]
    fn move_turkish_and_cjk() {
        assert_eq!(
            mv("çay\n漢字\nşeker", Selection::at(8), true),
            ("çay\nşeker\n漢字".into(), Selection::at(15))
        );
        assert_eq!(
            mv("çay\n漢字\nşeker", Selection::at(8), false),
            ("漢字\nçay\nşeker".into(), Selection::at(3))
        );
    }

    // ---- delete_lines

    #[test]
    fn delete_plain_and_at_the_ends() {
        assert_eq!(
            del("a\nb\nc", Selection::at(2)),
            ("a\nc".into(), Selection::at(2))
        );
        assert_eq!(
            del("a\nb\nc", Selection::at(0)),
            ("b\nc".into(), Selection::at(0))
        );
        // Last line without a break: the break before it goes too.
        assert_eq!(
            del("a\nb\nc", Selection::at(4)),
            ("a\nb".into(), Selection::at(2))
        );
        assert_eq!(
            del("a\r\nb", Selection::at(4)),
            ("a".into(), Selection::at(1))
        );
        // The empty last line.
        assert_eq!(del("a\n", Selection::at(2)), ("a".into(), Selection::at(0)));
        // The only line.
        assert_eq!(del("abc", Selection::at(2)), ("".into(), Selection::at(0)));
        // Empty buffer: nothing.
        assert_eq!(del("", Selection::at(0)), ("".into(), Selection::at(0)));
    }

    #[test]
    fn delete_columns_and_selections() {
        // Column clamped to the line that takes the place.
        assert_eq!(
            del("abc\nd\nefgh", Selection::at(2)),
            ("d\nefgh".into(), Selection::at(1))
        );
        // Both directions, ending at column 0: that line stays.
        assert_eq!(
            del("a\nb\nc\nd", sel(0, 4)),
            ("c\nd".into(), Selection::at(0))
        );
        assert_eq!(
            del("a\nb\nc\nd", sel(4, 0)),
            ("c\nd".into(), Selection::at(0))
        );
        assert_eq!(del("a\nb\nc\nd", sel(1, 5)), ("d".into(), Selection::at(1)));
        // CRLF and mixed.
        assert_eq!(
            del("a\r\nb\rc\nd", Selection::at(3)),
            ("a\r\nc\nd".into(), Selection::at(3))
        );
    }

    #[test]
    fn delete_turkish_and_cjk() {
        assert_eq!(
            del("çay\n漢字\nşeker", Selection::at(8)),
            ("çay\nşeker".into(), Selection::at(8))
        );
        // A column inside a character goes to its start.
        assert_eq!(
            del("ab\nşş", Selection::at(1)),
            ("şş".into(), Selection::at(0))
        );
        assert_eq!(
            del("abc\n漢字", Selection::at(4 + 3)),
            ("abc".into(), Selection::at(3))
        );
    }

    // ---- toggle_comment

    #[test]
    fn comment_and_uncomment() {
        let (t, s) = cmt("foo\nbar", sel(0, 7), "//");
        assert_eq!((t.as_str(), s), ("// foo\n// bar", sel(0, 13)));
        let (t, s) = cmt(&t, s, "//");
        assert_eq!((t.as_str(), s), ("foo\nbar", sel(0, 7)));
        // Without the space, with two spaces.
        assert_eq!(cmt("//a\n//b", sel(0, 7), "//").0, "a\nb");
        assert_eq!(cmt("//  a", Selection::at(0), "//").0, " a");
        // Some lines commented, some not: all get it.
        assert_eq!(cmt("// a\nb", sel(0, 6), "//").0, "// // a\n// b");
    }

    #[test]
    fn comment_uses_the_smallest_indent_and_skips_blank_lines() {
        assert_eq!(
            cmt("    a\n  b\n\n\tc", sel(0, 13), "#").0,
            " #    a\n #  b\n\n\t# c"
        );
        // Blank lines do not stop uncommenting.
        assert_eq!(cmt("  # a\n\n  # b", sel(0, 12), "#").0, "  a\n\n  b");
    }

    #[test]
    fn comment_all_blank_and_empty() {
        assert_eq!(
            cmt("  \n\n", Selection::at(1), "#"),
            ("  # \n\n".into(), Selection::at(1))
        );
        assert_eq!(
            cmt("  \n\n", Selection::at(2), "#"),
            ("  # \n\n".into(), Selection::at(4))
        );
        assert_eq!(
            cmt("", Selection::at(0), "#"),
            ("# ".into(), Selection::at(2))
        );
        // Several blank lines: the caret's line.
        assert_eq!(cmt("\n\n\n", sel(0, 2), "//").0, "\n// \n\n");
        assert_eq!(cmt("a", Selection::at(0), "").0, "a");
    }

    #[test]
    fn comment_selection_ends() {
        // Backwards, ending at column 0 of the third line.
        assert_eq!(
            cmt("a\nb\nc", sel(4, 0), "#"),
            ("# a\n# b\nc".into(), sel(8, 0))
        );
        // The caret in the middle of a line, removal and insertion.
        assert_eq!(
            cmt("ab", Selection::at(1), "#"),
            ("# ab".into(), Selection::at(3))
        );
        assert_eq!(
            cmt("# ab", Selection::at(3), "#"),
            ("ab".into(), Selection::at(1))
        );
        // An end inside the removed prefix goes to its start.
        assert_eq!(
            cmt("// ab", Selection::at(2), "//"),
            ("ab".into(), Selection::at(0))
        );
        assert_eq!(cmt("a\r\nb", sel(0, 4), "#").0, "# a\r\n# b");
    }

    #[test]
    fn comment_turkish_and_cjk() {
        assert_eq!(cmt("çay\n漢字", sel(0, 11), "//").0, "// çay\n// 漢字");
        assert_eq!(cmt("※ 漢字\n※漢", sel(0, 17), "※").0, "漢字\n漢");
        assert_eq!(cmt("ışık", Selection::at(2), "※").0, "※ ışık");
    }

    // ---- change_case

    #[test]
    fn case_plain_and_directions() {
        assert_eq!(
            case("straße x", sel(0, 7), true),
            ("STRASSE x".into(), sel(0, 7))
        );
        assert_eq!(
            case("straße x", sel(7, 0), true),
            ("STRASSE x".into(), sel(7, 0))
        );
        assert_eq!(case("Hello World", sel(3, 8), false).0, "Hello world");
        assert_eq!(case("Hello", sel(0, 5), true).0, "HELLO");
    }

    #[test]
    fn case_nothing_cases() {
        for (t, s) in [
            ("", Selection::at(0)),
            ("abc", Selection::at(1)),
            ("123 漢字", sel(0, 8)),
            ("ABC", sel(0, 3)),
        ] {
            let (_, o) = go(t, s, |b, h, s| change_case(b, h, s, true));
            assert!(o.change.is_none(), "{t:?}");
        }
        let (_, o) = go("abc", sel(0, 3), |b, h, s| change_case(b, h, s, true));
        assert!(o.change.is_some());
    }

    #[test]
    fn case_turkish_and_cjk() {
        assert_eq!(case("ığ iş", sel(0, 8), true).0, "IĞ IŞ");
        assert_eq!(case("İSTANBUL", sel(0, 9), false).0, "i\u{307}stanbul");
        assert_eq!(case("ΟΔΥΣΣΕΥΣ", sel(0, 16), false).0, "οδυσσευς");
        let (t, s) = case("漢字 ǆ カナ", sel(0, 16), true);
        assert_eq!(t, "漢字 Ǆ カナ");
        assert_eq!(s, sel(0, t.len()));
    }

    #[test]
    fn case_over_a_megabyte_is_one_step() {
        let unit = "straße ΑΣ ǆ ığ\n";
        let text = unit.repeat(150_000);
        assert!(text.len() > 3 << 20);
        for upper in [true, false] {
            let want = if upper {
                text.to_uppercase()
            } else {
                text.to_lowercase()
            };
            let (t, s) = case(&text, sel(0, text.len()), upper);
            assert!(t == want);
            assert_eq!(s, sel(0, want.len()));
        }
    }

    // ---- trim_trailing_whitespace

    #[test]
    fn trim_plain_and_mixed_breaks() {
        assert_eq!(
            trim("a  \nb\t \r\n  \nc \rd  ", Selection::at(0)).0,
            "a\nb\r\n\nc\rd"
        );
        assert_eq!(trim("x \t", Selection::at(0)).0, "x");
        assert_eq!(trim("a \n", Selection::at(0)).0, "a\n");
        // Nothing to trim, empty buffer; an ideographic space is not a space or tab.
        for t in ["", "a\nb", "漢　\n"] {
            let (_, o) = go(t, Selection::at(0), trim_trailing_whitespace);
            assert!(o.change.is_none(), "{t:?}");
        }
        assert_eq!(
            trim("çay  \nşeker\t\n漢字 ", Selection::at(0)).0,
            "çay\nşeker\n漢字"
        );
    }

    #[test]
    fn trim_selection_ends() {
        assert_eq!(trim("ab  \ncd", sel(3, 6)), ("ab\ncd".into(), sel(2, 4)));
        assert_eq!(trim("ab  \ncd", sel(6, 3)), ("ab\ncd".into(), sel(4, 2)));
        assert_eq!(trim("ab  \ncd", sel(4, 4)), ("ab\ncd".into(), sel(2, 2)));
        assert_eq!(trim("a \n b \n", sel(0, 7)), ("a\n b\n".into(), sel(0, 5)));
    }

    // ---- tabs_to_spaces

    #[test]
    fn tabs_plain_and_widths() {
        assert_eq!(tabs("\ta\tb", Selection::at(0), 4).0, "    a   b");
        assert_eq!(tabs("\ta\tb", sel(1, 3), 4).1, sel(4, 8));
        assert_eq!(tabs("\ta\tb", Selection::at(0), 8).0, "        a       b");
        assert_eq!(tabs("a\tb", Selection::at(0), 1).0, "a b");
        assert_eq!(tabs("a\tb", Selection::at(0), 0).0, "a b");
        // Each line counts from its own start, for any break.
        assert_eq!(
            tabs("ab\t\r\n\tc\n\t\rx\ty", Selection::at(0), 4).0,
            "ab  \r\n    c\n    \rx   y"
        );
        for t in ["", "a b\nc"] {
            let (_, o) = go(t, Selection::at(0), |b, h, s| tabs_to_spaces(b, h, s, 4));
            assert!(o.change.is_none());
        }
    }

    #[test]
    fn tabs_count_characters_as_the_display_does() {
        // A wide character and a combining mark count one column each.
        assert_eq!(tabs("漢\tx", Selection::at(0), 4).0, "漢   x");
        assert_eq!(tabs("e\u{301}\tx", Selection::at(0), 4).0, "e\u{301}  x");
        assert_eq!(tabs("ş\tx\n\tİ", Selection::at(0), 4).0, "ş   x\n    İ");
        assert_eq!(tabs("\t漢字", sel(1, 7), 4).1, sel(4, 10));
    }

    #[test]
    fn tabs_in_a_line_longer_than_a_batch() {
        let n = TAB_BATCH * 2 + 123;
        let text = format!("a{}x{}\n\tb", "\t".repeat(n), "\t".repeat(n));
        let want = model::tabs(&text, 0, 0, 4).unwrap().0;
        let (t, s) = tabs(&text, sel(5, text.len() - 1), 4);
        assert!(t == want);
        assert_eq!(s, sel(16, want.len() - 1));
    }

    // ---- the reference model: the commands on a plain String

    mod model {
        use super::*;

        /// The new text and selection, `None` when the command does nothing.
        pub type Res = Option<(String, usize, usize)>;

        pub fn bounds(t: &str) -> Vec<usize> {
            grapheme_bounds(t)
        }

        pub fn snap(t: &str, p: usize) -> usize {
            *bounds(t).iter().rev().find(|&&b| b <= p).unwrap()
        }

        pub fn prev_b(t: &str, p: usize) -> usize {
            bounds(t)
                .iter()
                .rev()
                .find(|&&b| b < p)
                .copied()
                .unwrap_or(0)
        }

        /// (content, break) of each line.
        pub fn pairs(t: &str) -> Vec<(String, String)> {
            lines_of(t)
                .into_iter()
                .map(|l| {
                    let n = if l.ends_with("\r\n") {
                        2
                    } else if l.ends_with(['\r', '\n']) {
                        1
                    } else {
                        0
                    };
                    (l[..l.len() - n].to_string(), l[l.len() - n..].to_string())
                })
                .collect()
        }

        pub fn join(p: &[(String, String)]) -> String {
            p.iter().map(|(c, b)| format!("{c}{b}")).collect()
        }

        /// Where each line starts, and the text's length last.
        pub fn starts(p: &[(String, String)]) -> Vec<usize> {
            let mut at = 0;
            let mut v: Vec<usize> = p
                .iter()
                .map(|(c, b)| {
                    let s = at;
                    at += c.len() + b.len();
                    s
                })
                .collect();
            v.push(at);
            v
        }

        /// The line holding `p`, and the column in it.
        pub fn locate(st: &[usize], p: usize) -> (usize, usize) {
            let n = st.len() - 1;
            let li = (0..n).rev().find(|&i| st[i] <= p).unwrap();
            (li, p - st[li])
        }

        /// The first and last touched line, and whether the end is a trailing column-0 end.
        pub fn touched(st: &[usize], a: usize, c: usize) -> (usize, usize, bool) {
            let (lo, hi) = (a.min(c), a.max(c));
            let first = locate(st, lo).0;
            let mut last = locate(st, hi).0;
            let mut trailing = false;
            if last > first && hi == st[last] {
                last -= 1;
                trailing = true;
            }
            (first, last, trailing)
        }

        fn done(p: &[(String, String)], a: usize, c: usize) -> (String, usize, usize) {
            let t = join(p);
            let (a, c) = (snap(&t, a), snap(&t, c));
            (t, a, c)
        }

        pub fn duplicate(t: &str, a: usize, c: usize, eol: &str) -> Res {
            let p = pairs(t);
            let st = starts(&p);
            let (f, l, _) = touched(&st, a, c);
            let mut d = st[l + 1] - st[f];
            let mut np = p[..=l].to_vec();
            if p[l].1.is_empty() {
                np[l].1 = eol.to_string();
                d += eol.len();
            }
            np.extend(p[f..=l].iter().cloned());
            np.extend(p[l + 1..].iter().cloned());
            Some(done(&np, a + d, c + d))
        }

        pub fn move_lines(t: &str, a: usize, c: usize, down: bool) -> Res {
            let p = pairs(t);
            let n = p.len();
            let st = starts(&p);
            let (f, l, trailing) = touched(&st, a, c);
            if (down && l + 1 >= n) || (!down && f == 0) {
                return None;
            }
            // Items: (old index, content, break).
            let mut items: Vec<(usize, String, String)> = p
                .iter()
                .enumerate()
                .map(|(i, (x, b))| (i, x.clone(), b.clone()))
                .collect();
            let nf = if down {
                let x = items.remove(l + 1);
                items.insert(f, x);
                f + 1
            } else {
                let x = items.remove(f - 1);
                items.insert(l, x);
                f - 1
            };
            // The line that had no break (the last) gets the break of the new last line.
            if p[n - 1].1.is_empty() {
                let zi = items.iter().position(|x| x.0 == n - 1).unwrap();
                if zi != n - 1 {
                    let br = items[n - 1].2.clone();
                    items[zi].2 = br;
                    items[n - 1].2 = String::new();
                }
            }
            let np: Vec<(String, String)> =
                items.iter().map(|x| (x.1.clone(), x.2.clone())).collect();
            let text = join(&np);
            if text == t {
                return None;
            }
            let nst = starts(&np);
            let map = |q: usize| {
                if trailing && q == a.max(c) {
                    return nst[nf + (l - f + 1)];
                }
                let (li, col) = locate(&st, q);
                let ni = items.iter().position(|x| x.0 == li).unwrap();
                nst[ni] + col.min(p[li].0.len())
            };
            let (na, nc) = (map(a), map(c));
            Some((text.clone(), snap(&text, na), snap(&text, nc)))
        }

        pub fn delete_lines(t: &str, a: usize, c: usize) -> Res {
            let mut p = pairs(t);
            let st = starts(&p);
            let (f, l, _) = touched(&st, a, c);
            let (_, col) = locate(&st, c);
            let last_no_break = l == p.len() - 1 && p[l].1.is_empty();
            p.drain(f..=l);
            if last_no_break && f > 0 {
                p[f - 1].1 = String::new();
            }
            let text = join(&p);
            if text == t {
                return None;
            }
            let pos = if p.is_empty() {
                0
            } else {
                let nst = starts(&p);
                let ti = f.min(p.len() - 1);
                nst[ti] + col.min(p[ti].0.len())
            };
            let pos = snap(&text, pos);
            Some((text, pos, pos))
        }

        pub fn toggle_comment(t: &str, a: usize, c: usize, prefix: &str) -> Res {
            if prefix.is_empty() {
                return None;
            }
            let p = pairs(t);
            let st = starts(&p);
            let (f, l, _) = touched(&st, a, c);
            let indent = |s: &str| s.len() - s.trim_start_matches([' ', '\t']).len();
            let solid: Vec<usize> = (f..=l)
                .filter(|&i| indent(&p[i].0) < p[i].0.len())
                .collect();
            // Per line: (column, bytes removed); an insertion removes 0.
            let mut edit: Vec<Option<(usize, usize)>> = vec![None; p.len()];
            let mut stay = true;
            if solid.is_empty() {
                let (cl, _) = locate(&st, c);
                let i = cl.clamp(f, l);
                edit[i] = Some((indent(&p[i].0), 0));
                stay = false;
            } else if solid
                .iter()
                .all(|&i| p[i].0[indent(&p[i].0)..].starts_with(prefix))
            {
                for &i in &solid {
                    let at = indent(&p[i].0);
                    let space = p[i].0[at + prefix.len()..].starts_with(' ');
                    edit[i] = Some((at, prefix.len() + usize::from(space)));
                }
            } else {
                let col = solid.iter().map(|&i| indent(&p[i].0)).min().unwrap();
                for &i in &solid {
                    edit[i] = Some((col, 0));
                }
            }
            let ins = format!("{prefix} ");
            let mut np = p.clone();
            for (i, e) in edit.iter().enumerate() {
                if let Some((col, rem)) = *e {
                    np[i]
                        .0
                        .replace_range(col..col + rem, if rem == 0 { &ins } else { "" });
                }
            }
            let nst = starts(&np);
            let map = |q: usize| {
                let (li, col) = locate(&st, q);
                let mut nc = col;
                if let Some((e, rem)) = edit[li] {
                    if rem == 0 {
                        if col > e || (col == e && !stay) {
                            nc += ins.len();
                        }
                    } else if col >= e + rem {
                        nc -= rem;
                    } else if col > e {
                        nc = e;
                    }
                }
                nst[li] + nc
            };
            let (na, nc) = (map(a), map(c));
            Some(done(&np, na, nc))
        }

        pub fn change_case(t: &str, a: usize, c: usize, upper: bool) -> Res {
            let (lo, hi) = (a.min(c), a.max(c));
            if lo == hi {
                return None;
            }
            let part = &t[lo..hi];
            let conv = if upper {
                part.to_uppercase()
            } else {
                part.to_lowercase()
            };
            if conv == part {
                return None;
            }
            let mut text = t.to_string();
            text.replace_range(lo..hi, &conv);
            let end = lo + conv.len();
            let (na, nc) = if a <= c { (lo, end) } else { (end, lo) };
            Some((text.clone(), snap(&text, na), snap(&text, nc)))
        }

        pub fn trim(t: &str, a: usize, c: usize) -> Res {
            let p = pairs(t);
            let st = starts(&p);
            let np: Vec<_> = p
                .iter()
                .map(|(x, b)| (x.trim_end_matches([' ', '\t']).to_string(), b.clone()))
                .collect();
            let text = join(&np);
            if text == t {
                return None;
            }
            let nst = starts(&np);
            let map = |q: usize| {
                let (li, col) = locate(&st, q);
                nst[li] + col.min(np[li].0.len())
            };
            let (na, nc) = (map(a), map(c));
            Some((text.clone(), snap(&text, na), snap(&text, nc)))
        }

        pub fn tabs(t: &str, a: usize, c: usize, w: usize) -> Res {
            let w = w.max(1);
            let p = pairs(t);
            let st = starts(&p);
            let expand = |s: &str| {
                let mut out = String::new();
                let mut col = 0;
                for ch in s.chars() {
                    if ch == '\t' {
                        let n = w - col % w;
                        out.push_str(&" ".repeat(n));
                        col += n;
                    } else {
                        out.push(ch);
                        col += 1;
                    }
                }
                out
            };
            let np: Vec<_> = p.iter().map(|(x, b)| (expand(x), b.clone())).collect();
            let text = join(&np);
            if text == t {
                return None;
            }
            let nst = starts(&np);
            let map = |q: usize| {
                let (li, col) = locate(&st, q);
                nst[li] + expand(&p[li].0[..col.min(p[li].0.len())]).len()
            };
            let (na, nc) = (map(a), map(c));
            Some((text.clone(), snap(&text, na), snap(&text, nc)))
        }
    }

    // ---- the scripted run

    const CHARS: [&str; 20] = [
        "a",
        "b",
        " ",
        " ",
        "é",
        "İ",
        "ı",
        "漢",
        "\t",
        "\t",
        "\u{301}",
        "👨‍👩‍👧",
        "\r",
        "\n",
        "\n",
        "e",
        "#",
        "/",
        "Σ",
        "ş",
    ];
    const STRINGS: [&str; 9] = [
        "foo bar",
        "\r\n",
        "hello\nworld\r",
        "  x  ",
        "// ğüş iöç\n",
        "a\r\nb  \r\nc",
        "日本語\tテキスト",
        "\t\ttabbed \n\t",
        "# comment\n",
    ];
    const EOLS: [&str; 3] = ["\n", "\r\n", "\r"];
    const PREFIXES: [&str; 4] = ["//", "#", "※", "--"];
    const START: &str = "Merhaba dünya!\r\nİstanbul ışık ğüşöç  \n\tindented line\rçalışma\r\n  two spaces  \n\
        漢字かな交じり文\n// 👨‍👩‍👧 family e\u{301}\n\n   \nlast line, no break";

    fn random_sel(rng: &mut Rng, text: &str, near: usize) -> Selection {
        let b = model::bounds(text);
        let i = if rng.chance(50) {
            let c = b.iter().position(|&x| x >= near).unwrap_or(b.len() - 1);
            (c + rng.below(7)).saturating_sub(3).min(b.len() - 1)
        } else {
            rng.below(b.len())
        };
        let j = match rng.below(40) {
            0..=11 => i,
            12..=33 => (i + rng.below(60)).saturating_sub(30).min(b.len() - 1),
            _ => rng.below(b.len()),
        };
        sel(b[j], b[i])
    }

    fn scripted(seed: u64, copies: usize, ops: usize) {
        let mut rng = Rng::new(seed);
        let original = START.repeat(copies);
        let mut buf = Buffer::from_text(&original);
        let mut hist = History::new();
        let mut counts = EolCounts::of(&original);
        let mut s = Selection::at(0);
        let mut commands = 0usize;
        for op in 0..ops {
            let before = txt(&buf);
            if rng.chance(20) {
                s = random_sel(&mut rng, &before, s.caret);
            }
            let (a, c) = (s.anchor, s.caret);
            let steps = hist.steps();
            let pick = if before.len() > 12_000 {
                60 + rng.below(8)
            } else if before.len() < 300 {
                rng.below(20)
            } else {
                rng.below(100)
            };
            let small = before.len() < 300;
            let (name, out, want, other): (String, Outcome, model::Res, bool) = match pick {
                0..=19 => {
                    let t: String = if rng.chance(30) {
                        (*rng.pick(&STRINGS)).to_string()
                    } else if small {
                        (0..100 + rng.below(200))
                            .map(|_| *rng.pick(&CHARS))
                            .collect()
                    } else {
                        (*rng.pick(&CHARS)).to_string()
                    };
                    let (lo, hi) = (a.min(c), a.max(c));
                    let mut m = before.clone();
                    m.replace_range(lo..hi, &t);
                    let w = (!(lo == hi && t.is_empty())).then(|| {
                        let p = model::snap(&m, lo + t.len());
                        (m, p, p)
                    });
                    (
                        format!("insert {t:?}"),
                        insert(&mut buf, &mut hist, s, &t),
                        w,
                        false,
                    )
                }
                20..=29 => {
                    let w = if a != c {
                        let mut m = before.clone();
                        m.replace_range(a.min(c)..a.max(c), "");
                        let p = model::snap(&m, a.min(c));
                        Some((m, p, p))
                    } else {
                        let pb = model::prev_b(&before, c);
                        (pb != c).then(|| {
                            let mut m = before.clone();
                            m.replace_range(pb..c, "");
                            let p = model::snap(&m, pb);
                            (m, p, p)
                        })
                    };
                    (
                        "backspace".into(),
                        backspace(&mut buf, &mut hist, s),
                        w,
                        false,
                    )
                }
                30..=36 => {
                    let eol = *rng.pick(&EOLS);
                    (
                        format!("duplicate {eol:?}"),
                        duplicate_lines(&mut buf, &mut hist, s, eol),
                        model::duplicate(&before, a, c, eol),
                        true,
                    )
                }
                37..=46 => {
                    let down = rng.chance(50);
                    (
                        format!("move down={down}"),
                        move_lines(&mut buf, &mut hist, s, down, "\n"),
                        model::move_lines(&before, a, c, down),
                        true,
                    )
                }
                47..=54 | 60..=67 => (
                    "delete lines".into(),
                    delete_lines(&mut buf, &mut hist, s),
                    model::delete_lines(&before, a, c),
                    true,
                ),
                55..=59 | 68..=74 => {
                    let p = *rng.pick(&PREFIXES);
                    (
                        format!("comment {p:?}"),
                        toggle_comment(&mut buf, &mut hist, s, p),
                        model::toggle_comment(&before, a, c, p),
                        true,
                    )
                }
                75..=84 => {
                    let upper = rng.chance(50);
                    (
                        format!("case upper={upper}"),
                        change_case(&mut buf, &mut hist, s, upper),
                        model::change_case(&before, a, c, upper),
                        true,
                    )
                }
                85..=91 => (
                    "trim".into(),
                    trim_trailing_whitespace(&mut buf, &mut hist, s),
                    model::trim(&before, a, c),
                    true,
                ),
                _ => {
                    let w = 1 + rng.below(8);
                    (
                        format!("tabs {w}"),
                        tabs_to_spaces(&mut buf, &mut hist, s, w),
                        model::tabs(&before, a, c, w),
                        true,
                    )
                }
            };
            let after = txt(&buf);
            let ctx = format!("op {op} {name} sel {s:?} on {before:?}");
            match want {
                None => {
                    assert_eq!(after, before, "{ctx}");
                    assert!(out.change.is_none(), "{ctx}");
                    assert_eq!(out.sel, s, "{ctx}");
                    assert_eq!(hist.steps(), steps, "{ctx}");
                }
                Some((text, wa, wc)) => {
                    assert_eq!(after, text, "{ctx}");
                    assert_eq!((out.sel.anchor, out.sel.caret), (wa, wc), "{ctx}");
                    assert_eq!(out.sel.goal_col, None, "{ctx}");
                    let ch = out.change.as_ref().expect(&ctx);
                    check_change(&before, &after, ch);
                    check_eols(&mut counts, &after, ch);
                    assert_eq!(buf.len_lines(), lines_of(&after).len(), "{ctx}");
                    if other {
                        commands += 1;
                        assert_eq!(hist.steps(), (steps.0 + 1, 0), "{ctx}");
                        if rng.chance(15) {
                            // Undo restores text and selection exactly; redo reapplies.
                            let u = undo(&mut buf, &mut hist).unwrap();
                            assert_eq!(txt(&buf), before, "{ctx}");
                            assert_eq!(u.sel, s, "{ctx}");
                            check_change(&after, &before, u.change.as_ref().unwrap());
                            check_eols(&mut counts, &before, u.change.as_ref().unwrap());
                            let r = redo(&mut buf, &mut hist).unwrap();
                            assert_eq!(txt(&buf), after, "{ctx}");
                            assert_eq!(r.sel, out.sel, "{ctx}");
                            check_change(&before, &after, r.change.as_ref().unwrap());
                            check_eols(&mut counts, &after, r.change.as_ref().unwrap());
                        }
                    }
                    s = out.sel;
                }
            }
            assert_eq!(counts, EolCounts::of(&after), "{ctx}");
        }
        assert!(commands > ops / 4, "seed {seed}: only {commands} commands");
        let last = txt(&buf);
        while undo(&mut buf, &mut hist).is_some() {}
        assert_eq!(txt(&buf), original, "seed {seed}");
        assert!(!hist.is_modified());
        while redo(&mut buf, &mut hist).is_some() {}
        assert_eq!(txt(&buf), last, "seed {seed}");
    }

    #[test]
    fn scripted_run_matches_the_reference_model() {
        scripted(1, 4, 6_000);
        scripted(2, 4, 3_000);
        scripted(3, 1, 3_000);
        scripted(4, 8, 3_000);
    }

    #[test]
    fn marks_follow_the_lines_of_an_edit() {
        let mut buf = Buffer::from_text("a\nb\nc\nd\ne\n");
        let mut hist = History::new();
        let mut marks = vec![0, 2, 3, 4];
        // Two lines put in at the start of line 1: marks from line 2 on move down.
        let out = crate::edit::insert(&mut buf, &mut hist, Selection::at(2), "x\ny\n");
        shift_marks(&mut marks, out.change.as_ref().unwrap());
        assert_eq!(marks, [0, 4, 5, 6]);
        // Lines 3 to 5 joined into line 3 ("b" to "d"): their marks meet on it.
        let (a, b) = (buf.line_start(3), buf.line_start(5));
        let out = crate::edit::insert(&mut buf, &mut hist, sel(a, b), "");
        shift_marks(&mut marks, out.change.as_ref().unwrap());
        assert_eq!(marks, [0, 3, 4]);
        // Undo brings the lines back; marks below move again, the merged one stays.
        let out = crate::edit::undo(&mut buf, &mut hist).unwrap();
        shift_marks(&mut marks, out.change.as_ref().unwrap());
        assert_eq!(marks, [0, 3, 6]);
        // An edit inside a line moves nothing.
        let out = crate::edit::insert(&mut buf, &mut hist, Selection::at(0), "zz");
        shift_marks(&mut marks, out.change.as_ref().unwrap());
        assert_eq!(marks, [0, 3, 6]);
    }

    // ---- convert_line_endings

    fn conv(text: &str, s: Selection, to: &str) -> (String, Selection) {
        let (t, o) = go(text, s, |b, h, s| convert_line_endings(b, h, s, to));
        (t, o.sel)
    }

    /// The reference: the converted text and, for every byte offset of `text`, where it goes (an
    /// offset inside a CRLF goes to the start of its replacement).
    fn conv_model(text: &str, to: &str) -> (String, Vec<usize>) {
        let mut out = String::new();
        let mut map = vec![0; text.len() + 1];
        let mut chars = text.char_indices().peekable();
        while let Some((i, ch)) = chars.next() {
            let w = ch.len_utf8();
            if ch == '\r' && chars.peek().map(|p| p.1) == Some('\n') {
                map[i] = out.len();
                map[i + 1] = out.len();
                chars.next();
                out.push_str(to);
            } else if ch == '\r' || ch == '\n' {
                map[i] = out.len();
                out.push_str(to);
            } else {
                for k in 0..w {
                    map[i + k] = out.len() + k;
                }
                out.push(ch);
            }
        }
        map[text.len()] = out.len();
        (out, map)
    }

    const ALL: [&str; 3] = ["\r\n", "\n", "\r"];

    #[test]
    fn convert_between_the_three_kinds() {
        let from = [
            ("\r\n", "a\r\nb\r\nc"),
            ("\n", "a\nb\nc"),
            ("\r", "a\rb\rc"),
        ];
        for (f, text) in from {
            for to in ALL {
                let (t, s) = conv(text, Selection::at(0), to);
                assert_eq!(t, format!("a{to}b{to}c"), "{f:?} -> {to:?}");
                assert_eq!(s, Selection::at(0));
            }
        }
    }

    #[test]
    fn convert_nothing_when_every_break_already_is_the_target() {
        let cases = [
            ("a\r\nb\r\n", "\r\n"),
            ("a\nb", "\n"),
            ("a\rb\r", "\r"),
            ("a\nb", "x"),
            ("a\nb", ""),
        ];
        for (text, to) in cases {
            let (_, o) = go(text, Selection::at(2), |b, h, s| {
                convert_line_endings(b, h, s, to)
            });
            assert!(o.change.is_none(), "{text:?} {to:?}");
        }
        // No breaks at all, and an empty buffer.
        for to in ALL {
            for t in ["", "abc", "İstanbul 漢字"] {
                let (_, o) = go(t, Selection::at(0), |b, h, s| {
                    convert_line_endings(b, h, s, to)
                });
                assert!(o.change.is_none());
            }
        }
    }

    #[test]
    fn convert_mixed_only_breaks_and_adjacent_sequences() {
        let mixed = "a\r\nb\nc\rd";
        assert_eq!(conv(mixed, Selection::at(0), "\n").0, "a\nb\nc\nd");
        assert_eq!(conv(mixed, Selection::at(0), "\r\n").0, "a\r\nb\r\nc\r\nd");
        assert_eq!(conv(mixed, Selection::at(0), "\r").0, "a\rb\rc\rd");
        // Only breaks, at the very start and end.
        assert_eq!(conv("\r\n\n\r", Selection::at(0), "\n").0, "\n\n\n");
        assert_eq!(conv("\n", Selection::at(0), "\r\n").0, "\r\n");
        assert_eq!(conv("\r\n", Selection::at(2), "\r").0, "\r");
        assert_eq!(conv("\nabc\r\n", Selection::at(0), "\r").0, "\rabc\r");
        // Adjacent sequences: no half pair is left and none is made.
        assert_eq!(conv("\r\r\n\n", Selection::at(0), "\n").0, "\n\n\n");
        assert_eq!(conv("\r\r\n\n", Selection::at(0), "\r").0, "\r\r\r");
        assert_eq!(conv("\r\r\n\n", Selection::at(0), "\r\n").0, "\r\n\r\n\r\n");
        assert_eq!(conv("\n\n", Selection::at(0), "\r").0, "\r\r");
        assert_eq!(conv("\r\n\r\n\n\r", Selection::at(0), "\r").0, "\r\r\r\r");
        // A lone CR and a lone LF, far apart, stay two breaks.
        assert_eq!(conv("a\rb\nc", Selection::at(0), "\r").0, "a\rb\rc");
        assert_eq!(conv("a\rb\nc", Selection::at(0), "\n").0, "a\nb\nc");
    }

    #[test]
    fn convert_leaves_other_unicode_breaks_alone() {
        let odd = "a\u{b}b\u{c}c\u{85}d\u{2028}e\u{2029}f\r\ng";
        assert_eq!(
            conv(odd, Selection::at(0), "\n").0,
            "a\u{b}b\u{c}c\u{85}d\u{2028}e\u{2029}f\ng"
        );
        let (_, o) = go("a\u{2028}b\u{85}c\u{b}", Selection::at(0), |b, h, s| {
            convert_line_endings(b, h, s, "\n")
        });
        assert!(o.change.is_none());
    }

    #[test]
    fn convert_moves_selection_ends() {
        // "a\r\nb\nc": CRLF at 1..3, LF at 4..5. To LF: before, at the start of, inside (goes to
        // the start) and after the CRLF.
        for (e, want) in [(0, 0), (1, 1), (2, 1), (3, 2), (4, 3), (5, 4), (6, 5)] {
            assert_eq!(
                conv("a\r\nb\nc", Selection::at(e), "\n").1,
                Selection::at(want)
            );
        }
        // To CRLF: the LF grows; ends after it move by one, the CRLF at 3..5 stays.
        for (e, want) in [(0, 0), (1, 1), (2, 3), (3, 4), (6, 7), (7, 9)] {
            assert_eq!(
                conv("a\nb\r\nc\n", Selection::at(e), "\r\n").1,
                Selection::at(want),
                "{e}"
            );
        }
        // Both ends, both directions, over multibyte characters.
        let text = "é\r\nş\rğ\n";
        assert_eq!(conv(text, sel(0, 10), "\n").1, sel(0, 9));
        assert_eq!(conv(text, sel(10, 0), "\n").1, sel(9, 0));
        assert_eq!(conv(text, sel(4, 6), "\n").1, sel(3, 5));
        assert_eq!(conv(text, sel(3, 4), "\r").1, sel(2, 3));
    }

    #[test]
    fn convert_selection_end_inside_a_kept_pair_stays_on_a_boundary() {
        let (t, s) = conv("a\r\nb\n", sel(2, 2), "\r\n");
        let b = grapheme_bounds(&t);
        assert!(b.contains(&s.anchor) && b.contains(&s.caret));
    }

    #[test]
    fn convert_reports_a_change_the_crate_can_check() {
        for text in ["a\r\nb\nc\rd", "\r\r\n\n", "x\n", "\n\n\nz\r\n"] {
            for to in ALL {
                let mut buf = Buffer::from_text(text);
                let mut hist = History::new();
                let mut counts = EolCounts::of(text);
                let o = convert_line_endings(&mut buf, &mut hist, Selection::at(0), to);
                let Some(ch) = &o.change else {
                    assert_eq!(txt(&buf), text);
                    continue;
                };
                let after = txt(&buf);
                check_change(text, &after, ch);
                check_eols(&mut counts, &after, ch);
                let u = undo(&mut buf, &mut hist).unwrap();
                check_eols(&mut counts, text, u.change.as_ref().unwrap());
                let r = redo(&mut buf, &mut hist).unwrap();
                check_eols(&mut counts, &after, r.change.as_ref().unwrap());
            }
        }
    }

    #[test]
    fn convert_matches_the_reference_model() {
        const PARTS: [&str; 12] = [
            "\r", "\n", "\r\n", "\r", "\n", "a", "é", "漢", "\u{b}", "\u{2028}", "\u{85}", "ş",
        ];
        for seed in 1..=60u64 {
            let mut rng = Rng::new(seed);
            for round in 0..40 {
                let n = rng.below(40);
                let text: String = (0..n).map(|_| *rng.pick(&PARTS)).collect();
                let to = *rng.pick(&ALL);
                let bounds: Vec<usize> = text
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain([text.len()])
                    .collect();
                let s = sel(*rng.pick(&bounds), *rng.pick(&bounds));
                let ctx = format!("seed {seed} round {round}: {text:?} -> {to:?}, {s:?}");
                let (want, map) = conv_model(&text, to);
                let (got, o) = go(&text, s, |b, h, s| convert_line_endings(b, h, s, to));
                assert_eq!(got, want, "{ctx}");
                assert_eq!(o.change.is_some(), want != text, "{ctx}");
                if want != text {
                    let b = grapheme_bounds(&got);
                    for (end, from) in [(o.sel.anchor, s.anchor), (o.sel.caret, s.caret)] {
                        let ex = map[from];
                        if b.contains(&ex) {
                            assert_eq!(end, ex, "{ctx}");
                        } else {
                            assert!(b.contains(&end), "{ctx}");
                        }
                    }
                }
            }
        }
    }

    // ---- speed

    /// Prints the time of the two whole-document commands on about 100 MB where every line
    /// changes.
    #[test]
    #[ignore]
    fn timing_whole_document_commands() {
        use std::time::Instant;
        let mut text = String::with_capacity(101 << 20);
        let mut i = 0;
        while text.len() < 100 << 20 {
            text.push_str(&format!("\tline {i} ğüşiöç 漢字 \t \r\n"));
            i += 1;
        }
        let mut buf = Buffer::from_text(&text);
        let lines = buf.len_lines();
        drop(text);
        let mut hist = History::new();
        let t = Instant::now();
        let o = trim_trailing_whitespace(&mut buf, &mut hist, Selection::at(0));
        println!("trim: {lines} lines in {:?}", t.elapsed());
        assert!(o.change.is_some());
        let t = Instant::now();
        let o = tabs_to_spaces(&mut buf, &mut hist, Selection::at(0), 4);
        println!("tabs: {lines} lines in {:?}", t.elapsed());
        assert!(o.change.is_some());
        let t = Instant::now();
        undo(&mut buf, &mut hist);
        undo(&mut buf, &mut hist);
        println!("two undos in {:?}", t.elapsed());
    }

    /// Prints the time of converting about 100 MB of short lines LF to CRLF and back, and the
    /// undo of each.
    #[test]
    #[ignore]
    fn timing_convert_line_endings() {
        use std::time::Instant;
        let mut text = String::with_capacity(101 << 20);
        let mut i = 0;
        while text.len() < 100 << 20 {
            text.push_str(&format!("line {i} ğüşiöç 漢字\n"));
            i += 1;
        }
        let original = text.clone();
        let mut buf = Buffer::from_text(&text);
        drop(text);
        let lines = buf.len_lines();
        let mut hist = History::new();
        let t = Instant::now();
        let o = convert_line_endings(&mut buf, &mut hist, Selection::at(0), "\r\n");
        println!("LF to CRLF: {lines} lines in {:?}", t.elapsed());
        assert!(o.change.is_some());
        let t = Instant::now();
        let o = convert_line_endings(&mut buf, &mut hist, Selection::at(0), "\n");
        println!("CRLF to LF: {lines} lines in {:?}", t.elapsed());
        assert!(o.change.is_some());
        assert_eq!(buf.len_bytes(), original.len());
        let t = Instant::now();
        undo(&mut buf, &mut hist);
        println!("undo of CRLF to LF in {:?}", t.elapsed());
        let t = Instant::now();
        undo(&mut buf, &mut hist);
        println!("undo of LF to CRLF in {:?}", t.elapsed());
        assert_eq!(buf.len_bytes(), original.len());
    }
}

#[cfg(test)]
mod block_tests {
    use super::*;
    use crate::edit::{redo, undo};
    use crate::eol::EolCounts;
    use crate::testutil::{Rng, check_change, check_eols};

    fn txt(buf: &Buffer) -> String {
        buf.rope().to_string()
    }

    fn sel(anchor: usize, caret: usize) -> Selection {
        sel_of(anchor, caret)
    }

    /// Runs `f` on a fresh buffer; checks the change, the line-ending counts, one undo step, undo
    /// and redo; for no change that nothing was recorded.
    fn go(
        text: &str,
        s: Selection,
        f: impl FnOnce(&mut Buffer, &mut History, Selection) -> Outcome,
    ) -> (String, Selection) {
        let mut buf = Buffer::from_text(text);
        let mut hist = History::new();
        let out = f(&mut buf, &mut hist, s);
        let after = txt(&buf);
        match &out.change {
            None => {
                assert_eq!(after, text);
                assert_eq!(out.sel, s);
                assert!(!hist.can_undo());
            }
            Some(c) => {
                check_change(text, &after, c);
                let mut counts = EolCounts::of(text);
                check_eols(&mut counts, &after, c);
                assert_ne!(after, text);
                assert_eq!(hist.steps(), (1, 0));
                assert!(
                    buf.is_char_boundary(out.sel.anchor) && buf.is_char_boundary(out.sel.caret)
                );
                undo(&mut buf, &mut hist).unwrap();
                assert_eq!(txt(&buf), text);
                let r = redo(&mut buf, &mut hist).unwrap();
                assert_eq!(txt(&buf), after);
                assert_eq!(r.sel, out.sel);
            }
        }
        (after, out.sel)
    }

    fn sort(text: &str, s: Selection, o: SortOrder) -> (String, Selection) {
        go(text, s, |b, h, s| sort_lines(b, h, s, o))
    }

    #[test]
    fn sort_orders() {
        use SortOrder::*;
        let none = Selection::at(0);
        assert_eq!(
            sort("b\nC\na\nB\n", none, Ascending),
            ("B\nC\na\nb\n".into(), sel(0, 7))
        );
        assert_eq!(
            sort("b\nC\na\nB\n", none, Descending),
            ("b\na\nC\nB\n".into(), sel(0, 7))
        );
        assert_eq!(
            sort("b\nC\na\nB\n", none, IgnoreCase).0,
            // Equal keys ("b" and "B") keep their order.
            "a\nb\nB\nC\n"
        );
        // Byte-wise: "é" (two bytes) sorts after "z".
        assert_eq!(sort("é\nz\n", none, Ascending).0, "z\né\n");
        // Simple lowercase of a capital with a longer full lowercase.
        assert_eq!(sort("j\nİ\ni\n", none, IgnoreCase).0, "İ\ni\nj\n");
        // Stable descending: equal lines keep their order, shown by the break that follows each.
        assert_eq!(sort("a\r\na\n", none, Descending).0, "a\r\na\n");
    }

    #[test]
    fn sort_keeps_breaks_in_place_and_the_last_line_ending() {
        use SortOrder::*;
        // Mixed breaks stay where they are; no final break stays none.
        assert_eq!(
            sort("b\r\na\nc", Selection::at(0), Ascending),
            ("a\r\nb\nc".into(), sel(0, 6))
        );
        assert_eq!(sort("c\nb\na", Selection::at(2), Ascending).0, "a\nb\nc");
        // A final empty line is not sorted: the final break stays last.
        assert_eq!(sort("b\na\n", Selection::at(0), Ascending).0, "a\nb\n");
        assert_eq!(sort("b\na\n\n", Selection::at(0), Ascending).0, "\na\nb\n");
        // Already in order, one line, empty document: nothing.
        assert_eq!(sort("a\nb\n", Selection::at(0), Ascending).0, "a\nb\n");
        assert_eq!(sort("a", Selection::at(0), Ascending).0, "a");
        assert_eq!(sort("", Selection::at(0), Descending).0, "");
        assert_eq!(sort("\n", Selection::at(0), Descending).0, "\n");
    }

    #[test]
    fn sort_with_a_selection_touches_only_its_lines() {
        use SortOrder::*;
        let t = "z\nc\nb\na\ny\n";
        // Lines 1..=3; the selection starts inside line 1 and ends inside line 3.
        let (r, s) = sort(t, sel(3, 7), Ascending);
        assert_eq!(r, "z\na\nb\nc\ny\n");
        assert_eq!(s, sel(2, 7));
        // Ending at column 0 of line 4 leaves line 4 out.
        assert_eq!(sort(t, sel(2, 8), Ascending).0, "z\na\nb\nc\ny\n");
        // Backwards selection gives the same result; the result is forward.
        assert_eq!(
            sort(t, sel(7, 3), Ascending),
            ("z\na\nb\nc\ny\n".into(), sel(2, 7))
        );
        // A caret or a selection in one line is that one line: nothing to sort... but with no
        // selection the whole document is sorted.
        assert_eq!(sort(t, sel(3, 3), Ascending).0, "a\nb\nc\ny\nz\n");
        assert_eq!(sort(t, sel(3, 4), Ascending).0, t);
    }

    #[test]
    fn sort_selection_to_the_end_of_a_document_without_a_final_break() {
        use SortOrder::*;
        assert_eq!(
            sort("x\nb\na", sel(2, 5), Ascending),
            ("x\na\nb".into(), sel(2, 5))
        );
        // The empty last line after the final break is not part of the block.
        assert_eq!(sort("x\nb\na\n", sel(2, 6), Ascending).0, "x\na\nb\n");
    }

    #[test]
    fn unique_keeps_the_first_of_each() {
        let none = Selection::at(0);
        let u = |t: &str, s| go(t, s, unique_lines);
        assert_eq!(u("a\nb\na\nc\nb\n", none), ("a\nb\nc\n".into(), sel(0, 5)));
        // Not adjacent only, and case, spaces count.
        assert_eq!(u("a\nA\na \na\n", none).0, "a\nA\na \n");
        // Empty lines are duplicates of each other.
        assert_eq!(u("a\n\n\nb\n\n", none).0, "a\n\nb\n");
        // The last line, a duplicate without a break: the document ends without one.
        assert_eq!(u("a\nb\na", none).0, "a\nb");
        assert_eq!(u("a\r\nb\r\na\r\nc", none).0, "a\r\nb\r\nc");
        assert_eq!(u("a\r\na\r\n", none).0, "a\r\n");
        // Nothing to remove, one line.
        assert_eq!(u("a\nb\n", none).0, "a\nb\n");
        assert_eq!(u("a", none).0, "a");
        // Only the selected lines.
        assert_eq!(u("a\na\nb\nb\nb\n", sel(2, 7)).0, "a\na\nb\nb\n");
        assert_eq!(u("a\nb\nb\nb\na\n", sel(2, 6)).0, "a\nb\nb\na\n");
    }

    #[test]
    fn reverse_reverses() {
        let r = |t: &str, s| go(t, s, reverse_lines);
        assert_eq!(
            r("a\nb\nc\n", Selection::at(1)),
            ("c\nb\na\n".into(), sel(0, 5))
        );
        assert_eq!(r("a\r\nb\nc", Selection::at(0)).0, "c\r\nb\na");
        assert_eq!(r("a\nb\nc\nd\n", sel(2, 5)).0, "a\nc\nb\nd\n");
        assert_eq!(r("a\nb\na\n", Selection::at(0)).0, "a\nb\na\n");
        assert_eq!(r("a\n", Selection::at(0)).0, "a\n");
        assert_eq!(r("", Selection::at(0)).0, "");
    }

    #[test]
    fn join_with_the_next_line() {
        let j = |t: &str, s| go(t, s, join_lines);
        // No selection: the caret's line with the next; the caret at the join.
        assert_eq!(
            j("a\nb\nc", Selection::at(0)),
            ("a b\nc".into(), Selection::at(1))
        );
        assert_eq!(j("a\n   \tb\nc", Selection::at(0)).0, "a b\nc");
        // Next line empty or blank: no space.
        assert_eq!(j("a\n\nc", Selection::at(0)).0, "a\nc");
        assert_eq!(j("a\n  \nc", Selection::at(0)).0, "a\nc");
        // Trailing whitespace of the first line stays; the next line's trailing too.
        assert_eq!(j("a \n b \nc", Selection::at(0)).0, "a  b \nc");
        // Last line: nothing. The empty final line is joined (the break goes).
        assert_eq!(j("a\nb", Selection::at(3)).0, "a\nb");
        assert_eq!(j("a\n", Selection::at(0)).0, "a");
        // CRLF: the joined line ends with the break of the last line joined.
        assert_eq!(j("a\r\nb\r\nc", Selection::at(0)).0, "a b\r\nc");
        assert_eq!(j("a\nb\r\nc", sel(0, 0)).0, "a b\r\nc");
        // A selection inside one line is the same as a caret.
        assert_eq!(j("ab\ncd\n", sel(0, 2)).0, "ab cd\n");
    }

    #[test]
    fn join_a_selection() {
        let j = |t: &str, s| go(t, s, join_lines);
        assert_eq!(
            j("a\n b\n  c\nd\n", sel(0, 8)),
            ("a b c\nd\n".into(), sel(0, 5))
        );
        // Ending at column 0 of the line after: that line is not joined.
        assert_eq!(j("a\nb\nc\n", sel(0, 4)).0, "a b\nc\n");
        // Backwards, with blank lines between.
        assert_eq!(j("a\n\n\nb\n", sel(6, 0)).0, "a b\n");
        // The last line without a break.
        assert_eq!(j("x\na\nb", sel(2, 5)).0, "x\na b");
        assert_eq!(j("é\n\té\n", sel(0, 6)).0, "é é\n");
    }

    #[test]
    fn line_commands_refuse_nothing_that_is_valid_and_survive_bad_selections() {
        // Selections past the end or inside a CRLF pair or a char do not panic.
        for t in ["", "a", "a\n", "é\r\nb\rc\n", "\n\n", "\r\n"] {
            let len = t.len();
            for a in 0..=len + 2 {
                for c in 0..=len + 2 {
                    let s = sel(a, c);
                    for f in 0..6 {
                        let mut buf = Buffer::from_text(t);
                        let mut hist = History::new();
                        match f {
                            0 => sort_lines(&mut buf, &mut hist, s, SortOrder::Ascending),
                            1 => sort_lines(&mut buf, &mut hist, s, SortOrder::IgnoreCase),
                            2 => unique_lines(&mut buf, &mut hist, s),
                            3 => reverse_lines(&mut buf, &mut hist, s),
                            4 => join_lines(&mut buf, &mut hist, s),
                            _ => sort_lines(&mut buf, &mut hist, s, SortOrder::Descending),
                        };
                        // Whatever happened, the lines still partition the text.
                        assert!(hist.steps().0 <= 1);
                    }
                }
            }
        }
    }

    /// A reference: the same commands on `Vec<(text, break)>`.
    fn model_lines(t: &str) -> Vec<(String, String)> {
        let b = t.as_bytes();
        let mut out = Vec::new();
        let (mut start, mut i) = (0, 0);
        while i < b.len() {
            let n = match b[i] {
                b'\n' => 1,
                b'\r' if b.get(i + 1) == Some(&b'\n') => 2,
                b'\r' => 1,
                _ => {
                    i += 1;
                    continue;
                }
            };
            out.push((t[start..i].to_string(), t[i..i + n].to_string()));
            i += n;
            start = i;
        }
        out.push((t[start..].to_string(), String::new()));
        out
    }

    #[test]
    fn random_documents_match_a_reference() {
        let mut rng = Rng::new(7);
        let words = ["", "a", "B", "b", "ab", "é", " x", "z "];
        let breaks = ["\n", "\r\n", "\r"];
        for _ in 0..400 {
            let n = 1 + rng.below(8);
            let mut t = String::new();
            for _ in 0..n {
                t.push_str(rng.pick(&words));
                if rng.chance(85) {
                    t.push_str(rng.pick(&breaks));
                }
            }
            let lines = model_lines(&t);
            let mut last = lines.len() - 1;
            if last > 0 && lines[last].0.is_empty() && lines[last].1.is_empty() {
                last -= 1;
            }
            let block = &lines[..=last];
            let texts: Vec<&str> = block.iter().map(|l| l.0.as_str()).collect();
            let rebuild = |new: &[&str], tail_break: &str| {
                let mut s = String::new();
                for (i, x) in new.iter().enumerate() {
                    s.push_str(x);
                    s.push_str(if i + 1 < new.len() {
                        &block[i].1
                    } else {
                        tail_break
                    });
                }
                for l in &lines[last + 1..] {
                    s.push_str(&l.0);
                    s.push_str(&l.1);
                }
                s
            };
            let tail = &block[last].1;
            let mut asc = texts.clone();
            asc.sort();
            assert_eq!(
                sort(&t, Selection::at(0), SortOrder::Ascending).0,
                rebuild(&asc, tail),
                "{t:?}"
            );
            let mut desc = texts.clone();
            desc.sort_by(|a, b| b.cmp(a));
            assert_eq!(
                sort(&t, Selection::at(0), SortOrder::Descending).0,
                rebuild(&desc, tail),
                "{t:?}"
            );
            let mut rev = texts.clone();
            rev.reverse();
            assert_eq!(
                go(&t, Selection::at(0), reverse_lines).0,
                rebuild(&rev, tail),
                "{t:?}"
            );
            let keep: Vec<usize> = (0..texts.len())
                .filter(|&k| texts.iter().position(|x| *x == texts[k]) == Some(k))
                .collect();
            let got = go(&t, Selection::at(0), unique_lines).0;
            if keep.len() == texts.len() {
                assert_eq!(got, t, "{t:?}");
            } else {
                let mut want = String::new();
                for (n, &k) in keep.iter().enumerate() {
                    want.push_str(texts[k]);
                    want.push_str(if n + 1 < keep.len() {
                        &block[k].1
                    } else {
                        tail
                    });
                }
                for l in &lines[last + 1..] {
                    want.push_str(&l.0);
                    want.push_str(&l.1);
                }
                assert_eq!(got, want, "{t:?}");
            }
        }
    }
}
