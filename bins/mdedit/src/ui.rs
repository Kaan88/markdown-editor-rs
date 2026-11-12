//! The window's own state apart from the documents: regions, hits, drags, popups, dialogs, the
//! Files pane and the find box. Drawing code reads it; the event loop in `app.rs` changes it.

use crate::commands::CommandId;
use crate::session::ThemeChoice;
use crate::status::STATUS_H;
use mdshared_ui::gfx::Rect;
use mdshared_ui::menu::Menu;
use mdshared_ui::textfield::TextField;
use mdshared_ui::tree::{FileTree, Rules};
use std::path::PathBuf;
use std::time::Instant;

pub use mdshared_ui::chrome::{BAR_H, TITLE_H};
pub const PANE_MIN: f32 = 160.0;
pub const PANE_MAX: f32 = 480.0;
pub const PANE_DEFAULT: f32 = 250.0;
/// Below this the text would be too narrow; the Files pane gives way first.
pub const PAGE_MIN: f32 = 360.0;

/// What can scroll with a scrollbar.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Scroller {
    /// The text, vertically.
    Text,
    /// The text, horizontally.
    TextH,
    Files,
    Dialog,
}

/// The command bar's buttons.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum BarItem {
    Open,
    Reload,
    Recent,
    Files,
    FindPrev,
    FindNext,
    Theme,
    More,
}

/// The screen areas that are redrawn independently.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Region {
    Title,
    Bar,
    Files,
    Text,
    Status,
}

pub const REGIONS: [Region; 5] = [
    Region::Title,
    Region::Bar,
    Region::Files,
    Region::Text,
    Region::Status,
];

impl Region {
    pub fn index(self) -> usize {
        self as usize
    }
}

/// Something the pointer can be over.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Hit {
    Tab(usize),
    TabClose(usize),
    NewTab,
    TitleDrag,
    WinMin,
    WinMax,
    WinClose,
    Bar(BarItem),
    Find,
    FindClear,
    FilesRow(usize),
    FilesFolder,
    FilesOpenFolder,
    Splitter,
    ScrollBar(Scroller),
    /// The text area (gutter included).
    Text,
    EmptyOpen,
    EmptyRecent(usize),
    DlgButton(usize),
    DlgOpen(usize),
    DlgRemove(usize),
    DlgCard,
    Backdrop,
}

impl Hit {
    /// The region whose look depends on this hit being hovered or pressed.
    pub fn region(self) -> Option<Region> {
        Some(match self {
            Hit::Tab(_) | Hit::TabClose(_) | Hit::NewTab | Hit::TitleDrag => Region::Title,
            Hit::WinMin | Hit::WinMax | Hit::WinClose => Region::Title,
            Hit::Bar(_) | Hit::Find | Hit::FindClear => Region::Bar,
            Hit::FilesRow(_)
            | Hit::FilesFolder
            | Hit::FilesOpenFolder
            | Hit::Splitter
            | Hit::ScrollBar(Scroller::Files) => Region::Files,
            Hit::ScrollBar(Scroller::Text | Scroller::TextH)
            | Hit::Text
            | Hit::EmptyOpen
            | Hit::EmptyRecent(_) => Region::Text,
            _ => return None,
        })
    }

    /// Hits that act on release over the same hit, and show a pressed state meanwhile.
    pub fn is_button(self) -> bool {
        !matches!(
            self,
            Hit::TitleDrag
                | Hit::Text
                | Hit::Splitter
                | Hit::ScrollBar(_)
                | Hit::Find
                | Hit::Tab(_)
                | Hit::DlgCard
        )
    }
}

