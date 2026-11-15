//! The results panel under the text: what "Find in files" and "Find in open tabs" found, a row
//! for each file and under it a row for each match. A folder is searched on a worker thread
//! (`mdedit_core::search::in_tree`), whose events are taken in as they come; a click on a match
//! opens the file there.

use crate::app::Wake;
use crate::chrome::{Ctx, glyph};
use crate::docs::Waker;
use crate::ui::{Drag, Hit, Scroller, Ui};
use mdedit_core::find::Query;
use mdedit_core::load::LoadOptions;
use mdedit_core::search::{self, Event, Hit as Found, NameFilter, TreeOptions};
use mdedit_highlight::DocId;
use mdshared_ui::gfx::{Face, Rect, mix};
use mdshared_ui::pane::draw_row_fill;
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};
use mdshared_ui::textfield::TextField;
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The panel's height, logical px, and what it leaves the text at least.
pub const RESULTS_H: f32 = 230.0;
/// The least the panel can be dragged to.
pub const RESULTS_MIN_H: f32 = 110.0;
pub const RESULTS_TEXT_MIN: f32 = 140.0;
/// The panel's title row.
const HEAD: f32 = 34.0;
/// The row under the title row with the boxes for the folder and the kinds of file.
const SCOPE_H: f32 = 34.0;
/// A row of the list and the part of it that is filled.
pub const ROW: f32 = 25.0;
const ROW_H: f32 = 23.0;
/// Matches kept of one open tab.
pub const TAB_HITS_MAX: usize = 2000;
/// An open tab with more text than this is not searched by "Find in open tabs", which runs
/// while the window waits.
pub const TAB_BYTES_MAX: usize = 64 << 20;
/// A document with matches listed that has been edited is searched again this long after its
/// last change.
pub const REFRESH_AFTER: Duration = Duration::from_millis(300);
/// The worker wakes the window at most this often while files with matches come in.
const WAKE_EVERY: Duration = Duration::from_millis(40);

/// What was found in one open tab: its id, name and file, the matches, and whether there were
/// more.
pub type TabHits = (DocId, String, Option<PathBuf>, Vec<Found>, bool);

/// A key that moves the row the keyboard is on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nav {
    Up,
    Down,
    Home,
    End,
    /// A page, in rows.
    PageUp(usize),
    PageDown(usize),
}

pub enum Row {
    /// A file with matches: where it is, or the open tab it is (a new document has no file).
    File {
        path: Option<PathBuf>,
        doc: Option<DocId>,
        name: String,
        count: usize,
        more: bool,
        /// The document was edited after these matches were found: they are not offered as
        /// places to go to until it is searched again.
        stale: bool,
    },
    Hit(Found),
}

/// One open tab as the panel needs to see it to keep its rows true.
pub struct TabRev<'a> {
    pub id: DocId,
    pub path: Option<&'a Path>,
    /// Changes with every edit of the text.
    pub revision: u64,
    pub modified: bool,
}

/// What the panel knows of an open tab whose matches it lists.
struct Seen {
    /// The revision the listed matches are of; `u64::MAX` for matches read from a file that
    /// the tab has changed since.
    revision: u64,
    /// The revision the next search is waited for, and when it starts.
    due_for: Option<u64>,
    due: Option<Instant>,
    /// The revision a search on a worker is running for.
    job: Option<u64>,
}

/// The answer of a search of one tab again, for the revision it started at.
struct Refreshed {
    epoch: u64,
    doc: DocId,
    path: Option<PathBuf>,
    revision: u64,
    hits: Vec<Found>,
    more: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Idle,
    Running {
        files: usize,
    },
    Done {
        skipped: usize,
        truncated: bool,
        cancelled: bool,
    },
}

pub struct Results {
    pub open: bool,
    pub rows: Vec<Row>,
    pub hits: usize,
    pub files: usize,
    pub state: State,
    /// The boxes of a folder search: the folder (empty: the open folder) and the kinds of file
    /// (patterns like `*.md;*.txt`; empty: every file). They keep what was typed from one search
    /// to the next.
    pub folder: TextField,
    pub kinds: TextField,
    /// What was looked for and where, for the title row.
    pub what: String,
    pub place: String,
    /// The panel's height the user dragged it to, logical px.
    pub height: f32,
    pub scroll: f32,
    pub content: f32,
    pub view: f32,
    /// The row last gone to, or the one the keyboard is on.
    pub selected: Option<usize>,
    /// The panel has the keyboard: Up, Down, Enter and Escape act on its rows.
    pub focused: bool,
    /// Counts every change of what the panel shows.
    pub generation: u64,
    root: PathBuf,
    /// What was looked for, to search a changed tab again.
    query: Option<Query>,
    /// The file rows by file and by tab, and what is known of each open tab with matches.
    by_path: HashMap<PathBuf, usize>,
    by_doc: HashMap<DocId, usize>,
    seen: HashMap<DocId, Seen>,
    /// Counts the searches; an answer of an older one is dropped.
    epoch: u64,
    /// Files with stale rows.
    stale_files: usize,
    refreshed: Arc<Mutex<Vec<Refreshed>>>,
    /// Replace in files: the box, the steps and the report of the panel.
    pub replace: crate::replace_ui::Replacing,
    /// What the worker has found and the window has not taken in yet. A list under a lock, not
    /// a channel, which costs the exe some 15 KB for each type sent.
    rx: Option<Arc<Mutex<Vec<Event>>>>,
    cancel: Arc<AtomicBool>,
}

