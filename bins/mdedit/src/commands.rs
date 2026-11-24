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
    /// The clipboard's text as it is: no image, no link made of an image's path, no
    /// conversion of rich text (issue #8).
    PastePlain,
    SelectAll,
    /// With several carets: adds the next match of the selected text (the word at a caret).
    SelectNextOccurrence,
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
    /// The format bar's Markdown: heading levels, lists, quotes, alerts, blocks, inline math,
    /// footnotes, images.
    MdHeading1,
    MdHeading2,
    MdHeading3,
    MdBulletList,
    MdNumberedList,
    MdTaskList,
    MdQuote,
    MdAlert,
    MdCodeBlock,
    MdMath,
    MdMathBlock,
    MdDiagram,
    MdDetails,
    MdRule,
    MdFootnote,
    MdImage,
    /// The format bar under the command bar, for Markdown documents.
    ToggleFormatBar,
    MdRowBelow,
    MdRowAbove,
    MdDeleteRow,
    MdColumnRight,
    MdColumnLeft,
    MdDeleteColumn,
    MdMoveColumnLeft,
    MdMoveColumnRight,
    MdMoveRowUp,
    MdMoveRowDown,
    MdSortColumn,
    Find,
    FindNext,
    FindPrevious,
    FindInFiles,
    FindInTabs,
    NextResult,
    PreviousResult,
    CloseResults,
    FocusResults,
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
    /// The bookmarked lines to the clipboard, or to the clipboard and out of the text, or out of
    /// the text.
    CopyBookmarkedLines,
    CutBookmarkedLines,
    DeleteBookmarkedLines,
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
    /// The left pane shows the notes library's tree.
    ToggleLibraryPane,
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
    /// Pins the tab, or unpins a pinned one.
    PinTab,
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
    /// In a comparison, with the caret in a difference: this side's lines replace the other
    /// side's, or the other side's replace this side's.
    CopyChangeAcross,
    TakeChangeAcross,
    CopyLineAcross,
    /// Off Windows, from an AppImage: the default application for text files.
    MakeDefault,
    /// Show the file in mdview and keep it scrolled to the caret's line, or stop doing so; and
    /// the same with the two windows placed side by side.
    PreviewInViewer,
    PreviewSideBySide,
    /// The preview pane beside the text, drawn by mdview's renderer; again closes it.
    PreviewPane,
    /// Markdown shown as its page, one block at a time edited as its source in place (Ctrl+E);
    /// again goes back to the text.
    VisualEdit,
    /// The word and snippet list at the caret; the snippets file.
    CompleteWord,
    OpenSnippets,
    /// The language list: the document's language, chosen over its extension's.
    Language,
    /// The notes library: the list of notes kept in the `library` folder, adding the open file
    /// to it under a heading, and the library files themselves.
    Library,
    LibraryAdd,
    LibraryEdit,
    /// "Add a folder to the notes library…": a folder of files, listed by its subfolders.
    LibraryAddFolder,
    /// Shows the open file's note in the pane of the notes library.
    ShowInLibrary,
    /// AI rewriting of the selection, or of the whole document when nothing is selected:
    /// the two built-in requests, one typed, the saved prompts file, the AI settings, and
    /// stopping a rewrite that is running.
    AiFormat,
    AiFix,
    AiRewrite,
    AiEditPrompts,
    AiSettings,
    AiCancel,
    /// Brings back the review of the last result, which Escape closed.
    AiShowResult,
    /// "Split into notes…" on the document in front: the plan of notes, cut at its headings.
    SplitIntoNotes,
    /// "AI runs…": the records of the runs that wrote notes, and undo of any of them.
    AiRuns,
    /// "AI task on your files…": the task window over files the owner picks (5.6).
    AiTaskFiles,
    /// "AI task on this file…": the same window with the document in front in it.
    AiTaskThisFile,
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
} else if cfg!(target_os = "macos") {
    "Show in Finder"
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
    cmd(C::PastePlain, "Paste as plain text", &["Ctrl+Shift+V"]),
    cmd(C::SelectAll, "Select all", &["Ctrl+A"]),
    cmd(
        C::SelectNextOccurrence,
        "Select next occurrence",
        &["Ctrl+Shift+D"],
    ),
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
    cmd(C::MdHeading1, "Heading 1", &["Ctrl+1"]),
    cmd(C::MdHeading2, "Heading 2", &["Ctrl+2"]),
    cmd(C::MdHeading3, "Heading 3", &["Ctrl+3"]),
    cmd(C::MdBulletList, "Bulleted list", &[]),
    cmd(C::MdNumberedList, "Numbered list", &[]),
    cmd(C::MdTaskList, "Task list", &[]),
    cmd(C::MdQuote, "Quote", &[]),
    cmd(
        C::MdAlert,
        "Alert (note, tip, important, warning, caution)",
        &[],
    ),
    cmd(C::MdCodeBlock, "Code block", &[]),
    cmd(C::MdMath, "Math", &[]),
    cmd(C::MdMathBlock, "Math block", &[]),
    cmd(C::MdDiagram, "Diagram", &[]),
    cmd(C::MdDetails, "Collapsible section", &[]),
    cmd(C::MdRule, "Horizontal rule", &[]),
    cmd(C::MdFootnote, "Footnote", &[]),
    cmd(C::MdImage, "Image", &[]),
    cmd(C::MdRowBelow, "Insert row below", &[]),
    cmd(C::MdRowAbove, "Insert row above", &[]),
    cmd(C::MdDeleteRow, "Delete row", &[]),
    cmd(C::MdColumnRight, "Insert column right", &[]),
    cmd(C::MdColumnLeft, "Insert column left", &[]),
    cmd(C::MdDeleteColumn, "Delete column", &[]),
    cmd(C::MdMoveColumnLeft, "Move column left", &[]),
    cmd(C::MdMoveColumnRight, "Move column right", &[]),
    cmd(C::MdMoveRowUp, "Move row up", &[]),
    cmd(C::MdMoveRowDown, "Move row down", &[]),
    cmd(C::MdSortColumn, "Sort table by this column", &[]),
    cmd(C::Find, "Find", &["Ctrl+F"]),
    cmd(C::FindNext, "Find next", &["F3"]),
    cmd(C::FindPrevious, "Find previous", &["Shift+F3"]),
    cmd(C::FindInFiles, "Find in files", &["Ctrl+Shift+F"]),
    cmd(C::FindInTabs, "Find in open tabs", &[]),
    cmd(C::NextResult, "Next result", &["F4"]),
    cmd(C::PreviousResult, "Previous result", &["Shift+F4"]),
    cmd(C::CloseResults, "Close the results", &[]),
    cmd(C::FocusResults, "Go to the results", &["F6"]),
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
    cmd(C::ToggleFormatBar, "Format bar", &[]),
    cmd(C::ToggleBookmark, "Toggle bookmark", &["Ctrl+F2"]),
    cmd(C::NextBookmark, "Next bookmark", &["F2"]),
    cmd(C::PreviousBookmark, "Previous bookmark", &["Shift+F2"]),
    cmd(C::ClearBookmarks, "Clear bookmarks", &[]),
    cmd(C::CutBookmarkedLines, "Cut bookmarked lines", &[]),
    cmd(C::CopyBookmarkedLines, "Copy bookmarked lines", &[]),
    cmd(C::DeleteBookmarkedLines, "Delete bookmarked lines", &[]),
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
        C::CopyChangeAcross,
        "Copy this change to the other side",
        &[],
    ),
    cmd(
        C::TakeChangeAcross,
        "Take this change from the other side",
        &[],
    ),
    cmd(C::CopyLineAcross, "Copy this line to the other side", &[]),
    cmd(
        C::PreviewInViewer,
        "Preview in mdview, or stop",
        // Ctrl+Shift+V is plain-text paste, as in most editors (issue #8).
        &["Ctrl+Shift+L"],
    ),
    cmd(C::PreviewSideBySide, "Preview side by side", &[]),
    cmd(C::PreviewPane, "Preview in this window, or close it", &[]),
    cmd(
        C::VisualEdit,
        "Edit on the page, or back to the text",
        &["Ctrl+E"],
    ),
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
    cmd(
        C::ToggleLibraryPane,
        "Notes library pane",
        &["Ctrl+Shift+3"],
    ),
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
    cmd(C::PinTab, "Pin or unpin tab", &[]),
    cmd(C::CopyFilePath, "Copy file path", &[]),
    cmd(C::ShowInFileManager, SHOW_IN_FILE_MANAGER, &[]),
    cmd(C::MakeDefault, strings::MAKE_DEFAULT, &[]),
    // Ctrl+Space can be taken by an input method on Linux (switching layouts), so Alt+/ too.
    cmd(C::CompleteWord, "Complete word", &["Ctrl+Space", "Alt+/"]),
    cmd(C::OpenSnippets, "Open snippets file", &[]),
    cmd(C::Language, "Language", &[]),
    cmd(C::Library, "Notes library", &["Ctrl+Shift+N"]),
    cmd(
        C::LibraryAddFolder,
        "Add a folder to the notes library.",
        &[],
    ),
    cmd(C::LibraryAdd, "Add this file to the notes library…", &[]),
    cmd(C::LibraryEdit, "Edit a notes library file…", &[]),
    cmd(C::ShowInLibrary, "Show this file in the notes library", &[]),
    cmd(C::AiFormat, "AI: Format as Markdown", &[]),
    cmd(C::AiFix, "AI: Fix Markdown", &[]),
    cmd(C::AiRewrite, "AI: Rewrite with instruction…", &[]),
    cmd(C::AiEditPrompts, "AI: Edit saved prompts", &[]),
    cmd(C::AiSettings, "AI: Settings", &[]),
    cmd(C::AiCancel, "AI: Stop rewriting", &[]),
    cmd(C::AiShowResult, "AI: Show the last result", &[]),
    cmd(C::SplitIntoNotes, "Split into notes\u{2026}", &[]),
    cmd(C::AiRuns, "AI runs\u{2026}", &[]),
    cmd(C::AiTaskFiles, "AI task on your files\u{2026}", &[]),
    cmd(C::AiTaskThisFile, "AI task on this file\u{2026}", &[]),
];

