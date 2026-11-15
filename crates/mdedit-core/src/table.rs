//! Pipe table editing for Markdown: Tab and Shift+Tab between cells, Enter at the end of a row,
//! and the row and column commands. What a table is, and its limits, are in
//! [`crate::markdown::format_table`], which also lines a table up on its own command.
//!
//! Every function here is one undo step with one merged [`Change`](crate::Change), and takes one
//! selection. The table is read into its cells, changed there, laid out again by
//! [`crate::markdown::lay_out`] (so every move that leaves a cell lines the columns up, and
//! nothing does it on a keystroke of typing) and put back as one replacement of the part of the
//! text that differs. Line endings of the rows that stay are kept; a new row takes the document's.
//! The caret is put in a cell by the row, the cell and the offset in the cell's text.
//!
//! Nothing here runs unless one of these functions is called, and each looks only at the table
//! around the caret (at most [`TABLE_ROWS`](crate::markdown::TABLE_ROWS) rows).

use crate::buffer::Buffer;
use crate::edit::{Cmd, Outcome};
use crate::history::{History, Kind};
use crate::markdown::{Located, Row, TABLE_COLS, TABLE_ROWS, lay_out, locate};
use crate::selection::Selection;

/// A command on the table the caret is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableOp {
    /// A row with empty cells under the caret's row (under the delimiter row from the header).
    RowBelow,
    /// A row above the caret's row, which must be a body row.
    RowAbove,
    /// The caret's row, which must be a body row.
    DeleteRow,
    /// A column of empty cells after the caret's.
    ColumnRight,
    /// A column of empty cells before the caret's.
    ColumnLeft,
    /// The caret's column, unless it is the only one.
    DeleteColumn,
    /// The caret's column swaps places with the one on its left.
    MoveColumnLeft,
    /// The caret's column swaps places with the one on its right.
    MoveColumnRight,
    /// The caret's row, which must be a body row, swaps places with the body row above it.
    MoveRowUp,
    /// The caret's row, which must be a body row, swaps places with the body row below it.
    MoveRowDown,
    /// The body rows are sorted by the caret's column: ascending, or descending when they are
    /// already in ascending order (so a second use reverses the first). A column whose cells
    /// all are numbers is compared as numbers, else as text without regard to case. Equal rows
    /// keep their order.
    SortColumn,
}

/// One row of the table being changed: the old row it came from and its cells' text.
struct GridRow {
    src: Option<usize>,
    cells: Vec<String>,
}

/// The table as cells, with the alignment of each column.
struct Grid {
    /// The text before the header row and before the others.
    first: String,
    rest: String,
    align: Vec<u8>,
    rows: Vec<GridRow>,
}

/// Where the caret goes: row, cell, byte offset in the cell's text.
#[derive(Clone, Copy)]
struct Target {
    row: usize,
    cell: usize,
    off: usize,
}

impl Grid {
    fn of(t: &Located) -> Grid {
        let rows = t
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| GridRow {
                src: Some(i),
                cells: r
                    .cells
                    .iter()
                    .map(|&(s, e)| r.text[s..e].to_string())
                    .collect(),
            })
            .collect();
        Grid {
            first: t.first.clone(),
            rest: t.rest.clone(),
            align: t.align.clone(),
            rows,
        }
    }

    fn cols(&self) -> usize {
        self.align.len()
    }

    /// Gives row `row` at least `n` cells.
    fn pad(&mut self, row: usize, n: usize) {
        let cells = &mut self.rows[row].cells;
        while cells.len() < n {
            cells.push(String::new());
        }
    }

    fn empty_row(&self) -> GridRow {
        GridRow {
            src: None,
            cells: vec![String::new(); self.cols()],
        }
    }
}

/// The cell of `row` that column `col` of its line is in: the cell whose text between the
/// separators holds it, a column at a pipe belonging to the cell before it. Before the first
/// cell (the prefix, the pipe in front) the first, after the last the last.
fn cell_at(row: &crate::markdown::Row, col: usize) -> usize {
    row.bounds
        .partition_point(|&(s, _)| s <= col)
        .saturating_sub(1)
}

/// The caret as a row, cell and offset in the cell's text.
fn caret_in(t: &Located, caret: usize) -> Target {
    let row = &t.rows[t.caret_row];
    let col = caret.saturating_sub(row.range.start);
    let cell = cell_at(row, col);
    let off = row.cells.get(cell).map_or(0, |&(s, e)| {
        // A delimiter cell has no text of its own to keep a place in.
        if t.caret_row == 1 {
            0
        } else {
            col.saturating_sub(s).min(e - s)
        }
    });
    Target {
        row: t.caret_row,
        cell,
        off,
    }
}

