//! The AI task window over files (`docs/ai-library-plan.md` 5.6, phase W): its state, the
//! numbers of its layout and what a file is to a task. `task_draw.rs` draws it and
//! `app/task.rs` is the window's part. Nothing here reads the disk: a file is probed by
//! `App::task_add` and the result is held in a [`FileEntry`].
//!
//! The window takes files, not the open document, and nothing it is given is opened in a tab.
//! Three kinds of file (text, picture, other) are decided when one is added and shown as three
//! groups, each saying what happens to it for the task chosen.

use mdedit_core::ai::PromptKind;
use mdedit_core::split::answer;
use mdshared_ui::gfx::Rect;
use mdshared_ui::scroll::scrollbar;
use mdshared_ui::textfield::TextField;
use std::path::{Path, PathBuf};

// ---- the numbers of the layout ----------------------------------------------------------------------

pub const CARD_W: f32 = 880.0;
pub const CARD_H: f32 = 704.0;
pub const HEADER_H: f32 = 64.0;
pub const TASK_H: f32 = 36.0;
pub const FIELD_H: f32 = 36.0;
pub const PROMPTS_H: f32 = 96.0;
pub const DEST_H: f32 = 40.0;
pub const PROMPT_ROW: f32 = 44.0;
pub const GROUP_H: f32 = 26.0;
pub const ROW_H: f32 = 34.0;
/// A picture's row on Write notes: its thumbnail beside the name (pictures plan 5).
pub const PICTURE_ROW_H: f32 = 52.0;
/// The thumbnail's square on that row.
pub const THUMB: f32 = 40.0;

/// The card for a `w` x `h` window.
pub fn card(w: f32, h: f32) -> Rect {
    mdshared_ui::overlay::dialog_card(CARD_W, CARD_H, w, h)
}

/// The height of a line of the Add menu.
pub const ADD_ROW_H: f32 = 30.0;

/// How many lines the Add menu has room for inside the card, under its button, in a window
/// `w` by `h`: at least one, for "Files…". The tabs beyond it are left out of the menu, so
/// "Files…", its last line, can always be clicked.
pub fn add_menu_lines(w: f32, h: f32) -> usize {
    let r = card(w, h);
    // The menu starts 4 px under the button (19 px down the card, 30 px high) and keeps 8 px
    // of the card under it; it has 4 px inside at the top and the bottom.
    let top = r.y + 19.0 + 30.0 + 4.0;
    let room = r.bottom() - 8.0 - top - 8.0;
    ((room / ADD_ROW_H).floor().max(1.0)) as usize
}

/// Where the parts of the card go.
#[derive(Clone, Copy, Debug)]
pub struct Geo {
    pub header: Rect,
    pub task: Rect,
    pub field: Rect,
    pub prompts: Rect,
    pub dest: Rect,
    pub list: Rect,
    pub footer: Rect,
}

pub fn geo(card: Rect) -> Geo {
    let pad = mdshared_ui::overlay::DIALOG_PAD;
    let x = card.x + pad;
    let w = card.w - 2.0 * pad;
    let footer = Rect::new(
        card.x,
        card.bottom() - mdshared_ui::overlay::DIALOG_FOOTER,
        card.w,
        mdshared_ui::overlay::DIALOG_FOOTER,
    );
    let header = Rect::new(card.x, card.y, card.w, HEADER_H);
    let task = Rect::new(x, header.bottom() + 8.0, w, TASK_H);
    let field = Rect::new(x, task.bottom() + 12.0, w, FIELD_H);
    let prompts = Rect::new(x, field.bottom() + 8.0, w, PROMPTS_H);
    let dest = Rect::new(x, prompts.bottom() + 8.0, w, DEST_H);
    let list = Rect::new(
        card.x + 1.0,
        dest.bottom() + 8.0,
        card.w - 2.0,
        (footer.y - dest.bottom() - 14.0).max(0.0),
    );
    Geo {
        header,
        task,
        field,
        prompts,
        dest,
        list,
        footer,
    }
}

/// The WHERE IT GOES row of File them and Split them: the group's button, File them's section
/// field, and the room left for the folder the files go to. Each part is cut to the row, so
/// nothing overlaps at any width the window allows (the owner's report, 2026-11-24: the folder
/// was drawn over the section field). `folder` is `None` when less than [`FOLDER_MIN`] is left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DestRow {
    pub button: Rect,
    pub section: Option<Rect>,
    pub folder: Option<Rect>,
}

/// The label's column, left of the button.
pub const DEST_LABEL_W: f32 = 100.0;
/// Below this the folder is left out of the row: the button still names the group.
pub const FOLDER_MIN: f32 = 72.0;

/// The row for a card whose inner part starts at `x` and is `inner` wide, its top at `y`.
pub fn dest_row(x: f32, inner: f32, y: f32, with_section: bool) -> DestRow {
    const GAP: f32 = 12.0;
    let right = x + inner;
    let start = x + DEST_LABEL_W;
    let room = (right - start).max(0.0);
    let (button_w, section_w) = if with_section {
        // Each as wide as it was (240 and 180) while the row has room; narrower in proportion
        // when it does not.
        let both = (room - GAP).max(0.0);
        let scale = (both / 420.0).min(1.0);
        ((240.0 * scale).floor(), (180.0 * scale).floor())
    } else {
        (240.0_f32.min(room), 0.0)
    };
    let button = Rect::new(start, y + 2.0, button_w, 30.0);
    let section = with_section.then(|| Rect::new(button.right() + GAP, y + 2.0, section_w, 30.0));
    let taken = section.map_or(button.right(), |s| s.right());
    let left = taken + GAP;
    let folder = (right - left >= FOLDER_MIN).then(|| Rect::new(left, y, right - left, DEST_H));
    DestRow {
        button,
        section,
        folder,
    }
}

// ---- what a task is, and what a file is -------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TaskKind {
    /// Write notes (pictures plan): notes from pictures and text files.
    Notes,
    File,
    Split,
    Rewrite,
}

