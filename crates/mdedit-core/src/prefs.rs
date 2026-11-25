//! The user's preferences as a plain struct: defaults, clamped ranges, and reading and writing as
//! the `key=value` lines of the settings file (the format of `mdshared_app::settings`, as the
//! session file uses it). Preferences live in their own section, `[preferences]`, so they can sit
//! in the same file as the session, whose reader skips sections that are not its own, and the
//! other way round.
//!
//! Reading never fails: unknown keys and sections are ignored, a value that does not parse or is
//! out of range falls back to the default of its key (a number in range but past a limit is
//! clamped when it parses, and a number that does not parse is the default), and the same text
//! read twice gives the same struct. Writing always writes every key, so write, read, write gives
//! the same text.

use crate::ai::{CUSTOMS_MAX, Connection, CustomProvider};
use crate::buffer::Buffer;
use crate::edit::{Cmd, Outcome};
use crate::history::{History, Kind};
use crate::limits::TAB_WIDTH;
use crate::lines::trim_into;
use crate::selection::Selection;
use crate::typing::{Assist, Indent};
use mdshared_app::backdrop::Backdrop;
use mdshared_app::settings::{self, Writer, escape, unescape};

/// The section of the settings file.
pub const SECTION: &str = "preferences";

/// The text size at zoom 100 in the shell (`FONT_PX` in `view/metrics.rs`: 14 logical px, treated
/// as 14 pt), the default of [`Prefs::font_pt`].
pub const DEFAULT_FONT_PT: f32 = 14.0;
pub const FONT_PT_MIN: f32 = 7.0;
pub const FONT_PT_MAX: f32 = 32.0;
pub const TAB_WIDTH_MAX: usize = 16;
pub const INDENT_SIZE_MAX: usize = 16;
/// The minimap's width, logical px: the default and the range the setting takes.
pub const MINIMAP_WIDTH_DEFAULT: usize = 96;
pub const MINIMAP_WIDTH_MIN: usize = 48;
pub const MINIMAP_WIDTH_MAX: usize = 240;
/// Days of use a tab may go unopened before it is offered for closing: the default (as
/// `mdshared_ui::stale::DAYS`) and the range the setting takes.
/// The AI task window's limits, as settings (D-203): files in one "File them" request, files in
/// one rewrite run, and the KB of text one rewrite sends. Defaults first, then the range.
pub const AI_TASK_BATCH_DEFAULT: usize = 40;
pub const AI_TASK_BATCH_MIN: usize = 1;
pub const AI_TASK_BATCH_MAX: usize = 200;
pub const AI_REWRITE_FILES_DEFAULT: usize = 20;
pub const AI_REWRITE_FILES_MIN: usize = 1;
pub const AI_REWRITE_FILES_MAX: usize = 100;
pub const AI_REWRITE_KB_DEFAULT: usize = 200;
pub const AI_REWRITE_KB_MIN: usize = 10;
pub const AI_REWRITE_KB_MAX: usize = 2000;
pub const STALE_DAYS_DEFAULT: u32 = 7;
pub const STALE_DAYS_MIN: u32 = 1;
pub const STALE_DAYS_MAX: u32 = 365;
/// Focused seconds under which a tab is not reopened at the next start, when that is on.
/// (Shared with mdview: [`mdshared_app::brief`].)
pub const BRIEF_SECS_DEFAULT: u32 = mdshared_app::brief::SECS_DEFAULT;
pub const BRIEF_SECS_MIN: u32 = mdshared_app::brief::SECS_MIN;
pub const BRIEF_SECS_MAX: u32 = mdshared_app::brief::SECS_MAX;
/// A font family name longer than this many bytes is not a name: the default is used.
pub const FONT_FAMILY_MAX: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    /// The text font; empty is the shell's default.
    pub font_family: String,
    /// Text size in pt, 7 to 32 (halves are kept, other fractions too; not NaN).
    pub font_pt: f32,
    /// Columns of a tab stop, 1 to 16 (default [`TAB_WIDTH`]).
    pub tab_width: usize,
    /// A file's own indentation ([`crate::indent::detect`]) wins over the two settings below when
    /// it has a clear style.
    pub detect_indent: bool,
    /// Tab inserts spaces rather than a tab character.
    pub indent_with_spaces: bool,
    /// Spaces of one indent level, 1 to 16 (default 4); used when `indent_with_spaces`, and as the
    /// size of a unit of auto-indentation in that case.
    pub indent_size: usize,
    pub auto_pair: bool,
    pub auto_indent: bool,
    pub trim_on_save: bool,
    pub final_newline_on_save: bool,
    pub line_numbers: bool,
    pub highlight_line: bool,
    /// Whether menus and the other things that move are animated (on by default). Off, every
    /// motion is instant, as when the system asks for less motion (which wins while this is on).
    pub animations: bool,
    /// Whether a new window shows the minimap.
    pub minimap: bool,
    /// The minimap column's width, logical px, 48 to 240 (default 96).
    pub minimap_width: usize,
    /// Whether a new window wraps long lines.
    pub word_wrap: bool,
    pub restore_session: bool,
    /// At start, show the pinned tab used last instead of the tab that was showing.
    pub start_on_pinned: bool,
    /// At start, offer to close the tabs not opened in [`stale_days`](Self::stale_days) days of
    /// use. On by default.
    pub offer_stale: bool,
    /// Days of use before an unopened tab is offered, 1 to 365 (default 7).
    pub stale_days: u32,
    /// At start, leave out the tabs looked at for less than [`brief_secs`](Self::brief_secs)
    /// in all, counted while the tab showed in a focused window. Off by default.
    pub skip_brief: bool,
    /// Focused seconds a tab needs to be reopened, 1 to 600 (default 10).
    pub brief_secs: u32,
    /// The rule above applies to the tab that was showing when the window closed as well, so a
    /// file glanced at and left on screen is not reopened either. Off by default: that tab is
    /// the one most likely to be wanted again. Pinned tabs and unsaved text still always come
    /// back.
    pub brief_last: bool,
    /// mdview shows the text of a previewed document as it is typed, not only when it is saved.
    pub live_preview: bool,
    /// What the command bar's Preview button opens (issue #7); the pane in this window by
    /// default, as before the choice.
    pub preview_mode: PreviewMode,
    /// Ctrl+V turns the clipboard's HTML into Markdown in a Markdown document (issue #8). Off by
    /// default.
    pub paste_rich: bool,
    /// The word list opens by itself after three letters (Ctrl+Space opens it either way).
    pub complete_auto: bool,
    /// A system backdrop behind the chrome (Windows 11); off is the flat colours.
    pub backdrop: Backdrop,
    /// Where AI rewrites go; off by default.
    pub ai_connection: Connection,
    /// A rewrite opens in a review before it goes in (on by default); off puts it in at once.
    pub ai_review: bool,
    /// The review opens on the rendered preview rather than the changes.
    pub ai_review_preview: bool,
    /// What opens after an AI run that writes a note (D-211): the first note with its preview,
    /// by default.
    pub ai_after_run: AfterRun,
    /// What File them does with the files it files (D-209): copies them in by default.
    pub ai_files: FilesGo,
    /// Write notes' options (pictures plan section 12).
    pub ai_notes: NotesOptions,
    /// A window of Write notes has said, once ever, that a picture is sent whole (pictures plan
    /// section 13): no later window says it.
    pub ai_pictures_told: bool,
    /// The task window's limits (D-203), in their ranges by [`Prefs::clamp`].
    pub ai_task_batch: usize,
    pub ai_rewrite_files: usize,
    pub ai_rewrite_kb: usize,
    /// The model and effort chosen for each connection but Off, in [`Connection::ALL`] order
    /// from the second; empty is the default.
    pub ai_models: [String; 4],
    pub ai_efforts: [String; 4],
    /// How many AI requests may be in flight at once, per connection, as the owner set it;
    /// empty is the default: three, and one for a local model (D-080).
    pub ai_parallel: [String; 4],
    /// The provider of the OpenAI-compatible API ([`crate::ai::PRESETS`]), and the base URL for
    /// a custom one.
    pub ai_preset: String,
    /// The custom providers, in the order they were added, and the one chosen while
    /// `ai_preset` is `custom`. The file keeps `ai_base_url` too, the chosen one's URL, which is
    /// all a build before the list (1.1.2) reads and writes; a file with only that (one of those
    /// builds wrote it) becomes a list of one, whichever provider is chosen.
    pub ai_customs: Vec<CustomProvider>,
    pub ai_custom: usize,
    /// The Claude Code and Codex programs; empty finds them.
    pub ai_claude_path: String,
    pub ai_codex_path: String,
    /// The folder of the notes library's files; empty is the default, `library` beside the
    /// settings file.
    pub library_dir: String,
}

