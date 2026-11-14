//! The line commands of [`crate::lines`] and [`crate::markdown`] with several selections: one
//! undo step for all of them, every selection kept in a sensible place.
//!
//! [`run`] takes the selections (any order; it normalises them with [`multi::normalize`]), the
//! index of the primary and a [`LineOp`], and returns what [`multi::edit`] returns: the primary's
//! outcome (one merged [`Change`](crate::Change), `None` when nothing changed), every selection
//! after the command in document order, and the primary's index.
//!
//! The rule per command (the lines "touched" by a selection are as for the single-selection
//! commands: from the line of its start to the line of its end, a non-empty selection ending at
//! column 0 leaving that line out):
//!
//! * Selections whose touched lines share a line (for the commands marked "adjacent", also lines
//!   that follow one another) are one *block*. A command acts on each block once, however many
//!   selections are in it, and runs on the blocks from the last to the first so the lines of the
//!   earlier ones do not move under it.
//! * Duplicate: every block is copied below itself; the selections of a block move onto its copy.
//!   Blocks that follow one another are copied each (a, b becomes a, a, b, b).
//! * Move up and down (adjacent): every block moves one line, as the single command does; a block
//!   at the edge of the document stays and the others still move. Selections go with their lines.
//! * Delete (adjacent): the touched lines of all selections go; one caret is left per removed
//!   block, at the column the block's last selection ended in, on the line that took its place.
//! * Toggle comment: the touched lines of all selections are one set. When every non-blank line
//!   of it is commented the comments are taken off, else every non-blank line gets one at the
//!   smallest indentation of the set. When every line is blank, each selection's caret line is
//!   commented.
//! * Change case: each non-empty selection alone; carets change nothing.
//! * Trim trailing spaces, tabs to spaces, line endings: the whole document, as with one caret;
//!   every selection end follows its text.
//! * Sort, reverse, remove duplicates (adjacent): every block of two or more lines is worked on
//!   alone, as if it were the only selection, so duplicates are found within a block and lines
//!   never move from one block to another; each block's selections become one selection over
//!   its lines. When every selection is empty the whole document is the block, as with one caret.
//! * Join (adjacent in the sense of lines it joins): a block is joined into its first line; a
//!   selection inside one line joins its line with the next, so carets on following lines join
//!   all of them.
//! * Heading level: every block steps through none, 1 to 6 by its own first non-blank line, as
//!   the single command does.
//! * Align table: every table with a selection in it, once.
//!
//! Cost is one single command per block (or per selection) plus a pass over the selections to
//! move them, so a column selection down a large file is as fast as the single command times the
//! number of blocks it makes.

use crate::buffer::{Buffer, Change};
use crate::edit::{Cmd, Outcome, clamped, nothing, touched_lines};
use crate::history::{History, Kind};
use crate::lines::{self, SortOrder};
use crate::markdown;
use crate::multi;
use crate::selection::Selection;
use std::ops::Range;