impl Default for Results {
    fn default() -> Results {
        Results {
            open: false,
            rows: Vec::new(),
            hits: 0,
            files: 0,
            state: State::Idle,
            folder: TextField::new(),
            kinds: TextField::new(),
            what: String::new(),
            place: String::new(),
            height: RESULTS_H,
            scroll: 0.0,
            content: 0.0,
            view: 0.0,
            selected: None,
            focused: false,
            generation: 0,
            root: PathBuf::new(),
            query: None,
            by_path: HashMap::new(),
            by_doc: HashMap::new(),
            seen: HashMap::new(),
            epoch: 0,
            stale_files: 0,
            refreshed: Arc::new(Mutex::new(Vec::new())),
            replace: Default::default(),
            rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// The name a folder or file is shown by.
fn short(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

impl Results {
    fn reset(&mut self, what: &str, place: String) {
        // A search still running stops; its events go nowhere.
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.rx = None;
        self.rows.clear();
        self.by_path.clear();
        self.by_doc.clear();
        self.seen.clear();
        self.stale_files = 0;
        self.epoch += 1;
        self.refreshed.lock().unwrap().clear();
        (self.hits, self.files) = (0, 0);
        self.what = what.to_string();
        self.place = place;
        self.scroll = 0.0;
        self.selected = None;
        self.open = true;
        self.generation += 1;
    }

    /// Starts a search of `root` on a worker thread.
    pub fn start_tree(
        &mut self,
        root: &Path,
        query: Query,
        load: LoadOptions,
        filter: NameFilter,
        waker: Waker,
    ) {
        let place = if filter.is_empty() {
            format!("in {}", short(root))
        } else {
            format!("in {} ({})", short(root), self.kinds.text().trim())
        };
        self.reset(query.text(), place);
        self.query = Some(query.clone());
        self.replace
            .searched(root, query.clone(), load, filter.clone());
        self.root = root.to_path_buf();
        self.state = State::Running { files: 0 };
        let tx = Arc::new(Mutex::new(Vec::new()));
        self.rx = Some(tx.clone());
        let cancel = self.cancel.clone();
        let root = root.to_path_buf();
        std::thread::spawn(move || {
            let mut woke = Instant::now();
            let opts = TreeOptions::default();
            let named = (!filter.is_empty()).then_some(&filter);
            search::in_tree_named(&root, &query, &load, &opts, named, &cancel, &mut |event| {
                let done = matches!(event, Event::Done { .. });
                tx.lock().unwrap().push(event);
                if done || woke.elapsed() >= WAKE_EVERY {
                    woke = Instant::now();
                    waker(Wake::Results);
                }
            });
        });
    }

    /// Shows what was found in the open tabs: for each, its id, name, file and matches.
    pub fn set_tabs(&mut self, what: &str, tabs: Vec<TabHits>, left_out: bool) {
        self.reset(what, "in the open tabs".into());
        self.replace.clear();
        let mut truncated = left_out;
        for (id, name, path, hits, more) in tabs {
            if hits.is_empty() {
                continue;
            }
            truncated |= more;
            self.push_file(path, Some(id), name, hits, more);
        }
        self.state = State::Done {
            skipped: 0,
            truncated,
            cancelled: false,
        };
    }

    /// Tells the panel what was searched for in the tabs, and each tab's revision at that
    /// time, so that a later edit of a tab is noticed.
    pub fn set_tab_search(&mut self, query: Query, revisions: &[(DocId, u64)]) {
        self.query = Some(query);
        for &(doc, revision) in revisions {
            self.seen.insert(
                doc,
                Seen {
                    revision,
                    due_for: None,
                    due: None,
                    job: None,
                },
            );
        }
    }

    fn push_file(
        &mut self,
        path: Option<PathBuf>,
        doc: Option<DocId>,
        name: String,
        hits: Vec<Found>,
        more: bool,
    ) {
        self.files += 1;
        self.hits += hits.len();
        if let Some(p) = &path {
            self.by_path.insert(p.clone(), self.rows.len());
        }
        if let Some(d) = doc {
            self.by_doc.insert(d, self.rows.len());
        }
        self.rows.push(Row::File {
            path,
            doc,
            name,
            count: hits.len(),
            more,
            stale: false,
        });
        self.rows.extend(hits.into_iter().map(Row::Hit));
    }

    /// Takes in what the worker has sent. True when the panel changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = self.take_refreshed();
        let Some(rx) = self.rx.take() else {
            return changed;
        };
        let mut finished = false;
        let events = std::mem::take(&mut *rx.lock().unwrap());
        for event in events {
            changed = true;
            match event {
                Event::File { path, hits, more } => {
                    let name = path
                        .strip_prefix(&self.root)
                        .ok()
                        .filter(|p| !p.as_os_str().is_empty())
                        .map_or_else(|| short(&path), |p| p.display().to_string());
                    self.push_file(Some(path), None, name, hits, more);
                }
                Event::Progress { files } => self.state = State::Running { files },
                Event::Done {
                    skipped,
                    truncated,
                    cancelled,
                    ..
                } => {
                    self.state = State::Done {
                        skipped,
                        truncated,
                        cancelled,
                    };
                    finished = true;
                }
            }
        }
        if !finished {
            self.rx = Some(rx);
        }
        if changed {
            self.generation += 1;
        }
        changed
    }

    /// The file row of a tab: by its id (matches found in the open tabs) or by its file.
    fn file_row(&self, id: DocId, path: Option<&Path>) -> Option<usize> {
        self.by_doc
            .get(&id)
            .or_else(|| path.and_then(|p| self.by_path.get(p)))
            .copied()
    }

    /// The rows after file row `file` up to the next file row.
    fn extent(&self, file: usize) -> Range<usize> {
        let end = self.rows[file + 1..]
            .iter()
            .position(|r| matches!(r, Row::File { .. }))
            .map_or(self.rows.len(), |n| file + 1 + n);
        file + 1..end
    }

    fn set_stale(&mut self, file: usize, on: bool) {
        if let Some(Row::File { stale, .. }) = self.rows.get_mut(file)
            && *stale != on
        {
            *stale = on;
            self.stale_files = if on {
                self.stale_files + 1
            } else {
                self.stale_files.saturating_sub(1)
            };
            self.generation += 1;
        }
    }

    /// True when the match row, or the file it belongs to, was found before the document was
    /// edited.
    pub fn is_stale(&self, row: usize) -> bool {
        if self.stale_files == 0 || row >= self.rows.len() {
            return false;
        }
        self.rows[..=row]
            .iter()
            .rev()
            .find_map(|r| match r {
                Row::File { stale, .. } => Some(*stale),
                Row::Hit(_) => None,
            })
            .unwrap_or(false)
    }

    /// Keeps the rows of the open tabs true to their text. A tab edited since its matches were
    /// found has its rows marked, is searched again on a worker once it has been left alone for
    /// `REFRESH_AFTER`, and its rows are replaced when the answer comes. `snapshot` gives a copy
    /// of a tab's text to search. Returns when it wants to be called again.
    pub fn sync<'a>(
        &mut self,
        tabs: impl Iterator<Item = TabRev<'a>>,
        now: Instant,
        snapshot: &dyn Fn(DocId) -> Option<mdedit_core::Buffer>,
        waker: &Waker,
    ) -> Option<Instant> {
        if !self.open || self.rows.is_empty() || self.query.is_none() {
            return None;
        }
        let mut next: Option<Instant> = None;
        for t in tabs {
            let Some(file) = self.file_row(t.id, t.path) else {
                continue;
            };
            let seen = self.seen.entry(t.id).or_insert(Seen {
                // Matches read from the file are of the tab's text only when it is unchanged.
                revision: if t.modified { u64::MAX } else { t.revision },
                due_for: None,
                due: None,
                job: None,
            });
            if seen.revision == t.revision {
                seen.due_for = None;
                seen.due = None;
                continue;
            }
            if seen.due_for != Some(t.revision) {
                seen.due_for = Some(t.revision);
                seen.due = Some(now + REFRESH_AFTER);
            }
            let due = seen.due.unwrap_or(now);
            let start = seen.job.is_none() && now >= due;
            if start {
                seen.job = Some(t.revision);
                seen.due = None;
            } else if seen.job.is_none() {
                next = Some(next.map_or(due, |n| n.min(due)));
            }
            self.set_stale(file, true);
            if start && let (Some(query), Some(buf)) = (self.query.clone(), snapshot(t.id)) {
                let (epoch, doc, revision) = (self.epoch, t.id, t.revision);
                let path = t.path.map(Path::to_path_buf);
                let (out, waker) = (self.refreshed.clone(), waker.clone());
                std::thread::spawn(move || {
                    let cancel = AtomicBool::new(false);
                    let (hits, more) = search::in_buffer(&buf, &query, TAB_HITS_MAX, &cancel);
                    out.lock().unwrap().push(Refreshed {
                        epoch,
                        doc,
                        path,
                        revision,
                        hits,
                        more,
                    });
                    waker(Wake::Results);
                });
            }
        }
        next
    }

    /// Puts the answers of the searches of single tabs into the rows. True when any did.
    fn take_refreshed(&mut self) -> bool {
        let list = std::mem::take(&mut *self.refreshed.lock().unwrap());
        let mut changed = false;
        let epoch = self.epoch;
        for r in list.into_iter().filter(|r| r.epoch == epoch) {
            if let Some(seen) = self.seen.get_mut(&r.doc) {
                seen.job = None;
                seen.revision = r.revision;
            }
            if let Some(file) = self.file_row(r.doc, r.path.as_deref()) {
                self.replace_file(file, r.hits, r.more);
                changed = true;
            }
        }
        changed
    }

    /// Replaces the matches under file row `file`; a file left with none loses its row.
    fn replace_file(&mut self, file: usize, hits: Vec<Found>, more: bool) {
        let old = self.extent(file);
        let (removed, added) = (old.len(), hits.len());
        if let Some(Row::File { stale: true, .. }) = self.rows.get(file) {
            self.stale_files = self.stale_files.saturating_sub(1);
        }
        self.hits = self.hits + added - removed;
        self.rows
            .splice(old.clone(), hits.into_iter().map(Row::Hit));
        // The row the keyboard was on keeps its place when it is before the changed rows or after
        // them, and goes when it was among them.
        let gone = usize::from(added == 0);
        self.selected = match self.selected {
            Some(s) if s <= file => Some(s),
            Some(s) if s >= old.end => Some(s + added - removed - gone),
            _ => None,
        };
        if added == 0 {
            self.rows.remove(file);
            self.files -= 1;
        } else if let Row::File {
            count,
            more: m,
            stale,
            ..
        } = &mut self.rows[file]
        {
            (*count, *m, *stale) = (added, more, false);
        }
        self.reindex();
        self.generation += 1;
    }

    /// Rebuilds the lookups of file rows after rows moved.
    fn reindex(&mut self) {
        self.by_path.clear();
        self.by_doc.clear();
        for (i, r) in self.rows.iter().enumerate() {
            if let Row::File { path, doc, .. } = r {
                if let Some(p) = path {
                    self.by_path.insert(p.clone(), i);
                }
                if let Some(d) = doc {
                    self.by_doc.insert(*d, i);
                }
            }
        }
    }

    /// Closes the panel and stops a search still running. A replacement being written keeps the
    /// panel open: it shows the progress and the Cancel button.
    pub fn close(&mut self) {
        if self.replace.busy() {
            return;
        }
        self.replace.clear();
        self.cancel.store(true, Ordering::Relaxed);
        self.rx = None;
        if matches!(self.state, State::Running { .. }) {
            self.state = State::Done {
                skipped: 0,
                truncated: false,
                cancelled: true,
            };
        }
        self.open = false;
        self.focused = false;
        self.generation += 1;
    }

    /// Gives the panel the keyboard, or takes it away.
    pub fn set_focus(&mut self, on: bool) {
        if self.focused != on {
            self.focused = on;
            self.generation += 1;
        }
    }

    /// The match row a navigation key leads to from the selected row (none selected: the key
    /// leads from before the first for the forward keys and after the last for the others),
    /// without going round. None when there is no match row in that direction.
    pub fn nav(&self, nav: Nav) -> Option<usize> {
        let n = self.rows.len();
        let is_hit = |i: &usize| matches!(self.rows[*i], Row::Hit(_));
        let from = self.selected.filter(|s| *s < n);
        let down = |start: usize| (start..n).find(is_hit);
        let up = |end: usize| (0..end.min(n)).rev().find(is_hit);
        match (nav, from) {
            (Nav::Home, _) => down(0),
            (Nav::End, _) => up(n),
            (Nav::Down, Some(s)) => down(s + 1).or(Some(s).filter(is_hit)),
            (Nav::Down, None) => down(0),
            (Nav::Up, Some(s)) => up(s).or(Some(s).filter(is_hit)),
            (Nav::Up, None) => up(n),
            // A page moves that many rows (file rows count), then lands on the nearest match.
            (Nav::PageDown(k), s) => {
                let t = s
                    .map_or(k.saturating_sub(1), |s| s + k)
                    .min(n.saturating_sub(1));
                down(t).or_else(|| up(n))
            }
            (Nav::PageUp(k), s) => {
                let t = s.map_or(n, |s| s.saturating_sub(k));
                up(t + 1).or_else(|| down(0))
            }
        }
    }

    /// The match row after the selected one (before it when not `forward`), going round.
    pub fn step(&self, forward: bool) -> Option<usize> {
        let n = self.rows.len();
        let is_hit = |i: &usize| matches!(self.rows[*i], Row::Hit(_));
        match (self.selected, forward) {
            (Some(s), true) => (s + 1..n)
                .chain(0..=s.min(n.saturating_sub(1)))
                .find(is_hit),
            (Some(s), false) => (0..s.min(n)).rev().chain((s.min(n)..n).rev()).find(is_hit),
            (None, true) => (0..n).find(is_hit),
            (None, false) => (0..n).rev().find(is_hit),
        }
    }

    /// Where a match row leads: its file or tab, and the match's range in the text.
    pub fn target(&self, row: usize) -> Option<(Option<PathBuf>, Option<DocId>, Range<usize>)> {
        let Row::Hit(hit) = self.rows.get(row)? else {
            return None;
        };
        let file = self.rows[..row].iter().rev().find_map(|r| match r {
            Row::File { path, doc, .. } => Some((path.clone(), *doc)),
            Row::Hit(_) => None,
        })?;
        Some((file.0, file.1, hit.range.clone()))
    }

    /// Marks a row as the one gone to and scrolls it into the list.
    pub fn select(&mut self, row: usize) {
        self.selected = Some(row);
        let y = row as f32 * ROW;
        if y < self.scroll {
            self.scroll = y;
        } else if y + ROW > self.scroll + self.view {
            self.scroll = y + ROW - self.view;
        }
        self.generation += 1;
    }

    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll = (self.scroll + dy)
            .clamp(0.0, (self.content - self.view).max(0.0))
            .round();
    }

    /// The title row's text.
    fn title(&self) -> String {
        if let Some(o) = &self.replace.outcome {
            return o.title.clone();
        }
        let count = |n: usize, one: &str, many: &str| {
            if n == 1 {
                format!("1 {one}")
            } else {
                format!("{n} {many}")
            }
        };
        let mut s = format!(
            "\u{201C}{}\u{201D} {}: {} in {}",
            self.what,
            self.place,
            count(self.hits, "match", "matches"),
            count(self.files, "file", "files"),
        );
        if self.stale_files > 0 {
            s.push_str(" \u{00B7} updating\u{2026}");
        }
        match self.state {
            State::Idle => {}
            State::Running { files } => {
                s.push_str(&format!(" \u{00B7} searching, {files} files read\u{2026}"))
            }
            State::Done {
                skipped,
                truncated,
                cancelled,
            } => {
                if truncated {
                    s.push_str(" \u{00B7} more were left out");
                }
                if cancelled {
                    s.push_str(" \u{00B7} stopped");
                }
                if skipped > 0 {
                    s.push_str(&format!(
                        " \u{00B7} {} skipped",
                        count(skipped, "file", "files")
                    ));
                }
            }
        }
        s
    }
}

/// The height a drag of the top edge asks for, kept between the least and what leaves the text
/// its minimum out of `room` (the text and the panel together).
pub fn dragged_height(want: f32, room: f32) -> f32 {
    let max = (room - RESULTS_TEXT_MIN).max(RESULTS_MIN_H);
    want.clamp(RESULTS_MIN_H, max).round()
}

impl Results {
    /// The row of boxes for the folder and the kinds shows after a search of a folder.
    pub fn scope_shown(&self) -> bool {
        self.replace.searched.is_some() && self.replace.outcome.is_none()
    }