impl TaskKind {
    pub const ALL: [TaskKind; 4] = [
        TaskKind::Notes,
        TaskKind::File,
        TaskKind::Split,
        TaskKind::Rewrite,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TaskKind::Notes => "Write notes",
            TaskKind::File => "File them",
            TaskKind::Split => "Split them",
            TaskKind::Rewrite => "Rewrite them",
        }
    }

    /// The saved prompts this task lists.
    pub fn prompt_kind(self) -> PromptKind {
        match self {
            TaskKind::Notes => PromptKind::Notes,
            TaskKind::File => PromptKind::File,
            TaskKind::Split => PromptKind::Split,
            TaskKind::Rewrite => PromptKind::Rewrite,
        }
    }

    /// The prompts of this task's kind, from the four lists the window keeps.
    pub fn prompts_of(
        self,
        notes: &[mdedit_core::ai::SavedPrompt],
        file: &[mdedit_core::ai::SavedPrompt],
        split: &[mdedit_core::ai::SavedPrompt],
        rewrite: &[mdedit_core::ai::SavedPrompt],
    ) -> Vec<mdedit_core::ai::SavedPrompt> {
        match self.prompt_kind() {
            PromptKind::Notes => notes.to_vec(),
            PromptKind::Split => split.to_vec(),
            PromptKind::File => file.to_vec(),
            _ => rewrite.to_vec(),
        }
    }

    pub fn placeholder(self) -> &'static str {
        match self {
            TaskKind::Notes => {
                "What should the notes be? For example: one note per room, with what to buy"
            }
            TaskKind::File => "How should they be filed? For example: by dish and by occasion",
            TaskKind::Split => "How should each be cut? For example: one note per chapter",
            TaskKind::Rewrite => "What should change? For example: make the prices a table",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileKind {
    Text,
    Picture,
    Other,
}

impl FileKind {
    /// What the group heading says of this kind of file for `task`.
    pub fn heading(self, task: TaskKind) -> &'static str {
        match (self, task) {
            (FileKind::Text, TaskKind::Notes) => {
                "TEXT \u{00B7} READ AND SENT WHOLE, WITH WHAT THE PICTURES SHOW"
            }
            (FileKind::Picture, TaskKind::Notes) => crate::notes_task::pictures_heading(true, true),
            (FileKind::Text, TaskKind::File) => "TEXT \u{00B7} READ AND SENT: A DIGEST GOES",
            (FileKind::Text, _) => "TEXT \u{00B7} READ AND SENT",
            (FileKind::Picture, TaskKind::File) => {
                "PICTURES \u{00B7} NOT FILED: A PICTURE IS NOT A NOTE"
            }
            (FileKind::Picture, _) => "PICTURES \u{00B7} NOT SENT FOR THIS TASK",
            (FileKind::Other, TaskKind::File) => "OTHER \u{00B7} LINKED BY NAME, NEVER READ",
            (FileKind::Other, _) => "OTHER \u{00B7} LEFT OUT, WITH THE REASON ON THE ROW",
        }
    }
}

/// One file the window holds: what it is, what can be done with it for the task, and what the
/// model said about it.
#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub kind: FileKind,
    /// Why the file is left out of the task's requests, on the row in the dim colour. `None`
    /// when it takes part.
    pub why: Option<String>,
    pub ticked: bool,
    /// The text, when it is UTF-8 text within [`Limits::rewrite_bytes`]: a rewrite sends it and its
    /// review edits it. `None` for a larger file or another encoding.
    pub text: Option<String>,
    pub bom: bool,
    /// The first 1,200 bytes of the text, for a digest; empty for a picture or a file not read.
    pub sample: String,
    /// The first `# ` heading of the text, without its marker.
    pub heading: Option<String>,
    /// What the model filed it as, and where.
    pub title: Option<String>,
    pub sections: Vec<String>,
    /// A picture or other file the model placed with a text file (by its place in the window).
    pub with: Option<usize>,
    /// A Rewrite run takes 20 files: while twenty are ticked, the others say so on their row.
    pub over_limit: bool,
    /// The limit unticked it, not the owner: another task ticks it again.
    pub limited: bool,
    /// The file's dominant line ending, as a rewrite writes its result back with it.
    pub eol: String,
    /// Its date of last change, `YYYY-MM-DD`, in the digest.
    pub modified: Option<String>,
    /// The library already lists it: File them shows it, never ticked, never sent; the other
    /// tasks take it like any file (a note of the library is what they work on, pictures plan
    /// 4.2).
    pub already: bool,
    /// The run wrote it (a split's plan was written, a rewrite went in).
    pub done: bool,
    /// The window's limits, as its reason says them (D-203).
    pub limits: Limits,
    /// Write notes: what the engine said the picture is sent as.
    pub measure: crate::notes_task::Measure,
    /// Its thumbnail, as the engine made it, decoded and made small (RGBA, width, height).
    pub thumb: Option<(Vec<u8>, u32, u32)>,
}

impl FileEntry {
    /// Why this file takes no part in `task`, on top of what `task_add` found.
    pub fn reason_for(&self, task: TaskKind) -> Option<String> {
        match task {
            TaskKind::Notes => crate::notes_task::reason(self, &self.measure),
            // Filed already: "Move the notes…" is the way to put it elsewhere.
            TaskKind::File if self.already => Some("already in the library".to_string()),
            // A PDF, a Word file, an archive: never read, but its name is a link like any other.
            // A picture is never a library entry of its own (D-204): not filed, not sent.
            TaskKind::File => match self.kind {
                FileKind::Picture => Some(PICTURE_NOT_FILED.to_string()),
                _ => None,
            },
            TaskKind::Split => match self.kind {
                FileKind::Text => None,
                FileKind::Picture => Some("a picture: not sent for this task".to_string()),
                FileKind::Other => self.why.clone(),
            },
            TaskKind::Rewrite => match self.kind {
                FileKind::Text => {
                    if self.over_limit {
                        return Some(format!("only {} files a run", self.limits.rewrite_files));
                    }
                    if self.text.is_none() {
                        if self.size as usize > self.limits.rewrite_bytes {
                            Some(format!(
                                "too long for one request: {} KB of {} KB",
                                self.size.div_ceil(1000),
                                self.limits.rewrite_bytes / 1000
                            ))
                        } else {
                            Some("not UTF-8: save it as UTF-8 first".to_string())
                        }
                    } else {
                        None
                    }
                }
                FileKind::Picture => Some("a picture: not sent for this task".to_string()),
                FileKind::Other => self.why.clone(),
            },
        }
    }

    /// Whether it is ticked and takes part.
    pub fn takes_part(&self, task: TaskKind) -> bool {
        self.ticked && self.reason_for(task).is_none()
    }

    /// Whether it could take a place of a rewrite's twenty: a text file with nothing else
    /// keeping it out.
    fn can_be_rewritten(&self) -> bool {
        self.kind == FileKind::Text && self.text.is_some() && self.why.is_none() && !self.done
    }
}

/// What a run of "Copy them in" did, for its toast: "2 copied with 3 pictures, 1 linked where it
/// is".
pub fn copied_text(
    plan: &[mdedit_core::moves::copy_in::Planned],
    written: &mdedit_core::moves::copy_in::Written,
) -> String {
    use mdedit_core::moves::copy_in::Copy;
    let new = plan
        .iter()
        .filter(|p| matches!(p.copy, Copy::New { .. }))
        .count();
    let there = plan
        .iter()
        .filter(|p| matches!(p.copy, Copy::There { .. }))
        .count();
    let stays = plan
        .iter()
        .filter(|p| matches!(p.copy, Copy::Stays { .. }))
        .count();
    let pictures = written.pictures.iter().filter(|p| p.created).count();
    let mut parts: Vec<String> = Vec::new();
    if new > 0 {
        parts.push(match pictures {
            0 => format!("{new} copied in"),
            1 => format!("{new} copied in with 1 picture"),
            n => format!("{new} copied in with {n} pictures"),
        });
    }
    if there > 0 {
        parts.push(format!("{there} already there"));
    }
    if stays > 0 {
        parts.push(format!("{stays} linked where it is"));
    }
    parts.join(", ")
}

/// The footer's warning on "Move them in" (pictures plan 4.3): the library's notes that link to
/// a moved file are rewritten, and nothing else on the disk is looked for.
pub const MOVE_WARNING: &str =
    "Moved in: files outside the library that link to them will not find them.";

