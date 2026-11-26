//! "Add a folder to the library…" (`docs/ai-library-plan.md` 5.7, phase 4a): the walk of a folder,
//! the plan of links it becomes, and the text of the group's file. No AI, and nothing is copied,
//! moved or changed: an import writes lines in one library file, and the files stay where they
//! are.
//!
//! The walk is here rather than in the window so that every rule of it is a test without a screen:
//! which files are taken (`.md`, `.markdown`, `.txt`, and the folders the Files pane leaves out),
//! what a note's sections are (the subfolders, three levels at most), and what its title is (the
//! file's first heading, else its name). The card is `import_ui.rs`; the write is
//! `app/importing.rs`.
//!
//! Nothing here can panic on what a folder holds: every name is read lossily, every file is read
//! through a length cap, and a heading is looked for at a character boundary.

use mdshared_app::library::{self, Entry, tree};
use mdshared_app::paths::{comparable, same_path};
use mdshared_app::runs::Link;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The extensions the walk takes (5.7).
pub const EXTENSIONS: [&str; 3] = ["md", "markdown", "txt"];

/// The deepest subfolder level a note is filed under: deeper folders are folded into the third.
pub const MAX_SECTIONS: usize = 3;

/// The most of a file read to find its heading and the lines a row shows.
pub const HEAD_BYTES: u64 = 8 * 1024;

/// The most lines of a file a row's preview holds.
pub const PREVIEW_LINES: usize = 8;

/// The most entries a walk reads, files and folders together, before it stops: a drive or a
/// home folder with few text files in it is not read to its end (audit 4, C8).
pub const MAX_READ: usize = 50_000;

/// The deepest folder a walk goes into below the folder picked, so that no folder can take the
/// walk's thread to the end of its stack (audit 4, C12).
pub const MAX_DEPTH: usize = 64;

/// The most characters of a note's title as a library line writes it (audit 4, C7).
pub const TITLE_MAX: usize = 200;

/// One file the walk took.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub path: PathBuf,
    /// The subfolders it is in, relative to the folder picked, at most [`MAX_SECTIONS`].
    pub sections: Vec<String>,
    /// The first heading of the file, else its own name without the extension.
    pub title: String,
    /// The first lines of the file, for the card's preview.
    pub head: Vec<String>,
    /// The start of the file, for an AI request: at most [`import_ai::DIGEST_MAX`] bytes.
    pub start: String,
    /// Its size in bytes, and the day it was last changed, as an AI request says them.
    pub bytes: u64,
    pub modified: String,
}

/// What a walk found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Walk {
    /// The files, in the order the folder gives them: the folder's own first, then each subfolder
    /// by name, case-insensitively.
    pub found: Vec<Found>,
    /// How many folders below the picked one hold at least one file, the picked one included.
    pub folders: usize,
    /// The folder holds more files than the plan keeps: the walk stopped at the cap.
    pub more: bool,
    /// The walk stopped before the end of the folder at one of its own bounds; a plan of what
    /// it read would be part of a folder, and is refused.
    pub stopped: Option<Stop>,
    /// How many entries of folders the walk read, files and folders together.
    pub read: usize,
    /// The library folder is inside the folder walked, and was left out: its files are the
    /// library's own, not notes (audit 4, C5).
    pub library_left_out: bool,
    /// Files the walk would have taken whose line in a library file would not read back as
    /// them: a name with `%20` in it, which the reader takes for a space. They are left out, so
    /// the rest of the folder can go in, and the card names them (audit 4, C6).
    pub unlinkable: Vec<PathBuf>,
    /// Folders the walk could not read at all: a permission refused, a folder gone between the
    /// listing and the read. Each is named, so a folder that adds nothing can say why (D-250).
    pub unreadable: Vec<PathBuf>,
}

/// Why a walk stopped before the end of its folder, besides the 2,000.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// It read [`MAX_READ`] entries.
    Entries,
    /// A folder is more than [`MAX_DEPTH`] folders deep.
    Depth,
}

/// The bounds of one walk.
#[derive(Clone, Copy, Debug)]
struct Limits {
    files: usize,
    read: usize,
    depth: usize,
}

const LIMITS: Limits = Limits {
    files: library::MAX_ENTRIES,
    read: MAX_READ,
    depth: MAX_DEPTH,
};

/// Walks `folder` for the files an import takes: the Files pane's rules for what to leave out (a
/// dot name, the hidden attribute, `target`, `node_modules`), links to folders not followed, and
/// [`EXTENSIONS`] only. `cancel` is looked at before every folder and every file, so a large
/// folder can be given up on.
pub fn walk(folder: &Path, cancel: &dyn Fn() -> bool) -> Walk {
    walk_with(folder, None, LIMITS, cancel)
}

/// [`walk`] for the library in `library`: the library folder is left out when the folder walked
/// holds it, and a file whose line the library would not read back as that file is left out and
/// named in [`Walk::unlinkable`].
pub fn walk_for(folder: &Path, library: &Path, cancel: &dyn Fn() -> bool) -> Walk {
    walk_with(folder, Some(library), LIMITS, cancel)
}

/// What a walk for the AI task window takes: every file in the folder, whatever its kind, and
/// nothing of it read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Every {
    /// The files, in [`walk`]'s order: the folder's own first, then each subfolder's by name,
    /// case-insensitively.
    pub files: Vec<PathBuf>,
    /// The folder holds more files the walk takes than it was allowed to take: it stopped at the
    /// cap. Only files [`EveryWalk::wanted`] takes are counted.
    pub more: bool,
    /// The walk stopped before the end of the folder, at the entries read or the depth.
    pub stopped: Option<Stop>,
    /// The folders the walk could not read at all (D-250).
    pub unreadable: Vec<PathBuf>,
}

/// What a walk for the task window is told before it starts: its room, the library, and which
/// files are new to the window.
pub struct EveryWalk<'a> {
    /// The most files to take: files [`EveryWalk::wanted`] turns away are not counted, so this is
    /// the window's room for new files.
    pub files: usize,
    /// The library folder, left out when the folder walked holds it, as the import does (audit 4,
    /// C5).
    pub library: Option<&'a Path>,
    /// Whether a file is to be taken: the task window takes the ones it holds already out, so the
    /// cap counts only the files an add brings.
    pub wanted: &'a dyn Fn(&Path) -> bool,
}

/// Walks `folder` for every file in it, whatever its kind, for the task window's "Add": the
/// Files pane's rules for what to leave out (a dot name, the hidden attribute, `target`,
/// `node_modules`), links to folders not followed, the library folder left out, and at most
/// [`EveryWalk::files`] of what [`EveryWalk::wanted`] takes, [`MAX_READ`] entries read and
/// [`MAX_DEPTH`] folders deep. No file is read here: the window probes each of what comes back
/// itself.
pub fn walk_every(folder: &Path, walk: EveryWalk<'_>, cancel: &dyn Fn() -> bool) -> Every {
    let sorted = walk_sorted(
        folder,
        walk.library,
        Limits {
            files: walk.files,
            ..LIMITS
        },
        Sort::Every,
        walk.wanted,
        cancel,
    );
    every_of(sorted)
}

/// A [`Walk`] as a walk for the task window says it: its files, and whether it was cut short.
fn every_of(walk: Walk) -> Every {
    Every {
        files: walk.found.into_iter().map(|f| f.path).collect(),
        more: walk.more,
        stopped: walk.stopped,
        unreadable: walk.unreadable,
    }
}

/// What a walk takes of each folder it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sort {
    /// The library import: [`EXTENSIONS`] only, each file read for its first heading and preview.
    Notes,
    /// The AI task window: every file, whatever its kind, and none read: a [`Found`]'s `title`,
    /// `head` and `start` stay empty.
    Every,
}

fn walk_with(
    folder: &Path,
    library: Option<&Path>,
    limits: Limits,
    cancel: &dyn Fn() -> bool,
) -> Walk {
    walk_sorted(folder, library, limits, Sort::Notes, &|_| true, cancel)
}