    /// The list below the panel's title row (and the row of boxes, when it shows).
    pub fn list_area(&self, r: Rect) -> Rect {
        let top = HEAD + if self.scope_shown() { SCOPE_H } else { 0.0 };
        Rect::new(r.x, r.y + top, r.w, (r.h - top).max(0.0))
    }
}

/// The boxes of the folder and the kinds of file, under the title row.
fn draw_scope(c: &mut Ctx, ui: &mut Ui, r: Rect) {
    let (focus, on) = (ui.find_focused, ui.caret_on);
    let which = ui.field;
    let y = r.y + HEAD + 4.0;
    let inner = (r.w - 32.0 - 8.0).max(0.0);
    let fw = (inner * 0.6).round();
    let folder = Rect::new(r.x + 16.0, y, fw, 26.0);
    let kinds = Rect::new(folder.right() + 8.0, y, inner - fw, 26.0);
    let res = &mut ui.results;
    crate::chrome::draw_field(
        c,
        &mut res.folder,
        focus && which == crate::ui::BarField::ResultsFolder,
        on,
        folder,
        crate::ui::BarField::ResultsFolder,
        false,
    );
    crate::chrome::draw_field(
        c,
        &mut res.kinds,
        focus && which == crate::ui::BarField::ResultsKinds,
        on,
        kinds,
        crate::ui::BarField::ResultsKinds,
        false,
    );
}

pub fn draw(c: &mut Ctx, ui: &mut Ui, r: Rect) {
    let pal = *c.pal;
    c.p.fill(r, pal.pane);
    let px = c.p.px();
    // The panel's top line shows who has the keyboard.
    let top = if ui.results.focused {
        pal.accent
    } else {
        pal.line
    };
    c.p.fill(Rect::new(r.x, r.y, r.w, px), top);
    let search_done = matches!(ui.results.state, State::Done { .. }) && ui.results.files > 0;
    let controls = crate::replace_ui::draw_head(c, ui, r, search_done);
    if ui.results.scope_shown() {
        draw_scope(c, ui, r);
    }
    let res = &mut ui.results;
    let title = res.title();
    c.p.text(
        Face::Semibold,
        12.5,
        r.x + 16.0,
        r.y + HEAD / 2.0 + 1.0,
        &title,
        pal.text,
        (controls - 12.0 - (r.x + 16.0)).min(r.w - 64.0),
    );
    let b = Rect::new(r.right() - 36.0, r.y + 4.0, 26.0, 26.0);
    if let Some(fill) = c.fill_for(Hit::ResultsClose, false, false) {
        c.p.rbox(b, 5.0, fill);
    }
    c.p.icon(glyph::CLOSE, 10.0, b.cx(), b.cy(), pal.muted);
    c.hit(Hit::ResultsClose, b);

    let area = res.list_area(r);
    let shown_rows = res
        .replace
        .outcome
        .as_ref()
        .map_or(res.rows.len(), |o| o.rows.len());
    res.content = shown_rows as f32 * ROW + 6.0;
    res.view = area.h;
    res.scroll = res.scroll.clamp(0.0, (res.content - area.h).max(0.0));
    let scroll = res.scroll;
    if let Some(o) = &res.replace.outcome {
        crate::replace_ui::draw_outcome(c, o, r, area, scroll);
    } else if res.rows.is_empty() {
        let text = match res.state {
            State::Running { .. } => "Searching\u{2026}",
            _ => "Nothing found.",
        };
        let at = Rect::new(area.x, area.y + 14.0, area.w, 20.0);
        c.p.text_center(Face::Ui, 13.0, at, text, pal.muted);
    }
    let quiet = mix(pal.text, pal.muted, 0.35);
    let old = c.p.clip(area);
    let first = (scroll / ROW).floor() as usize;
    // Whether the rows being drawn belong to a file whose matches are out of date.
    let mut stale = res.stale_files > 0 && res.is_stale(first);
    for (i, row) in res.rows.iter().enumerate().skip(first) {
        let y = area.y + i as f32 * ROW - scroll;
        if y > area.bottom() {
            break;
        }
        let rr = Rect::new(r.x + 6.0, y, r.w - 12.0, ROW_H);
        let cy = rr.cy();
        match row {
            Row::File {
                name,
                count,
                more,
                stale: s,
                ..
            } => {
                stale = *s;
                c.p.icon(glyph::DOC, 13.0, r.x + 24.0, cy, pal.muted);
                let w = c.p.text(
                    Face::Semibold,
                    12.5,
                    r.x + 38.0,
                    cy,
                    name,
                    pal.text,
                    rr.w - 120.0,
                );
                let n = if *more {
                    format!("{count}+")
                } else {
                    count.to_string()
                };
                c.p.text(Face::Ui, 12.0, r.x + 48.0 + w, cy, &n, pal.faint, 80.0);
            }
            Row::Hit(hit) => {
                let h = Hit::ResultsRow(i);
                draw_row_fill(
                    c.p,
                    &pal,
                    rr,
                    c.hover == Some(h),
                    c.pressed == Some(h),
                    res.selected == Some(i),
                );
                let num = (hit.line + 1).to_string();
                c.p.text_right(Face::Ui, 12.0, r.x + 76.0, cy, &num, pal.faint);
                // The match gets the find mark's colour behind it.
                let x = r.x + 88.0;
                let max = (rr.right() - x - 14.0).max(0.0);
                let (a, b) = (
                    hit.mark.start.min(hit.snippet.len()),
                    hit.mark.end.min(hit.snippet.len()),
                );
                if hit.snippet.is_char_boundary(a) && hit.snippet.is_char_boundary(b) && a < b {
                    let xa = c.p.measure(Face::Ui, 13.0, &hit.snippet[..a]);
                    let xb = c.p.measure(Face::Ui, 13.0, &hit.snippet[..b]);
                    if xa < max {
                        let m =
                            Rect::new(x + xa - 1.0, y + 3.0, xb.min(max) - xa + 2.0, ROW_H - 6.0);
                        c.p.rbox_a(m, 3.0, pal.find_mark, if pal.dark { 0.38 } else { 0.45 });
                    }
                }
                let ink = if stale { pal.faint } else { quiet };
                c.p.text(Face::Ui, 13.0, x, cy, &hit.snippet, ink, max);
                c.hit(h, rr);
            }
        }
    }
    c.p.restore(old);
    let thumb_hit = Hit::ScrollBar(Scroller::Results);
    if let Some((track, thumb)) = scrollbar(area, res.content, area.h, scroll) {
        let active = c.hover == Some(thumb_hit)
            || matches!(
                ui.drag,
                Some(Drag::Thumb {
                    target: Scroller::Results,
                    ..
                })
            );
        draw_scrollbar(c.p, &pal, track, thumb, active);
        c.hit(thumb_hit, track);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdedit_core::Buffer;
    use mdedit_core::find::FindOptions;
    use std::sync::mpsc::channel;

    fn query(text: &str) -> Query {
        Query::new(text, FindOptions::default()).unwrap()
    }

    fn found(text: &str, what: &str) -> Vec<Found> {
        let cancel = AtomicBool::new(false);
        search::in_buffer(&Buffer::from_text(text), &query(what), 100, &cancel).0
    }

    #[test]
    fn rows_step_round_and_lead_to_their_file() {
        let mut res = Results::default();
        assert_eq!(res.step(true), None);
        res.set_tabs(
            "x",
            vec![
                (
                    1,
                    "a.txt".into(),
                    Some("a.txt".into()),
                    found("x\n.x\n", "x"),
                    false,
                ),
                (2, "Untitled 1".into(), None, Vec::new(), false),
                (3, "b.txt".into(), None, found("..x", "x"), true),
            ],
            false,
        );
        // File a, two matches, file b, one match: a tab with none has no row.
        assert_eq!((res.rows.len(), res.hits, res.files), (5, 3, 2));
        assert!(res.open);
        assert_eq!(
            res.state,
            State::Done {
                skipped: 0,
                truncated: true,
                cancelled: false
            }
        );
        assert_eq!(res.step(true), Some(1));
        assert_eq!(res.step(false), Some(4));
        let mut seen = Vec::new();
        for _ in 0..4 {
            let row = res.step(true).unwrap();
            res.select(row);
            seen.push(row);
        }
        assert_eq!(seen, [1, 2, 4, 1]);
        assert_eq!(res.step(false), Some(4));
        assert_eq!(res.target(0), None);
        assert_eq!(res.target(2), Some((Some("a.txt".into()), Some(1), 3..4)));
        assert_eq!(res.target(4), Some((None, Some(3), 2..3)));
        assert!(res.title().contains("3 matches in 2 files"));
        res.close();
        assert!(!res.open);
    }

    #[test]
    fn navigation_keys_land_on_match_rows_and_stop_at_the_ends() {
        let mut res = Results::default();
        assert_eq!(res.nav(Nav::Down), None);
        res.set_tabs(
            "x",
            vec![
                (1, "a".into(), None, found("x\n.x\n", "x"), false),
                (3, "b".into(), None, found("..x", "x"), false),
            ],
            false,
        );
        // Rows: 0 file, 1 hit, 2 hit, 3 file, 4 hit.
        assert_eq!(res.nav(Nav::Down), Some(1));
        assert_eq!(res.nav(Nav::Up), Some(4));
        assert_eq!(res.nav(Nav::Home), Some(1));
        assert_eq!(res.nav(Nav::End), Some(4));
        res.selected = Some(2);
        assert_eq!(res.nav(Nav::Down), Some(4));
        assert_eq!(res.nav(Nav::Up), Some(1));
        res.selected = Some(4);
        assert_eq!(res.nav(Nav::Down), Some(4));
        res.selected = Some(1);
        assert_eq!(res.nav(Nav::Up), Some(1));
        assert_eq!(res.nav(Nav::PageDown(1)), Some(2));
        assert_eq!(res.nav(Nav::PageDown(2)), Some(4));
        assert_eq!(res.nav(Nav::PageDown(50)), Some(4));
        res.selected = Some(4);
        assert_eq!(res.nav(Nav::PageUp(1)), Some(2));
        assert_eq!(res.nav(Nav::PageUp(50)), Some(1));
        // A selected row gone stale after a new search does not break it.
        res.selected = Some(99);
        assert_eq!(res.nav(Nav::Down), Some(1));
    }

    fn wait_refreshed(res: &mut Results, until: impl Fn(&Results) -> bool) {
        let end = Instant::now() + Duration::from_secs(20);
        while !until(res) && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(5));
            res.poll();
        }
        assert!(until(res), "timed out");
    }