/// Puts `grid` in the place of the table `t` and the caret at `to`. One undo step; no change
/// (only the caret moved) when the text comes out the same.
fn apply(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    t: &Located,
    grid: &Grid,
    to: Target,
    eol: &str,
) -> Outcome {
    let n = t.rows.len();
    let cells: Vec<Vec<String>> = grid.rows.iter().map(|r| r.cells.clone()).collect();
    let made = lay_out((&grid.first, &grid.rest), &grid.align, &cells);
    // The old text, and the new, each row followed by the line break that followed it.
    let term = |i: usize| -> String {
        let mut s = String::new();
        buf.text(t.rows[i].range.end..t.rows[i + 1].range.start, &mut s);
        s
    };
    let mut old = String::new();
    for i in 0..n {
        old.push_str(&t.rows[i].text);
        if i + 1 < n {
            old.push_str(&term(i));
        }
    }
    let mut new = String::new();
    let mut starts = Vec::with_capacity(made.len());
    for (j, (text, _)) in made.iter().enumerate() {
        starts.push(new.len());
        new.push_str(text);
        if j + 1 < made.len() {
            match grid.rows[j].src {
                Some(i) if i + 1 < n => new.push_str(&term(i)),
                _ => new.push_str(eol),
            }
        }
    }
    let base = t.rows[0].range.start;
    let (row, cell) = (to.row.min(made.len() - 1), to.cell);
    let (text, cs) = &made[row];
    let text_len = grid.rows[row].cells.get(cell).map_or(0, String::len);
    let in_row = cs
        .get(cell)
        .map_or(text.len(), |&c| c + to.off.min(text_len));
    let caret = base + starts[row] + in_row;
    let after = Selection {
        anchor: caret,
        caret,
        goal_col: None,
    };
    if old == new {
        return Outcome {
            sel: after,
            change: None,
        };
    }
    // Only the middle that differs is replaced.
    let mut a = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(x, y)| x == y)
        .count();
    while !old.is_char_boundary(a) || !new.is_char_boundary(a) {
        a -= 1;
    }
    let limit = old.len().min(new.len()) - a;
    let mut z = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take_while(|(x, y)| x == y)
        .count()
        .min(limit);
    while !old.is_char_boundary(old.len() - z) || !new.is_char_boundary(new.len() - z) {
        z -= 1;
    }
    let mut cmd = Cmd::new(buf);
    cmd.replace(base + a..base + old.len() - z, &new[a..new.len() - z]);
    cmd.finish(hist, Kind::Other, sel, after)
}

/// Whether the text of `row` from `col` on is only white space, with something before it.
fn at_end(row: &Row, col: usize) -> bool {
    let text = &row.text;
    col > row.lead && text[col.min(text.len())..].trim().is_empty()
}

/// Tab (or Shift+Tab with `back`) in a table: lines the table up and puts the caret at the start
/// of the text of the next (previous) cell. After a row's last cell it is the first cell of the
/// next row (the delimiter row is skipped); Tab in the last cell of the last row adds a row. A
/// row with fewer cells than the header gets the missing cell. Shift+Tab in the first cell of
/// the header stays where it is. `None` (nothing done, the shell does its Tab) when the caret is
/// not in a table, a selection spans lines, or the table is at its row limit.
pub fn next_cell(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    back: bool,
    eol: &str,
) -> Option<Outcome> {
    if buf.line_of(sel.anchor.min(buf.len_bytes())) != buf.line_of(sel.caret.min(buf.len_bytes())) {
        return None;
    }
    let caret = buf.floor_char(sel.caret.min(buf.len_bytes()));
    let t = locate(buf, caret)?;
    let mut g = Grid::of(&t);
    let at = caret_in(&t, caret);
    let (r, c) = (at.row, at.cell);
    let cols = g.cols();
    let to = if !back {
        if c + 1 < g.rows[r].cells.len().max(cols) {
            g.pad(r, c + 2);
            Target {
                row: r,
                cell: c + 1,
                off: 0,
            }
        } else {
            let next = if r == 0 { 2 } else { r + 1 };
            if next >= g.rows.len() {
                if g.rows.len() >= TABLE_ROWS {
                    return None;
                }
                let new = g.empty_row();
                g.rows.push(new);
            }
            g.pad(next, 1);
            Target {
                row: next,
                cell: 0,
                off: 0,
            }
        }
    } else if c > 0 {
        Target {
            row: r,
            cell: c - 1,
            off: 0,
        }
    } else if r == 0 {
        Target {
            row: 0,
            cell: 0,
            off: 0,
        }
    } else {
        let prev = if r == 2 { 0 } else { r - 1 };
        g.pad(prev, 1);
        Target {
            row: prev,
            cell: g.rows[prev].cells.len() - 1,
            off: 0,
        }
    };
    Some(apply(buf, hist, sel, &t, &g, to, eol))
}