fn walk_sorted(
    folder: &Path,
    library: Option<&Path>,
    limits: Limits,
    sort: Sort,
    wanted: &dyn Fn(&Path) -> bool,
    cancel: &dyn Fn() -> bool,
) -> Walk {
    let walker = Walker {
        limits,
        sort,
        wanted,
        cancel,
        library: library.map(|dir| LibraryDir {
            dir: dir.to_path_buf(),
            name: dir
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default(),
            real: comparable(dir),
        }),
    };
    let mut out = Walk::default();
    walker.dir(folder, &mut Vec::new(), 0, &mut out);
    out
}

/// What a walk knows of the library it reads a folder for.
struct LibraryDir {
    dir: PathBuf,
    /// Its own name, in lower case, so that a folder of another name costs no look at the disk.
    name: String,
    /// Its path as the disk has it.
    real: PathBuf,
}

struct Walker<'a> {
    limits: Limits,
    sort: Sort,
    wanted: &'a dyn Fn(&Path) -> bool,
    cancel: &'a dyn Fn() -> bool,
    library: Option<LibraryDir>,
}

impl Walker<'_> {
    /// Whether the folder `name` at `path` is the library folder.
    fn is_library(&self, name: &str, path: &Path) -> bool {
        self.library.as_ref().is_some_and(|lib| {
            !lib.name.is_empty()
                && name.to_lowercase() == lib.name
                && same_path(&comparable(path), &lib.real)
        })
    }

    /// Whether the line a library file would list `path` on, under `title`, reads back as that
    /// file and that title; always, for a walk with no library.
    fn links_back(&self, path: &Path, title: &str) -> bool {
        self.library
            .as_ref()
            .is_none_or(|lib| reads_back_as_link(&lib.dir, path, title))
    }

    fn dir(&self, dir: &Path, sections: &mut Vec<String>, depth: usize, out: &mut Walk) {
        if (self.cancel)() || out.more || out.stopped.is_some() {
            return;
        }
        if depth > self.limits.depth {
            out.stopped = Some(Stop::Depth);
            return;
        }
        self.list(dir, sections, depth, out);
    }

    fn list(&self, dir: &Path, sections: &mut Vec<String>, depth: usize, out: &mut Walk) {
        let cancel = self.cancel;
        let Ok(read) = std::fs::read_dir(dir) else {
            // Nothing of it could be read: the folder is named, so an add that finds nothing can
            // say why (D-250).
            out.unreadable.push(dir.to_path_buf());
            return;
        };
        let mut dirs: Vec<(String, PathBuf)> = Vec::new();
        let mut files: Vec<(String, PathBuf, u64, String)> = Vec::new();
        for entry in read.flatten() {
            if cancel() {
                return;
            }
            out.read += 1;
            if out.read > self.limits.read {
                out.stopped = Some(Stop::Entries);
                return;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            // A link is read as the file it points at, and a link to a folder is not followed,
            // so the walk cannot loop (the Files pane's rule).
            let meta = match if kind.is_symlink() {
                std::fs::metadata(&path)
            } else {
                entry.metadata()
            } {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            if (kind.is_symlink() && meta.is_dir())
                || mdshared_app::notes::skips_entry(&name, meta.is_dir(), &meta)
            {
                continue;
            }
            if meta.is_dir() {
                if self.is_library(&name, &path) {
                    out.library_left_out = true;
                } else {
                    dirs.push((name, path));
                }
            } else if meta.is_file() && (self.sort == Sort::Every || takes(&path)) {
                let modified = match self.sort {
                    Sort::Notes => modified_day(meta.modified().ok()),
                    Sort::Every => String::new(),
                };
                files.push((name, path, meta.len(), modified));
            }
        }
        let key = |name: &str| (name.to_lowercase(), name.to_string());
        dirs.sort_by_key(|d| key(&d.0));
        files.sort_by_key(|f| key(&f.0));
        if !files.is_empty() {
            out.folders += 1;
        }
        for (_, path, bytes, modified) in files {
            // A walk for the task window takes only the files it was told are new; the others take
            // no room from its cap.
            if self.sort == Sort::Every && !(self.wanted)(&path) {
                continue;
            }
            if out.found.len() >= self.limits.files {
                out.more = true;
                return;
            }
            if cancel() {
                return;
            }
            if self.sort == Sort::Every {
                out.found.push(Found {
                    sections: Vec::new(),
                    path,
                    title: String::new(),
                    head: Vec::new(),
                    start: String::new(),
                    bytes,
                    modified,
                });
                continue;
            }
            let head = read_head(&path);
            let title = title_of(&head)
                .or_else(|| clean_title(&stem_of(&path)))
                .unwrap_or_else(|| "Untitled".to_string());
            if !self.links_back(&path, &title) {
                out.unlinkable.push(path);
                continue;
            }
            out.found.push(Found {
                sections: sections.clone(),
                title,
                head: preview_of(&head),
                start: cut_bytes(&head, crate::import_ai::DIGEST_MAX),
                bytes,
                modified,
                path,
            });
        }
        for (name, path) in dirs {
            if out.more || out.stopped.is_some() {
                return;
            }
            // A note is filed under the subfolders three deep; a folder deeper than that is
            // folded into the third, and its files sit under it.
            if sections.len() < MAX_SECTIONS {
                sections.push(name);
                self.dir(&path, sections, depth + 1, out);
                sections.pop();
            } else {
                self.dir(&path, sections, depth + 1, out);
            }
        }
    }
}

/// Whether the line a library file in `library` would list `path` on, under `title`, reads
/// back as that file and that title, as the write's own check will want it to (audit 4, C6).
/// It does not for a name with `%20` in it, which the reader takes for a space; nor, on Linux,
/// for a name with a line break in it.
pub fn reads_back_as_link(library: &Path, path: &Path, title: &str) -> bool {
    let link = tree::link_path(library, path);
    let title = title.trim().replace(['[', ']'], "");
    let line = library::link_line(&title, &link);
    let parsed = library::parse(&line, library);
    let [entry] = parsed.entries.as_slice() else {
        return false;
    };
    same_path(&entry.path, &resolved(&link, library)) && entry.title.trim() == title.trim()
}

/// A link's path as a library file resolves it: from the library folder when it has no root,
/// with the separators the rest of the program spells paths with (the shared reader's rule).
fn resolved(link: &Path, base: &Path) -> PathBuf {
    let written = link.to_string_lossy();
    let path = if cfg!(windows) {
        PathBuf::from(written.replace('/', "\\"))
    } else {
        PathBuf::from(written.as_ref())
    };
    if path.is_absolute() || base.as_os_str().is_empty() {
        path
    } else {
        base.join(path)
    }
}

/// Whether a file is one the walk takes: a name ending in one of [`EXTENSIONS`], in any case.
fn takes(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| EXTENSIONS.contains(&e.as_str()))
}