pub fn command(id: CommandId) -> &'static Command {
    COMMANDS
        .iter()
        .find(|c| c.id == id)
        .expect("every CommandId has a table entry")
}

/// The first shortcut of a command, for menu hints and tooltips, as this platform has it and
/// prints it ("⌘O" on a Mac); empty when it has none.
pub fn hint(id: CommandId) -> &'static str {
    let first = command(id).keys.first().copied().unwrap_or("");
    mdshared_ui::keys::label(mdshared_ui::keys::for_platform(first))
}

/// Lines for the Keyboard shortcuts dialog: (keys joined by ", ", label), each shortcut as this
/// platform has it and prints it.
pub fn shortcut_help() -> Vec<(String, &'static str)> {
    use mdshared_ui::keys::{for_platform, label};
    let mut rows: Vec<(String, &'static str)> = COMMANDS
        .iter()
        .filter(|c| !c.keys.is_empty())
        .map(|c| {
            let keys: Vec<&str> = c.keys.iter().map(|k| label(for_platform(k))).collect();
            (keys.join(", "), c.label)
        })
        .collect();
    // Not commands: made with the keys and the mouse in the text.
    let column = if mdshared_ui::keys::MAC {
        strings::COLUMN_KEYS_MAC
    } else {
        strings::COLUMN_KEYS
    };
    rows.push((column.to_string(), strings::COLUMN_LABEL));
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
    /// Closing a pinned tab.
    pub const PINNED: &str = "This tab is pinned. Unpin it to close it.";
    pub const COLUMN_KEYS: &str = "Alt+Shift+Arrows, Alt+Drag";
    /// On a Mac Option+Shift+Left and Right select by word, so the column takes Command too.
    pub const COLUMN_KEYS_MAC: &str = "\u{2325}\u{21e7}\u{2318}Arrows, \u{2325}Drag";
    pub const MANAGE_RECENT_TITLE: &str = "Manage recent files";
    /// One line in the narrowest text area (`ui::PAGE_MIN`, a 520 px window with the Files pane).
    pub const EMPTY_HINT: &str = "Open a file, drag one in, or start a new one.";
    pub const EMPTY_BUTTON: &str = "Open file…";
    pub const EMPTY_NEW: &str = "New file";
    pub const RECENT_EMPTY: &str = "No recent files";
    pub const RECENT_CLEAR_LIST: &str = "Clear list";
    pub const RECENT_DONE: &str = "Done";
    pub const FIND_PLACEHOLDER: &str = "Find in file";
    pub const REPLACE_PLACEHOLDER: &str = "Replace with";
    pub const RESULTS_REPLACE_PLACEHOLDER: &str = "Replace with, in these files";
    pub const RESULTS_FOLDER_PLACEHOLDER: &str = "Folder (the open folder when empty)";
    pub const RESULTS_KINDS_PLACEHOLDER: &str = "File kinds, like *.md;*.txt";
    pub const NO_FOLDER_THERE: &str = "That folder is not there.";
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
    pub const RESULT_STALE: &str = "That document changed; its results are being updated.";
    pub const NO_NEXT_OCCURRENCE: &str = "There is no other place with that text to select.";
    pub const NO_BOOKMARKS: &str = "No bookmarks. Ctrl+F2 sets one on the caret's line.";
    pub const NO_BLOCK: &str = "Nothing to fold here: no lines below are indented more.";
    pub const CRASH_REPORT_COPIED: &str = "The report is on the clipboard.";
    pub const SPLIT_NEEDS_TWO: &str = "Open a second file to show two side by side.";
    pub const PALETTE_PLACEHOLDER: &str =
        "Type a file name, > for commands, : for a line, @ for symbols";
    pub const PALETTE_READING: &str = "Reading the symbols\u{2026}";
    pub const PALETTE_NO_SYMBOLS: &str = "This kind of file has no symbols to list.";
    pub const PALETTE_NO_SYMBOLS_FOUND: &str = "No symbols found in this file.";
    pub const NO_OTHER_PANE: &str = "One pane only. Shift+F8 shows two files side by side.";

    // ---- the notes library ----
    pub const LIBRARY_PLACEHOLDER: &str = "Type the name of a note";
    pub const LIBRARY_ADD_PLACEHOLDER: &str = "Where to keep it";
    pub const LIBRARY_FILES_PLACEHOLDER: &str = "Which library file to edit";
    pub const LIBRARY_MISSING: &str = "missing";
    pub const LIBRARY_END: &str = "at the end";
    pub const LIBRARY_NEW_FILE: &str = "a new library file";
    pub const LIBRARY_EMPTY: &str =
        "The library is empty. \u{201c}Add this file to the notes library\u{201d} starts it.";
    pub const LIBRARY_NO_FOLDER: &str = "There is no folder for the notes library.";
    /// What a run whose links did not go in can do next, at the end of its toast (audit 3 C4,
    /// audit 4 F3).
    pub const LINKS_PENDING_NEXT: &str = "AI runs can finish it or undo it.";
    pub const AI_RESULT_READY: &str = "AI result ready";
    pub const LIBRARY_PANE_NO_FOLDER: &str = "No library folder yet";
    pub const LIBRARY_NEW_GROUP_BUTTON: &str = "New group\u{2026}";
    pub const LIBRARY_ADD_BUTTON: &str = "Add this file";
    pub const LIBRARY_NOT_LISTED: &str = "That file is not in the notes library.";
    pub const LIBRARY_NO_FOLDER_YET: &str = "The library folder is not made yet: nothing is in it.";
    pub const LIBRARY_MOVE_PLACEHOLDER: &str = "Where to move it";
    pub const LIBRARY_NAME_PLACEHOLDER: &str = "Name";
    pub const LIBRARY_NEW_GROUP_PLACEHOLDER: &str = "Name of the group";
    pub const LIBRARY_NEW_SECTION_PLACEHOLDER: &str = "Name of the section";
    pub const LIBRARY_NEW_NOTE_PLACEHOLDER: &str = "Name of the note";
    pub const LIBRARY_REMOVE_TITLE: &str = "Remove the section?";
    pub const LIBRARY_REMOVE: &str = "Remove";
    pub const LIBRARY_DELETE_TITLE: &str = "Delete the library file?";
    pub const LIBRARY_DELETE: &str = "Delete";
    pub fn library_is_a_note(file: &str, group: &str, sections: &[String]) -> String {
        let place = crate::palette::section_path(sections);
        let place = if place.is_empty() {
            group.to_string()
        } else {
            format!("{group} \u{203a} {place}")
        };
        format!("\u{201c}{file}\u{201d} is a note in {place}, not a library file. Remove it there.")
    }
    pub const LIBRARY_NOT_IN_FOLDER: &str =
        "That file is not one of the library folder's own Markdown files: it is left alone.";
    pub fn library_delete_body(group: &str, file: &str, notes: usize) -> String {
        let listed = match notes {
            0 => "It lists no notes.".to_string(),
            1 => "It lists 1 note.".to_string(),
            n => format!("It lists {n} notes."),
        };
        // With nothing listed, saying where the listed notes are would answer a question the
        // person does not have.
        let kept = match notes {
            0 => "",
            _ => " The notes it lists are kept on disk.",
        };
        format!(
            "The library file {file} (the group \u{201c}{group}\u{201d}) will be deleted from the library folder. {listed}{kept}"
        )
    }
    pub fn library_deleted(group: &str) -> String {
        format!("The library file of \u{201c}{group}\u{201d} is deleted. Its notes are untouched.")
    }
    pub fn library_remove_body(section: &str, notes: usize) -> String {
        let links = match notes {
            0 => "It has no notes in it.".to_string(),
            1 => "The 1 note listed in it goes from the library with it.".to_string(),
            n => format!("The {n} notes listed in it go from the library with it."),
        };
        format!(
            "\u{201c}{section}\u{201d} and everything written under it will be taken out of the library file. {links} The notes themselves are not touched."
        )
    }
    pub fn library_took_out(title: &str, group: &str) -> String {
        format!("\u{201c}{title}\u{201d} is out of {group}. The file itself is untouched.")
    }
    pub fn library_moved(title: &str, group: &str) -> String {
        format!("\u{201c}{title}\u{201d} is in {group} now.")
    }
    pub fn library_move_half(title: &str, there: &str, here: &str, why: &str) -> String {
        format!(
            "\u{201c}{title}\u{201d} is in {there} now, but it could not be taken out of {here} ({why}). It is listed in both."
        )
    }
    pub fn library_busy_file(name: &str) -> String {
        format!("Close {name} first: the library file is open in a tab.")
    }
    pub fn library_exists(name: &str) -> String {
        format!("\u{201c}{name}\u{201d} is already there.")
    }
    pub fn library_not_a_name(name: &str) -> String {
        format!("\u{201c}{name}\u{201d} cannot be used as a file name.")
    }
    pub const LIBRARY_EMPTY_PANE: &str = "No notes yet";
    pub const LIBRARY_MISSING_FILE: &str = "That note is not where the library says it is.";
    pub const LIBRARY_NEEDS_FILE: &str = "Save this file first: the library keeps links to files.";
    pub fn library_added(title: &str, group: &str) -> String {
        format!("\u{201c}{title}\u{201d} is in {group} now.")
    }
    /// Where a group's notes go now, said when its `home:` line is written.
    pub fn library_home_now(group: &str, folder: &str) -> String {
        format!("Notes in \u{201c}{group}\u{201d} now go to {folder}.")
    }
    /// The `home:` line is gone. Said plainly: no note was moved.
    pub fn library_home_cleared(group: &str) -> String {
        format!(
            "Notes in \u{201c}{group}\u{201d} go to the library folder again. The notes already written are not moved."
        )
    }
    pub fn library_home_failed(e: &str) -> String {
        format!("The group's folder cannot be made: {e}")
    }
    pub fn library_has(title: &str) -> String {
        format!("\u{201c}{title}\u{201d} is already in the library.")
    }
    pub fn library_failed(e: &str) -> String {
        format!("The library file could not be written: {e}")
    }
    pub fn library_unsaved(name: &str) -> String {
        format!("Save or close {name} first: it has changes that are not saved.")
    }
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
    pub const FILE_GONE: &str = "The file was deleted or moved. Showing the text last read.";
    pub const SESSION_BUSY: &str = "Another mdedit window has the session and did not answer. This window runs without it: its tabs and drafts are not kept.";
    pub fn session_unreadable(e: &std::io::Error) -> String {
        format!(
            "The session file could not be read ({e}). This window runs without it and leaves it as it is: its tabs and drafts are not kept."
        )
    }
    pub fn recovered(n: usize) -> String {
        match n {
            1 => "Recovered 1 unsaved document".to_string(),
            n => format!("Recovered {n} unsaved documents"),
        }
    }
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

    /// "Split into notes…" refuses a document whose bytes are not text (plan 3.11).
    pub fn split_lossy(name: &str) -> String {
        format!("{name} holds bytes that are not text: it cannot be split into notes.")
    }

    /// "Split into notes…" refuses a tab that does not hold what its file holds now: another
    /// program saved the file and the tab has not read it again (audit 4, A F1).
    pub fn split_stale(name: &str) -> String {
        format!(
            "{name} changed on disk since this tab read it. Reload it first: the notes are made \
             from the file on disk."
        )
    }

    /// A group whose folder is the library folder: its notes would be library files.
    pub const LIBRARY_HOME_IS_LIBRARY: &str =
        "The library folder cannot be a group's folder: its notes would be library files.";

    /// The plan's new group, when the library folder already has a file of that name.
    pub fn plan_group_taken(name: &str) -> String {
        format!(
            "\u{201C}{name}\u{201D} is already in the library folder, so this new group cannot be made there. Choose another name, or send the notes to that group."
        )
    }

    pub const PREVIEW_UNSAVED_TOO_LARGE: &str =
        "This document is too large to preview before it is saved.";
    pub const PREVIEW_VIEWER_OLDER: &str =
        "This mdview cannot preview a document that has no file: save it, or update mdview.";
    pub const PREVIEW_TOO_LARGE: &str =
        "This file is too large to preview as you type: mdview shows it when you save.";
    pub const PREVIEW_TILING: &str =
        "Place the windows side by side with your desktop's tiling keys.";
    pub const NO_PLACING: &str = "This desktop does not let a window be placed";

    pub const NOT_MARKDOWN: &str = "This is for Markdown files.";
    pub const NOT_IN_TABLE: &str = "The caret is not in a table.";
    pub const NOT_HERE_IN_TABLE: &str = "That cannot be done at this place in the table.";
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
    pub const NOT_IN_A_DIFFERENCE: &str = "The caret is not in a difference.";
    pub const NO_PARTNER_LINE: &str =
        "This line has no matching line on the other side. Copy the whole change instead.";
    pub const COMPARE_STALE: &str = "Wait until the comparison is up to date.";
    pub const COMPARE_TARGET_READ_ONLY: &str = "The other side cannot be changed.";
    pub const TAKE_TARGET_READ_ONLY: &str = "This side cannot be changed.";

    pub fn saved_copy_name(name: &str) -> String {
        format!("{name} (saved)")
    }

    pub fn compare_unreadable(name: &str, why: &str) -> String {
        format!("Could not read {name} from disk: {why}")
    }

    /// A program missing beside the executable could not be put in place (the updater's repair).
    pub fn repair_failed(error: &str) -> String {
        format!(
            "The AI engine could not be installed ({error}). mdedit tries again at its next start, or now with Check for updates."
        )
    }

    /// An edit to a tab whose unsaved text from the last run could not be read yet.
    pub fn waiting_text_toast(name: &str) -> String {
        format!(
            "The unsaved text of {name} cannot be read now, so the tab is read-only. It comes back at the next start."
        )
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
        // Printed as the platform prints them (on a Mac in its symbols).
        let l = mdshared_ui::keys::label;
        let copy = format!("{}, {}", l("Ctrl+C"), l("Ctrl+Insert"));
        assert!(help.iter().any(|(k, c)| *k == copy && *c == "Copy"));
        let close = format!("{}, {}", l("Ctrl+W"), l("Ctrl+F4"));
        assert!(help.iter().any(|(k, _)| *k == close));
        let column = if mdshared_ui::keys::MAC {
            strings::COLUMN_KEYS_MAC
        } else {
            "Alt+Shift+Arrows, Alt+Drag"
        };
        assert!(
            help.iter()
                .any(|(k, c)| k == column && *c == "Select a column")
        );
        assert_eq!(hint(CommandId::FindNext), "F3");
        assert_eq!(hint(CommandId::About), "");
        for c in COMMANDS {
            assert_eq!(command(c.id).label, c.label);
        }
    }
}
