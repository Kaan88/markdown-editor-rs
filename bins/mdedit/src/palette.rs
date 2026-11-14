//! The command palette's sources. The card, the query box, the ranking and the list are
//! `mdshared_ui::palette`; this module decides what the entries are, by the first character of the
//! query:
//!
//! - none: the open tabs, the recent files, and (once something is typed) the files of the open
//!   folder, listed on a worker thread the first time they are wanted and kept until the folder
//!   changes.
//! - `>`: the commands of the table, with their first shortcut; one that cannot run now is listed
//!   greyed, as the menus show it.
//! - `:`: go to a line, "Go to line 120" or "120:5", answered by Enter.
//! - `@`: the symbols of the current document, from the function list's rules, made on a worker
//!   thread when first asked for.
//! - `?`: the prefixes themselves.
//!
//! Nothing here exists until the palette opens: the host is empty at launch and the lists, the
//! threads and the buffers are made on demand. An entry's id is its index in `targets`, which says
//! what choosing it does.

use crate::app::Wake;
use crate::chrome::glyph;
use crate::commands::{COMMANDS, CommandId as C, hint, strings};
use crate::docs::{Docs, Waker};
use crate::macros::Macros;
use crate::ui::Ui;
use mdedit_core::search::walk_files;
use mdedit_highlight::DocId;
use mdedit_outline::{Flavor, Symbol, symbols};
use mdshared_ui::palette::{Entry, Palette};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The most files of the folder listed, as the Files pane's own listing stops somewhere too.
pub const MAX_FILES: usize = 5_000;
/// The most symbols kept of a document.
const MAX_SYMBOLS: usize = crate::outline::SYMBOLS_MAX;

const G_TABS: u8 = 0;
const G_RECENT: u8 = 1;
const G_FILES: u8 = 2;
/// Commands, symbols and the help rows have no header.
const G_PLAIN: u8 = 100;

/// What choosing an entry does.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Cmd(C),
    Tab(DocId),
    Recent(PathBuf),
    File(PathBuf),
    /// The 0-based line of a symbol of the current document.
    Symbol(usize),
    /// A row of the `?` list: puts this prefix in the box.
    Prefix(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Files,
    Commands,
    Line,
    Symbols,
    Help,
}

/// The mode a query is in and how many chars of it are the prefix.
pub fn mode_of(query: &str) -> (Mode, usize) {
    match query.chars().next() {
        Some('>') => (Mode::Commands, 1),
        Some(':') => (Mode::Line, 1),
        Some('@') => (Mode::Symbols, 1),
        Some('?') => (Mode::Help, 1),
        _ => (Mode::Files, 0),
    }
}

/// A worker's answer: the folder's files, each with its relative name.
type Shared = Arc<Mutex<Option<Vec<(PathBuf, String)>>>>;

/// The files of a folder, listed on a worker thread.
struct FolderFiles {
    root: PathBuf,
    /// `None` while the worker is still listing.
    done: Option<Vec<(PathBuf, String)>>,
    shared: Shared,
    cancel: Arc<AtomicBool>,
}

impl Drop for FolderFiles {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// The symbols of one revision of one document.
struct SymbolList {
    key: (DocId, u64),
    /// `None` while the worker is still reading; empty and `no_rules` when the language has no
    /// rules for a list.
    done: Option<Vec<Symbol>>,
    no_rules: bool,
    shared: Arc<Mutex<Option<Vec<Symbol>>>>,
    cancel: Arc<AtomicBool>,
}

impl Drop for SymbolList {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// What the entries are made from, apart from the window's own state.
pub struct Sources<'a> {
    pub docs: &'a Docs,
    pub ui: &'a Ui,
    /// A macro is recorded and none is being recorded (what Play and Save as need).
    macro_idle: bool,
    /// The folder's files, once listed.
    files: Option<&'a [(PathBuf, String)]>,
    /// The current document's symbols, once made; `None` while they are being made.
    symbols: Option<&'a [Symbol]>,
    no_rules: bool,
}

pub struct PaletteState {
    pub palette: Palette,
    targets: Vec<Target>,
    /// The source the entries came from; `None` before the first.
    mode: Option<Mode>,
    /// Whether the folder's files are among the entries (only once the query has text).
    with_files: bool,
}

impl PaletteState {
    fn new(query: &str) -> PaletteState {
        let mut palette = Palette::new();
        palette.set_placeholder(strings::PALETTE_PLACEHOLDER);
        palette.set_groups(vec![
            (G_TABS, "Open tabs".into()),
            (G_RECENT, "Recent".into()),
            (G_FILES, "Files".into()),
        ]);
        palette.open(query);
        PaletteState {
            palette,
            targets: Vec::new(),
            mode: None,
            with_files: false,
        }
    }

