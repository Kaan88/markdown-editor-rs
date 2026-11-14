//! The command table: menus, shortcuts and the Keyboard shortcuts dialog are built from it, so the
//! keys that work are exactly the keys the dialog lists. Also the user-visible strings.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandId {
    NewFile,
    OpenFiles,
    OpenFolder,
    Save,
    SaveAs,
    Reload,
    OpenRecent,
    ManageRecent,
    ClearRecent,
    CloseTab,
    CloseAllTabs,
    Exit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    DuplicateLine,
    MoveLineUp,
    MoveLineDown,
    DeleteLine,
    ToggleComment,
    UpperCase,
    LowerCase,
    TrimTrailing,
    TabsToSpaces,
    SortLines,
    SortLinesDescending,
    SortLinesIgnoreCase,
    ReverseLines,
    UniqueLines,
    JoinLines,
    MdBold,
    MdItalic,
    MdCode,
    MdStrike,
    MdHeading,
    MdLink,
    MdTable,
    Find,
    FindNext,
    FindPrevious,
    FindInFiles,
    FindInTabs,
    NextResult,
    PreviousResult,
    CloseResults,
    Replace,
    ReplaceNext,
    ReplaceAll,
    GoToLine,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleWhitespace,
    ToggleWrap,
    ToggleMinimap,
    ToggleBookmark,
    NextBookmark,
    PreviousBookmark,
    ClearBookmarks,
    ToggleFold,
    FoldAll,
    UnfoldAll,
    ToggleSplit,
    FocusOther,
    RecordMacro,
    PlayMacro,
    PlayMacroToEnd,
    SaveMacroAs,
    DeleteMacros,
    ClearSearch,
    ToggleFiles,
    ToggleOutline,
    NextTab,
    PreviousTab,
    Escape,
    Theme,
    ThemeSystem,
    ThemeLight,
    ThemeDark,
    Settings,
    ShowShortcuts,
    About,
    CloseOtherTabs,
    CopyFilePath,
    ShowInFileManager,
    CommandPalette,
    /// Compare the two panes of the split view, the active file with another, or the text with
    /// the file on disk; leave the comparison; step through the differences.
    CompareOther,
    ShowInOtherPane,
    CompareFile,
    CompareSaved,
    StopCompare,
    NextDifference,
    PreviousDifference,
    /// Off Windows, from an AppImage: the default application for text files.
    MakeDefault,
    /// Show the file in mdview and keep it scrolled to the caret's line, or stop doing so; and
    /// the same with the two windows placed side by side.
    PreviewInViewer,
    PreviewSideBySide,
}

pub struct Command {
    pub id: CommandId,
    pub label: &'static str,
    /// Shortcuts as `keys::parse` reads them. No Ctrl+Alt: on Windows that is AltGr, which types
    /// characters on many layouts.
    pub keys: &'static [&'static str],
}