/// Enter with the caret at the end of a table row (nothing but white space after it, no
/// selection): a new row of empty cells under it, the columns lined up, the caret in its first
/// cell. At the end of the header or the delimiter row the new row is the first of the body. At
/// the end of the last row when its cells are all empty (the row an earlier Enter added) the row is
/// emptied instead, so the table ends and the caret is on a line of its own. `None` (the shell's
/// Enter) elsewhere.
pub fn row_after(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    eol: &str,
) -> Option<Outcome> {
    let len = buf.len_bytes();
    if sel.anchor != sel.caret {
        return None;
    }
    let caret = buf.floor_char(sel.caret.min(len));
    let t = locate(buf, caret)?;
    let row = &t.rows[t.caret_row];
    if !at_end(row, caret - row.range.start) {
        return None;
    }
    let r = t.caret_row;
    // Only the last row: an empty row in the middle gets a row after it like any other, so
    // Enter never cuts a table in two.
    if r >= 2 && r + 1 == t.rows.len() && row.cells.iter().all(|&(s, e)| s == e) {
        // The row goes; its quote markers stay.
        let prefix = &row.text[..row.indent];
        let keep = if prefix.contains('>') { prefix } else { "" };
        let mut cmd = Cmd::new(buf);
        cmd.replace(row.range.clone(), keep);
        let at = row.range.start + keep.len();
        return Some(cmd.finish(hist, Kind::Other, sel, Selection::at(at)));
    }
    if t.rows.len() >= TABLE_ROWS {
        return None;
    }
    let mut g = Grid::of(&t);
    let at = r.max(1) + 1;
    let new = g.empty_row();
    g.rows.insert(at, new);
    let to = Target {
        row: at,
        cell: 0,
        off: 0,
    };
    Some(apply(buf, hist, sel, &t, &g, to, eol))
}

/// Whether [`edit`] can do `op` with the caret where it is: the caret is in a table and the
/// command has something to work on there.
pub fn can(buf: &Buffer, sel: Selection, op: TableOp) -> bool {
    let caret = buf.floor_char(sel.caret.min(buf.len_bytes()));
    let Some(t) = locate(buf, caret) else {
        return false;
    };
    let cols = t.align.len();
    let at = caret_in(&t, caret);
    let c = at.cell.min(cols - 1);
    match op {
        TableOp::RowBelow => t.rows.len() < TABLE_ROWS,
        TableOp::RowAbove => at.row >= 2 && t.rows.len() < TABLE_ROWS,
        TableOp::DeleteRow => at.row >= 2,
        TableOp::ColumnRight | TableOp::ColumnLeft => cols < TABLE_COLS,
        TableOp::DeleteColumn => cols >= 2,
        TableOp::MoveColumnLeft => c >= 1,
        TableOp::MoveColumnRight => c + 1 < cols,
        TableOp::MoveRowUp => at.row >= 3,
        TableOp::MoveRowDown => at.row >= 2 && at.row + 1 < t.rows.len(),
        TableOp::SortColumn => t.rows.len() >= 4,
    }
}

/// A row or column command (see [`TableOp`]) on the table the caret (the selection's caret end;
/// the selection collapses to it) is in. The table is lined up, the caret ends in the same cell,
/// or in the new one (a new row or column), or in the cell that took the place of a deleted
/// one, or follows a moved column with its place in the text. `None`, nothing done, when
/// [`can`] is false.
pub fn edit(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    op: TableOp,
    eol: &str,
) -> Option<Outcome> {
    if !can(buf, sel, op) {
        return None;
    }
    let caret = buf.floor_char(sel.caret.min(buf.len_bytes()));
    let t = locate(buf, caret)?;
    let mut g = Grid::of(&t);
    let at = caret_in(&t, caret);
    let (r, cols) = (at.row, g.cols());
    let c = at.cell.min(cols - 1);
    let keep = Target {
        row: r,
        cell: at.cell,
        off: at.off,
    };
    let to = match op {
        TableOp::RowBelow => {
            let i = r.max(1) + 1;
            let new = g.empty_row();
            g.rows.insert(i, new);
            Target {
                row: i,
                cell: c,
                off: 0,
            }
        }
        TableOp::RowAbove => {
            let new = g.empty_row();
            g.rows.insert(r, new);
            Target {
                row: r,
                cell: c,
                off: 0,
            }
        }
        TableOp::DeleteRow => {
            g.rows.remove(r);
            let row = if r < g.rows.len() { r } else { r - 1 };
            // With no body row left the caret goes to the header.
            let row = if row == 1 { 0 } else { row };
            g.pad(row, 1);
            Target {
                row,
                cell: c.min(g.rows[row].cells.len() - 1),
                off: 0,
            }
        }
        TableOp::ColumnRight | TableOp::ColumnLeft => {
            let pos = if op == TableOp::ColumnRight { c + 1 } else { c };
            for (i, row) in g.rows.iter_mut().enumerate() {
                if row.cells.len() >= pos {
                    row.cells
                        .insert(pos, if i == 1 { "---".into() } else { String::new() });
                }
            }
            g.align.insert(pos, 0);
            g.pad(r, pos + 1);
            Target {
                row: r,
                cell: pos,
                off: 0,
            }
        }
        TableOp::DeleteColumn => {
            for row in &mut g.rows {
                if row.cells.len() > c {
                    row.cells.remove(c);
                }
            }
            g.align.remove(c);
            g.pad(r, 1);
            Target {
                row: r,
                cell: c.min(g.rows[r].cells.len() - 1),
                off: 0,
            }
        }
        TableOp::MoveColumnLeft | TableOp::MoveColumnRight => {
            let (a, b) = if op == TableOp::MoveColumnLeft {
                (c - 1, c)
            } else {
                (c, c + 1)
            };
            for row in &mut g.rows {
                if row.cells.len() > a {
                    while row.cells.len() <= b {
                        row.cells.push(String::new());
                    }
                    row.cells.swap(a, b);
                }
            }
            g.align.swap(a, b);
            let cell = if op == TableOp::MoveColumnLeft {
                c - 1
            } else {
                c + 1
            };
            Target { cell, ..keep }
        }
        TableOp::MoveRowUp | TableOp::MoveRowDown => {
            let to = if op == TableOp::MoveRowUp {
                r - 1
            } else {
                r + 1
            };
            g.rows.swap(r, to);
            Target { row: to, ..keep }
        }
        TableOp::SortColumn => {
            let row = sort_body(&mut g, c, r);
            Target { row, ..keep }
        }
    };
    Some(apply(buf, hist, sel, &t, &g, to, eol))
}

