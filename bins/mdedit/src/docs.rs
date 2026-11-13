//! Open documents (one per tab): load jobs, buffers, selection, scroll, find and colour state.
//!
//! A document is `Pending` until its tab is first shown (restored tabs load on activation),
//! `Loading` while its first load runs on a thread, then `Ready` or `Failed`. A reload keeps the
//! `Ready` text on screen until the new one arrives. Every job result carries the document id and
//! the revision it was started for (a counter per document, bumped on every load); results for
//! another revision are dropped.
//!
//! An edit goes through [`Docs::edit`]: it changes the buffer and its undo history, bumps the
//! revision, and keeps what hangs on the text in step (colours, find marks, line-ending counts,
//! the highlight worker's copy). A document with unsaved changes is never reloaded by itself.

use crate::app::Wake;
use crate::commands::strings;
use crate::session::{ThemeChoice, Unsaved};
use crate::view::cache::Lru;
use mdedit_core::display::ChunkIndex;
use mdedit_core::edit::{self, Outcome};
use mdedit_core::encoding::Encoding;
use mdedit_core::eol::normalize_eols;
use mdedit_core::find::{self, FindOptions, Matches, Query};
use mdedit_core::fold::{self, Folds};
use mdedit_core::limits::{FIND_CAP, MAX_FILE_BYTES, SYNC_FIND_BYTES, TAB_WIDTH};
use mdedit_core::lines;
use mdedit_core::load::{self, LoadError, LoadOptions, Loaded};
use mdedit_core::multi::{self, MultiOp};
use mdedit_core::replace;
use mdedit_core::save::{self, SaveError};
use mdedit_core::selection::Selection;
use mdedit_core::{Buffer, Change, FileInfo, History, Position};
use mdedit_highlight::{Colored, DocId, HighlightService, Language, Span};
use mdshared_app::files::{FileStamp, stamp};
use mdshared_app::paths::{canonical_or_absolute, same_path, tab_labels};
use mdshared_app::recents::Recents;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Wakes the event loop from a worker thread. A no-op until the loop exists, and in `--shot`.
pub type Waker = Arc<dyn Fn(Wake) + Send + Sync>;

/// The colours of a window of lines: the last `Colored` received for the document.
#[derive(Default)]
pub struct LineColors {
    pub first: usize,
    pub lines: Vec<Vec<Span>>,
    pub dark: bool,
    /// Bumped on every `Colored` taken in; 0 means none yet.
    pub generation: u64,
    /// The lines last asked of the service and the theme they were asked in.
    pub requested: Option<(Range<usize>, bool)>,
}

impl LineColors {
    /// The spans of `line` in the `dark` theme, if they are known. Empty means "foreground".
    pub fn line(&self, line: usize, dark: bool) -> Option<&[Span]> {
        if self.generation == 0 || self.dark != dark || line < self.first {
            return None;
        }
        self.lines.get(line - self.first).map(Vec::as_slice)
    }

    /// Keeps the window in step with an edit until the worker answers for the new text: lines
    /// below the change move with it, the edited line keeps its colours around the edit (the
    /// typed text takes the colour it lands in), other changed lines go plain. `rel` is the
    /// edit's start in bytes from the start of `change.first_line`.
    pub fn edited(&mut self, change: &Change, rel: usize) {
        self.requested = None;
        if self.generation == 0 {
            return;
        }
        let first = change.first_line;
        let end = self.first + self.lines.len();
        if first >= end {
            return;
        }
        if first < self.first {
            if first + change.lines_before <= self.first {
                self.first = self.first.saturating_add_signed(change.line_delta());
            } else {
                // The change runs into the window from above: nothing here is known to be right.
                self.lines.clear();
            }
            self.generation += 1;
            return;
        }
        let a = first - self.first;
        let b = (a + change.lines_before).min(self.lines.len());
        let mut head = std::mem::take(&mut self.lines[a]);
        let one_line = change.lines_before == 1 && change.lines_after == 1;
        let (removed, inserted) = if one_line {
            (change.removed, change.inserted)
        } else {
            (usize::MAX / 2, 0)
        };
        shift_spans(&mut head, rel, removed, inserted);
        let mut fresh = vec![Vec::new(); change.lines_after];
        fresh[0] = head;
        self.lines.splice(a..b, fresh);
        self.generation += 1;
    }

    /// Whether every line of `lines` has colours in `dark`.
    pub fn covers(&self, lines: &Range<usize>, dark: bool) -> bool {
        self.generation > 0
            && self.dark == dark
            && lines.start >= self.first
            && lines.end <= self.first + self.lines.len()
    }
}

/// Moves a line's colour spans for an edit at byte `rel` of the line that removed `removed`
/// bytes and inserted `inserted`. Spans stay sorted and contiguous; the span the edit starts in
/// (or, at the line's end, the last one) grows over the inserted text.
fn shift_spans(spans: &mut Vec<Span>, rel: usize, removed: usize, inserted: usize) {
    let old_end = spans.last().map_or(0, |s| s.end as usize);
    let cut = rel.saturating_add(removed);
    let start = |x: usize| {
        if x <= rel {
            x
        } else if x < cut {
            rel + inserted
        } else {
            x - removed + inserted
        }
    };
    let end = |x: usize| {
        if x <= rel {
            x
        } else if x <= cut {
            rel + inserted
        } else {
            x - removed + inserted
        }
    };
    for s in spans.iter_mut() {
        let (a, b) = (start(s.start as usize), end(s.end as usize));
        s.start = a as u32;
        s.end = b.max(a) as u32;
    }
    spans.retain(|s| s.start < s.end);
    if rel >= old_end
        && let Some(last) = spans.last_mut()
        && last.end as usize == rel
    {
        last.end = (rel + inserted) as u32;
    }
}

/// The find state of one document: what the matches were computed for and where the user is.
#[derive(Default)]
pub struct FindState {
    pub text: String,
    pub options: FindOptions,
    /// Why the query cannot be searched for (a regular expression that does not compile).
    pub error: Option<String>,
    pub matches: Vec<Range<usize>>,
    pub capped: bool,
    /// Index into `matches` of the current match.
    pub current: Option<usize>,
    /// A search thread is running for `text`.
    pub searching: bool,
    /// Bumped whenever the marks or the current match change, for the text region's signature.
    pub generation: u64,
    /// The text changed since the matches were found: they were moved with it, and a new search
    /// is due once typing pauses.
    pub stale: bool,
    cancel: Option<Arc<AtomicBool>>,
}

impl FindState {
    /// Moves the matches with an edit: those the edit touches go, later ones shift.
    fn edited(&mut self, change: &Change) {
        self.cancel();
        self.searching = false;
        if self.text.is_empty() {
            return;
        }
        self.stale = true;
        self.generation += 1;
        let (at, cut) = (change.at, change.at + change.removed);
        let delta = change.inserted as isize - change.removed as isize;
        // A match the edit starts inside, or removes part of, is gone; one that ends where the
        // edit starts stays; one that starts at or after its end moves.
        let kept = |m: &Range<usize>| m.end <= at || m.start >= cut;
        let moved = |m: &Range<usize>| {
            if m.end > at {
                m.start.saturating_add_signed(delta)..m.end.saturating_add_signed(delta)
            } else {
                m.clone()
            }
        };
        let current = self
            .current
            .and_then(|k| self.matches.get(k))
            .filter(|m| kept(m))
            .map(moved);
        self.matches.retain(kept);
        for m in &mut self.matches {
            *m = moved(m);
        }
        self.current = current.and_then(|c| self.matches.iter().position(|m| *m == c));
    }

    fn cancel(&mut self) {
        if let Some(c) = self.cancel.take() {
            c.store(true, Ordering::Relaxed);
        }
    }

    /// The counter beside the find box.
    pub fn label(&self) -> String {
        if self.error.is_some() {
            return strings::BAD_PATTERN.to_string();
        }
        if self.searching && !self.text.is_empty() {
            return strings::FIND_SEARCHING.to_string();
        }
        strings::find_label(
            self.text.is_empty(),
            self.current.map_or(0, |i| i + 1),
            self.matches.len(),
            self.capped,
        )
    }
}

/// A revision and caret, and the bracket pair found there.
type BracketAt = (u64, usize, Option<(usize, usize)>);

pub struct Ready {
    pub buffer: Buffer,
    /// The document revision this text was loaded at. A reload bumps the document's revision at
    /// once but keeps drawing this text until the new one arrives, so layouts are keyed by this.
    pub revision: u64,
    pub info: FileInfo,
    /// Undo and redo, and whether the text differs from the file.
    pub history: History,
    pub language: Option<Language>,
    /// The language exists and the file is within its limits: the service colours it.
    pub colourable: bool,
    pub sel: Selection,
    /// Bookmarked lines, sorted. They follow edits and a reload, and are gone when the tab
    /// closes.
    pub bookmarks: Vec<usize>,
    /// The folded blocks. They follow edits; a reload opens them all.
    pub folds: Folds,
    /// The other selections when there are several carets, in document order. `sel` is the
    /// primary: the one the view follows and Undo puts back.
    pub extra: Vec<Selection>,
    /// The column selection being extended by keys: anchor line and column, caret line and
    /// column, and the selections it made (it goes on only while they are still the ones).
    pub column: Option<(usize, usize, usize, usize, Selection, usize)>,
    /// First visible line, fractional. With word wrap a whole number: the line whose row
    /// `top_row` is at the view's top.
    pub top: f64,
    pub top_row: usize,
    /// The part of a row a scroll left over (word wrap scrolls by whole rows).
    pub row_rest: f64,
    /// Word wrap is on (`Docs::wrap`, copied here before each frame).
    pub wrap: bool,
    /// The rows of wrapped lines and what they were found for: revision, view width, advances.
    pub rows: Lru<usize, crate::view::wrap::Rows>,
    pub rows_stamp: (u64, u32, u32, u32),
    /// Horizontal scroll, logical px.
    pub left: f64,
    pub colors: LineColors,
    pub find: FindState,
    /// The bracket pair at the caret, kept for the revision and caret it was found at.
    bracket: std::cell::Cell<Option<BracketAt>>,
    /// Chunk indexes of long lines, per line, the most recently used
    /// [`CHUNK_INDEXES`](crate::view::CHUNK_INDEXES). A reload makes a new `Ready`, so they are
    /// per revision.
    pub chunks: Lru<usize, ChunkIndex>,
    /// The right end of the widest line or chunk shaped so far, logical px.
    pub widest: f32,
    /// The view as last drawn, for keeping the caret in view between frames: lines that fit,
    /// text width, and the advances of a narrow and a wide glyph, in logical px.
    pub view_lines: f64,
    pub view_w: f32,
    pub advance: f32,
    pub wide_advance: f32,
}

impl Ready {
    /// Every selection in document order, and which of them is the primary.
    pub fn all_sels(&self) -> (Vec<Selection>, usize) {
        let at = self
            .extra
            .partition_point(|s| s.range().start < self.sel.range().start);
        let mut v = self.extra.clone();
        v.insert(at, self.sel);
        (v, at)
    }

    /// Takes selections in document order: the primary and the others.
    pub fn set_sels(&mut self, mut list: Vec<Selection>, primary: usize) {
        if list.is_empty() {
            return;
        }
        self.sel = list.remove(primary.min(list.len() - 1));
        self.extra = list;
    }

    /// Adds a caret at `offset`, which becomes the primary.
    pub fn add_caret(&mut self, offset: usize) {
        let (mut list, _) = self.all_sels();
        list.push(Selection::at(offset));
        let (list, primary) = multi::normalize(&self.buffer, &list, list.len() - 1);
        self.set_sels(list, primary);
    }

    /// The column selection between two (line, display column) corners; the caret's is the
    /// primary.
    pub fn select_column(&mut self, anchor: (usize, usize), caret: (usize, usize)) {
        let (list, primary) = multi::column(
            &self.buffer,
            anchor.0,
            anchor.1,
            caret.0,
            caret.1,
            TAB_WIDTH,
        );
        self.set_sels(list, primary);
        self.column = Some((
            anchor.0,
            anchor.1,
            caret.0,
            caret.1,
            self.sel,
            self.extra.len(),
        ));
    }

