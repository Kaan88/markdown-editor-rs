//! Markdown conveniences: wrap the selection in emphasis markers, set the heading level of
//! lines, line up a pipe table, make a link of the selection. List and quote continuation on Enter
//! and list indentation on Tab are in [`crate::typing`], which owns the typing keys.
//!
//! Each function is one undo step with one merged [`Change`](crate::Change). [`toggle_wrap`] and
//! [`link_from_selection`] take a list of selections and return it as [`crate::typing`]'s functions
//! do (the outcome of the primary, all selections after, the primary's index; `None` when
//! nothing can be done), working on every selection alone. [`toggle_heading`], [`cycle_heading`]
//! and [`format_table`] take one selection, like the line commands of [`crate::lines`]: they
//! return the selection unchanged with no change (and no history entry) when they change no byte.
//!
//! Nothing here parses a whole document: the scans are bounded by [`SCAN`] bytes around the
//! caret, or, for a table, by [`TABLE_ROWS`], [`TABLE_COLS`], [`TABLE_LINE_BYTES`] and
//! [`TABLE_BYTES`].

use crate::buffer::Buffer;
use crate::display::{Width, width_of};
use crate::edit::{Cmd, Outcome, clamped, nothing, touched_lines};
use crate::history::{History, Kind};
use crate::selection::Selection;
use crate::typing::{Act, Edit, Typed, run};
use std::ops::Range;

/// How far from the caret [`toggle_wrap`] looks for the markers of a span, each way, in bytes.
pub const SCAN: usize = 4096;
/// The most rows, columns, bytes per line and bytes in all that [`format_table`] takes; a larger
/// table is left alone.
pub const TABLE_ROWS: usize = 2000;
pub const TABLE_COLS: usize = 64;
pub const TABLE_LINE_BYTES: usize = 64 * 1024;
pub const TABLE_BYTES: usize = 4 << 20;
/// A selection longer than this, in bytes, is not taken for a URL by [`link_from_selection`].
pub const URL_MAX: usize = 2048;

fn text_of(buf: &Buffer, range: Range<usize>) -> String {
    let mut s = String::new();
    buf.text(range, &mut s);
    s
}

fn ins(at: usize, text: &str) -> Edit {
    Edit {
        start: at,
        end: at,
        text: text.to_string(),
    }
}

fn cut(range: Range<usize>) -> Edit {
    Edit {
        start: range.start,
        end: range.end,
        text: String::new(),
    }
}

// ---- toggle_wrap

/// Whether `marker` stands at `at..at+marker.len()` as a marker of its own: for a one-char marker
/// (`*`, `` ` ``) the chars just outside it must not be the same char, so `*` does not see half
/// of `**`.
fn is_marker(buf: &Buffer, at: usize, marker: &str) -> bool {
    let end = at + marker.len();
    if end > buf.len_bytes() || !buf.is_char_boundary(at) || !buf.is_char_boundary(end) {
        return false;
    }
    if !buf.chunks(at..end).flat_map(str::bytes).eq(marker.bytes()) {
        return false;
    }
    if marker.chars().count() == 1 {
        let c = marker.chars().next();
        let before = if at == 0 {
            None
        } else {
            buf.char_at(buf.floor_char(at - 1))
        };
        if before == c || buf.char_at(end) == c {
            return false;
        }
    }
    true
}

/// The span of `marker` markers on the caret's line that has the caret strictly inside, as
/// (opening marker start, closing marker start). Markers on the line are paired from the first.
fn span_around(buf: &Buffer, caret: usize, marker: &str) -> Option<(usize, usize)> {
    let line = buf.line_range(buf.line_of(caret));
    let from = buf.floor_char(line.start.max(caret.saturating_sub(SCAN)));
    let to = buf.floor_char(line.end.min(caret + SCAN));
    let text = text_of(buf, from..to);
    let m = marker.len();
    let mut at = 0;
    let mut open: Option<usize> = None;
    while at + m <= text.len() {
        if text.is_char_boundary(at) && is_marker(buf, from + at, marker) {
            match open.take() {
                None => open = Some(from + at),
                Some(o) => {
                    if o < caret && caret < from + at + m {
                        return Some((o, from + at));
                    }
                }
            }
            at += m;
        } else {
            at += 1;
        }
    }
    None
}

