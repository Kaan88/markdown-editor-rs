//! Open documents (one per tab): load jobs, buffers, selection, scroll, find and colour state.
//!
//! A document is `Pending` until its tab is first shown (restored tabs load on activation),
//! `Loading` while its first load runs on a thread, then `Ready` or `Failed`. A reload keeps the
//! `Ready` text on screen until the new one arrives. Every job result carries the document id and
//! the revision it was started for (a counter per document, bumped on every load); results for
//! another revision are dropped.

use crate::app::Wake;
use crate::commands::strings;
use crate::session::ThemeChoice;
use crate::view::cache::Lru;
use mdedit_core::display::ChunkIndex;
use mdedit_core::find::{self, FindOptions, Matches, Query};
use mdedit_core::limits::{FIND_CAP, MAX_FILE_BYTES, SYNC_FIND_BYTES};
use mdedit_core::load::{self, LoadError, LoadOptions, Loaded};
use mdedit_core::selection::Selection;
use mdedit_core::{Buffer, FileInfo, Position};
use mdedit_highlight::{Colored, DocId, HighlightService, Language, Span};
use mdshared_app::files::{FileStamp, stamp};
use mdshared_app::paths::{canonical_or_absolute, same_path, tab_labels};
use mdshared_app::recents::Recents;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
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

    /// Whether every line of `lines` has colours in `dark`.
    pub fn covers(&self, lines: &Range<usize>, dark: bool) -> bool {
        self.generation > 0
            && self.dark == dark
            && lines.start >= self.first
            && lines.end <= self.first + self.lines.len()
    }
}

/// The find state of one document: what the matches were computed for and where the user is.
#[derive(Default)]
pub struct FindState {
    pub text: String,
    pub match_case: bool,
    pub matches: Vec<Range<usize>>,
    pub capped: bool,
    /// Index into `matches` of the current match.
    pub current: Option<usize>,
    /// A search thread is running for `text`.
    pub searching: bool,
    /// Bumped whenever the marks or the current match change, for the text region's signature.
    pub generation: u64,
    cancel: Option<Arc<AtomicBool>>,
}

impl FindState {
    fn cancel(&mut self) {
        if let Some(c) = self.cancel.take() {
            c.store(true, Ordering::Relaxed);
        }
    }

