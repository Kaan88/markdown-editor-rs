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
use mdshared_ui::gfx::{Face, Painter, Rect};
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
/// The notes library: one group per library file, from this number up.
const G_LIBRARY: u8 = 10;
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
    /// A saved AI prompt, by its place in the file.
    AiPrompt(usize),
    /// A language for the current document (`None`: plain text).
    Language(Option<mdedit_highlight::Language>),
    /// A note of the library: opened as any file is.
    Note(PathBuf),
    /// Where the open file is written in the library: a library file and the heading under it
    /// (empty for the end of the file).
    NotePlace(PathBuf, Vec<String>),
    /// Where the note the pane is moving goes: a library file and the heading under it.
    NoteMoveTo(PathBuf, Vec<String>),
    /// A library file, to be opened as the Markdown file it is.
    LibraryFile(PathBuf),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Files,
    Commands,
    Line,
    Symbols,
    Help,
    /// The language list: opened by itself (the status bar's language field, the "Language"
    /// command), never by a prefix.
    Languages,
    /// The notes of the library, and where in it the open file is written. Both are opened by
    /// their command, never by a prefix.
    Library,
    LibraryAdd,
    /// Where a note of the pane is moved to: the same places, asked for another way.
    LibraryMove,
    /// The library's files, to open one for editing.
    LibraryFiles,
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
    /// The mode it was opened in, whatever the query: the language list.
    fixed: Option<Mode>,
    /// The language list stands above the status bar's language field.
    pub anchored: bool,
    /// The language list's small print, drawn over the footer ([`draw_note`]).
    pub note: Option<String>,
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
            fixed: None,
            anchored: false,
            note: None,
        }
    }

    /// The mode the entries come from: the one it was opened in, or the query's prefix.
    fn mode(&self) -> (Mode, usize) {
        match self.fixed {
            Some(mode) => (mode, 0),
            None => mode_of(self.palette.query()),
        }
    }

    /// Swaps the entries when the query's prefix (or, for files, whether it has text) changed
    /// the source, or `force` says the data behind them changed (a list came in). Typing within
    /// a mode only re-ranks, which the palette does by itself. The go-to line's message follows
    /// every change of the query.
    fn sync(&mut self, src: &Sources, force: bool) {
        let (mode, skip) = self.mode();
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
                Mode::Languages => {
                    self.note = Some(language_note(src.docs));
                    languages(src)
                }
                Mode::Library => library(src),
                Mode::LibraryAdd => library_places(src, false),
                Mode::LibraryMove => library_places(src, true),
                Mode::LibraryFiles => library_files(src),
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
                Mode::Library if entries.is_empty() => Some(strings::LIBRARY_EMPTY.to_string()),
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
        let (mode, _) = self.mode();
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
        C::ClearRecent | C::DeleteMacros | C::ClearBookmarks | C::DeleteBookmarkedLines => {
            glyph::DELETE
        }
        C::CutBookmarkedLines => glyph::CUT,
        C::CopyBookmarkedLines => glyph::COPY,
        C::CloseTab | C::CloseResults | C::ClearSearch | C::StopCompare => glyph::CLOSE,
        C::CloseAllTabs => glyph::CLOSE_ALL,
        C::CloseOtherTabs => glyph::CLOSE_OTHERS,
        C::PinTab => glyph::PIN,
        C::Exit => glyph::POWER,
        C::Undo => glyph::UNDO,
        C::Redo => glyph::REDO,
        C::Cut => glyph::CUT,
        C::Copy | C::CopyFilePath => glyph::COPY,
        C::Paste | C::PastePlain => glyph::PASTE,
        C::SelectAll | C::SelectNextOccurrence => glyph::SELECT_ALL,
        C::DuplicateLine => glyph::DUPLICATE_LINE,
        C::MoveLineUp | C::PreviousTab | C::PreviousBookmark | C::PreviousResult => glyph::UP,
        C::PreviousDifference | C::FindPrevious => glyph::UP,
        C::MoveLineDown | C::NextTab | C::NextBookmark | C::NextResult => glyph::DOWN,
        C::NextDifference | C::FindNext => glyph::DOWN,
        C::CopyChangeAcross => glyph::FORWARD,
        C::TakeChangeAcross => glyph::BACK,
        C::CopyLineAcross => glyph::FORWARD,
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
        C::ToggleSplit | C::CompareOther | C::ShowInOtherPane => glyph::SPLIT,
        C::PreviewInViewer | C::PreviewSideBySide | C::PreviewPane | C::VisualEdit => glyph::VIEW,
        C::FocusOther | C::FocusResults => glyph::OTHER_PANE,
        C::CompareFile => glyph::OPEN,
        C::CompareSaved => glyph::SAVE,
        C::RecordMacro => glyph::RECORD,
        C::PlayMacro => glyph::PLAY,
        C::PlayMacroToEnd => glyph::PLAY_END,
        C::SaveMacroAs => glyph::SAVE_AS,
        C::ToggleFiles => glyph::SIDEBAR,
        C::ToggleOutline => glyph::FUNCTIONS,
        C::ToggleLibraryPane => glyph::SIDEBAR,
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
        C::MdHeading1 | C::MdHeading2 | C::MdHeading3 => glyph::HEADING,
        C::MdBulletList | C::MdNumberedList => glyph::OUTLINE,
        C::MdTaskList => glyph::TASK,
        C::MdQuote | C::MdDetails | C::MdRule | C::MdFootnote => glyph::EDIT,
        C::MdAlert => glyph::INFO,
        C::MdCodeBlock => glyph::CODE,
        C::MdMath | C::MdMathBlock | C::MdDiagram => glyph::FUNCTIONS,
        C::MdImage => glyph::IMAGE,
        C::ToggleFormatBar => glyph::BOLD,
        C::MdRowBelow | C::MdRowAbove | C::MdColumnRight | C::MdColumnLeft => glyph::PLUS,
        C::MdDeleteRow | C::MdDeleteColumn => glyph::DELETE,
        C::MdMoveColumnLeft => glyph::BACK,
        C::MdMoveColumnRight => glyph::FORWARD,
        C::MdMoveRowUp => glyph::UP,
        C::MdMoveRowDown => glyph::DOWN,
        C::MdSortColumn => glyph::EDIT,
        C::Settings => glyph::SLIDERS,
        C::CompleteWord => glyph::EDIT,
        C::OpenSnippets => glyph::DOC,
        C::AiFormat | C::AiFix | C::AiRewrite => crate::chrome::SPARKLE,
        C::AiEditPrompts => glyph::DOC,
        C::AiSettings => glyph::SLIDERS,
        C::AiCancel => glyph::STOP,
        C::AiShowResult => crate::chrome::SPARKLE,
        C::SplitIntoNotes => glyph::SPLIT,
        C::AiRuns => glyph::MACRO,
        C::AiTaskFiles | C::AiTaskThisFile => crate::chrome::SPARKLE,
        C::Language => glyph::CODE,
        C::Library | C::LibraryAdd | C::ShowInLibrary => glyph::BOOKMARK,
        C::LibraryAddFolder => glyph::FOLDER_OPEN,
        C::LibraryEdit => glyph::DOC,
        C::Theme | C::Escape | C::MakeDefault => "",
    }
}