/// Where the Preview button shows a document: the pane beside the text, mdview, or mdview
/// placed beside this window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PreviewMode {
    #[default]
    Window,
    Viewer,
    SideBySide,
}

impl PreviewMode {
    /// In the order of [`PreviewMode::LABELS`].
    pub const ALL: [PreviewMode; 3] = [
        PreviewMode::Window,
        PreviewMode::Viewer,
        PreviewMode::SideBySide,
    ];
    /// The choices as the settings row and the Preview button's menu name them.
    pub const LABELS: [&'static str; 3] = ["In this window", "In mdview", "Side by side"];

    /// The value in the settings file.
    pub fn key(self) -> &'static str {
        match self {
            PreviewMode::Window => "window",
            PreviewMode::Viewer => "mdview",
            PreviewMode::SideBySide => "side-by-side",
        }
    }

    /// Reads [`PreviewMode::key`]; anything else is `None` (the default).
    pub fn parse(value: &str) -> Option<PreviewMode> {
        let v = value.trim();
        PreviewMode::ALL
            .into_iter()
            .find(|m| m.key().eq_ignore_ascii_case(v))
    }

    /// The place in [`PreviewMode::LABELS`].
    pub fn index(self) -> usize {
        PreviewMode::ALL
            .iter()
            .position(|m| *m == self)
            .unwrap_or(0)
    }

    /// The mode at `index` of [`PreviewMode::LABELS`]; out of range is the default.
    pub fn from_index(index: usize) -> PreviewMode {
        PreviewMode::ALL.get(index).copied().unwrap_or_default()
    }

    pub fn label(self) -> &'static str {
        PreviewMode::LABELS[self.index()]
    }
}

/// What opens after an AI run that writes a note (D-211, pictures plan 4.4): only the first
/// note, never one tab per note.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AfterRun {
    /// The first note in a tab, and the preview shown the way the Preview button shows it.
    #[default]
    Preview,
    /// The first note in a tab; the preview as it was.
    Open,
    /// Nothing opens; the Notes pane and the report say where the notes are.
    Nothing,
}

impl AfterRun {
    /// In the order of [`AfterRun::LABELS`].
    pub const ALL: [AfterRun; 3] = [AfterRun::Preview, AfterRun::Open, AfterRun::Nothing];
    /// The choices as the settings row names them.
    pub const LABELS: [&'static str; 3] =
        ["First note and its preview", "First note only", "Nothing"];

    /// The value in the settings file.
    pub fn key(self) -> &'static str {
        match self {
            AfterRun::Preview => "preview",
            AfterRun::Open => "open",
            AfterRun::Nothing => "nothing",
        }
    }

    /// Reads [`AfterRun::key`]; anything else is `None` (the default).
    pub fn parse(value: &str) -> Option<AfterRun> {
        let v = value.trim();
        AfterRun::ALL
            .into_iter()
            .find(|m| m.key().eq_ignore_ascii_case(v))
    }

    /// The place in [`AfterRun::LABELS`].
    pub fn index(self) -> usize {
        AfterRun::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }

    /// The choice at `index` of [`AfterRun::LABELS`]; out of range is the default.
    pub fn from_index(index: usize) -> AfterRun {
        AfterRun::ALL.get(index).copied().unwrap_or_default()
    }
}

/// THE FILES of the task window's File them (pictures plan 4.3, D-209): what happens to a file
/// outside the library that is filed. Remembered once changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FilesGo {
    /// Each file is copied into its group's folder and the line leads to the copy.
    #[default]
    Copy,
    /// A line in the group; the file stays where it is.
    Link,
    /// Copied in as `Copy`, and then the original is gone from where it was (phase C2, D-210).
    Move,
}

impl FilesGo {
    /// In the order the switch shows them.
    pub const ALL: [FilesGo; 3] = [FilesGo::Link, FilesGo::Copy, FilesGo::Move];

    /// The value in the settings file.
    pub fn key(self) -> &'static str {
        match self {
            FilesGo::Copy => "copy",
            FilesGo::Link => "link",
            FilesGo::Move => "move",
        }
    }

    /// Reads [`FilesGo::key`]; anything else is `None` (the default).
    pub fn parse(value: &str) -> Option<FilesGo> {
        let v = value.trim();
        FilesGo::ALL
            .into_iter()
            .find(|m| m.key().eq_ignore_ascii_case(v))
    }
}

/// Write notes' options (pictures plan section 12): kept in the settings file once changed, as
/// the words of those that are on (`as-needed copy rename describe strip`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotesOptions {
    pub how_many: crate::notes_ai::HowMany,
    /// Copy the pictures beside the notes; off links them where they are.
    pub copy: bool,
    /// Name the copies after what they show.
    pub rename: bool,
    /// Describe each picture under it.
    pub describe: bool,
    /// Write out the text in the pictures, as a quote under each.
    pub text_out: bool,
    /// Remove location and camera details before sending.
    pub strip: bool,
    /// Send larger pictures (2576 px on the long side), for small print.
    pub larger: bool,
}

impl Default for NotesOptions {
    fn default() -> NotesOptions {
        NotesOptions {
            how_many: crate::notes_ai::HowMany::AsNeeded,
            copy: true,
            rename: true,
            describe: true,
            text_out: false,
            strip: true,
            larger: false,
        }
    }
}

impl NotesOptions {
    /// The value in the settings file.
    pub fn key(&self) -> String {
        use crate::notes_ai::HowMany;
        let mut words = vec![match self.how_many {
            HowMany::AsNeeded => "as-needed",
            HowMany::One => "one",
            HowMany::PerPicture => "per-picture",
        }];
        for (on, word) in [
            (self.copy, "copy"),
            (self.rename, "rename"),
            (self.describe, "describe"),
            (self.text_out, "text"),
            (self.strip, "strip"),
            (self.larger, "larger"),
        ] {
            if on {
                words.push(word);
            }
        }
        words.join(" ")
    }

    /// Reads [`NotesOptions::key`]: a value with no word of how many notes is `None` (the
    /// default); a word not known is passed over.
    pub fn parse(value: &str) -> Option<NotesOptions> {
        use crate::notes_ai::HowMany;
        let mut how = None;
        let mut o = NotesOptions {
            how_many: HowMany::AsNeeded,
            copy: false,
            rename: false,
            describe: false,
            text_out: false,
            strip: false,
            larger: false,
        };
        for w in value.split([' ', ',']).filter(|w| !w.is_empty()) {
            match w.to_ascii_lowercase().as_str() {
                "as-needed" => how = Some(HowMany::AsNeeded),
                "one" => how = Some(HowMany::One),
                "per-picture" => how = Some(HowMany::PerPicture),
                "copy" => o.copy = true,
                "rename" => o.rename = true,
                "describe" => o.describe = true,
                "text" => o.text_out = true,
                "strip" => o.strip = true,
                "larger" => o.larger = true,
                _ => {}
            }
        }
        o.how_many = how?;
        Some(o)
    }
}

/// A setting's text longer than this many bytes is not a setting: the default is used.
const AI_TEXT_MAX: usize = 2048;