    /// Swaps the entries when the query's prefix (or, for files, whether it has text) changed
    /// the source, or `force` says the data behind them changed (a list came in). Typing within
    /// a mode only re-ranks, which the palette does by itself. The go-to line's message follows
    /// every change of the query.
    fn sync(&mut self, src: &Sources, force: bool) {
        let (mode, skip) = mode_of(self.palette.query());
        let with_files = mode == Mode::Files && !self.palette.needle().is_empty();
        let changed = self.mode != Some(mode) || with_files != self.with_files || force;
        if changed {
            self.mode = Some(mode);
            self.with_files = with_files;
            self.palette.set_ignore_prefix(skip);
            let (entries, targets) = match mode {
                Mode::Files => files(src, with_files),
                Mode::Commands => commands(src),
                Mode::Line => (Vec::new(), Vec::new()),
                Mode::Symbols => symbol_entries(src),
                Mode::Help => help(),
            };
            self.targets = targets;
            self.palette.set_message(match mode {
                Mode::Symbols if src.no_rules => Some(strings::PALETTE_NO_SYMBOLS.to_string()),
                Mode::Symbols if src.symbols.is_none() => {
                    Some(strings::PALETTE_READING.to_string())
                }
                Mode::Symbols if entries.is_empty() => {
                    Some(strings::PALETTE_NO_SYMBOLS_FOUND.to_string())
                }
                _ => None,
            });
            self.palette.set_entries(entries);
        }
        if mode == Mode::Line {
            let lines = src.docs.active_ready().map(|r| r.buffer.len_lines().max(1));
            self.palette
                .set_message(Some(line_message(&self.palette.needle(), lines)));
        }
    }

    /// What the entry with this id does.
    pub fn target(&self, id: u32) -> Option<&Target> {
        self.targets.get(id as usize)
    }