/// What a run of "Move them in" did, for its toast: "2 moved in with 3 pictures, 1 copied (on
/// another drive: copied), 1 library note now leads to its copy".
pub fn moved_text(
    plan: &[mdedit_core::moves::copy_in::Planned],
    mp: &mdedit_core::moves::copy_in::MovePlan,
    out: &mdedit_core::moves::copy_in::MoveOut,
) -> String {
    use mdedit_core::moves::copy_in::Copy;
    let gone = |p: &std::path::Path| {
        out.removed
            .iter()
            .any(|r| mdshared_app::paths::same_path(r, p))
    };
    let filed: Vec<_> = plan
        .iter()
        .filter(|p| !matches!(p.copy, Copy::Stays { .. }))
        .collect();
    let moved = filed.iter().filter(|p| gone(&p.from)).count();
    let pictures = out
        .removed
        .iter()
        .filter(|r| {
            !plan
                .iter()
                .any(|p| mdshared_app::paths::same_path(&p.from, r))
        })
        .count();
    let copied: Vec<_> = filed.iter().filter(|p| !gone(&p.from)).collect();
    let stays = plan.len() - filed.len();
    let mut parts: Vec<String> = Vec::new();
    if moved > 0 {
        parts.push(match pictures {
            0 => format!("{moved} moved in"),
            1 => format!("{moved} moved in with 1 picture"),
            n => format!("{moved} moved in with {n} pictures"),
        });
    }
    if !copied.is_empty() {
        // Each file's own reason, so none is taken for another's (Fable's review of C2, nit):
        // one sentence when they share it, else the first few by name.
        let why = |p: &std::path::Path| {
            mp.why_stays(p)
                .map(str::to_string)
                .or_else(|| {
                    out.failures
                        .iter()
                        .find(|(f, _)| mdshared_app::paths::same_path(f, p))
                        .map(|(_, w)| w.clone())
                })
                .unwrap_or_else(|| "copied".to_string())
        };
        let whys: Vec<String> = copied.iter().map(|p| why(&p.from)).collect();
        if whys.iter().all(|w| *w == whys[0]) {
            parts.push(format!("{} copied, not moved ({})", copied.len(), whys[0]));
        } else {
            const NAMED: usize = 3;
            let mut named: Vec<String> = copied
                .iter()
                .zip(&whys)
                .take(NAMED)
                .map(|(p, w)| {
                    let name = p.from.file_name().unwrap_or_default().to_string_lossy();
                    format!("{name} ({w})")
                })
                .collect();
            if copied.len() > NAMED {
                named.push(format!("{} more", copied.len() - NAMED));
            }
            parts.push(format!(
                "{} copied, not moved: {}",
                copied.len(),
                named.join("; ")
            ));
        }
    }
    if stays > 0 {
        parts.push(format!("{stays} linked where it is"));
    }
    match out.relinked.len() {
        0 => {}
        1 => parts.push("1 library note now leads to its copy".to_string()),
        n => parts.push(format!("{n} library notes now lead to the copies")),
    }
    parts.join(", ")
}

/// Why File them leaves a picture out (D-204). Write notes, which puts pictures inside notes,
/// is not built yet; until it is, the row says only that it is not filed, and why.
pub const PICTURE_NOT_FILED: &str = "not filed: a picture is not a note of its own";

/// What the footer says when Rewrite them is run with nothing in the box (audit 4, B14).
pub const REWRITE_NEEDS_INSTRUCTION: &str =
    "Rewrite them needs an instruction: type one, or pick a saved prompt.";

/// The window's limits, from Settings › AI tasks (D-203, D-214): the most text one rewrite takes,
/// the most files one rewrite run takes, and the most files one "File them" request takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub rewrite_bytes: usize,
    pub rewrite_files: usize,
    pub batch: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        use mdedit_core::prefs as p;
        Limits {
            rewrite_bytes: p::AI_REWRITE_KB_DEFAULT * 1000,
            rewrite_files: p::AI_REWRITE_FILES_DEFAULT,
            batch: p::AI_TASK_BATCH_DEFAULT,
        }
    }
}

impl Limits {
    pub fn of(prefs: &mdedit_core::prefs::Prefs) -> Limits {
        Limits {
            rewrite_bytes: prefs.ai_rewrite_kb.saturating_mul(1000),
            rewrite_files: prefs.ai_rewrite_files.max(1),
            batch: prefs.ai_task_batch.max(1),
        }
    }
}
/// The most text a file may hold to be read into the window at all.
pub const ADD_MAX: u64 = 8 * 1024 * 1024;
/// The first bytes of a text a digest sends.
pub const DIGEST_BYTES: usize = 1200;
/// The most a model's title takes.
pub const TITLE_MAX: usize = 120;
/// The most sections one file takes.
pub const SECTIONS_MAX: usize = 3;

/// The picture extensions (the list image paste uses).
pub const PICTURES: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "avif"];

/// Extensions that are a kind of file, never text: not read at all.
pub fn other_reason(ext: &str) -> Option<&'static str> {
    Some(match ext.to_ascii_lowercase().as_str() {
        "pdf" => "PDF: this task needs text",
        "doc" | "docx" | "odt" | "rtf" => "Word document: save it as text or Markdown first",
        "xls" | "xlsx" | "ods" | "csv" => "Spreadsheet: save it as text or Markdown first",
        "ppt" | "pptx" | "odp" => "Presentation: save it as text or Markdown first",
        "zip" | "rar" | "7z" | "gz" | "bz2" | "xz" | "tar" | "iso" => "an archive: not a text file",
        "exe" | "dll" | "so" | "dylib" | "bin" | "msi" | "app" => "a program: not a text file",
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "mp4" | "mkv" | "mov" | "avi" | "webm" | "wmv" => {
            "a sound or video file: not a text file"
        }
        "psd" | "ai" | "blend" | "db" | "sqlite" | "ttf" | "otf" | "woff" | "woff2" => {
            "not a text file"
        }
        _ => return None,
    })
}

/// The first `# ` heading of `text`, without its marker.
pub fn first_heading(text: &str) -> Option<String> {
    text.lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").trim().to_string())
        .filter(|s| !s.is_empty())
}

/// A name as a title or section takes it: one line, no control characters, `max` characters.
pub fn clean_name(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or("");
    let mut out = String::new();
    let mut n = 0;
    for c in line.chars() {
        if c.is_control() {
            continue;
        }
        if n == max {
            break;
        }
        out.push(c);
        n += 1;
    }
    out.trim().to_string()
}

/// A section path a model returned: at most [`SECTIONS_MAX`] names, each cleaned for a folder,
/// empty ones dropped.
pub fn clean_sections(list: &[String]) -> Vec<String> {
    list.iter()
        .map(|s| clean_name(s, 64))
        .filter(|s| !s.is_empty())
        .take(SECTIONS_MAX)
        .map(|s| mdshared_app::library::folder_name(&s))
        .filter(|s| !s.is_empty())
        .collect()
}

/// The title a file gets when the model names none: its first heading, else its name without
/// its extension.
pub fn default_title(file: &FileEntry) -> String {
    file.heading.clone().unwrap_or_else(|| {
        Path::new(&file.name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.name.clone())
    })
}

// ---- destinations -----------------------------------------------------------------------------------

/// Where a task's files go: a group of the library, a new one, or decided on each plan card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DestChoice {
    Group(usize),
    New(String),
    Decide,
}

/// A group the window can file into.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub file: PathBuf,
    pub home: PathBuf,
}