/// A line command to run at every selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineOp<'a> {
    /// With the line ending to join a last line to its copy.
    Duplicate(&'a str),
    /// Down when true.
    Move(bool),
    Delete,
    /// With the language's line comment prefix.
    Comment(&'a str),
    /// Upper case when true.
    Case(bool),
    Trim,
    /// With the tab width.
    TabsToSpaces(usize),
    /// To `"\r\n"`, `"\n"` or `"\r"`.
    LineEndings(&'a str),
    Sort(SortOrder),
    Reverse,
    Unique,
    Join,
    /// Markdown: the heading level steps up.
    Heading,
    /// Markdown: line up the table.
    Table,
}

/// The selections whose touched lines make one block: `first..=last` and the indexes of its
/// selections in the normalised list (contiguous: the list is in document order).
struct Block {
    first: usize,
    last: usize,
    members: Range<usize>,
}

/// Which lines of a selection a block stands for, and when two blocks are one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Merge {
    /// Blocks that share a line are one.
    Share,
    /// Blocks that share a line or follow one another are one.
    Adjacent,
}

/// The blocks of the normalised selections `norm`. `extend_one` makes a selection inside one line
/// stand for that line and the next (join).
fn blocks(buf: &Buffer, norm: &[Selection], merge: Merge, extend_one: bool) -> Vec<Block> {
    let gap = usize::from(merge == Merge::Adjacent);
    let mut out: Vec<Block> = Vec::new();
    for (i, s) in norm.iter().enumerate() {
        let r = clamped(buf, *s);
        let (first, mut last) = touched_lines(buf, &r);
        if extend_one && first == last {
            last = first + 1;
        }
        match out.last_mut() {
            Some(b) if first <= b.last + gap => {
                b.last = b.last.max(last);
                b.members.end = i + 1;
            }
            _ => out.push(Block {
                first,
                last,
                members: i..i + 1,
            }),
        }
    }
    out
}

/// A selection for a block, to give the single-selection command: the selection itself for one
/// member, else the first member's start to the last member's end (the commands take the lines
/// from the block, not from this).
fn hull(norm: &[Selection], b: &Block) -> Selection {
    if b.members.len() == 1 {
        return norm[b.members.start];
    }
    let m = &norm[b.members.clone()];
    let start = m.iter().map(|s| s.range().start).min().unwrap_or(0);
    let end = m.iter().map(|s| s.range().end).max().unwrap_or(0);
    Selection {
        anchor: start,
        caret: end,
        goal_col: None,
    }
}

/// Moves the selections after `from` by `delta` bytes.
fn shift_after(cur: &mut [Selection], from: usize, delta: isize) {
    for s in &mut cur[from..] {
        s.anchor = s.anchor.saturating_add_signed(delta);
        s.caret = s.caret.saturating_add_signed(delta);
    }
}

fn plain(s: Selection) -> Selection {
    Selection {
        goal_col: None,
        ..s
    }
}

/// Whether a block the sort, reverse or duplicate-removing command would work on is over
/// [`LINE_BLOCK_BYTES`](crate::limits::LINE_BLOCK_BYTES), or (join) the lines to join are. The
/// shell asks before running, to say so.
pub fn too_large(buf: &Buffer, sels: &[Selection], primary: usize, op: LineOp<'_>) -> bool {
    let (norm, p) = multi::normalize(buf, sels, primary);
    if norm.is_empty() {
        return false;
    }
    match op {
        LineOp::Sort(_) | LineOp::Reverse | LineOp::Unique => {
            if norm.iter().all(Selection::is_empty) {
                return lines::block_too_large(buf, norm[p], true);
            }
            blocks(buf, &norm, Merge::Adjacent, false).iter().any(|b| {
                let (f, l) = lines::block_range(buf, b.first, b.last);
                lines::range_too_large(buf, f, l)
            })
        }
        LineOp::Join => blocks(buf, &norm, Merge::Share, true)
            .iter()
            .any(|b| b.last < buf.len_lines() && lines::range_too_large(buf, b.first, b.last)),
        _ => false,
    }
}

/// Runs `op` at every selection in `sels` as one undo step; see the module's rules. When nothing
/// changes the outcome has no change and the list is the normalised input.
pub fn run(
    buf: &mut Buffer,
    hist: &mut History,
    sels: &[Selection],
    primary: usize,
    op: LineOp<'_>,
) -> (Outcome, Vec<Selection>, usize) {
    let (norm, p) = multi::normalize(buf, sels, primary);
    if norm.is_empty() {
        return (nothing(Selection::at(0)), norm, 0);
    }
    let before = norm[p];
    let mut cur = norm.clone();
    let id = hist.next_step_id();
    let mut merged: Option<Change> = None;
    let mut add = |c: Option<Change>| {
        if let Some(c) = c {
            match merged.as_mut() {
                Some(m) => m.merge(&c),
                None => merged = Some(c),
            }
        }
    };
    match op {
        LineOp::Duplicate(_) | LineOp::Move(_) | LineOp::Delete => {
            let merge = if matches!(op, LineOp::Duplicate(_)) {
                Merge::Share
            } else {
                Merge::Adjacent
            };
            for b in blocks(buf, &norm, merge, false).iter().rev() {
                let h = hull(&cur, b);
                let len0 = buf.len_bytes();
                let old_start = buf.line_start(b.first);
                let out = match op {
                    LineOp::Duplicate(eol) => {
                        lines::duplicate_range(buf, hist, h, b.first, b.last, eol)
                    }
                    LineOp::Move(down) => lines::move_range(buf, hist, h, b.first, b.last, down),
                    _ => lines::delete_range(buf, hist, h, b.first, b.last),
                };
                if out.change.is_none() {
                    continue;
                }
                let delta = buf.len_bytes() as isize - len0 as isize;
                shift_after(&mut cur, b.members.end, delta);
                if matches!(op, LineOp::Delete) {
                    for s in &mut cur[b.members.clone()] {
                        *s = plain(out.sel);
                    }
                } else {
                    // The block's lines moved as one: every end of it by the same distance, the
                    // block's first line being where the line it swapped with was (a duplicate:
                    // the selection's own move).
                    let d = match op {
                        LineOp::Move(down) => {
                            let to = if down { b.first + 1 } else { b.first - 1 };
                            buf.line_start(to) as isize - old_start as isize
                        }
                        _ => out.sel.anchor as isize - h.anchor as isize,
                    };
                    shift_after(&mut cur[..b.members.end], b.members.start, d);
                    let len = buf.len_bytes();
                    for s in &mut cur[b.members.clone()] {
                        s.anchor = s.anchor.min(len);
                        s.caret = s.caret.min(len);
                    }
                }
                add(out.change);
            }
        }
        LineOp::Sort(_) | LineOp::Reverse | LineOp::Unique => {
            let whole = norm.iter().all(Selection::is_empty);
            let list = if whole {
                vec![Block {
                    first: 0,
                    last: buf.len_lines() - 1,
                    members: 0..norm.len(),
                }]
            } else {
                blocks(buf, &norm, Merge::Adjacent, false)
            };
            for b in list.iter().rev() {
                let (first, last) = lines::block_range(buf, b.first, b.last);
                if first == last {
                    continue;
                }
                let h = hull(&cur, b);
                let len0 = buf.len_bytes();
                let out = match op {
                    LineOp::Sort(order) => lines::sort_range(buf, hist, h, first, last, order),
                    LineOp::Reverse => lines::reverse_range(buf, hist, h, first, last),
                    _ => lines::unique_range(buf, hist, h, first, last),
                };
                if out.change.is_none() {
                    continue;
                }
                let delta = buf.len_bytes() as isize - len0 as isize;
                shift_after(&mut cur, b.members.end, delta);
                for s in &mut cur[b.members.clone()] {
                    *s = plain(out.sel);
                }
                add(out.change);
            }
        }
        LineOp::Join => {
            for b in blocks(buf, &norm, Merge::Share, true).iter().rev() {
                let h = hull(&cur, b);
                let single = b.members.len() == 1 && h.is_empty();
                let len0 = buf.len_bytes();
                let out = lines::join_range(buf, hist, h, b.first, b.last, single);
                if out.change.is_none() {
                    continue;
                }
                let delta = buf.len_bytes() as isize - len0 as isize;
                shift_after(&mut cur, b.members.end, delta);
                for s in &mut cur[b.members.clone()] {
                    *s = plain(out.sel);
                }
                add(out.change);
            }
        }
        LineOp::Case(upper) => {
            for i in (0..cur.len()).rev() {
                if cur[i].is_empty() {
                    continue;
                }
                let len0 = buf.len_bytes();
                let out = lines::change_case(buf, hist, cur[i], upper);
                if out.change.is_none() {
                    continue;
                }
                let delta = buf.len_bytes() as isize - len0 as isize;
                shift_after(&mut cur, i + 1, delta);
                cur[i] = plain(out.sel);
                add(out.change);
            }
        }
        LineOp::Comment(prefix) => {
            let mut set: Vec<usize> = Vec::new();
            let mut carets: Vec<usize> = Vec::new();
            for s in &norm {
                let (f, l) = touched_lines(buf, &clamped(buf, *s));
                carets.push(buf.line_of(buf.floor_char(s.caret)).clamp(f, l));
                set.extend(f..=l);
            }
            set.dedup();
            carets.dedup();
            if let Some(plan) = lines::comment_plan(buf, &set, &carets, prefix) {
                let len = buf.len_bytes();
                for s in &mut cur {
                    s.anchor = plan.map(s.anchor.min(len));
                    s.caret = plan.map(s.caret.min(len));
                    s.goal_col = None;
                }
                let mut cmd = Cmd::new(buf);
                plan.apply(&mut cmd);
                let out = cmd.finish(hist, Kind::Other, before, cur[p]);
                add(out.change);
            }
        }
        LineOp::Trim | LineOp::TabsToSpaces(_) | LineOp::LineEndings(_) => {
            let out = match op {
                LineOp::Trim => lines::trim_trailing_whitespace(buf, hist, before),
                LineOp::TabsToSpaces(tab) => lines::tabs_to_spaces(buf, hist, before, tab),
                LineOp::LineEndings(to) => lines::convert_line_endings(buf, hist, before, to),
                _ => nothing(before),
            };
            if let Some(change) = &out.change {
                // The primary moved as the command says; the others through its replacements.
                for (i, s) in cur.iter_mut().enumerate() {
                    if i == p {
                        *s = out.sel;
                    } else {
                        s.anchor = change.map_offset(s.anchor);
                        s.caret = change.map_offset(s.caret);
                        s.goal_col = None;
                    }
                }
            }
            add(out.change);
        }
        LineOp::Heading => {
            for b in blocks(buf, &norm, Merge::Share, false).iter().rev() {
                let h = hull(&cur, b);
                let mut pts: Vec<(usize, bool)> = Vec::with_capacity(cur.len() * 2);
                for s in &cur {
                    let high = s.anchor.max(s.caret);
                    pts.push((s.anchor, s.is_empty() || s.anchor == high));
                    pts.push((s.caret, s.is_empty() || s.caret == high));
                }
                let out = markdown::cycle_heading_pts(buf, hist, h, (b.first, b.last), &mut pts);
                if out.change.is_none() {
                    continue;
                }
                for (s, ends) in cur.iter_mut().zip(pts.chunks(2)) {
                    s.anchor = ends[0].0;
                    s.caret = ends[1].0;
                    s.goal_col = None;
                }
                add(out.change);
            }
        }
        LineOp::Table => {
            for i in (0..cur.len()).rev() {
                // Only a caret on a line with a pipe can be in a table.
                let line = buf.line_range(buf.line_of(cur[i].caret));
                if !buf.chunks(line).any(|c| c.contains('|')) {
                    continue;
                }
                let mut pts: Vec<usize> = Vec::with_capacity(cur.len() * 2);
                for (j, s) in cur.iter().enumerate() {
                    if j != i {
                        pts.extend([s.anchor, s.caret]);
                    }
                }
                let out = markdown::format_table_pts(buf, hist, cur[i], &mut pts);
                if out.change.is_none() {
                    continue;
                }
                let mut ends = pts.chunks(2);
                for (j, s) in cur.iter_mut().enumerate() {
                    if j == i {
                        *s = plain(out.sel);
                    } else if let Some(e) = ends.next() {
                        s.anchor = e[0];
                        s.caret = e[1];
                        s.goal_col = None;
                    }
                }
                add(out.change);
            }
        }
    }
    let Some(change) = merged else {
        return (nothing(before), norm, p);
    };
    let after = cur[p];
    hist.fuse_since(id, before, after);
    let (list, pi) = multi::normalize(buf, &cur, p);
    (
        Outcome {
            sel: list[pi],
            change: Some(change),
        },
        list,
        pi,
    )
}

#[cfg(test)]
mod tests;
