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
    Find,
    FindNext,
    FindPrevious,
    ClearSearch,
    ToggleFiles,
    NextTab,
    PreviousTab,
    Escape,
    Theme,
    ThemeSystem,
    ThemeLight,
    ThemeDark,
    ShowShortcuts,
    About,
    CloseOtherTabs,
    CopyFilePath,
    ShowInFileManager,
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
    cmd(C::Find, "Find", &["Ctrl+F"]),
    cmd(C::FindNext, "Find next", &["F3"]),
    cmd(C::FindPrevious, "Find previous", &["Shift+F3"]),
    cmd(C::ClearSearch, "Clear search", &[]),
    cmd(C::ToggleFiles, "Files pane", &["Ctrl+Shift+1"]),
    cmd(C::NextTab, "Next tab", &["Ctrl+Tab"]),
    cmd(C::PreviousTab, "Previous tab", &["Ctrl+Shift+Tab"]),
    cmd(C::Escape, "Close a menu or leave the find box", &["Escape"]),
    cmd(C::Theme, "Theme", &[]),
    cmd(C::ThemeSystem, "System theme", &[]),
    cmd(C::ThemeLight, "Light theme", &[]),
    cmd(C::ThemeDark, "Dark theme", &[]),
    cmd(C::ShowShortcuts, "Keyboard shortcuts", &[]),
    cmd(C::About, "About mdedit", &[]),
    cmd(C::CloseOtherTabs, "Close other tabs", &[]),
    cmd(C::CopyFilePath, "Copy file path", &[]),
    cmd(C::ShowInFileManager, SHOW_IN_FILE_MANAGER, &[]),
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
    COMMANDS
        .iter()
        .filter(|c| !c.keys.is_empty())
        .map(|c| (c.keys.join(", "), c.label))
        .collect()
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
    pub const SHORTCUTS_TITLE: &str = "Keyboard shortcuts";
    pub const MANAGE_RECENT_TITLE: &str = "Manage recent files";
    pub const EMPTY_HINT: &str = "Open a file or drag one into this window.";
    pub const EMPTY_BUTTON: &str = "Open file…";
    pub const RECENT_EMPTY: &str = "No recent files";
    pub const RECENT_CLEAR_LIST: &str = "Clear list";
    pub const RECENT_DONE: &str = "Done";
    pub const FIND_PLACEHOLDER: &str = "Find in file";
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

    /// The toast for a file read with replacement characters.
    pub fn lossy_toast(name: &str) -> String {
        format!("Some bytes in {name} could not be read and are shown as \u{FFFD}.")
    }

    pub fn read_only_toast(name: &str) -> String {
        format!("{name} has bytes that could not be read, so it is open read-only.")
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
        assert_eq!(hint(CommandId::FindNext), "F3");
        assert_eq!(hint(CommandId::About), "");
        for c in COMMANDS {
            assert_eq!(command(c.id).label, c.label);
        }
    }
}