// ---- the window's state -----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hit {
    Card,
    Field,
    Prompt(usize),
    Kind(TaskKind),
    /// "Add": opens the Add menu.
    AddFiles,
    /// An item of the open Add menu.
    AddItem(usize),
    File(usize),
    Body,
    Dest,
    Section,
    /// The name of a new group being typed.
    NewGroupField,
    /// A side of File them's THE FILES switch (D-209).
    FilesGo(mdedit_core::prefs::FilesGo),
    /// An item of the open destination list.
    DestItem(usize),
    ShowSent,
    Primary,
    Cancel,
    OpenSettings,
    Track,
    Thumb,
    /// Write notes' Options button, and its popover's lines.
    Options,
    HowMany(mdedit_core::notes_ai::HowMany),
    Tick(crate::notes_task::Tick),
    /// A look request failed: send the failed ones again, or go on without them.
    TryAgain,
    GoOn,
    /// The Options popover's own ground: a click there does nothing, and keeps it open.
    Popover,
}

/// The list's scroll and the places the last draw put its rows.
#[derive(Clone, Debug, Default)]
pub struct ListState {
    pub scroll: f32,
    pub view: f32,
    pub content: f32,
    pub area: Rect,
    pub grab: Option<f32>,
}

impl ListState {
    pub fn max_scroll(&self) -> f32 {
        (self.content - self.view).max(0.0)
    }

    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll = (self.scroll + dy).clamp(0.0, self.max_scroll());
    }

    /// The track and thumb, as last drawn.
    pub fn bar(&self) -> Option<(Rect, Rect)> {
        scrollbar(self.area, self.content, self.view, self.scroll)
    }
}

/// One line of the window's Add menu (D-207): an open tab's file, by name, the one in front
/// first; then "Files…", the file dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddItem {
    Tab {
        path: PathBuf,
        name: String,
        /// The document in front.
        front: bool,
    },
    Files,
}

impl AddItem {
    pub fn label(&self) -> String {
        match self {
            AddItem::Tab { name, front, .. } if *front => format!("{name} \u{00B7} in front"),
            AddItem::Tab { name, .. } => name.clone(),
            AddItem::Files => "Files\u{2026}".to_string(),
        }
    }
}

pub struct TaskScreen {
    /// Which window this is: a review kept from one window is never applied to another's files
    /// (audit 4, B4). Set by the app when it opens the window; 0 in a test that makes one.
    pub id: u64,
    pub task: TaskKind,
    pub files: Vec<FileEntry>,
    /// The groups read when the window opened; a new group is added when it is made.
    pub groups: Vec<Group>,
    pub dest: DestChoice,
    pub list: ListState,
    pub instruction: TextField,
    /// The name of a new group being typed.
    pub new_group: Option<TextField>,
    /// The field the keyboard is in.
    pub focus: Focus,
    /// The section the files are filed under (File them): a heading path, one name.
    pub section: TextField,
    /// The saved prompts of the current task's kind.
    pub prompts: Vec<mdedit_core::ai::SavedPrompt>,
    pub prompt_sel: Option<usize>,
    /// How far the saved prompts are scrolled.
    pub prompt_scroll: f32,
    /// The destination list is open.
    pub dest_open: bool,
    /// The Add menu is open, with these lines.
    pub add_open: bool,
    pub add_items: Vec<AddItem>,
    /// The limits it was opened with (D-203), from the settings.
    pub limits: Limits,
    /// Where the files came from, when the window was opened from a row of the Notes pane
    /// (pictures plan 4.2); said in the header, and forgotten when a file is added.
    pub from: Option<String>,
    /// What File them does with the files (THE FILES, D-209), from the settings.
    pub files_go: mdedit_core::prefs::FilesGo,
    /// The library's folder: a file inside it is linked where it is, whatever THE FILES says.
    pub library: PathBuf,
    pub dest_hot: Option<usize>,
    pub hover: Option<Hit>,
    pub pressed: Option<Hit>,
    pub hits: Vec<(Hit, Rect)>,
    /// What the footer says about the run: bytes, requests, the provider.
    pub via: String,
    /// The model's answers have been read and the rows show where each file goes.
    pub planned: bool,
    /// A request is out: what it is for.
    pub asking: Option<Asking>,
    /// "File them"'s batches still to send, and the answers read so far.
    pub batch_queue: Vec<Vec<usize>>,
    pub answers: Vec<(Vec<usize>, Filing)>,
    /// Why the window cannot run: the reason on the footer.
    pub blocked: Option<String>,
    /// A change of what the window shows.
    pub tick: u64,
    pub caret_on: bool,
    pub scale: f32,
    pub win: (f32, f32),
    /// Set by the window's part from the settings: the connection is not off.
    pub ai_on: bool,
    /// A run of Split them from this window has opened its note: the later runs open none, so
    /// a split over several files is one tab, not one per file.
    pub split_opened: bool,
    /// Write notes' options, as the settings keep them.
    pub notes: mdedit_core::prefs::NotesOptions,
    /// The Options popover is open.
    pub options_open: bool,
    /// The `prepare` request out, and the files it measures, by their place in the window.
    pub prepare: Vec<(u64, Vec<usize>)>,
    /// Write notes' run: the looks, then the arrange.
    pub notes_run: Option<crate::notes_task::Run>,
    /// Why no picture can be sent with this connection (an older engine, a CLI that takes none).
    pub pictures_refused: Option<String>,
}

/// A request in flight from the window.
#[derive(Clone, Debug, PartialEq)]
pub struct Asking {
    pub request: u64,
    /// The task that asked: the answer is read as this task's, whatever the window shows now
    /// (audit 4, B5).
    pub task: TaskKind,
    pub what: String,
    pub chars: usize,
    pub index: usize,
    pub prompt: String,
    /// The batch's file places, in the order of its ids.
    pub batch: Vec<usize>,
}

impl TaskScreen {
    pub fn new(
        task: TaskKind,
        groups: Vec<Group>,
        dest: DestChoice,
        prompts: Vec<mdedit_core::ai::SavedPrompt>,
        via: String,
    ) -> TaskScreen {
        TaskScreen {
            id: 0,
            task,
            files: Vec::new(),
            groups,
            dest,
            list: ListState::default(),
            instruction: TextField::new(),
            new_group: None,
            focus: Focus::Instruction,
            section: TextField::new(),
            prompts,
            prompt_sel: None,
            prompt_scroll: 0.0,
            dest_open: false,
            add_open: false,
            add_items: Vec::new(),
            limits: Limits::default(),
            from: None,
            files_go: mdedit_core::prefs::FilesGo::default(),
            library: PathBuf::new(),
            dest_hot: None,
            hover: None,
            pressed: None,
            hits: Vec::new(),
            via,
            planned: false,
            asking: None,
            batch_queue: Vec::new(),
            answers: Vec::new(),
            blocked: None,
            tick: 0,
            caret_on: false,
            scale: 1.0,
            win: (1280.0, 800.0),
            split_opened: false,
            ai_on: false,
            notes: mdedit_core::prefs::NotesOptions::default(),
            options_open: false,
            prepare: Vec::new(),
            notes_run: None,
            pictures_refused: None,
        }
    }

    /// A request is out: the window keeps what it asked about as it was sent.
    pub fn busy(&self) -> bool {
        self.asking.is_some()
            || self
                .notes_run
                .as_ref()
                .is_some_and(|r| r.looking() || r.arrange.is_some())
    }

    /// Write notes: the pictures and the text files that take part, by their place.
    pub fn notes_parts(&self) -> (Vec<usize>, Vec<usize>) {
        let ids = self.taking_part();
        let pictures = ids
            .iter()
            .copied()
            .filter(|&i| self.files[i].kind == FileKind::Picture)
            .collect();
        let texts = ids
            .into_iter()
            .filter(|&i| self.files[i].kind == FileKind::Text)
            .collect();
        (pictures, texts)
    }