/// What a popup menu item does.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Cmd(CommandId),
    /// A tab context menu command for this tab.
    TabCmd(CommandId, usize),
    Recent(PathBuf),
    /// Replace the open menu by the Recent menu.
    RecentMenu,
    Theme(ThemeChoice),
    /// An editing command for the find box.
    Edit(EditAct),
    MatchCase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditAct {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PopupKind {
    Main,
    Recent,
    Tab(usize),
    /// The text area's right-click menu.
    Text,
    /// The find box's editing menu.
    Find,
}

pub struct Popup {
    pub kind: PopupKind,
    pub menu: Menu<Act>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DialogKind {
    About,
    Shortcuts,
    Recent,
}

pub struct Dialog {
    pub kind: DialogKind,
    /// Scroll of a long list, logical px.
    pub scroll: f32,
    /// The scrollable list's height and its content's, set when drawn.
    pub view: f32,
    pub content: f32,
    pub list: Rect,
}

impl Dialog {
    pub fn new(kind: DialogKind) -> Dialog {
        Dialog {
            kind,
            scroll: 0.0,
            view: 0.0,
            content: 0.0,
            list: Rect::default(),
        }
    }
}

/// How a text selection drag grows: by characters, words (after a double click) or lines (after
/// a triple click), from the unit first pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectUnit {
    Grapheme,
    Word,
    Line,
}

/// A press being held: what it started on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drag {
    /// A tab pressed; it becomes a reorder once the pointer moves a few px.
    Tab {
        index: usize,
        x: f32,
        moving: bool,
    },
    Splitter {
        x: f32,
        width: f32,
    },
    /// A scrollbar thumb, grabbed `grab` logical px from its start.
    Thumb {
        target: Scroller,
        grab: f32,
    },
    /// Selecting text: the unit and the range first pressed.
    Select {
        unit: SelectUnit,
        origin: (usize, usize),
    },
    /// Selecting in the find box.
    FindSelect,
}

/// A tooltip waiting to show, or showing.
#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub hit: Hit,
    pub text: String,
    pub anchor: Rect,
    pub due: Instant,
    pub shown: bool,
}

pub struct Panes {
    pub files: bool,
    pub files_w: f32,
    pub files_scroll: f32,
    /// Content and view heights, set when drawn, for wheel clamping.
    pub files_content: f32,
    pub files_view: f32,
}

impl Panes {
    /// Scrolls the Files pane by `dy`, clamped. True if it moved.
    pub fn scroll_by(&mut self, dy: f32) -> bool {
        let next = (self.files_scroll + dy)
            .clamp(0.0, (self.files_content - self.files_view).max(0.0))
            .round();
        let moved = next != self.files_scroll;
        self.files_scroll = next;
        moved
    }
}

/// The Files pane lists every file.
pub fn new_tree() -> FileTree {
    FileTree::new(Rules {
        shows_file: Box::new(|_| true),
    })
}

pub struct Ui {
    pub panes: Panes,
    pub tree: FileTree,
    pub find: TextField,
    pub find_focused: bool,
    pub match_case: bool,
    pub caret_on: bool,
    /// When the caret next blinks.
    pub caret_due: Option<Instant>,
    /// Logical pointer position.
    pub mouse: (f32, f32),
    pub hover: Option<Hit>,
    pub pressed: Option<Hit>,
    pub drag: Option<Drag>,
    pub popup: Option<Popup>,
    pub dialog: Option<Dialog>,
    pub tip: Option<Tip>,
    /// A tooltip was dismissed by a press; it stays hidden until the pointer leaves that hit.
    pub tip_blocked: Option<Hit>,
    /// Hover row in the Manage recent dialog.
    pub dlg_hover_row: Option<usize>,
    /// The last left press, for double and triple clicks: when, where, and the count.
    pub last_press: Option<(Instant, (f32, f32), u32)>,
    /// When the next auto-scroll step of a selection drag is due.
    pub autoscroll_due: Option<Instant>,
}

impl Ui {
    pub fn new() -> Ui {
        Ui {
            panes: Panes {
                files: false,
                files_w: PANE_DEFAULT,
                files_scroll: 0.0,
                files_content: 0.0,
                files_view: 0.0,
            },
            tree: new_tree(),
            find: TextField::new(),
            find_focused: false,
            match_case: false,
            caret_on: true,
            caret_due: None,
            mouse: (-1.0, -1.0),
            hover: None,
            pressed: None,
            drag: None,
            popup: None,
            dialog: None,
            tip: None,
            tip_blocked: None,
            dlg_hover_row: None,
            last_press: None,
            autoscroll_due: None,
        }
    }