/// Wraps each selection in `marker` (`"**"`, `"*"`, `` "`" ``, `"~~"`, any other short text
/// without a line break), or takes the markers off:
///
/// * a selection with the marker right inside both its ends, or right outside both, is unwrapped
///   (inside is tried first), and stays on the same text; otherwise it is wrapped and stays on
///   its text, inside the markers. A one-char marker next to the same char does not count as a
///   marker (`*` does not unwrap `**a**`);
/// * a caret inside a span of the marker on its line (markers paired from the line's first, looked
///   for [`SCAN`] bytes each way) unwraps that span and stays where it was in the text; any other
///   caret inserts the marker twice, the caret between.
///
/// Two carets in one span unwrap it once. `None` for an empty marker, one over 16 bytes or with a
/// line break in it, or when the carets' edits would overlap.
pub fn toggle_wrap(
    buf: &mut Buffer,
    hist: &mut History,
    sels: &[Selection],
    primary: usize,
    marker: &str,
) -> Option<Typed> {
    if marker.is_empty() || marker.len() > 16 || marker.contains(['\n', '\r']) {
        return None;
    }
    let m = marker.len();
    run(buf, hist, sels, primary, |buf, s| {
        let len = buf.len_bytes();
        let r = buf.floor_char(s.range().start.min(len))..buf.floor_char(s.range().end.min(len));
        let r = r.start..r.end.max(r.start);
        let forward = s.anchor <= s.caret;
        // The selection's ends, in the old text, keep their direction.
        let sel_act = |edits: Vec<Edit>, extra: usize| {
            let (lo, hi) = ((r.start, extra), (r.end, 0));
            Act {
                edits,
                anchor: if forward { lo } else { hi },
                caret: if forward { hi } else { lo },
                special: true,
                typing: false,
            }
        };
        if !r.is_empty() {
            let inside = r.len() >= 2 * m
                && is_marker(buf, r.start, marker)
                && is_marker(buf, r.end - m, marker);
            if inside {
                return sel_act(vec![cut(r.start..r.start + m), cut(r.end - m..r.end)], 0);
            }
            if r.start >= m && is_marker(buf, r.start - m, marker) && is_marker(buf, r.end, marker)
            {
                return sel_act(vec![cut(r.start - m..r.start), cut(r.end..r.end + m)], 0);
            }
            return sel_act(vec![ins(r.start, marker), ins(r.end, marker)], m);
        }
        if let Some((o, c)) = span_around(buf, r.start, marker) {
            return Act {
                edits: vec![cut(o..o + m), cut(c..c + m)],
                anchor: (r.start, 0),
                caret: (r.start, 0),
                special: true,
                typing: false,
            };
        }
        let pair = format!("{marker}{marker}");
        Act::replace(r.start..r.start, &pair, m, true)
    })
}

// ---- headings

/// The heading prefix of a line's text: (indent length, number of `#`, length of the blanks after
/// them), when the line is a heading (up to 3 spaces, 1 to 6 `#`, then a blank or the end).
fn heading_of(text: &str) -> Option<(usize, usize, usize)> {
    let b = text.as_bytes();
    let indent = b.iter().take_while(|&&c| c == b' ').count();
    if indent > 3 {
        return None;
    }
    let hashes = b[indent..].iter().take_while(|&&c| c == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &b[indent + hashes..];
    if !rest.is_empty() && !matches!(rest[0], b' ' | b'\t') {
        return None;
    }
    let blanks = rest
        .iter()
        .take_while(|&&c| c == b' ' || c == b'\t')
        .count();
    Some((indent, hashes, blanks))
}

/// What the heading commands know of a touched line.
struct HeadLine {
    start: usize,
    blank: bool,
    /// (indent, hashes, blanks after) when it is a heading.
    head: Option<(usize, usize, usize)>,
    /// The leading spaces and tabs, for a line that is not a heading.
    indent: usize,
}

fn head_lines(buf: &Buffer, first: usize, last: usize) -> Vec<HeadLine> {
    (first..=last)
        .map(|l| {
            let r = buf.line_range(l);
            let end = buf.floor_char(r.end.min(r.start + 4096));
            let t = text_of(buf, r.start..end);
            let indent = t.bytes().take_while(|&c| c == b' ' || c == b'\t').count();
            HeadLine {
                start: r.start,
                blank: indent == t.len() && end == r.end,
                head: heading_of(&t),
                indent: if indent <= 3 && t.as_bytes()[..indent].iter().all(|&c| c == b' ') {
                    indent
                } else {
                    0
                },
            }
        })
        .collect()
}

/// Sets every non-blank touched line to heading `level` (0: none).
fn set_heading(buf: &mut Buffer, hist: &mut History, sel: Selection, level: usize) -> Outcome {
    let r = clamped(buf, sel);
    let (first, last) = touched_lines(buf, &r);
    set_heading_pts(buf, hist, sel, level, (first, last), &mut [])
}

/// [`set_heading`], also moving `pts` (offset, whether an insertion at the offset pushes it
/// along) as the text of the lines changes: the other selections' ends, for [`crate::multiline`].
fn set_heading_pts(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    level: usize,
    (first, last): (usize, usize),
    pts: &mut [(usize, bool)],
) -> Outcome {
    let lines = head_lines(buf, first, last);
    let want = "#".repeat(level);
    // (position, bytes removed, text inserted), ascending.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for l in lines.iter().filter(|l| !l.blank) {
        match l.head {
            Some((indent, hashes, blanks)) => {
                let at = l.start + indent;
                let old = hashes + blanks;
                let new = if level == 0 {
                    String::new()
                } else {
                    format!("{want} ")
                };
                // Keep a heading that is already right, whatever the blanks are.
                if level == hashes && blanks >= 1 {
                    continue;
                }
                edits.push((at, old, new));
            }
            None if level > 0 => edits.push((l.start + l.indent, 0, format!("{want} "))),
            None => {}
        }
    }
    if edits.is_empty() {
        return nothing(sel);
    }
    let high = sel.anchor.max(sel.caret);
    let map = |p: usize, grow_at: bool| {
        let mut q = p as isize;
        for (pos, rem, text) in &edits {
            if *rem == 0 {
                if p > *pos || (p == *pos && grow_at) {
                    q += text.len() as isize;
                }
            } else if p >= pos + rem {
                q += text.len() as isize - *rem as isize;
            } else if p > *pos {
                q += text.len() as isize - (p - pos) as isize;
            }
        }
        q as usize
    };
    let after = Selection {
        anchor: map(sel.anchor, sel.is_empty() || sel.anchor == high),
        caret: map(sel.caret, sel.is_empty() || sel.caret == high),
        goal_col: None,
    };
    for p in pts.iter_mut() {
        p.0 = map(p.0, p.1);
    }
    let mut cmd = Cmd::new(buf);
    for (pos, rem, text) in edits.iter().rev() {
        cmd.replace(*pos..pos + rem, text);
    }
    cmd.finish(hist, Kind::Other, sel, after)
}