/// The range of the "Requests at once" setting (D-080).
pub const AI_PARALLEL_MIN: usize = 1;
pub const AI_PARALLEL_MAX: usize = 8;

/// The index of a connection's model and effort in [`Prefs::ai_models`].
fn ai_index(c: Connection) -> Option<usize> {
    Connection::ALL[1..].iter().position(|&x| x == c)
}

impl Default for Prefs {
    fn default() -> Prefs {
        Prefs {
            font_family: String::new(),
            font_pt: DEFAULT_FONT_PT,
            tab_width: TAB_WIDTH,
            detect_indent: true,
            indent_with_spaces: false,
            indent_size: 4,
            auto_pair: true,
            auto_indent: true,
            trim_on_save: false,
            final_newline_on_save: false,
            line_numbers: true,
            highlight_line: true,
            animations: true,
            minimap: false,
            minimap_width: MINIMAP_WIDTH_DEFAULT,
            word_wrap: false,
            restore_session: true,
            start_on_pinned: false,
            offer_stale: true,
            stale_days: STALE_DAYS_DEFAULT,
            skip_brief: false,
            brief_secs: BRIEF_SECS_DEFAULT,
            brief_last: false,
            live_preview: true,
            preview_mode: PreviewMode::Window,
            paste_rich: false,
            complete_auto: false,
            backdrop: Backdrop::Off,
            ai_connection: Connection::Off,
            ai_review: true,
            ai_review_preview: false,
            ai_after_run: AfterRun::Preview,
            ai_files: FilesGo::Copy,
            ai_notes: NotesOptions::default(),
            ai_pictures_told: false,
            ai_task_batch: AI_TASK_BATCH_DEFAULT,
            ai_rewrite_files: AI_REWRITE_FILES_DEFAULT,
            ai_rewrite_kb: AI_REWRITE_KB_DEFAULT,
            ai_models: Default::default(),
            ai_efforts: Default::default(),
            ai_parallel: Default::default(),
            ai_preset: "openai".into(),
            ai_customs: Vec::new(),
            ai_custom: 0,
            ai_claude_path: String::new(),
            ai_codex_path: String::new(),
            library_dir: String::new(),
        }
    }
}

/// A text setting, or empty when it is too long or holds a line break.
fn ai_text(value: &str) -> String {
    let v = unescape(value);
    if v.len() <= AI_TEXT_MAX && !v.contains(['\n', '\r']) {
        v
    } else {
        String::new()
    }
}