    #[test]
    fn rows_of_an_edited_tab_go_stale_then_are_searched_again_on_a_worker() {
        let waker: Waker = Arc::new(|_| {});
        let mut text = String::from("x\nab\nx\n");
        let mut res = Results::default();
        res.set_tabs(
            "x",
            vec![
                (1, "a.txt".into(), None, found(&text, "x"), false),
                (2, "b.txt".into(), None, found("x", "x"), false),
            ],
            false,
        );
        res.set_tab_search(query("x"), &[(1, 5), (2, 7)]);
        let now = Instant::now();
        let tab = |id, revision, modified| TabRev {
            id,
            path: None,
            revision,
            modified,
        };
        let snap = |t: &str| Buffer::from_text(t);
        // Rows: 0 file a, 1 and 2 matches, 3 file b, 4 match. Nothing changed: nothing to do.
        let buf = snap(&text);
        let same = [tab(1, 5, false), tab(2, 7, false)];
        assert_eq!(
            res.sync(same.into_iter(), now, &|_| Some(buf.clone()), &waker),
            None
        );
        assert!(!res.is_stale(1) && res.stale_files == 0);
        res.select(2);
        // The first tab is edited: its rows (and only its) are stale at once, a search is
        // waited for.
        text = "x\nab\nx\nx\nx\n".into();
        let buf = snap(&text);
        let edited = [tab(1, 6, true), tab(2, 7, false)];
        let due = res.sync(edited.into_iter(), now, &|_| Some(buf.clone()), &waker);
        assert_eq!(due, Some(now + REFRESH_AFTER));
        assert!(res.is_stale(1) && res.is_stale(2) && !res.is_stale(4));
        assert!(res.title().contains("updating"));
        // Another change moves the wait; no search has started yet.
        let later = now + Duration::from_millis(100);
        let edited = [tab(1, 7, true), tab(2, 7, false)];
        let due = res.sync(edited.into_iter(), later, &|_| Some(buf.clone()), &waker);
        assert_eq!(due, Some(later + REFRESH_AFTER));
        assert!(res.refreshed.lock().unwrap().is_empty());
        // Left alone for long enough, it is searched again and the rows are replaced.
        let edited = [tab(1, 7, true), tab(2, 7, false)];
        let end = later + REFRESH_AFTER + Duration::from_millis(1);
        res.sync(edited.into_iter(), end, &|_| Some(buf.clone()), &waker);
        wait_refreshed(&mut res, |r| r.hits == 5);
        assert_eq!((res.rows.len(), res.files, res.stale_files), (7, 2, 0));
        assert!(!res.is_stale(1) && res.title().contains("5 matches"));
        // The selected row was inside the changed ones, so it went.
        assert_eq!(res.selected, None);
        assert_eq!(res.target(4), Some((None, Some(1), 9..10)));
        // The second file is now at row 5 and its match at 6.
        assert_eq!(res.target(6), Some((None, Some(2), 0..1)));
        // Asked again for the same revision nothing starts: the rows are of it.
        let edited = [tab(1, 7, true), tab(2, 7, false)];
        assert_eq!(
            res.sync(edited.into_iter(), end, &|_| Some(buf.clone()), &waker),
            None
        );
        // The text loses its last match: the file's row goes.
        let buf = snap("ab\n");
        let gone = [tab(1, 8, true), tab(2, 7, false)];
        res.sync(gone.into_iter(), end, &|_| Some(buf.clone()), &waker);
        let gone = [tab(1, 8, true), tab(2, 7, false)];
        let after = end + REFRESH_AFTER;
        res.sync(gone.into_iter(), after, &|_| Some(buf.clone()), &waker);
        wait_refreshed(&mut res, |r| r.files == 1);
        assert_eq!((res.rows.len(), res.hits, res.stale_files), (2, 1, 0));
        assert!(matches!(
            &res.rows[0],
            Row::File {
                doc: Some(2),
                count: 1,
                ..
            }
        ));
        assert_eq!(res.target(1), Some((None, Some(2), 0..1)));
    }

