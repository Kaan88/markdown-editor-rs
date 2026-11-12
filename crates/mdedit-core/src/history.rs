//! Undo and redo. An undo step is the list of replacements one command made, in the order it made
//! them, with the selection before and after. Each record keeps the text it removed (the text the
//! buffer no longer has) and how many bytes it inserted; the inserted text itself is not kept,
//! because while the step is undoable it is in the buffer, and undoing takes it from there. So
//! the history of a long typing session holds almost no text, and a large removal holds a `Rope`
//! slice that shares the buffer's chunks instead of a copy.
//!
//! Undoing flips every record (the text it takes out of the buffer becomes the record's removed
//! text), so the same step, moved to the redo list, redoes. `edit.rs` makes and applies steps;
//! this module decides what merges into one step and whether the text is at the save point.

use crate::limits::{UNDO_BYTES, UNDO_SHARE_BYTES};
use crate::selection::Selection;
use ropey::Rope;
use std::collections::VecDeque;
use std::ops::Range;

/// What an edit was, for grouping into undo steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One typed character at the caret.
    Typing,
    /// Backspace over one grapheme.
    Backspace,
    /// Delete over one grapheme.
    Delete,
    /// Anything else: always its own step.
    Other,
}

/// Text taken out of the buffer: a `String` when small, else a `Rope` sharing the buffer's
/// chunks (ropey makes a rope of a slice in O(log n) without copying them).
#[derive(Debug)]
pub(crate) enum Text {
    Small(String),
    Large(Rope),
}

impl Text {
    pub(crate) fn empty() -> Text {
        Text::Small(String::new())
    }