/// Makes the lines of the selection (the caret's line; a selection ending at column 0 leaves that
/// line out, as for the other line commands) headings of `level` (1 to 6): a line that is not a
/// heading gets `"# "` of that level after its indent (at most 3 spaces count), a heading of
/// another level changes level, a line that already is one of that level loses its prefix, and
/// when every non-blank line is already at `level` all of them lose it. Blank lines are left
/// alone. The text after the prefix is untouched. Nothing for a level outside 1..=6.
pub fn toggle_heading(buf: &mut Buffer, hist: &mut History, sel: Selection, level: u8) -> Outcome {
    if !(1..=6).contains(&level) {
        return nothing(sel);
    }
    let r = clamped(buf, sel);
    let (first, last) = touched_lines(buf, &r);
    let lines = head_lines(buf, first, last);
    let solid: Vec<&HeadLine> = lines.iter().filter(|l| !l.blank).collect();
    let all_there = !solid.is_empty()
        && solid
            .iter()
            .all(|l| l.head.is_some_and(|(_, h, _)| h == level as usize));
    set_heading(buf, hist, sel, if all_there { 0 } else { level as usize })
}

/// Steps the heading level of the lines of the selection through none, 1, 2, ..., 6 and back to
/// none, by the first non-blank line, and sets all of them to the result (so the lines end up
/// alike).
pub fn cycle_heading(buf: &mut Buffer, hist: &mut History, sel: Selection) -> Outcome {
    let r = clamped(buf, sel);
    let lines = touched_lines(buf, &r);
    cycle_heading_pts(buf, hist, sel, lines, &mut [])
}

/// [`cycle_heading`] that also moves `pts` (see `set_heading_pts`).
pub(crate) fn cycle_heading_pts(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    (first, last): (usize, usize),
    pts: &mut [(usize, bool)],
) -> Outcome {
    let lines = head_lines(buf, first, last);
    let Some(l) = lines.iter().find(|l| !l.blank) else {
        return nothing(sel);
    };
    let now = l.head.map_or(0, |(_, h, _)| h);
    set_heading_pts(buf, hist, sel, (now + 1) % 7, (first, last), pts)
}

// ---- tables

/// The cells of a table row: (start, end) of each cell's trimmed text in the line. An optional
/// pipe after the indent and one at the end are not cell separators; cells are cut at the other
/// pipes that no backslash escapes.
fn cells_of(line: &str) -> Vec<(usize, usize)> {
    let b = line.as_bytes();
    let mut from = b.iter().take_while(|&&c| c == b' ' || c == b'\t').count();
    if b.get(from) == Some(&b'|') {
        from += 1;
    }
    let mut bounds: Vec<(usize, usize)> = Vec::new();
    let mut i = from;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'|' => {
                bounds.push((from, i));
                from = i + 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    bounds.push((from.min(b.len()), b.len()));
    if bounds
        .last()
        .is_some_and(|&(s, e)| line[s..e].trim().is_empty())
    {
        bounds.pop();
    }
    bounds
        .into_iter()
        .map(|(s, e)| {
            let t = &line[s..e];
            let lead = t.len() - t.trim_start_matches([' ', '\t']).len();
            let trimmed = t.trim_matches([' ', '\t']);
            (s + lead, s + lead + trimmed.len())
        })
        .collect()
}

/// The alignment of a delimiter cell: `Some(0)` none, 1 left, 2 right, 3 center; `None` when the
/// cell is not made of an optional colon, dashes and an optional colon.
fn alignment(cell: &str) -> Option<u8> {
    let left = cell.starts_with(':');
    let right = cell.len() > 1 && cell.ends_with(':');
    let dashes = &cell[usize::from(left)..cell.len() - usize::from(right)];
    if dashes.is_empty() || !dashes.bytes().all(|c| c == b'-') {
        return None;
    }
    Some(match (left, right) {
        (false, false) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (true, true) => 3,
    })
}

/// The columns a string takes: wide characters 2, combining marks 0, the rest 1.
fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| match width_of(c) {
            Width::Zero => 0,
            Width::Narrow => 1,
            Width::Wide => 2,
        })
        .sum()
}