fn flag(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn number(value: &str, min: usize, max: usize) -> Option<usize> {
    value
        .trim()
        .parse::<usize>()
        .ok()
        .map(|n| n.clamp(min, max))
}

impl Prefs {
    /// Puts every field in its range.
    pub fn clamp(&mut self) {
        if !self.font_pt.is_finite() {
            self.font_pt = DEFAULT_FONT_PT;
        }
        self.font_pt = self.font_pt.clamp(FONT_PT_MIN, FONT_PT_MAX);
        self.tab_width = self.tab_width.clamp(1, TAB_WIDTH_MAX);
        self.indent_size = self.indent_size.clamp(1, INDENT_SIZE_MAX);
        self.minimap_width = self
            .minimap_width
            .clamp(MINIMAP_WIDTH_MIN, MINIMAP_WIDTH_MAX);
        self.stale_days = self.stale_days.clamp(STALE_DAYS_MIN, STALE_DAYS_MAX);
        self.brief_secs = self.brief_secs.clamp(BRIEF_SECS_MIN, BRIEF_SECS_MAX);
        self.ai_task_batch = self
            .ai_task_batch
            .clamp(AI_TASK_BATCH_MIN, AI_TASK_BATCH_MAX);
        self.ai_rewrite_files = self
            .ai_rewrite_files
            .clamp(AI_REWRITE_FILES_MIN, AI_REWRITE_FILES_MAX);
        self.ai_rewrite_kb = self
            .ai_rewrite_kb
            .clamp(AI_REWRITE_KB_MIN, AI_REWRITE_KB_MAX);
        if self.font_family.len() > FONT_FAMILY_MAX || self.font_family.contains(['\n', '\r']) {
            self.font_family.clear();
        }
        for t in self
            .ai_models
            .iter_mut()
            .chain(self.ai_efforts.iter_mut())
            .chain(self.ai_parallel.iter_mut())
            .chain([
                &mut self.ai_claude_path,
                &mut self.ai_codex_path,
                &mut self.library_dir,
            ])
            .chain(
                self.ai_customs
                    .iter_mut()
                    .flat_map(|c| [&mut c.name, &mut c.base_url, &mut c.model, &mut c.effort]),
            )
        {
            if t.len() > AI_TEXT_MAX || t.contains(['\n', '\r']) {
                t.clear();
            }
        }
        self.ai_customs.truncate(CUSTOMS_MAX);
        self.ai_custom = self.ai_custom.min(self.ai_customs.len().saturating_sub(1));
        if !crate::ai::PRESETS.iter().any(|x| x.key == self.ai_preset)
            || (self.ai_preset == "custom" && self.ai_customs.is_empty())
        {
            self.ai_preset = "openai".into();
        }
    }

    /// Reads the `[preferences]` section of `text` (text with no section header is read as that
    /// section too).
    pub fn parse(text: &str) -> Prefs {
        let mut p = Prefs::default();
        let d = Prefs::default();
        let mut in_section = true;
        // The single custom provider's URL, of the files before the list.
        let mut legacy_url = None;
        // The list as read, by its numbers in the file (gaps are closed below).
        let mut customs: Vec<Option<CustomProvider>> = Vec::new();
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with('[') && t.ends_with(']') {
                in_section = &t[1..t.len() - 1] == SECTION;
                continue;
            }
            if !in_section {
                continue;
            }
            for (key, value) in settings::pairs(line) {
                match key {
                    "font_family" => {
                        let v = unescape(value);
                        p.font_family = if v.len() <= FONT_FAMILY_MAX && !v.contains(['\n', '\r']) {
                            v
                        } else {
                            String::new()
                        };
                    }
                    "font_size" => {
                        p.font_pt = value
                            .trim()
                            .parse::<f32>()
                            .ok()
                            .filter(|v| v.is_finite())
                            .map_or(d.font_pt, |v| v.clamp(FONT_PT_MIN, FONT_PT_MAX))
                    }
                    "tab_width" => {
                        p.tab_width = number(value, 1, TAB_WIDTH_MAX).unwrap_or(d.tab_width)
                    }
                    "detect_indent" => p.detect_indent = flag(value).unwrap_or(d.detect_indent),
                    "indent_spaces" => {
                        p.indent_with_spaces = flag(value).unwrap_or(d.indent_with_spaces)
                    }
                    "indent_size" => {
                        p.indent_size = number(value, 1, INDENT_SIZE_MAX).unwrap_or(d.indent_size)
                    }
                    "auto_pair" => p.auto_pair = flag(value).unwrap_or(d.auto_pair),
                    "auto_indent" => p.auto_indent = flag(value).unwrap_or(d.auto_indent),
                    "trim_on_save" => p.trim_on_save = flag(value).unwrap_or(d.trim_on_save),
                    "final_newline_on_save" => {
                        p.final_newline_on_save = flag(value).unwrap_or(d.final_newline_on_save)
                    }
                    "line_numbers" => p.line_numbers = flag(value).unwrap_or(d.line_numbers),
                    "highlight_line" => p.highlight_line = flag(value).unwrap_or(d.highlight_line),
                    "animations" => p.animations = flag(value).unwrap_or(d.animations),
                    "minimap" => p.minimap = flag(value).unwrap_or(d.minimap),
                    "minimap_width" => {
                        p.minimap_width = number(value, MINIMAP_WIDTH_MIN, MINIMAP_WIDTH_MAX)
                            .unwrap_or(d.minimap_width)
                    }
                    "word_wrap" => p.word_wrap = flag(value).unwrap_or(d.word_wrap),
                    "restore_session" => {
                        p.restore_session = flag(value).unwrap_or(d.restore_session)
                    }
                    "offer_stale" => p.offer_stale = flag(value).unwrap_or(d.offer_stale),
                    "stale_days" => {
                        p.stale_days =
                            number(value, STALE_DAYS_MIN as usize, STALE_DAYS_MAX as usize)
                                .map_or(d.stale_days, |n| n as u32)
                    }
                    "skip_brief" => p.skip_brief = flag(value).unwrap_or(d.skip_brief),
                    "brief_last" => p.brief_last = flag(value).unwrap_or(d.brief_last),
                    "brief_secs" => {
                        p.brief_secs =
                            number(value, BRIEF_SECS_MIN as usize, BRIEF_SECS_MAX as usize)
                                .map_or(d.brief_secs, |n| n as u32)
                    }
                    "start_on_pinned" => {
                        p.start_on_pinned = flag(value).unwrap_or(d.start_on_pinned)
                    }
                    "live_preview" => p.live_preview = flag(value).unwrap_or(d.live_preview),
                    "preview_mode" => {
                        p.preview_mode = PreviewMode::parse(value).unwrap_or(d.preview_mode)
                    }
                    "paste_rich" => p.paste_rich = flag(value).unwrap_or(d.paste_rich),
                    "complete_auto" => p.complete_auto = flag(value).unwrap_or(d.complete_auto),
                    "backdrop" => p.backdrop = Backdrop::parse(value).unwrap_or(d.backdrop),
                    "ai_connection" => {
                        p.ai_connection = Connection::parse(value).unwrap_or(d.ai_connection)
                    }
                    "ai_review" => p.ai_review = flag(value).unwrap_or(d.ai_review),
                    "ai_review_preview" => {
                        p.ai_review_preview = flag(value).unwrap_or(d.ai_review_preview)
                    }
                    "ai_after_run" => {
                        p.ai_after_run = AfterRun::parse(value).unwrap_or(d.ai_after_run)
                    }
                    "ai_files" => p.ai_files = FilesGo::parse(value).unwrap_or(d.ai_files),
                    "ai_notes" => p.ai_notes = NotesOptions::parse(value).unwrap_or(d.ai_notes),
                    "ai_pictures_told" => {
                        p.ai_pictures_told = flag(value).unwrap_or(d.ai_pictures_told)
                    }
                    "ai_task_batch" => {
                        p.ai_task_batch = number(value, AI_TASK_BATCH_MIN, AI_TASK_BATCH_MAX)
                            .unwrap_or(d.ai_task_batch)
                    }
                    "ai_rewrite_files" => {
                        p.ai_rewrite_files =
                            number(value, AI_REWRITE_FILES_MIN, AI_REWRITE_FILES_MAX)
                                .unwrap_or(d.ai_rewrite_files)
                    }
                    "ai_rewrite_kb" => {
                        p.ai_rewrite_kb = number(value, AI_REWRITE_KB_MIN, AI_REWRITE_KB_MAX)
                            .unwrap_or(d.ai_rewrite_kb)
                    }
                    "ai_preset" => {
                        let v = ai_text(value);
                        p.ai_preset = if crate::ai::PRESETS.iter().any(|x| x.key == v) {
                            v
                        } else {
                            d.ai_preset.clone()
                        };
                    }
                    "ai_base_url" => legacy_url = Some(ai_text(value)),
                    "ai_custom" => p.ai_custom = value.trim().parse().unwrap_or(0),
                    "ai_claude_path" => p.ai_claude_path = ai_text(value),
                    "ai_codex_path" => p.ai_codex_path = ai_text(value),
                    "library_dir" => p.library_dir = ai_text(value),
                    k => {
                        let per = |prefix: &str| {
                            k.strip_prefix(prefix)
                                .and_then(Connection::parse)
                                .and_then(ai_index)
                        };
                        // `ai_custom_<n>_<field>`: one field of custom provider n.
                        if let Some((n, field)) = k
                            .strip_prefix("ai_custom_")
                            .and_then(|r| r.split_once('_'))
                            .and_then(|(n, f)| Some((n.parse::<usize>().ok()?, f)))
                            .filter(|(n, _)| *n < CUSTOMS_MAX)
                        {
                            if customs.len() <= n {
                                customs.resize(n + 1, None);
                            }
                            let c = customs[n].get_or_insert_with(CustomProvider::default);
                            let v = ai_text(value);
                            match field {
                                "name" => c.name = v,
                                "url" => c.base_url = v,
                                "model" => c.model = v,
                                "effort" => c.effort = v,
                                _ => {}
                            }
                        } else if let Some(i) = per("ai_model_") {
                            p.ai_models[i] = ai_text(value);
                        } else if let Some(i) = per("ai_effort_") {
                            p.ai_efforts[i] = ai_text(value);
                        } else if let Some(i) = per("ai_parallel_") {
                            p.ai_parallel[i] = ai_text(value);
                        }
                    }
                }
            }
        }
        // The list, its gaps closed and the chosen one kept chosen.
        let chosen = p.ai_custom;
        p.ai_custom = customs[..chosen.min(customs.len())]
            .iter()
            .filter(|c| c.is_some())
            .count();
        p.ai_customs = customs.into_iter().flatten().collect();
        // A file from before the list (or written by such a build since): its one custom
        // provider, with the model and effort the connection had (they were shared by every
        // provider then). Kept whichever provider is chosen, so its URL is not lost.
        let legacy_url = legacy_url.unwrap_or_default();
        if p.ai_customs.is_empty() && (p.ai_preset == "custom" || !legacy_url.trim().is_empty()) {
            let i = ai_index(Connection::OpenAi).unwrap_or(0);
            p.ai_customs.push(CustomProvider {
                name: "Custom".into(),
                base_url: legacy_url,
                model: p.ai_models[i].clone(),
                effort: p.ai_efforts[i].clone(),
            });
            p.ai_custom = 0;
        }
        p.clamp();
        p
    }

    /// The `[preferences]` section: every key, one per line, flags as `1` and `0`.
    pub fn serialize(&self) -> String {
        let mut p = self.clone();
        p.clamp();
        let b = |v: bool| if v { "1" } else { "0" };
        let mut w = Writer::new(SECTION);
        w.line("font_family", &escape(&p.font_family));
        w.line("font_size", &p.font_pt.to_string());
        w.line("tab_width", &p.tab_width.to_string());
        w.line("detect_indent", b(p.detect_indent));
        w.line("indent_spaces", b(p.indent_with_spaces));
        w.line("indent_size", &p.indent_size.to_string());
        w.line("auto_pair", b(p.auto_pair));
        w.line("auto_indent", b(p.auto_indent));
        w.line("trim_on_save", b(p.trim_on_save));
        w.line("final_newline_on_save", b(p.final_newline_on_save));
        w.line("line_numbers", b(p.line_numbers));
        w.line("highlight_line", b(p.highlight_line));
        w.line("animations", b(p.animations));
        w.line("minimap", b(p.minimap));
        w.line("minimap_width", &p.minimap_width.to_string());
        w.line("word_wrap", b(p.word_wrap));
        w.line("restore_session", b(p.restore_session));
        w.line("start_on_pinned", b(p.start_on_pinned));
        w.line("offer_stale", b(p.offer_stale));
        w.line("stale_days", &p.stale_days.to_string());
        w.line("skip_brief", b(p.skip_brief));
        w.line("brief_secs", &p.brief_secs.to_string());
        w.line("brief_last", b(p.brief_last));
        w.line("live_preview", b(p.live_preview));
        w.line("preview_mode", p.preview_mode.key());
        w.line("paste_rich", b(p.paste_rich));
        w.line("complete_auto", b(p.complete_auto));
        w.line("backdrop", p.backdrop.key());
        w.line("ai_connection", p.ai_connection.key());
        w.line("ai_review", b(p.ai_review));
        w.line("ai_review_preview", b(p.ai_review_preview));
        w.line("ai_after_run", p.ai_after_run.key());
        w.line("ai_files", p.ai_files.key());
        w.line("ai_notes", &p.ai_notes.key());
        w.line("ai_pictures_told", b(p.ai_pictures_told));
        w.line("ai_task_batch", &p.ai_task_batch.to_string());
        w.line("ai_rewrite_files", &p.ai_rewrite_files.to_string());
        w.line("ai_rewrite_kb", &p.ai_rewrite_kb.to_string());
        for (i, c) in Connection::ALL[1..].iter().enumerate() {
            w.line(&format!("ai_model_{}", c.key()), &escape(&p.ai_models[i]));
            w.line(&format!("ai_effort_{}", c.key()), &escape(&p.ai_efforts[i]));
            w.line(
                &format!("ai_parallel_{}", c.key()),
                &escape(&p.ai_parallel[i]),
            );
        }
        w.line("ai_preset", &escape(&p.ai_preset));
        // The chosen custom provider's URL, for a build before the list (1.1.2), which keeps
        // only this: written back by it, it comes in here as a list of one.
        if let Some(c) = p.ai_customs.get(p.ai_custom) {
            w.line("ai_base_url", &escape(&c.base_url));
        }
        w.line("ai_custom", &p.ai_custom.to_string());
        for (n, c) in p.ai_customs.iter().enumerate() {
            w.line(&format!("ai_custom_{n}_name"), &escape(&c.name));
            w.line(&format!("ai_custom_{n}_url"), &escape(&c.base_url));
            w.line(&format!("ai_custom_{n}_model"), &escape(&c.model));
            w.line(&format!("ai_custom_{n}_effort"), &escape(&c.effort));
        }
        w.line("ai_claude_path", &escape(&p.ai_claude_path));
        w.line("ai_codex_path", &escape(&p.ai_codex_path));
        w.line("library_dir", &escape(&p.library_dir));
        w.finish()
    }

    /// The custom provider chosen, while the provider is a custom one.
    pub fn ai_custom_chosen(&self) -> Option<&CustomProvider> {
        (self.ai_preset == "custom")
            .then(|| self.ai_customs.get(self.ai_custom))
            .flatten()
    }

    pub fn ai_custom_chosen_mut(&mut self) -> Option<&mut CustomProvider> {
        if self.ai_preset == "custom" {
            self.ai_customs.get_mut(self.ai_custom)
        } else {
            None
        }
    }

    /// The chosen custom provider's base URL; empty for any other provider.
    pub fn ai_url(&self) -> &str {
        self.ai_custom_chosen().map_or("", |c| c.base_url.as_str())
    }

    /// The model chosen for a connection; empty is its default. A custom provider has its own.
    pub fn ai_model(&self, c: Connection) -> &str {
        if c == Connection::OpenAi
            && let Some(x) = self.ai_custom_chosen()
        {
            return &x.model;
        }
        ai_index(c).map_or("", |i| self.ai_models[i].as_str())
    }

    pub fn ai_effort(&self, c: Connection) -> &str {
        if c == Connection::OpenAi
            && let Some(x) = self.ai_custom_chosen()
        {
            return &x.effort;
        }
        ai_index(c).map_or("", |i| self.ai_efforts[i].as_str())
    }

    pub fn set_ai_model(&mut self, c: Connection, m: &str) {
        if c == Connection::OpenAi
            && let Some(x) = self.ai_custom_chosen_mut()
        {
            x.model = m.trim().to_string();
        } else if let Some(i) = ai_index(c) {
            self.ai_models[i] = m.trim().to_string();
        }
    }

    pub fn set_ai_effort(&mut self, c: Connection, e: &str) {
        if c == Connection::OpenAi
            && let Some(x) = self.ai_custom_chosen_mut()
        {
            x.effort = e.trim().to_string();
        } else if let Some(i) = ai_index(c) {
            self.ai_efforts[i] = e.trim().to_string();
        }
    }

    /// How many requests may be in flight at once for `c` (D-080): what the owner set, else
    /// three, and one for a local model.
    pub fn ai_parallel(&self, c: Connection) -> usize {
        ai_index(c)
            .and_then(|i| self.ai_parallel[i].trim().parse::<usize>().ok())
            .filter(|n| (AI_PARALLEL_MIN..=AI_PARALLEL_MAX).contains(n))
            .unwrap_or_else(|| self.ai_parallel_default(c))
    }

    /// What [`Prefs::ai_parallel`] gives before the owner changes it.
    pub fn ai_parallel_default(&self, c: Connection) -> usize {
        let local = c == Connection::OpenAi && crate::ai::preset_by_key(&self.ai_preset).local;
        if local { 1 } else { 3 }
    }

    pub fn set_ai_parallel(&mut self, c: Connection, n: usize) {
        if let Some(i) = ai_index(c) {
            self.ai_parallel[i] = n.clamp(AI_PARALLEL_MIN, AI_PARALLEL_MAX).to_string();
        }
    }

    /// One level of indentation: a tab, or `indent_size` spaces.
    pub fn indent(&self) -> Indent {
        if self.indent_with_spaces {
            Indent::Spaces(self.indent_size.clamp(1, INDENT_SIZE_MAX) as u8)
        } else {
            Indent::Tab
        }
    }

    /// The typing help for a language (`typing::Assist::for_language`) with these preferences'
    /// switches, indent unit and tab width.
    pub fn assist(&self, language: &str) -> Assist {
        self.assist_for(language, None)
    }

    /// [`Prefs::assist`] for a document whose indentation was detected (`detected`): with
    /// `detect_indent` on, that unit is used instead of the settings'.
    pub fn assist_for(&self, language: &str, detected: Option<Indent>) -> Assist {
        let unit = detected.filter(|_| self.detect_indent);
        let mut a = Assist::for_language(language, unit.unwrap_or(self.indent()), self.tab_width);
        a.auto_pair = self.auto_pair;
        a.auto_indent = self.auto_indent;
        a
    }
}