    /// The line and display column of an offset.
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        (
            self.buffer.line_of(offset),
            multi::display_col(&self.buffer, offset, TAB_WIDTH),
        )
    }

    /// Extends the column selection by keys: `lines` down and `cols` right. It starts from the
    /// primary selection when no column selection is being made.
    pub fn extend_column(&mut self, lines: isize, cols: isize) {
        let going = self
            .column
            .filter(|c| (c.4, c.5) == (self.sel, self.extra.len()));
        let (al, ac, cl, cc) = match going {
            Some(c) => (c.0, c.1, c.2, c.3),
            None => {
                let (a, c) = (
                    self.line_col(self.sel.anchor),
                    self.line_col(self.sel.caret),
                );
                (a.0, a.1, c.0, c.1)
            }
        };
        let last = self.buffer.len_lines().saturating_sub(1);
        let cl = cl.saturating_add_signed(lines).min(last);
        let cc = cc.saturating_add_signed(cols);
        self.select_column((al, ac), (cl, cc));
    }

    /// Lines show wrapped: word wrap is on and no line is too long for it.
    pub fn wrapped(&self) -> bool {
        self.wrap && self.info.longest_line_chars <= crate::view::wrap::WRAP_LINE_MAX
    }

    /// The bracket at the caret and its partner, as byte offsets, when nothing is selected.
    pub fn brackets(&self) -> Option<(usize, usize)> {
        if !self.sel.range().is_empty() {
            return None;
        }
        let (revision, caret) = (self.revision, self.sel.caret);
        if let Some((r, c, found)) = self.bracket.get()
            && (r, c) == (revision, caret)
        {
            return found;
        }
        let found = mdedit_core::bracket::matching(
            self.buffer.rope(),
            caret,
            mdedit_core::limits::BRACKET_SCAN,
        );
        self.bracket.set(Some((revision, caret, found)));
        found
    }

    /// The selection when it is one whole word on one line: its other occurrences are marked.
    pub fn twin_word(&self) -> Option<String> {
        let sel = self.sel.range();
        if sel.is_empty() || sel.len() > mdedit_core::limits::TWIN_WORD_MAX {
            return None;
        }
        let word_char = |c: char| c.is_alphanumeric() || c == '_';
        let mut word = String::new();
        self.buffer.text(sel.clone(), &mut word);
        if !word.chars().all(word_char) {
            return None;
        }
        // Whole: no word character touches it on either side.
        let rope = self.buffer.rope();
        let (a, b) = (rope.byte_to_char(sel.start), rope.byte_to_char(sel.end));
        let before = a.checked_sub(1).map(|i| rope.char(i));
        let after = (b < rope.len_chars()).then(|| rope.char(b));
        (!before.is_some_and(word_char) && !after.is_some_and(word_char)).then_some(word)
    }

    /// Puts unsaved text back over the text just read, as one edit: Undo gives the file's text.
    fn put_back(&mut self, text: &str) {
        if self.info.lossy {
            return;
        }
        let all = mdedit_core::selection::select_all(&self.buffer);
        let out = edit::insert(&mut self.buffer, &mut self.history, all, text);
        if let Some(change) = out.change {
            self.info.eol.apply(&change.eol_removed, &change.eol_added);
        }
        self.history.seal();
        self.sel = Selection::at(0);
    }

    /// Keeps what hangs on the text in step with an edit.
    fn changed(&mut self, change: &Change) {
        self.info.eol.apply(&change.eol_removed, &change.eol_added);
        let rel = change
            .at
            .saturating_sub(self.buffer.line_start(change.first_line));
        self.colors.edited(change, rel);
        self.find.edited(change);
        self.chunks.clear();
        lines::shift_marks(&mut self.bookmarks, change);
        self.folds.shift(change);
    }

    /// Opens the folds a caret is in: a caret is never on a line that does not show.
    pub fn unfold_carets(&mut self) {
        if self.folds.is_empty() {
            return;
        }
        let len = self.buffer.len_bytes();
        for k in 0..=self.extra.len() {
            let caret = self.extra.get(k).map_or(self.sel.caret, |s| s.caret);
            let line = self.buffer.line_of(caret.min(len));
            self.folds.reveal(line);
        }
    }

    /// After folding: one caret, on a line that shows (the end of the fold's first line when
    /// it was inside), and in view.
    fn caret_out_of_folds(&mut self) {
        let line = self.buffer.line_of(self.sel.caret);
        let inside = self.folds.is_hidden(line);
        if inside || !self.extra.is_empty() {
            self.history.seal();
            self.extra.clear();
            self.column = None;
        }
        if inside {
            let at = self.buffer.line_range(self.folds.shown(line)).end;
            self.sel = Selection::at(at);
        }
        let caret = self.sel.caret;
        crate::view::reveal(self, caret, false);
    }

    /// Opens the fold the caret's line heads, or else folds the block the line heads or is
    /// in. False when the line is in no block.
    pub fn toggle_fold(&mut self) -> bool {
        let line = self.buffer.line_of(self.sel.caret);
        if self.folds.unfold(line) {
            return true;
        }
        let Some((header, end)) = fold::block_at(&self.buffer, line, TAB_WIDTH) else {
            return false;
        };
        if !self.folds.fold(header, end) {
            return false;
        }
        self.caret_out_of_folds();
        true
    }

    /// Folds every block that starts at the left margin. The number folded, and whether the
    /// document has more lines than were looked at.
    pub fn fold_all(&mut self) -> (usize, bool) {
        let (blocks, more) = fold::top_blocks(&self.buffer, TAB_WIDTH);
        let mut n = 0;
        for (header, end) in blocks {
            n += self.folds.fold(header, end) as usize;
        }
        self.caret_out_of_folds();
        (n, more)
    }

    /// Opens every fold, keeping the caret in view.
    pub fn unfold_all(&mut self) {
        self.folds.clear();
        let caret = self.sel.caret;
        crate::view::reveal(self, caret, false);
    }

    /// Sets or removes the bookmark of the caret's line.
    pub fn toggle_bookmark(&mut self) {
        let line = self.buffer.line_of(self.sel.caret);
        match self.bookmarks.binary_search(&line) {
            Ok(k) => {
                self.bookmarks.remove(k);
            }
            Err(k) => self.bookmarks.insert(k, line),
        }
    }

    /// The bookmarked line after the caret's (before it when not `forward`), going round at
    /// the document's ends.
    pub fn bookmark_from_caret(&self, forward: bool) -> Option<usize> {
        let line = self.buffer.line_of(self.sel.caret);
        let marks = &self.bookmarks;
        if forward {
            let k = marks.partition_point(|&m| m <= line);
            marks.get(k).or(marks.first()).copied()
        } else {
            let k = marks.partition_point(|&m| m < line);
            k.checked_sub(1).map(|k| marks[k]).or(marks.last().copied())
        }
    }

    fn new(loaded: Loaded, language: Option<Language>, revision: u64) -> Ready {
        Ready {
            buffer: loaded.buffer,
            revision,
            info: loaded.info,
            history: History::new(),
            language,
            colourable: false,
            sel: Selection::at(0),
            bookmarks: Vec::new(),
            folds: Folds::default(),
            extra: Vec::new(),
            column: None,
            top: 0.0,
            top_row: 0,
            row_rest: 0.0,
            wrap: false,
            rows: Lru::new(crate::view::wrap::ROW_CACHE),
            rows_stamp: (0, 0, 0, 0),
            left: 0.0,
            colors: LineColors::default(),
            find: FindState::default(),
            bracket: std::cell::Cell::new(None),
            chunks: Lru::new(crate::view::CHUNK_INDEXES),
            widest: 0.0,
            view_lines: 30.0,
            view_w: 800.0,
            advance: 7.7,
            wide_advance: 14.0,
        }
    }

    /// Selects `range` as far as the text has it: a range found in the file as it was read
    /// some time ago, which the text may no longer reach.
    pub fn select_clamped(&mut self, range: Range<usize>) {
        let len = self.buffer.len_bytes();
        let start = self.buffer.snap_to_grapheme(range.start.min(len));
        let end = self.buffer.snap_to_grapheme(range.end.min(len)).max(start);
        self.history.seal();
        self.select_and_reveal(start..end);
    }

    /// Selects `range` (caret at its end) and brings it into view, centred if it was off screen.
    pub fn select_and_reveal(&mut self, range: Range<usize>) {
        self.extra.clear();
        self.sel = Selection {
            anchor: range.start,
            caret: range.end,
            goal_col: None,
        };
        crate::view::reveal(self, range.start, true);
        crate::view::reveal(self, range.end, true);
    }
}

pub enum DocState {
    /// A restored tab not shown yet.
    Pending,
    Loading,
    Ready(Box<Ready>),
    Failed(String),
}

pub struct Doc {
    pub id: DocId,
    /// The file, or `None` for a new document that has not been saved yet.
    pub path: Option<PathBuf>,
    /// The number in the name of a new document ("Untitled 2"); 0 once it has a file.
    pub untitled: u32,
    pub revision: u64,
    pub state: DocState,
    /// The file is not on disk: gone since it was read (the tab keeps the text last read), or
    /// never there (the tab shows why it could not be opened).
    pub missing: bool,
    /// The file changed on disk while the text had unsaved changes, so it was not reloaded.
    pub changed_on_disk: bool,
    /// The user chose not to save these changes while closing; cleared by the next edit or when
    /// the close is cancelled.
    pub discard: bool,
    /// A range to select once the text is read: a result of "Find in files" was opened.
    pub pending_select: Option<Range<usize>>,
    /// The file holding the unsaved text this tab had when the window last closed; it replaces
    /// the file's text once that is read.
    pub restore: Option<PathBuf>,
    /// The encoding the user chose to read the file as; every load of this tab uses it.
    pub forced: Option<Encoding>,
    /// The encoding to save in was changed since the file was read or written.
    pub encoding_changed: bool,
    /// The lossy- or binary-file toast was shown for this document.
    lossy_toasted: bool,
    /// Bytes read and total, written by the load thread.
    progress: Arc<(AtomicU64, AtomicU64)>,
    cancel: Option<Arc<AtomicBool>>,
}

impl Doc {
    fn new(id: DocId, path: PathBuf) -> Doc {
        Doc {
            id,
            path: Some(path),
            untitled: 0,
            revision: 0,
            state: DocState::Pending,
            missing: false,
            changed_on_disk: false,
            discard: false,
            pending_select: None,
            restore: None,
            forced: None,
            encoding_changed: false,
            lossy_toasted: false,
            progress: Arc::new((AtomicU64::new(0), AtomicU64::new(0))),
            cancel: None,
        }
    }

    pub fn progress(&self) -> (u64, u64) {
        (
            self.progress.0.load(Ordering::Relaxed),
            self.progress.1.load(Ordering::Relaxed),
        )
    }

    /// A load job is running (the first load, or a reload behind a `Ready` text).
    pub fn is_loading(&self) -> bool {
        self.cancel.is_some()
    }

    pub fn ready(&self) -> Option<&Ready> {
        match &self.state {
            DocState::Ready(r) => Some(r),
            _ => None,
        }
    }

    pub fn ready_mut(&mut self) -> Option<&mut Ready> {
        match &mut self.state {
            DocState::Ready(r) => Some(r),
            _ => None,
        }
    }

    /// The "deleted or moved, showing the text last read" banner: only over text that was read.
    pub fn shows_gone_banner(&self) -> bool {
        self.missing && self.ready().is_some()
    }

    /// The text has changes that are not in the file (also unsaved text still to be put back).
    pub fn modified(&self) -> bool {
        !self.discard
            && (self.restore.is_some()
                || self.encoding_changed
                || self.ready().is_some_and(|r| r.history.is_modified()))
    }

    /// The banner over the text, if any.
    pub fn banner(&self) -> Option<&'static str> {
        if self.shows_gone_banner() {
            Some(strings::FILE_GONE)
        } else if self.changed_on_disk && self.ready().is_some() {
            Some(strings::FILE_CHANGED)
        } else {
            None
        }
    }

    /// The name with the unsaved mark, for the window title.
    pub fn title(&self) -> String {
        if self.modified() {
            format!("{}{}", strings::MODIFIED_MARK, self.name())
        } else {
            self.name()
        }
    }

    pub fn name(&self) -> String {
        let Some(path) = &self.path else {
            return strings::untitled(self.untitled);
        };
        path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    }

    fn cancel_load(&mut self) {
        if let Some(c) = self.cancel.take() {
            c.store(true, Ordering::Relaxed);
        }
    }
}

pub struct LoadDone {
    pub doc: DocId,
    pub revision: u64,
    pub result: Result<(Loaded, Option<Language>), LoadError>,
}

pub enum FindKind {
    All(Matches),
    /// One match from `find_next`, past the kept ones.
    Next(Option<Range<usize>>),
}

pub struct FindDone {
    pub doc: DocId,
    pub revision: u64,
    pub text: String,
    pub options: FindOptions,
    pub kind: FindKind,
}

/// How much of the first line the language lookup sees: enough for a shebang or a mode line.
const FIRST_LINE_BYTES: usize = 1024;

/// The document's language from its path and the start of its first line, worked out on the
/// load thread. Only the first `FIRST_LINE_BYTES` are copied, so a file that is one long line is
/// not copied whole beside its rope, and syntect's first-line patterns run over that much only.
fn language_of(path: &Path, buffer: &Buffer) -> Option<Language> {
    let mut first = String::new();
    if buffer.len_lines() > 0 {
        let range = buffer.line_range(0);
        let end = buffer.snap_to_grapheme(range.end.min(range.start + FIRST_LINE_BYTES));
        buffer.text(range.start..end, &mut first);
    }
    Language::for_path(path, &first)
}

/// Runs one load. Shared by the thread and the synchronous `--shot` path.
fn run_load(
    path: &Path,
    opts: &LoadOptions,
    progress: &(AtomicU64, AtomicU64),
    cancel: &AtomicBool,
    report: &dyn Fn(),
) -> Result<(Loaded, Option<Language>), LoadError> {
    let loaded = load::load_path(
        path,
        opts,
        &|done, total| {
            progress.0.store(done, Ordering::Relaxed);
            progress.1.store(total, Ordering::Relaxed);
            report();
        },
        cancel,
    )?;
    let language = language_of(path, &loaded.buffer);
    Ok((loaded, language))
}