    pub fn popup_kind(&self) -> Option<PopupKind> {
        self.popup.as_ref().map(|p| p.kind)
    }
}

/// Where everything goes this frame, in logical px.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Areas {
    pub w: f32,
    pub h: f32,
    pub title: Rect,
    pub bar: Rect,
    pub files: Option<Rect>,
    /// The text region: gutter, text and scrollbars, or the empty state.
    pub text: Rect,
    pub status: Rect,
}

impl Areas {
    pub fn region(&self, r: Region) -> Option<Rect> {
        match r {
            Region::Title => Some(self.title),
            Region::Bar => Some(self.bar),
            Region::Files => self.files,
            Region::Text => Some(self.text),
            Region::Status => Some(self.status),
        }
    }

    /// Splits a `w` x `h` window. The status strip runs along the whole bottom; the Files pane
    /// shrinks, then hides, so the text keeps `PAGE_MIN`.
    pub fn compute(w: f32, h: f32, files_on: bool, files_w: f32) -> Areas {
        let title = Rect::new(0.0, 0.0, w, TITLE_H);
        let bar = Rect::new(0.0, TITLE_H, w, BAR_H);
        let top = TITLE_H + BAR_H;
        let status_y = (h - STATUS_H).max(top);
        let status = Rect::new(0.0, status_y, w, (h - status_y).max(0.0));
        let body_h = (status_y - top).max(0.0);
        let mut fw = if files_on { files_w } else { 0.0 };
        if fw + PAGE_MIN > w && fw > 0.0 {
            fw = (w - PAGE_MIN).max(PANE_MIN);
        }
        if fw + PAGE_MIN > w {
            fw = 0.0;
        }
        let files = (fw > 0.0).then(|| Rect::new(0.0, top, fw, body_h));
        let text = Rect::new(fw, top, (w - fw).max(0.0), body_h);
        Areas {
            w,
            h,
            title,
            bar,
            files,
            text,
            status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panes_give_way_to_the_text() {
        let a = Areas::compute(1280.0, 800.0, true, 250.0);
        assert_eq!(a.files.unwrap().w, 250.0);
        assert_eq!(a.text.x, 250.0);
        assert_eq!(a.text.w, 1030.0);
        assert_eq!(a.text.y, TITLE_H + BAR_H);
        // The Files pane shrinks to keep the text at its minimum, then hides.
        let a = Areas::compute(560.0, 600.0, true, 250.0);
        assert_eq!(a.files.unwrap().w, 200.0);
        assert_eq!(a.text.w, PAGE_MIN);
        let a = Areas::compute(500.0, 600.0, true, 250.0);
        assert!(a.files.is_none());
        assert_eq!(a.text.w, 500.0);
        let a = Areas::compute(1280.0, 800.0, false, 250.0);
        assert!(a.files.is_none());
        assert_eq!(a.text.x, 0.0);
    }

    #[test]
    fn status_strip_runs_along_the_bottom() {
        for files in [false, true] {
            let a = Areas::compute(1280.0, 800.0, files, 300.0);
            assert_eq!(a.status, Rect::new(0.0, 800.0 - STATUS_H, 1280.0, STATUS_H));
            assert_eq!(a.text.bottom(), a.status.y);
            if let Some(f) = a.files {
                assert_eq!(f.bottom(), a.status.y);
            }
        }
        // A window shorter than the chrome keeps every rectangle non-negative.
        let a = Areas::compute(520.0, 60.0, true, 250.0);
        assert!(a.text.h >= 0.0 && a.status.h >= 0.0);
    }

    #[test]
    fn hits_belong_to_regions() {
        assert_eq!(Hit::Tab(0).region(), Some(Region::Title));
        assert_eq!(Hit::Find.region(), Some(Region::Bar));
        assert_eq!(Hit::Splitter.region(), Some(Region::Files));
        assert_eq!(Hit::ScrollBar(Scroller::TextH).region(), Some(Region::Text));
        assert_eq!(Hit::Backdrop.region(), None);
        assert!(Hit::Bar(BarItem::Open).is_button());
        assert!(!Hit::Text.is_button());
    }
}
