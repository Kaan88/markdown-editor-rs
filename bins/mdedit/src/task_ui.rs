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

/// The card for a `w` x `h` window.
pub fn card(w: f32, h: f32) -> Rect {
    mdshared_ui::overlay::dialog_card(CARD_W, CARD_H, w, h)
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

// ---- what a task is, and what a file is -------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TaskKind {
    File,
    Split,
    Rewrite,
}

impl TaskKind {
    pub const ALL: [TaskKind; 3] = [TaskKind::File, TaskKind::Split, TaskKind::Rewrite];

    pub fn label(self) -> &'static str {
        match self {
            TaskKind::File => "File them",
            TaskKind::Split => "Split them",
            TaskKind::Rewrite => "Rewrite them",
        }
    }

    /// The saved prompts this task lists.
    pub fn prompt_kind(self) -> PromptKind {
        match self {
            TaskKind::File => PromptKind::File,
            TaskKind::Split => PromptKind::Split,
            TaskKind::Rewrite => PromptKind::Rewrite,
        }
    }

    /// The prompts of this task's kind, from the three lists the window keeps.
    pub fn prompts_of(
        self,
        file: &[mdedit_core::ai::SavedPrompt],
        split: &[mdedit_core::ai::SavedPrompt],
        rewrite: &[mdedit_core::ai::SavedPrompt],
    ) -> Vec<mdedit_core::ai::SavedPrompt> {
        match self.prompt_kind() {
            PromptKind::Split => split.to_vec(),
            PromptKind::File => file.to_vec(),
            _ => rewrite.to_vec(),
        }
    }

    pub fn placeholder(self) -> &'static str {
        match self {
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
            (FileKind::Text, TaskKind::File) => "TEXT \u{00B7} READ AND SENT: A DIGEST GOES",
            (FileKind::Text, _) => "TEXT \u{00B7} READ AND SENT",
            (FileKind::Picture, TaskKind::File) => {
                "PICTURES \u{00B7} NAMES ARE SENT, THE PICTURES ARE NOT: LINKED"
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
    /// The text, when it is UTF-8 text of at most [`REWRITE_MAX`]: a rewrite sends it and its
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
    /// A Rewrite run takes 20 files: the 21st and after say so on their row.
    pub over_limit: bool,
    /// The file's dominant line ending, as a rewrite writes its result back with it.
    pub eol: String,
    /// Its date of last change, `YYYY-MM-DD`, in the digest.
    pub modified: Option<String>,
    /// The library already lists it: shown, never ticked, never sent.
    pub already: bool,
    /// The run wrote it (a split's plan was written, a rewrite went in).
    pub done: bool,
}

impl FileEntry {
    /// Why this file takes no part in `task`, on top of what `task_add` found.
    pub fn reason_for(&self, task: TaskKind) -> Option<String> {
        if self.already {
            return Some("already in the library".to_string());
        }
        match task {
            // A PDF, a Word file, an archive: never read, but its name is a link like any other.
            TaskKind::File => None,
            TaskKind::Split => match self.kind {
                FileKind::Text => None,
                FileKind::Picture => Some("a picture: not sent for this task".to_string()),
                FileKind::Other => self.why.clone(),
            },
            TaskKind::Rewrite => match self.kind {
                FileKind::Text => {
                    if self.over_limit {
                        return Some(format!("only {} files a run", REWRITE_MAX_FILES));
                    }
                    if self.text.is_none() {
                        if self.size as usize > REWRITE_MAX {
                            Some(format!(
                                "too long for one request: {} KB of {} KB",
                                self.size.div_ceil(1000),
                                REWRITE_MAX / 1000
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
}

/// The most text one rewrite takes.
pub const REWRITE_MAX: usize = 200_000;
/// The most files one rewrite run takes.
pub const REWRITE_MAX_FILES: usize = 20;
/// The most files one "File them" request takes.
pub const BATCH: usize = 40;
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
    AddFiles,
    File(usize),
    Body,
    Dest,
    Section,
    /// The name of a new group being typed.
    NewGroupField,
    /// An item of the open destination list.
    DestItem(usize),
    ShowSent,
    Primary,
    Cancel,
    OpenSettings,
    Track,
    Thumb,
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

pub struct TaskScreen {
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
}

/// A request in flight from the window.
#[derive(Clone, Debug, PartialEq)]
pub struct Asking {
    pub request: u64,
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
            ai_on: false,
        }
    }

    /// The files that take part, by their place in `files`.
    pub fn taking_part(&self) -> Vec<usize> {
        (0..self.files.len())
            .filter(|&i| self.files[i].takes_part(self.task) && !self.files[i].done)
            .collect()
    }

    /// Whether a file has a place on the plan face.
    pub fn is_placed(&self, i: usize) -> bool {
        let Some(f) = self.files.get(i) else {
            return false;
        };
        f.title.is_some() || !f.sections.is_empty() || f.with.is_some()
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
        self.dest_open = false;
        self.focus = Focus::Instruction;
        self.recount_limit();
        self.tick += 1;
    }

    /// A rewrite takes at most [`REWRITE_MAX_FILES`] files: the ones after the twentieth say so.
    pub fn recount_limit(&mut self) {
        let mut n = 0;
        for f in self.files.iter_mut() {
            let takes = f.kind == FileKind::Text && f.text.is_some() && f.why.is_none();
            f.over_limit = self.task == TaskKind::Rewrite && takes && {
                n += 1;
                n > REWRITE_MAX_FILES
            };
            if f.over_limit && f.ticked {
                f.ticked = false;
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
            TaskKind::File => {
                if self.planned {
                    return format!("{n} file{s} placed");
                }
                if !self.asks_ai() {
                    return format!("{n} file{s} \u{00B7} no request");
                }
                let requests = n.div_ceil(BATCH);
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
            TaskKind::File => self.ai_on,
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
    out.truncate(2048);
    out
}

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
    for kind in [FileKind::Text, FileKind::Picture, FileKind::Other] {
        let of: Vec<usize> = (0..s.files.len())
            .filter(|&i| s.files[i].kind == kind)
            .collect();
        if of.is_empty() {
            continue;
        }
        out.push(ListRow::Heading(kind));
        out.extend(of.into_iter().map(ListRow::File));
    }
    out
}

/// The height of the list's content in `s` as last made.
pub fn content_h(rows: &[ListRow]) -> f32 {
    rows.iter()
        .map(|r| match r {
            ListRow::Heading(_) | ListRow::Placed | ListRow::Unplaced => GROUP_H,
            ListRow::File(_) => ROW_H,
        })
        .sum::<f32>()
        + 8.0
}

#[cfg(test)]
mod tests {
    use super::*;

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
            eol: "\n".to_string(),
            modified: None,
            already: false,
            done: false,
        }
    }

    #[test]
    fn a_picture_is_a_picture_and_a_pdf_says_why_it_is_not_text() {
        let mut f = entry("lasagne.jpg", FileKind::Picture);
        f.text = None;
        assert_eq!(
            f.reason_for(TaskKind::Rewrite).as_deref(),
            Some("a picture: not sent for this task")
        );
        assert!(f.reason_for(TaskKind::File).is_none());
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
}