/// A line of the table.
struct Row {
    /// The line's content range in the buffer.
    range: Range<usize>,
    text: String,
    cells: Vec<(usize, usize)>,
}

/// Lines up the pipe table the caret is in. A table is a header row, a delimiter row (every cell
/// colons and dashes, as many cells as the header) and the pipe lines after it; a pipe line is
/// one that, after optional spaces or tabs, contains a `|` (so the table ends at the first line
/// without one, blank lines included). The caret (the selection's caret end) must be on one of
/// its lines.
///
/// Every row is rewritten as `| cell | cell |`, each cell padded to the widest of its column
/// (display width: wide characters count 2), by the column's alignment (left, none: text first;
/// right: spaces first; center: spaces both sides); the delimiter row is dashes to that width with
/// its colons (3 wide at least). Cell text is only trimmed, never changed; a row with fewer cells
/// than the header keeps as many cells as it had; the header's indent (spaces and tabs) is used
/// for every row; line endings are not touched. A selection end moves with the cell and the
/// offset in the cell text it was at, or, when it was in the padding or at a pipe, to the
/// nearest end of that cell's text. Nothing (the selection returned) outside a table, when the
/// table is over [`TABLE_ROWS`] rows, [`TABLE_COLS`] columns, [`TABLE_LINE_BYTES`] bytes per
/// line or [`TABLE_BYTES`] in all, or when it is already lined up.
pub fn format_table(buf: &mut Buffer, hist: &mut History, sel: Selection) -> Outcome {
    format_table_pts(buf, hist, sel, &mut [])
}