/// The first [`HEAD_BYTES`] of a file as text. A file that cannot be read gives none: the note is
/// still listed, by its name, as a note missing on disk is.
fn read_head(path: &Path) -> String {
    let Ok(file) = std::fs::File::open(path) else {
        return String::new();
    };
    let mut bytes = Vec::new();
    if file.take(HEAD_BYTES).read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The first `max` bytes of `text`, cut at a character.
fn cut_bytes(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.get(..end).unwrap_or_default().to_string()
}

/// The day a file was last changed, as `2026-11-23`, for a request's digest; empty when the
/// file system does not say. In UTC, as the modification time's own clock.
fn modified_day(time: Option<std::time::SystemTime>) -> String {
    let Some(since) = time.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()) else {
        return String::new();
    };
    let (y, m, d) = civil_from_days((since.as_secs() / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The year, month and day of a count of days since 1970-01-01 (Howard Hinnant's civil_from_days,
/// which needs no crate).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// The text of the file's first heading, at any level, outside a code fence; `None` when it has
/// none. A heading of one line only: a setext heading (`Title` underlined) is not read, as the
/// library's own title reader does not read it.
pub fn title_of(head: &str) -> Option<String> {
    let mut fence: Option<String> = None;
    for line in text_lines(head) {
        let trimmed = line.trim_start();
        match (&fence, fence_marker(trimmed)) {
            (None, Some(marker)) => {
                fence = Some(marker);
                continue;
            }
            (Some(open), Some(marker)) => {
                if marker.len() >= open.len() && marker.starts_with(open.as_str()) {
                    fence = None;
                }
                continue;
            }
            (Some(_), None) => continue,
            (None, None) => {}
        }
        if let Some(text) = heading_text(trimmed).and_then(|t| clean_title(&t)) {
            return Some(text);
        }
    }
    None
}

/// The lines of `text`, a bare CR ending one as LF and CRLF do: an old Mac file is many lines,
/// not one (audit 4, C7).
fn text_lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n')
        .flat_map(|line| line.strip_suffix('\r').unwrap_or(line).split('\r'))
}

/// A title as one line of a library file can hold it: no control character, at most
/// [`TITLE_MAX`] characters, and `None` when nothing is left.
fn clean_title(text: &str) -> Option<String> {
    let plain: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let plain = plain.trim();
    if plain.is_empty() {
        return None;
    }
    let cut: String = plain.chars().take(TITLE_MAX).collect();
    Some(cut.trim_end().to_string())
}

/// The three or more backticks or tildes that open or close a code fence.
fn fence_marker(line: &str) -> Option<String> {
    let first = line.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let marker: String = line.chars().take_while(|&c| c == first).collect();
    (marker.len() >= 3).then_some(marker)
}

/// A `#` heading's text, with its `#` signs and a closing run of them taken off.
fn heading_text(line: &str) -> Option<String> {
    let rest = line.strip_prefix('#')?;
    let more = rest.len() - rest.trim_start_matches('#').len();
    let after = &rest[more..];
    // `#hashtag` is not a heading; `#` alone is an empty one.
    if !after.is_empty() && !after.starts_with(' ') && !after.starts_with('\t') {
        return None;
    }
    Some(after.trim().trim_end_matches('#').trim().to_string())
}

/// A file's name without its extension, for a file with no heading.
fn stem_of(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            path.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

/// The lines of a row's preview: the file's first lines, with blank ones at either end dropped,
/// at most [`PREVIEW_LINES`].
fn preview_of(head: &str) -> Vec<String> {
    let mut lines: Vec<String> = text_lines(head)
        .take(PREVIEW_LINES)
        .map(str::to_string)
        .collect();
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    lines
}

// ---- the plan ------------------------------------------------------------------------------------

/// One note the plan would add: a file of the owner's, listed by one line.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    /// Where the file is, as it is on disk.
    pub path: PathBuf,
    /// The headings it is written under: the subfolders, at most [`MAX_SECTIONS`]; after an AI
    /// sort, one of the AI's section paths.
    pub sections: Vec<String>,
    pub title: String,
    pub head: Vec<String>,
    /// The start of the file, as an AI request sends it.
    pub start: String,
    pub bytes: u64,
    pub modified: String,
    pub ticked: bool,
    /// The group's file lists this file already: its row says so, and it cannot be ticked.
    pub already: bool,
    /// The AI's confidence in where it put the file, when the AI sorted it; `None` for a plan by
    /// folders.
    pub confidence: Option<f32>,
    /// The AI's one-line reason, shown dim and never written anywhere.
    pub why: String,
    /// The AI did not place this file: it sits under "Unsorted" and is not ticked.
    pub unsorted: bool,
}

/// What a row of the card is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// A subfolder's heading, which ticks everything under it.
    Section { path: Vec<String> },
    /// Note `i` of [`Plan::notes`].
    File(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    pub ticked: bool,
}

/// What the library already holds of the group the plan would write into.
pub struct Where<'a> {
    /// The library folder: a link to a file inside it is written relative to it.
    pub root: &'a Path,
    /// The group found by the folder's name, if the library has one: its name and library file.
    pub group: Option<(&'a str, &'a Path)>,
    /// That group's own folder, from its `home:` line, when it names one.
    pub group_home: Option<&'a Path>,
    /// That folder is neither the folder picked nor a folder that holds it: the group keeps its
    /// notes elsewhere, and the files cannot join it (audit 4, C4). The window decides it, as
    /// it needs the disk to.
    pub home_apart: bool,
    /// The library file a new group would be made at.
    pub new_file: PathBuf,
    /// The group's library file's notes, as it reads now.
    pub entries: usize,
    /// Every note the group lists, to tell a file the library has from one it has not.
    pub listed: &'a [PathBuf],
}

/// The plan a card shows: the rows, the ticks, and what would be written.
#[derive(Clone, Debug)]
pub struct Plan {
    /// The folder picked.
    pub folder: PathBuf,
    /// The group's name: the folder's own, or the one the library already has for it.
    pub group_name: String,
    pub library: PathBuf,
    /// The library file the links go in.
    pub group_file: PathBuf,
    /// The run would make the library file.
    pub group_new: bool,
    /// Where the group's notes are: for a new group the folder picked, which its `home:` line
    /// names; for a group the library has, the folder its own `home:` line names, and empty when
    /// it names none.
    pub home: PathBuf,
    pub notes: Vec<Note>,
    pub rows: Vec<Row>,
    /// The notes the group's file lists now, for the 2,000.
    pub entries: usize,
    /// How many folders hold the files, for the card's header.
    pub folders: usize,
    /// The folder holds more files than the plan can hold: it is refused with the count.
    pub more: bool,
    /// Nothing can be written, for this reason (a name the library cannot read back, a folder
    /// with no name).
    pub blocked: Option<String>,
}

/// Builds the plan of a walk over `folder`. Nothing is read from disk or written here.
pub fn plan(walked: Walk, folder: &Path, at: Where) -> Plan {
    let Walk {
        found,
        folders,
        more,
        stopped,
        ..
    } = walked;
    let (group_name, group_file, group_new) = match at.group {
        Some((name, file)) => (name.to_string(), file.to_path_buf(), false),
        None => {
            let name = folder
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            (name, at.new_file.clone(), true)
        }
    };
    let mut notes: Vec<Note> = found
        .into_iter()
        .map(|f| {
            // A note the group lists through `..` is the same note (audit 4, C12).
            let already = at.listed.iter().any(|p| tree::same_note(p, &f.path));
            Note {
                path: f.path,
                sections: f.sections,
                title: f.title,
                head: f.head,
                start: f.start,
                bytes: f.bytes,
                modified: f.modified,
                ticked: !already,
                already,
                confidence: None,
                why: String::new(),
                unsorted: false,
            }
        })
        .collect();
    // The card lists the folder's own notes first, then each folder's by name, as the walk does.
    notes.sort_by(|a, b| {
        a.sections
            .iter()
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>()
            .cmp(&b.sections.iter().map(|s| s.to_lowercase()).collect())
            .then_with(|| a.sections.cmp(&b.sections))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            .then_with(|| a.title.cmp(&b.title))
    });
    let rows = rows_of(&notes);
    let home = match (group_new, at.group_home) {
        (true, _) => folder.to_path_buf(),
        (false, Some(home)) => home.to_path_buf(),
        (false, None) => PathBuf::new(),
    };
    let mut plan = Plan {
        folder: folder.to_path_buf(),
        group_name,
        library: at.root.to_path_buf(),
        group_file,
        group_new,
        home,
        notes,
        rows,
        entries: at.entries,
        folders,
        more,
        blocked: None,
    };
    // A walk cut short is part of a folder; a group that keeps its notes in another folder is
    // not this folder's: either refuses before anything else is said.
    plan.blocked = stopped
        .map(stopped_reason)
        .or_else(|| {
            (!group_new && at.home_apart).then(|| home_apart_reason(&plan.group_name, &plan.home))
        })
        .or_else(|| plan.why_nothing_can_be_written());
    plan
}

/// What the card says of a walk that stopped at one of its bounds.
pub fn stopped_reason(stop: Stop) -> String {
    match stop {
        Stop::Entries => format!(
            "The folder holds more than {} files and folders, and reading it stopped there. Pick a folder inside it.",
            group_digits(MAX_READ)
        ),
        Stop::Depth => format!(
            "The folder has folders more than {MAX_DEPTH} deep, and reading it stopped there. Pick a folder inside it."
        ),
    }
}

/// What the card says when the library's group of the folder's name keeps its notes in another
/// folder, `home` (audit 4, C4; DESIGN 81: a link is refused that sits neither in the library
/// nor in the group's home).
pub fn home_apart_reason(group: &str, home: &Path) -> String {
    format!(
        "The group \u{201c}{group}\u{201d} keeps its notes in {}: rename this folder to make a group of its own.",
        home.display()
    )
}

/// `50000` as `50,000`, for a sentence that names a count.
pub fn group_digits(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The rows of a list of notes in their order: a heading row where a section starts, and a row
/// per note under it, nested by the section paths.
pub fn rows_of(notes: &[Note]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    let mut open: Vec<String> = Vec::new();
    for (i, note) in notes.iter().enumerate() {
        // The part of the note's section path the rows do not have yet. Sections are one when
        // they differ only in case, as a section's box ticks them (`under`) and the library
        // writes them (audit 4, C12): `A` and `a` on a file system that has both are one row.
        let shared = open
            .iter()
            .zip(&note.sections)
            .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
            .count();
        for n in shared..note.sections.len() {
            let path = note.sections[..=n].to_vec();
            let ticked = notes
                .iter()
                .filter(|x| x.sections.len() > n && under(x, &path))
                .all(|x| x.ticked);
            rows.push(Row {
                kind: RowKind::Section { path },
                ticked,
            });
        }
        open = note.sections.clone();
        rows.push(Row {
            kind: RowKind::File(i),
            ticked: note.ticked,
        });
    }
    rows
}

impl Plan {
    /// The note a row is, when it is one.
    pub fn note_of(&self, row: usize) -> Option<usize> {
        match self.rows.get(row)?.kind {
            RowKind::File(i) => Some(i),
            RowKind::Section { .. } => None,
        }
    }

    /// How many notes are ticked.
    pub fn ticked(&self) -> usize {
        self.notes.iter().filter(|n| n.ticked).count()
    }

    /// The notes the group's file lists already, which are not written again.
    pub fn already(&self) -> usize {
        self.notes.iter().filter(|n| n.already).count()
    }

    /// Ticks or unticks the note of a row. A file the library lists already is left as it is: its
    /// line is in the group's file, and a second one is no use.
    pub fn toggle_file(&mut self, i: usize) {
        let Some(note) = self.notes.get_mut(i) else {
            return;
        };
        if note.already {
            return;
        }
        note.ticked = !note.ticked;
        self.rows = rows_of(&self.notes);
    }

    /// Ticks or unticks everything under a section heading: all of it when any note under it is
    /// unticked, nothing when every one is ticked. A file the library lists already stays as it
    /// is.
    pub fn toggle_section(&mut self, path: &[String]) {
        let on = self
            .notes
            .iter()
            .filter(|n| under(n, path))
            .any(|n| !n.ticked && !n.already);
        for note in &mut self.notes {
            if under(note, path) && !note.already {
                note.ticked = on;
            }
        }
        self.rows = rows_of(&self.notes);
    }

    /// Ticks the row of the card: a note's own box, or a section's, which is all of it.
    pub fn toggle_row(&mut self, row: usize) {
        match self.rows.get(row).map(|r| r.kind.clone()) {
            Some(RowKind::File(i)) => self.toggle_file(i),
            Some(RowKind::Section { path }) => self.toggle_section(&path),
            None => {}
        }
    }

    pub fn tick_all(&mut self) {
        for note in &mut self.notes {
            note.ticked = !note.already;
        }
        self.rows = rows_of(&self.notes);
    }

    pub fn untick_all(&mut self) {
        for note in &mut self.notes {
            note.ticked = false;
        }
        self.rows = rows_of(&self.notes);
    }

    /// Why "Add the files" cannot be pressed, or `None` when it can.
    pub fn problem(&self) -> Option<String> {
        if let Some(why) = &self.blocked {
            return Some(why.clone());
        }
        let n = self.ticked();
        if n == 0 {
            return Some("Nothing is ticked.".to_string());
        }
        if self.entries.saturating_add(n) > library::MAX_ENTRIES {
            return Some(format!(
                "The group would have more than {} notes: {} in it, and {} more ticked.",
                library::MAX_ENTRIES,
                self.entries,
                n
            ));
        }
        self.read_back_fault()
    }

    /// Why a plan cannot be written at all, before any tick is counted.
    fn why_nothing_can_be_written(&self) -> Option<String> {
        if self.group_name.trim().is_empty() {
            return Some("That folder has no name to give the group.".to_string());
        }
        if self.more {
            return Some(format!(
                "The folder holds more than {} files, which is the most the library keeps for a group. Add them in parts.",
                library::MAX_ENTRIES
            ));
        }
        if self.notes.is_empty() || self.notes.iter().all(|n| n.already) {
            return Some("Every file in it is in the library already.".to_string());
        }
        None
    }

    /// The first heading the library would read back differently, as the sentence the card shows:
    /// the reader takes a closing run of `#` off a heading, so a group or a folder called `C#` reads
    /// back as `C`, and one called `#` reads back as no heading at all.
    fn read_back_fault(&self) -> Option<String> {
        if !reads_back(&self.group_name) {
            return Some(format!(
                "\u{201c}{}\u{201d} would not read back from the library: a heading that ends in '#' loses it. Rename the folder first.",
                self.group_name
            ));
        }
        for note in self.notes.iter().filter(|n| n.ticked) {
            if let Some(section) = note.sections.iter().find(|s| !reads_back(s)) {
                return Some(format!(
                    "\u{201c}{section}\u{201d} would not read back from the library: a heading that ends in '#' loses it. Rename the folder first."
                ));
            }
        }
        None
    }

    /// The links the group's file would hold, one per ticked note.
    pub fn links(&self) -> Vec<Link> {
        self.notes
            .iter()
            .filter(|n| n.ticked)
            .map(|note| {
                let path = tree::link_path(&self.library, &note.path);
                let title = note.title.trim().replace(['[', ']'], "");
                Link {
                    line: library::link_line(&title, &path),
                    path,
                    title,
                    sections: note.sections.clone(),
                }
            })
            .collect()
    }

    /// The text a new group's library file is made from: its heading and its `home:` line, which
    /// names the folder picked — relative to the library when it is inside it, so the library can
    /// move as a whole, and by its whole path otherwise.
    pub fn group_text(&self) -> Option<String> {
        library::new_group_with_home(&self.group_name, Some(&self.written_home()))
    }

    /// The folder picked as a library file writes it (see [`Plan::group_text`]).
    pub fn written_home(&self) -> PathBuf {
        match self.home.strip_prefix(&self.library) {
            Ok(inside) if !inside.as_os_str().is_empty() => inside.to_path_buf(),
            _ => self.home.clone(),
        }
    }
}

/// Whether a heading's text reads back as itself: the library's reader takes a closing run of `#`
/// off it, and an empty heading is no section at all.
fn reads_back(text: &str) -> bool {
    let written = text.trim();
    !written.is_empty() && written.trim_end_matches('#').trim() == written
}

/// Whether `note` is under the heading path `section`.
fn under(note: &Note, section: &[String]) -> bool {
    note.sections.len() >= section.len()
        && note
            .sections
            .iter()
            .zip(section)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// A path as a card says it: its file name when it has one.
pub fn shown(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// The notes of a group a plan would join, from a library as it reads now.
pub fn listed(entries: &[Entry]) -> Vec<PathBuf> {
    entries.iter().map(|e| e.path.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mdedit-import-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn put(dir: &Path, name: &str, text: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    fn at<'a>(root: &'a Path, new_file: PathBuf) -> Where<'a> {
        Where {
            root,
            group: None,
            group_home: None,
            home_apart: false,
            new_file,
            entries: 0,
            listed: &[],
        }
    }

    #[test]
    fn the_walk_takes_three_kinds_of_file_and_leaves_the_rest() {
        let dir = scratch("walk");
        put(&dir, "a.md", "# A\n");
        put(&dir, "b.markdown", "# B\n");
        put(&dir, "c.txt", "c\n");
        put(&dir, "d.pdf", "no\n");
        put(&dir, "e", "no\n");
        put(&dir, ".hidden.md", "# hidden\n");
        put(&dir, "target/generated.md", "# generated\n");
        put(&dir, "node_modules/dep.md", "# dep\n");
        put(&dir, "deep/inner/f.md", "# F\n");
        let w = walk(&dir, &|| false);
        let names: Vec<String> = w
            .found
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a.md", "b.markdown", "c.txt", "f.md"], "{names:?}");
        // The folders that hold a file: the picked one, and `deep/inner`.
        assert_eq!(w.folders, 2);
        assert!(!w.more);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_note_is_filed_under_its_subfolders_three_deep_and_deeper_folders_are_folded() {
        let dir = scratch("sections");
        put(&dir, "one.md", "# One\n");
        put(&dir, "A/two.md", "# Two\n");
        put(&dir, "A/B/three.md", "# Three\n");
        put(&dir, "A/B/C/four.md", "# Four\n");
        put(&dir, "A/B/C/D/five.md", "# Five\n");
        let w = walk(&dir, &|| false);
        let sections: Vec<Vec<String>> = w.found.iter().map(|f| f.sections.clone()).collect();
        assert_eq!(
            sections,
            vec![
                Vec::<String>::new(),
                vec!["A".to_string()],
                vec!["A".to_string(), "B".to_string()],
                vec!["A".to_string(), "B".to_string(), "C".to_string()],
                // The fourth folder is folded into the third.
                vec!["A".to_string(), "B".to_string(), "C".to_string()],
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_title_is_the_first_heading_and_a_fence_is_not_a_heading() {
        assert_eq!(title_of("# Pasta\n\nText\n"), Some("Pasta".to_string()));
        assert_eq!(title_of("text\n\n## Bread ##\n"), Some("Bread".to_string()));
        // A fence's `#` is code, and the heading after it is the file's first.
        assert_eq!(
            title_of("```sh\n# not a title\n```\n# Real\n"),
            Some("Real".to_string())
        );
        assert_eq!(title_of("~~~\n# not a title\n~~~\n"), None);
        assert_eq!(title_of("#hashtag\n"), None);
        assert_eq!(title_of("no heading\n"), None);
        assert_eq!(title_of("## C# ##\n"), Some("C#".to_string()));
        let dir = scratch("titles");
        put(&dir, "no-heading-file.md", "just text\n");
        put(&dir, "fenced.md", "```\n# code\n```\n\n# The title\n");
        let w = walk(&dir, &|| false);
        let titles: Vec<&str> = w.found.iter().map(|f| f.title.as_str()).collect();
        assert_eq!(titles, ["The title", "no-heading-file"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rows_preview_is_the_files_first_lines_with_the_blank_ones_at_the_ends_gone() {
        let dir = scratch("preview");
        put(&dir, "a.md", "\n\n# A\n\nOne\nTwo\n");
        let w = walk(&dir, &|| false);
        assert_eq!(w.found[0].head, ["# A", "", "One", "Two"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_plan_files_notes_under_heading_rows_and_a_section_row_ticks_them_all() {
        let dir = scratch("rows");
        let root = dir.join("library");
        std::fs::create_dir_all(&root).unwrap();
        let folder = dir.join("Recipes");
        put(&folder, "one.md", "# One\n");
        put(&folder, "Pasta/two.md", "# Two\n");
        put(&folder, "Pasta/Italian/three.md", "# Three\n");
        put(&folder, "Sauces/four.md", "# Four\n");
        let w = walk(&folder, &|| false);
        let mut p = plan(w, &folder, at(&root, root.join("Recipes.md")));
        let kinds: Vec<(String, bool)> = p
            .rows
            .iter()
            .map(|r| match &r.kind {
                RowKind::Section { path } => (path.join("/"), r.ticked),
                RowKind::File(i) => (p.notes[*i].title.clone(), r.ticked),
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("One".to_string(), true),
                ("Pasta".to_string(), true),
                ("Two".to_string(), true),
                ("Pasta/Italian".to_string(), true),
                ("Three".to_string(), true),
                ("Sauces".to_string(), true),
                ("Four".to_string(), true),
            ]
        );
        // Unticking "Pasta" unticks its own note and its subsection's, and nothing else.
        p.toggle_section(&["Pasta".to_string()]);
        assert_eq!(p.ticked(), 2);
        // Its row says the section is not all ticked, and so does the subsection's.
        let section_row = |p: &Plan, name: &str| {
            p.rows
                .iter()
                .position(
                    |r| matches!(&r.kind, RowKind::Section { path } if path.join("/") == name),
                )
                .map(|i| p.rows[i].ticked)
                .unwrap()
        };
        assert!(!section_row(&p, "Pasta"));
        assert!(!section_row(&p, "Pasta/Italian"));
        assert!(section_row(&p, "Sauces"));
        // Ticking it again ticks both.
        p.toggle_section(&["Pasta".to_string()]);
        assert_eq!(p.ticked(), 4);
        // A note ticks on its own, and its section row follows it.
        p.toggle_file(0);
        assert_eq!(p.ticked(), 3);
        assert!(
            !p.rows
                .iter()
                .find(|r| r.kind == RowKind::File(0))
                .unwrap()
                .ticked
        );
        // A row is ticked through the card's own call, section or note.
        p.toggle_row(0);
        assert_eq!(p.ticked(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_the_group_lists_already_cannot_be_ticked() {
        let dir = scratch("already");
        let root = dir.join("library");
        std::fs::create_dir_all(&root).unwrap();
        let folder = dir.join("Recipes");
        let one = put(&folder, "one.md", "# One\n");
        put(&folder, "two.md", "# Two\n");
        let w = walk(&folder, &|| false);
        let listed = vec![one.clone()];
        let mut p = plan(
            w,
            &folder,
            Where {
                root: &root,
                group: Some(("Recipes", &root.join("Recipes.md"))),
                group_home: None,
                home_apart: false,
                new_file: root.join("Recipes.md"),
                entries: 1,
                listed: &listed,
            },
        );
        assert_eq!(p.already(), 1);
        let i = p
            .notes
            .iter()
            .position(|n| same_path(&n.path, &one))
            .unwrap();
        assert!(!p.notes[i].ticked);
        p.toggle_file(i);
        assert!(!p.notes[i].ticked, "a listed file stays unticked");
        p.tick_all();
        assert!(!p.notes[i].ticked, "Tick all leaves it unticked");
        assert_eq!(p.ticked(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_over_the_librarys_most_is_refused_with_the_count() {
        let dir = scratch("more");
        let root = dir.join("library");
        std::fs::create_dir_all(&root).unwrap();
        let folder = dir.join("Big");
        for i in 0..(library::MAX_ENTRIES + 1) {
            put(&folder, &format!("n{i}.md"), "# N\n");
        }
        let w = walk(&folder, &|| false);
        assert!(w.more);
        let p = plan(w, &folder, at(&root, root.join("Big.md")));
        let why = p.problem().unwrap_or_default();
        assert!(why.contains("2000") && why.contains("parts"), "{why}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_group_would_pass_two_thousand_notes_is_said_with_both_numbers() {
        let dir = scratch("cap");
        let root = dir.join("library");
        std::fs::create_dir_all(&root).unwrap();
        let folder = dir.join("Recipes");
        put(&folder, "one.md", "# One\n");
        put(&folder, "two.md", "# Two\n");
        let w = walk(&folder, &|| false);
        let p = plan(
            w,
            &folder,
            Where {
                root: &root,
                group: Some(("Recipes", &root.join("Recipes.md"))),
                group_home: None,
                home_apart: false,
                new_file: root.join("Recipes.md"),
                entries: library::MAX_ENTRIES - 1,
                listed: &[],
            },
        );
        let why = p.problem().unwrap_or_default();
        assert!(why.contains("2000") && why.contains("2 more"), "{why}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_title_that_ends_in_a_hash_reads_back_without_it_and_a_folder_that_ends_in_one_is_refused()
    {
        let dir = scratch("hash");
        let root = dir.join("library");
        std::fs::create_dir_all(&root).unwrap();
        // The library's reader takes a closing `#` off a heading, so a file's `# C#` is the title
        // `C`: the plan says what the library will read, not what the file looks like.
        let folder = dir.join("Code");
        put(&folder, "csharp.md", "# C#\n");
        let p = plan(
            walk(&folder, &|| false),
            &folder,
            at(&root, root.join("Code.md")),
        );
        assert_eq!(p.notes[0].title, "C");
        assert_eq!(p.problem(), None);
        // A folder whose name ends in `#` would be written as a heading and read back without it.
        let folder = dir.join("C#");
        put(&folder, "notes.md", "# Notes\n");
        let p = plan(
            walk(&folder, &|| false),
            &folder,
            at(&root, root.join("C.md")),
        );
        let why = p.problem().unwrap_or_default();
        assert!(why.contains("C#"), "{why}");
        assert!(why.contains("'#'"), "{why}");
        // The same for a subfolder, named by its folder.
        let folder = dir.join("Sharp");
        put(&folder, "D#/notes.md", "# Notes\n");
        let p = plan(
            walk(&folder, &|| false),
            &folder,
            at(&root, root.join("Sharp.md")),
        );
        let why = p.problem().unwrap_or_default();
        assert!(why.contains("D#"), "{why}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_link_is_relative_inside_the_library_and_whole_outside_it() {
        // A temp folder is not always spelled the way the system resolves it: macOS's
        // `temp_dir` is under `/var`, a link to `/private/var`, and a Windows CI runner's
        // `%TEMP%` is the 8.3 name `RUNNER~1` of `runneradmin`. A note outside the library is
        // written by the whole path the folder was given (`tree::link_path`: "as it is
        // otherwise"), so the fixture starts from the folder as the system resolves it, and the
        // expectation is that same whole path. A plan that made the link relative, or dropped a
        // part of it, still fails here.
        let dir = mdshared_app::paths::canonical_or_absolute(&scratch("links"));
        let root = dir.join("library");
        std::fs::create_dir_all(&root).unwrap();
        let inside = root.join("Recipes");
        let outside = dir.join("Dishes");
        put(&inside, "one.md", "# One\n");
        put(&outside, "two.md", "# Two\n");
        let p = plan(
            walk(&inside, &|| false),
            &inside,
            at(&root, root.join("Recipes.md")),
        );
        assert_eq!(p.links()[0].path, PathBuf::from("Recipes").join("one.md"));
        assert_eq!(p.links()[0].line, "- [One](Recipes/one.md)");
        let p = plan(
            walk(&outside, &|| false),
            &outside,
            at(&root, root.join("Dishes.md")),
        );
        assert_eq!(p.links()[0].path, outside.join("two.md"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_with_nothing_the_walk_takes_is_refused_plainly() {
        let dir = scratch("empty");
        let root = dir.join("library");
        std::fs::create_dir_all(&root).unwrap();
        let folder = dir.join("Empty");
        std::fs::create_dir_all(&folder).unwrap();
        let p = plan(
            walk(&folder, &|| false),
            &folder,
            at(&root, root.join("Empty.md")),
        );
        assert_eq!(
            p.problem().unwrap_or_default(),
            "Every file in it is in the library already."
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_group_text_is_the_folders_name_and_its_home() {
        let dir = scratch("group");
        let root = dir.join("library");
        std::fs::create_dir_all(&root).unwrap();
        let folder = root.join("Recipes");
        std::fs::create_dir_all(&folder).unwrap();
        let p = plan(
            walk(&folder, &|| false),
            &folder,
            at(&root, root.join("Recipes.md")),
        );
        let text = p.group_text().unwrap();
        assert!(text.starts_with("# Recipes\nhome: Recipes\n"), "{text}");
        let read = library::parse(&text, &root);
        assert_eq!(read.name.as_deref(), Some("Recipes"));
        assert_eq!(read.home, Some(folder.clone()));
        assert!(read.entries.is_empty(), "the template holds no note");
        // A folder outside the library is named by its whole path.
        let outside = dir.join("Dishes");
        std::fs::create_dir_all(&outside).unwrap();
        let p = plan(
            walk(&outside, &|| false),
            &outside,
            at(&root, root.join("Dishes.md")),
        );
        let text = p.group_text().unwrap();
        let written = outside.to_string_lossy().replace('\\', "/");
        assert!(text.contains(&written), "{text}");
        let read = library::parse(&text, &root);
        assert_eq!(read.home, Some(outside.clone()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_found_file_carries_the_start_an_ai_request_sends_its_size_and_the_day() {
        let dir = scratch("digest");
        let text = format!("# A\n{}", "x".repeat(crate::import_ai::DIGEST_MAX * 2));
        let path = put(&dir, "a.md", &text);
        let w = walk(&dir, &|| false);
        let f = &w.found[0];
        assert_eq!(f.start.len(), crate::import_ai::DIGEST_MAX);
        assert!(f.start.starts_with("# A\n"));
        assert_eq!(f.bytes as usize, text.len());
        assert!(f.modified.len() == 10, "{}", f.modified);
        assert!(f.modified.starts_with("20"), "{}", f.modified);
        let _ = std::fs::remove_file(path);
        // A day known exactly, from the first second of 1970 and from 2026-11-23.
        let at = |secs: u64| {
            super::modified_day(Some(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs),
            ))
        };
        assert_eq!(at(0), "1970-01-01");
        assert_eq!(at(86_400), "1970-01-02");
        assert_eq!(at(1_795_392_000), "2026-11-23");
        assert_eq!(super::modified_day(None), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn names(w: &Walk) -> Vec<String> {
        w.found
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    /// Audit 4, C5: the folder that holds the library does not offer the library's own files
    /// as notes. A folder that only has the library's name is read as any other.
    #[test]
    fn the_library_folder_inside_the_folder_walked_is_left_out() {
        let dir = scratch("own-library");
        put(&dir, "a.md", "# A\n");
        put(&dir, "library/personal.md", "# Personal\n");
        put(&dir, "library/Recipes/notes.txt", "notes\n");
        put(&dir, "Old/library/b.md", "# B\n");
        let library = dir.join("library");
        let w = walk_for(&dir, &library, &|| false);
        assert_eq!(names(&w), ["a.md", "b.md"]);
        assert!(w.library_left_out);
        // A walk that knows no library reads it all.
        let all = walk(&dir, &|| false);
        assert_eq!(all.found.len(), 4);
        assert!(!all.library_left_out);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 4, C6: a file whose name the library cannot write is left out and named, and the
    /// rest of the folder is taken. `%20` reads back as a space; a space is written in `<>`.
    #[test]
    fn a_file_whose_line_would_not_read_back_is_left_out_and_named() {
        let library = scratch("percent");
        let folder = library.join("Recipes");
        put(&folder, "pasta.md", "# Pasta\n");
        put(&folder, "tomato sauce.md", "# Tomato sauce\n");
        let odd = put(&folder, "my%20draft.md", "# Draft\n");
        let w = walk_for(&folder, &library, &|| false);
        assert_eq!(names(&w), ["pasta.md", "tomato sauce.md"]);
        assert_eq!(w.unlinkable, vec![odd.clone()]);
        assert!(!reads_back_as_link(&library, &odd, "Draft"));
        assert!(reads_back_as_link(
            &library,
            &folder.join("tomato sauce.md"),
            "Tomato sauce"
        ));
        // Outside the library, where the line holds the whole path, the same.
        let outside = scratch("percent-out");
        let far = put(&outside, "a%20b.md", "# AB\n");
        assert!(!reads_back_as_link(&library, &far, "AB"));
        assert!(reads_back_as_link(
            &library,
            &put(&outside, "c.md", "# C\n"),
            "C"
        ));
        // The plan holds only what can be written, and the dry run of its links passes.
        let p = plan(w, &folder, at(&library, library.join("Recipes.md")));
        assert_eq!(p.notes.len(), 2);
        let text = p.group_text().unwrap();
        let links = p.links();
        let new = mdshared_app::runs::with_links_added(&text, &links);
        assert!(mdshared_app::runs::check_links_added(
            &library::parse(&text, &library),
            &library::parse(&new, &library),
            &links,
            &library
        ));
        let _ = std::fs::remove_dir_all(&library);
        let _ = std::fs::remove_dir_all(&outside);
    }

    /// Audit 4, C7: a file with bare CR line ends (an old Mac file) gives the first line as its
    /// title and a preview of its lines; a title holds no control character and at most
    /// `TITLE_MAX` characters.
    #[test]
    fn a_title_is_one_line_with_no_control_character_and_a_length_it_can_be_written_at() {
        assert_eq!(
            title_of("# Shopping\rmilk\reggs\r"),
            Some("Shopping".to_string())
        );
        assert_eq!(
            preview_of("# Shopping\rmilk\r\reggs\r"),
            ["# Shopping", "milk", "", "eggs"]
        );
        assert_eq!(preview_of("a\r\nb\r\n"), ["a", "b"], "CRLF is one break");
        assert_eq!(title_of("# A\u{7}B\tC\n"), Some("A B C".to_string()));
        let long = format!("# {}\n", "\u{e9}".repeat(TITLE_MAX + 50));
        assert_eq!(title_of(&long).unwrap().chars().count(), TITLE_MAX);
        // A heading of control characters only gives way to the next one.
        assert_eq!(title_of("# \u{1}\u{2}\n# Real\n"), Some("Real".to_string()));
        let dir = scratch("cr");
        put(&dir, "mac.txt", "# Shopping\rmilk\reggs\r");
        let w = walk(&dir, &|| false);
        assert_eq!(w.found[0].title, "Shopping");
        assert_eq!(w.found[0].head, ["# Shopping", "milk", "eggs"]);
        let line = &plan(w, &dir, at(&dir, dir.join("x.md"))).links()[0].line;
        assert!(!line.contains('\r'), "{line:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 4, C8 and C12: a walk stops at the entries it reads and at the depth it goes to,
    /// and the plan of a walk cut short is refused with that reason, not written in part.
    #[test]
    fn a_walk_stops_at_its_bounds_and_its_plan_is_refused_with_the_reason() {
        let dir = scratch("bounds");
        for i in 0..12 {
            put(&dir, &format!("f{i:02}.md"), "# F\n");
        }
        let few = Limits {
            files: 100,
            read: 10,
            depth: MAX_DEPTH,
        };
        let w = walk_with(&dir, None, few, &|| false);
        assert_eq!(w.stopped, Some(Stop::Entries));
        assert_eq!(w.read, 11);
        let p = plan(w, &dir, at(&dir, dir.join("x.md")));
        assert_eq!(p.problem(), Some(stopped_reason(Stop::Entries)));
        assert!(stopped_reason(Stop::Entries).contains("50,000"));
        // The same folder within the bounds is read to its end.
        let w = walk(&dir, &|| false);
        assert_eq!((w.stopped, w.found.len()), (None, 12));

        let deep = scratch("depth");
        put(&deep, "a/b/c/d/e/x.md", "# X\n");
        let shallow = Limits {
            files: 100,
            read: 1000,
            depth: 3,
        };
        let w = walk_with(&deep, None, shallow, &|| false);
        assert_eq!(w.stopped, Some(Stop::Depth));
        assert!(w.found.is_empty());
        let p = plan(w, &deep, at(&deep, deep.join("x.md")));
        assert_eq!(p.problem(), Some(stopped_reason(Stop::Depth)));
        let w = walk(&deep, &|| false);
        assert_eq!((w.stopped, w.found.len()), (None, 1));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&deep);
    }

    /// Audit 4, C10: a junction back to the folder walked is not followed (Windows makes one
    /// without rights a symlink would need).
    #[cfg(windows)]
    #[test]
    fn a_junction_to_a_folder_is_not_followed() {
        let dir = scratch("junction");
        put(&dir, "a.md", "# A\n");
        let link = dir.join("loop");
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&dir)
            .output()
            .expect("cmd runs");
        assert!(made.status.success(), "{made:?}");
        assert!(link.join("a.md").is_file(), "the junction leads back");
        let w = walk(&dir, &|| false);
        assert_eq!(names(&w), ["a.md"]);
        assert_eq!(w.stopped, None);
        // The junction goes, and what it led to stays.
        std::fs::remove_dir(&link).unwrap();
        assert!(dir.join("a.md").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 4, C10: a symlink back to the folder walked is not followed; one to a file is read
    /// as that file.
    #[cfg(unix)]
    #[test]
    fn a_symlink_to_a_folder_is_not_followed() {
        let dir = scratch("symlink");
        put(&dir, "a.md", "# A\n");
        let other = scratch("symlink-other");
        let b = put(&other, "b.md", "# B\n");
        std::os::unix::fs::symlink(&dir, dir.join("loop")).unwrap();
        std::os::unix::fs::symlink(&b, dir.join("b.md")).unwrap();
        let w = walk(&dir, &|| false);
        assert_eq!(names(&w), ["a.md", "b.md"]);
        assert_eq!(w.stopped, None);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&other);
    }

    fn note(sections: &[&str], title: &str) -> Note {
        Note {
            path: PathBuf::from(format!("{title}.md")),
            sections: sections.iter().map(|s| s.to_string()).collect(),
            title: title.to_string(),
            head: Vec::new(),
            start: String::new(),
            bytes: 0,
            modified: String::new(),
            ticked: true,
            already: false,
            confidence: None,
            why: String::new(),
            unsorted: false,
        }
    }

    /// Audit 4, C12: folders `A` and `a`, which a case-sensitive file system can hold side by
    /// side, are one section row, as the section's box ticks both and the library writes both
    /// under one heading.
    #[test]
    fn sections_that_differ_only_in_case_are_one_row() {
        let notes = vec![note(&["A"], "one"), note(&["a"], "two")];
        let rows = rows_of(&notes);
        let sections = rows
            .iter()
            .filter(|r| matches!(r.kind, RowKind::Section { .. }))
            .count();
        assert_eq!((rows.len(), sections), (3, 1));
        let mut notes = notes;
        notes[1].ticked = false;
        assert!(!rows_of(&notes)[0].ticked, "the row is not all ticked");
    }

    /// Audit 4, C12: a file the group lists by a path with `..` in it is in the group already.
    #[test]
    fn a_file_the_group_lists_through_dot_dot_is_in_it_already() {
        let root = scratch("dots");
        let folder = root.join("Recipes");
        put(&folder, "pasta.md", "# Pasta\n");
        let listed = vec![
            root.join("Other")
                .join("..")
                .join("Recipes")
                .join(".")
                .join("pasta.md"),
        ];
        let file = root.join("Recipes.md");
        let p = plan(
            walk(&folder, &|| false),
            &folder,
            Where {
                root: &root,
                group: Some(("Recipes", &file)),
                group_home: Some(&folder),
                home_apart: false,
                new_file: file.clone(),
                entries: 1,
                listed: &listed,
            },
        );
        assert!(p.notes[0].already);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Audit 4, C4: a group of the folder's name that keeps its notes in another folder refuses
    /// the plan, which names the group and that folder; its own folder, or none, is joined.
    #[test]
    fn a_group_that_keeps_its_notes_in_another_folder_refuses_the_plan() {
        let root = scratch("apart");
        let folder = root.join("Recipes");
        put(&folder, "pasta.md", "# Pasta\n");
        let elsewhere = root.join("elsewhere").join("Recipes");
        let file = root.join("Recipes.md");
        let joining = |home: Option<&Path>, apart: bool| {
            plan(
                walk(&folder, &|| false),
                &folder,
                Where {
                    root: &root,
                    group: Some(("Recipes", &file)),
                    group_home: home,
                    home_apart: apart,
                    new_file: file.clone(),
                    entries: 0,
                    listed: &[],
                },
            )
        };
        let p = joining(Some(&elsewhere), true);
        assert_eq!(p.home, elsewhere, "the plan says the group's own folder");
        let why = p.problem().unwrap_or_default();
        assert_eq!(why, home_apart_reason("Recipes", &elsewhere));
        assert!(why.contains("\u{201c}Recipes\u{201d}"), "{why}");
        assert!(why.contains(&elsewhere.display().to_string()), "{why}");
        let p = joining(Some(&folder), false);
        assert_eq!((p.problem(), &p.home), (None, &folder));
        let p = joining(None, false);
        assert_eq!(p.problem(), None);
        assert!(p.home.as_os_str().is_empty(), "the group names no folder");
        let _ = std::fs::remove_dir_all(&root);
    }

    // ---- the task window's walk (D-247, D-248) -------------------------------------------------

    fn every_at(
        dir: &Path,
        limits: Limits,
        wanted: &dyn Fn(&Path) -> bool,
        cancel: &dyn Fn() -> bool,
    ) -> Every {
        every_of(walk_sorted(dir, None, limits, Sort::Every, wanted, cancel))
    }

    fn every_names(w: &Every) -> Vec<String> {
        w.files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    /// The task window's walk stops where the import's does: at the entries it reads and at the
    /// depth it goes to.
    #[test]
    fn a_walk_for_the_task_window_stops_at_its_bounds() {
        let dir = scratch("every-bounds");
        for i in 0..12 {
            put(&dir, &format!("f{i:02}.md"), "x\n");
        }
        let few = Limits {
            files: 100,
            read: 10,
            depth: MAX_DEPTH,
        };
        let w = every_at(&dir, few, &|_| true, &|| false);
        assert_eq!(w.stopped, Some(Stop::Entries));

        let deep = scratch("every-depth");
        put(&deep, "a/b/c/d/x.md", "# X\n");
        let shallow = Limits {
            files: 100,
            read: 1000,
            depth: 2,
        };
        let w = every_at(&deep, shallow, &|_| true, &|| false);
        assert_eq!(w.stopped, Some(Stop::Depth));
        assert!(w.files.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&deep);
    }

    /// A walk for the task window takes only the files it was told are new, so its cap is the room
    /// left in the window, not the folder's size.
    #[test]
    fn a_walk_for_the_task_window_counts_only_the_files_it_was_told_are_new() {
        let dir = scratch("every-wanted");
        for i in 0..5 {
            put(&dir, &format!("f{i}.md"), "x\n");
        }
        let held = |p: &Path| p.file_name().is_some_and(|n| n == "f0.md" || n == "f1.md");
        let w = walk_every(
            &dir,
            EveryWalk {
                files: 2,
                library: None,
                wanted: &|p| !held(p),
            },
            &|| false,
        );
        assert_eq!(every_names(&w), ["f2.md", "f3.md"]);
        assert!(w.more, "f4 is there and there is no room for it");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The import's rule, for the task window too: the library folder inside the folder walked is
    /// left out, and a walk that knows no library reads it like any other folder (audit 4, C5).
    #[test]
    fn a_walk_for_the_task_window_leaves_the_library_folder_out() {
        let dir = scratch("every-library");
        put(&dir, "a.md", "# A\n");
        put(&dir, "library/note.md", "# Note\n");
        let library = dir.join("library");
        let w = walk_every(
            &dir,
            EveryWalk {
                files: 100,
                library: Some(&library),
                wanted: &|_| true,
            },
            &|| false,
        );
        assert_eq!(every_names(&w), ["a.md"]);
        let all = walk_every(
            &dir,
            EveryWalk {
                files: 100,
                library: None,
                wanted: &|_| true,
            },
            &|| false,
        );
        assert_eq!(all.files.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A folder is entered by name, ignoring case: `A` before `b`.
    #[test]
    fn a_walk_for_the_task_window_orders_subfolders_by_name_ignoring_case() {
        let dir = scratch("every-order");
        put(&dir, "b/second.md", "x\n");
        put(&dir, "A/first.md", "x\n");
        let w = every_at(&dir, LIMITS, &|_| true, &|| false);
        assert_eq!(every_names(&w), ["first.md", "second.md"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A cancel stops the walk between files, and before a folder is entered.
    #[test]
    fn a_walk_for_the_task_window_stops_when_a_cancel_comes() {
        let dir = scratch("every-cancel-files");
        for i in 0..5 {
            put(&dir, &format!("f{i}.md"), "x\n");
        }
        // The second file looked at: the first is taken, and the walk stops there.
        let seen = std::cell::Cell::new(0);
        let wanted = |_: &Path| {
            seen.set(seen.get() + 1);
            true
        };
        let cancel = || seen.get() >= 2;
        let w = every_at(&dir, LIMITS, &wanted, &cancel);
        assert_eq!(every_names(&w), ["f0.md"]);

        // Cancelled after the first folder's file: the second folder is never entered.
        let folder = scratch("every-cancel-folders");
        put(&folder, "one/first.md", "x\n");
        put(&folder, "two/second.md", "x\n");
        let calls = std::cell::Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() >= 7
        };
        let w = every_at(&folder, LIMITS, &|_| true, &cancel);
        assert_eq!(every_names(&w), ["first.md"]);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// A junction is not followed, so a walk cannot loop. A real junction, made by `mklink /J`;
    /// when it cannot be made the test passes over it and says so (D-250).
    #[cfg(windows)]
    #[test]
    fn a_walk_for_the_task_window_does_not_follow_a_junction() {
        let dir = scratch("every-junction");
        put(&dir, "a.md", "# A\n");
        let link = dir.join("loop");
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&dir)
            .output()
            .is_ok_and(|out| out.status.success());
        if !made {
            eprintln!("skipped: no junction could be made in {dir:?}");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let w = every_at(&dir, LIMITS, &|_| true, &|| false);
        assert_eq!(every_names(&w), ["a.md"]);
        let _ = std::fs::remove_dir(&link);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The same for a directory symlink, which needs Developer Mode or a privilege (D-251).
    #[cfg(windows)]
    #[test]
    fn a_walk_for_the_task_window_does_not_follow_a_directory_symlink() {
        let dir = scratch("every-dir-symlink");
        put(&dir, "a.md", "# A\n");
        let link = dir.join("loop");
        if std::os::windows::fs::symlink_dir(&dir, &link).is_err() {
            eprintln!("skipped: no directory symlink could be made in {dir:?}");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let w = every_at(&dir, LIMITS, &|_| true, &|| false);
        assert_eq!(every_names(&w), ["a.md"]);
        let _ = std::fs::remove_file(&link);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A folder the walk cannot read at all is named, so an add that finds nothing can say why
    /// (D-250). A file given where a folder is expected is the unreadable folder this can make
    /// without a policy change: `read_dir` refuses it.
    #[test]
    fn a_walk_for_the_task_window_names_a_folder_it_cannot_read() {
        let dir = scratch("every-unreadable");
        let file = dir.join("not-a-folder.md");
        std::fs::write(&file, "# A file\n").expect("a file");
        let w = every_at(&file, LIMITS, &|_| true, &|| false);
        assert!(w.files.is_empty(), "nothing came out of it");
        assert_eq!(w.unreadable, vec![file.clone()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The same with a symlink, where the system makes one.
    #[cfg(unix)]
    #[test]
    fn a_walk_for_the_task_window_does_not_follow_a_symlink() {
        let dir = scratch("every-symlink");
        put(&dir, "a.md", "# A\n");
        if std::os::unix::fs::symlink(&dir, dir.join("loop")).is_err() {
            return;
        }
        let w = every_at(&dir, LIMITS, &|_| true, &|| false);
        assert_eq!(every_names(&w), ["a.md"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