/// Whether a command can run now, by the conditions the menus grey their items by.
/// Why "Split into notes…" cannot run on the document in front, in the words the command uses;
/// `None` when it can.
pub fn split_reason(docs: &Docs) -> Option<String> {
    let doc = docs.active_doc()?;
    let name = doc.name();
    (doc.path.is_none() || doc.modified())
        .then(|| format!("Save {name} first: the notes are made from the file on disk."))
}

pub fn enabled(id: C, docs: &Docs, ui: &Ui, macro_idle: bool) -> bool {
    let ready = docs.active_ready();
    let has_tab = docs.active().is_some();
    let many = docs.tabs.len() > 1;
    let has_file = docs.active_doc().is_some_and(|d| d.path.is_some());
    let selected = ready.is_some_and(|r| !r.view.sel.range().is_empty());
    match id {
        C::Save => docs.active_doc().is_some_and(|d| d.modified()),
        C::SaveAs
        | C::Paste
        | C::PastePlain
        | C::SelectAll
        | C::SelectNextOccurrence
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
        | C::MdHeading1
        | C::MdHeading2
        | C::MdHeading3
        | C::MdBulletList
        | C::MdNumberedList
        | C::MdTaskList
        | C::MdQuote
        | C::MdAlert
        | C::MdCodeBlock
        | C::MdMath
        | C::MdMathBlock
        | C::MdDiagram
        | C::MdDetails
        | C::MdRule
        | C::MdFootnote
        | C::MdImage
        | C::CompleteWord
        | C::MdRowBelow
        | C::MdRowAbove
        | C::MdDeleteRow
        | C::MdColumnRight
        | C::MdColumnLeft
        | C::MdDeleteColumn
        | C::MdMoveColumnLeft
        | C::MdMoveColumnRight
        | C::MdMoveRowUp
        | C::MdMoveRowDown
        | C::MdSortColumn
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
        C::PinTab => has_tab,
        C::AiFormat | C::AiFix | C::AiRewrite => ready.is_some() && docs.ai_status.is_none(),
        C::AiCancel => docs.ai_status.is_some(),
        C::AiShowResult => docs.ai_kept,
        C::SplitIntoNotes => has_file && docs.active_doc().is_some_and(|d| !d.modified()),
        C::AiRuns => true,
        C::AiTaskFiles => true,
        C::AiTaskThisFile => has_file,
        C::Language => ready.is_some(),
        C::LibraryAdd => has_file,
        C::LibraryAddFolder => true,
        C::ShowInLibrary => docs
            .active_doc()
            .and_then(|d| d.path.as_deref())
            .is_some_and(|p| crate::library_pane::lists(&docs.library, p)),
        C::CopyFilePath | C::ShowInFileManager => has_file,
        // A document that has no file is previewed from its text.
        C::PreviewInViewer | C::PreviewSideBySide | C::PreviewPane | C::VisualEdit => has_tab,
        C::Undo => ready.is_some_and(|r| r.history.can_undo()),
        C::Redo => ready.is_some_and(|r| r.history.can_redo()),
        // Copy takes the page's selection while the page has the keyboard.
        C::Copy => selected || ui.page_selected,
        C::Cut | C::UpperCase | C::LowerCase => selected,
        C::ToggleComment => ready
            .and_then(|r| r.language)
            .is_some_and(|l| l.line_comment().is_some()),
        C::ClearRecent | C::OpenRecent | C::ManageRecent => !docs.recents.is_empty(),
        C::FocusOther => docs.other().is_some() || docs.same_file,
        C::ShowInOtherPane => ready.is_some(),
        C::UnfoldAll => ready.is_some_and(|r| !r.view.folds.is_empty()),
        C::ZoomReset => docs.zoom != 100,
        C::PlayMacro | C::PlayMacroToEnd | C::SaveMacroAs => macro_idle,
        C::NextResult | C::PreviousResult | C::CloseResults | C::FocusResults => ui.results.open,
        C::ClearBookmarks
        | C::CutBookmarkedLines
        | C::CopyBookmarkedLines
        | C::DeleteBookmarkedLines => ready.is_some_and(|r| !r.bookmarks.is_empty()),
        C::NextBookmark | C::PreviousBookmark => ready.is_some_and(|r| !r.bookmarks.is_empty()),
        C::CompareOther
        | C::CompareFile
        | C::CompareSaved
        | C::StopCompare
        | C::NextDifference
        | C::PreviousDifference
        | C::CopyChangeAcross
        | C::TakeChangeAcross
        | C::CopyLineAcross => crate::compare::enabled(id, docs),
        _ => true,
    }
}