/// Applies the save options as one undo step: with `trim_on_save`, removes the spaces and tabs at
/// the end of every line (as [`crate::lines::trim_trailing_whitespace`] does); with
/// `final_newline_on_save`, adds one line break `eol` at the end of a document that is not empty
/// and does not end with a line break (after the trimming). Nothing when no byte would change
/// (no history entry, the selection returned). The selection ends move with the text; one at the
/// very end stays before an added break. `eol` is the document's line ending, "\r\n", "\n" or
/// "\r" (the shell's most frequent one); anything else is taken as "\n".
///
/// Cost as the trimming: one walk over the lines when `trim_on_save` is set, nothing else
/// otherwise.
pub fn apply_on_save(
    buf: &mut Buffer,
    hist: &mut History,
    sel: Selection,
    prefs: &Prefs,
    eol: &str,
) -> Outcome {
    let len = buf.len_bytes();
    let (mut a, mut c) = (sel.anchor.min(len), sel.caret.min(len));
    let mut cmd = Cmd::new(buf);
    if prefs.trim_on_save {
        trim_into(&mut cmd, &mut a, &mut c);
    }
    if prefs.final_newline_on_save {
        let end = cmd.buf.len_bytes();
        if end > 0 && !matches!(cmd.buf.rope().byte(end - 1), b'\n' | b'\r') {
            let eol = if matches!(eol, "\r\n" | "\n" | "\r") {
                eol
            } else {
                "\n"
            };
            cmd.replace(end..end, eol);
        }
    }
    let after = Selection {
        anchor: a,
        caret: c,
        goal_col: None,
    };
    cmd.finish(hist, Kind::Other, sel, after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{redo, undo};
    use crate::eol::EolCounts;
    use crate::testutil::{Rng, check_change, check_eols};

    #[test]
    fn defaults() {
        let d = Prefs::default();
        assert_eq!(d.font_pt, 14.0);
        assert_eq!(d.tab_width, 4);
        assert!(d.auto_pair && d.auto_indent && d.line_numbers && d.highlight_line);
        assert!(d.restore_session);
        assert!(!d.trim_on_save && !d.final_newline_on_save && !d.minimap && !d.word_wrap);
        assert!(!d.indent_with_spaces);
        assert_eq!((d.indent_size, d.font_family.as_str()), (4, ""));
        assert_eq!(Prefs::parse(""), d);
        assert_eq!(Prefs::parse("garbage\n\n=\n[x]\n"), d);
    }

    /// Animations are on unless the file says off, and the value goes round the file.
    #[test]
    fn animations_on_by_default_and_kept() {
        assert!(Prefs::default().animations);
        assert!(Prefs::parse("[preferences]\nanimations=maybe\n").animations);
        let off = Prefs::parse("[preferences]\nanimations=0\n");
        assert!(!off.animations);
        assert!(!Prefs::parse(&off.serialize()).animations);
        assert!(Prefs::parse(&Prefs::default().serialize()).animations);
    }

    #[test]
    fn parse_clamps_and_falls_back() {
        let p = Prefs::parse(
            "[preferences]\nfont_size=99\ntab_width=0\nindent_size=500\nauto_pair=0\nminimap=yes\n\
             trim_on_save=maybe\nfont_family=Fira%20Code\nunknown=1\nword_wrap=TRUE\n",
        );
        assert_eq!(p.font_pt, 32.0);
        assert_eq!((p.tab_width, p.indent_size), (1, 16));
        assert!(!p.auto_pair && p.minimap && p.word_wrap);
        assert!(!p.trim_on_save, "a bad value is the default");
        assert_eq!(p.font_family, "Fira Code");
        let p = Prefs::parse("font_size=abc\ntab_width=-3\nindent_size=x\nfont_size=nan\n");
        assert_eq!((p.font_pt, p.tab_width, p.indent_size), (14.0, 4, 4));
        assert_eq!(Prefs::parse("font_size=3").font_pt, 7.0);
        assert_eq!(Prefs::parse("font_size=inf").font_pt, 14.0);
        assert_eq!(Prefs::parse("font_size= 10.5 ").font_pt, 10.5);
        // The last of a repeated key wins; spaces around the key are ignored.
        assert_eq!(Prefs::parse("tab_width=2\n tab_width = 8\n").tab_width, 8);
        // A huge family name is dropped.
        let long = format!("font_family={}", "x".repeat(1000));
        assert_eq!(Prefs::parse(&long).font_family, "");
    }

    #[test]
    fn the_library_folder_is_empty_by_default_and_kept_when_set() {
        assert_eq!(Prefs::default().library_dir, "");
        assert_eq!(Prefs::parse("").library_dir, "");
        let p = Prefs::parse("[preferences]\nlibrary_dir=D:\\One%20Drive\\library\n");
        assert_eq!(p.library_dir, "D:\\One Drive\\library");
        assert_eq!(Prefs::parse(&p.serialize()).library_dir, p.library_dir);
        // Empty is written as empty, so the default is what comes back.
        let d = Prefs::default().serialize();
        assert!(d.contains("library_dir=\n"), "{d}");
        assert_eq!(Prefs::parse(&d).library_dir, "");
        // A path no one typed (too long, or over two lines) is the default again.
        let long = format!("library_dir={}", "x".repeat(3000));
        assert_eq!(Prefs::parse(&long).library_dir, "");
    }

    /// Issues #7 and #8: the Preview button's mode and the rich paste are kept, and a file
    /// without them (an older build's) reads as before: the pane, plain paste.
    #[test]
    fn the_preview_mode_and_rich_paste_are_kept_and_default_as_before() {
        let d = Prefs::parse(
            "[preferences]
live_preview=1
",
        );
        assert_eq!((d.preview_mode, d.paste_rich), (PreviewMode::Window, false));
        for m in PreviewMode::ALL {
            let p = Prefs {
                preview_mode: m,
                paste_rich: true,
                ..Prefs::default()
            };
            let back = Prefs::parse(&p.serialize());
            assert_eq!((back.preview_mode, back.paste_rich), (m, true));
            assert_eq!(PreviewMode::from_index(m.index()), m);
        }
        let odd = Prefs::parse(
            "[preferences]
preview_mode=bogus
paste_rich=maybe
",
        );
        assert_eq!(
            (odd.preview_mode, odd.paste_rich),
            (PreviewMode::Window, false)
        );
        assert_eq!(
            Prefs::parse(
                "[preferences]
preview_mode= MDVIEW 
"
            )
            .preview_mode,
            PreviewMode::Viewer
        );
    }

    /// Pictures plan section 12: Write notes' options are the plan's defaults, read back as
    /// they were written, and a value that names no number of notes is the default.
    #[test]
    fn write_notes_options_read_back() {
        use crate::notes_ai::HowMany;
        let d = Prefs::default().ai_notes;
        assert_eq!(d.how_many, HowMany::AsNeeded);
        assert!(d.copy && d.rename && d.describe && d.strip);
        assert!(!d.text_out && !d.larger);
        assert_eq!(d.key(), "as-needed copy rename describe strip");
        let mine = NotesOptions {
            how_many: HowMany::One,
            copy: false,
            text_out: true,
            larger: true,
            ..NotesOptions::default()
        };
        let q = Prefs {
            ai_notes: mine,
            ..Prefs::default()
        };
        assert_eq!(Prefs::parse(&q.serialize()).ai_notes, mine);
        assert_eq!(
            Prefs::parse("[preferences]\nai_notes=copy strip\n").ai_notes,
            d
        );
        assert_eq!(
            Prefs::parse("[preferences]\nai_notes=per-picture, wobble\n")
                .ai_notes
                .how_many,
            HowMany::PerPicture
        );
    }

    /// D-209: File them copies the files in unless the owner chose to link them, and the
    /// choice reads back.
    #[test]
    fn the_files_choice_is_copy_by_default_and_reads_back() {
        assert_eq!(Prefs::default().ai_files, FilesGo::Copy);
        assert_eq!(
            Prefs::parse("[preferences]\nai_files=Link\n").ai_files,
            FilesGo::Link
        );
        assert_eq!(
            Prefs::parse("[preferences]\nai_files=fly\n").ai_files,
            FilesGo::Copy
        );
        let q = Prefs {
            ai_files: FilesGo::Link,
            ..Prefs::default()
        };
        assert_eq!(Prefs::parse(&q.serialize()).ai_files, FilesGo::Link);
    }

    /// D-211 and D-203: what opens after a run, and the task window's limits, are read back as
    /// written, put in their ranges, and a value that is not one is the default.
    #[test]
    fn the_after_run_choice_and_the_task_limits_read_back_in_their_ranges() {
        let text = "[preferences]\nai_after_run=open\nai_task_batch=999\nai_rewrite_files=0\n\
                    ai_rewrite_kb=abc\n";
        let p = Prefs::parse(text);
        assert_eq!(p.ai_after_run, AfterRun::Open);
        assert_eq!(p.ai_task_batch, AI_TASK_BATCH_MAX);
        assert_eq!(p.ai_rewrite_files, AI_REWRITE_FILES_MIN);
        assert_eq!(p.ai_rewrite_kb, AI_REWRITE_KB_DEFAULT);
        let odd = Prefs::parse("[preferences]\nai_after_run=sometimes\n");
        assert_eq!(odd.ai_after_run, AfterRun::Preview, "the default");
        let mut q = Prefs {
            ai_after_run: AfterRun::Nothing,
            ai_task_batch: 7,
            ai_rewrite_files: 3,
            ai_rewrite_kb: 50,
            ..Prefs::default()
        };
        let back = Prefs::parse(&q.serialize());
        assert_eq!(back.ai_after_run, AfterRun::Nothing);
        assert_eq!(
            (
                back.ai_task_batch,
                back.ai_rewrite_files,
                back.ai_rewrite_kb
            ),
            (7, 3, 50)
        );
        q.ai_rewrite_kb = 1;
        q.clamp();
        assert_eq!(q.ai_rewrite_kb, AI_REWRITE_KB_MIN);
    }

    #[test]
    fn sections_are_respected() {
        let text =
            "[mdedit]\ntab_width=9\nwrap=1\n[preferences]\ntab_width=3\n[other]\ntab_width=7\n";
        assert_eq!(Prefs::parse(text).tab_width, 3);
        let none = "[mdedit]\ntab_width=9\n";
        assert_eq!(Prefs::parse(none).tab_width, 4);
        // Written beside a session in one file, the keys do not collide with the session's.
        let both = format!(
            "[mdedit]\nminimap=1\nzoom=120\n{}",
            Prefs::default().serialize()
        );
        assert!(!Prefs::parse(&both).minimap);
    }

    /// The user: more than one custom provider, each with its own name, URL, model and effort;
    /// a file from before the list gives its one custom provider to the list.
    #[test]
    fn custom_providers_are_a_list_and_the_old_single_one_joins_it() {
        let old = "[preferences]\nai_connection=openai\nai_preset=custom\n\
                   ai_base_url=http://localhost:1234/v1\nai_model_openai=qwen\nai_effort_openai=low\n";
        let mut p = Prefs::parse(old);
        assert_eq!(
            p.ai_customs,
            [CustomProvider {
                name: "Custom".into(),
                base_url: "http://localhost:1234/v1".into(),
                model: "qwen".into(),
                effort: "low".into(),
            }]
        );
        assert_eq!(p.ai_url(), "http://localhost:1234/v1");
        assert_eq!(p.ai_model(Connection::OpenAi), "qwen");
        // D-080: three at once by default, one for a local model, and the owner's number kept
        // through the file.
        assert_eq!(p.ai_parallel(Connection::ClaudeApi), 3);
        p.ai_preset = "lmstudio".into();
        assert_eq!(p.ai_parallel(Connection::OpenAi), 1);
        p.ai_preset = "openai".into();
        p.set_ai_parallel(Connection::ClaudeApi, 5);
        assert_eq!(p.ai_parallel(Connection::ClaudeApi), 5);
        let text = p.serialize();
        assert!(text.contains("ai_parallel_claude-api=5"), "{text}");
        let back = Prefs::parse(&text);
        assert_eq!(back.ai_parallel(Connection::ClaudeApi), 5);
        p.set_ai_parallel(Connection::ClaudeApi, 99);
        assert_eq!(p.ai_parallel(Connection::ClaudeApi), AI_PARALLEL_MAX);
        // Written back it is the list, and the chosen one's URL for a build before the list.
        let t = p.serialize();
        assert!(t.contains("ai_custom_0_url=http://localhost:1234/v1\n"));
        assert!(t.contains("ai_base_url=http://localhost:1234/v1\n"));
        assert_eq!(Prefs::parse(&t), p);
        // An old file with another provider chosen keeps its custom URL as a provider.
        let p = Prefs::parse("ai_preset=groq\nai_base_url=http://x/v1\n");
        assert_eq!(p.ai_preset, "groq");
        assert_eq!(p.ai_customs.len(), 1);
        assert_eq!(p.ai_customs[0].base_url, "http://x/v1");
        // No URL and another provider: no custom provider.
        assert!(Prefs::parse("ai_preset=groq\n").ai_customs.is_empty());

        // New, then a build before the list (it keeps only the keys it knows), then new again:
        // the chosen provider comes back.
        let mut two = Prefs::parse(old);
        two.ai_customs.push(CustomProvider {
            name: "Box".into(),
            base_url: "http://box:11434/v1".into(),
            ..CustomProvider::default()
        });
        two.ai_custom = 1;
        let by_old: String = two
            .serialize()
            .lines()
            .filter(|l| !l.starts_with("ai_custom"))
            .map(|l| format!("{l}\n"))
            .collect();
        let back = Prefs::parse(&by_old);
        assert_eq!(back.ai_preset, "custom");
        assert_eq!(back.ai_url(), "http://box:11434/v1");

        // Two of them: each its own model; the chosen one is the one used.
        let mut p = Prefs::parse(old);
        p.ai_customs.push(CustomProvider {
            name: "Ollama box".into(),
            base_url: "http://box:11434/v1".into(),
            ..CustomProvider::default()
        });
        p.ai_custom = 1;
        assert_eq!(p.ai_model(Connection::OpenAi), "");
        p.set_ai_model(Connection::OpenAi, "llama");
        assert_eq!(p.ai_customs[1].model, "llama");
        assert_eq!(p.ai_customs[0].model, "qwen", "the other keeps its own");
        let back = Prefs::parse(&p.serialize());
        assert_eq!(back, p);
        assert_eq!(back.ai_url(), "http://box:11434/v1");
        // A preset's model is not a custom provider's.
        p.ai_preset = "groq".into();
        p.set_ai_model(Connection::OpenAi, "mixtral");
        assert_eq!(p.ai_model(Connection::OpenAi), "mixtral");
        assert_eq!(p.ai_customs[1].model, "llama");
        // Gaps in the numbers close, and the chosen one stays chosen; a chosen custom that is
        // gone gives way to the default provider.
        let gaps = Prefs::parse(
            "ai_preset=custom\nai_custom=5\nai_custom_2_name=A\nai_custom_5_name=B\nai_custom_5_url=u\n",
        );
        assert_eq!(gaps.ai_customs.len(), 2);
        assert_eq!((gaps.ai_custom, gaps.ai_url()), (1, "u"));
        let gone = Prefs {
            ai_preset: "custom".into(),
            ..Prefs::default()
        };
        assert_eq!(Prefs::parse(&gone.serialize()).ai_preset, "openai");
    }

    #[test]
    fn round_trip_is_stable() {
        let p = Prefs {
            font_family: "Cascadia Code, %weird\\".into(),
            font_pt: 11.5,
            tab_width: 8,
            detect_indent: false,
            indent_with_spaces: true,
            indent_size: 2,
            auto_pair: false,
            auto_indent: false,
            trim_on_save: true,
            final_newline_on_save: true,
            line_numbers: false,
            highlight_line: false,
            animations: false,
            minimap: true,
            minimap_width: 150,
            word_wrap: true,
            restore_session: false,
            start_on_pinned: true,
            offer_stale: false,
            stale_days: 30,
            skip_brief: true,
            brief_secs: 45,
            brief_last: true,
            live_preview: false,
            preview_mode: PreviewMode::SideBySide,
            paste_rich: true,
            complete_auto: true,
            backdrop: Backdrop::Acrylic,
            ai_connection: Connection::OpenAi,
            ai_review: false,
            ai_review_preview: true,
            ai_after_run: AfterRun::Nothing,
            ai_files: FilesGo::Link,
            ai_notes: NotesOptions {
                how_many: crate::notes_ai::HowMany::PerPicture,
                ..NotesOptions::default()
            },
            ai_pictures_told: true,
            ai_task_batch: 12,
            ai_rewrite_files: 7,
            ai_rewrite_kb: 640,
            ai_models: [
                "opus".into(),
                "".into(),
                "claude-fable-5-2".into(),
                "llama 4".into(),
            ],
            ai_efforts: ["max".into(), "minimal".into(), "".into(), "low".into()],
            ai_parallel: ["2".into(), "".into(), "5".into(), "".into()],
            ai_preset: "custom".into(),
            ai_customs: vec![
                CustomProvider {
                    name: "Work = server".into(),
                    base_url: "http://h:8080/v1?x=%1".into(),
                    model: "qwen 3".into(),
                    effort: "low".into(),
                },
                CustomProvider {
                    name: "LM Studio".into(),
                    base_url: "http://localhost:1234/v1".into(),
                    ..CustomProvider::default()
                },
            ],
            ai_custom: 1,
            ai_claude_path: "C:\\Program Files\\claude.exe".into(),
            ai_codex_path: "/opt/codex = 2".into(),
            library_dir: "D:\\Notes = 1\\library".into(),
        };
        let t = p.serialize();
        assert_eq!(Prefs::parse(&t), p);
        assert_eq!(Prefs::parse(&t).serialize(), t);
        let d = Prefs::default().serialize();
        assert_eq!(Prefs::parse(&d), Prefs::default());
        assert_eq!(Prefs::parse(&d).serialize(), d);
        // Out-of-range values are written clamped; every line is one line.
        let wild = Prefs {
            font_pt: f32::NAN,
            tab_width: 99,
            minimap_width: 5,
            font_family: "a\nb".into(),
            ai_preset: "custom".into(),
            ai_customs: vec![CustomProvider {
                name: "x".into(),
                base_url: "a\nb".into(),
                ..CustomProvider::default()
            }],
            ..Prefs::default()
        };
        let t = wild.serialize();
        assert_eq!(t.lines().count(), 61);
        assert_eq!(Prefs::parse(&t).ai_url(), "");
        assert_eq!(
            Prefs::parse("ai_preset=nope\nai_connection=x\n").ai_preset,
            "openai"
        );
        let r = Prefs::parse(&t);
        assert_eq!(
            (r.font_pt, r.tab_width, r.font_family.as_str()),
            (14.0, 16, "")
        );
        assert_eq!(r.minimap_width, MINIMAP_WIDTH_MIN);
        assert_eq!(
            Prefs::parse("[preferences]\nminimap_width=9999\n").minimap_width,
            MINIMAP_WIDTH_MAX
        );
        assert_eq!(
            Prefs::parse("[preferences]\nminimap_width=120\n").minimap_width,
            120
        );
        // The days before a tab is offered: kept in range, a bad value is the default.
        let days = |v: &str| Prefs::parse(&format!("[preferences]\nstale_days={v}\n")).stale_days;
        assert_eq!(
            (days("14"), days("0"), days("9999"), days("x")),
            (14, STALE_DAYS_MIN, STALE_DAYS_MAX, STALE_DAYS_DEFAULT)
        );
        // The seconds before a tab is reopened: the same.
        let secs = |v: &str| Prefs::parse(&format!("[preferences]\nbrief_secs={v}\n")).brief_secs;
        assert_eq!(
            (secs("30"), secs("0"), secs("9999"), secs("x")),
            (30, BRIEF_SECS_MIN, BRIEF_SECS_MAX, BRIEF_SECS_DEFAULT)
        );
        let on = Prefs::parse("[preferences]\nskip_brief=1\nbrief_secs=25\n");
        assert!(on.skip_brief);
        let back = Prefs::parse(&on.serialize());
        assert!(back.skip_brief);
        assert_eq!(back.brief_secs, 25);
        // Random fractions survive.
        let mut rng = Rng::new(9);
        for _ in 0..200 {
            let p = Prefs {
                font_pt: 7.0 + rng.below(2500) as f32 / 100.0,
                ..Prefs::default()
            };
            let q = Prefs::parse(&p.serialize());
            assert_eq!(q.font_pt, p.font_pt);
        }
    }

    #[test]
    fn hostile_text_never_panics() {
        let mut rng = Rng::new(2);
        let pieces = [
            "font_size",
            "=",
            "\n",
            "[",
            "]",
            "preferences",
            "%",
            "%zz",
            "é",
            "tab_width",
            "9999999999999999999999",
            "-",
            "\r\n",
            "nan",
        ];
        for _ in 0..2000 {
            let t: String = (0..rng.below(20)).map(|_| *rng.pick(&pieces)).collect();
            let p = Prefs::parse(&t);
            assert_eq!(Prefs::parse(&p.serialize()), p, "{t:?}");
        }
    }

    #[test]
    fn indent_and_assist_follow_the_preferences() {
        let mut p = Prefs::default();
        assert_eq!(p.indent(), Indent::Tab);
        p.indent_with_spaces = true;
        p.indent_size = 2;
        p.tab_width = 8;
        p.auto_pair = false;
        let a = p.assist("python");
        assert_eq!(a.indent, Indent::Spaces(2));
        assert_eq!(a.tab_width, 8);
        assert!(!a.auto_pair && a.auto_indent && a.colon_blocks);
        // A detected unit wins while detection is on, and only then.
        let a = p.assist_for("python", Some(Indent::Tab));
        assert_eq!(a.indent, Indent::Tab);
        p.detect_indent = false;
        assert_eq!(
            p.assist_for("python", Some(Indent::Tab)).indent,
            Indent::Spaces(2)
        );
        p.detect_indent = true;
        assert_eq!(p.assist_for("python", None).indent, Indent::Spaces(2));
    }

    fn save(text: &str, sel: Selection, trim: bool, nl: bool, eol: &str) -> (String, Outcome) {
        let mut buf = Buffer::from_text(text);
        let mut hist = History::new();
        let prefs = Prefs {
            trim_on_save: trim,
            final_newline_on_save: nl,
            ..Prefs::default()
        };
        let o = apply_on_save(&mut buf, &mut hist, sel, &prefs, eol);
        let after = buf.rope().to_string();
        match &o.change {
            None => {
                assert_eq!(after, text);
                assert!(!hist.can_undo());
                assert_eq!(o.sel, sel);
            }
            Some(c) => {
                check_change(text, &after, c);
                let mut counts = EolCounts::of(text);
                check_eols(&mut counts, &after, c);
                assert_eq!(hist.steps(), (1, 0), "one undo step");
                undo(&mut buf, &mut hist).unwrap();
                assert_eq!(buf.rope().to_string(), text);
                redo(&mut buf, &mut hist).unwrap();
                assert_eq!(buf.rope().to_string(), after);
            }
        }
        (after, o)
    }

    #[test]
    fn save_options() {
        let s = Selection::at(0);
        assert_eq!(save("a  \nb\t\n", s, true, false, "\n").0, "a\nb\n");
        assert_eq!(save("a\nb", s, false, true, "\n").0, "a\nb\n");
        assert_eq!(save("a \r\nb ", s, true, true, "\r\n").0, "a\r\nb\r\n");
        assert_eq!(save("a\rb", s, false, true, "\r").0, "a\rb\r");
        // Odd eol: LF. Empty document: stays empty. Already fine: nothing.
        assert_eq!(save("a", s, false, true, "xx").0, "a\n");
        assert_eq!(save("", s, true, true, "\n").0, "");
        assert_eq!(save("a\n", s, true, true, "\n").0, "a\n");
        assert_eq!(save("a  ", s, false, true, "\n").0, "a  \n");
        // Both off: nothing, whatever the text.
        assert_eq!(save("a  \nb", s, false, false, "\n").0, "a  \nb");
        // A last line of blanks after a break is trimmed, and the break stays the last thing.
        assert_eq!(save("a\n   ", s, true, true, "\n").0, "a\n");
        // The selection follows the text.
        let (_, o) = save(
            "a  \nb",
            Selection {
                anchor: 5,
                caret: 2,
                goal_col: None,
            },
            true,
            true,
            "\n",
        );
        assert_eq!((o.sel.anchor, o.sel.caret), (3, 1));
        // A caret at the very end stays before the added break.
        let (t, o) = save("ab", Selection::at(2), false, true, "\n");
        assert_eq!((t.as_str(), o.sel), ("ab\n", Selection::at(2)));
    }

    #[test]
    fn save_options_on_random_text() {
        let mut rng = Rng::new(4);
        let pieces = ["a", " ", "\t", "\n", "\r\n", "\r", "é"];
        for _ in 0..500 {
            let text: String = (0..rng.below(14)).map(|_| *rng.pick(&pieces)).collect();
            let (trim, nl) = (rng.chance(60), rng.chance(60));
            let (t, _) = save(
                &text,
                Selection::at(rng.below(text.len() + 1)),
                trim,
                nl,
                "\n",
            );
            if nl && !t.is_empty() {
                assert!(t.ends_with(['\n', '\r']));
            }
            if trim {
                // No blanks before a line end, except that a lone CR then LF cannot appear.
                assert!(!t.contains(" \n") && !t.contains("\t\n") && !t.contains(" \r"));
            }
        }
    }
}