    /// The text of `range` of `rope` (char boundaries).
    pub(crate) fn of(rope: &Rope, range: Range<usize>) -> Text {
        let n = range.end - range.start;
        if n <= UNDO_SHARE_BYTES {
            let mut s = String::with_capacity(n);
            for piece in rope.byte_slice(range).chunks() {
                s.push_str(piece);
            }
            Text::Small(s)
        } else {
            Text::Large(Rope::from(rope.byte_slice(range)))
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Text::Small(s) => s.len(),
            Text::Large(r) => r.len_bytes(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One replacement: at `at`, `inserted` bytes now stand where `removed` was.
#[derive(Debug)]
pub(crate) struct Record {
    pub at: usize,
    pub removed: Text,
    pub inserted: usize,
    /// For a typed character: it is whitespace.
    pub ws: bool,
}

#[derive(Debug)]
pub(crate) struct Step {
    /// In the order the replacements were made. Undo applies them from the last.
    pub records: Vec<Record>,
    pub sel_before: Selection,
    pub sel_after: Selection,
    kind: Kind,
    /// Where the next edit must be to merge: the end of the typing; the start of the last
    /// removal.
    pos: usize,
    /// The step's typing ends in whitespace.
    ends_ws: bool,
    /// Identifies the state of the text after this step, for the save point.
    id: u64,
    bytes: usize,
}

impl Step {
    /// Replaces the records, as undo or redo does after flipping them.
    pub(crate) fn set_records(&mut self, records: Vec<Record>) {
        self.bytes = records.iter().map(|r| r.removed.len()).sum();
        self.records = records;
    }

    /// Merges the one-record edit `rec` into this step when the kind and positions allow it; on
    /// success `rec` is left as it was or emptied.
    fn try_merge(&mut self, kind: Kind, rec: &mut Option<Record>, sel_after: Selection) -> bool {
        let Some(r) = rec.as_ref() else {
            return false;
        };
        if self.kind != kind {
            return false;
        }
        let (at, len, inserted, ws) = (r.at, r.removed.len(), r.inserted, r.ws);
        let Some(last) = self.records.last_mut() else {
            return false;
        };
        match kind {
            Kind::Typing => {
                if !last.removed.is_empty() || len != 0 || at != self.pos || (self.ends_ws && !ws) {
                    return false;
                }
                last.inserted += inserted;
                self.pos = at + inserted;
                self.ends_ws = ws;
            }
            Kind::Backspace | Kind::Delete => {
                let back = kind == Kind::Backspace;
                let contiguous = last.inserted == 0
                    && inserted == 0
                    && if back {
                        at + len == self.pos
                    } else {
                        at == self.pos
                    };
                if !contiguous {
                    return false;
                }
                let join = match (&last.removed, &r.removed) {
                    (Text::Small(a), Text::Small(b)) => a.len() + b.len() <= UNDO_SHARE_BYTES,
                    _ => false,
                };
                if join {
                    if let (
                        Text::Small(a),
                        Some(Record {
                            removed: Text::Small(b),
                            ..
                        }),
                    ) = (&mut last.removed, rec.take())
                    {
                        if back {
                            a.insert_str(0, &b);
                            last.at = at;
                        } else {
                            a.push_str(&b);
                        }
                    }
                } else {
                    self.records.extend(rec.take());
                }
                self.pos = at;
                self.bytes += len;
            }
            Kind::Other => return false,
        }
        self.sel_after = sel_after;
        true
    }
}

/// The undo and redo steps of one document, and where the save point is.
#[derive(Debug)]
pub struct History {
    /// Oldest first. Over the cap, steps leave from the front.
    undo: VecDeque<Step>,
    /// The last step undone is the last element.
    redo: Vec<Step>,
    /// The next edit may merge into the newest undo step.
    open: bool,
    /// Text held by undo and redo steps.
    bytes: usize,
    cap: usize,
    next_id: u64,
    /// The state before the oldest undo step: 0 at the start, the dropped step's id after.
    base_id: u64,
    /// The state at the save point, if it can still be reached.
    saved: Option<u64>,
}

impl Default for History {
    fn default() -> History {
        History::new()
    }
}

impl History {
    pub fn new() -> History {
        History::with_cap(UNDO_BYTES)
    }

    pub(crate) fn with_cap(cap: usize) -> History {
        History {
            undo: VecDeque::new(),
            redo: Vec::new(),
            open: false,
            bytes: 0,
            cap,
            next_id: 1,
            base_id: 0,
            saved: Some(0),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The id of the current state: the newest undo step's, or the base.
    fn state(&self) -> u64 {
        self.undo.back().map_or(self.base_id, |s| s.id)
    }

    /// The text differs from the save point: the state at `new()` or at the last
    /// [`History::mark_saved`], reached again by undoing or redoing back to it. Once the save
    /// point's state cannot be reached (a new edit cleared the redo steps holding it, or the
    /// steps before it were dropped for memory), the text counts as modified until the next
    /// `mark_saved`.
    pub fn is_modified(&self) -> bool {
        self.saved != Some(self.state())
    }

    /// Records the current state as the save point. Seals, so typing after a save is a new step.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.state());
        self.open = false;
    }

    /// The next edit starts a new undo step: the shell calls this on caret moves, clicks, focus
    /// loss and pauses in typing.
    pub fn seal(&mut self) {
        self.open = false;
    }

    /// Bytes of text held by undo and redo steps (what edits removed; typed text is in the
    /// buffer).
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Forgets every step, after a reload; the save point becomes the current state.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.open = false;
        self.bytes = 0;
        self.base_id = self.next_id;
        self.next_id += 1;
        self.saved = Some(self.base_id);
    }

    /// Number of undo and redo steps.
    #[cfg(test)]
    pub(crate) fn steps(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }

    /// Records the replacements of one command as a step, or merges them into the newest step
    /// when `kind` and the positions allow it. Clears the redo steps.
    pub(crate) fn commit(
        &mut self,
        kind: Kind,
        sel_before: Selection,
        sel_after: Selection,
        mut records: Vec<Record>,
    ) {
        if records.is_empty() {
            return;
        }
        self.clear_redo();
        if self.open
            && kind != Kind::Other
            && records.len() == 1
            && let Some(top) = self.undo.back_mut()
        {
            let before = top.bytes;
            let mut rec = records.pop();
            if top.try_merge(kind, &mut rec, sel_after) {
                self.bytes += top.bytes - before;
                self.enforce_cap();
                return;
            }
            records.extend(rec);
        }
        let (pos, ends_ws) = match (kind, records.last()) {
            (Kind::Typing, Some(r)) => (r.at + r.inserted, r.ws),
            (_, Some(r)) => (r.at, false),
            _ => (0, false),
        };
        let mut step = Step {
            records: Vec::new(),
            sel_before,
            sel_after,
            kind,
            pos,
            ends_ws,
            id: self.next_id,
            bytes: 0,
        };
        self.next_id += 1;
        step.set_records(records);
        self.bytes += step.bytes;
        self.undo.push_back(step);
        self.open = kind != Kind::Other;
        self.enforce_cap();
    }

    fn clear_redo(&mut self) {
        for s in self.redo.drain(..) {
            self.bytes -= s.bytes;
        }
    }

    /// Drops the oldest undo steps while over the cap, never the newest.
    fn enforce_cap(&mut self) {
        while self.bytes > self.cap && self.undo.len() > 1 {
            if let Some(s) = self.undo.pop_front() {
                self.bytes -= s.bytes;
                self.base_id = s.id;
            }
        }
    }

    pub(crate) fn pop_undo(&mut self) -> Option<Step> {
        let s = self.undo.pop_back()?;
        self.bytes -= s.bytes;
        self.open = false;
        Some(s)
    }

    pub(crate) fn push_redo(&mut self, step: Step) {
        self.bytes += step.bytes;
        self.redo.push(step);
    }

    pub(crate) fn pop_redo(&mut self) -> Option<Step> {
        let s = self.redo.pop()?;
        self.bytes -= s.bytes;
        self.open = false;
        Some(s)
    }

    pub(crate) fn push_undo(&mut self, step: Step) {
        self.bytes += step.bytes;
        self.undo.push_back(step);
        self.open = false;
        self.enforce_cap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;
    use crate::edit::{Outcome, backspace, delete, delete_word_left, insert, newline, redo, undo};

    fn setup(text: &str) -> (Buffer, History) {
        (Buffer::from_text(text), History::new())
    }

    fn txt(buf: &Buffer) -> String {
        buf.rope().to_string()
    }

    /// Types `s` one character at a time at the caret `sel`, returning the last selection.
    fn type_str(buf: &mut Buffer, hist: &mut History, mut sel: Selection, s: &str) -> Selection {
        for c in s.chars() {
            sel = insert(buf, hist, sel, &c.to_string()).sel;
        }
        sel
    }

    #[test]
    fn typing_merges_into_one_step() {
        let (mut buf, mut hist) = setup("");
        let sel = type_str(&mut buf, &mut hist, Selection::at(0), "hello");
        assert_eq!(hist.steps(), (1, 0));
        assert_eq!(sel, Selection::at(5));
        let o = undo(&mut buf, &mut hist).unwrap();
        assert_eq!(txt(&buf), "");
        assert_eq!(o.sel, Selection::at(0));
        let o = redo(&mut buf, &mut hist).unwrap();
        assert_eq!(txt(&buf), "hello");
        assert_eq!(o.sel, Selection::at(5));
        assert_eq!(hist.steps(), (1, 0));
    }

    #[test]
    fn typing_after_whitespace_starts_a_new_step() {
        // Notepad's rule: "ab " is one step, "cd" the next (a step ending in whitespace takes no
        // non-whitespace character), and "cd " goes on with them.
        let (mut buf, mut hist) = setup("");
        type_str(&mut buf, &mut hist, Selection::at(0), "ab cd ef");
        assert_eq!(hist.steps().0, 3);
        undo(&mut buf, &mut hist);
        assert_eq!(txt(&buf), "ab cd ");
        undo(&mut buf, &mut hist);
        assert_eq!(txt(&buf), "ab ");
        undo(&mut buf, &mut hist);
        assert_eq!(txt(&buf), "");
        // Whitespace after whitespace stays in the step.
        let (mut buf, mut hist) = setup("");
        type_str(&mut buf, &mut hist, Selection::at(0), "a   b");
        assert_eq!(hist.steps().0, 2);
    }

    #[test]
    fn typing_elsewhere_does_not_merge() {
        let (mut buf, mut hist) = setup("abc");
        let s = insert(&mut buf, &mut hist, Selection::at(0), "x").sel;
        insert(&mut buf, &mut hist, Selection::at(3), "y");
        assert_eq!(hist.steps().0, 2);
        // The first caret is where the second typing would have continued: still merges.
        let (mut buf, mut hist) = setup("abc");
        let s2 = insert(&mut buf, &mut hist, Selection::at(0), "x").sel;
        insert(&mut buf, &mut hist, s2, "y");
        assert_eq!(hist.steps().0, 1);
        assert_eq!(s, Selection::at(1));
    }

    #[test]
    fn seal_ends_merging() {
        let (mut buf, mut hist) = setup("");
        let s = type_str(&mut buf, &mut hist, Selection::at(0), "ab");
        hist.seal();
        type_str(&mut buf, &mut hist, s, "cd");
        assert_eq!(hist.steps().0, 2);
        undo(&mut buf, &mut hist);
        assert_eq!(txt(&buf), "ab");
    }

    #[test]
    fn multi_character_insert_and_replacement_are_their_own_steps() {
        let (mut buf, mut hist) = setup("");
        let s = type_str(&mut buf, &mut hist, Selection::at(0), "a");
        let s = insert(&mut buf, &mut hist, s, "bc").sel; // two graphemes: Other
        type_str(&mut buf, &mut hist, s, "d");
        assert_eq!(hist.steps().0, 3);
        // A character typed over a selection is a replacement: Other.
        let (mut buf, mut hist) = setup("abc");
        let s = type_str(&mut buf, &mut hist, Selection::at(3), "d");
        let sel = Selection {
            anchor: 0,
            caret: 2,
            goal_col: None,
        };
        insert(&mut buf, &mut hist, sel, "x");
        type_str(&mut buf, &mut hist, Selection::at(1), "y");
        assert_eq!(hist.steps().0, 3);
        assert_eq!(s, Selection::at(4));
        // A typed line break is Other too.
        let (mut buf, mut hist) = setup("");
        let s = type_str(&mut buf, &mut hist, Selection::at(0), "a");
        let s = insert(&mut buf, &mut hist, s, "\n").sel;
        type_str(&mut buf, &mut hist, s, "b");
        assert_eq!(hist.steps().0, 3);
    }

    #[test]
    fn backspaces_merge_while_contiguous() {
        let (mut buf, mut hist) = setup("abcdef");
        let mut sel = Selection::at(6);
        for _ in 0..3 {
            sel = backspace(&mut buf, &mut hist, sel).sel;
        }
        assert_eq!(txt(&buf), "abc");
        assert_eq!(hist.steps().0, 1);
        // Backspacing somewhere else is a new step.
        backspace(&mut buf, &mut hist, Selection::at(1));
        assert_eq!(hist.steps().0, 2);
        let o = undo(&mut buf, &mut hist).unwrap();
        assert_eq!(txt(&buf), "abc");
        assert_eq!(o.sel, Selection::at(1));
        let o = undo(&mut buf, &mut hist).unwrap();
        assert_eq!(txt(&buf), "abcdef");
        assert_eq!(o.sel, Selection::at(6));
        redo(&mut buf, &mut hist);
        assert_eq!(txt(&buf), "abc");
    }

    #[test]
    fn deletes_merge_while_contiguous() {
        let (mut buf, mut hist) = setup("a漢e\u{301}d\r\nf");
        let sel = Selection::at(1);
        for _ in 0..4 {
            delete(&mut buf, &mut hist, sel);
        }
        assert_eq!(txt(&buf), "af");
        assert_eq!(hist.steps().0, 1);
        // Backspace after delete: a different kind, a new step.
        backspace(&mut buf, &mut hist, sel);
        assert_eq!(hist.steps().0, 2);
        undo(&mut buf, &mut hist);
        undo(&mut buf, &mut hist);
        assert_eq!(txt(&buf), "a漢e\u{301}d\r\nf");
    }

    #[test]
    fn typing_then_backspacing_are_separate_steps() {
        let (mut buf, mut hist) = setup("");
        let s = type_str(&mut buf, &mut hist, Selection::at(0), "abc");
        let s = backspace(&mut buf, &mut hist, s).sel;
        backspace(&mut buf, &mut hist, s);
        assert_eq!(hist.steps().0, 2);
    }

    #[test]
    fn other_edits_are_always_their_own_step() {
        let (mut buf, mut hist) = setup("one two three");
        let mut s = Selection::at(13);
        s = delete_word_left(&mut buf, &mut hist, s).sel;
        s = delete_word_left(&mut buf, &mut hist, s).sel;
        assert_eq!(hist.steps().0, 2);
        s = newline(&mut buf, &mut hist, s, "\n", false).sel;
        newline(&mut buf, &mut hist, s, "\n", false);
        assert_eq!(hist.steps().0, 4);
    }

    #[test]
    fn a_new_edit_clears_the_redo_steps() {
        let (mut buf, mut hist) = setup("");
        let s = type_str(&mut buf, &mut hist, Selection::at(0), "ab");
        hist.seal();
        type_str(&mut buf, &mut hist, s, "cd");
        undo(&mut buf, &mut hist);
        assert!(hist.can_redo());
        type_str(&mut buf, &mut hist, Selection::at(2), "x");
        assert!(!hist.can_redo());
        assert!(redo(&mut buf, &mut hist).is_none());
        assert_eq!(txt(&buf), "abx");
        // Undo does not merge back into the step it left: the typing is a new step.
        assert_eq!(hist.steps(), (2, 0));
    }

    #[test]
    fn an_edit_that_changes_nothing_records_nothing() {
        let (mut buf, mut hist) = setup("ab");
        let o = insert(&mut buf, &mut hist, Selection::at(1), "");
        assert_eq!(
            o,
            Outcome {
                sel: Selection::at(1),
                change: None
            }
        );
        assert!(!hist.can_undo());
        assert!(!hist.is_modified());
    }

    #[test]
    fn is_modified_follows_the_save_point() {
        let (mut buf, mut hist) = setup("");
        assert!(!hist.is_modified());
        let s = type_str(&mut buf, &mut hist, Selection::at(0), "ab");
        assert!(hist.is_modified());
        undo(&mut buf, &mut hist);
        assert!(!hist.is_modified());
        redo(&mut buf, &mut hist);
        assert!(hist.is_modified());
        hist.mark_saved();
        assert!(!hist.is_modified());
        // Typing after a save is a new step, not part of the saved one.
        let s = type_str(&mut buf, &mut hist, s, "c");
        assert_eq!(hist.steps().0, 2);
        assert!(hist.is_modified());
        undo(&mut buf, &mut hist);
        assert!(!hist.is_modified());
        assert_eq!(txt(&buf), "ab");
        // Undo past the save point and back.
        undo(&mut buf, &mut hist);
        assert!(hist.is_modified());
        redo(&mut buf, &mut hist);
        assert!(!hist.is_modified());
        // A new edit after undoing past the save point loses it for good.
        undo(&mut buf, &mut hist);
        type_str(&mut buf, &mut hist, Selection::at(0), "z");
        assert!(hist.is_modified());
        undo(&mut buf, &mut hist);
        assert!(hist.is_modified());
        assert_eq!(txt(&buf), "");
        hist.mark_saved();
        assert!(!hist.is_modified());
        redo(&mut buf, &mut hist);
        assert!(hist.is_modified());
        let _ = s;
    }

    #[test]
    fn typing_then_deleting_it_by_a_new_edit_is_modified() {
        // The text is the same as at the save point, the history is not.
        let (mut buf, mut hist) = setup("");
        let s = type_str(&mut buf, &mut hist, Selection::at(0), "a");
        hist.seal();
        backspace(&mut buf, &mut hist, s);
        assert_eq!(txt(&buf), "");
        assert!(hist.is_modified());
        undo(&mut buf, &mut hist);
        undo(&mut buf, &mut hist);
        assert!(!hist.is_modified());
    }

    #[test]
    fn clear_forgets_everything_and_marks_the_present_saved() {
        let (mut buf, mut hist) = setup("");
        type_str(&mut buf, &mut hist, Selection::at(0), "abc");
        undo(&mut buf, &mut hist);
        type_str(&mut buf, &mut hist, Selection::at(0), "x");
        hist.clear();
        assert!(!hist.can_undo() && !hist.can_redo());
        assert!(!hist.is_modified());
        assert_eq!(hist.bytes(), 0);
        type_str(&mut buf, &mut hist, Selection::at(1), "y");
        assert!(hist.is_modified());
        undo(&mut buf, &mut hist);
        assert!(!hist.is_modified());
    }

    #[test]
    fn the_memory_cap_drops_the_oldest_steps_but_never_the_newest() {
        // Ten steps of 30 removed bytes each against a cap of 100.
        let text: String = (0..10).map(|i| format!("{i:0>29}\n")).collect();
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::with_cap(100);
        for _ in 0..10 {
            let sel = Selection {
                anchor: 0,
                caret: 30,
                goal_col: None,
            };
            backspace(&mut buf, &mut hist, sel);
            assert!(hist.bytes() <= 100, "{}", hist.bytes());
            assert!(hist.can_undo());
        }
        assert_eq!(buf.len_bytes(), 0);
        assert_eq!(hist.steps().0, 3);
        assert_eq!(hist.bytes(), 90);
        assert!(hist.is_modified());
        // What is left undoes; the dropped part cannot, and the text is never "saved" again.
        while undo(&mut buf, &mut hist).is_some() {}
        assert_eq!(buf.len_bytes(), 90);
        assert!(hist.is_modified());
        assert_eq!(
            hist.bytes(),
            0,
            "undone steps hold no removed text: it is in the buffer"
        );
        hist.mark_saved();
        assert!(!hist.is_modified());
        redo(&mut buf, &mut hist);
        assert!(hist.is_modified());

        // One step over the cap is kept.
        let mut buf = Buffer::from_text(&"x".repeat(500));
        let mut hist = History::with_cap(100);
        let sel = Selection {
            anchor: 0,
            caret: 400,
            goal_col: None,
        };
        backspace(&mut buf, &mut hist, sel);
        assert_eq!(hist.steps().0, 1);
        assert_eq!(hist.bytes(), 400);
        undo(&mut buf, &mut hist);
        assert_eq!(buf.len_bytes(), 500);
    }

    #[test]
    fn the_save_point_survives_the_drop_of_older_steps() {
        let text: String = (0..6).map(|i| format!("{i:0>29}\n")).collect();
        let mut buf = Buffer::from_text(&text);
        let mut hist = History::with_cap(100);
        let sel = Selection {
            anchor: 0,
            caret: 30,
            goal_col: None,
        };
        backspace(&mut buf, &mut hist, sel);
        backspace(&mut buf, &mut hist, sel);
        hist.mark_saved();
        for _ in 0..3 {
            backspace(&mut buf, &mut hist, sel);
        }
        // Steps 1 and 2 were dropped, and with them the way back before the save point; the
        // save point itself (after step 2) is the state under the oldest step left.
        assert_eq!(hist.steps().0, 3);
        assert!(hist.is_modified());
        while undo(&mut buf, &mut hist).is_some() {}
        assert!(!hist.is_modified());
    }

    #[test]
    fn a_large_removal_shares_the_buffers_chunks() {
        // 6 MiB of text; the middle 5 MiB removed in one edit.
        let line = "0123456789 abcdefghij ğüşiöç 漢字 e\u{301}\r\n";
        let mut text = String::new();
        while text.len() < 6 << 20 {
            text.push_str(line);
        }
        let mut buf = Buffer::from_text(&text);
        let original = buf.snapshot();
        let mut hist = History::new();
        let (a, b) = (
            buf.snap_to_grapheme(500_000),
            buf.snap_to_grapheme(500_000 + (5 << 20)),
        );
        let sel = Selection {
            anchor: a,
            caret: b,
            goal_col: None,
        };
        let o = backspace(&mut buf, &mut hist, sel);
        assert_eq!(buf.len_bytes(), text.len() - (b - a));
        assert_eq!(o.change.unwrap().removed, b - a);
        assert_eq!(hist.bytes(), b - a);
        // The kept text is a rope whose leaves are the buffer's own: count those the original
        // rope also has.
        let step = hist.undo.back().unwrap();
        let Text::Large(kept) = &step.records[0].removed else {
            panic!("a 5 MiB removal is kept as a rope");
        };
        let theirs: std::collections::HashSet<usize> =
            original.chunks().map(|c| c.as_ptr() as usize).collect();
        let (mut shared, mut total) = (0, 0);
        for c in kept.chunks() {
            total += 1;
            shared += usize::from(theirs.contains(&(c.as_ptr() as usize)));
        }
        assert!(total > 1000, "{total} chunks");
        assert!(
            shared * 100 >= total * 99,
            "{shared} of {total} chunks shared"
        );
        // Undo puts the text back exactly, and the restored text shares them too.
        undo(&mut buf, &mut hist).unwrap();
        assert_eq!(*buf.rope(), original);
        assert_eq!(txt(&buf), text);
        redo(&mut buf, &mut hist).unwrap();
        assert_eq!(buf.len_bytes(), text.len() - (b - a));
        assert_eq!(hist.bytes(), b - a);
    }
}