/// Whether the palette lists a command at all: not the palette itself, an Escape, or the theme's
/// cycle (which has its three choices).
pub fn listed(id: C, docs: &Docs) -> bool {
    match id {
        // "AI task on this file…" belongs where the pointer names the file, a tab's menu:
        // the palette has "AI task…", whose Add menu offers the document in front (D-207).
        C::CommandPalette | C::Escape | C::Theme | C::OpenRecent | C::AiTaskThisFile => false,
        C::MakeDefault => make_default_offered(),
        // Without mdview the previews in it are not listed, not even greyed; the pane is
        // listed while it is offered.
        C::PreviewInViewer | C::PreviewSideBySide => docs.viewer,
        C::PreviewPane | C::VisualEdit => docs.pane,
        _ => true,
    }
}

/// Every command [`listed`], with its first shortcut.
fn commands(src: &Sources) -> (Vec<Entry>, Vec<Target>) {
    let (mut entries, mut targets) = (Vec::new(), Vec::new());
    for c in COMMANDS {
        if !listed(c.id, src.docs) {
            continue;
        }
        let on = enabled(c.id, src.docs, src.ui, src.macro_idle);
        let hint = match (c.id, on) {
            (C::SplitIntoNotes, false) => split_reason(src.docs).unwrap_or_default(),
            _ => hint(c.id).to_string(),
        };
        push(&mut entries, &mut targets, Target::Cmd(c.id), |id| {
            Entry::new(id, c.label)
                .hint(hint.as_str())
                .icon(icon(c.id))
                .group(G_PLAIN)
                .enabled(on)
        });
    }
    let ai_on = enabled(C::AiRewrite, src.docs, src.ui, src.macro_idle);
    for (k, p) in src.docs.ai_prompts.iter().enumerate() {
        let label = format!("AI: {}", p.name);
        push(&mut entries, &mut targets, Target::AiPrompt(k), |id| {
            Entry::new(id, label.as_str())
                .icon(glyph::PLAY)
                .group(G_PLAIN)
                .enabled(ai_on)
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

/// The language list: plain text, Markdown, then every other language by name; the document's own checked, and
/// Markdown marked as the one with a preview.
fn languages(src: &Sources) -> (Vec<Entry>, Vec<Target>) {
    let (mut entries, mut targets) = (Vec::new(), Vec::new());
    let current = src.docs.active_ready().map(|r| r.language);
    let markdown = crate::languages::markdown();
    // Markdown, the one with a preview, right under plain text.
    let rest = crate::languages::all()
        .into_iter()
        .filter(|l| Some(*l) != markdown)
        .map(Some);
    let list = [None, markdown].into_iter().chain(rest);
    for lang in list {
        let label = crate::languages::name_of(lang);
        let icon = if current == Some(lang) {
            glyph::CHECK
        } else {
            ""
        };
        let hint = if lang.is_some() && lang == markdown {
            "preview"
        } else {
            ""
        };
        push(&mut entries, &mut targets, Target::Language(lang), |id| {
            Entry::new(id, label).hint(hint).icon(icon).group(G_PLAIN)
        });
    }
    (entries, targets)
}

/// A note's headings as one line: `Money › 2026`.
pub fn section_path(sections: &[String]) -> String {
    sections.join(" \u{203a} ")
}

/// The notes of the library, each under the group (the library file) it is in: the title is the
/// row, the section and a note that is no longer on disk are its hint. A missing note cannot be
/// chosen; the library file is where it is put right.
fn library(src: &Sources) -> (Vec<Entry>, Vec<Target>) {
    let (mut entries, mut targets) = (Vec::new(), Vec::new());
    for (g, group) in src.docs.library.groups.iter().enumerate() {
        for e in &group.entries {
            let section = section_path(&e.sections);
            let hint = match (section.as_str(), e.missing) {
                ("", false) => String::new(),
                ("", true) => strings::LIBRARY_MISSING.to_string(),
                (section, false) => section.to_string(),
                (section, true) => format!("{section} · {}", strings::LIBRARY_MISSING),
            };
            let group_id = G_LIBRARY.saturating_add(g as u8);
            push(
                &mut entries,
                &mut targets,
                Target::Note(e.path.clone()),
                |id| {
                    Entry::new(id, e.title.as_str())
                        .hint(hint)
                        .group(group_id)
                        .enabled(!e.missing)
                },
            );
        }
    }
    (entries, targets)
}

/// The group headers of the library list, one per library file.
fn library_groups(docs: &Docs) -> Vec<(u8, String)> {
    docs.library
        .groups
        .iter()
        .enumerate()
        .map(|(g, group)| (G_LIBRARY.saturating_add(g as u8), group.name.clone()))
        .collect()
}

/// Where the open file can be written in the library: every section of every group, each
/// group's end, and, while the library has no file at all, the first one to make.
fn library_places(src: &Sources, moving: bool) -> (Vec<Entry>, Vec<Target>) {
    let (mut entries, mut targets) = (Vec::new(), Vec::new());
    let row = |entries: &mut Vec<Entry>,
               targets: &mut Vec<Target>,
               file: &Path,
               section: &[String],
               label: String,
               hint: String| {
        let target = if moving {
            Target::NoteMoveTo(file.to_path_buf(), section.to_vec())
        } else {
            Target::NotePlace(file.to_path_buf(), section.to_vec())
        };
        push(entries, targets, target, |id| {
            Entry::new(id, label.as_str()).hint(hint).group(G_PLAIN)
        });
    };
    for group in &src.docs.library.groups {
        for section in &group.sections {
            row(
                &mut entries,
                &mut targets,
                &group.file,
                section,
                format!("{} \u{203a} {}", group.name, section_path(section)),
                String::new(),
            );
        }
        row(
            &mut entries,
            &mut targets,
            &group.file,
            &[],
            group.name.clone(),
            strings::LIBRARY_END.to_string(),
        );
    }
    if src.docs.library.groups.is_empty() {
        let file = src
            .docs
            .library
            .dir
            .join(mdshared_app::library::DEFAULT_FILE);
        let label = mdshared_app::library::DEFAULT_FILE.to_string();
        row(
            &mut entries,
            &mut targets,
            &file,
            &[],
            label,
            strings::LIBRARY_NEW_FILE.to_string(),
        );
    }
    (entries, targets)
}

/// The library's files, one row each: the group's name, and the file's own name as the hint.
fn library_files(src: &Sources) -> (Vec<Entry>, Vec<Target>) {
    let (mut entries, mut targets) = (Vec::new(), Vec::new());
    for group in &src.docs.library.groups {
        let file = group
            .file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        push(
            &mut entries,
            &mut targets,
            Target::LibraryFile(group.file.clone()),
            |id| {
                Entry::new(id, group.name.as_str())
                    .hint(file)
                    .group(G_PLAIN)
            },
        );
    }
    (entries, targets)
}

/// What the language list says under its rows: a choice is kept for the file, and the
/// extension's language is the way back.
pub fn language_note(docs: &Docs) -> String {
    let own = docs.active().map(|i| docs.own_language(i));
    let own = crate::languages::name_of(own.flatten());
    if docs.active_ready().is_some_and(|r| r.language_set) {
        format!("Kept for this file. Back to the extension's: {own}.")
    } else {
        format!("From the extension: {own}. A choice is kept for this file.")
    }
}

/// The width of the language list.
const LANGUAGES_W: f32 = 380.0;
/// The shared palette's margin beside its card (its `MARGIN`) and its footer's height.
const CARD_MARGIN: f32 = 24.0;
const FOOTER_H: f32 = 28.0;

/// The area to lay the palette out in so its card stands just above `field` (the status bar's
/// language field), its right edge at the field's, no higher than `top`. The shared palette
/// centres its card near the top of what it is given: it is given a narrow area that ends at the
/// field, moved down once the card's height is known.
fn anchored_window(p: &mut Palette, field: Rect, w: f32, top: f32) -> Rect {
    let card_w = LANGUAGES_W.min(w - 16.0).max(240.0);
    let x = (field.right() - card_w).min(w - card_w - 8.0).max(8.0) - CARD_MARGIN;
    // The card ends 16 px above the area's bottom when it is as tall as the area allows.
    let bottom = field.y - 4.0 + 16.0;
    // The card's top is the area's top plus 12% of its height: put it at `top`.
    let y = (top - 0.12 * bottom) / 0.88;
    let area = Rect::new(x, y, card_w + 2.0 * CARD_MARGIN, bottom - y);
    let card = p.layout(area, 0.0);
    let dy = (field.y - 4.0 - card.bottom()).max(0.0);
    Rect::new(area.x, area.y + dy, area.w, area.h)
}

/// The same placement for damage and drawing. The language list uses the status field.
pub fn placement(open: &mut PaletteState, field: Option<Rect>, w: f32, h: f32) -> (Rect, f32) {
    match field.filter(|_| open.anchored) {
        Some(field) => (
            anchored_window(&mut open.palette, field, w, crate::ui::TITLE_H + 8.0),
            0.0,
        ),
        None => (Rect::new(0.0, 0.0, w, h), crate::ui::TITLE_H),
    }
}

/// The language list's small print in place of the footer's count and keys (the shared footer
/// gives its text a third of the width).
pub fn draw_note(p: &mut Painter, pal: &mdshared_ui::theme::Palette, card: Rect, note: &str) {
    if card.h < 50.0 + FOOTER_H {
        return;
    }
    let f = Rect::new(card.x, card.bottom() - FOOTER_H, card.w, FOOTER_H);
    p.fill(
        Rect::new(f.x + 10.0, f.y + 1.0, f.w - 20.0, f.h - 3.0),
        pal.popup,
    );
    p.text(
        Face::Ui,
        11.5,
        f.x + 16.0,
        f.cy() + 1.0,
        note,
        pal.faint,
        f.w - 32.0,
    );
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

    /// Opens the language list: anchored above the status bar's language field.
    pub fn open_languages(&mut self, docs: &Docs, ui: &Ui, macros: &mut Macros) {
        let mut state = PaletteState::new("");
        state.palette.set_placeholder("Type a language");
        state.palette.set_groups(Vec::new());
        state.fixed = Some(Mode::Languages);
        state.anchored = true;
        self.open = Some(state);
        self.sync(docs, ui, macros, true);
        self.tick += 1;
    }

    /// Opens the notes library: its notes under a header per library file.
    pub fn open_library(&mut self, docs: &Docs, ui: &Ui, macros: &mut Macros) {
        let mut state = PaletteState::new("");
        state.palette.set_placeholder(strings::LIBRARY_PLACEHOLDER);
        state.palette.set_groups(library_groups(docs));
        state.fixed = Some(Mode::Library);
        self.open = Some(state);
        self.sync(docs, ui, macros, true);
        self.tick += 1;
    }

    /// Opens the list of places the open file can be written in the library.
    pub fn open_library_add(&mut self, docs: &Docs, ui: &Ui, macros: &mut Macros) {
        let mut state = PaletteState::new("");
        state
            .palette
            .set_placeholder(strings::LIBRARY_ADD_PLACEHOLDER);
        state.palette.set_groups(Vec::new());
        state.fixed = Some(Mode::LibraryAdd);
        self.open = Some(state);
        self.sync(docs, ui, macros, true);
        self.tick += 1;
    }

    /// Opens the list of places a note of the pane can be moved to.
    pub fn open_library_move(&mut self, docs: &Docs, ui: &Ui, macros: &mut Macros) {
        let mut state = PaletteState::new("");
        state
            .palette
            .set_placeholder(strings::LIBRARY_MOVE_PLACEHOLDER);
        state.palette.set_groups(Vec::new());
        state.fixed = Some(Mode::LibraryMove);
        self.open = Some(state);
        self.sync(docs, ui, macros, true);
        self.tick += 1;
    }

    /// Opens the list of library files, to choose the one to edit.
    pub fn open_library_files(&mut self, docs: &Docs, ui: &Ui, macros: &mut Macros) {
        let mut state = PaletteState::new("");
        state
            .palette
            .set_placeholder(strings::LIBRARY_FILES_PLACEHOLDER);
        state.palette.set_groups(Vec::new());
        state.fixed = Some(Mode::LibraryFiles);
        self.open = Some(state);
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
        let (mode, _) = state.mode();
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