    /// Why Write notes cannot be sent now, with both numbers where there are two.
    pub fn notes_blocked(&self) -> Option<String> {
        let (pictures, texts) = self.notes_parts();
        if pictures.is_empty() && texts.is_empty() {
            let texts_ticked = self
                .files
                .iter()
                .any(|f| f.kind == FileKind::Text && f.ticked);
            return Some(if texts_ticked && !self.ai_on {
                "With the AI off only pictures make notes: add a picture, or turn the AI on."
                    .to_string()
            } else {
                "No file takes part.".to_string()
            });
        }
        if self.ai_on
            && !pictures.is_empty()
            && let Some(why) = &self.pictures_refused
        {
            return Some(why.clone());
        }
        let text_bytes = texts
            .iter()
            .map(|&i| self.files[i].text.as_ref().map_or(0, String::len))
            .sum();
        let waiting = self.ai_on
            && pictures
                .iter()
                .any(|&i| self.files[i].measure == crate::notes_task::Measure::Waiting);
        crate::notes_task::blocked(pictures.len(), text_bytes, waiting)
    }

    /// The files that take part, by their place in `files`.
    pub fn taking_part(&self) -> Vec<usize> {
        (0..self.files.len())
            .filter(|&i| {
                let f = &self.files[i];
                f.ticked && !f.done && self.reason_of(i).is_none()
            })
            .collect()
    }

    /// Why the file at `i` takes no part: its own reason, or (Write notes) a text file that
    /// this run would not use.
    pub fn reason_of(&self, i: usize) -> Option<String> {
        let f = self.files.get(i)?;
        f.reason_for(self.task).or_else(|| {
            (self.task == TaskKind::Notes && f.kind == FileKind::Text)
                .then(|| self.notes_text_unused())
                .flatten()
                .map(str::to_string)
        })
    }

    /// Write notes: why the text files are not used now, if they are not. With the AI off the
    /// notes are the pictures' own; with One per picture no request reads the text.
    pub fn notes_text_unused(&self) -> Option<&'static str> {
        if !self.ai_on {
            return Some("not used with the AI off: the pictures alone make the notes");
        }
        let pictures = self
            .files
            .iter()
            .any(|f| f.kind == FileKind::Picture && f.ticked && !f.done && f.takes_part(self.task));
        (self.notes.how_many == mdedit_core::notes_ai::HowMany::PerPicture && pictures)
            .then_some("not used with One per picture: no request reads it")
    }

    /// Whether a file has a place on the plan face.
    pub fn is_placed(&self, i: usize) -> bool {
        let Some(f) = self.files.get(i) else {
            return false;
        };
        f.title.is_some() || !f.sections.is_empty() || f.with.is_some()
    }

    /// What the model did not place is left out, unticked, with its row saying so.
    pub fn leave_out_unplaced(&mut self) {
        for i in 0..self.files.len() {
            if !self.is_placed(i) {
                self.files[i].ticked = false;
            }
        }
    }

    /// The files "Add N to the library" writes: those ticked and placed.
    pub fn to_write(&self) -> Vec<usize> {
        self.taking_part()
            .into_iter()
            .filter(|&i| self.is_placed(i))
            .collect()
    }

    /// How many files the button counts.
    pub fn count(&self) -> usize {
        if self.planned {
            self.to_write().len()
        } else {
            self.taking_part().len()
        }
    }

    /// The destination group, when one is chosen.
    pub fn group(&self) -> Option<&Group> {
        match &self.dest {
            DestChoice::Group(i) => self.groups.get(*i),
            _ => None,
        }
    }

    pub fn dest_text(&self) -> String {
        match &self.dest {
            DestChoice::Group(i) => self
                .groups
                .get(*i)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "Choose a group".to_string()),
            DestChoice::New(name) if !name.is_empty() => format!("A new group: {name}"),
            DestChoice::New(_) => "A new group\u{2026}".to_string(),
            DestChoice::Decide => "Decide on the plan".to_string(),
        }
    }

    /// The folder the files go to, beside the destination: `→ library\Recipes\Pasta\`.
    pub fn dest_folder(&self) -> Option<PathBuf> {
        let g = self.group()?;
        let section = clean_name(self.section.text(), 64);
        Some(mdshared_app::library::note_folder(
            &g.home,
            if section.is_empty() {
                &[]
            } else {
                std::slice::from_ref(&section)
            },
        ))
    }

    /// What THE FILES does to the file at `i` when it is filed, as its row says it on the plan
    /// face: `None` when the files are linked, or the task is not File them.
    pub fn copy_note(&self, i: usize) -> Option<&'static str> {
        use mdedit_core::prefs::FilesGo;
        if self.task != TaskKind::File || self.files_go == FilesGo::Link {
            return None;
        }
        let f = self.files.get(i)?;
        let home = self.group().map(|g| g.home.clone());
        let inside = f.path.ancestors().skip(1).any(|a| {
            (!self.library.as_os_str().is_empty()
                && mdshared_app::paths::same_path(a, &self.library))
                || home.as_ref().is_some_and(|h| {
                    !h.as_os_str().is_empty() && mdshared_app::paths::same_path(a, h)
                })
        });
        Some(if inside {
            "linked where it is: already inside the library's folder"
        } else if self.files_go == FilesGo::Move {
            "moved in"
        } else {
            "copied in"
        })
    }

    pub fn hit_at(&self, x: f32, y: f32) -> Option<Hit> {
        self.hits
            .iter()
            .rev()
            .find(|(_, r)| r.contains(x, y))
            .map(|(h, _)| *h)
    }

    /// The task changes: what takes part changes with it, and the prompts are of its kind.
    pub fn set_task(&mut self, task: TaskKind) {
        if self.task == task {
            return;
        }
        self.task = task;
        self.planned = false;
        self.options_open = false;
        self.dest_open = false;
        self.add_open = false;
        self.focus = Focus::Instruction;
        self.recount_limit();
        self.tick += 1;
    }

    /// A click on file `i`'s row: its tick changes when it can take part and was not written,
    /// and the twenty a rewrite takes are counted again.
    pub fn toggle(&mut self, i: usize) {
        let task = self.task;
        let Some(f) = self.files.get_mut(i) else {
            return;
        };
        if f.reason_for(task).is_some() || f.done {
            return;
        }
        f.ticked = !f.ticked;
        f.limited = false;
        self.recount_limit();
        self.tick += 1;
    }

    /// A rewrite takes at most [`Limits::rewrite_files`] files. Only the ticked files that could be
    /// rewritten count: one past the twentieth is unticked, and while twenty are ticked every
    /// other says so on its row. Unticking one frees its place; a file the limit unticked is
    /// ticked again when the task is another (audit 4, B13).
    pub fn recount_limit(&mut self) {
        let rewrite = self.task == TaskKind::Rewrite;
        let max = self.limits.rewrite_files;
        let mut n = 0;
        for f in self.files.iter_mut() {
            f.limits = self.limits;
            f.over_limit = false;
            if !rewrite {
                if f.limited {
                    f.ticked = true;
                    f.limited = false;
                }
                continue;
            }
            if f.ticked && f.can_be_rewritten() {
                n += 1;
                if n > max {
                    f.ticked = false;
                    f.limited = true;
                }
            }
        }
        if rewrite && n >= max {
            for f in self.files.iter_mut() {
                f.over_limit = !f.ticked && f.can_be_rewritten();
            }
        }
    }

    /// The footer's summary: what goes, how much and in how many requests.
    pub fn summary(&self) -> String {
        let n = self.count();
        if n == 0 {
            return "No file takes part.".to_string();
        }
        let s = if n == 1 { "" } else { "s" };
        match self.task {
            TaskKind::Notes => {
                let (pictures, texts) = self.notes_parts();
                let measures: Vec<&crate::notes_task::Measure> =
                    pictures.iter().map(|&i| &self.files[i].measure).collect();
                crate::notes_task::summary(&measures, texts.len(), self.notes.how_many, self.ai_on)
            }
            TaskKind::File => {
                if self.planned {
                    return format!("{n} file{s} placed");
                }
                if !self.asks_ai() {
                    return format!("{n} file{s} \u{00B7} no request");
                }
                let requests = n.div_ceil(self.limits.batch);
                let kb = self
                    .taking_part()
                    .iter()
                    .map(|&i| digest_of(&self.files[i]).len())
                    .sum::<usize>();
                format!(
                    "{n} file{s} \u{00B7} about {} KB in {requests} request{}",
                    kb.div_ceil(1000).max(1),
                    if requests == 1 { "" } else { "s" }
                )
            }
            TaskKind::Split => format!("{n} file{s} \u{00B7} a plan for each"),
            TaskKind::Rewrite => {
                let bytes: usize = self
                    .taking_part()
                    .iter()
                    .map(|&i| self.files[i].text.as_ref().map_or(0, |t| t.len()))
                    .sum();
                format!(
                    "{n} file{s} \u{00B7} {} KB in {n} request{}",
                    bytes.div_ceil(1000).max(1),
                    if n == 1 { "" } else { "s" }
                )
            }
        }
    }

    /// The primary button's words.
    pub fn primary_label(&self) -> String {
        let n = self.count();
        if self.planned {
            return format!("Add {n} to the library");
        }
        match self.task {
            TaskKind::Notes => {
                let (pictures, texts) = self.notes_parts();
                // After a stop, the pictures looked at already do not go again.
                let instruction = self.instruction.text();
                let to_send = match self
                    .notes_run
                    .as_ref()
                    .filter(|r| r.keeps(instruction.trim(), self.notes.text_out))
                {
                    Some(run) => pictures.iter().filter(|&&i| run.look(i).is_none()).count(),
                    None => pictures.len(),
                };
                crate::notes_task::primary_label(to_send, texts.len(), self.ai_on)
            }
            TaskKind::File | TaskKind::Split if self.asks_ai() => {
                format!("Run on {n} file{}", if n == 1 { "" } else { "s" })
            }
            TaskKind::File => "Make the plan".to_string(),
            TaskKind::Split => {
                format!("Make the plan on {n} file{}", if n == 1 { "" } else { "s" })
            }
            TaskKind::Rewrite => format!("Run on {n} file{}", if n == 1 { "" } else { "s" }),
        }
    }

    /// Whether this task sends a request now (the AI is off for a File them, which needs none).
    pub fn asks_ai(&self) -> bool {
        match self.task {
            TaskKind::Rewrite => true,
            TaskKind::File | TaskKind::Notes => self.ai_on,
            TaskKind::Split => false,
        }
    }
}