    #[test]
    fn a_selected_row_after_the_changed_ones_keeps_its_place() {
        let mut res = Results::default();
        res.set_tabs(
            "x",
            vec![
                (1, "a".into(), None, found("x\nx\n", "x"), false),
                (2, "b".into(), None, found("x", "x"), false),
            ],
            false,
        );
        // Rows: 0 file, 1 and 2 matches, 3 file, 4 match. The first file gets one more match.
        res.selected = Some(4);
        res.replace_file(0, found("x\nx\nx\n", "x"), false);
        assert_eq!((res.rows.len(), res.hits, res.selected), (6, 4, Some(5)));
        // It loses all of them: file row and matches go, the selected row follows.
        res.replace_file(0, Vec::new(), false);
        assert_eq!((res.rows.len(), res.files, res.selected), (2, 1, Some(1)));
    }

    #[test]
    fn matches_read_from_a_file_that_its_open_tab_has_changed_are_stale_from_the_start() {
        let waker: Waker = Arc::new(|_| {});
        let buf = Buffer::from_text("y");
        let path = Path::new("C:/d/a.txt");
        let tab = |modified| TabRev {
            id: 9,
            path: Some(path),
            revision: 3,
            modified,
        };
        let now = Instant::now();
        let folder_search = || {
            let mut res = Results::default();
            res.set_tabs("x", Vec::new(), false);
            res.push_file(
                Some(path.to_path_buf()),
                None,
                "a.txt".into(),
                found("x", "x"),
                false,
            );
            res.query = Some(query("x"));
            res
        };
        // A tab that is unchanged: the rows read from its file are of it.
        let mut res = folder_search();
        let due = res.sync(
            [tab(false)].into_iter(),
            now,
            &|_| Some(buf.clone()),
            &waker,
        );
        assert_eq!(due, None);
        assert!(!res.is_stale(1));
        // One with unsaved changes is not: they were read from the file.
        let mut res = folder_search();
        let due = res.sync([tab(true)].into_iter(), now, &|_| Some(buf.clone()), &waker);
        assert_eq!(due, Some(now + REFRESH_AFTER));
        assert!(res.is_stale(1));
    }