const fn cmd(id: CommandId, label: &'static str, keys: &'static [&'static str]) -> Command {
    Command { id, label, keys }
}

use CommandId as C;

const SHOW_IN_FILE_MANAGER: &str = if cfg!(windows) {
    "Show in File Explorer"
} else {
    "Show in file manager"
};

/// In the order the Keyboard shortcuts dialog lists them.
pub const COMMANDS: &[Command] = &[
    cmd(C::NewFile, "New file", &["Ctrl+N"]),
    cmd(C::OpenFiles, "Open files", &["Ctrl+O"]),
    cmd(C::OpenFolder, "Open folder", &["Ctrl+Shift+O"]),
    cmd(C::Save, "Save", &["Ctrl+S"]),
    cmd(C::SaveAs, "Save as", &["Ctrl+Shift+S"]),
    cmd(C::Reload, "Reload", &["F5"]),
    cmd(C::OpenRecent, "Open recent", &[]),
    cmd(C::ManageRecent, "Manage recent files", &[]),
    cmd(C::ClearRecent, "Clear recent files", &[]),
    cmd(C::CloseTab, "Close tab", &["Ctrl+W", "Ctrl+F4"]),
    cmd(C::CloseAllTabs, "Close all tabs", &["Ctrl+Shift+W"]),
    cmd(C::Exit, "Exit", &["Ctrl+Q"]),
    cmd(C::Undo, "Undo", &["Ctrl+Z"]),
    cmd(C::Redo, "Redo", &["Ctrl+Y", "Ctrl+Shift+Z"]),
    cmd(C::Cut, "Cut", &["Ctrl+X", "Shift+Delete"]),
    cmd(C::Copy, "Copy", &["Ctrl+C", "Ctrl+Insert"]),
    cmd(C::Paste, "Paste", &["Ctrl+V", "Shift+Insert"]),
    cmd(C::SelectAll, "Select all", &["Ctrl+A"]),
    cmd(C::DuplicateLine, "Duplicate line", &["Ctrl+D"]),
    cmd(C::MoveLineUp, "Move line up", &["Alt+Up"]),
    cmd(C::MoveLineDown, "Move line down", &["Alt+Down"]),
    cmd(C::DeleteLine, "Delete line", &["Ctrl+Shift+K"]),
    // Ctrl+/ needs Shift on layouts where "/" is on a digit key (Turkish Q), so Ctrl+K too.
    cmd(C::ToggleComment, "Toggle comment", &["Ctrl+K", "Ctrl+/"]),
    cmd(C::UpperCase, "UPPERCASE", &["Ctrl+Shift+U"]),
    cmd(C::LowerCase, "lowercase", &["Ctrl+U"]),
    cmd(C::TrimTrailing, "Trim trailing spaces", &[]),
    cmd(C::TabsToSpaces, "Tabs to spaces", &[]),
    cmd(C::SortLines, "Sort lines", &[]),
    cmd(C::SortLinesDescending, "Sort lines descending", &[]),
    cmd(C::SortLinesIgnoreCase, "Sort lines ignoring case", &[]),
    cmd(C::ReverseLines, "Reverse lines", &[]),
    cmd(C::UniqueLines, "Remove duplicate lines", &[]),
    cmd(C::JoinLines, "Join lines", &["Ctrl+J"]),
    // Markdown documents only. Ctrl+K is Toggle comment, so Link has no key.
    cmd(C::MdBold, "Bold", &["Ctrl+B"]),
    cmd(C::MdItalic, "Italic", &["Ctrl+I"]),
    cmd(C::MdCode, "Code", &[]),
    cmd(C::MdStrike, "Strikethrough", &[]),
    cmd(C::MdHeading, "Heading level", &["Ctrl+Shift+H"]),
    cmd(C::MdLink, "Link", &[]),
    cmd(C::MdTable, "Align table", &[]),
    cmd(C::Find, "Find", &["Ctrl+F"]),
    cmd(C::FindNext, "Find next", &["F3"]),
    cmd(C::FindPrevious, "Find previous", &["Shift+F3"]),
    cmd(C::FindInFiles, "Find in files", &["Ctrl+Shift+F"]),
    cmd(C::FindInTabs, "Find in open tabs", &[]),
    cmd(C::NextResult, "Next result", &["F4"]),
    cmd(C::PreviousResult, "Previous result", &["Shift+F4"]),
    cmd(C::CloseResults, "Close the results", &[]),
    cmd(C::Replace, "Replace", &["Ctrl+H"]),
    cmd(C::ReplaceNext, "Replace and find next", &[]),
    cmd(C::ReplaceAll, "Replace all", &["Ctrl+Shift+Enter"]),
    cmd(C::GoToLine, "Go to line", &["Ctrl+G"]),
    cmd(C::CommandPalette, "Command palette", &["Ctrl+P"]),
    // The shared key code takes Ctrl+= for this on US layouts. Where "+" and "=" are on digit
    // keys (Turkish Q) no key gives it: Ctrl and the wheel does.
    cmd(C::ZoomIn, "Zoom in", &["Ctrl++"]),
    cmd(C::ZoomOut, "Zoom out", &["Ctrl+-"]),
    cmd(C::ZoomReset, "Reset zoom", &["Ctrl+0"]),
    cmd(C::ToggleWhitespace, "Show whitespace", &[]),
    cmd(C::ToggleWrap, "Word wrap", &["Alt+Z"]),
    cmd(C::ToggleMinimap, "Minimap", &["Ctrl+Shift+M"]),
    cmd(C::ToggleBookmark, "Toggle bookmark", &["Ctrl+F2"]),
    cmd(C::NextBookmark, "Next bookmark", &["F2"]),
    cmd(C::PreviousBookmark, "Previous bookmark", &["Shift+F2"]),
    cmd(C::ClearBookmarks, "Clear bookmarks", &[]),
    cmd(C::ToggleFold, "Fold or open the block", &["F9"]),
    cmd(C::FoldAll, "Fold all blocks", &["Shift+F9"]),
    cmd(C::UnfoldAll, "Open all folds", &["Ctrl+F9"]),
    cmd(C::ToggleSplit, "Two files side by side", &["Shift+F8"]),
    cmd(C::FocusOther, "Go to the other pane", &["F8"]),
    cmd(C::ShowInOtherPane, "Show this file in the other pane", &[]),
    cmd(C::CompareOther, "Compare with the other pane", &[]),
    cmd(C::CompareFile, "Compare with file", &[]),
    cmd(C::CompareSaved, "Compare with saved", &[]),
    cmd(C::StopCompare, "Stop comparing", &[]),
    cmd(C::NextDifference, "Next difference", &["F7"]),
    cmd(C::PreviousDifference, "Previous difference", &["Shift+F7"]),
    cmd(
        C::PreviewInViewer,
        "Preview in mdview, or stop",
        &["Ctrl+Shift+V"],
    ),
    cmd(C::PreviewSideBySide, "Preview side by side", &[]),
    cmd(C::RecordMacro, "Record a macro, or stop", &["Ctrl+Shift+R"]),
    cmd(C::PlayMacro, "Play the macro", &["Ctrl+Shift+P"]),
    cmd(
        C::PlayMacroToEnd,
        "Play the macro to the end of the file",
        &[],
    ),
    cmd(C::SaveMacroAs, "Save the macro as", &[]),
    cmd(C::DeleteMacros, "Delete saved macros", &[]),
    cmd(C::ClearSearch, "Clear search", &[]),
    cmd(C::ToggleFiles, "Files pane", &["Ctrl+Shift+1"]),
    cmd(C::ToggleOutline, "Function list", &["Ctrl+Shift+2"]),
    cmd(C::NextTab, "Next tab", &["Ctrl+Tab"]),
    cmd(C::PreviousTab, "Previous tab", &["Ctrl+Shift+Tab"]),
    cmd(C::Escape, "Close a menu or leave the find box", &["Escape"]),
    cmd(C::Theme, "Theme", &[]),
    cmd(C::ThemeSystem, "System theme", &[]),
    cmd(C::ThemeLight, "Light theme", &[]),
    cmd(C::ThemeDark, "Dark theme", &[]),
    cmd(C::Settings, "Settings", &["Ctrl+,"]),
    cmd(C::ShowShortcuts, "Keyboard shortcuts", &[]),
    cmd(C::About, "About mdedit", &[]),
    cmd(C::CloseOtherTabs, "Close other tabs", &[]),
    cmd(C::CopyFilePath, "Copy file path", &[]),
    cmd(C::ShowInFileManager, SHOW_IN_FILE_MANAGER, &[]),
    cmd(C::MakeDefault, strings::MAKE_DEFAULT, &[]),
];

pub fn command(id: CommandId) -> &'static Command {
    COMMANDS
        .iter()
        .find(|c| c.id == id)
        .expect("every CommandId has a table entry")
}