/// A digest of one file as a "File them" request sends it: its name, size, date, first heading
/// and first bytes, at most two kilobytes.
pub fn digest_of(file: &FileEntry) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{} \u{00B7} {}\n",
        file.name,
        size_text(file.size)
    ));
    if let Some(date) = &file.modified {
        out.push_str(&format!("modified: {date}\n"));
    }
    if let Some(h) = &file.heading {
        out.push_str(&format!("first heading: {h}\n"));
    }
    let sample: String = file.sample.chars().take(DIGEST_BYTES).collect();
    if !sample.is_empty() {
        out.push_str("first bytes:\n");
        out.push_str(&sample);
        out.push('\n');
    }
    // Cut at a character, never inside one: `truncate` inside a two- or three-byte letter
    // panics, and the release build aborts on a panic (audit 4, B1).
    let mut end = out.len().min(DIGEST_MAX);
    while !out.is_char_boundary(end) {
        end -= 1;
    }
    out.truncate(end);
    out
}

/// The most bytes one file's digest takes in a "File them" request.
pub const DIGEST_MAX: usize = 2048;

/// The date of `secs` since 1970, as `YYYY-MM-DD` (UTC), for a digest: the model need only a
/// rough date, and the program has no date library of its own.
pub fn date_of(secs: u64) -> String {
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The year, month and day of a count of days since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// "1.2 MB", "412 KB", "12 bytes".
pub fn size_text(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1000 {
        format!("{} KB", bytes.div_ceil(1000))
    } else {
        format!("{bytes} bytes")
    }
}

/// The request text of "File them": the instruction and the batch, each file an id. The model
/// answers only the JSON of 5.6.
pub fn filing_prompt(instruction: &str, ids: &[usize], files: &[FileEntry]) -> String {
    let mut s = String::new();
    s.push_str("File these files into a notes library.\n");
    s.push_str("Answer with one JSON object and nothing else: {\"version\":1,\"files\":[{\"id\":0,\"title\":\"A title\",\"sections\":[\"Section\"]}]}. For a picture or another file placed with a text file, answer {\"id\":5,\"with\":0}. A file you cannot place is left out.\n");
    s.push_str(if instruction.trim().is_empty() {
        "No instruction was given: file each by what it is about.\n"
    } else {
        "INSTRUCTION: "
    });
    if !instruction.trim().is_empty() {
        s.push_str(instruction.trim());
        s.push('\n');
    }
    s.push_str("FILES:\n");
    for (at, &i) in ids.iter().enumerate() {
        let f = &files[i];
        match f.kind {
            FileKind::Text => s.push_str(&format!("{at}. {}", digest_of(f))),
            FileKind::Picture => {
                s.push_str(&format!("{at}. {} \u{00B7} a picture\n", f.name));
            }
            FileKind::Other => {
                s.push_str(&format!("{at}. {} \u{00B7} another kind of file\n", f.name));
            }
        }
    }
    s
}

/// The system text of "File them". The engine is told only a system text and a prompt; the JSON
/// contract lives here, in mdedit, never in the engine.
pub const FILING_SYSTEM: &str = "You file files into a Markdown notes library. You are given an instruction and a numbered list of files with a digest of each text file. Answer with one JSON object and nothing else: {\"version\":1,\"files\":[{\"id\":0,\"title\":\"A title\",\"sections\":[\"Section\"]}]}. A picture or another file placed with a text file answers with {\"id\":5,\"with\":0}; one you cannot place is left out. A title is at most 120 characters, one line; at most three sections, each one folder name. Never write a path.";

/// What a model's "File them" answer says about one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filed {
    pub id: usize,
    pub title: Option<String>,
    pub sections: Vec<String>,
    pub with: Option<usize>,
}

/// A read answer: the entries it placed, in the order of its ids, and the ids it named that
/// were not sent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filing {
    pub placed: Vec<Filed>,
    pub unknown: usize,
}

