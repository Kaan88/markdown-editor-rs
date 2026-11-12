//! The selection and caret movement, in buffer byte offsets. Movement steps over grapheme
//! clusters, never into one; words follow the find box's classes (letters, digits and `_`;
//! whitespace; anything else). Up and Down keep a goal display column, so moving through a short
//! line and back returns to the same column.

use crate::buffer::Buffer;
use std::ops::Range;

/// `anchor` is where the selection started, `caret` where it ends and the caret is drawn. Both
/// are grapheme boundaries. Equal means no selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Selection {
    pub anchor: usize,
    pub caret: usize,
    /// The display column Up and Down aim for; set by the first vertical move, cleared by any
    /// other motion or click.
    pub goal_col: Option<usize>,
}

impl Selection {
    pub fn at(offset: usize) -> Selection {
        Selection {
            anchor: offset,
            caret: offset,
            goal_col: None,
        }
    }

    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.caret
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    /// Smart Home: to the line's first non-blank character, or to column 0 when already there.
    Home,
    End,
    Up(usize),
    Down(usize),
    DocStart,
    DocEnd,
}

/// The selection after a motion. With `extend` the anchor stays; without it, Left and Right on a
/// non-empty selection collapse it to its start or end instead of moving.
pub fn apply(
    buf: &Buffer,
    sel: Selection,
    motion: Motion,
    extend: bool,
    tab_width: usize,
) -> Selection {
    let _ = (buf, sel, motion, extend, tab_width);
    todo!()
}

/// The word, run of whitespace, or single other grapheme at `offset` (double click). At a line's
/// end it is the empty range there.
pub fn word_at(buf: &Buffer, offset: usize) -> Range<usize> {
    let _ = (buf, offset);
    todo!()
}

/// The line holding `offset` with its line break (triple click).
pub fn line_at(buf: &Buffer, offset: usize) -> Range<usize> {
    let _ = (buf, offset);
    todo!()
}

pub fn select_all(buf: &Buffer) -> Selection {
    let _ = buf;
    todo!()
}

/// A drag selection: from the press's anchor range to the pointer's offset, growing by words
/// (after a double click) or lines (after a triple click) as the press did.
pub fn drag_to(buf: &Buffer, origin: Range<usize>, unit: Unit, offset: usize) -> Selection {
    let _ = (buf, origin, unit, offset);
    todo!()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    Grapheme,
    Word,
    Line,
}
