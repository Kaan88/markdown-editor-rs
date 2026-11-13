//! The window's own state apart from the documents: regions, hits, drags, popups, dialogs, the
//! Files pane and the find box. Drawing code reads it; the event loop in `app.rs` changes it.

use crate::commands::CommandId;
use crate::session::ThemeChoice;
use crate::status::STATUS_H;
use mdedit_core::find::FindOptions;
use mdedit_highlight::DocId;
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
/// The height of the replace strip under the command bar.
pub const REPLACE_H: f32 = 40.0;

/// The options of a search, as the strip's toggles and the find box's menu name them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FindOpt {
    Case,
    Word,
    Regex,
}

/// The two text boxes of the bar: the query and what replaces it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum BarField {
    Find,
    Replace,
}

impl BarField {
    pub fn hit(self) -> Hit {
        match self {
            BarField::Find => Hit::Find,
            BarField::Replace => Hit::Replace,
        }
    }
}

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
    /// The replace strip: its text box, its buttons and the search options.
    Replace,
    ReplaceOne,
    ReplaceAll,
    ReplaceClose,
    FindOpt(FindOpt),
    /// The update bar's button and its close button.
    UpdateGo,
    UpdateClose,
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
            Hit::Bar(_) | Hit::Find | Hit::FindClear | Hit::UpdateGo | Hit::UpdateClose => {
                Region::Bar
            }
            Hit::Replace
            | Hit::ReplaceOne
            | Hit::ReplaceAll
            | Hit::ReplaceClose
            | Hit::FindOpt(_) => Region::Bar,
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
                | Hit::Replace
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
    /// An editing command for the focused box of the bar.
    Edit(EditAct),
    /// Turns a search option on or off.
    Opt(FindOpt),
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
    /// A tab with unsaved changes is about to close: Save, Don't save, Cancel.
    Unsaved,
    /// A save found the file changed on disk: Overwrite, Cancel.
    Overwrite,
    /// A reload would drop unsaved changes: Reload, Cancel.
    Discard,
}

impl DialogKind {
    /// A question whose answer something waits for (`Ui::pending`).
    pub fn is_confirm(self) -> bool {
        matches!(
            self,
            DialogKind::Unsaved | DialogKind::Overwrite | DialogKind::Discard
        )
    }
}

/// What goes on once the unsaved changes in its way are saved or given up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
    Nothing,
    CloseTab(DocId),
    CloseOthers(DocId),
    CloseAll,
    Exit,
    Reload(DocId),
}

/// The question on screen: the document it is about and what waits for the answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending {
    pub doc: DocId,
    pub after: After,
}

pub struct Dialog {
    pub kind: DialogKind,
    /// Scroll of a long list, logical px.
    pub scroll: f32,
    /// The scrollable list's height and its content's, set when drawn.
    pub view: f32,
    pub content: f32,
    pub list: Rect,
    /// The file a question is about.
    pub subject: String,
}

impl Dialog {
    pub fn new(kind: DialogKind) -> Dialog {
        Dialog {
            kind,
            scroll: 0.0,
            view: 0.0,
            content: 0.0,
            list: Rect::default(),
            subject: String::new(),
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
    /// Selecting in the focused box of the bar.
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
    /// A box of the bar has the keyboard; `field` says which.
    pub find_focused: bool,
    pub field: BarField,
    pub match_case: bool,
    pub whole_word: bool,
    pub regex: bool,
    /// The replacement text, and whether its strip shows.
    pub replace: TextField,
    pub replace_open: bool,
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
    /// What the question dialog on screen is about.
    pub pending: Option<Pending>,
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
            field: BarField::Find,
            match_case: false,
            whole_word: false,
            regex: false,
            replace: TextField::new(),
            replace_open: false,
            caret_on: true,
            caret_due: None,
            mouse: (-1.0, -1.0),
            hover: None,
            pressed: None,
            drag: None,
            popup: None,
            dialog: None,
            pending: None,
            tip: None,
            tip_blocked: None,
            dlg_hover_row: None,
            last_press: None,
            autoscroll_due: None,
        }
    }

    /// The box of the bar the keyboard goes to when one is focused.
    pub fn field(&self) -> &TextField {
        match self.field {
            BarField::Find => &self.find,
            BarField::Replace => &self.replace,
        }
    }

    pub fn field_mut(&mut self) -> &mut TextField {
        match self.field {
            BarField::Find => &mut self.find,
            BarField::Replace => &mut self.replace,
        }
    }

    /// Gives one of the bar's boxes the keyboard.
    pub fn focus_field(&mut self, field: BarField) {
        self.field = field;
        self.find_focused = true;
    }

    /// Hides the replace strip; the keyboard goes back to the text if its box had it.
    pub fn close_replace(&mut self) {
        self.replace_open = false;
        if self.field == BarField::Replace {
            self.field = BarField::Find;
            self.find_focused = false;
        }
    }

    pub fn options(&self) -> FindOptions {
        FindOptions {
            match_case: self.match_case,
            whole_word: self.whole_word,
            regex: self.regex,
        }
    }