/// [`format_table`] that also moves `pts` (offsets) as the table's rows change, like a selection
/// end: the other selections' ends, for [`crate::multiline`].
pub(crate) fn format_table_pts(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    pts: &mut [usize],
) -> Outcome {
    let len = buf.len_bytes();
    let caret = buf.floor_char(sel.caret.min(len));
    let line = buf.line_of(caret);
    let is_pipe = |buf: &Buffer, l: usize| -> Option<bool> {
        let r = buf.line_range(l);
        if r.len() > TABLE_LINE_BYTES {
            return None;
        }
        Some(buf.chunks(r).any(|p| p.bytes().any(|c| c == b'|')))
    };
    // The run of pipe lines around the caret's line.
    if is_pipe(buf, line) != Some(true) {
        return nothing(sel);
    }
    let (mut r0, mut r1) = (line, line);
    while r0 > 0 {
        match is_pipe(buf, r0 - 1) {
            Some(true) => r0 -= 1,
            Some(false) => break,
            None => return nothing(sel),
        }
        if line - r0 > TABLE_ROWS {
            return nothing(sel);
        }
    }
    while r1 + 1 < buf.len_lines() {
        match is_pipe(buf, r1 + 1) {
            Some(true) => r1 += 1,
            Some(false) => break,
            None => return nothing(sel),
        }
        if r1 - line > TABLE_ROWS {
            return nothing(sel);
        }
    }
    if r1 - r0 + 1 > TABLE_ROWS {
        return nothing(sel);
    }
    let rows_of = |first: usize, last: usize| -> Vec<Row> {
        (first..=last)
            .map(|l| {
                let range = buf.line_range(l);
                let text = text_of(buf, range.clone());
                let cells = cells_of(&text);
                Row { range, text, cells }
            })
            .collect()
    };
    let rows = rows_of(r0, r1);
    if rows.iter().map(|r| r.text.len()).sum::<usize>() > TABLE_BYTES {
        return nothing(sel);
    }
    // The first delimiter row that has a header above it with the same number of cells.
    let delim_of = |row: &Row| -> Option<Vec<u8>> {
        row.cells
            .iter()
            .map(|&(s, e)| alignment(&row.text[s..e]))
            .collect()
    };
    let Some(d) = (1..rows.len()).find(|&i| {
        !rows[i].cells.is_empty()
            && rows[i].cells.len() == rows[i - 1].cells.len()
            && delim_of(&rows[i]).is_some()
    }) else {
        return nothing(sel);
    };
    let start = d - 1;
    if line - r0 < start {
        return nothing(sel);
    }
    let table = &rows[start..];
    let cols = table.iter().map(|r| r.cells.len()).max().unwrap_or(0);
    if cols > TABLE_COLS {
        return nothing(sel);
    }
    let align: Vec<u8> = delim_of(&table[1]).unwrap_or_default();
    let mut width = vec![3usize; cols];
    for (i, row) in table.iter().enumerate() {
        if i == 1 {
            continue;
        }
        for (c, &(s, e)) in row.cells.iter().enumerate() {
            width[c] = width[c].max(display_width(&row.text[s..e]));
        }
    }
    let indent: String = table[0]
        .text
        .chars()
        .take_while(|&c| c == ' ' || c == '\t')
        .collect();
    // The new text of each row and where each cell's text starts in it.
    let mut made: Vec<(String, Vec<usize>)> = Vec::with_capacity(table.len());
    for (i, row) in table.iter().enumerate() {
        let mut out = indent.clone();
        let mut starts = Vec::with_capacity(row.cells.len());
        out.push('|');
        for (c, &(s, e)) in row.cells.iter().enumerate() {
            let a = align.get(c).copied().unwrap_or(0);
            let w = width[c];
            out.push(' ');
            if i == 1 {
                starts.push(out.len());
                let dashes = match a {
                    1 | 2 => w - 1,
                    3 => w - 2,
                    _ => w,
                };
                if a & 1 == 1 {
                    out.push(':');
                }
                out.push_str(&"-".repeat(dashes));
                if a & 2 == 2 {
                    out.push(':');
                }
            } else {
                let cell = &row.text[s..e];
                let pad = w - display_width(cell);
                let (l, r) = match a {
                    2 => (pad, 0),
                    3 => (pad / 2, pad - pad / 2),
                    _ => (0, pad),
                };
                out.push_str(&" ".repeat(l));
                starts.push(out.len());
                out.push_str(cell);
                out.push_str(&" ".repeat(r));
            }
            out.push(' ');
            out.push('|');
        }
        made.push((out, starts));
    }
    // Rows that change, with the map of the selection ends through them.
    struct Change<'a> {
        row: &'a Row,
        new: &'a str,
        starts: &'a [usize],
    }
    let changes: Vec<Change<'_>> = table
        .iter()
        .zip(&made)
        .filter(|(row, (new, _))| row.text != *new)
        .map(|(row, (new, starts))| Change { row, new, starts })
        .collect();
    if changes.is_empty() {
        return nothing(sel);
    }
    let map = |p: usize| -> usize {
        let mut delta = 0isize;
        for c in &changes {
            let r = &c.row.range;
            if p < r.start {
                break;
            }
            if p <= r.end {
                let col = p - r.start;
                let cells = &c.row.cells;
                // The last cell that starts at or before the column.
                let k = cells.partition_point(|&(s, _)| s <= col).saturating_sub(1);
                let (s, e) = cells.get(k).copied().unwrap_or((0, 0));
                let off = col.saturating_sub(s).min(e - s);
                let base = c.starts.get(k).copied().unwrap_or(c.new.len());
                return (r.start as isize + delta) as usize + (base + off).min(c.new.len());
            }
            delta += c.new.len() as isize - r.len() as isize;
        }
        (p as isize + delta) as usize
    };
    let after = Selection {
        anchor: map(sel.anchor.min(len)),
        caret: map(sel.caret.min(len)),
        goal_col: None,
    };
    for p in pts.iter_mut() {
        *p = map((*p).min(len));
    }
    let mut cmd = Cmd::new(buf);
    for c in changes.iter().rev() {
        cmd.replace(c.row.range.clone(), c.new);
    }
    cmd.finish(hist, Kind::Other, sel, after)
}

// ---- links

/// `s` is an `http://` or `https://` URL: no white space, something after the scheme, at most
/// [`URL_MAX`] bytes.
fn looks_like_url(s: &str) -> bool {
    if s.len() > URL_MAX || s.chars().any(char::is_whitespace) {
        return false;
    }
    let lower = s.to_ascii_lowercase();
    ["http://", "https://"]
        .iter()
        .any(|p| lower.starts_with(p) && lower.len() > p.len())
}