    /// The line and column a go-to query names ("12", "12:5"), when it is in the line mode.
    pub fn line_query(&self) -> Option<(usize, usize)> {
        let (mode, _) = mode_of(self.palette.query());
        if mode != Mode::Line {
            return None;
        }
        strings::parse_line_col(&self.palette.needle()).filter(|&(l, c)| l >= 1 && c >= 1)
    }
}

/// The message row of the go-to mode.
fn line_message(needle: &str, lines: Option<usize>) -> String {
    match strings::parse_line_col(needle).filter(|&(l, c)| l >= 1 && c >= 1) {
        Some((l, 1)) if !needle.contains([':', ',']) => format!("Go to line {l}"),
        Some((l, c)) => format!("Go to line {l}:{c}"),
        None => match lines {
            Some(n) => format!("Type a line number, or line:column. This file has {n} lines."),
            None => "Open a file first.".to_string(),
        },
    }
}

/// Pushes an entry and its target; the entry's id is the target's index.
fn push(
    entries: &mut Vec<Entry>,
    targets: &mut Vec<Target>,
    target: Target,
    e: impl FnOnce(u32) -> Entry,
) {
    entries.push(e(targets.len() as u32));
    targets.push(target);
}

fn folder_name(path: &Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn files(src: &Sources, with_files: bool) -> (Vec<Entry>, Vec<Target>) {
    let (mut entries, mut targets) = (Vec::new(), Vec::new());
    let mut listed: HashSet<PathBuf> = HashSet::new();
    let labels = src.docs.labels();
    for (i, doc) in src.docs.tabs.iter().enumerate() {
        let label = labels.get(i).cloned().unwrap_or_default();
        let hint = doc.path.as_deref().map(folder_name).unwrap_or_default();
        let icon = if doc.missing { glyph::WARN } else { glyph::DOC };
        if let Some(p) = &doc.path {
            listed.insert(p.clone());
        }
        push(&mut entries, &mut targets, Target::Tab(doc.id), |id| {
            Entry::new(id, label).hint(hint).icon(icon).group(G_TABS)
        });
    }
    for e in src.docs.recents.entries() {
        if listed.contains(&e.path) {
            continue;
        }
        let hint = folder_name(&e.path);
        let icon = if e.missing { glyph::WARN } else { glyph::DOC };
        listed.insert(e.path.clone());
        let label = e.label;
        push(&mut entries, &mut targets, Target::Recent(e.path), |id| {
            Entry::new(id, label).hint(hint).icon(icon).group(G_RECENT)
        });
    }
    if with_files {
        for (path, rel) in src.files.unwrap_or_default() {
            if listed.contains(path) {
                continue;
            }
            push(
                &mut entries,
                &mut targets,
                Target::File(path.clone()),
                |id| Entry::new(id, rel.as_str()).icon(glyph::DOC).group(G_FILES),
            );
        }
    }
    (entries, targets)
}

/// The icon a command has in the menus; the commands the menus do not offer get one that says
/// what they do.
fn icon(id: C) -> &'static str {
    match id {
        C::NewFile => glyph::NEW_FILE,
        C::OpenFiles => glyph::OPEN,
        C::OpenFolder => glyph::FOLDER_OPEN,
        C::Save => glyph::SAVE,
        C::SaveAs => glyph::SAVE_AS,
        C::Reload => glyph::RELOAD,
        C::OpenRecent | C::ManageRecent => glyph::RECENT,
        C::ClearRecent | C::DeleteMacros | C::ClearBookmarks => glyph::DELETE,
        C::CloseTab | C::CloseResults | C::ClearSearch | C::StopCompare => glyph::CLOSE,
        C::CloseAllTabs => glyph::CLOSE_ALL,
        C::CloseOtherTabs => glyph::CLOSE_OTHERS,
        C::Exit => glyph::POWER,
        C::Undo => glyph::UNDO,
        C::Redo => glyph::REDO,
        C::Cut => glyph::CUT,
        C::Copy | C::CopyFilePath => glyph::COPY,
        C::Paste => glyph::PASTE,
        C::SelectAll => glyph::SELECT_ALL,
        C::DuplicateLine => glyph::DUPLICATE_LINE,
        C::MoveLineUp | C::PreviousTab | C::PreviousBookmark | C::PreviousResult => glyph::UP,
        C::PreviousDifference | C::FindPrevious => glyph::UP,
        C::MoveLineDown | C::NextTab | C::NextBookmark | C::NextResult => glyph::DOWN,
        C::NextDifference | C::FindNext => glyph::DOWN,
        C::DeleteLine => glyph::DELETE_LINE,
        C::ToggleComment => glyph::COMMENT,
        C::UpperCase => glyph::UPPERCASE,
        C::LowerCase => glyph::LOWERCASE,
        C::TrimTrailing => glyph::TRIM,
        C::TabsToSpaces => glyph::TABS,
        C::Find | C::FindInFiles | C::CommandPalette => glyph::SEARCH,
        C::FindInTabs => glyph::FIND_TABS,
        C::Replace | C::ReplaceNext | C::ReplaceAll => glyph::REPLACE,
        C::GoToLine => glyph::GO_TO_LINE,
        C::ZoomIn => glyph::ZOOM,
        C::ZoomOut => glyph::ZOOM_OUT,
        C::ZoomReset => glyph::ZOOM_RESET,
        C::ToggleWhitespace => glyph::WHITESPACE,
        C::ToggleWrap => glyph::WRAP,
        C::ToggleMinimap => glyph::MINIMAP,
        C::ToggleBookmark => glyph::BOOKMARK,
        C::ToggleFold => glyph::FOLD,
        C::FoldAll => glyph::FOLD_ALL,
        C::UnfoldAll => glyph::UNFOLD,
        C::ToggleSplit | C::CompareOther => glyph::SPLIT,
        C::FocusOther => glyph::OTHER_PANE,
        C::CompareFile => glyph::OPEN,
        C::CompareSaved => glyph::SAVE,
        C::RecordMacro => glyph::RECORD,
        C::PlayMacro => glyph::PLAY,
        C::PlayMacroToEnd => glyph::PLAY_END,
        C::SaveMacroAs => glyph::SAVE_AS,
        C::ToggleFiles => glyph::SIDEBAR,
        C::ToggleOutline => glyph::FUNCTIONS,
        C::ThemeSystem => glyph::SYSTEM,
        C::ThemeLight => glyph::SUN,
        C::ThemeDark => glyph::MOON,
        C::ShowShortcuts => glyph::KEYBOARD,
        C::About => glyph::INFO,
        C::ShowInFileManager => glyph::FOLDER_OPEN,
        C::SortLines
        | C::SortLinesDescending
        | C::SortLinesIgnoreCase
        | C::ReverseLines
        | C::UniqueLines
        | C::JoinLines => glyph::EDIT,
        C::MdBold | C::MdItalic | C::MdCode | C::MdStrike | C::MdHeading => glyph::WORD,
        C::MdLink => glyph::LINK,
        C::MdTable => glyph::TABS,
        C::Settings => glyph::SLIDERS,
        C::Theme | C::Escape | C::MakeDefault => "",
    }
}

/// Whether a command can run now, by the conditions the menus grey their items by.
pub fn enabled(id: C, docs: &Docs, ui: &Ui, macro_idle: bool) -> bool {
    let ready = docs.active_ready();
    let has_tab = docs.active().is_some();
    let many = docs.tabs.len() > 1;
    let has_file = docs.active_doc().is_some_and(|d| d.path.is_some());
    let selected = ready.is_some_and(|r| !r.sel.range().is_empty());
    match id {
        C::Save => docs.active_doc().is_some_and(|d| d.modified()),
        C::SaveAs
        | C::Paste
        | C::SelectAll
        | C::DuplicateLine
        | C::MoveLineUp
        | C::MoveLineDown
        | C::DeleteLine
        | C::TrimTrailing
        | C::TabsToSpaces
        | C::ToggleBookmark
        | C::SortLines
        | C::SortLinesDescending
        | C::SortLinesIgnoreCase
        | C::ReverseLines
        | C::UniqueLines
        | C::JoinLines
        | C::MdBold
        | C::MdItalic
        | C::MdCode
        | C::MdStrike
        | C::MdHeading
        | C::MdLink
        | C::MdTable
        | C::GoToLine => ready.is_some(),
        C::Reload | C::CloseTab | C::CloseAllTabs => has_tab,
        C::Find
        | C::FindNext
        | C::FindPrevious
        | C::FindInFiles
        | C::FindInTabs
        | C::Replace
        | C::ReplaceNext
        | C::ReplaceAll => has_tab,
        C::CloseOtherTabs | C::NextTab | C::PreviousTab => many,
        C::CopyFilePath | C::ShowInFileManager => has_file,
        C::Undo => ready.is_some_and(|r| r.history.can_undo()),
        C::Redo => ready.is_some_and(|r| r.history.can_redo()),
        C::Cut | C::Copy | C::UpperCase | C::LowerCase => selected,
        C::ToggleComment => ready
            .and_then(|r| r.language)
            .is_some_and(|l| l.line_comment().is_some()),
        C::ClearRecent | C::OpenRecent | C::ManageRecent => !docs.recents.is_empty(),
        C::FocusOther => docs.other().is_some(),
        C::UnfoldAll => ready.is_some_and(|r| !r.folds.is_empty()),
        C::ZoomReset => docs.zoom != 100,
        C::PlayMacro | C::PlayMacroToEnd | C::SaveMacroAs => macro_idle,
        C::NextResult | C::PreviousResult | C::CloseResults => ui.results.open,
        C::ClearBookmarks => ready.is_some_and(|r| !r.bookmarks.is_empty()),
        C::NextBookmark | C::PreviousBookmark => ready.is_some_and(|r| !r.bookmarks.is_empty()),
        C::CompareOther
        | C::CompareFile
        | C::CompareSaved
        | C::StopCompare
        | C::NextDifference
        | C::PreviousDifference => crate::compare::enabled(id, docs),
        _ => true,
    }
}

/// Every command that is not the palette itself, an Escape, or the theme's cycle (which has its
/// three choices), with its first shortcut.
fn commands(src: &Sources) -> (Vec<Entry>, Vec<Target>) {
    let (mut entries, mut targets) = (Vec::new(), Vec::new());
    for c in COMMANDS {
        let listed = match c.id {
            C::CommandPalette | C::Escape | C::Theme | C::OpenRecent => false,
            C::MakeDefault => make_default_offered(),
            _ => true,
        };
        if !listed {
            continue;
        }
        let on = enabled(c.id, src.docs, src.ui, src.macro_idle);
        push(&mut entries, &mut targets, Target::Cmd(c.id), |id| {
            Entry::new(id, c.label)
                .hint(hint(c.id))
                .icon(icon(c.id))
                .group(G_PLAIN)
                .enabled(on)
        });
    }
    (entries, targets)
}

/// Off Windows, from an AppImage, until mdedit is the default: as the main menu offers it.
fn make_default_offered() -> bool {
    #[cfg(not(windows))]
    {
        mdshared_app::appimage::path().is_some()
            && !mdshared_ui::desktop_entry::is_default(
                strings::APP_NAME,
                mdshared_ui::desktop_entry::EDITOR_MIME_TYPES,
            )
    }
    #[cfg(windows)]
    {
        false
    }
}

fn symbol_entries(src: &Sources) -> (Vec<Entry>, Vec<Target>) {
    let (mut entries, mut targets) = (Vec::new(), Vec::new());
    for s in src.symbols.unwrap_or_default() {
        if s.name.trim().is_empty() {
            continue;
        }
        let at = format!("line {}", s.line + 1);
        push(&mut entries, &mut targets, Target::Symbol(s.line), |id| {
            Entry::new(id, s.name.as_str()).hint(at).group(G_PLAIN)
        });
    }
    (entries, targets)
}

fn help() -> (Vec<Entry>, Vec<Target>) {
    let (mut entries, mut targets) = (Vec::new(), Vec::new());
    for (prefix, text, hint) in [
        (
            "",
            "Open tabs, recent files and files of the folder",
            "file name",
        ),
        (">", "Commands", ">"),
        (":", "Go to a line", ":"),
        ("@", "Symbols of this document", "@"),
    ] {
        push(&mut entries, &mut targets, Target::Prefix(prefix), |id| {
            Entry::new(id, text).hint(hint).group(G_PLAIN)
        });
    }
    (entries, targets)
}

/// The palette as the window holds it: the open card, and the lists that outlive one opening.
/// Empty until the palette is first used.
#[derive(Default)]
pub struct Host {
    pub open: Option<PaletteState>,
    /// Raised by every event the palette handles, so the overlay signature changes and the
    /// frame is drawn again (the palette does not say what changed inside).
    pub tick: u64,
    files: Option<FolderFiles>,
    symbols: Option<SymbolList>,
}

impl Host {
    /// Opens the palette on `query`, with its entries.
    pub fn open(&mut self, query: &str, docs: &Docs, ui: &Ui, macros: &mut Macros) {
        self.open = Some(PaletteState::new(query));
        self.sync(docs, ui, macros, true);
        self.tick += 1;
    }