    pub fn option(&self, opt: FindOpt) -> bool {
        match opt {
            FindOpt::Case => self.match_case,
            FindOpt::Word => self.whole_word,
            FindOpt::Regex => self.regex,
        }
    }

    pub fn toggle_option(&mut self, opt: FindOpt) {
        let flag = match opt {
            FindOpt::Case => &mut self.match_case,
            FindOpt::Word => &mut self.whole_word,
            FindOpt::Regex => &mut self.regex,
        };
        *flag = !*flag;
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
    /// shrinks, then hides, so the text keeps `PAGE_MIN`. `notice` is the height of a bar shown
    /// under the command bar (the update bar), which belongs to the bar's region.
    pub fn compute(w: f32, h: f32, files_on: bool, files_w: f32, notice: f32) -> Areas {
        let title = Rect::new(0.0, 0.0, w, TITLE_H);
        let bar = Rect::new(0.0, TITLE_H, w, BAR_H + notice);
        let top = TITLE_H + BAR_H + notice;
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
        let a = Areas::compute(1280.0, 800.0, true, 250.0, 0.0);
        assert_eq!(a.files.unwrap().w, 250.0);
        assert_eq!(a.text.x, 250.0);
        assert_eq!(a.text.w, 1030.0);
        assert_eq!(a.text.y, TITLE_H + BAR_H);
        // A bar under the command bar pushes the body down and is part of the bar's region.
        let b = Areas::compute(1280.0, 800.0, true, 250.0, 34.0);
        assert_eq!(b.bar.h, BAR_H + 34.0);
        assert_eq!(b.text.y, a.text.y + 34.0);
        assert_eq!(b.text.h, a.text.h - 34.0);
        // The Files pane shrinks to keep the text at its minimum, then hides.
        let a = Areas::compute(560.0, 600.0, true, 250.0, 0.0);
        assert_eq!(a.files.unwrap().w, 200.0);
        assert_eq!(a.text.w, PAGE_MIN);
        let a = Areas::compute(500.0, 600.0, true, 250.0, 0.0);
        assert!(a.files.is_none());
        assert_eq!(a.text.w, 500.0);
        let a = Areas::compute(1280.0, 800.0, false, 250.0, 0.0);
        assert!(a.files.is_none());
        assert_eq!(a.text.x, 0.0);
    }

    #[test]
    fn status_strip_runs_along_the_bottom() {
        for files in [false, true] {
            let a = Areas::compute(1280.0, 800.0, files, 300.0, 0.0);
            assert_eq!(a.status, Rect::new(0.0, 800.0 - STATUS_H, 1280.0, STATUS_H));
            assert_eq!(a.text.bottom(), a.status.y);
            if let Some(f) = a.files {
                assert_eq!(f.bottom(), a.status.y);
            }
        }
        // A window shorter than the chrome keeps every rectangle non-negative.
        let a = Areas::compute(520.0, 60.0, true, 250.0, 0.0);
        assert!(a.text.h >= 0.0 && a.status.h >= 0.0);
    }

    #[test]
    fn the_bar_has_two_boxes_and_three_options() {
        let mut ui = Ui::new();
        ui.find.set_text("a");
        ui.replace.set_text("b");
        assert_eq!(ui.field().text(), "a");
        ui.replace_open = true;
        ui.focus_field(BarField::Replace);
        assert_eq!(ui.field().text(), "b");
        assert_eq!(ui.field.hit(), Hit::Replace);
        // Closing the strip under the focused box gives the keyboard back to the text.
        ui.close_replace();
        assert!(!ui.find_focused && !ui.replace_open && ui.field == BarField::Find);
        // Closing it while the find box is focused leaves that focus alone.
        ui.replace_open = true;
        ui.focus_field(BarField::Find);
        ui.close_replace();
        assert!(ui.find_focused);
        ui.toggle_option(FindOpt::Word);
        ui.toggle_option(FindOpt::Regex);
        let o = ui.options();
        assert!(!o.match_case && o.whole_word && o.regex);
        assert!(ui.option(FindOpt::Word) && !ui.option(FindOpt::Case));
    }

    #[test]
    fn hits_belong_to_regions() {
        assert_eq!(Hit::Tab(0).region(), Some(Region::Title));
        assert_eq!(Hit::Find.region(), Some(Region::Bar));
        assert_eq!(Hit::Replace.region(), Some(Region::Bar));
        assert_eq!(Hit::FindOpt(FindOpt::Regex).region(), Some(Region::Bar));
        assert!(Hit::ReplaceAll.is_button() && !Hit::Replace.is_button());
        assert_eq!(Hit::Splitter.region(), Some(Region::Files));
        assert_eq!(Hit::ScrollBar(Scroller::TextH).region(), Some(Region::Text));
        assert_eq!(Hit::Backdrop.region(), None);
        assert!(Hit::Bar(BarItem::Open).is_button());
        assert!(!Hit::Text.is_button());
    }
}