/// Makes a link of each selection. Selected text becomes `[text](|)`, the caret between the
/// parentheses; a selected URL (`http://` or `https://`, see [`URL_MAX`]) becomes `[|](url)`,
/// the caret in the brackets; an empty selection inserts `[|]()`. No text is copied: the
/// brackets are inserted around the selection. `None` when the carets' edits would overlap.
pub fn link_from_selection(
    buf: &mut Buffer,
    hist: &mut History,
    sels: &[Selection],
    primary: usize,
) -> Option<Typed> {
    run(buf, hist, sels, primary, |buf, s| {
        let len = buf.len_bytes();
        let r = buf.floor_char(s.range().start.min(len))..buf.floor_char(s.range().end.min(len));
        let r = r.start..r.end.max(r.start);
        if r.is_empty() {
            return Act::replace(r.start..r.start, "[]()", 1, true);
        }
        let url = r.len() <= URL_MAX && looks_like_url(&text_of(buf, r.clone()));
        let (open, close, at) = if url {
            ("[](", ")", 1)
        } else {
            ("[", "]()", 2)
        };
        let caret = if url { (r.start, 1) } else { (r.end, at) };
        Act {
            edits: vec![ins(r.start, open), ins(r.end, close)],
            anchor: caret,
            caret,
            special: true,
            typing: false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{redo, undo};
    use crate::testutil::{Rng, check_change, marked_parse, marked_show};

    const C: char = '¦';

    fn parse(s: &str) -> (String, Vec<Selection>) {
        marked_parse(s, C)
    }

    /// Runs `f`, checks the change and one undo step with undo and redo; returns the marked text.
    fn go(
        s: &str,
        f: impl FnOnce(
            &mut Buffer,
            &mut History,
            &[Selection],
        ) -> Option<(Option<crate::Change>, Vec<Selection>)>,
    ) -> Option<String> {
        let (text, sels) = parse(s);
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        let Some((change, list)) = f(&mut buf, &mut hist, &sels) else {
            assert_eq!(buf.rope().to_string(), text);
            assert!(!hist.can_undo());
            return None;
        };
        let after = buf.rope().to_string();
        match change {
            None => {
                assert_eq!(after, text);
                assert!(!hist.can_undo());
            }
            Some(c) => {
                check_change(&text, &after, &c);
                assert_eq!(hist.steps(), (1, 0), "one undo step");
                undo(&mut buf, &mut hist).unwrap();
                assert_eq!(buf.rope().to_string(), text);
                redo(&mut buf, &mut hist).unwrap();
                assert_eq!(buf.rope().to_string(), after);
            }
        }
        for x in &list {
            assert!(buf.is_char_boundary(x.anchor) && buf.is_char_boundary(x.caret));
        }
        Some(marked_show(&after, &list, C))
    }

    fn wrap(s: &str, marker: &str) -> Option<String> {
        go(s, |b, h, sels| {
            let (o, l, _) = toggle_wrap(b, h, sels, 0, marker)?;
            Some((o.change, l))
        })
    }

    fn one(s: &str, f: impl FnOnce(&mut Buffer, &mut History, Selection) -> Outcome) -> String {
        go(s, |b, h, sels| {
            let o = f(b, h, sels[0]);
            Some((o.change, vec![o.sel]))
        })
        .unwrap()
    }

    fn head(s: &str, level: u8) -> String {
        one(s, |b, h, sel| toggle_heading(b, h, sel, level))
    }

    fn table(s: &str) -> String {
        one(s, format_table)
    }

    fn link(s: &str) -> Option<String> {
        go(s, |b, h, sels| {
            let (o, l, _) = link_from_selection(b, h, sels, 0)?;
            Some((o.change, l))
        })
    }

    #[test]
    fn wrap_a_selection_and_keep_it() {
        assert_eq!(wrap("a ‹b c› d", "**").unwrap(), "a **‹b c›** d");
        assert_eq!(wrap("a ›b‹ d", "*").unwrap(), "a *›b‹* d");
        assert_eq!(wrap("‹é\nz›", "`").unwrap(), "`‹é\nz›`");
        assert_eq!(wrap("‹x›", "~~").unwrap(), "~~‹x›~~");
        // Markers inside the selection come off; so do those right outside it.
        assert_eq!(wrap("‹**b**›", "**").unwrap(), "‹b›");
        assert_eq!(wrap("**‹b›**", "**").unwrap(), "‹b›");
        assert_eq!(wrap("a ›~~b c~~‹ d", "~~").unwrap(), "a ›b c‹ d");
        assert_eq!(wrap("*‹b›*", "*").unwrap(), "‹b›");
        // A single `*` does not see half of `**`: it wraps again.
        assert_eq!(wrap("**‹b›**", "*").unwrap(), "***‹b›***");
        assert_eq!(wrap("‹**b**›", "*").unwrap(), "*‹**b**›*");
        // The other marker: wrap.
        assert_eq!(wrap("*‹b›*", "**").unwrap(), "***‹b›***");
        // A marker pair alone, and one too short to be both.
        assert_eq!(wrap("‹****›", "**").unwrap(), "¦");
    }

    #[test]
    fn wrap_a_caret() {
        // No span: the pair, the caret between.
        assert_eq!(wrap("a ¦ b", "**").unwrap(), "a **¦** b");
        assert_eq!(wrap("¦", "`").unwrap(), "`¦`");
        // Inside a wrapped word or span: unwrap, caret stays on its text.
        assert_eq!(wrap("**bo¦ld**", "**").unwrap(), "bo¦ld");
        assert_eq!(wrap("x *a b¦ c* y", "*").unwrap(), "x a b¦ c y");
        assert_eq!(wrap("`co¦de` and `x`", "`").unwrap(), "co¦de and `x`");
        // Outside the span, or at its edge: the pair.
        assert_eq!(wrap("*a* ¦ *b*", "*").unwrap(), "*a* *¦* *b*");
        assert_eq!(wrap("**a**¦", "**").unwrap(), "**a****¦**");
        // Only the caret's line counts.
        assert_eq!(wrap("**a\nb¦**", "**").unwrap(), "**a\nb**¦****");
    }

    #[test]
    fn wrap_several_carets_and_refusals() {
        assert_eq!(wrap("‹a› ‹b›", "**").unwrap(), "**‹a›** **‹b›**");
        assert_eq!(wrap("¦ ¦", "*").unwrap(), "*¦* *¦*");
        assert_eq!(wrap("**a ¦b ¦c**", "**").unwrap(), "a ¦b ¦c");
        assert_eq!(wrap("¦", ""), None);
        assert_eq!(wrap("¦", "a\nb"), None);
        assert_eq!(wrap("¦", &"*".repeat(17)), None);
        // CRLF documents.
        assert_eq!(wrap("‹a›\r\nb", "**").unwrap(), "**‹a›**\r\nb");
        assert_eq!(wrap("*a¦*\r\n*b*", "*").unwrap(), "a¦\r\n*b*");
    }

    #[test]
    fn headings() {
        assert_eq!(head("a¦", 1), "# a¦");
        assert_eq!(head("¦a", 2), "## ¦a");
        assert_eq!(head("# a¦", 2), "## a¦");
        assert_eq!(head("## a¦", 2), "a¦");
        assert_eq!(head("###### a¦", 6), "a¦");
        assert_eq!(head("   # a¦", 1), "   a¦");
    }

    #[test]
    fn headings_over_lines() {
        assert_eq!(head("‹a\n\nb›", 1), "‹# a\n\n# b›");
        // Mixed: set all; all at the level: remove all.
        assert_eq!(head("‹# a\n## b›", 1), "‹# a\n# b›");
        assert_eq!(head("‹# a\n# b›", 1), "‹a\nb›");
        // Ending at column 0: the next line is left alone. CRLF.
        assert_eq!(head("‹a\r\n›b", 3), "‹### a\r\n›b");
        assert_eq!(head("#a¦", 1), "# #a¦");
        // Indented code is not a heading.
        assert_eq!(head("    # a¦", 1), "#     # a¦");
        // Out of range, and nothing to change.
        let mut buf = Buffer::from_text("a");
        let mut hist = History::new();
        assert!(
            toggle_heading(&mut buf, &mut hist, Selection::at(0), 0)
                .change
                .is_none()
        );
        assert!(
            toggle_heading(&mut buf, &mut hist, Selection::at(0), 7)
                .change
                .is_none()
        );
        let mut buf = Buffer::from_text("\n");
        assert!(
            toggle_heading(&mut buf, &mut hist, Selection::at(0), 1)
                .change
                .is_none()
        );
        assert!(!hist.can_undo());
    }

    #[test]
    fn cycling_headings() {
        let cyc = |s: &str| one(s, cycle_heading);
        assert_eq!(cyc("a¦"), "# a¦");
        assert_eq!(cyc("# a¦"), "## a¦");
        assert_eq!(cyc("##### a¦"), "###### a¦");
        assert_eq!(cyc("###### a¦"), "a¦");
        assert_eq!(cyc("‹# a\nb›"), "‹## a\n## b›");
        assert_eq!(cyc("‹\nb›"), "‹\n# b›");
    }

    #[test]
    fn tables_line_up() {
        assert_eq!(
            table("|a|bb|\n|-|:-:|\n|ccc|d¦|"),
            "| a   | bb  |\n| --- | :-: |\n| ccc |  d¦  |"
        );
        // Outer pipes added, alignment kept, indent of the header used.
        assert_eq!(
            table("  Name | Qty\n---|--:\nx¦ | 10\nlonger | 2"),
            "  | Name   | Qty |\n  | ------ | --: |\n  | x¦      |  10 |\n  | longer |   2 |"
        );
        // Wide characters count 2.
        assert_eq!(
            table("|日本|b|\n|--|-|\n|a¦|語語|"),
            "| 日本 | b    |\n| ---- | ---- |\n| a¦    | 語語 |"
        );
    }

    #[test]
    fn tables_in_context() {
        // Only the table's lines change; CRLF kept.
        let r = table("text | here\r\n\r\n|a|b|\r\n|:-|-:|\r\n|1¦|22|\r\n\r\nafter | x");
        assert_eq!(
            r,
            "text | here\r\n\r\n| a   |   b |\r\n| :-- | --: |\r\n| 1¦   |  22 |\r\n\r\nafter | x"
        );
        // Escaped pipes are not cell separators.
        assert_eq!(
            table("|a\\|b|c|\n|-|-|\n|1¦|2|"),
            "| a\\|b | c   |\n| ---- | --- |\n| 1¦    | 2   |"
        );
        // The caret in the delimiter row, in the padding, at a pipe.
        assert!(
            table("|a|b|\n|-¦|-|")
                .replace(C, "")
                .contains("| --- | --- |")
        );
        // A line above the table with a pipe in it is not part of the table.
        assert_eq!(
            table("x | y\n|a|b|\n|-|-|\n|1¦|2|"),
            "x | y\n| a   | b   |\n| --- | --- |\n| 1¦   | 2   |"
        );
    }

    #[test]
    fn nothing_outside_a_table() {
        for s in [
            "plain¦ text",
            "a | b¦\nc | d",
            "|a|b|\n|-|-|\n\n|1¦|2|",
            "|a|b|\n|--|\n|1¦|2|",
            "|a|b|\n|-|-|\n|1|2|\n¦",
            "¦",
            "|¦",
        ] {
            let (text, sels) = parse(s);
            let mut buf = Buffer::from_text(&text);
            let mut hist = History::new();
            let o = format_table(&mut buf, &mut hist, sels[0]);
            assert!(o.change.is_none() && o.sel == sels[0], "{s:?}");
            assert!(!hist.can_undo());
        }
        // Already lined up.
        let (text, sels) = parse("| a   | b   |\n| --- | --- |\n| 1¦   | 2   |");
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        assert!(format_table(&mut buf, &mut hist, sels[0]).change.is_none());
    }

    #[test]
    fn tables_over_the_caps_are_left_alone() {
        let mut hist = History::new();
        let cols = TABLE_COLS + 1;
        let row = |c: &str| format!("|{}\n", format!("{c}|").repeat(cols));
        let text = format!("{}{}{}", row("a"), row("-"), row("b"));
        let mut buf = Buffer::from_text(&text);
        assert!(
            format_table(&mut buf, &mut hist, Selection::at(2))
                .change
                .is_none()
        );
        let mut text = String::from("|a|b|\n|-|-|\n");
        for _ in 0..TABLE_ROWS + 5 {
            text.push_str("|1|2|\n");
        }
        let mut buf = Buffer::from_text(&text);
        let at = text.len() / 2;
        assert!(
            format_table(&mut buf, &mut hist, Selection::at(at))
                .change
                .is_none()
        );
        let long = format!("|a|b|\n|-|-|\n|{}|2|", "x".repeat(TABLE_LINE_BYTES + 1));
        let mut buf = Buffer::from_text(&long);
        assert!(
            format_table(&mut buf, &mut hist, Selection::at(2))
                .change
                .is_none()
        );
        assert!(!hist.can_undo());
        // The largest allowed table works.
        let mut text = String::from("|a|b|\n|-|-|\n");
        for _ in 0..TABLE_ROWS - 3 {
            text.push_str("|1|22|\n");
        }
        let mut buf = Buffer::from_text(&text);
        assert!(
            format_table(&mut buf, &mut hist, Selection::at(2))
                .change
                .is_some()
        );
    }

    #[test]
    fn links() {
        assert_eq!(link("‹text›").unwrap(), "[text](¦)");
        assert_eq!(link("a ‹two words› b").unwrap(), "a [two words](¦) b");
        assert_eq!(
            link("‹https://example.com/a?b=1›").unwrap(),
            "[¦](https://example.com/a?b=1)"
        );
        assert_eq!(link("›HTTP://x.org‹").unwrap(), "[¦](HTTP://x.org)");
        // Not URLs: spaces, no host, another scheme.
        assert_eq!(link("‹http://a b›").unwrap(), "[http://a b](¦)");
        assert_eq!(link("‹https://›").unwrap(), "[https://](¦)");
        assert_eq!(link("‹ftp://x›").unwrap(), "[ftp://x](¦)");
        assert_eq!(link("¦").unwrap(), "[¦]()");
        assert_eq!(link("‹a›\r\n‹b›").unwrap(), "[a](¦)\r\n[b](¦)");
        let long = format!("‹http://{}›", "a".repeat(URL_MAX));
        assert!(link(&long).unwrap().starts_with("[http://"));
    }

    #[test]
    fn hostile_input_never_panics() {
        let mut rng = Rng::new(5);
        let pieces = [
            "|", "-", ":", "\\", "*", "`", "~", "#", " ", "\t", "\n", "\r\n", "日", "é",
            "http://x", "a",
        ];
        for _ in 0..3000 {
            let mut text = String::new();
            for _ in 0..rng.below(24) {
                text.push_str(rng.pick(&pieces));
            }
            let sels: Vec<Selection> = (0..1 + rng.below(3))
                .map(|_| Selection {
                    anchor: rng.below(text.len() + 3),
                    caret: rng.below(text.len() + 3),
                    goal_col: None,
                })
                .collect();
            let mut buf = Buffer::from_text(&text);
            let mut hist = History::new();
            let changed = match rng.below(5) {
                0 => toggle_wrap(
                    &mut buf,
                    &mut hist,
                    &sels,
                    0,
                    rng.pick(&["**", "*", "`", "~~"]),
                )
                .and_then(|r| r.0.change),
                1 => toggle_heading(&mut buf, &mut hist, sels[0], 1 + rng.below(6) as u8).change,
                2 => cycle_heading(&mut buf, &mut hist, sels[0]).change,
                3 => format_table(&mut buf, &mut hist, sels[0]).change,
                _ => link_from_selection(&mut buf, &mut hist, &sels, 0).and_then(|r| r.0.change),
            };
            if let Some(c) = changed {
                let after = buf.rope().to_string();
                check_change(&text, &after, &c);
                assert_eq!(hist.steps(), (1, 0));
                undo(&mut buf, &mut hist).unwrap();
                assert_eq!(buf.rope().to_string(), text);
            } else {
                assert_eq!(buf.rope().to_string(), text);
            }
        }
    }
}