    /// Closes it. The folder's files stay listed for the next opening; the symbols go.
    pub fn close(&mut self) {
        self.open = None;
        self.symbols = None;
        self.tick += 1;
    }

    /// Brings the entries up to date after the query changed (or `force`: a list came in), and
    /// starts the worker the mode needs, once.
    pub fn sync(&mut self, docs: &Docs, ui: &Ui, macros: &mut Macros, force: bool) {
        let Some(mut state) = self.open.take() else {
            return;
        };
        let (mode, _) = mode_of(state.palette.query());
        if mode == Mode::Files && !state.palette.needle().is_empty() {
            self.ensure_files(ui.tree.root(), docs.waker());
        }
        if mode == Mode::Symbols {
            self.ensure_symbols(docs);
        }
        // The macro file is read when the commands are first listed, as the Macro menu does.
        let macro_idle =
            mode == Mode::Commands && macros.recording.is_none() && !macros.last().is_empty();
        let files = self.files.as_ref().and_then(|f| f.done.as_deref());
        let list = self.symbols.as_ref();
        let src = Sources {
            docs,
            ui,
            macro_idle,
            files,
            symbols: list.and_then(|s| s.done.as_deref()),
            no_rules: list.is_some_and(|s| s.no_rules),
        };
        state.sync(&src, force);
        self.open = Some(state);
    }