/// A cell as a number: trimmed, finite, and a number for Rust's float parser.
fn number(cell: &str) -> Option<f64> {
    let t = cell.trim();
    if !t.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '-' | '+' | '.')) {
        return None;
    }
    t.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Sorts the body rows of `g` (from the third) by column `col` as [`TableOp::SortColumn`] says;
/// returns where the row that was at `caret_row` is now (the same when it is not a body row).
fn sort_body(g: &mut Grid, col: usize, caret_row: usize) -> usize {
    use std::cmp::Ordering;
    let cell = |row: &GridRow| row.cells.get(col).map_or("", String::as_str).to_string();
    let nums: Option<Vec<f64>> = g.rows[2..].iter().map(|r| number(&cell(r))).collect();
    let keys: Vec<String> = g.rows[2..].iter().map(|r| cell(r).to_lowercase()).collect();
    let mut order: Vec<usize> = (0..keys.len()).collect();
    let cmp = |a: usize, b: usize| -> Ordering {
        match &nums {
            Some(n) => n[a].total_cmp(&n[b]),
            None => keys[a].cmp(&keys[b]),
        }
    };
    let sorted = order
        .windows(2)
        .all(|w| cmp(w[0], w[1]) != Ordering::Greater);
    if sorted {
        order.sort_by(|&a, &b| cmp(b, a));
    } else {
        order.sort_by(|&a, &b| cmp(a, b));
    }
    let mut body: Vec<Option<GridRow>> = g.rows.drain(2..).map(Some).collect();
    g.rows.extend(
        order
            .iter()
            .map(|&i| body[i].take().expect("each row once")),
    );
    if caret_row < 2 {
        return caret_row;
    }
    2 + order.iter().position(|&i| i + 2 == caret_row).unwrap_or(0)
}