/// Reads a "File them" answer. `ids` is the batch as sent: its place is the id the model saw and
/// its value is the file's place in the window. The first answer for an id wins; an id that was
/// not sent, and a `with` that names a file that is not a text file of the batch, are ignored.
pub fn read_filing(text: &str, ids: &[usize], files: &[FileEntry]) -> Result<Filing, String> {
    let json = answer::first_object(text).ok_or_else(|| "no JSON object".to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
    let list = value
        .get("files")
        .and_then(|f| f.as_array())
        .ok_or_else(|| "the answer has no files".to_string())?;
    let mut placed: Vec<Filed> = Vec::new();
    let mut unknown = 0;
    for item in list {
        let Some(id) = item.get("id").and_then(|v| v.as_u64()).map(|v| v as usize) else {
            unknown += 1;
            continue;
        };
        if id >= ids.len() {
            unknown += 1;
            continue;
        }
        if placed.iter().any(|p| p.id == id) {
            continue;
        }
        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .map(|t| clean_name(t, TITLE_MAX))
            .filter(|t| !t.is_empty());
        let sections: Vec<String> = item
            .get("sections")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let sections = clean_sections(&sections);
        let with = item
            .get("with")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        // `with` must name a text file of this batch: anything else is ignored (and the file is
        // left unplaced).
        let with = with.and_then(|w| {
            let at = ids.get(w)?;
            (files.get(*at)?.kind == FileKind::Text).then_some(w)
        });
        if title.is_none() && sections.is_empty() && with.is_none() {
            // An answer with nothing in it places nothing: the row is left "not placed".
            continue;
        }
        placed.push(Filed {
            id,
            title,
            sections,
            with,
        });
    }
    Ok(Filing { placed, unknown })
}

/// The files of a batch that the answer did not place.
#[cfg(test)]
pub fn unplaced(filing: &Filing, sent: usize) -> Vec<usize> {
    (0..sent)
        .filter(|id| !filing.placed.iter().any(|p| p.id == *id))
        .collect()
}

// ---- the list's rows --------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Focus {
    Instruction,
    Section,
    NewGroup,
}

/// A line of the list: a group's heading or a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListRow {
    Heading(FileKind),
    /// On the plan face: the files the model placed, with what each becomes.
    Placed,
    File(usize),
    /// The heading over the files a model could not place, on the plan face.
    Unplaced,
    /// A picture's row on Write notes, with its thumbnail: taller than a file's.
    Picture(usize),
}

/// The rows to draw, in order. On the plan face the placed files come first, in the order they
/// were added, and the files the model could not place under "NOT PLACED", unticked. Before
/// that, text, pictures and other files each sit under their group's heading.
pub fn rows(s: &TaskScreen) -> Vec<ListRow> {
    let mut out: Vec<ListRow> = Vec::new();
    if s.planned {
        let placed: Vec<usize> = (0..s.files.len())
            .filter(|&i| {
                s.files[i].title.is_some()
                    || !s.files[i].sections.is_empty()
                    || s.files[i].with.is_some()
            })
            .collect();
        if !placed.is_empty() {
            out.push(ListRow::Placed);
            out.extend(placed.into_iter().map(ListRow::File));
        }
        let not: Vec<usize> = (0..s.files.len())
            .filter(|&i| {
                s.files[i].title.is_none()
                    && s.files[i].sections.is_empty()
                    && s.files[i].with.is_none()
            })
            .collect();
        if !not.is_empty() {
            out.push(ListRow::Unplaced);
            out.extend(not.into_iter().map(ListRow::File));
        }
        return out;
    }
    // Write notes puts its pictures first: they are what it is for.
    let order = if s.task == TaskKind::Notes {
        [FileKind::Picture, FileKind::Text, FileKind::Other]
    } else {
        [FileKind::Text, FileKind::Picture, FileKind::Other]
    };
    for kind in order {
        let of: Vec<usize> = (0..s.files.len())
            .filter(|&i| s.files[i].kind == kind)
            .collect();
        if of.is_empty() {
            continue;
        }
        out.push(ListRow::Heading(kind));
        if kind == FileKind::Picture && s.task == TaskKind::Notes {
            out.extend(of.into_iter().map(ListRow::Picture));
        } else {
            out.extend(of.into_iter().map(ListRow::File));
        }
    }
    out
}