    fn ensure_files(&mut self, root: Option<&Path>, wake: Waker) {
        let Some(root) = root else {
            self.files = None;
            return;
        };
        if self.files.as_ref().is_some_and(|f| f.root == root) {
            return;
        }
        let shared = Arc::new(Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));
        let (out, flag, dir) = (shared.clone(), cancel.clone(), root.to_path_buf());
        std::thread::spawn(move || {
            let mut list: Vec<(PathBuf, String)> = Vec::new();
            walk_files(&dir, true, &flag, &mut |p| {
                let rel = p.strip_prefix(&dir).unwrap_or(p).display().to_string();
                list.push((p.to_path_buf(), rel));
                list.len() < MAX_FILES
            });
            if !flag.load(Ordering::Relaxed) {
                *out.lock().unwrap() = Some(list);
                wake(Wake::Palette);
            }
        });
        self.files = Some(FolderFiles {
            root: root.to_path_buf(),
            done: None,
            shared,
            cancel,
        });
    }

    fn ensure_symbols(&mut self, docs: &Docs) {
        let Some(doc) = docs.active_doc() else {
            self.symbols = None;
            return;
        };
        let Some(r) = doc.ready() else {
            self.symbols = None;
            return;
        };
        let key = (doc.id, r.revision);
        if self.symbols.as_ref().is_some_and(|s| s.key == key) {
            return;
        }
        let flavor = r.language.and_then(|l| Flavor::for_language(l.name()));
        let shared = Arc::new(Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));
        let mut list = SymbolList {
            key,
            done: None,
            no_rules: flavor.is_none(),
            shared: shared.clone(),
            cancel: cancel.clone(),
        };
        match flavor {
            None => list.done = Some(Vec::new()),
            Some(flavor) => {
                let (text, wake) = (r.buffer.snapshot(), docs.waker());
                std::thread::spawn(move || {
                    let (found, _) = symbols(&text, flavor, MAX_SYMBOLS, &cancel);
                    if !cancel.load(Ordering::Relaxed) {
                        *shared.lock().unwrap() = Some(found);
                        wake(Wake::Palette);
                    }
                });
            }
        }
        self.symbols = Some(list);
    }

    /// Takes in a list a worker made. True when the open palette's entries need making again.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        if let Some(f) = self.files.as_mut()
            && f.done.is_none()
            && let Some(list) = f.shared.lock().unwrap().take()
        {
            f.done = Some(list);
            changed = true;
        }
        if let Some(s) = self.symbols.as_mut()
            && s.done.is_none()
            && let Some(list) = s.shared.lock().unwrap().take()
        {
            s.done = Some(list);
            changed = true;
        }
        changed && self.open.is_some()
    }

    /// A list is still being made (`--shot` waits for it).
    pub fn pending(&self) -> bool {
        self.files.as_ref().is_some_and(|f| f.done.is_none())
            || self.symbols.as_ref().is_some_and(|s| s.done.is_none())
    }
}