    /// The counter beside the find box.
    pub fn label(&self) -> String {
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

pub struct Ready {
    pub buffer: Buffer,
    /// The document revision this text was loaded at. A reload bumps the document's revision at
    /// once but keeps drawing this text until the new one arrives, so layouts are keyed by this.
    pub revision: u64,
    pub info: FileInfo,
    pub language: Option<Language>,
    /// The language exists and the file is within its limits: the service colours it.
    pub colourable: bool,
    pub sel: Selection,
    /// First visible line, fractional.
    pub top: f64,
    /// Horizontal scroll, logical px.
    pub left: f64,
    pub colors: LineColors,
    pub find: FindState,
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
    fn new(loaded: Loaded, language: Option<Language>, revision: u64) -> Ready {
        Ready {
            buffer: loaded.buffer,
            revision,
            info: loaded.info,
            language,
            colourable: false,
            sel: Selection::at(0),
            top: 0.0,
            left: 0.0,
            colors: LineColors::default(),
            find: FindState::default(),
            chunks: Lru::new(crate::view::CHUNK_INDEXES),
            widest: 0.0,
            view_lines: 30.0,
            view_w: 800.0,
            advance: 7.7,
            wide_advance: 14.0,
        }
    }

    /// Selects `range` (caret at its end) and brings it into view, centred if it was off screen.
    pub fn select_and_reveal(&mut self, range: Range<usize>) {
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
    pub path: PathBuf,
    pub revision: u64,
    pub state: DocState,
    /// The file is not on disk: gone since it was read (the tab keeps the text last read), or
    /// never there (the tab shows why it could not be opened).
    pub missing: bool,
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
            path,
            revision: 0,
            state: DocState::Pending,
            missing: false,
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

    pub fn name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
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
    pub match_case: bool,
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

/// What taking in a load result means for the shell.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// The active document became ready (first load or reload).
    pub active_ready: bool,
}

pub struct Docs {
    pub tabs: Vec<Doc>,
    active: Option<usize>,
    next_id: DocId,
    pub recents: Recents,
    pub folder: Option<PathBuf>,
    pub theme: ThemeChoice,
    opts: LoadOptions,
    waker: Waker,
    load_tx: Sender<LoadDone>,
    load_rx: Receiver<LoadDone>,
    find_tx: Sender<FindDone>,
    find_rx: Receiver<FindDone>,
    toast: Option<(String, Instant)>,
    /// The set of open files changed: the watcher needs the new list.
    pub watch_changed: bool,
}

impl Docs {
    pub fn new(code_page: u32, waker: Waker) -> Docs {
        let (load_tx, load_rx) = channel();
        let (find_tx, find_rx) = channel();
        Docs {
            tabs: Vec::new(),
            active: None,
            next_id: 1,
            recents: Recents::default(),
            folder: None,
            theme: ThemeChoice::System,
            opts: LoadOptions {
                legacy: mdedit_core::encoding::legacy_for_code_page(code_page),
                max_bytes: MAX_FILE_BYTES,
            },
            waker,
            load_tx,
            load_rx,
            find_tx,
            find_rx,
            toast: None,
            watch_changed: true,
        }
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
        let paths: Vec<PathBuf> = self.tabs.iter().map(|d| d.path.clone()).collect();
        tab_labels(&paths)
    }

    pub fn index_of(&self, path: &Path) -> Option<usize> {
        self.tabs.iter().position(|d| same_path(&d.path, path))
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

    /// Opens `path` in a tab (or shows the tab that has it) and makes it active.
    pub fn open(&mut self, path: &Path) -> usize {
        let i = self.add_pending(path);
        self.activate(i);
        i
    }

    /// Makes tab `i` active; its first activation starts its load.
    pub fn activate(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
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

    pub fn paths(&self) -> Vec<PathBuf> {
        self.tabs.iter().map(|d| d.path.clone()).collect()
    }

    // ---- loading ---------------------------------------------------------------------------

    /// Starts a load (or reload) of tab `i` on a thread.
    pub fn start_load(&mut self, i: usize) {
        let opts = self.opts;
        let tx = self.load_tx.clone();
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
        let (id, revision, path, progress) =
            (doc.id, doc.revision, doc.path.clone(), doc.progress.clone());
        std::thread::spawn(move || {
            let w = waker.clone();
            let result = run_load(&path, &opts, &progress, &cancel, &move || w(Wake::Loaded));
            if matches!(result, Err(LoadError::Cancelled)) {
                return;
            }
            let _ = tx.send(LoadDone {
                doc: id,
                revision,
                result,
            });
            waker(Wake::Loaded);
        });
    }

    /// Loads tab `i` on this thread (for `--shot`) and takes the result in.
    pub fn load_now(&mut self, i: usize, service: Option<&HighlightService>, dark: bool) {
        let doc = &mut self.tabs[i];
        doc.cancel_load();
        doc.revision += 1;
        let cancel = AtomicBool::new(false);
        let result = run_load(&doc.path, &self.opts, &doc.progress, &cancel, &|| {});
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
        let mut out = Applied::default();
        let now = Instant::now();
        if deadline > now
            && let Ok(done) = self.load_rx.recv_timeout(deadline - now)
        {
            out.active_ready |= self.apply_load(done, service, dark).active_ready;
        }
        out.active_ready |= self.drain_loads(service, dark).active_ready;
        out
    }

    /// Takes in every finished load.
    pub fn drain_loads(&mut self, service: Option<&HighlightService>, dark: bool) -> Applied {
        let mut out = Applied::default();
        while let Ok(done) = self.load_rx.try_recv() {
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
                    r.top = old.top;
                    r.left = old.left;
                    r.view_lines = old.view_lines;
                    r.view_w = old.view_w;
                    r.advance = old.advance;
                    r.wide_advance = old.wide_advance;
                    old.find.cancel();
                    if !old.find.text.is_empty() {
                        query = Some((old.find.text.clone(), old.find.match_case));
                    }
                }
                if let (Some(lang), Some(s)) = (language, service)
                    && lang.limits().allows_file(r.buffer.len_bytes())
                {
                    s.open(doc.id, doc.revision, r.buffer.snapshot(), lang, dark);
                    r.colourable = true;
                }
                doc.state = DocState::Ready(Box::new(r));
                doc.missing = false;
                let path = doc.path.clone();
                self.recents.add(path);
                if let Some((text, case)) = query {
                    self.find_in(i, &text, case, false);
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
            let now = stamp(&doc.path);
            match (&doc.state, now) {
                (DocState::Ready(_), FileStamp::Missing) => self.tabs[i].missing = true,
                (DocState::Ready(r), s) if s != r.info.stamp || doc.missing => {
                    self.tabs[i].missing = false;
                    if !self.tabs[i].is_loading() {
                        self.start_load(i);
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
        {
            self.start_load(i);
        }
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
    pub fn find_set(&mut self, text: &str, match_case: bool) {
        if let Some(i) = self.active {
            self.find_in(i, text, match_case, true);
        }
    }

    fn find_in(&mut self, i: usize, text: &str, match_case: bool, go: bool) {
        let (tx, waker) = (self.find_tx.clone(), self.waker.clone());
        let doc = &mut self.tabs[i];
        let (id, revision) = (doc.id, doc.revision);
        let Some(r) = doc.ready_mut() else { return };
        if r.find.text == text && r.find.match_case == match_case {
            return;
        }
        r.find.cancel();
        r.find.text = text.to_string();
        r.find.match_case = match_case;
        r.find.matches.clear();
        r.find.capped = false;
        r.find.current = None;
        r.find.searching = false;
        r.find.generation += 1;
        let Some(query) = Query::new(text, FindOptions { match_case }) else {
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
            let _ = tx.send(FindDone {
                doc: id,
                revision,
                text,
                match_case,
                kind: FindKind::All(m),
            });
            waker(Wake::Found);
        });
    }

    /// Goes to the next or previous match, wrapping. Past the last kept match of a capped
    /// search, `find_next` looks further from the caret.
    pub fn find_step(&mut self, forward: bool) {
        let (tx, waker) = (self.find_tx.clone(), self.waker.clone());
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
        let Some(query) = Query::new(
            &r.find.text,
            FindOptions {
                match_case: r.find.match_case,
            },
        ) else {
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
        let (text, match_case) = (r.find.text.clone(), r.find.match_case);
        std::thread::spawn(move || {
            let found = find::find_next(&rope, &query, from, forward, true, &cancel);
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let _ = tx.send(FindDone {
                doc: id,
                revision,
                text,
                match_case,
                kind: FindKind::Next(found),
            });
            waker(Wake::Found);
        });
    }

    /// Takes in finished searches. True if the active document's changed.
    pub fn drain_finds(&mut self) -> bool {
        let active = self.active_doc().map(|d| d.id);
        let mut changed = false;
        while let Ok(done) = self.find_rx.try_recv() {
            let Some(doc) = self.tabs.iter_mut().find(|d| d.id == done.doc) else {
                continue;
            };
            if doc.revision != done.revision {
                continue;
            }
            let id = doc.id;
            let Some(r) = doc.ready_mut() else { continue };
            if r.find.text != done.text || r.find.match_case != done.match_case {
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
        assert_eq!(d.add_pending(&dir.join("A.txt")) == 0, cfg!(windows));
        let c = d.add_pending(&dir.join("c.txt"));
        d.active = Some(c);
        assert!(d.move_tab(2, 0));
        assert_eq!(d.active(), Some(0));
        assert_eq!(d.tabs[0].name(), "c.txt");
        d.close(0, None);
        assert_eq!(d.tabs.len(), 2);
        assert_eq!(d.active(), Some(0));
        d.close_all(None);
        assert!(d.tabs.is_empty() && d.active().is_none());
    }
}