/// The height of the list's content in `s` as last made.
pub fn content_h(rows: &[ListRow]) -> f32 {
    rows.iter()
        .map(|r| match r {
            ListRow::Heading(_) | ListRow::Placed | ListRow::Unplaced => GROUP_H,
            ListRow::File(_) => ROW_H,
            ListRow::Picture(_) => PICTURE_ROW_H,
        })
        .sum::<f32>()
        + 8.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const REWRITE_MAX_FILES: usize = mdedit_core::prefs::AI_REWRITE_FILES_DEFAULT;

    fn entry(name: &str, kind: FileKind) -> FileEntry {
        let path = PathBuf::from(name);
        FileEntry {
            path: path.clone(),
            name: name.to_string(),
            size: 10,
            kind,
            why: None,
            ticked: true,
            text: Some("text\n".to_string()),
            bom: false,
            sample: "text\n".to_string(),
            heading: None,
            title: None,
            sections: Vec::new(),
            with: None,
            over_limit: false,
            limited: false,
            eol: "\n".to_string(),
            modified: None,
            already: false,
            done: false,
            limits: Limits::default(),
            measure: Default::default(),
            thumb: None,
        }
    }

    /// Audit 4, B13: the twenty a rewrite takes are the ticked ones. Unticking one of them frees
    /// a place for another; a file the limit unticked is ticked again under another task.
    #[test]
    fn the_twenty_a_rewrite_takes_are_counted_among_the_ticked_files() {
        let mut s = TaskScreen::new(
            TaskKind::File,
            Vec::new(),
            DestChoice::New(String::new()),
            Vec::new(),
            "Claude".to_string(),
        );
        for i in 0..25 {
            s.files.push(entry(&format!("n{i}.md"), FileKind::Text));
        }
        s.files[24].already = true;
        s.files[24].ticked = false;
        s.set_task(TaskKind::Rewrite);
        let ticked = |s: &TaskScreen| s.files.iter().filter(|f| f.ticked).count();
        assert_eq!(ticked(&s), REWRITE_MAX_FILES);
        assert!(s.files[19].ticked && !s.files[20].ticked);
        let limit = format!("only {REWRITE_MAX_FILES} files a run");
        assert_eq!(
            s.files[20].reason_for(TaskKind::Rewrite),
            Some(limit.clone())
        );
        assert_eq!(
            s.files[24].reason_for(TaskKind::File).as_deref(),
            Some("already in the library"),
            "File them leaves out a file the library lists"
        );
        assert!(
            !s.files[24].ticked,
            "an unticked file is not ticked by the change of task"
        );
        // Unticking the first frees its place: the twenty-first can be ticked.
        s.toggle(0);
        assert!(!s.files[0].ticked);
        assert_eq!(s.files[20].reason_for(TaskKind::Rewrite), None);
        s.toggle(20);
        assert!(s.files[20].ticked, "the freed place is taken");
        assert_eq!(ticked(&s), REWRITE_MAX_FILES);
        assert_eq!(
            s.files[21].reason_for(TaskKind::Rewrite),
            Some(limit.clone())
        );
        assert_eq!(
            s.files[0].reason_for(TaskKind::Rewrite),
            Some(limit),
            "with twenty ticked, the one unticked waits too"
        );
        s.toggle(21);
        assert!(!s.files[21].ticked, "no twenty-first");
        // File them takes them all: the ones the limit unticked are ticked again, the one the
        // owner unticked is not.
        s.set_task(TaskKind::File);
        assert!(s.files[21].ticked && s.files[23].ticked);
        assert!(!s.files[0].ticked);
    }

    #[test]
    fn a_picture_is_a_picture_and_a_pdf_says_why_it_is_not_text() {
        let mut f = entry("lasagne.jpg", FileKind::Picture);
        f.text = None;
        assert_eq!(
            f.reason_for(TaskKind::Rewrite).as_deref(),
            Some("a picture: not sent for this task")
        );
        assert_eq!(
            f.reason_for(TaskKind::File).as_deref(),
            Some(PICTURE_NOT_FILED),
            "File them never files a picture (D-204)"
        );
        f.ticked = true;
        assert!(!f.takes_part(TaskKind::File));
        let mut pdf = entry("menu.pdf", FileKind::Other);
        pdf.why = other_reason("pdf").map(str::to_string);
        assert_eq!(
            pdf.reason_for(TaskKind::Rewrite).as_deref(),
            Some("PDF: this task needs text")
        );
    }

    #[test]
    fn a_rewrite_refuses_a_file_over_200_kb_with_the_numbers() {
        let mut f = entry("long.md", FileKind::Text);
        f.size = 412_000;
        f.text = None;
        assert_eq!(
            f.reason_for(TaskKind::Rewrite).as_deref(),
            Some("too long for one request: 412 KB of 200 KB")
        );
        assert!(f.reason_for(TaskKind::Split).is_none());
    }

    #[test]
    fn a_filing_answer_places_what_was_sent_and_ignores_an_id_it_was_not() {
        let files = vec![
            entry("a.md", FileKind::Text),
            entry("b.md", FileKind::Text),
            entry("c.jpg", FileKind::Picture),
        ];
        let ids = [0, 1, 2];
        let answer = r#"{"version":1,"files":[
            {"id":0,"title":"Pasta","sections":["Food","Dinner"]},
            {"id":2,"with":0},
            {"id":9,"title":"Not sent"},
            {"id":0,"title":"The first one wins"}
        ]}"#;
        let filing = read_filing(answer, &ids, &files).expect("it reads");
        assert_eq!(filing.unknown, 1);
        assert_eq!(filing.placed.len(), 2);
        assert_eq!(filing.placed[0].title.as_deref(), Some("Pasta"));
        assert_eq!(filing.placed[0].sections, ["Food", "Dinner"]);
        assert_eq!(filing.placed[1].with, Some(0));
        assert_eq!(unplaced(&filing, ids.len()), vec![1]);
    }

    #[test]
    fn a_with_that_names_a_picture_is_ignored() {
        let files = vec![
            entry("a.md", FileKind::Text),
            entry("b.jpg", FileKind::Picture),
        ];
        let filing = read_filing(
            r#"{"version":1,"files":[{"id":1,"with":1}]}"#,
            &[0, 1],
            &files,
        )
        .expect("it reads");
        assert!(filing.placed.is_empty());
    }

    #[test]
    fn a_date_is_the_utc_day_since_1970() {
        assert_eq!(date_of(0), "1970-01-01");
        assert_eq!(date_of(86_400 * 11_017), "2000-03-01");
        assert_eq!(date_of(1_763_856_000), "2025-11-23");
    }

    #[test]
    fn a_title_is_one_clean_line_and_a_section_a_folder_name() {
        assert_eq!(clean_name("  Pasta\nsecond line", TITLE_MAX), "Pasta");
        assert_eq!(clean_name("a\u{0007}b", TITLE_MAX), "ab");
        assert_eq!(
            clean_sections(&[
                "Food/Drink".to_string(),
                "  ".to_string(),
                "A:B".to_string(),
                "Fourth".to_string()
            ]),
            ["Food_Drink", "A_B", "Fourth"]
        );
    }

    #[test]
    fn the_digest_is_at_most_two_kilobytes_and_holds_the_heading() {
        let mut f = entry("a.md", FileKind::Text);
        f.sample = "x".repeat(5000);
        f.heading = Some("Pasta".to_string());
        let d = digest_of(&f);
        assert!(d.len() <= 2048, "{}", d.len());
        assert!(d.contains("first heading: Pasta"));
        assert_eq!(
            first_heading("# Pasta\n\n# Second\n"),
            Some("Pasta".to_string())
        );
    }

    /// Audit 4, B1: a digest whose 2,048th byte falls inside a letter is cut before it, and
    /// does not panic (the release build aborts on a panic). Every offset of the cut is tried,
    /// with letters of two, three and four bytes.
    #[test]
    fn the_digest_is_cut_at_a_character_whatever_the_letters() {
        for letter in ["ж", "語", "😀"] {
            for lead in 0..4 {
                let mut f = entry(&format!("{}.md", "a".repeat(lead)), FileKind::Text);
                f.heading = Some("T".to_string());
                f.sample = letter.repeat(1300);
                let d = digest_of(&f);
                assert!(d.len() <= DIGEST_MAX, "{}", d.len());
                assert!(d.len() > DIGEST_MAX - 4, "{}", d.len());
                assert!(d.ends_with(letter), "{letter} {lead}");
            }
        }
    }

    #[test]
    fn the_list_puts_text_first_and_then_the_pictures() {
        let mut s = TaskScreen::new(
            TaskKind::File,
            Vec::new(),
            DestChoice::Decide,
            Vec::new(),
            String::new(),
        );
        s.files = vec![
            entry("x.jpg", FileKind::Picture),
            entry("a.md", FileKind::Text),
        ];
        assert_eq!(
            rows(&s),
            vec![
                ListRow::Heading(FileKind::Text),
                ListRow::File(1),
                ListRow::Heading(FileKind::Picture),
                ListRow::File(0)
            ]
        );
    }

    /// Fable's review of C2, nit: each file copied and not moved is given its own reason; one
    /// reason for all is said once.
    #[test]
    fn the_message_gives_each_file_its_own_reason() {
        use mdedit_core::moves::copy_in::{self as ci, Copy, MoveOut, MovePlan, Planned};
        let planned = |name: &str| Planned {
            from: PathBuf::from(format!("/desk/{name}")),
            copy: Copy::New {
                to: PathBuf::from(format!("/lib/{name}")),
                text: None,
            },
            pictures: Vec::new(),
            rewrites: 0,
            note: None,
        };
        let plan = vec![planned("a.md"), planned("b.md")];
        let mp = MovePlan {
            stays: vec![
                (PathBuf::from("/desk/a.md"), ci::OTHER_DRIVE.to_string()),
                (PathBuf::from("/desk/b.md"), ci::UNSAVED.to_string()),
            ],
            ..MovePlan::default()
        };
        let text = moved_text(&plan, &mp, &MoveOut::default());
        assert_eq!(
            text,
            format!(
                "2 copied, not moved: a.md ({}); b.md ({})",
                ci::OTHER_DRIVE,
                ci::UNSAVED
            )
        );
        let same = MovePlan {
            stays: vec![
                (PathBuf::from("/desk/a.md"), ci::UNSAVED.to_string()),
                (PathBuf::from("/desk/b.md"), ci::UNSAVED.to_string()),
            ],
            ..MovePlan::default()
        };
        assert_eq!(
            moved_text(&plan, &same, &MoveOut::default()),
            format!("2 copied, not moved ({})", ci::UNSAVED)
        );
    }
}