/// One editing command for [`Docs::edit`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditOp<'a> {
    /// Replaces the selection (typing; empty text deletes the selection).
    Insert(&'a str),
    /// Clipboard text: its line breaks become the document's.
    Paste(&'a str),
    Newline,
    Backspace,
    Delete,
    WordLeft,
    WordRight,
    Indent,
    Outdent,
    Undo,
    Redo,
    DuplicateLines,
    /// Down when true.
    MoveLines(bool),
    DeleteLines,
    /// With the language's line comment.
    ToggleComment,
    /// Upper case when true.
    Case(bool),
    TrimTrailing,
    TabsToSpaces,
    /// Makes every line break this one.
    LineEndings(&'static str),
    /// Replaces the selection by this template when the selection is a match of the document's
    /// query.
    ReplaceOne(&'a str),
    /// Replaces every match of the document's query by this template, as one undo step.
    ReplaceAll(&'a str),
}

/// The largest text Replace all takes. It runs on the UI thread at about 1.4 to 2.2 s per
/// 100 MB when matches are dense, so this keeps the worst case near half a second.
pub const REPLACE_ALL_MAX_BYTES: usize = 24 * 1024 * 1024;

/// The largest text the two every-line commands (trim trailing spaces, tabs to spaces) take.
/// They run on the UI thread at about 4 to 7 s per 100 MB when every line changes, so this
/// keeps the worst case near half a second.
pub const ALL_LINES_MAX_BYTES: usize = 8 * 1024 * 1024;

/// Why a save did not happen.
#[derive(Debug)]
pub enum SaveFailed {
    /// Nothing to save to or from (no ready document, or one with bytes that could not be read).
    NotSavable,
    /// A new document: it needs a file name first (Save as).
    Untitled,
    Error(SaveError),
}

/// What taking in a load result means for the shell.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// The active document became ready (first load or reload).
    pub active_ready: bool,
}

pub struct Docs {
    /// The update on offer, downloading or installed, as the bar and About show it.
    pub updates: crate::updater::Updates,
    /// The setting: look for a newer version at launch.
    pub check_updates: bool,
    pub tabs: Vec<Doc>,
    active: Option<usize>,
    next_id: DocId,
    pub recents: Recents,
    pub folder: Option<PathBuf>,
    pub theme: ThemeChoice,
    /// The text size, percent; the view draws at it and the status strip shows it.
    pub zoom: u32,
    /// Spaces, tabs and line endings show as marks.
    pub whitespace: bool,
    /// Split view: the tab that shows in the second pane, and whether that pane is the left
    /// one. The active tab shows in the other pane and has the keyboard.
    pub other: Option<DocId>,
    pub other_left: bool,
    /// A macro is being recorded (the status strip says so).
    pub recording: bool,
    /// Lines longer than the view go on in rows below (word wrap).
    pub wrap: bool,
    opts: LoadOptions,
    waker: Waker,
    /// The loads that are done and not taken in yet, with a signal for the start's wait. As
    /// with `find_done`, not a channel.
    load_done: Arc<(std::sync::Mutex<Vec<LoadDone>>, std::sync::Condvar)>,
    /// What the find workers have done and the window has not taken in: a list under a lock, not
    /// a channel, which costs the exe some 15 KB for each type sent.
    find_done: Arc<std::sync::Mutex<Vec<FindDone>>>,
    toast: Option<(String, Instant)>,
    /// The set of open files changed: the watcher needs the new list.
    pub watch_changed: bool,
}

impl Docs {
    pub fn new(code_page: u32, waker: Waker) -> Docs {
        Docs {
            tabs: Vec::new(),
            active: None,
            next_id: 1,
            recents: Recents::default(),
            folder: None,
            theme: ThemeChoice::System,
            zoom: 100,
            whitespace: false,
            other: None,
            other_left: false,
            recording: false,
            wrap: false,
            opts: LoadOptions {
                legacy: mdedit_core::encoding::legacy_for_code_page(code_page),
                max_bytes: MAX_FILE_BYTES,
                forced: None,
            },
            waker,
            load_done: Default::default(),
            find_done: Default::default(),
            toast: None,
            updates: crate::updater::Updates::default(),
            check_updates: true,
            watch_changed: true,
        }
    }

    /// Wakes the event loop, for threads the shell starts.
    /// How files are read, for a search of files that reads them the same way.
    pub fn load_options(&self) -> LoadOptions {
        LoadOptions {
            forced: None,
            ..self.opts
        }
    }

    /// Selects `range` in tab `i`, now or when its text has been read.
    pub fn select_in(&mut self, i: usize, range: Range<usize>) {
        let Some(doc) = self.tabs.get_mut(i) else {
            return;
        };
        match doc.ready_mut() {
            Some(r) => r.select_clamped(range),
            None => doc.pending_select = Some(range),
        }
    }

    pub fn waker(&self) -> Waker {
        self.waker.clone()
    }

    // ---- tabs ------------------------------------------------------------------------------

    pub fn active(&self) -> Option<usize> {
        self.active
    }

    pub fn active_doc(&self) -> Option<&Doc> {
        self.tabs.get(self.active?)
    }

    pub fn active_doc_mut(&mut self) -> Option<&mut Doc> {
        let i = self.active?;
        self.tabs.get_mut(i)
    }

    pub fn active_ready(&self) -> Option<&Ready> {
        self.active_doc()?.ready()
    }

    pub fn active_ready_mut(&mut self) -> Option<&mut Ready> {
        self.active_doc_mut()?.ready_mut()
    }

    pub fn labels(&self) -> Vec<String> {
        // A new document's label is its name; it has no folder to tell it from another.
        let paths: Vec<PathBuf> = self
            .tabs
            .iter()
            .map(|d| d.path.clone().unwrap_or_else(|| PathBuf::from(d.name())))
            .collect();
        let mut labels = tab_labels(&paths);
        for (label, doc) in labels.iter_mut().zip(&self.tabs) {
            if doc.modified() {
                label.insert_str(0, strings::MODIFIED_MARK);
            }
        }
        labels
    }

    pub fn index_of_id(&self, id: DocId) -> Option<usize> {
        self.tabs.iter().position(|d| d.id == id)
    }

    pub fn index_of(&self, path: &Path) -> Option<usize> {
        self.tabs
            .iter()
            .position(|d| d.path.as_deref().is_some_and(|p| same_path(p, path)))
    }

    /// Adds a tab without loading it (a restored tab). Returns its index.
    pub fn add_pending(&mut self, path: &Path) -> usize {
        let path = canonical_or_absolute(path);
        if let Some(i) = self.index_of(&path) {
            return i;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Doc::new(id, path));
        self.watch_changed = true;
        self.tabs.len() - 1
    }

    /// Opens an empty document that has no file yet and makes it active. It takes the platform's
    /// line ending and is saved as UTF-8.
    pub fn new_untitled(&mut self) -> usize {
        let taken = |n: u32| {
            self.tabs
                .iter()
                .any(|d| d.path.is_none() && d.untitled == n)
        };
        let number = (1..).find(|&n| !taken(n)).unwrap_or(1);
        let id = self.next_id;
        self.next_id += 1;
        let loaded = load::load_bytes(b"", &self.opts);
        self.tabs.push(Doc {
            id,
            path: None,
            untitled: number,
            revision: 1,
            state: DocState::Ready(Box::new(Ready::new(loaded, None, 1))),
            missing: false,
            changed_on_disk: false,
            discard: false,
            pending_select: None,
            restore: None,
            forced: None,
            encoding_changed: false,
            lossy_toasted: false,
            progress: Arc::new((AtomicU64::new(0), AtomicU64::new(0))),
            cancel: None,
        });
        let i = self.tabs.len() - 1;
        self.active = Some(i);
        i
    }

    /// Opens a document that has no file with the unsaved text it had. `number` is the one in
    /// its name, or 0 for the next free one.
    pub fn restore_untitled(&mut self, number: u32, text: &str) -> usize {
        let i = self.new_untitled();
        let free = !self
            .tabs
            .iter()
            .any(|d| d.path.is_none() && d.untitled == number);
        let doc = &mut self.tabs[i];
        if number > 0 && free {
            doc.untitled = number;
        }
        if let Some(r) = doc.ready_mut() {
            r.put_back(text);
        }
        i
    }

    /// Writes the text of every tab with unsaved changes into `dir`, removes the backups nothing
    /// refers to any more, and returns the list for the session. A tab whose unsaved text was
    /// never put back (it was not read yet) keeps the backup it has.
    pub fn write_backups(&self, dir: &Path) -> std::io::Result<Vec<Unsaved>> {
        use std::io::Write;
        let mut out = Vec::new();
        let waiting = |d: &Doc| {
            let file = d.restore.as_ref()?.file_name()?.to_str()?.to_string();
            Unsaved::is_file_name(&file).then_some(file)
        };
        let kept: Vec<String> = self.tabs.iter().filter_map(waiting).collect();
        let mut number = 0u32;
        for d in self.tabs.iter().filter(|d| d.modified()) {
            let file = match waiting(d) {
                Some(file) => file,
                None => {
                    let Some(r) = d.ready() else { continue };
                    std::fs::create_dir_all(dir)?;
                    let file = loop {
                        number += 1;
                        let name = Unsaved::file_name(number);
                        if !kept.contains(&name) {
                            break name;
                        }
                    };
                    // Written beside its place and renamed, so a backup is whole or absent.
                    let (tmp, path) = (dir.join(format!("{file}.tmp")), dir.join(&file));
                    let mut w = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
                    for chunk in r.buffer.rope().chunks() {
                        w.write_all(chunk.as_bytes())?;
                    }
                    w.into_inner().map_err(|e| e.into_error())?.sync_all()?;
                    std::fs::rename(&tmp, &path)?;
                    file
                }
            };
            out.push(Unsaved {
                file,
                untitled: d.untitled,
                path: d.path.clone(),
            });
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if !out.iter().any(|u| u.file == name) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        Ok(out)
    }

    /// Opens `path` in a tab (or shows the tab that has it) and makes it active.
    pub fn open(&mut self, path: &Path) -> usize {
        let i = self.add_pending(path);
        self.activate(i);
        i
    }

    /// Makes tab `i` active; its first activation starts its load.
    /// The tab in the second pane of a split view: one that is still open and is not the
    /// active one.
    pub fn other(&self) -> Option<usize> {
        let i = self.index_of_id(self.other?)?;
        (Some(i) != self.active).then_some(i)
    }

    /// Splits the view, with the tab after the active one in the second pane at the right, or
    /// ends the split. False when there is no second tab to show.
    pub fn toggle_split(&mut self) -> bool {
        if self.other.take().is_some() {
            return true;
        }
        let Some(a) = self.active.filter(|_| self.tabs.len() > 1) else {
            return false;
        };
        let i = (a + 1) % self.tabs.len();
        self.other = Some(self.tabs[i].id);
        self.other_left = false;
        if matches!(self.tabs[i].state, DocState::Pending) {
            self.start_load(i);
        }
        true
    }

    /// Gives the keyboard to the second pane. False without one.
    pub fn focus_other(&mut self) -> bool {
        let Some(i) = self.other() else {
            return false;
        };
        self.activate(i);
        true
    }

    pub fn activate(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        // The tab of the second pane: the panes change roles, each keeping its tab.
        if self.other == Some(self.tabs[i].id)
            && let Some(a) = self.active.filter(|&a| a != i && a < self.tabs.len())
        {
            self.other = Some(self.tabs[a].id);
            self.other_left = !self.other_left;
        }
        self.active = Some(i);
        if matches!(self.tabs[i].state, DocState::Pending) {
            self.start_load(i);
        }
        self.toast_lossy(i);
    }

    pub fn close(&mut self, i: usize, service: Option<&HighlightService>) {
        if i >= self.tabs.len() {
            return;
        }
        let mut doc = self.tabs.remove(i);
        doc.cancel_load();
        if let Some(r) = doc.ready_mut() {
            r.find.cancel();
        }
        if let Some(s) = service {
            s.close(doc.id);
        }
        self.watch_changed = true;
        self.active = match self.active {
            _ if self.tabs.is_empty() => None,
            Some(a) if a > i => Some(a - 1),
            Some(a) if a == i => Some(i.min(self.tabs.len() - 1)),
            other => other,
        };
        if let Some(a) = self.active {
            self.activate(a);
        }
    }

    pub fn close_others(&mut self, keep: usize, service: Option<&HighlightService>) {
        let Some(id) = self.tabs.get(keep).map(|d| d.id) else {
            return;
        };
        while let Some(i) = self.tabs.iter().position(|d| d.id != id) {
            self.close(i, service);
        }
    }

    pub fn close_all(&mut self, service: Option<&HighlightService>) {
        while !self.tabs.is_empty() {
            self.close(self.tabs.len() - 1, service);
        }
    }

    /// Moves tab `from` to `to`; the active tab stays active.
    pub fn move_tab(&mut self, from: usize, to: usize) -> bool {
        if from == to || from >= self.tabs.len() || to >= self.tabs.len() {
            return false;
        }
        let active_id = self.active_doc().map(|d| d.id);
        let doc = self.tabs.remove(from);
        self.tabs.insert(to, doc);
        self.active = active_id.and_then(|id| self.tabs.iter().position(|d| d.id == id));
        true
    }

    pub fn step_tab(&mut self, forward: bool) {
        let n = self.tabs.len();
        if let Some(a) = self.active.filter(|_| n > 1) {
            self.activate(if forward {
                (a + 1) % n
            } else {
                (a + n - 1) % n
            });
        }
    }

    /// The files of the tabs that have one, in tab order.
    pub fn paths(&self) -> Vec<PathBuf> {
        self.tabs.iter().filter_map(|d| d.path.clone()).collect()
    }

    /// The active tab's place in [`Docs::paths`]. With a new document active, the nearest tab
    /// before it that has a file.
    pub fn active_among_paths(&self) -> usize {
        let Some(a) = self.active else { return 0 };
        self.tabs[..=a.min(self.tabs.len().saturating_sub(1))]
            .iter()
            .filter(|d| d.path.is_some())
            .count()
            .saturating_sub(1)
    }

    // ---- loading ---------------------------------------------------------------------------

    /// Starts a load (or reload) of tab `i` on a thread.
    pub fn start_load(&mut self, i: usize) {
        // A new document has nothing to read.
        let Some(path) = self.tabs[i].path.clone() else {
            return;
        };
        let opts = LoadOptions {
            forced: self.tabs[i].forced,
            ..self.opts
        };
        let tx = self.load_done.clone();
        let waker = self.waker.clone();
        let doc = &mut self.tabs[i];
        doc.cancel_load();
        doc.revision += 1;
        doc.progress.0.store(0, Ordering::Relaxed);
        doc.progress.1.store(0, Ordering::Relaxed);
        if matches!(doc.state, DocState::Pending | DocState::Failed(_)) {
            doc.state = DocState::Loading;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        doc.cancel = Some(cancel.clone());
        let (id, revision, progress) = (doc.id, doc.revision, doc.progress.clone());
        std::thread::spawn(move || {
            let w = waker.clone();
            let result = run_load(&path, &opts, &progress, &cancel, &move || w(Wake::Loaded));
            if matches!(result, Err(LoadError::Cancelled)) {
                return;
            }
            tx.0.lock().unwrap().push(LoadDone {
                doc: id,
                revision,
                result,
            });
            tx.1.notify_all();
            waker(Wake::Loaded);
        });
    }

    /// Loads tab `i` on this thread (for `--shot`) and takes the result in.
    pub fn load_now(&mut self, i: usize, service: Option<&HighlightService>, dark: bool) {
        let doc = &mut self.tabs[i];
        let Some(path) = doc.path.clone() else {
            return;
        };
        doc.cancel_load();
        doc.revision += 1;
        let cancel = AtomicBool::new(false);
        let opts = LoadOptions {
            forced: doc.forced,
            ..self.opts
        };
        let result = run_load(&path, &opts, &doc.progress, &cancel, &|| {});
        let done = LoadDone {
            doc: doc.id,
            revision: doc.revision,
            result,
        };
        doc.cancel = Some(Arc::new(AtomicBool::new(false)));
        self.apply_load(done, service, dark);
    }

    /// Waits up to `deadline` for a load result and takes in everything that has arrived.
    pub fn wait_load(
        &mut self,
        deadline: Instant,
        service: Option<&HighlightService>,
        dark: bool,
    ) -> Applied {
        {
            let (list, signal) = &*self.load_done;
            let mut done = list.lock().unwrap();
            while done.is_empty() {
                let now = Instant::now();
                if deadline <= now {
                    break;
                }
                done = signal.wait_timeout(done, deadline - now).unwrap().0;
            }
        }
        self.drain_loads(service, dark)
    }

    /// Takes in every finished load.
    pub fn drain_loads(&mut self, service: Option<&HighlightService>, dark: bool) -> Applied {
        let mut out = Applied::default();
        let all = std::mem::take(&mut *self.load_done.0.lock().unwrap());
        for done in all {
            out.active_ready |= self.apply_load(done, service, dark).active_ready;
        }
        out
    }

    fn apply_load(
        &mut self,
        done: LoadDone,
        service: Option<&HighlightService>,
        dark: bool,
    ) -> Applied {
        let Some(i) = self.tabs.iter().position(|d| d.id == done.doc) else {
            return Applied::default();
        };
        if self.tabs[i].revision != done.revision {
            return Applied::default();
        }
        let active = self.active == Some(i);
        let doc = &mut self.tabs[i];
        doc.cancel = None;
        match done.result {
            Ok((loaded, language)) => {
                let mut r = Ready::new(loaded, language, done.revision);
                // A reload keeps the caret's line and column, the scroll and the find query.
                let mut query = None;
                if let Some(old) = doc.ready_mut() {
                    let pos: Position = old.buffer.position(old.sel.caret);
                    let caret = r.buffer.snap_to_grapheme(r.buffer.offset(pos));
                    r.sel = Selection::at(caret);
                    let last = r.buffer.len_lines().saturating_sub(1);
                    r.bookmarks = old.bookmarks.iter().map(|&m| m.min(last)).collect();
                    r.bookmarks.dedup();
                    r.top = old.top;
                    r.top_row = old.top_row;
                    r.wrap = old.wrap;
                    r.left = old.left;
                    r.view_lines = old.view_lines;
                    r.view_w = old.view_w;
                    r.advance = old.advance;
                    r.wide_advance = old.wide_advance;
                    old.find.cancel();
                    if !old.find.text.is_empty() {
                        query = Some((old.find.text.clone(), old.find.options));
                    }
                }
                // Unsaved text from the last run goes back over what was read.
                if let Some(text) = doc
                    .restore
                    .take()
                    .and_then(|p| std::fs::read_to_string(p).ok())
                {
                    r.put_back(&text);
                }
                if let (Some(lang), Some(s)) = (language, service)
                    && lang.limits().allows_file(r.buffer.len_bytes())
                {
                    s.open(doc.id, doc.revision, r.buffer.snapshot(), lang, dark);
                    r.colourable = true;
                }
                if let Some(range) = doc.pending_select.take() {
                    r.select_clamped(range);
                }
                doc.state = DocState::Ready(Box::new(r));
                doc.missing = false;
                doc.changed_on_disk = false;
                doc.discard = false;
                doc.encoding_changed = false;
                if let Some(path) = doc.path.clone() {
                    self.recents.add(path);
                }
                if let Some((text, case)) = query {
                    self.find_in(i, &text, case, false, false);
                }
                if active {
                    self.toast_lossy(i);
                }
                Applied {
                    active_ready: active,
                }
            }
            Err(e) => {
                let gone = matches!(e, LoadError::NotFound);
                let was_ready = matches!(doc.state, DocState::Ready(_));
                if was_ready && gone {
                    // A file that is gone keeps its text, marked missing.
                    doc.missing = true;
                } else if was_ready {
                    let text = format!("{}: {e}", doc.name());
                    self.set_toast(&text);
                } else {
                    doc.missing = gone;
                    doc.state = DocState::Failed(e.to_string());
                }
                Applied::default()
            }
        }
    }

    /// The file watcher saw a change: reload changed files, mark gone ones.
    pub fn check_changed(&mut self) {
        for i in 0..self.tabs.len() {
            let doc = &self.tabs[i];
            let Some(path) = &doc.path else {
                continue;
            };
            let now = stamp(path);
            match (&doc.state, now) {
                (DocState::Ready(_), FileStamp::Missing) => self.tabs[i].missing = true,
                (DocState::Ready(r), s) if s != r.info.stamp || doc.missing => {
                    let changed = s != r.info.stamp;
                    if r.history.is_modified() {
                        // Unsaved changes are never replaced behind the user's back.
                        self.tabs[i].changed_on_disk = changed;
                        self.tabs[i].missing = false;
                    } else {
                        self.tabs[i].missing = false;
                        if !self.tabs[i].is_loading() {
                            self.start_load(i);
                        }
                    }
                }
                (DocState::Failed(_), FileStamp::Present { .. }) if doc.missing => {
                    self.tabs[i].missing = false;
                    self.start_load(i);
                }
                _ => {}
            }
        }
    }

    pub fn reload_active(&mut self) {
        if let Some(i) = self.active
            && !matches!(self.tabs[i].state, DocState::Pending)
            && self.tabs[i].path.is_some()
        {
            self.start_load(i);
        }
    }

    // ---- editing ---------------------------------------------------------------------------

    /// Applies an editing command to the active document. True when the text changed.
    pub fn edit(&mut self, op: EditOp, service: Option<&HighlightService>) -> bool {
        let Some(i) = self.active else { return false };
        let doc = &mut self.tabs[i];
        // A reload in flight would replace the text the edit went into.
        if doc.is_loading() {
            return false;
        }
        let name = doc.name();
        let Doc {
            id,
            revision,
            state,
            discard,
            ..
        } = doc;
        let DocState::Ready(r) = state else {
            return false;
        };
        if r.info.lossy {
            self.set_toast(&strings::read_only_toast(&name));
            return false;
        }
        let comment = r.language.and_then(Language::line_comment);
        let refused = match op {
            EditOp::ToggleComment if comment.is_none() => Some(strings::NO_LINE_COMMENT),
            EditOp::TrimTrailing | EditOp::TabsToSpaces | EditOp::LineEndings(_)
                if r.buffer.len_bytes() > ALL_LINES_MAX_BYTES =>
            {
                Some(strings::TOO_LARGE_FOR_ALL_LINES)
            }
            EditOp::ReplaceAll(_) if r.buffer.len_bytes() > REPLACE_ALL_MAX_BYTES => {
                Some(strings::TOO_LARGE_FOR_REPLACE_ALL)
            }
            _ => None,
        };
        if let Some(text) = refused {
            self.set_toast(text);
            return false;
        }
        let eol = r.info.eol.dominant();
        let query = match op {
            EditOp::ReplaceOne(_) | EditOp::ReplaceAll(_) => {
                match Query::new(&r.find.text, r.find.options) {
                    Some(q) => Some(q),
                    None => return false,
                }
            }
            _ => None,
        };
        // How many matches Replace all found.
        let mut replaced = None;
        // With several carets, typing and removing happen at each of them; anything else
        // goes on with the primary alone.
        let pasted;
        let pieces: Vec<&str>;
        let multi_op = if r.extra.is_empty() {
            None
        } else {
            match op {
                EditOp::Insert(text) => Some(MultiOp::Insert(text)),
                EditOp::Newline => Some(MultiOp::Insert(eol)),
                EditOp::Indent => Some(MultiOp::Insert("\t")),
                EditOp::Backspace => Some(MultiOp::Backspace),
                EditOp::Delete => Some(MultiOp::Delete),
                EditOp::Paste(text) => {
                    pasted = normalize_eols(text, eol);
                    // As many lines as carets: one line to each.
                    pieces = pasted
                        .strip_suffix(eol)
                        .unwrap_or(&pasted)
                        .split(eol)
                        .collect();
                    if pieces.len() == r.extra.len() + 1 {
                        Some(MultiOp::InsertEach(&pieces))
                    } else {
                        Some(MultiOp::Insert(&pasted))
                    }
                }
                _ => None,
            }
        };
        if multi_op.is_none() {
            r.extra.clear();
        }
        r.column = None;
        let (buf, hist, sel) = (&mut r.buffer, &mut r.history, r.sel);
        let out = if let Some(multi_op) = multi_op {
            let at = r
                .extra
                .partition_point(|s| s.range().start < sel.range().start);
            let mut list = std::mem::take(&mut r.extra);
            list.insert(at, sel);
            let (out, list, primary) = multi::edit(buf, hist, &list, at, multi_op);
            let mut list = list;
            if !list.is_empty() {
                list.remove(primary.min(list.len() - 1));
            }
            r.extra = list;
            out
        } else {
            match op {
                EditOp::ReplaceOne(with) => {
                    let (Some(q), range) = (&query, sel.range()) else {
                        return false;
                    };
                    if range.is_empty() {
                        return false;
                    }
                    replace::replace_one(buf, hist, sel, q, range, with, eol)
                }
                EditOp::ReplaceAll(with) => {
                    let Some(q) = &query else { return false };
                    let never = AtomicBool::new(false);
                    let (out, n) = replace::replace_all(buf, hist, sel, q, with, eol, None, &never);
                    replaced = Some(n);
                    out
                }
                EditOp::DuplicateLines => lines::duplicate_lines(buf, hist, sel, eol),
                EditOp::MoveLines(down) => lines::move_lines(buf, hist, sel, down, eol),
                EditOp::DeleteLines => lines::delete_lines(buf, hist, sel),
                EditOp::ToggleComment => {
                    lines::toggle_comment(buf, hist, sel, comment.unwrap_or_default())
                }
                EditOp::Case(upper) => lines::change_case(buf, hist, sel, upper),
                EditOp::TrimTrailing => lines::trim_trailing_whitespace(buf, hist, sel),
                EditOp::TabsToSpaces => lines::tabs_to_spaces(buf, hist, sel, TAB_WIDTH),
                EditOp::LineEndings(to) => lines::convert_line_endings(buf, hist, sel, to),
                EditOp::Insert(text) => edit::insert(buf, hist, sel, text),
                EditOp::Paste(text) => {
                    let text = normalize_eols(text, r.info.eol.dominant());
                    edit::insert(buf, hist, sel, &text)
                }
                EditOp::Newline => edit::newline(buf, hist, sel, r.info.eol.dominant(), true),
                EditOp::Backspace => edit::backspace(buf, hist, sel),
                EditOp::Delete => edit::delete(buf, hist, sel),
                EditOp::WordLeft => edit::delete_word_left(buf, hist, sel),
                EditOp::WordRight => edit::delete_word_right(buf, hist, sel),
                EditOp::Indent => edit::indent(buf, hist, sel, TAB_WIDTH),
                EditOp::Outdent => edit::outdent(buf, hist, sel, TAB_WIDTH),
                EditOp::Undo => match edit::undo(buf, hist) {
                    Some(out) => out,
                    None => return false,
                },
                EditOp::Redo => match edit::redo(buf, hist) {
                    Some(out) => out,
                    None => return false,
                },
            }
        };
        let Outcome { sel, change } = out;
        r.sel = sel;
        let changed = change.is_some();
        if let Some(change) = change {
            *revision += 1;
            *discard = false;
            r.revision = *revision;
            r.changed(&change);
            if let Some(s) = service.filter(|_| r.colourable) {
                s.edit(
                    *id,
                    *revision,
                    r.buffer.snapshot(),
                    change.first_line,
                    change.line_delta(),
                );
            }
        }
        crate::view::reveal(r, r.sel.caret, false);
        match replaced {
            Some(0) => self.set_toast(strings::NOTHING_TO_REPLACE),
            Some(n) => self.set_toast(&strings::replaced(n)),
            None => {}
        }
        changed
    }

    /// Gives every open document the word-wrap setting. Called before each frame, so a
    /// document that has just been read has it too.
    pub fn sync_wrap(&mut self) {
        let wrap = self.wrap;
        for doc in &mut self.tabs {
            if let Some(r) = doc.ready_mut()
                && r.wrap != wrap
            {
                r.wrap = wrap;
                r.top_row = 0;
                r.left = 0.0;
                // The caret's line stays in view across the change.
                let caret = r.sel.caret;
                crate::view::reveal(r, caret, false);
            }
        }
    }

    /// The system's code page, as the encoding menus offer it.
    pub fn legacy(&self) -> Encoding {
        Encoding::Legacy(self.opts.legacy)
    }

    /// Saves the active document in `encoding` from now on. Nothing is written yet: the tab
    /// shows as modified. Refused, with a toast naming the first of them, when the text has a
    /// character the encoding cannot hold. True when the encoding changed.
    pub fn save_in(&mut self, encoding: Encoding) -> bool {
        let Some(doc) = self.active_doc_mut() else {
            return false;
        };
        let Some(r) = doc.ready_mut().filter(|r| !r.info.lossy) else {
            return false;
        };
        if r.info.encoding == encoding {
            return false;
        }
        if let Some((ch, offset)) = save::first_unencodable(r.buffer.rope(), encoding) {
            let line = r.buffer.line_of(offset) + 1;
            let text = strings::cannot_encode(ch, line, &encoding.label());
            self.set_toast(&text);
            return false;
        }
        r.info.encoding = encoding;
        doc.encoding_changed = true;
        doc.discard = false;
        true
    }

    /// Puts the caret of the active document at a line and column, both from 1 and clamped to
    /// the text, and shows it. False without a document.
    pub fn go_to(&mut self, line: usize, col: usize) -> bool {
        let Some(r) = self.active_ready_mut() else {
            return false;
        };
        let line = line.clamp(1, r.buffer.len_lines().max(1)) - 1;
        let range = r.buffer.line_range(line);
        let at = (range.start + col.saturating_sub(1)).min(range.end);
        let at = r.buffer.snap_to_grapheme(at).min(range.end);
        r.history.seal();
        r.extra.clear();
        r.sel = Selection::at(at);
        crate::view::reveal(r, at, true);
        true
    }

    /// Goes to the next or the previous bookmark. False when the document has none.
    pub fn go_to_bookmark(&mut self, forward: bool) -> bool {
        let Some(line) = self
            .active_ready()
            .and_then(|r| r.bookmark_from_caret(forward))
        else {
            return false;
        };
        self.go_to(line + 1, 1)
    }

    /// Ends the current undo step of the active document (the caret moved, the focus left).
    pub fn seal(&mut self) {
        if let Some(r) = self.active_ready_mut() {
            r.history.seal();
        }
    }

    // ---- saving ----------------------------------------------------------------------------

    /// Writes tab `i` to its file. `overwrite` skips the check that the file is still the one
    /// that was read. The text is written on this thread: about 0.1 s per 100 MB.
    pub fn save(&mut self, i: usize, overwrite: bool) -> Result<(), SaveFailed> {
        let Some(doc) = self.tabs.get_mut(i) else {
            return Err(SaveFailed::NotSavable);
        };
        let Some(path) = doc.path.clone() else {
            return Err(SaveFailed::Untitled);
        };
        let Some(r) = doc.ready_mut().filter(|r| !r.info.lossy) else {
            return Err(SaveFailed::NotSavable);
        };
        let expected = (!overwrite).then_some(&r.info.stamp);
        let saved = save::save_path(
            &path,
            r.buffer.rope(),
            r.info.encoding,
            expected,
            &AtomicBool::new(false),
        )
        .map_err(SaveFailed::Error)?;
        r.info.stamp = saved.stamp;
        r.info.file_bytes = saved.bytes;
        r.history.mark_saved();
        doc.missing = false;
        doc.changed_on_disk = false;
        doc.discard = false;
        doc.encoding_changed = false;
        Ok(())
    }

    /// Writes tab `i` to `path` and makes that its file: the language follows the new name.
    pub fn save_as(
        &mut self,
        i: usize,
        path: &Path,
        service: Option<&HighlightService>,
        dark: bool,
    ) -> Result<(), SaveFailed> {
        let path = canonical_or_absolute(path);
        // Another tab already shows that file: it would then show stale text.
        if let Some(other) = self.index_of(&path).filter(|&o| o != i) {
            let name = self.tabs[other].name();
            return Err(SaveFailed::Error(SaveError::Io(format!(
                "{name} is open in another tab"
            ))));
        }
        let Some(doc) = self.tabs.get_mut(i) else {
            return Err(SaveFailed::NotSavable);
        };
        let Some(r) = doc.ready_mut().filter(|r| !r.info.lossy) else {
            return Err(SaveFailed::NotSavable);
        };
        let saved = save::save_path(
            &path,
            r.buffer.rope(),
            r.info.encoding,
            None,
            &AtomicBool::new(false),
        )
        .map_err(SaveFailed::Error)?;
        r.info.stamp = saved.stamp;
        r.info.file_bytes = saved.bytes;
        r.history.mark_saved();
        // The file may have been created just now: its real name is known only after that.
        let path = canonical_or_absolute(&path);
        r.language = language_of(&path, &r.buffer);
        r.colors = LineColors::default();
        r.colourable = false;
        doc.revision += 1;
        let (id, revision) = (doc.id, doc.revision);
        if let Some(s) = service {
            s.close(id);
        }
        let r = doc.ready_mut().expect("checked above");
        r.revision = revision;
        if let (Some(lang), Some(s)) = (r.language, service)
            && lang.limits().allows_file(r.buffer.len_bytes())
        {
            s.open(id, revision, r.buffer.snapshot(), lang, dark);
            r.colourable = true;
        }
        doc.path = Some(path.clone());
        doc.untitled = 0;
        doc.missing = false;
        doc.changed_on_disk = false;
        doc.discard = false;
        doc.encoding_changed = false;
        self.recents.add(path);
        self.watch_changed = true;
        Ok(())
    }

    /// Tabs with unsaved changes.
    pub fn modified_ids(&self) -> Vec<DocId> {
        self.tabs
            .iter()
            .filter(|d| d.modified())
            .map(|d| d.id)
            .collect()
    }

    // ---- toast -----------------------------------------------------------------------------

    pub fn set_toast(&mut self, text: &str) {
        self.toast = Some((
            text.to_string(),
            Instant::now() + Duration::from_millis(strings::TOAST_MS),
        ));
    }

    pub fn clear_toast(&mut self) {
        self.toast = None;
    }

    pub fn toast(&self, now: Instant) -> Option<&str> {
        self.toast
            .as_ref()
            .filter(|(_, until)| *until > now)
            .map(|(t, _)| t.as_str())
    }

    pub fn toast_expires(&self) -> Option<Instant> {
        self.toast.as_ref().map(|t| t.1)
    }

    /// The toast for a file shown with replaced bytes, or one that holds NUL characters (most
    /// likely not text), once per document.
    fn toast_lossy(&mut self, i: usize) {
        let doc = &mut self.tabs[i];
        let Some(info) = doc.ready().map(|r| &r.info) else {
            return;
        };
        let text = if info.lossy {
            strings::lossy_toast(&doc.name())
        } else if info.has_nul {
            strings::binary_toast(&doc.name())
        } else {
            return;
        };
        if !doc.lossy_toasted {
            doc.lossy_toasted = true;
            self.set_toast(&text);
        }
    }

    // ---- colours ---------------------------------------------------------------------------

    /// Takes in colours from the highlight worker. True if the active document's changed.
    pub fn colored(&mut self, results: Vec<Colored>) -> bool {
        let active = self.active_doc().map(|d| d.id);
        let mut changed = false;
        for c in results {
            let Some(doc) = self.tabs.iter_mut().find(|d| d.id == c.doc) else {
                continue;
            };
            if doc.revision != c.revision {
                continue;
            }
            let id = doc.id;
            if let Some(r) = doc.ready_mut().filter(|r| r.colourable) {
                r.colors.first = c.first_line;
                r.colors.lines = c.lines;
                r.colors.dark = c.dark;
                r.colors.generation += 1;
                changed |= Some(id) == active;
            }
        }
        changed
    }

    // ---- find ------------------------------------------------------------------------------

    /// Sets the active document's query. Searches at once up to `SYNC_FIND_BYTES`, else on a
    /// thread; then goes to the first match at or after the selection.
    pub fn find_set(&mut self, text: &str, options: FindOptions) {
        if let Some(i) = self.active {
            self.find_in(i, text, options, true, false);
        }
    }

    /// Searches the active document again after edits (its matches were only moved along).
    pub fn refind(&mut self) {
        if let Some(i) = self.active
            && let Some(r) = self.tabs[i].ready()
            && r.find.stale
        {
            let (text, options) = (r.find.text.clone(), r.find.options);
            self.find_in(i, &text, options, false, true);
        }
    }

    /// The active document's matches are waiting for a new search.
    pub fn find_stale(&self) -> bool {
        self.active_ready().is_some_and(|r| r.find.stale)
    }

    fn find_in(&mut self, i: usize, text: &str, options: FindOptions, go: bool, force: bool) {
        let (tx, waker) = (self.find_done.clone(), self.waker.clone());
        let doc = &mut self.tabs[i];
        let (id, revision) = (doc.id, doc.revision);
        let Some(r) = doc.ready_mut() else { return };
        if r.find.text == text && r.find.options == options && !force {
            return;
        }
        r.find.cancel();
        r.find.text = text.to_string();
        r.find.options = options;
        r.find.error = Query::check(text, options).err();
        r.find.matches.clear();
        r.find.capped = false;
        r.find.current = None;
        r.find.searching = false;
        r.find.stale = false;
        r.find.generation += 1;
        let Some(query) = Query::new(text, options) else {
            return;
        };
        if r.buffer.len_bytes() <= SYNC_FIND_BYTES {
            let m = find::find_all(r.buffer.rope(), &query, FIND_CAP, &AtomicBool::new(false));
            take_matches(r, m, go);
            return;
        }
        r.find.searching = true;
        let cancel = Arc::new(AtomicBool::new(false));
        r.find.cancel = Some(cancel.clone());
        let rope = r.buffer.snapshot();
        let text = text.to_string();
        std::thread::spawn(move || {
            let m = find::find_all(&rope, &query, FIND_CAP, &cancel);
            if m.cancelled {
                return;
            }
            tx.lock().unwrap().push(FindDone {
                doc: id,
                revision,
                text,
                options,
                kind: FindKind::All(m),
            });
            waker(Wake::Found);
        });
    }

    /// Goes to the next or previous match, wrapping. Past the last kept match of a capped
    /// search, `find_next` looks further from the caret.
    pub fn find_step(&mut self, forward: bool) {
        let (tx, waker) = (self.find_done.clone(), self.waker.clone());
        let Some(doc) = self.active_doc_mut() else {
            return;
        };
        let (id, revision) = (doc.id, doc.revision);
        let Some(r) = doc.ready_mut() else { return };
        let f = &r.find;
        if f.text.is_empty() || f.searching || f.matches.is_empty() {
            return;
        }
        let n = f.matches.len();
        let sel = r.sel.range();
        // The index to go to, or None to look past the kept matches from `from`.
        let (next, from) = match (f.current, forward) {
            (Some(i), true) if i + 1 < n => (Some(i + 1), 0),
            (Some(i), true) => ((!f.capped).then_some(0), f.matches[i].end),
            (Some(0), false) => ((!f.capped).then_some(n - 1), f.matches[0].start),
            (Some(i), false) => (Some(i - 1), 0),
            (None, true) => {
                let k = find::first_at_or_after(&f.matches, sel.end);
                if k < n {
                    (Some(k), 0)
                } else {
                    ((!f.capped).then_some(0), sel.end)
                }
            }
            (None, false) => {
                let k = find::first_at_or_after(&f.matches, sel.start);
                if k > 0 {
                    (Some(k - 1), 0)
                } else {
                    ((!f.capped).then_some(n - 1), sel.start)
                }
            }
        };
        if let Some(k) = next {
            r.find.current = Some(k);
            r.find.generation += 1;
            let range = r.find.matches[k].clone();
            r.select_and_reveal(range);
            return;
        }
        let Some(query) = Query::new(&r.find.text, r.find.options) else {
            return;
        };
        if r.buffer.len_bytes() <= SYNC_FIND_BYTES {
            let found = find::find_next(
                r.buffer.rope(),
                &query,
                from,
                forward,
                true,
                &AtomicBool::new(false),
            );
            take_next(r, found);
            return;
        }
        r.find.searching = true;
        r.find.generation += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        r.find.cancel = Some(cancel.clone());
        let rope = r.buffer.snapshot();
        let (text, options) = (r.find.text.clone(), r.find.options);
        std::thread::spawn(move || {
            let found = find::find_next(&rope, &query, from, forward, true, &cancel);
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            tx.lock().unwrap().push(FindDone {
                doc: id,
                revision,
                text,
                options,
                kind: FindKind::Next(found),
            });
            waker(Wake::Found);
        });
    }

    /// Takes in finished searches. True if the active document's changed.
    pub fn drain_finds(&mut self) -> bool {
        let active = self.active_doc().map(|d| d.id);
        let mut changed = false;
        let all = std::mem::take(&mut *self.find_done.lock().unwrap());
        for done in all {
            let Some(doc) = self.tabs.iter_mut().find(|d| d.id == done.doc) else {
                continue;
            };
            if doc.revision != done.revision {
                continue;
            }
            let id = doc.id;
            let Some(r) = doc.ready_mut() else { continue };
            if r.find.text != done.text || r.find.options != done.options {
                continue;
            }
            r.find.cancel = None;
            match done.kind {
                FindKind::All(m) => take_matches(r, m, Some(id) == active),
                FindKind::Next(found) => take_next(r, found),
            }
            changed |= Some(id) == active;
        }
        changed
    }

    /// Clears the query of every document.
    pub fn find_clear(&mut self) {
        for d in &mut self.tabs {
            if let Some(r) = d.ready_mut()
                && !r.find.text.is_empty()
            {
                r.find.cancel();
                let generation = r.find.generation + 1;
                r.find = FindState {
                    generation,
                    ..FindState::default()
                };
            }
        }
    }
}

fn take_matches(r: &mut Ready, m: Matches, go: bool) {
    r.find.searching = false;
    r.find.matches = m.ranges;
    r.find.capped = m.capped;
    r.find.generation += 1;
    if r.find.matches.is_empty() {
        r.find.current = None;
        return;
    }
    let k = find::first_at_or_after(&r.find.matches, r.sel.range().start);
    let k = if k >= r.find.matches.len() { 0 } else { k };
    r.find.current = Some(k);
    if go {
        let range = r.find.matches[k].clone();
        r.select_and_reveal(range);
    }
}

fn take_next(r: &mut Ready, found: Option<Range<usize>>) {
    r.find.searching = false;
    r.find.generation += 1;
    let Some(range) = found else { return };
    r.find.current = r.find.matches.iter().position(|m| *m == range);
    r.select_and_reveal(range);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: u32, end: u32, c: u8) -> Span {
        Span {
            start,
            end,
            color: [c, c, c],
        }
    }

    #[test]
    fn colour_spans_follow_an_edit() {
        // "let x = 1;" as [0,3) [3,8) [8,10)
        let base = vec![span(0, 3, 1), span(3, 8, 2), span(8, 10, 3)];
        // Typing inside the middle span grows it.
        let mut s = base.clone();
        shift_spans(&mut s, 5, 0, 2);
        assert_eq!(s, [span(0, 3, 1), span(3, 10, 2), span(10, 12, 3)]);
        // Typing at a boundary goes to the span that starts there.
        let mut s = base.clone();
        shift_spans(&mut s, 3, 0, 1);
        assert_eq!(s, [span(0, 3, 1), span(3, 9, 2), span(9, 11, 3)]);
        // Typing at the line's end grows the last span.
        let mut s = base.clone();
        shift_spans(&mut s, 10, 0, 4);
        assert_eq!(s, [span(0, 3, 1), span(3, 8, 2), span(8, 14, 3)]);
        // Removing across spans closes the gap; a span removed whole is dropped.
        let mut s = base.clone();
        shift_spans(&mut s, 2, 7, 0);
        assert_eq!(s, [span(0, 2, 1), span(2, 3, 3)]);
        // Replacing the whole line.
        let mut s = base.clone();
        shift_spans(&mut s, 0, 10, 3);
        assert_eq!(s, [span(0, 3, 1)]);
        // The rest of the line went to another line.
        let mut s = base.clone();
        shift_spans(&mut s, 4, usize::MAX / 2, 0);
        assert_eq!(s, [span(0, 3, 1), span(3, 4, 2)]);
        // Whatever the edit, spans stay sorted, contiguous and non-empty.
        for rel in 0..=10 {
            for removed in 0..=(10 - rel) {
                for inserted in 0..3 {
                    let mut s = base.clone();
                    shift_spans(&mut s, rel, removed, inserted);
                    let mut at = s.first().map_or(0, |x| x.start);
                    for x in &s {
                        assert!(
                            x.start == at && x.end > x.start,
                            "{rel} {removed} {inserted}"
                        );
                        at = x.end;
                    }
                    assert!(at as usize <= 10 - removed + inserted);
                }
            }
        }
    }

    fn file_docs(name: &str, text: &[u8]) -> (Docs, usize, PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("mdedit-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("file.txt");
        std::fs::write(&path, text).unwrap();
        let mut d = docs();
        let i = d.open(&path);
        d.load_now(i, None, false);
        (d, i, path, dir)
    }

    fn text_of(d: &Docs, i: usize) -> String {
        let r = d.tabs[i].ready().unwrap();
        let mut s = String::new();
        r.buffer.text(0..r.buffer.len_bytes(), &mut s);
        s
    }

    #[test]
    fn a_new_document_has_no_file_until_save_as() {
        let (mut d, i, path, dir) = file_docs("new", b"one\n");
        let n = d.new_untitled();
        assert_eq!(d.active(), Some(n));
        assert_eq!(d.tabs[n].name(), "Untitled 1");
        assert_eq!(d.labels(), ["file.txt", "Untitled 1"]);
        assert!(!d.tabs[n].modified());
        // It is not a session tab or a watched file, and nothing reads it.
        assert_eq!(d.paths(), [d.tabs[i].path.clone().unwrap()]);
        assert_eq!(d.active_among_paths(), 0);
        d.reload_active();
        d.check_changed();
        assert!(!d.tabs[n].is_loading() && !d.tabs[n].missing);
        // A second one takes the next number; a closed one's number is free again.
        let m = d.new_untitled();
        assert_eq!(d.tabs[m].name(), "Untitled 2");
        d.close(m, None);
        d.activate(n);
        assert!(d.edit(EditOp::Insert("\u{15f}"), None));
        assert!(d.edit(EditOp::Newline, None));
        assert!(d.tabs[n].modified());
        assert_eq!(d.labels(), ["file.txt", "\u{2022} Untitled 1"]);
        // Save needs a name first.
        assert!(matches!(d.save(n, false), Err(SaveFailed::Untitled)));
        // A file open in another tab is refused; a new name is taken.
        assert!(d.save_as(n, &path, None, false).is_err());
        let to = dir.join("made.rs");
        d.save_as(n, &to, None, false).unwrap();
        let eol = if cfg!(windows) { "\r\n" } else { "\n" };
        assert_eq!(
            std::fs::read(&to).unwrap(),
            format!("\u{15f}{eol}").as_bytes()
        );
        assert_eq!(d.tabs[n].name(), "made.rs");
        assert!(!d.tabs[n].modified() && d.tabs[n].path.is_some());
        assert_eq!(d.paths().len(), 2);
        assert_eq!(d.active_among_paths(), 1);
        d.save(n, false).unwrap();
        // The freed number is used again.
        let k = d.new_untitled();
        assert_eq!(d.tabs[k].name(), "Untitled 1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn line_commands_go_through_the_document() {
        let (mut d, i, _path, dir) = file_docs("lines", b"one\r\n  two  \r\n\tthree\r\n");
        let caret = |d: &mut Docs, at: usize| {
            d.tabs[i].ready_mut().unwrap().sel = Selection::at(at);
        };
        // A plain text file has no line comment: nothing changes and a toast says so.
        assert!(!d.edit(EditOp::ToggleComment, None));
        assert!(!d.tabs[i].modified());
        assert!(d.edit(EditOp::DuplicateLines, None));
        assert_eq!(text_of(&d, i), "one\r\none\r\n  two  \r\n\tthree\r\n");
        assert!(d.edit(EditOp::MoveLines(true), None));
        assert_eq!(text_of(&d, i), "one\r\n  two  \r\none\r\n\tthree\r\n");
        assert!(d.edit(EditOp::DeleteLines, None));
        assert_eq!(text_of(&d, i), "one\r\n  two  \r\n\tthree\r\n");
        assert!(d.edit(EditOp::TrimTrailing, None));
        assert_eq!(text_of(&d, i), "one\r\n  two\r\n\tthree\r\n");
        assert!(d.edit(EditOp::TabsToSpaces, None));
        assert_eq!(text_of(&d, i), "one\r\n  two\r\n    three\r\n");
        // Case needs a selection.
        caret(&mut d, 0);
        assert!(!d.edit(EditOp::Case(true), None));
        d.tabs[i].ready_mut().unwrap().sel = Selection {
            anchor: 0,
            caret: 3,
            goal_col: None,
        };
        assert!(d.edit(EditOp::Case(true), None));
        assert_eq!(text_of(&d, i), "ONE\r\n  two\r\n    three\r\n");
        // Each was one undo step, and the line-ending counts followed.
        let eol = d.tabs[i].ready().unwrap().info.eol;
        assert_eq!((eol.crlf, eol.lf, eol.cr), (3, 0, 0));
        for _ in 0..6 {
            assert!(d.edit(EditOp::Undo, None));
        }
        assert_eq!(text_of(&d, i), "one\r\n  two  \r\n\tthree\r\n");
        assert!(!d.tabs[i].modified());
        // With a language, the comment is its own.
        let rs = dir.join("a.rs");
        std::fs::write(&rs, "fn a() {}\n").unwrap();
        let k = d.open(&rs);
        d.load_now(k, None, false);
        assert!(d.edit(EditOp::ToggleComment, None));
        assert_eq!(text_of(&d, k), "// fn a() {}\n");
        assert!(d.edit(EditOp::ToggleComment, None));
        assert_eq!(text_of(&d, k), "fn a() {}\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replace_goes_through_the_document() {
        let (mut d, i, _path, dir) = file_docs("replace", b"cat Cat concat\r\ncat\r\n");
        let word = FindOptions {
            whole_word: true,
            ..FindOptions::default()
        };
        // The query selected the first match: Replace changes it, and the caret is after it.
        d.find_set("cat", word);
        assert_eq!(d.tabs[i].ready().unwrap().find.matches.len(), 3);
        assert!(d.edit(EditOp::ReplaceOne("dog"), None));
        assert_eq!(text_of(&d, i), "dog Cat concat\r\ncat\r\n");
        // The marks moved with the edit, so the next step selects the next match.
        d.find_step(true);
        assert_eq!(d.tabs[i].ready().unwrap().sel.range(), 4..7);
        // A selection that is not a match is left alone.
        d.tabs[i].ready_mut().unwrap().sel = Selection {
            anchor: 8,
            caret: 11,
            goal_col: None,
        };
        assert!(!d.edit(EditOp::ReplaceOne("dog"), None));
        // Replace all is one undo step and says how many.
        assert!(d.edit(EditOp::ReplaceAll("bird\n"), None));
        assert_eq!(text_of(&d, i), "dog bird\r\n concat\r\nbird\r\n\r\n");
        assert_eq!(d.toast.as_ref().unwrap().0, "Replaced 2 matches.");
        assert!(d.edit(EditOp::Undo, None));
        assert_eq!(text_of(&d, i), "dog Cat concat\r\ncat\r\n");
        // A regular expression with groups; one that does not compile changes nothing.
        let regex = FindOptions {
            regex: true,
            ..FindOptions::default()
        };
        d.find_set(r"(\w+) (\w+)", regex);
        assert!(d.edit(EditOp::ReplaceAll("$2 $1"), None));
        assert_eq!(text_of(&d, i), "Cat dog concat\r\ncat\r\n");
        d.find_set("(", regex);
        let r = d.tabs[i].ready().unwrap();
        assert!(r.find.error.is_some() && r.find.matches.is_empty());
        assert_eq!(r.find.label(), strings::BAD_PATTERN);
        assert!(!d.edit(EditOp::ReplaceAll("x"), None));
        // No match: nothing changes and a toast says so.
        d.find_set("zebra", FindOptions::default());
        assert!(!d.edit(EditOp::ReplaceAll("x"), None));
        assert_eq!(d.toast.as_ref().unwrap().0, strings::NOTHING_TO_REPLACE);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsaved_text_survives_a_restart() {
        let (mut d, i, path, dir) = file_docs("unsaved", b"one\r\ntwo\r\n");
        let backup = dir.join("backup");
        // Nothing unsaved: no list, and stray files in the folder go.
        std::fs::create_dir_all(&backup).unwrap();
        std::fs::write(backup.join("9.txt"), "old").unwrap();
        assert!(d.write_backups(&backup).unwrap().is_empty());
        assert!(!backup.join("9.txt").exists());
        // An edited file and an edited new document are both kept.
        d.tabs[i].ready_mut().unwrap().sel = Selection::at(0);
        assert!(d.edit(EditOp::Insert("zero\n"), None));
        let n = d.new_untitled();
        d.new_untitled();
        d.activate(n);
        assert!(d.edit(EditOp::Insert("note"), None));
        let list = d.write_backups(&backup).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!((list[0].path.as_ref(), list[0].untitled), (Some(&path), 0));
        assert_eq!((list[1].path.as_ref(), list[1].untitled), (None, 1));
        assert_eq!(std::fs::read_dir(&backup).unwrap().count(), 2);

        // The next run: the file's tab waits for its text, the new document has it at once.
        let mut e = docs();
        let k = e.add_pending(&path);
        e.tabs[k].restore = Some(backup.join(&list[0].file));
        assert!(e.tabs[k].modified());
        let text = std::fs::read_to_string(backup.join(&list[1].file)).unwrap();
        let u = e.restore_untitled(list[1].untitled, &text);
        assert_eq!((text_of(&e, u).as_str(), e.tabs[u].untitled), ("note", 1));
        assert!(e.tabs[u].modified());
        // Closing again before the file's tab was read keeps its backup as it is.
        let again = e.write_backups(&backup).unwrap();
        assert_eq!(again.len(), 2);
        assert_eq!(again[0], list[0]);
        assert!(backup.join(&list[0].file).is_file());
        assert_ne!(again[1].file, list[0].file);
        // Read: the unsaved text is back, the tab is modified, and Undo gives the file's text.
        e.load_now(k, None, false);
        assert_eq!(text_of(&e, k), "zero\none\r\ntwo\r\n");
        assert!(e.tabs[k].modified() && e.tabs[k].restore.is_none());
        let eol = e.tabs[k].ready().unwrap().info.eol;
        assert_eq!((eol.crlf, eol.lf), (2, 1));
        e.activate(k);
        assert!(e.edit(EditOp::Undo, None));
        assert_eq!(text_of(&e, k), "one\r\ntwo\r\n");
        assert!(!e.tabs[k].modified());
        // Saved or undone, nothing is left to keep.
        e.close(u, None);
        assert!(e.write_backups(&backup).unwrap().is_empty());
        assert_eq!(std::fs::read_dir(&backup).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn brackets_and_twin_words_come_from_the_selection() {
        let (mut d, i, _path, dir) = file_docs("aids", b"foo(bar) foo_bar foo\n");
        let set = |d: &mut Docs, a: usize, c: usize| {
            d.tabs[i].ready_mut().unwrap().sel = Selection {
                anchor: a,
                caret: c,
                goal_col: None,
            };
        };
        let r = |d: &Docs| d.tabs[i].ready().unwrap().brackets();
        set(&mut d, 3, 3);
        assert_eq!(r(&d), Some((3, 7)));
        set(&mut d, 8, 8);
        assert_eq!(r(&d), Some((7, 3)));
        set(&mut d, 0, 0);
        assert_eq!(r(&d), None);
        // An edit moves the pair: the answer is found again.
        set(&mut d, 4, 4);
        assert!(d.edit(EditOp::Insert("("), None));
        assert_eq!(r(&d), Some((4, 8)));
        assert!(d.edit(EditOp::Undo, None));
        // A selection has no bracket pair, and marks its twins when it is a whole word.
        set(&mut d, 0, 3);
        assert_eq!(r(&d), None);
        let w = |d: &Docs| d.tabs[i].ready().unwrap().twin_word();
        assert_eq!(w(&d).as_deref(), Some("foo"));
        set(&mut d, 0, 2);
        assert_eq!(w(&d), None);
        set(&mut d, 9, 12);
        assert_eq!(w(&d), None);
        set(&mut d, 3, 8);
        assert_eq!(w(&d), None);
        set(&mut d, 17, 20);
        assert_eq!(w(&d).as_deref(), Some("foo"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn line_endings_and_encodings_change_from_the_status_strip() {
        let (mut d, i, path, dir) = file_docs("status", "çay\r\nsu\n".as_bytes());
        // Every break becomes the chosen one, as one undo step.
        assert!(d.edit(EditOp::LineEndings("\n"), None));
        assert_eq!(text_of(&d, i), "çay\nsu\n");
        let eol = d.tabs[i].ready().unwrap().info.eol;
        assert_eq!((eol.crlf, eol.lf, eol.cr), (0, 2, 0));
        assert!(!d.edit(EditOp::LineEndings("\n"), None));
        assert!(d.edit(EditOp::Undo, None));
        assert_eq!(text_of(&d, i), "çay\r\nsu\n");
        assert!(!d.tabs[i].modified());
        // Saving in another encoding marks the tab and writes those bytes.
        assert!(!d.save_in(Encoding::Utf8));
        assert!(d.save_in(Encoding::Utf16Le));
        assert!(d.tabs[i].modified());
        d.save(i, false).unwrap();
        assert!(!d.tabs[i].modified());
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..4], [0xFF, 0xFE, 0xE7, 0x00]);
        // An encoding that cannot hold the text is refused with a toast.
        assert!(d.edit(EditOp::Insert("\u{4E2D}"), None));
        let latin = Encoding::Legacy(mdedit_core::encoding::legacy_for_code_page(1254));
        assert!(!d.save_in(latin));
        assert!(
            d.toast
                .as_ref()
                .unwrap()
                .0
                .contains("Windows-1254 cannot hold")
        );
        assert_eq!(d.tabs[i].ready().unwrap().info.encoding, Encoding::Utf16Le);
        assert!(d.edit(EditOp::Undo, None));
        // Reading the file again as another encoding: the bytes are the same, the text is not.
        d.tabs[i].forced = Some(latin);
        d.load_now(i, None, false);
        let r = d.tabs[i].ready().unwrap();
        assert_eq!(r.info.encoding, latin);
        assert!(text_of(&d, i).starts_with("\u{FF}\u{15F}\u{E7}\u{0}"));
        // The choice stays for the next load of this tab.
        d.load_now(i, None, false);
        assert_eq!(d.tabs[i].ready().unwrap().info.encoding, latin);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn folds_hide_lines_from_the_view_and_open_for_the_caret() {
        use crate::view::wrap::{self, Pos};
        let text = b"a {\n  b\n  c {\n    d\n  }\n}\ne\nf\ng\nh\n";
        let (mut d, i, _path, dir) = file_docs("fold", text);
        let r = d.tabs[i].ready_mut().unwrap();
        (r.advance, r.wide_advance, r.view_w, r.view_lines) = (10.0, 20.0, 400.0, 3.0);
        // On a line in no block nothing folds.
        r.sel = Selection::at(r.buffer.line_start(6));
        assert!(!r.toggle_fold());
        // Inside the outer block, on a line that heads none: the block around it folds and
        // the caret goes to the end of its first line.
        r.sel = Selection::at(r.buffer.line_start(1));
        assert!(r.toggle_fold());
        assert_eq!(r.folds.ranges(), [(0, 4)]);
        assert_eq!(r.sel, Selection::at(3));
        // Six rows are left: lines 0, 5, 6, 7, 8, 9 (the last is empty).
        assert_eq!(crate::view::shown_lines(r), 0..8);
        crate::view::scroll_rows(r, 2.0);
        assert_eq!(r.top, 6.0);
        crate::view::scroll_rows(r, -1.5);
        assert_eq!(r.top, 0.5);
        r.top = 0.0;
        crate::view::reveal(r, r.buffer.line_start(8), false);
        // Line 8 is row 4: with a row of margin, row 3 (line 7) is at the top.
        assert_eq!(r.top, 7.0);
        // With word wrap the rows of hidden lines are not walked.
        r.wrap = true;
        let at = |line, row| Pos { line, row };
        assert_eq!(wrap::step(r, at(0, 0), 2), at(6, 0));
        assert_eq!(wrap::step(r, at(6, 0), -1), at(5, 0));
        assert_eq!(wrap::step(r, at(5, 0), -9), at(0, 0));
        assert_eq!(wrap::rows_between(r, at(0, 0), at(7, 0), 10), Some(3));
        assert_eq!(wrap::vertical(r, 1, None, 1).0, r.buffer.line_start(5) + 1);
        r.wrap = false;
        // The fold's own line opens it again; a fold inside is taken in by one around it.
        assert!(r.toggle_fold());
        assert!(r.folds.is_empty());
        r.sel = Selection::at(r.buffer.line_start(2));
        assert!(r.toggle_fold());
        assert_eq!(r.folds.ranges(), [(2, 3)]);
        assert_eq!(r.fold_all(), (1, false));
        assert_eq!(r.folds.ranges(), [(0, 4)]);
        assert_eq!(r.sel, Selection::at(3));
        // Bringing a hidden place into view opens its fold, and so does a caret put there.
        crate::view::reveal(r, r.buffer.line_start(3), false);
        assert!(r.folds.is_empty());
        r.fold_all();
        r.sel = Selection::at(r.buffer.line_start(4));
        r.unfold_carets();
        assert!(r.folds.is_empty());
        // A line typed below leaves the fold; a line break in its first line opens it.
        r.sel = Selection::at(r.buffer.line_start(6));
        r.fold_all();
        d.edit(
            EditOp::Insert(
                "x
",
            ),
            None,
        );
        let r = d.tabs[i].ready_mut().unwrap();
        assert_eq!(r.folds.ranges(), [(0, 4)]);
        r.sel = Selection::at(0);
        d.edit(
            EditOp::Insert(
                "x
",
            ),
            None,
        );
        let r = d.tabs[i].ready_mut().unwrap();
        assert!(r.folds.is_empty());
        // A fold below an edit moves with its lines.
        r.sel = Selection::at(r.buffer.line_start(7));
        assert_eq!(r.fold_all(), (1, false));
        assert_eq!(r.folds.ranges(), [(1, 5)]);
        r.sel = Selection::at(0);
        d.edit(
            EditOp::Insert(
                "y
",
            ),
            None,
        );
        let r = d.tabs[i].ready_mut().unwrap();
        assert_eq!(r.folds.ranges(), [(2, 6)]);
        r.unfold_all();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn wrapped_rows_are_walked_from_where_the_view_is() {
        use crate::view::wrap::{self, Pos};
        let text = b"aaaa bbbb cccc dddd\nx\n0123456789abcdefghij\n";
        let (mut d, i, _path, dir) = file_docs("wrap", text);
        let r = d.tabs[i].ready_mut().unwrap();
        // Ten columns of 10 px fit beside the pad; four rows show.
        r.wrap = true;
        (r.advance, r.wide_advance, r.view_w, r.view_lines) = (10.0, 20.0, 116.0, 4.0);
        assert!(r.wrapped());
        let at = |line, row| Pos { line, row };
        let starts = |r: &mut Ready, line| -> Vec<usize> {
            wrap::rows(r, line).iter().map(|s| s.offset).collect()
        };
        assert_eq!(starts(r, 0), [0, 10]);
        assert_eq!(starts(r, 1), [20]);
        assert_eq!(starts(r, 2), [22, 32]);
        assert_eq!(starts(r, 3), [43]);
        assert_eq!(wrap::pos_of(r, 9), at(0, 0));
        assert_eq!(wrap::pos_of(r, 10), at(0, 1));
        assert_eq!(wrap::pos_of(r, 42), at(2, 1));
        assert_eq!(wrap::row(r, at(0, 1)).1, 19);
        assert_eq!(wrap::step(r, at(0, 0), 3), at(2, 0));
        assert_eq!(wrap::step(r, at(2, 1), -4), at(0, 0));
        assert_eq!(wrap::step(r, at(0, 0), 99), at(3, 0));
        assert_eq!(wrap::step(r, at(3, 0), -99), at(0, 0));
        assert_eq!(wrap::rows_between(r, at(0, 1), at(2, 1), 10), Some(3));
        assert_eq!(wrap::rows_between(r, at(0, 1), at(2, 1), 2), None);
        assert_eq!(wrap::rows_between(r, at(2, 1), at(0, 1), 10), None);
        // Down and up keep the column in the row.
        let mut caret = (3, None);
        let mut seen = Vec::new();
        for _ in 0..6 {
            let (o, goal) = wrap::vertical(r, caret.0, caret.1, 1);
            caret = (o, Some(goal));
            seen.push(o);
        }
        assert_eq!(seen, [13, 21, 25, 35, 43, 43]);
        assert_eq!(caret.1, Some(3));
        assert_eq!(wrap::vertical(r, 25, None, -1), (21, 3));
        assert_eq!(wrap::vertical(r, 3, None, -1), (0, 3));
        // The end of a row that goes on shows on the next row: the caret stays before it.
        assert_eq!(wrap::vertical(r, 42, None, -1), (31, 10));
        // Keeping an offset in view scrolls by rows, with a row of margin in a view of four.
        crate::view::reveal(r, 43, false);
        assert_eq!((r.top, r.top_row), (2.0, 0));
        crate::view::reveal(r, 12, false);
        assert_eq!((r.top, r.top_row), (0.0, 0));
        crate::view::reveal(r, 35, false);
        assert_eq!((r.top, r.top_row), (1.0, 0));
        // Parts of a row add up.
        wrap::set_top(r, at(0, 0));
        crate::view::scroll_rows(r, 0.5);
        assert_eq!((r.top, r.top_row), (0.0, 0));
        crate::view::scroll_rows(r, 0.5);
        assert_eq!((r.top, r.top_row), (0.0, 1));
        crate::view::scroll_rows(r, -5.0);
        assert_eq!((r.top, r.top_row), (0.0, 0));
        // An edit makes the rows anew.
        assert!(d.edit(EditOp::Insert("zz "), None));
        let r = d.tabs[i].ready_mut().unwrap();
        assert_eq!(starts(r, 0), [0, 8, 18]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn several_carets_type_remove_and_paste_together() {
        let (mut d, i, _path, dir) = file_docs("multi", b"one\ntwo\nthree\n");
        let r = d.tabs[i].ready_mut().unwrap();
        // A caret at the start of each of the three lines; the last added is the primary.
        r.sel = Selection::at(0);
        r.add_caret(4);
        r.add_caret(8);
        assert_eq!((r.sel.caret, r.extra.len()), (8, 2));
        assert_eq!(r.all_sels().1, 2);
        assert!(d.edit(EditOp::Insert("- "), None));
        assert_eq!(text_of(&d, i), "- one\n- two\n- three\n");
        let r = d.tabs[i].ready().unwrap();
        assert_eq!((r.sel.caret, r.extra.len()), (14, 2));
        assert!(d.edit(EditOp::Backspace, None));
        assert_eq!(text_of(&d, i), "-one\n-two\n-three\n");
        // As many pasted lines as carets: one each. Otherwise the whole text at each.
        assert!(d.edit(EditOp::Paste("1\r\n2\r\n3"), None));
        assert_eq!(text_of(&d, i), "-1one\n-2two\n-3three\n");
        assert!(d.edit(EditOp::Paste("ab"), None));
        assert_eq!(text_of(&d, i), "-1abone\n-2abtwo\n-3abthree\n");
        // One undo step each, and Undo leaves one caret.
        assert!(d.edit(EditOp::Undo, None));
        assert_eq!(text_of(&d, i), "-1one\n-2two\n-3three\n");
        assert!(d.tabs[i].ready().unwrap().extra.is_empty());
        // A column selection by keys: two lines down, three columns right, then typed over.
        let r = d.tabs[i].ready_mut().unwrap();
        r.sel = Selection::at(1);
        r.extend_column(2, 0);
        r.extend_column(0, 1);
        let (list, primary) = r.all_sels();
        let ranges: Vec<_> = list.iter().map(|s| s.range()).collect();
        assert_eq!(ranges, [1..2, 7..8, 13..14]);
        assert_eq!(primary, 2);
        assert!(d.edit(EditOp::Insert("#"), None));
        assert_eq!(text_of(&d, i), "-#one\n-#two\n-#three\n");
        // A command that is not typing goes on with the primary alone.
        assert!(d.edit(EditOp::DuplicateLines, None));
        assert!(d.tabs[i].ready().unwrap().extra.is_empty());
        assert_eq!(text_of(&d, i), "-#one\n-#two\n-#three\n-#three\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bookmarks_toggle_follow_edits_and_go_round() {
        let (mut d, i, _path, dir) = file_docs("marks", b"a\nb\nc\nd\n");
        assert!(!d.go_to_bookmark(true));
        for line in [2, 4] {
            d.go_to(line, 1);
            d.tabs[i].ready_mut().unwrap().toggle_bookmark();
        }
        assert_eq!(d.tabs[i].ready().unwrap().bookmarks, [1, 3]);
        let caret_line = |d: &Docs| {
            let r = d.tabs[i].ready().unwrap();
            r.buffer.line_of(r.sel.caret)
        };
        assert!(d.go_to_bookmark(true));
        assert_eq!(caret_line(&d), 1);
        assert!(d.go_to_bookmark(true));
        assert_eq!(caret_line(&d), 3);
        assert!(d.go_to_bookmark(false));
        assert_eq!(caret_line(&d), 1);
        assert!(d.go_to_bookmark(false));
        assert_eq!(caret_line(&d), 3);
        // A line typed above moves both; Undo moves them back.
        d.go_to(1, 1);
        assert!(d.edit(EditOp::Newline, None));
        assert_eq!(d.tabs[i].ready().unwrap().bookmarks, [2, 4]);
        assert!(d.edit(EditOp::Undo, None));
        assert_eq!(d.tabs[i].ready().unwrap().bookmarks, [1, 3]);
        d.go_to(2, 1);
        d.tabs[i].ready_mut().unwrap().toggle_bookmark();
        assert_eq!(d.tabs[i].ready().unwrap().bookmarks, [3]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn go_to_clamps_to_the_text() {
        let (mut d, i, _path, dir) = file_docs("goto", "one\nçay two\nthree".as_bytes());
        let caret = |d: &Docs| d.tabs[i].ready().unwrap().sel.caret;
        assert!(d.go_to(2, 1));
        assert_eq!(caret(&d), 4);
        // A column inside a character goes to its start; one past the end, to the line's end.
        assert!(d.go_to(2, 2));
        assert_eq!(caret(&d), 4);
        assert!(d.go_to(2, 99));
        assert_eq!(caret(&d), 12);
        assert!(d.go_to(0, 0));
        assert_eq!(caret(&d), 0);
        assert!(d.go_to(99, 3));
        assert_eq!(caret(&d), 15);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn typing_marks_the_tab_and_saving_clears_it() {
        let (mut d, i, path, dir) = file_docs("edit", b"one\r\ntwo\r\n");
        assert!(!d.tabs[i].modified());
        assert_eq!(d.labels(), ["file.txt"]);
        let rev = d.tabs[i].revision;
        assert!(d.edit(EditOp::Insert("X"), None));
        assert!(d.tabs[i].modified());
        assert_eq!(d.labels(), ["\u{2022} file.txt"]);
        assert_eq!(d.tabs[i].title(), "\u{2022} file.txt");
        assert!(d.tabs[i].revision > rev);
        assert_eq!(d.tabs[i].ready().unwrap().revision, d.tabs[i].revision);
        // Enter uses the file's line ending; a paste is converted to it.
        assert!(d.edit(EditOp::Newline, None));
        assert!(d.edit(EditOp::Paste("a\nb\rc"), None));
        assert_eq!(text_of(&d, i), "X\r\na\r\nb\r\ncone\r\ntwo\r\n");
        let eol = d.tabs[i].ready().unwrap().info.eol;
        assert_eq!((eol.crlf, eol.lf, eol.cr), (5, 0, 0));
        // The file is untouched until it is saved.
        assert_eq!(std::fs::read(&path).unwrap(), b"one\r\ntwo\r\n");
        d.save(i, false).unwrap();
        assert!(!d.tabs[i].modified());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"X\r\na\r\nb\r\ncone\r\ntwo\r\n"
        );
        // The save is not seen as a change on disk.
        d.check_changed();
        assert!(!d.tabs[i].is_loading() && !d.tabs[i].changed_on_disk);
        // Undo goes back past the save point: modified again; redo returns to it.
        assert!(d.edit(EditOp::Undo, None));
        assert!(d.tabs[i].modified());
        assert!(d.edit(EditOp::Redo, None));
        assert!(!d.tabs[i].modified());
        assert!(!d.edit(EditOp::Redo, None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_change_on_disk_does_not_replace_unsaved_text() {
        let (mut d, i, path, dir) = file_docs("disk", b"one\n");
        d.edit(EditOp::Insert("mine "), None);
        std::fs::write(&path, "theirs, and longer\n").unwrap();
        d.check_changed();
        assert!(d.tabs[i].changed_on_disk && !d.tabs[i].is_loading());
        assert_eq!(d.tabs[i].banner(), Some(strings::FILE_CHANGED));
        assert_eq!(text_of(&d, i), "mine one\n");
        // Saving refuses until told to overwrite.
        assert!(matches!(
            d.save(i, false),
            Err(SaveFailed::Error(SaveError::ChangedOnDisk))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"theirs, and longer\n");
        d.save(i, true).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"mine one\n");
        assert!(!d.tabs[i].changed_on_disk && d.tabs[i].banner().is_none());
        // Without unsaved changes a change on disk reloads, as before.
        std::fs::write(&path, "theirs again, longer still\n").unwrap();
        d.check_changed();
        assert!(d.tabs[i].is_loading());
        d.wait_load(Instant::now() + Duration::from_secs(10), None, false);
        assert_eq!(text_of(&d, i), "theirs again, longer still\n");
        assert!(!d.tabs[i].modified());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_with_replaced_bytes_is_read_only() {
        // UTF-16 with an odd final byte.
        let (mut d, i, _, dir) = file_docs("lossy", &[0xFF, 0xFE, b'a', 0, b'b']);
        assert!(d.tabs[i].ready().unwrap().info.lossy);
        d.clear_toast();
        assert!(!d.edit(EditOp::Insert("x"), None));
        assert!(
            d.toast(Instant::now())
                .is_some_and(|t| t.contains("read-only"))
        );
        assert!(matches!(d.save(i, false), Err(SaveFailed::NotSavable)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_as_moves_the_tab_to_the_new_file() {
        let (mut d, i, path, dir) = file_docs("saveas", b"fn main() {}\n");
        d.edit(EditOp::Insert("// x\n"), None);
        let to = dir.join("copy.rs");
        d.save_as(i, &to, None, false).unwrap();
        assert_eq!(d.tabs[i].name(), "copy.rs");
        assert!(!d.tabs[i].modified());
        assert_eq!(std::fs::read(&to).unwrap(), b"// x\nfn main() {}\n");
        assert_eq!(std::fs::read(&path).unwrap(), b"fn main() {}\n");
        let r = d.tabs[i].ready().unwrap();
        assert_eq!(r.language.map(Language::name), Some("Rust"));
        // Onto a file another tab shows: refused.
        let j = d.open(&path);
        d.load_now(j, None, false);
        assert!(d.save_as(j, &to, None, false).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_marks_move_with_edits() {
        let (mut d, i, _, dir) = file_docs("find", b"ab ab ab ab\n");
        d.find_set("ab", FindOptions::default());
        let marks = |d: &Docs| d.tabs[i].ready().unwrap().find.matches.clone();
        assert_eq!(marks(&d), [0..2, 3..5, 6..8, 9..11]);
        // find_set selected the first match; put the caret inside the second and type.
        d.tabs[i].ready_mut().unwrap().sel = Selection::at(4);
        d.edit(EditOp::Insert("X"), None);
        assert_eq!(marks(&d), [0..2, 7..9, 10..12]);
        assert!(d.find_stale());
        // Typing right before a match moves it; right after one leaves it.
        d.tabs[i].ready_mut().unwrap().sel = Selection::at(7);
        d.edit(EditOp::Insert("Y"), None);
        assert_eq!(marks(&d), [0..2, 8..10, 11..13]);
        d.tabs[i].ready_mut().unwrap().sel = Selection::at(2);
        d.edit(EditOp::Insert("Z"), None);
        assert_eq!(marks(&d), [0..2, 9..11, 12..14]);
        // The new search finds what the text now holds.
        d.refind();
        assert!(!d.find_stale());
        assert_eq!(text_of(&d, i), "abZ aXb Yab ab\n");
        assert_eq!(marks(&d), [0..2, 9..11, 12..14]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn colours_window_follows_line_changes() {
        let mut c = LineColors {
            first: 10,
            lines: (0..5).map(|k| vec![span(0, 4, k)]).collect(),
            dark: false,
            generation: 1,
            requested: Some((0..20, false)),
        };
        let change = |first_line, lines_before, lines_after| Change {
            first_line,
            lines_before,
            lines_after,
            ..Change::default()
        };
        // A line added above the window: it moves down.
        c.edited(&change(3, 1, 2), 0);
        assert_eq!((c.first, c.lines.len()), (11, 5));
        assert!(c.requested.is_none());
        // Line 12 split in two at byte 2: its start keeps its colour, the new line is plain.
        c.edited(&change(12, 1, 2), 2);
        assert_eq!(c.lines.len(), 6);
        assert_eq!(c.lines[1], [span(0, 2, 1)]);
        assert!(c.lines[2].is_empty());
        assert_eq!(c.lines[3], [span(0, 4, 2)]);
        // Three lines joined into one.
        c.edited(&change(13, 3, 1), 0);
        assert_eq!(c.lines.len(), 4);
        // Below the window: nothing moves.
        c.edited(&change(40, 1, 5), 0);
        assert_eq!((c.first, c.lines.len()), (11, 4));
        // A change that runs into the window from above drops it.
        c.edited(&change(9, 4, 1), 0);
        assert!(c.line(11, false).is_none());
    }

    #[test]
    fn line_colors_window() {
        let mut c = LineColors::default();
        assert!(c.line(0, false).is_none());
        c.first = 10;
        c.lines = vec![Vec::new(); 5];
        c.dark = true;
        c.generation = 1;
        assert!(c.line(9, true).is_none());
        assert!(c.line(10, true).is_some());
        assert!(c.line(14, true).is_some());
        assert!(c.line(15, true).is_none());
        assert!(c.line(12, false).is_none());
        assert!(c.covers(&(10..15), true));
        assert!(!c.covers(&(10..16), true));
        assert!(!c.covers(&(10..15), false));
    }

    #[test]
    fn find_labels() {
        let mut f = FindState::default();
        assert_eq!(f.label(), "");
        f.text = "x".into();
        assert_eq!(f.label(), "No matches");
        f.searching = true;
        assert_eq!(f.label(), "Searching");
        f.searching = false;
        f.matches = vec![0..1, 4..5];
        assert_eq!(f.label(), "2 matches");
        f.current = Some(1);
        assert_eq!(f.label(), "2 of 2");
    }

    fn docs() -> Docs {
        Docs::new(1254, Arc::new(|_| {}))
    }

    #[test]
    fn a_file_that_never_existed_has_no_gone_banner() {
        let dir = std::env::temp_dir().join(format!("mdedit-gone-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut d = docs();
        let i = d.open(&dir.join("never-there.txt"));
        d.load_now(i, None, false);
        let doc = &d.tabs[i];
        assert!(matches!(doc.state, DocState::Failed(_)));
        assert!(doc.missing);
        assert!(!doc.shows_gone_banner());
        // A file read and then deleted shows the banner over its text.
        let path = dir.join("was-there.txt");
        std::fs::write(&path, "text\n").unwrap();
        let j = d.open(&path);
        d.load_now(j, None, false);
        assert!(d.tabs[j].ready().is_some());
        std::fs::remove_file(&path).unwrap();
        d.check_changed();
        assert!(d.tabs[j].shows_gone_banner());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layouts_follow_the_text_not_the_reload() {
        // A reload bumps the document's revision while the old text stays; the text's own
        // revision changes only when the new text is taken in.
        let dir = std::env::temp_dir().join(format!("mdedit-rev-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notes.txt");
        std::fs::write(&path, "one\ntwo\n").unwrap();
        let mut d = docs();
        let i = d.open(&path);
        d.load_now(i, None, false);
        let first = d.tabs[i].ready().unwrap().revision;
        assert_eq!(first, d.tabs[i].revision);
        std::fs::write(&path, "one\nTWO\n").unwrap();
        d.start_load(i);
        assert_ne!(d.tabs[i].revision, first);
        assert_eq!(d.tabs[i].ready().unwrap().revision, first);
        let applied = d.wait_load(Instant::now() + Duration::from_secs(10), None, false);
        assert!(applied.active_ready);
        let r = d.tabs[i].ready().unwrap();
        assert_eq!(r.revision, d.tabs[i].revision);
        assert_ne!(r.revision, first);
        let mut line = String::new();
        r.buffer.line_text(1, &mut line);
        assert_eq!(line, "TWO");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn language_lookup_reads_only_the_start_of_the_first_line() {
        let long = format!("#!/usr/bin/env python\n{}", "x".repeat(10));
        let buf = Buffer::from_text(&long);
        let lang = language_of(Path::new("script"), &buf);
        assert_eq!(lang.map(Language::name), Some("Python"));
        // One line of 3 MB: found by extension, and the first line is not copied whole (the
        // lookup sees at most FIRST_LINE_BYTES, which this checks through a mode line placed
        // past them).
        let mut one = "a".repeat(3 << 20);
        one.push_str(" -*- mode: python -*-");
        let buf = Buffer::from_text(&one);
        assert_eq!(
            language_of(Path::new("data.json"), &buf).map(Language::name),
            Some("JSON")
        );
        assert!(language_of(Path::new("data.unknown"), &buf).is_none());
    }

    #[test]
    fn tabs_open_close_move() {
        let dir = std::env::temp_dir().join(format!("mdedit-docs-{}", std::process::id()));
        let mut d = docs();
        let a = d.add_pending(&dir.join("a.txt"));
        let b = d.add_pending(&dir.join("b.txt"));
        assert_eq!((a, b), (0, 1));
        // File names compare without case on Windows only: there "A.txt" is the tab "a.txt".
        let upper = d.add_pending(&dir.join("A.txt"));
        assert_eq!(upper, if cfg!(windows) { 0 } else { 2 });
        let c = d.add_pending(&dir.join("c.txt"));
        assert_eq!(c, if cfg!(windows) { 2 } else { 3 });
        d.active = Some(c);
        assert!(d.move_tab(c, 0));
        assert_eq!(d.active(), Some(0));
        assert_eq!(d.tabs[0].name(), "c.txt");
        d.close(0, None);
        assert_eq!(d.tabs.len(), if cfg!(windows) { 2 } else { 3 });
        assert_eq!(d.active(), Some(0));
        d.close_all(None);
        assert!(d.tabs.is_empty() && d.active().is_none());
    }
}