    #[test]
    fn the_height_is_dragged_within_its_limits() {
        assert_eq!(dragged_height(300.4, 700.0), 300.0);
        assert_eq!(dragged_height(20.0, 700.0), RESULTS_MIN_H);
        assert_eq!(dragged_height(900.0, 700.0), 700.0 - RESULTS_TEXT_MIN);
        // A window too low for both limits keeps the least.
        assert_eq!(dragged_height(500.0, 200.0), RESULTS_MIN_H);
        assert_eq!(Results::default().height, RESULTS_H);
    }

    #[test]
    fn a_folder_search_keeps_to_the_kinds_of_file_chosen_and_says_so() {
        let dir = std::env::temp_dir().join(format!("mdedit-kinds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "needle\n").unwrap();
        std::fs::write(dir.join("b.txt"), "needle\n").unwrap();
        let load = LoadOptions {
            legacy: mdedit_core::encoding::legacy_for_code_page(1252),
            max_bytes: 1 << 20,
            forced: None,
        };
        let mut res = Results::default();
        assert!(!res.scope_shown());
        res.kinds.set_text(" *.md ");
        let filter = NameFilter::parse(res.kinds.text());
        res.start_tree(&dir, query("needle"), load, filter, Arc::new(|_| {}));
        // The boxes show once a folder was searched, and the title says what it looked in.
        assert!(res.scope_shown());
        assert!(res.title().contains("(*.md)"));
        wait_refreshed(&mut res, |r| matches!(r.state, State::Done { .. }));
        assert_eq!((res.files, res.hits), (1, 1));
        assert!(matches!(&res.rows[0], Row::File { name, .. } if name == "a.md"));
        // The replacement that follows plans from the same files.
        let s = res.replace.searched.as_ref().unwrap();
        assert!(s.filter.matches("a.md") && !s.filter.matches("b.txt"));
        // A search of the open tabs has no folder: the boxes go.
        res.set_tabs("x", Vec::new(), false);
        assert!(!res.scope_shown());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_is_searched_on_a_worker() {
        let dir = std::env::temp_dir().join(format!("mdedit-results-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("one.txt"), "needle\nhay\n").unwrap();
        std::fs::write(dir.join("sub").join("two.txt"), "hay\n  a needle\n").unwrap();
        std::fs::write(dir.join("three.txt"), "hay\n").unwrap();
        let load = LoadOptions {
            legacy: mdedit_core::encoding::legacy_for_code_page(1252),
            max_bytes: 1 << 20,
            forced: None,
        };
        let (tx, woken) = channel();
        let waker: Waker = Arc::new(move |_| {
            let _ = tx.send(());
        });
        let mut res = Results::default();
        res.start_tree(&dir, query("needle"), load, NameFilter::default(), waker);
        assert_eq!(res.state, State::Running { files: 0 });
        let until = Instant::now() + Duration::from_secs(20);
        while matches!(res.state, State::Running { .. }) && Instant::now() < until {
            let _ = woken.recv_timeout(Duration::from_millis(50));
            res.poll();
        }
        assert_eq!(
            res.state,
            State::Done {
                skipped: 0,
                truncated: false,
                cancelled: false
            }
        );
        assert_eq!((res.hits, res.files, res.rows.len()), (2, 2, 4));
        let names: Vec<&str> = res
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::File { name, .. } => Some(name.as_str()),
                Row::Hit(_) => None,
            })
            .collect();
        assert_eq!(names[0], "one.txt");
        assert_eq!(names[1].replace('\\', "/"), "sub/two.txt");
        let (path, doc, range) = res.target(3).unwrap();
        assert_eq!(path, Some(dir.join("sub").join("two.txt")));
        assert_eq!((doc, range), (None, 8..14));
        // A new search drops the old rows; closing stops it.
        res.start_tree(
            &dir,
            query("hay"),
            load,
            NameFilter::default(),
            Arc::new(|_| {}),
        );
        assert!(res.rows.is_empty() && res.selected.is_none());
        res.close();
        assert!(!res.poll());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