/// What the caret is in, for the shell: `Nothing` outside a table.
pub fn in_table(buf: &Buffer, sel: Selection) -> bool {
    locate(buf, buf.floor_char(sel.caret.min(buf.len_bytes()))).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{redo, undo};
    use crate::testutil::{Rng, check_change, marked_parse, marked_show};

    const C: char = '¦';

    /// Runs `f` on the marked text; checks the change, one undo step, undo and redo; returns the
    /// marked result (`None` when `f` did nothing at all).
    fn go(
        s: &str,
        f: impl FnOnce(&mut Buffer, &mut History, Selection) -> Option<Outcome>,
    ) -> Option<String> {
        let (text, sels) = marked_parse(s, C);
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        let out = f(&mut buf, &mut hist, sels[0]);
        let Some(out) = out else {
            assert_eq!(buf.rope().to_string(), text);
            assert!(!hist.can_undo());
            return None;
        };
        let after = buf.rope().to_string();
        match &out.change {
            None => {
                assert_eq!(after, text);
                assert!(!hist.can_undo());
            }
            Some(c) => {
                check_change(&text, &after, c);
                assert_eq!(hist.steps(), (1, 0), "one undo step");
                undo(&mut buf, &mut hist).unwrap();
                assert_eq!(buf.rope().to_string(), text);
                redo(&mut buf, &mut hist).unwrap();
                assert_eq!(buf.rope().to_string(), after);
            }
        }
        assert!(buf.is_char_boundary(out.sel.caret));
        Some(marked_show(&after, &[out.sel], C))
    }

    fn tab(s: &str, back: bool) -> Option<String> {
        go(s, |b, h, sel| next_cell(b, h, sel, back, "\n"))
    }

    fn op(s: &str, o: TableOp) -> Option<String> {
        go(s, |b, h, sel| edit(b, h, sel, o, "\n"))
    }

    fn enter(s: &str) -> Option<String> {
        go(s, |b, h, sel| row_after(b, h, sel, "\n"))
    }

    #[test]
    fn tab_moves_to_the_next_cell_and_lines_the_table_up() {
        assert_eq!(
            tab("|a¦|bb|\n|-|-|\n|1|2|", false).unwrap(),
            "| a   | ¦bb  |\n| --- | --- |\n| 1   | 2   |"
        );
        // From the last cell: the first of the next row, skipping the delimiter row.
        assert_eq!(
            tab("| a | bb¦ |\n| --- | --- |\n| 1 | 2 |", false).unwrap(),
            "| a   | bb  |\n| --- | --- |\n| ¦1   | 2   |"
        );
        // Already lined up: only the caret moves.
        assert_eq!(
            tab("| a¦   | b   |\n| --- | --- |\n| 1   | 2   |", false).unwrap(),
            "| a   | ¦b   |\n| --- | --- |\n| 1   | 2   |"
        );
    }

    #[test]
    fn tab_in_the_last_cell_adds_a_row() {
        assert_eq!(
            tab("| a | b |\n| --- | --- |\n| 1 | 2¦ |", false).unwrap(),
            "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n| ¦    |     |"
        );
        // From the header of a table with no body.
        assert_eq!(
            tab("| a | b¦ |\n| --- | --- |", false).unwrap(),
            "| a   | b   |\n| --- | --- |\n| ¦    |     |"
        );
        // CRLF documents get CRLF rows.
        let (text, sels) = marked_parse("| a | b |\r\n| - | - |\r\n| 1 | 2¦ |", C);
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        next_cell(&mut buf, &mut hist, sels[0], false, "\r\n").unwrap();
        assert_eq!(
            buf.rope().to_string(),
            "| a   | b   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n|     |     |"
        );
    }

    #[test]
    fn shift_tab_goes_back() {
        assert_eq!(
            tab("| a | b |\n| - | - |\n| ¦1 | 2 |", true).unwrap(),
            "| a   | ¦b   |\n| --- | --- |\n| 1   | 2   |"
        );
        assert_eq!(
            tab("| a | b |\n| - | - |\n| 1 | ¦2 |", true).unwrap(),
            "| a   | b   |\n| --- | --- |\n| ¦1   | 2   |"
        );
        // From the header's first cell: stays.
        assert!(
            tab("| ¦a | b |\n| - | - |", true)
                .unwrap()
                .contains("| ¦a ")
        );
    }

    #[test]
    fn tab_is_left_alone_outside_a_table_or_over_lines() {
        assert!(tab("plain ¦text", false).is_none());
        assert!(tab("a | b¦\nc | d", false).is_none());
        // A selection across lines.
        let (text, _) = marked_parse("| a | b |\n| - | - |\n| 1 | 2 |", C);
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        let sel = Selection {
            anchor: 2,
            caret: 24,
            goal_col: None,
        };
        assert!(next_cell(&mut buf, &mut hist, sel, false, "\n").is_none());
        assert!(!hist.can_undo());
    }

    #[test]
    fn ragged_rows_get_their_cells() {
        // A short row: Tab from its last cell adds the missing one.
        assert_eq!(
            tab("| a | b | c |\n| - | - | - |\n| 1¦ |", false).unwrap(),
            "| a   | b   | c   |\n| --- | --- | --- |\n| 1   | ¦    |"
        );
        // A long row keeps its extra cell.
        let r = tab("| a | b |\n| - | - |\n| 1 | 2¦ | 3 |", false).unwrap();
        assert!(r.contains("| 1   | 2   | ¦3   |"), "{r}");
    }

    #[test]
    fn escaped_pipes_code_wide_characters_and_alignment() {
        let r = tab("| a\\|b¦ | c |\n|:-:|--:|\n| 1 | 2 |", false).unwrap();
        assert_eq!(r, "| a\\|b |   ¦c |\n| :--: | --: |\n|  1   |   2 |");
        let r = tab("| `a|b`¦ | c |\n| - | - |", false).unwrap();
        assert!(r.starts_with("| `a|b` | ¦c   |"), "{r}");
        // Wide characters count 2 columns.
        let r = tab("| \u{6f22}\u{5b57}¦ | x |\n| - | - |\n| ab | y |", false).unwrap();
        assert!(r.contains("| \u{6f22}\u{5b57} | ¦x   |"), "{r}");
        assert!(r.contains("| ab   | y   |"), "{r}");
    }

    #[test]
    fn tables_in_quotes_and_indents_keep_their_prefix() {
        let r = tab("> | a¦ | b |\n> | - | - |\n> | 1 | 2 |", false).unwrap();
        assert_eq!(r, "> | a   | ¦b   |\n> | --- | --- |\n> | 1   | 2   |");
        let r = tab("  | a | b |\n  | - | - |\n  | 1 | 2¦ |", false).unwrap();
        assert!(
            r.ends_with("  |     |     |") || r.contains("  | ¦    "),
            "{r}"
        );
        assert!(r.lines().all(|l| l.starts_with("  |")));
    }

    #[test]
    fn enter_at_the_end_of_a_row_adds_a_row() {
        assert_eq!(
            enter("| a | b |\n| - | - |\n| 1 | 2 |¦\n\nafter").unwrap(),
            "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n| ¦    |     |\n\nafter"
        );
        // Between rows.
        let r = enter("| a | b |\n| - | - |\n| 1 | 2 |¦\n| 3 | 4 |").unwrap();
        assert!(
            r.contains("| 1   | 2   |\n| ¦    |     |\n| 3   | 4   |"),
            "{r}"
        );
        // From the header: the first body row.
        let r = enter("| a | b |¦\n| - | - |\n| 1 | 2 |").unwrap();
        assert!(
            r.contains("| --- | --- |\n| ¦    |     |\n| 1   | 2   |"),
            "{r}"
        );
        // Without trailing pipes the end of the row is the end of the text.
        assert!(enter("a | b\n- | -\n1 | 2¦").is_some());
    }

    #[test]
    fn enter_elsewhere_is_the_shells() {
        assert!(enter("| a | b |\n| - | - |\n| 1 |¦ 2 |").is_none());
        assert!(enter("| a | b |\n| - | - |\n| 1 | 2 ¦| ").is_none());
        assert!(enter("text¦").is_none());
        // With a selection.
        let (text, _) = marked_parse("| a | b |\n| - | - |\n| 1 | 2 |", C);
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        let sel = Selection {
            anchor: 20,
            caret: text.len(),
            goal_col: None,
        };
        assert!(row_after(&mut buf, &mut hist, sel, "\n").is_none());
    }

    #[test]
    fn enter_at_the_end_of_an_empty_row_in_the_middle_adds_a_row() {
        let r = enter(
            "| a | b |
| - | - |
|   |   |¦
| 3 | 4 |",
        )
        .unwrap();
        assert_eq!(
            r,
            "| a   | b   |
| --- | --- |
|     |     |
| ¦    |     |
| 3   | 4   |"
        );
    }

    #[test]
    fn enter_at_the_end_of_an_empty_row_ends_the_table() {
        let r = enter("| a | b |\n| - | - |\n| 1 | 2 |\n|   |   |¦").unwrap();
        assert_eq!(r, "| a | b |\n| - | - |\n| 1 | 2 |\n¦");
        let r = enter("> | a | b |\n> | - | - |\n> |   |   |¦").unwrap();
        assert_eq!(r, "> | a | b |\n> | - | - |\n> ¦");
    }

    #[test]
    fn rows() {
        let t = "| a | b |\n| - | - |\n| 1¦ | 2 |\n| 3 | 4 |";
        let r = op(t, TableOp::RowBelow).unwrap();
        assert_eq!(
            r,
            "| a   | b   |\n| --- | --- |\n| 1   | 2   |\n| ¦    |     |\n| 3   | 4   |"
        );
        let r = op(t, TableOp::RowAbove).unwrap();
        assert_eq!(
            r,
            "| a   | b   |\n| --- | --- |\n| ¦    |     |\n| 1   | 2   |\n| 3   | 4   |"
        );
        let r = op(t, TableOp::DeleteRow).unwrap();
        assert_eq!(r, "| a   | b   |\n| --- | --- |\n| ¦3   | 4   |");
        // The last row: the caret goes up.
        let r = op(
            "| a | b |\n| - | - |\n| 1 | 2 |\n| 3¦ | 4 |",
            TableOp::DeleteRow,
        )
        .unwrap();
        assert!(r.ends_with("| ¦1   | 2   |"), "{r}");
        // Header and delimiter rows are not deleted; nothing above the header.
        assert!(op("| a¦ | b |\n| - | - |\n| 1 | 2 |", TableOp::DeleteRow).is_none());
        assert!(op("| a | b |\n| -¦ | - |\n| 1 | 2 |", TableOp::RowAbove).is_none());
        // Below the header: the first body row.
        let r = op("| a¦ | b |\n| - | - |\n| 1 | 2 |", TableOp::RowBelow).unwrap();
        assert!(r.contains("| --- | --- |\n| ¦    |     |\n| 1   |"), "{r}");
        // Deleting the only body row leaves the header and delimiter.
        let r = op("| a | b |\n| - | - |\n| 1¦ | 2 |", TableOp::DeleteRow).unwrap();
        assert_eq!(r, "| ¦a   | b   |\n| --- | --- |");
    }

    #[test]
    fn columns() {
        let t = "| a | b |\n|:-:| -: |\n| 1 | 2¦ |";
        let r = op(t, TableOp::ColumnRight).unwrap();
        assert_eq!(
            r,
            "|  a  |   b |     |\n| :-: | --: | --- |\n|  1  |   2 | ¦    |"
        );
        let r = op(t, TableOp::ColumnLeft).unwrap();
        assert_eq!(
            r,
            "|  a  |     |   b |\n| :-: | --- | --: |\n|  1  | ¦    |   2 |"
        );
        let r = op(t, TableOp::DeleteColumn).unwrap();
        assert_eq!(r, "|  a  |\n| :-: |\n|  ¦1  |");
        // One column stays.
        assert!(op("| a¦ |\n| - |", TableOp::DeleteColumn).is_none());
        // Moving keeps the caret with its text.
        let r = op("| a | b |\n|:-|-:|\n| 1 | 2¦ |", TableOp::MoveColumnLeft).unwrap();
        assert_eq!(r, "|   b | a   |\n| --: | :-- |\n|   2¦ | 1   |");
        let r = op("| a | b |\n|:-|-:|\n| 1¦ | 2 |", TableOp::MoveColumnRight).unwrap();
        assert_eq!(r, "|   b | a   |\n| --: | :-- |\n|   2 | 1¦   |");
        assert!(op("| a¦ | b |\n| - | - |", TableOp::MoveColumnLeft).is_none());
        assert!(op("| a | b¦ |\n| - | - |", TableOp::MoveColumnRight).is_none());
        // Outside a table.
        assert!(op("text¦", TableOp::ColumnLeft).is_none());
    }

    #[test]
    fn column_commands_on_ragged_rows() {
        let r = op(
            "| a | b | c |\n| - | - | - |\n| 1¦ |\n| 3 | 4 | 5 |",
            TableOp::MoveColumnRight,
        )
        .unwrap();
        assert!(
            r.contains("| ¦    |     |") || r.contains("|     | 1¦"),
            "{r}"
        );
        let r = op(
            "| a | b | c |\n| - | - | - |\n| 1 |\n| 3 | 4¦ | 5 |",
            TableOp::DeleteColumn,
        )
        .unwrap();
        assert!(r.contains("| 1   |     |") || r.contains("| 1   |"), "{r}");
    }

    #[test]
    fn a_table_starting_a_list_item_keeps_its_marker_and_indentation() {
        // Tab lines the table up under the marker and moves on.
        assert_eq!(
            tab("- | a¦ | bb |\n  | - | - |\n  | 1 | 2 |", false).unwrap(),
            "- | a   | ¦bb  |\n  | --- | --- |\n  | 1   | 2   |"
        );
        // Tab in the last cell of the last row adds a row indented like the others.
        assert_eq!(
            tab("- | a | b |\n  | --- | --- |\n  | 1 | 2¦ |", false).unwrap(),
            "- | a   | b   |\n  | --- | --- |\n  | 1   | 2   |\n  | ¦    |     |"
        );
        // Enter at the end of the header gives the first body row.
        assert_eq!(
            enter("1. | a | b |¦\n   | --- | --- |").unwrap(),
            "1. | a   | b   |\n   | --- | --- |\n   | ¦    |     |"
        );
        // Shift+Tab back from the first body cell goes to the header's last cell.
        assert_eq!(
            tab("- | a | b |\n  | --- | --- |\n  | ¦1 | 2 |", true).unwrap(),
            "- | a   | ¦b   |\n  | --- | --- |\n  | 1   | 2   |"
        );
        // Row and column commands, in a quote too.
        assert_eq!(
            op(
                "> * | a | b |\n>   | --- | --- |\n>   | 1¦ | 2 |",
                TableOp::ColumnRight
            )
            .unwrap(),
            "> * | a   |     | b   |\n>   | --- | --- | --- |\n>   | 1   | ¦    | 2   |"
        );
        assert_eq!(
            op("- | a¦ | b |\n  | --- | --- |", TableOp::RowBelow).unwrap(),
            "- | a   | b   |\n  | --- | --- |\n  | ¦    |     |"
        );
        // A table whose rows are indented further than the marker keeps that indentation.
        assert_eq!(
            tab("- | a¦ | b |\n    | --- | --- |", false).unwrap(),
            "- | a   | ¦b   |\n    | --- | --- |"
        );
        // A second list item is not part of the table.
        assert!(
            op(
                "- | a | b |\n  | --- | --- |\n  | 1 | 2 |\n- | x¦ | y |",
                TableOp::RowBelow
            )
            .is_none()
        );
        // Without a delimiter row it is only a list item.
        assert!(tab("- | a¦ | b |\n  | 1 | 2 |", false).is_none());
    }

    #[test]
    fn align_table_works_in_a_list_item() {
        let (text, sels) = marked_parse("- | a¦ | bb |\n  | - | - |\n  | 1 | 2 |\n", C);
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        let out = crate::markdown::format_table(&mut buf, &mut hist, sels[0]);
        assert_eq!(
            marked_show(&buf.rope().to_string(), &[out.sel], C),
            "- | a¦   | bb  |\n  | --- | --- |\n  | 1   | 2   |\n"
        );
        assert_eq!(hist.steps(), (1, 0));
    }

    #[test]
    fn body_rows_move_up_and_down_but_not_past_the_header() {
        let t = "| h |\n| - |\n| a |\n| b¦ |\n| c |";
        assert_eq!(
            op(t, TableOp::MoveRowUp).unwrap(),
            "| h   |\n| --- |\n| b¦   |\n| a   |\n| c   |"
        );
        assert_eq!(
            op(t, TableOp::MoveRowDown).unwrap(),
            "| h   |\n| --- |\n| a   |\n| c   |\n| b¦   |"
        );
        // The first body row cannot go up, the last cannot go down, the header cannot move.
        assert!(op("| h |\n| - |\n| a¦ |\n| b |", TableOp::MoveRowUp).is_none());
        assert!(op("| h |\n| - |\n| a |\n| b¦ |", TableOp::MoveRowDown).is_none());
        assert!(op("| h¦ |\n| - |\n| a |", TableOp::MoveRowDown).is_none());
        // The last row has no line break: the row that comes to be last has none either.
        assert_eq!(
            op("| h |\n| - |\n| a |\n| b¦ |", TableOp::MoveRowUp).unwrap(),
            "| h   |\n| --- |\n| b¦   |\n| a   |"
        );
    }

    #[test]
    fn sorting_a_column_goes_up_then_down_and_compares_numbers_as_numbers() {
        // Numbers: 9, 10, 100 (as text it would be 10, 100, 9).
        let up = op(
            "| n | s |\n| - | - |\n| 10¦ | b |\n| 9 | C |\n| 100 | a |",
            TableOp::SortColumn,
        )
        .unwrap();
        assert_eq!(
            up,
            "| n   | s   |\n| --- | --- |\n| 9   | C   |\n| 10¦  | b   |\n| 100 | a   |"
        );
        // Once ascending, the same command sorts descending.
        let down = op(
            "| n | s |\n| - | - |\n| 9 | C |\n| 10¦ | b |\n| 100 | a |",
            TableOp::SortColumn,
        )
        .unwrap();
        assert_eq!(
            down,
            "| n   | s   |\n| --- | --- |\n| 100 | a   |\n| 10¦  | b   |\n| 9   | C   |"
        );
        // Text compares without regard to case, and equal rows keep their order.
        let text = op(
            "| n | s¦ |\n| - | - |\n| 1 | b |\n| 2 | C |\n| 3 | a |\n| 4 | B |",
            TableOp::SortColumn,
        )
        .unwrap();
        assert_eq!(
            text,
            "| n   | s¦   |\n| --- | --- |\n| 3   | a   |\n| 1   | b   |\n| 4   | B   |\n| 2   | C   |"
        );
        // One cell that is not a number makes the column text: "10" before "9".
        let mixed = op("| n |\n| - |\n| 9¦ |\n| 10 |\n| x |", TableOp::SortColumn).unwrap();
        assert_eq!(mixed, "| n   |\n| --- |\n| 10  |\n| 9¦   |\n| x   |");
        // With a header only, or one body row, there is nothing to sort.
        assert!(op("| n¦ |\n| - |\n| 1 |", TableOp::SortColumn).is_none());
        // In a table in a list item the marker stays.
        let list = op("- | n¦ |\n  | - |\n  | 2 |\n  | 1 |", TableOp::SortColumn).unwrap();
        assert_eq!(list, "- | n¦   |\n  | --- |\n  | 1   |\n  | 2   |");
    }

    #[test]
    fn limits_are_kept() {
        let cols = TABLE_COLS;
        let row = |c: &str| format!("|{}\n", format!("{c}|").repeat(cols));
        let text = format!("{}{}{}", row("a"), row("-"), row("b"));
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::new();
        let sel = Selection::at(2);
        assert!(edit(&mut buf, &mut hist, sel, TableOp::ColumnRight, "\n").is_none());
        assert!(!hist.can_undo());
    }

    #[test]
    fn hostile_input_never_panics_and_one_step_each() {
        let mut rng = Rng::new(7);
        let bits = [
            "|", "a", " ", "-", ":", "\n", "\\|", "`", "> ", "\u{6f22}", "  ", "\t", "||", "- ",
            "1. ", "* | ", "\r\n",
        ];
        for _ in 0..3000 {
            let mut s = String::new();
            for _ in 0..rng.below(40) {
                s.push_str(rng.pick(&bits));
            }
            let mut buf = Buffer::from_text(&s);
            let at = buf.floor_char(rng.below(s.len() + 1));
            let sel = Selection::at(at);
            let mut hist = History::new();
            let ops = [
                TableOp::RowBelow,
                TableOp::RowAbove,
                TableOp::DeleteRow,
                TableOp::ColumnRight,
                TableOp::ColumnLeft,
                TableOp::DeleteColumn,
                TableOp::MoveColumnLeft,
                TableOp::MoveColumnRight,
                TableOp::MoveRowUp,
                TableOp::MoveRowDown,
                TableOp::SortColumn,
            ];
            let which = *rng.pick(&ops);
            let before = buf.rope().to_string();
            let out = match rng.below(4) {
                0 => next_cell(&mut buf, &mut hist, sel, rng.chance(50), "\n"),
                1 => row_after(&mut buf, &mut hist, sel, "\n"),
                _ => edit(&mut buf, &mut hist, sel, which, "\n"),
            };
            if let Some(out) = out {
                let after = buf.rope().to_string();
                assert!(
                    buf.is_char_boundary(out.sel.caret.min(after.len())),
                    "{s:?}"
                );
                assert!(out.sel.caret <= after.len(), "{s:?}");
                if let Some(c) = &out.change {
                    check_change(&before, &after, c);
                    assert_eq!(hist.steps(), (1, 0));
                    undo(&mut buf, &mut hist).unwrap();
                    assert_eq!(buf.rope().to_string(), before);
                } else {
                    assert_eq!(after, before);
                }
            }
        }
    }
}