/// The first shortcut of a command, for menu hints and tooltips; empty when it has none.
pub fn hint(id: CommandId) -> &'static str {
    command(id).keys.first().copied().unwrap_or("")
}

/// Lines for the Keyboard shortcuts dialog: (keys joined by ", ", label).
pub fn shortcut_help() -> Vec<(String, &'static str)> {
    let mut rows: Vec<(String, &'static str)> = COMMANDS
        .iter()
        .filter(|c| !c.keys.is_empty())
        .map(|c| (c.keys.join(", "), c.label))
        .collect();
    // Not commands: made with the keys and the mouse in the text.
    rows.push((strings::COLUMN_KEYS.to_string(), strings::COLUMN_LABEL));
    rows
}

pub mod strings {
    pub const APP_NAME: &str = "mdedit";
    /// The name of a new document that has no file yet.
    pub fn untitled(n: u32) -> String {
        format!("Untitled {n}")
    }
    pub const ABOUT: &str =
        "A light native text editor with syntax colours, the companion of mdview.";
    pub const ABOUT_DETAIL: &str =
        "winit and softbuffer, with every pixel drawn by the shared renderer. No webview.";
    pub const MAKE_DEFAULT: &str = "Make mdedit the default for text files";
    pub const SHORTCUTS_TITLE: &str = "Keyboard shortcuts";
    pub const COLUMN_LABEL: &str = "Select a column";
    pub const COLUMN_KEYS: &str = "Alt+Shift+Arrows, Alt+Drag";
    pub const MANAGE_RECENT_TITLE: &str = "Manage recent files";
    pub const EMPTY_HINT: &str = "Open a file or drag one into this window.";
    pub const EMPTY_BUTTON: &str = "Open file…";
    pub const RECENT_EMPTY: &str = "No recent files";
    pub const RECENT_CLEAR_LIST: &str = "Clear list";
    pub const RECENT_DONE: &str = "Done";
    pub const FIND_PLACEHOLDER: &str = "Find in file";
    pub const REPLACE_PLACEHOLDER: &str = "Replace with";
    pub const RESULTS_REPLACE_PLACEHOLDER: &str = "Replace with, in these files";
    pub const REPLACE_BUSY: &str = "Wait for the replacement to finish, or Cancel it.";
    pub const REPLACE_NOTHING: &str = "Nothing to replace in those files.";
    pub const REPLACE_BLOCKED: &str = "Another dialog is open: press Replace all… again.";
    pub const GOTO_PLACEHOLDER: &str = "Go to line, or line:column";

    /// The line and column "12", "12:5" or "12,5" names, both from 1.
    pub fn parse_line_col(text: &str) -> Option<(usize, usize)> {
        let text = text.trim();
        let (line, col) = match text.split_once([':', ',']) {
            Some((l, c)) => (l.trim(), c.trim().parse().ok()?),
            None => (text, 1),
        };
        Some((line.parse().ok()?, col))
    }
    pub const REPLACE_ONE: &str = "Replace";
    pub const REPLACE_ALL: &str = "Replace all";
    pub const BAD_PATTERN: &str = "Not a valid pattern";
    pub const NOTHING_TO_REPLACE: &str = "Nothing to replace.";
    pub const FIND_WHAT: &str = "Type what to look for in the find box first.";
    pub const NO_SEARCH_FOLDER: &str = "Open a folder or a file first: its folder is searched.";
    pub const NO_RESULTS: &str = "No results. Ctrl+Shift+F finds in files.";
    pub const RESULT_TAB_CLOSED: &str = "That tab is closed.";
    pub const NO_BOOKMARKS: &str = "No bookmarks. Ctrl+F2 sets one on the caret's line.";
    pub const NO_BLOCK: &str = "Nothing to fold here: no lines below are indented more.";
    pub const SPLIT_NEEDS_TWO: &str = "Open a second file to show two side by side.";
    pub const PALETTE_PLACEHOLDER: &str =
        "Type a file name, > for commands, : for a line, @ for symbols";
    pub const PALETTE_READING: &str = "Reading the symbols\u{2026}";
    pub const PALETTE_NO_SYMBOLS: &str = "This kind of file has no symbols to list.";
    pub const PALETTE_NO_SYMBOLS_FOUND: &str = "No symbols found in this file.";
    pub const NO_OTHER_PANE: &str = "One pane only. Shift+F8 shows two files side by side.";
    pub const MACRO_RECORDING: &str = "Recording the keys you press. Ctrl+Shift+R stops.";
    pub const MACRO_EMPTY: &str = "No key was pressed: nothing recorded.";
    pub const NO_MACRO: &str = "No macro yet. Ctrl+Shift+R records one.";
    pub const MACRO_NAME_PLACEHOLDER: &str = "Name for the macro, then Enter";
    pub const MACRO_NAME_BAD: &str = "A name has 1 to 64 characters and no control characters.";
    pub const MACROS_TITLE: &str = "Saved macros";
    pub const MACROS_EMPTY: &str = "No saved macros";
    pub const MACROS_DELETE_ALL: &str = "Delete all";
    pub fn macro_saved(name: &str) -> String {
        format!("Saved the macro as {name}.")
    }
    pub const MACRO_BUSY: &str = "Still recording. Ctrl+Shift+R stops.";

    pub fn macro_recorded(keys: usize) -> String {
        match keys {
            1 => "Macro recorded: 1 key. Ctrl+Shift+P plays it.".to_string(),
            n => format!("Macro recorded: {n} keys. Ctrl+Shift+P plays it."),
        }
    }

    pub fn macro_ran(runs: usize) -> String {
        match runs {
            1 => "The macro ran once.".to_string(),
            n => format!("The macro ran {n} times."),
        }
    }
    pub const NO_BLOCKS: &str = "Nothing to fold: no line has indented lines under it.";
    pub const FOLD_ALL_PART: &str = "Folded the first 200,000 lines; the rest was left alone.";

    /// A character the chosen encoding has no bytes for.
    pub fn cannot_encode(ch: char, line: usize, encoding: &str) -> String {
        format!("{encoding} cannot hold \u{201C}{ch}\u{201D} (line {line}).")
    }
    pub const TOO_LARGE_FOR_REPLACE_ALL: &str =
        "This file is too large to replace every match at once.";

    pub fn replaced(n: usize) -> String {
        match n {
            1 => "Replaced 1 match.".to_string(),
            n => format!("Replaced {n} matches."),
        }
    }
    pub const FIND_NO_MATCHES: &str = "No matches";
    pub const FIND_SEARCHING: &str = "Searching";
    pub const FILES_DOCK: &str = "FILES";
    pub const FILE_GONE: &str = "The file was deleted or moved. Showing the text last read.";
    pub const FILE_CHANGED: &str =
        "The file changed on disk. Saving asks before it overwrites; Reload drops your changes.";
    /// Before the name of a file with unsaved changes, on its tab and in the window title.
    pub const MODIFIED_MARK: &str = "\u{2022} ";
    pub const COPY_TOO_LARGE: &str = "The selection is too large to copy.";
    pub const UNSAVED_TITLE: &str = "Save changes?";
    pub const OVERWRITE_TITLE: &str = "The file changed on disk";
    pub const DISCARD_TITLE: &str = "Reload and lose changes?";
    pub const SAVE: &str = "Save";
    pub const DONT_SAVE: &str = "Don't save";
    pub const CANCEL: &str = "Cancel";
    pub const OVERWRITE: &str = "Overwrite";
    pub const RELOAD: &str = "Reload";

    pub fn unsaved_body(name: &str) -> String {
        format!("{name} has changes that are not saved.")
    }

    pub fn overwrite_body(name: &str) -> String {
        format!(
            "{name} was changed by another program after it was opened. Saving replaces that \
             version with this text."
        )
    }

    pub fn discard_body(name: &str) -> String {
        format!(
            "{name} has changes that are not saved. Reloading reads the file again and drops them."
        )
    }
    /// How long a toast stays, ms.
    pub const TOAST_MS: u64 = 2500;

    /// The window title: the active file's name and the application, as the viewer joins them.
    pub fn window_title(file: Option<&str>) -> String {
        match file {
            Some(f) => format!("{f} — {APP_NAME}"),
            None => APP_NAME.to_string(),
        }
    }

    /// The match counter beside the find box, as the viewer's: `current` is 1-based, 0 when no
    /// match is current.
    pub fn find_label(query_empty: bool, current: usize, total: usize, capped: bool) -> String {
        let plus = if capped { "+" } else { "" };
        match (query_empty, total, current) {
            (true, _, _) => String::new(),
            (false, 0, _) => FIND_NO_MATCHES.to_string(),
            (false, _, 0) => format!("{total}{plus} matches"),
            _ => format!("{current} of {total}{plus}"),
        }
    }

    pub const FIND_NO_MATCHES_IN_SELECTION: &str = "No matches in selection";

    /// The toast for a file read with replacement characters.
    pub fn lossy_toast(name: &str) -> String {
        format!("Some bytes in {name} could not be read and are shown as \u{FFFD}.")
    }

    pub const PREVIEW_NEEDS_FILE: &str = "Save the file first: mdview shows files.";
    pub const PREVIEW_TOO_LARGE: &str =
        "This file is too large to preview as you type: mdview shows it when you save.";
    pub const PREVIEW_TILING: &str =
        "Place the windows side by side with your desktop's tiling keys.";
    pub const NO_PLACING: &str = "This desktop does not let a window be placed";

    pub const NOT_MARKDOWN: &str = "This is for Markdown files.";
    pub const BLOCK_TOO_LARGE: &str = "That block of lines is too large to change at once.";
    pub const NO_LINE_COMMENT: &str = "This kind of file has no line comment.";
    pub const TOO_LARGE_FOR_ALL_LINES: &str =
        "This file is too large to change every line at once.";

    pub fn read_only_toast(name: &str) -> String {
        format!("{name} has bytes that could not be read, so it is open read-only.")
    }

    pub const COMPARE_NEEDS_SPLIT: &str =
        "Open two files side by side first (Shift+F8), or use Compare with file.";
    pub const COMPARE_NOT_READ: &str = "Wait until both files are read.";
    pub const COMPARE_SAME_FILE: &str = "That is the file already open here. Choose another.";
    pub const COMPARE_NOTHING_SAVED: &str = "Nothing to compare: the file has no unsaved changes.";
    pub const NO_NEXT_DIFFERENCE: &str = "No more differences below.";
    pub const NO_PREVIOUS_DIFFERENCE: &str = "No more differences above.";

    pub fn saved_copy_name(name: &str) -> String {
        format!("{name} (saved)")
    }

    pub fn compare_unreadable(name: &str, why: &str) -> String {
        format!("Could not read {name} from disk: {why}")
    }

    pub fn read_only_copy_toast(name: &str) -> String {
        format!("{name} is a read-only copy.")
    }

    pub fn binary_toast(name: &str) -> String {
        format!("{name} contains NUL characters and may not be a text file.")
    }
}

#[cfg(test)]
mod tests {
    use super::strings::*;
    use super::*;

    #[test]
    fn lines_and_columns_are_read() {
        use strings::parse_line_col as p;
        assert_eq!(p("12"), Some((12, 1)));
        assert_eq!(p(" 12 : 5 "), Some((12, 5)));
        assert_eq!(p("12,5"), Some((12, 5)));
        assert_eq!(p(""), None);
        assert_eq!(p("x"), None);
        assert_eq!(p("12:"), None);
        assert_eq!(p("-3"), None);
    }

    #[test]
    fn find_counter_strings() {
        assert_eq!(find_label(true, 0, 0, false), "");
        assert_eq!(find_label(false, 0, 0, false), "No matches");
        assert_eq!(find_label(false, 0, 12, false), "12 matches");
        assert_eq!(find_label(false, 3, 12, false), "3 of 12");
        assert_eq!(find_label(false, 0, 10_000, true), "10000+ matches");
        assert_eq!(find_label(false, 7, 10_000, true), "7 of 10000+");
    }

    #[test]
    fn titles_and_toasts() {
        assert_eq!(window_title(None), "mdedit");
        assert_eq!(window_title(Some("a.rs")), "a.rs — mdedit");
        assert!(lossy_toast("x.txt").contains('\u{FFFD}'));
        assert!(binary_toast("x.exe").starts_with("x.exe contains NUL"));
    }

    #[test]
    fn help_lists_every_bound_command() {
        let help = shortcut_help();
        assert!(
            help.iter()
                .any(|(k, l)| k == "Ctrl+C, Ctrl+Insert" && *l == "Copy")
        );
        assert!(help.iter().any(|(k, _)| k == "Ctrl+W, Ctrl+F4"));
        assert!(
            help.iter()
                .any(|(k, l)| k == "Alt+Shift+Arrows, Alt+Drag" && *l == "Select a column")
        );
        assert_eq!(hint(CommandId::FindNext), "F3");
        assert_eq!(hint(CommandId::About), "");
        for c in COMMANDS {
            assert_eq!(command(c.id).label, c.label);
        }
    }
}
