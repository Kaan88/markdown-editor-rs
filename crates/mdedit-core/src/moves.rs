//! Moving notes that exist (`docs/ai-library-plan.md` 5.12): a group's notes into its folder, the
//! notes to match the headings, and the library itself into another folder. One machine; the
//! three differ only in the rows a plan opens with.
//!
//! What this module never does, whatever the plan says:
//!
//! - **Copy and delete.** A file is renamed. Where the rename crosses a volume it is copied
//!   (created exclusively, flushed, read back and compared), and the source is removed only after
//!   the library lines that name it have been written.
//! - **Overwrite.** A name that is taken is refused before the first file moves, and again as it
//!   moves; there is no "2" to make it fit.
//! - **Leave a dangling link behind a refusal.** Every library file that lists a moved note is
//!   rewritten by the window's own gate ([`MoveGate`]), which refuses a file that is open with
//!   unsaved changes. All the rewrites of one run are one unit: if one is refused, the ones
//!   written are written back and every file is put back.
//! - **Forget.** The record is written first (`runs::Record`, kind `move`), so a move is listed
//!   in "AI runs…" and is undone by the same machine run the other way, under the rules of
//!   D-069 to D-072: the record says what happened, every path in it is confined, and a file is
//!   put back only while it is still the file the run moved.
//!
//! Everything that can be refused is refused by [`preflight`] before the first file moves.

use crate::runs::{
    self, DIGEST_MAX_BYTES, Failure, Json, Kind, Reach, Record, State, esc, has_extension,
    has_parent_dir, inside, read_regular, sections_eq, shown, stored, under,
};
use mdshared_app::library::{self, Parsed};
use mdshared_app::paths::{canonical_or_absolute, same_path};
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

/// The longest a note's whole path may be after it moves (the plan's 240).
pub const PATH_MAX_CHARS: usize = 240;

/// The most records of earlier runs a library move carries along.
pub const RECORDS_MAX: usize = 400;

/// The most entries of the old library folder looked at to say how many files stay behind.
const WALK_MAX: usize = 20_000;

/// The most library files looked at: the number the undo's listing reads.
const LIBRARY_FILES_MAX: usize = 4096;

// ---- what a plan is made of --------------------------------------------------------------------

/// Where a plan started from: the three moves of 5.12.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// A group's notes that are not in its folder.
    Group,
    /// A group whose notes are in its folder, in folders that disagree with their headings.
    Headings,
    /// The library folder changed, and the library goes with it.
    Library,
}

impl Start {
    pub fn as_str(self) -> &'static str {
        match self {
            Start::Group => "group",
            Start::Headings => "headings",
            Start::Library => "library",
        }
    }

    fn parse(text: &str) -> Option<Start> {
        match text {
            "group" => Some(Start::Group),
            "headings" => Some(Start::Headings),
            "library" => Some(Start::Library),
            _ => None,
        }
    }
}

/// What a moved file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    /// A note a library file lists.
    Note,
    /// A library file: a `.md` directly in the library folder.
    Library,
    /// A record of an earlier run, or its journal.
    Record,
}

impl FileKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FileKind::Note => "note",
            FileKind::Library => "library",
            FileKind::Record => "record",
        }
    }

    fn parse(text: &str) -> Option<FileKind> {
        match text {
            "note" => Some(FileKind::Note),
            "library" => Some(FileKind::Library),
            "record" => Some(FileKind::Record),
            _ => None,
        }
    }

    /// Whether a file of this kind may have the extension of `path`.
    fn fits(self, path: &Path) -> bool {
        match self {
            FileKind::Note | FileKind::Library => has_extension(path, &["md"]),
            FileKind::Record => has_extension(path, &["json", "done"]),
        }
    }
}

/// Why a row can or cannot be ticked, and what it says in place of its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Ready,
    /// A note outside the library's own area: the row says where it lives. Not blocked, but it
    /// comes unticked, because the move would take a file the owner keeps elsewhere.
    Outside(String),
    /// "There is already a file called this."
    Taken,
    Missing,
    NotAFile,
    TooLarge,
    /// The library lists the note this many times, not once: a move could not keep every link.
    Listed(usize),
    TooLong(usize),
    /// A file is where one of its folders would go.
    InTheWay(String),
}

impl Verdict {
    /// A blocked row is never ticked and never moved.
    pub fn blocked(&self) -> bool {
        !matches!(self, Verdict::Ready | Verdict::Outside(_))
    }

    /// What the row says about itself; empty for a row with nothing to say.
    pub fn text(&self) -> String {
        match self {
            Verdict::Ready => String::new(),
            Verdict::Outside(dir) => format!(
                "lives in {dir}, outside the library: moving it takes a file you keep elsewhere"
            ),
            Verdict::Taken => "there is already a file called this".to_string(),
            Verdict::Missing => "the file is not there".to_string(),
            Verdict::NotAFile => "not a plain file: it is a link or a folder".to_string(),
            Verdict::TooLarge => "too large to be checked after a copy".to_string(),
            Verdict::Listed(n) => {
                format!("the library lists it {n} times, so a move could not keep every link")
            }
            Verdict::TooLong(n) => format!("the new path would be {n} characters long"),
            Verdict::InTheWay(name) => format!("{name} is a file where its folder would go"),
        }
    }
}

/// One file the plan would move.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: FileKind,
    pub from: PathBuf,
    pub to: PathBuf,
    /// What the library calls it.
    pub title: String,
    pub sections: Vec<String>,
    /// Why it moves.
    pub why: String,
    pub verdict: Verdict,
    pub ticked: bool,
    pub len: u64,
    pub sha256: String,
    pub outside: bool,
}

/// The group a plan is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupRef {
    pub name: String,
    pub file: PathBuf,
    pub home: PathBuf,
}

/// The rows a move opens with, and what is known about them.
#[derive(Clone, Debug)]
pub struct Plan {
    pub start: Start,
    /// The library folder the plan writes into and records in: the current one.
    pub library: PathBuf,
    /// The folder the library moves from, for [`Start::Library`].
    pub from_library: Option<PathBuf>,
    pub group: Option<GroupRef>,
    pub label: String,
    pub rows: Vec<Row>,
    /// The whole plan cannot be written, and why.
    pub blocked: Option<String>,
    /// Notes the library lists that are not on disk: not rows, and left as they are.
    pub missing: usize,
    /// Files in the old library folder that are not rows and stay there (a library move).
    pub others: usize,
    pub others_capped: bool,
    /// Library files beyond the reader's limit, which a library move leaves.
    pub omitted: usize,
}

impl Plan {
    fn empty(start: Start, library: &Path, label: String) -> Plan {
        Plan {
            start,
            library: library.to_path_buf(),
            from_library: None,
            group: None,
            label,
            rows: Vec::new(),
            blocked: None,
            missing: 0,
            others: 0,
            others_capped: false,
            omitted: 0,
        }
    }

    fn block(mut self, why: impl Into<String>) -> Plan {
        self.blocked = Some(why.into());
        self
    }

    /// How many rows are ticked.
    pub fn ticked(&self) -> usize {
        self.rows.iter().filter(|r| r.ticked).count()
    }

    /// Ticks or unticks a row. A blocked row stays unticked.
    pub fn set_ticked(&mut self, i: usize, on: bool) {
        if let Some(r) = self.rows.get_mut(i) {
            r.ticked = on && !r.verdict.blocked();
        }
    }

    pub fn tick_all(&mut self) {
        for r in &mut self.rows {
            r.ticked = !r.verdict.blocked() && !matches!(r.verdict, Verdict::Outside(_));
        }
    }

    pub fn untick_all(&mut self) {
        for r in &mut self.rows {
            r.ticked = false;
        }
    }

    /// The ticked rows, as a write takes them.
    pub fn request(&self) -> Request {
        Request {
            start: self.start,
            library: self.library.clone(),
            from_library: self.from_library.clone(),
            group: self.group.clone(),
            label: self.label.clone(),
            rows: self
                .rows
                .iter()
                .filter(|r| r.ticked && !r.verdict.blocked())
                .cloned()
                .collect(),
        }
    }
}

/// What a write is asked to do: the ticked rows.
#[derive(Clone, Debug)]
pub struct Request {
    pub start: Start,
    pub library: PathBuf,
    pub from_library: Option<PathBuf>,
    pub group: Option<GroupRef>,
    pub label: String,
    pub rows: Vec<Row>,
}

// ---- reading the disk for a plan ---------------------------------------------------------------

fn canon(path: &Path) -> PathBuf {
    canonical_or_absolute(path)
}

/// `path` as a library file at `base` writes it: relative when it is inside `base`, as it is
/// otherwise (`library_pane::link_path`'s rule, for a path that may not exist yet).
fn written(base: &Path, path: &Path) -> PathBuf {
    let (b, p) = (canon(base), canon(path));
    if under(&p, &b) && !same_path(&p, &b) {
        p.components().skip(b.components().count()).collect()
    } else {
        path.to_path_buf()
    }
}

/// What lies below `base` on the way to `path`: `path` with `base`'s components taken off.
fn below(base: &Path, path: &Path) -> Option<PathBuf> {
    let (b, p) = (canon(base), canon(path));
    (under(&p, &b) && !same_path(&p, &b))
        .then(|| p.components().skip(b.components().count()).collect())
}

fn chars(path: &Path) -> usize {
    path.to_string_lossy().chars().count()
}

/// `.md` files directly in `dir`, as the reader takes them: regular files, by name.
fn library_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| has_extension(p, &["md"]) && p.is_file())
        .collect();
    files.sort();
    files.truncate(LIBRARY_FILES_MAX);
    files
}

fn is_plain_file(path: &Path) -> Option<Result<u64, Verdict>> {
    match std::fs::symlink_metadata(path) {
        Err(_) => Some(Err(Verdict::Missing)),
        Ok(m) if m.file_type().is_symlink() || !m.is_file() => Some(Err(Verdict::NotAFile)),
        Ok(m) if m.len() > DIGEST_MAX_BYTES => Some(Err(Verdict::TooLarge)),
        Ok(m) => Some(Ok(m.len())),
    }
}

/// The first existing ancestor of `dir` that is not a folder: a file where a folder should go.
fn in_the_way(dir: &Path) -> Option<String> {
    for a in dir.ancestors() {
        match std::fs::symlink_metadata(a) {
            Ok(m) if m.is_dir() => return None,
            Ok(_) => {
                return Some(
                    a.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| a.display().to_string()),
                );
            }
            Err(_) => {}
        }
    }
    None
}

/// A row for `from` going to `to`, with everything the disk can say about it. Checks a row can
/// fail, in the order a person would want them said.
#[allow(clippy::too_many_arguments)]
fn make_row(
    kind: FileKind,
    from: &Path,
    to: &Path,
    title: &str,
    sections: &[String],
    why: String,
    taken: &[PathBuf],
    listed: Option<usize>,
) -> Row {
    let mut row = Row {
        kind,
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        title: title.to_string(),
        sections: sections.to_vec(),
        why,
        verdict: Verdict::Ready,
        ticked: false,
        len: 0,
        sha256: String::new(),
        outside: false,
    };
    row.verdict = match is_plain_file(from) {
        Some(Err(v)) => v,
        Some(Ok(len)) => {
            row.len = len;
            if let Some(n) = listed.filter(|&n| n != 1) {
                Verdict::Listed(n)
            } else if std::fs::symlink_metadata(to).is_ok()
                || taken.iter().any(|t| same_path(t, to))
            {
                Verdict::Taken
            } else if chars(to) > PATH_MAX_CHARS {
                Verdict::TooLong(chars(to))
            } else if let Some(name) = to.parent().and_then(in_the_way) {
                Verdict::InTheWay(name)
            } else {
                Verdict::Ready
            }
        }
        None => Verdict::Missing,
    };
    if !row.verdict.blocked() {
        match runs::digest_of(from) {
            Some(d) => row.sha256 = d,
            None => row.verdict = Verdict::Missing,
        }
    }
    row
}

/// The plan for one group: the notes it lists that are not where its folder and its headings put
/// them (`library::note_folder(home, sections)`). Nothing is written.
pub fn plan_group(library_dir: &Path, group: &library::Group) -> Plan {
    let label = format!("the notes of {}", group.name);
    let mut plan = Plan::empty(Start::Headings, library_dir, label);
    if !library_dir.is_absolute() {
        return plan.block("There is no library folder to move the notes within.");
    }
    let Some(home) = group.home.clone().filter(|h| !h.as_os_str().is_empty()) else {
        return plan.block(format!(
            "{} has no folder for its notes to go to. Set the group's folder first.",
            group.name
        ));
    };
    plan.group = Some(GroupRef {
        name: group.name.clone(),
        file: group.file.clone(),
        home: home.clone(),
    });
    if has_parent_dir(&home) || !home.is_absolute() {
        return plan.block(format!(
            "{} is not a folder this can move into.",
            home.display()
        ));
    }
    let (home_c, lib_c) = (canon(&home), canon(library_dir));
    if same_path(&home_c, &lib_c) || under(&lib_c, &home_c) {
        return plan.block(format!(
            "{} is the library folder or holds it, so the notes would be library files. \
             Choose another folder for the group.",
            home.display()
        ));
    }
    let listed = runs::list_library(library_dir);
    if !listed.complete {
        return plan.block(
            "The library could not be read in full, so there is no telling which lines list \
             which notes.",
        );
    }
    let mut taken: Vec<PathBuf> = Vec::new();
    let mut outside_home = false;
    for entry in &group.entries {
        let Some(name) = entry.path.file_name() else {
            continue;
        };
        let target = library::note_folder(&home, &entry.sections);
        if entry.path.parent().is_some_and(|d| same_path(d, &target)) {
            continue;
        }
        if entry.missing || !entry.path.exists() {
            plan.missing += 1;
            continue;
        }
        let to = target.join(name);
        let in_home = under(&canon(&entry.path), &home_c);
        outside_home |= !in_home;
        let why = if !in_home {
            "not in the group's folder".to_string()
        } else if entry.sections.is_empty() {
            "it has no section, so it belongs in the group's own folder".to_string()
        } else {
            format!("its section is {}", entry.sections.join(" \u{203A} "))
        };
        let n = runs::named_in(&listed, &entry.path).len();
        let mut row = make_row(
            FileKind::Note,
            &entry.path,
            &to,
            &entry.title,
            &entry.sections,
            why,
            &taken,
            Some(n),
        );
        taken.push(to);
        if !row.verdict.blocked() && !inside(library_dir, &home, &entry.path) {
            let dir = entry
                .path
                .parent()
                .map(|d| d.display().to_string())
                .unwrap_or_default();
            row.verdict = Verdict::Outside(dir);
            row.outside = true;
        }
        row.ticked = !row.verdict.blocked() && !row.outside;
        plan.rows.push(row);
    }
    plan.start = if outside_home {
        Start::Group
    } else {
        Start::Headings
    };
    plan.label = match plan.start {
        Start::Headings => format!("the notes of {} to match the headings", group.name),
        _ => format!("the notes of {} into their folder", group.name),
    };
    plan
}

/// The plan for the library itself: its library files, the notes they list that are inside the
/// old folder, and the records of earlier runs, from `old` into `new` (the current library
/// folder). Nothing outside `old` is a row.
pub fn plan_library(old: &Path, new: &Path) -> Plan {
    let label = format!("the library from {}", old.display());
    let mut plan = Plan::empty(Start::Library, new, label);
    plan.from_library = Some(old.to_path_buf());
    if !old.is_absolute() || !new.is_absolute() {
        return plan.block("The library folders are not known.");
    }
    if !old.is_dir() {
        return plan.block(format!(
            "{} is not a folder, so there is nothing to move.",
            old.display()
        ));
    }
    let (oc, nc) = (canon(old), canon(new));
    if same_path(&oc, &nc) {
        return plan.block("The library is in that folder already.");
    }
    if under(&nc, &oc) || under(&oc, &nc) {
        return plan.block(
            "One of the two folders is inside the other, so the notes could not be told \
             from the ones that are there.",
        );
    }
    let listed = runs::list_library(old);
    if !listed.complete {
        return plan.block(
            "The old library could not be read in full, so there is no telling which lines \
             list which notes.",
        );
    }
    let mut taken: Vec<PathBuf> = Vec::new();
    // The library files.
    let mut files = library_files(old);
    if files.len() > library::MAX_FILES {
        plan.omitted = files.len() - library::MAX_FILES;
        files.truncate(library::MAX_FILES);
    }
    for file in &files {
        let Some(name) = file.file_name() else {
            continue;
        };
        let to = new.join(name);
        let stem = file
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut row = make_row(
            FileKind::Library,
            file,
            &to,
            &stem,
            &[],
            "a library file".to_string(),
            &taken,
            None,
        );
        // A note that lives directly in the library folder is a library file as far as the
        // reader goes, and is listed by another: its links are the lines that are rewritten.
        let n = runs::named_in(&listed, file).len();
        if n > 1 && !row.verdict.blocked() {
            row.verdict = Verdict::Listed(n);
        }
        taken.push(to);
        row.ticked = !row.verdict.blocked();
        plan.rows.push(row);
    }
    // The notes inside the old folder that those files list.
    let lib = library::read(old);
    let mut seen: Vec<PathBuf> = files.clone();
    for group in &lib.groups {
        for entry in &group.entries {
            let Some(rel) = below(old, &entry.path) else {
                continue;
            };
            if seen.iter().any(|s| same_path(s, &entry.path)) || rel.starts_with(runs::RUNS_DIR) {
                continue;
            }
            seen.push(entry.path.clone());
            if !entry.path.exists() {
                plan.missing += 1;
                continue;
            }
            let to = new.join(&rel);
            let n = runs::named_in(&listed, &entry.path).len();
            let sections = entry.sections.clone();
            let mut row = make_row(
                FileKind::Note,
                &entry.path,
                &to,
                &entry.title,
                &sections,
                if sections.is_empty() {
                    "it is in the library's folder".to_string()
                } else {
                    format!("its section is {}", sections.join(" \u{203A} "))
                },
                &taken,
                Some(n),
            );
            taken.push(to);
            row.ticked = !row.verdict.blocked();
            plan.rows.push(row);
        }
    }
    // The records of earlier runs, so "AI runs…" keeps its history in the new folder.
    let runs_dir = old.join(runs::RUNS_DIR);
    let mut records: Vec<PathBuf> = std::fs::read_dir(&runs_dir)
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| has_extension(p, &["json", "done"]) && p.is_file())
                .collect()
        })
        .unwrap_or_default();
    records.sort();
    records.truncate(RECORDS_MAX);
    for file in &records {
        let Some(name) = file.file_name() else {
            continue;
        };
        let to = new.join(runs::RUNS_DIR).join(name);
        let mut row = make_row(
            FileKind::Record,
            file,
            &to,
            &name.to_string_lossy(),
            &[],
            "the record of an earlier run".to_string(),
            &taken,
            None,
        );
        taken.push(to);
        row.ticked = !row.verdict.blocked();
        plan.rows.push(row);
    }
    // What stays behind, so the footer can say so.
    let (mut count, mut stack, mut visited) = (0usize, vec![old.to_path_buf()], 0usize);
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            visited += 1;
            if visited > WALK_MAX {
                plan.others_capped = true;
                stack.clear();
                break;
            }
            let Ok(t) = e.file_type() else { continue };
            if t.is_dir() {
                stack.push(e.path());
            } else if t.is_file() {
                count += 1;
            }
        }
    }
    plan.others = count.saturating_sub(plan.rows.len());
    plan
}

// ---- the disk, as a trait so a test can make any step fail --------------------------------------

/// What the machine does to the disk. [`Disk`] is the real one; a test wraps it to make one step
/// fail and asserts what is left.
pub trait Files {
    /// Renames `from` to `to`, never replacing a file at `to`. `ErrorKind::CrossesDevices` says
    /// the two are on different volumes.
    fn rename_new(&mut self, from: &Path, to: &Path) -> io::Result<()>;
    /// Creates `to` holding `bytes`, flushed, and refuses a name that is taken.
    fn create_new(&mut self, to: &Path, bytes: &[u8]) -> io::Result<()>;
    /// A regular file, read whole, up to [`DIGEST_MAX_BYTES`].
    fn read(&mut self, path: &Path) -> io::Result<Vec<u8>>;
    fn remove_file(&mut self, path: &Path) -> io::Result<()>;
    fn create_dir_all(&mut self, path: &Path) -> io::Result<()>;
    /// Removes a folder only while it is empty.
    fn remove_dir(&mut self, path: &Path) -> io::Result<()>;
}

/// The real disk. `rename` is the platform's rename that refuses an existing name: the window
/// passes the one that asks the system not to replace; [`Disk::portable`] looks first.
pub struct Disk {
    pub rename: fn(&Path, &Path) -> io::Result<()>,
}

impl Disk {
    /// A rename that looks before it moves. The pair is not atomic against another program
    /// creating the name in between, as `create_new`'s is not; the window uses the atomic one.
    pub fn portable() -> Disk {
        Disk {
            rename: |from, to| {
                if std::fs::symlink_metadata(to).is_ok() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "the file already exists",
                    ));
                }
                std::fs::rename(from, to)
            },
        }
    }
}

impl Files for Disk {
    fn rename_new(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        (self.rename)(from, to)
    }

    fn create_new(&mut self, to: &Path, bytes: &[u8]) -> io::Result<()> {
        mdshared_app::files::create_new(to, bytes)
    }

    fn read(&mut self, path: &Path) -> io::Result<Vec<u8>> {
        read_regular(path, DIGEST_MAX_BYTES, false)
            .ok_or_else(|| io::Error::other("it is not a plain file, or it is too large"))
    }

    fn remove_file(&mut self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn create_dir_all(&mut self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn remove_dir(&mut self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir(path)
    }
}

// ---- library lines: what changes in a library file when a note moves --------------------------

/// What a line is: a note's link, or the group's `home:` line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Note,
    Home,
}

impl ChangeKind {
    fn as_str(self) -> &'static str {
        match self {
            ChangeKind::Note => "note",
            ChangeKind::Home => "home",
        }
    }

    fn parse(text: &str) -> Option<ChangeKind> {
        match text {
            "note" => Some(ChangeKind::Note),
            "home" => Some(ChangeKind::Home),
            _ => None,
        }
    }
}

/// One line of a library file, as it was and as it becomes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub kind: ChangeKind,
    pub sections: Vec<String>,
    pub before: String,
    pub after: String,
}

/// A note as the file must list it once it is written: its title and sections as they were, and
/// the path the line must lead to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Want {
    pub title: String,
    pub sections: Vec<String>,
    pub path: PathBuf,
}

/// What a library file must read back as. A count is not enough: every note, in order, under its
/// section, leading to where the note is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Expected {
    pub entries: Vec<Want>,
    pub home: Option<PathBuf>,
    /// When set, the new text must be exactly this: a line put back to what it said.
    pub exact: Option<String>,
}

/// The text with every edit made: each edit replaces the first line that is exactly its `before`
/// (a note's line only under its own sections), keeping the file's line ending. `None` when a
/// line is not found, so a file that changed since the plan was made is refused, not guessed at.
pub fn with_edits(text: &str, edits: &[Edit], base: &Path) -> Option<String> {
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_string).collect();
    let parsed = library::parse(text, base);
    let mut used = vec![false; lines.len()];
    for edit in edits {
        let at = lines.iter().enumerate().position(|(j, line)| {
            let body = line.trim_end_matches(['\r', '\n']);
            !used[j]
                && body.trim_end() == edit.before.trim_end()
                && match edit.kind {
                    ChangeKind::Home => true,
                    ChangeKind::Note => parsed
                        .entries
                        .iter()
                        .any(|e| e.line == j + 1 && sections_eq(&e.sections, &edit.sections)),
                }
        })?;
        let body_len = lines[at].trim_end_matches(['\r', '\n']).len();
        let eol = lines[at][body_len..].to_string();
        lines[at] = format!("{}{}", edit.after, eol);
        used[at] = true;
    }
    Some(lines.concat())
}

/// Whether `new` reads as `expect` says.
fn reads_as(new: &Parsed, expect: &Expected) -> bool {
    new.entries.len() == expect.entries.len()
        && new.entries.iter().zip(&expect.entries).all(|(e, w)| {
            e.title == w.title && e.sections == w.sections && same_path(&e.path, &w.path)
        })
        && match (&new.home, &expect.home) {
            (Some(a), Some(b)) => same_path(a, b),
            (None, None) => true,
            _ => false,
        }
}

/// The text with the edits made, checked against what it must read back as: what a gate
/// writes, so the window's and the tests' are one. `base` is the folder the file's paths resolve
/// against once it is where it will be.
pub fn apply_edits(
    text: &str,
    edits: &[Edit],
    expect: &Expected,
    base: &Path,
) -> Result<String, String> {
    let new = with_edits(text, edits, base).ok_or_else(|| {
        "a line it names is not in the file as it was when the plan was made".to_string()
    })?;
    match &expect.exact {
        Some(exact) => {
            if &new != exact {
                return Err("the file would not read back as it was".to_string());
            }
        }
        None => {
            if !reads_as(&library::parse(&new, base), expect) {
                return Err("the file would not read back as its notes".to_string());
            }
        }
    }
    Ok(new)
}

/// Where each moved file is going, for working out what a library file must say.
struct Mapping {
    /// The row, where the file was and where it goes: only the files that did move.
    pairs: Vec<(usize, PathBuf, PathBuf)>,
    /// The library folder the files leave and the one they go to, for a `home:` line.
    dirs: Option<(PathBuf, PathBuf)>,
}

impl Mapping {
    fn path(&self, p: &Path) -> (PathBuf, Option<usize>) {
        match self.pairs.iter().find(|(_, from, _)| same_path(from, p)) {
            Some((i, _, to)) => (to.clone(), Some(*i)),
            None => (p.to_path_buf(), None),
        }
    }

    fn dir(&self, p: &Path) -> PathBuf {
        if let Some((from, to)) = &self.dirs
            && let Some(rel) = below(from, p)
        {
            return to.join(rel);
        }
        p.to_path_buf()
    }
}

/// What one library file needs.
struct Planned {
    edits: Vec<Edit>,
    /// The row each edit belongs to, in step with `edits`.
    edit_rows: Vec<Option<usize>>,
    expect: Expected,
    /// The rows whose notes this file lists: they stay moved while this file says so.
    rows: BTreeSet<usize>,
    /// The folder the file's paths resolve against once it is where it will be.
    base: PathBuf,
}

/// The lines a library file needs so that every note it lists, and its home, lead to where they
/// are once the files in `map` have moved. `text` is the file as it is; `base_before` is the
/// folder its paths resolve against now, `base_after` the one they will once it is where it will
/// be. `None` when it needs nothing.
fn plan_edits(
    text: &str,
    base_before: &Path,
    base_after: &Path,
    map: &Mapping,
    prefer: &[Edit],
    own: Option<usize>,
) -> Result<Option<Planned>, String> {
    let before = library::parse(text, base_before);
    let after = library::parse(text, base_after);
    if before.entries.len() != after.entries.len() {
        return Err("the file reads differently where it is going".to_string());
    }
    let mut new_text = text.to_string();
    let mut rows = BTreeSet::new();
    let mut entries = Vec::new();
    let mut by_line: Vec<(usize, Option<usize>)> = Vec::new();
    let line_of = |t: &str, line: usize| {
        t.lines()
            .nth(line.saturating_sub(1))
            .unwrap_or("")
            .to_string()
    };
    for (b, a) in before.entries.iter().zip(&after.entries) {
        let (want, hit) = map.path(&b.path);
        if let Some(i) = hit {
            rows.insert(i);
        }
        by_line.push((b.line, hit));
        // A line this run wrote goes back to the words it had, when those lead where the note
        // is now, whether or not the words it has now would still lead there.
        let now = line_of(&new_text, b.line);
        let old_words = prefer
            .iter()
            .filter(|p| p.kind == ChangeKind::Note && sections_eq(&p.sections, &b.sections))
            .find(|p| p.after.trim_end() == now.trim_end())
            .and_then(|p| set_line(&new_text, b.line, &p.before))
            .filter(|t| {
                library::parse(t, base_after)
                    .entries
                    .iter()
                    .any(|e| e.line == b.line && same_path(&e.path, &want))
            });
        if let Some(t) = old_words {
            new_text = t;
        } else if !same_path(&a.path, &want) {
            new_text = library::set_note_path(&new_text, b.line, &written(base_after, &want))
                .ok_or_else(|| {
                    format!(
                        "the line for \u{2018}{}\u{2019} could not be rewritten",
                        b.title
                    )
                })?;
        }
        entries.push(Want {
            title: b.title.clone(),
            sections: b.sections.clone(),
            path: want,
        });
    }
    let home = before.home.as_ref().map(|h| map.dir(h));
    if let (Some(want), Some(now)) = (&home, &after.home) {
        let old_words = text
            .lines()
            .enumerate()
            .find_map(|(j, l)| {
                let p = prefer
                    .iter()
                    .find(|p| p.kind == ChangeKind::Home && p.after.trim_end() == l.trim_end())?;
                set_line(&new_text, j + 1, &p.before)
            })
            .filter(|t| {
                library::parse(t, base_after)
                    .home
                    .is_some_and(|h| same_path(&h, want))
            });
        if let Some(t) = old_words {
            new_text = t;
        } else if !same_path(now, want) {
            new_text = library::set_group_home(&new_text, &written(base_after, want))
                .ok_or_else(|| "the group's folder line could not be rewritten".to_string())?;
        }
    }
    if new_text == text {
        return Ok(None);
    }
    let old_lines: Vec<&str> = text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();
    if old_lines.len() != new_lines.len() {
        return Err("the file would change in more than its lines".to_string());
    }
    let mut edits = Vec::new();
    let mut edit_rows = Vec::new();
    for (j, (o, n)) in old_lines.iter().zip(&new_lines).enumerate() {
        if o == n {
            continue;
        }
        match before.entries.iter().find(|e| e.line == j + 1) {
            Some(e) => {
                edits.push(Edit {
                    kind: ChangeKind::Note,
                    sections: e.sections.clone(),
                    before: (*o).to_string(),
                    after: (*n).to_string(),
                });
                edit_rows.push(
                    by_line
                        .iter()
                        .find(|(l, _)| *l == j + 1)
                        .and_then(|(_, r)| *r),
                );
            }
            None => {
                edits.push(Edit {
                    kind: ChangeKind::Home,
                    sections: Vec::new(),
                    before: (*o).to_string(),
                    after: (*n).to_string(),
                });
                edit_rows.push(own);
            }
        }
    }
    let expect = Expected {
        entries,
        home,
        exact: None,
    };
    // The dry run: what a gate will do, done here, so the refusal comes before any file moves.
    apply_edits(text, &edits, &expect, base_after)?;
    Ok(Some(Planned {
        edits,
        edit_rows,
        expect,
        rows,
        base: base_after.to_path_buf(),
    }))
}

/// The text with line `line` (1-based) made `new`, keeping its line ending.
fn set_line(text: &str, line: usize, new: &str) -> Option<String> {
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_string).collect();
    let at = lines.get_mut(line.checked_sub(1)?)?;
    let body_len = at.trim_end_matches(['\r', '\n']).len();
    let eol = at[body_len..].to_string();
    *at = format!("{new}{eol}");
    Some(lines.concat())
}

/// What the window's gate for a move does: the library writer every other command uses. `free`
/// says no to a file open with unsaved changes; `edit` rewrites one library file in one atomic
/// write with [`apply_edits`].
pub trait MoveGate {
    fn free(&mut self, file: &Path) -> Result<(), String>;
    fn edit(
        &mut self,
        file: &Path,
        base: &Path,
        edits: &[Edit],
        expect: &Expected,
    ) -> Result<(), String>;
}

// ---- the record --------------------------------------------------------------------------------

/// One file a move carried, as its record holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovedFile {
    pub kind: FileKind,
    pub from: PathBuf,
    pub to: PathBuf,
    /// This run moved it: set only after it was moved, so a run that stopped before holds files
    /// it never touched.
    pub done: bool,
    pub len: u64,
    /// The digest of what was moved, so undo moves it back only while it is the same file.
    pub sha256: String,
    /// The plan said it lived outside the library's own area, and the owner ticked it anyway.
    pub outside: bool,
    pub sections: Vec<String>,
}

/// One line of a library file the run rewrote. Undo reads the file, so a line that is neither
/// `after` (to put back) nor `before` (already so) is the owner's, and is kept with its note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// The library file as it was after the run: where it is.
    pub file: PathBuf,
    pub edit: Edit,
    /// The file the line belongs to, as an index into [`Moved::files`].
    pub row: Option<usize>,
}

/// What a [`Kind::Move`] record holds besides the header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moved {
    pub start: Start,
    pub library_from: Option<PathBuf>,
    pub files: Vec<MovedFile>,
    pub changes: Vec<Change>,
    /// The folders the run removed because it left them empty, so undo can make them again, and
    /// nothing else.
    pub emptied: Vec<PathBuf>,
}

/// What a record says of itself in "AI runs…": `Move · the notes of Work · 5 files`.
pub fn headline(record: &Record) -> String {
    let n = record
        .moved
        .as_ref()
        .map_or(0, |m| m.files.iter().filter(|f| f.done).count());
    format!(
        "Move \u{00B7} {} \u{00B7} {}",
        record.instruction,
        if n == 1 {
            "1 file".to_string()
        } else {
            format!("{n} files")
        }
    )
}

fn stored_opt(path: &Path, library: &Path) -> String {
    if path.as_os_str().is_empty() {
        String::new()
    } else {
        stored(path, library)
    }
}

fn resolved_opt(text: &str, library: &Path) -> PathBuf {
    if text.is_empty() {
        PathBuf::new()
    } else {
        runs::resolved(text, library)
    }
}

fn strings_json(items: &[String]) -> String {
    let parts: Vec<String> = items.iter().map(|s| format!("\"{}\"", esc(s))).collect();
    format!("[{}]", parts.join(", "))
}

/// The JSON of a move's record. Paths inside the library folder are relative to it.
pub(crate) fn record_json(rec: &Record, library: &Path) -> String {
    let none = Moved {
        start: Start::Library,
        library_from: None,
        files: Vec::new(),
        changes: Vec::new(),
        emptied: Vec::new(),
    };
    let m = rec.moved.as_ref().unwrap_or(&none);
    let mut s = String::from("{\n");
    s.push_str(&format!("  \"version\": {},\n", rec.version));
    s.push_str("  \"kind\": \"move\",\n");
    s.push_str(&format!("  \"started\": \"{}\",\n", esc(&rec.started)));
    s.push_str(&format!("  \"state\": \"{}\",\n", esc(rec.state.as_str())));
    s.push_str(&format!(
        "  \"instruction\": \"{}\",\n",
        esc(&rec.instruction)
    ));
    s.push_str(&format!(
        "  \"group\": {{\"name\": \"{}\", \"file\": \"{}\", \"home\": \"{}\"}},\n",
        esc(&rec.group.name),
        esc(&stored_opt(&rec.group.file, library)),
        esc(&stored_opt(&rec.group.home, library))
    ));
    let folders: Vec<String> = rec
        .folders
        .iter()
        .map(|f| format!("\"{}\"", esc(&stored(f, library))))
        .collect();
    s.push_str(&format!("  \"folders\": [{}],\n", folders.join(", ")));
    s.push_str("  \"move\": {\n");
    s.push_str(&format!("    \"start\": \"{}\",\n", m.start.as_str()));
    match &m.library_from {
        Some(p) => s.push_str(&format!(
            "    \"library_from\": \"{}\",\n",
            esc(&stored(p, library))
        )),
        None => s.push_str("    \"library_from\": null,\n"),
    }
    s.push_str("    \"files\": [");
    for (i, f) in m.files.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "\n      {{\"kind\": \"{}\", \"from\": \"{}\", \"to\": \"{}\", \"done\": {}, \"len\": {}, \"sha256\": \"{}\", \"outside\": {}, \"sections\": {}}}",
            f.kind.as_str(),
            esc(&stored(&f.from, library)),
            esc(&stored(&f.to, library)),
            f.done,
            f.len,
            esc(&f.sha256),
            f.outside,
            strings_json(&f.sections)
        ));
    }
    s.push_str(if m.files.is_empty() {
        "],\n"
    } else {
        "\n    ],\n"
    });
    s.push_str("    \"changes\": [");
    for (i, c) in m.changes.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let row = c.row.map_or_else(|| "null".to_string(), |r| r.to_string());
        s.push_str(&format!(
            "\n      {{\"file\": \"{}\", \"kind\": \"{}\", \"sections\": {}, \"before\": \"{}\", \"after\": \"{}\", \"row\": {}}}",
            esc(&stored(&c.file, library)),
            c.edit.kind.as_str(),
            strings_json(&c.edit.sections),
            esc(&c.edit.before),
            esc(&c.edit.after),
            row
        ));
    }
    s.push_str(if m.changes.is_empty() {
        "],\n"
    } else {
        "\n    ],\n"
    });
    let emptied: Vec<String> = m
        .emptied
        .iter()
        .map(|f| format!("\"{}\"", esc(&stored(f, library))))
        .collect();
    s.push_str(&format!("    \"emptied\": [{}]\n", emptied.join(", ")));
    s.push_str("  }\n}\n");
    s
}

/// A move's record read back. Strict where a split's is lenient: an entry that does not parse
/// makes the whole file "not a record this version can read", because an index into the files
/// that shifted would tie a line to the wrong note.
pub(crate) fn record_from_json(
    json: &Json,
    library: &Path,
    head: (u32, String, u64, State, String),
) -> Option<Record> {
    let (version, started, started_secs, state, instruction) = head;
    let body = json.get("move")?;
    let start = Start::parse(body.get("start")?.str()?)?;
    let library_from = match body.get("library_from") {
        Some(j) => j.str().map(|s| runs::resolved(s, library)),
        None => None,
    };
    let strings = |j: &Json| -> Option<Vec<String>> {
        j.arr()?
            .iter()
            .map(|s| s.str().map(str::to_string))
            .collect()
    };
    let files: Vec<MovedFile> = body
        .get("files")?
        .arr()?
        .iter()
        .map(|f| {
            Some(MovedFile {
                kind: FileKind::parse(f.get("kind")?.str()?)?,
                from: runs::resolved(f.get("from")?.str()?, library),
                to: runs::resolved(f.get("to")?.str()?, library),
                done: f.get("done").and_then(Json::bool).unwrap_or(false),
                len: f.get("len").and_then(Json::uint).unwrap_or(0),
                sha256: f
                    .get("sha256")
                    .and_then(Json::str)
                    .unwrap_or_default()
                    .to_string(),
                outside: f.get("outside").and_then(Json::bool).unwrap_or(false),
                sections: f.get("sections").and_then(strings).unwrap_or_default(),
            })
        })
        .collect::<Option<_>>()?;
    let changes: Vec<Change> = body
        .get("changes")?
        .arr()?
        .iter()
        .map(|c| {
            let row = match c.get("row") {
                Some(r) if r.str().is_none() => r.usize().filter(|&i| i < files.len()),
                _ => None,
            };
            Some(Change {
                file: runs::resolved(c.get("file")?.str()?, library),
                edit: Edit {
                    kind: ChangeKind::parse(c.get("kind")?.str()?)?,
                    sections: c.get("sections").and_then(strings).unwrap_or_default(),
                    before: c.get("before")?.str()?.to_string(),
                    after: c.get("after")?.str()?.to_string(),
                },
                row,
            })
        })
        .collect::<Option<_>>()?;
    let emptied: Vec<PathBuf> = body
        .get("emptied")
        .and_then(Json::arr)
        .map(|a| {
            a.iter()
                .filter_map(Json::str)
                .map(|s| runs::resolved(s, library))
                .collect()
        })
        .unwrap_or_default();
    let g = json.get("group");
    let text = |k: &str| {
        g.and_then(|g| g.get(k))
            .and_then(Json::str)
            .unwrap_or_default()
            .to_string()
    };
    let folders = json
        .get("folders")
        .and_then(Json::arr)
        .map(|a| {
            a.iter()
                .filter_map(Json::str)
                .map(|s| runs::resolved(s, library))
                .collect()
        })
        .unwrap_or_default();
    Some(Record {
        version,
        kind: Kind::Move,
        started,
        started_secs,
        state,
        instruction,
        sent: None,
        source: runs::Source {
            path: PathBuf::new(),
            len: 0,
            sha256: String::new(),
            recycled: false,
        },
        group: runs::Group {
            name: text("name"),
            file: resolved_opt(&text("file"), library),
            created: false,
            home: resolved_opt(&text("home"), library),
        },
        folders,
        notes: Vec::new(),
        pictures: Vec::new(),
        sections_added: Vec::new(),
        moved: Some(Moved {
            start,
            library_from,
            files,
            changes,
            emptied,
        }),
    })
}

// ---- the report --------------------------------------------------------------------------------

/// What a write, or an undo, did.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub record: Option<Record>,
    pub record_path: Option<PathBuf>,
    /// Nothing was done, and why ("The run could not start: …" for a write).
    pub start_error: Option<String>,
    /// The files that moved and stay moved: where they were, and where they are.
    pub moved: Vec<(PathBuf, PathBuf)>,
    /// Files that could not be moved. They are where they were, and nothing names them anew.
    pub failed: Vec<Failure>,
    /// The library files rewritten.
    pub edited: Vec<PathBuf>,
    /// How many lines were rewritten.
    pub lines: usize,
    /// A library file was refused or could not be written, so every move was put back.
    pub rolled_back: Option<Failure>,
    pub put_back: Vec<PathBuf>,
    /// Files that could not be put back, and files the rollback could not restore: they are
    /// where the message says, and the record says so too.
    pub not_put_back: Vec<Failure>,
    /// The old copy of a file that crossed a volume could not be removed: it is in two places.
    pub leftovers: Vec<Failure>,
    pub emptied: Vec<PathBuf>,
    /// Files an undo left where they are, and why.
    pub kept: Vec<(PathBuf, String)>,
    pub closed: bool,
    pub close_error: Option<String>,
}

impl Report {
    pub fn state(&self) -> Option<State> {
        self.record.as_ref().map(|r| r.state)
    }
}

// ---- running a move ------------------------------------------------------------------------------

/// One file as the machine moves it.
#[derive(Clone, Debug)]
struct ExecRow {
    from: PathBuf,
    to: PathBuf,
    sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Via {
    Renamed,
    Copied,
}

/// What the machine needs besides the rows and the disk.
struct Setup<'a> {
    /// The library folder the files leave and the one they go to, for `home:` lines.
    dirs: Option<(PathBuf, PathBuf)>,
    /// The library files, where they are now.
    library_files: Vec<PathBuf>,
    /// Lines a record says were written, so that putting them back uses the words they had.
    prefer: Vec<Edit>,
    /// Whether a folder left empty may be removed.
    removable: &'a dyn Fn(&Path) -> bool,
}

/// One library file's rewrite: where the file is, what it said, what it needs.
struct Job {
    at: PathBuf,
    text: String,
    planned: Planned,
    /// The row of the library file itself, when it moved.
    own: Option<usize>,
}

/// The record while it is written: the file is written again as the run goes.
struct Journal {
    record: Record,
    path: PathBuf,
    library: PathBuf,
}

impl Journal {
    fn flush(&self) {
        let _ = self.record.close(&self.path, &self.library);
    }

    fn moved(&mut self) -> &mut Moved {
        self.record.moved.get_or_insert_with(|| Moved {
            start: Start::Library,
            library_from: None,
            files: Vec::new(),
            changes: Vec::new(),
            emptied: Vec::new(),
        })
    }

    fn note(&self, line: &str) {
        self.record.note_done(&self.path, line);
    }
}

/// Moves the rows, rewrites the library files and tidies up. A write runs the rows as planned
/// (and `journal` is its record, row `i` of the rows being file `i` of the record); undo runs
/// them the other way, with no journal.
fn run(
    rows: &[ExecRow],
    setup: &Setup,
    gate: &mut dyn MoveGate,
    files: &mut dyn Files,
    mut journal: Option<&mut Journal>,
    report: &mut Report,
) {
    // The folders the files go to; the ones that were not there are noted as made by this run.
    let mut made: Vec<PathBuf> = Vec::new();
    let mut no_folder: Vec<(PathBuf, String)> = Vec::new();
    let mut dests: Vec<PathBuf> = rows
        .iter()
        .filter_map(|r| r.to.parent().map(Path::to_path_buf))
        .collect();
    dests.sort_by_key(|d| d.components().count());
    dests.dedup_by(|a, b| same_path(a, b));
    for dir in &dests {
        let mut missing: Vec<PathBuf> = Vec::new();
        let mut at = Some(dir.as_path());
        while let Some(d) = at {
            if d.symlink_metadata().is_ok() {
                break;
            }
            missing.push(d.to_path_buf());
            at = d.parent();
        }
        let result = files.create_dir_all(dir);
        for d in missing.into_iter().rev() {
            if d.is_dir() && !made.iter().any(|m| same_path(m, &d)) {
                made.push(d);
            }
        }
        if let Err(e) = result {
            no_folder.push((dir.clone(), e.to_string()));
        }
    }
    if let Some(j) = journal.as_deref_mut() {
        j.record.folders = made.clone();
        j.flush();
    }

    // The record says what the run is about to do before it does it: a run that dies half-way
    // leaves files moved that no record names otherwise. Undo takes each file by what is on disk
    // (a file that is not at its new place is gone, one at its old place too is kept), so a
    // file the run never reached costs a line in its sentence and nothing else. Whatever could
    // not be moved is marked again below.
    if let Some(j) = journal.as_deref_mut() {
        for f in &mut j.moved().files {
            f.done = true;
        }
        j.flush();
    }

    // The files, one at a time; one that cannot move stays where it is and is reported.
    let mut via: Vec<Option<Via>> = vec![None; rows.len()];
    for (i, row) in rows.iter().enumerate() {
        if let Some((dir, why)) = row
            .to
            .parent()
            .and_then(|p| no_folder.iter().find(|(d, _)| same_path(d, p)))
        {
            if let Some(f) = journal
                .as_deref_mut()
                .and_then(|j| j.moved().files.get_mut(i))
            {
                f.done = false;
            }
            report.failed.push(Failure {
                path: row.from.clone(),
                why: format!("{} could not be made: {why}", dir.display()),
            });
            continue;
        }
        match move_one(row, files) {
            Ok(v) => {
                via[i] = Some(v);
                if let Some(j) = journal.as_deref_mut() {
                    j.note(&format!("moved {}", shown(&row.to)));
                }
            }
            Err(why) => {
                if let Some(f) = journal
                    .as_deref_mut()
                    .and_then(|j| j.moved().files.get_mut(i))
                {
                    f.done = false;
                }
                report.failed.push(Failure {
                    path: row.from.clone(),
                    why,
                });
            }
        }
    }
    if let Some(j) = journal.as_deref_mut() {
        j.flush();
    }
    let moved_rows: Vec<usize> = (0..rows.len()).filter(|&i| via[i].is_some()).collect();
    let map = Mapping {
        pairs: moved_rows
            .iter()
            .map(|&i| (i, rows[i].from.clone(), rows[i].to.clone()))
            .collect(),
        dirs: setup.dirs.clone(),
    };

    // What each library file must say, worked out against where the files really are, and
    // recorded before any is written, so a run that stops half-way can be undone from the lines.
    let mut jobs: Vec<Job> = Vec::new();
    let mut failure: Option<Failure> = None;
    for file in &setup.library_files {
        let own = moved_rows
            .iter()
            .copied()
            .find(|&i| same_path(&rows[i].from, file));
        let at = own.map_or_else(|| file.clone(), |i| rows[i].to.clone());
        // A library file that is not text lists nothing that can be rewritten.
        let Ok(text) = std::fs::read_to_string(&at) else {
            continue;
        };
        let base_before = file.parent().unwrap_or(Path::new(""));
        let base_after = at.parent().unwrap_or(Path::new(""));
        match plan_edits(&text, base_before, base_after, &map, &setup.prefer, own) {
            Ok(Some(mut planned)) => {
                if let Some(i) = own {
                    planned.rows.insert(i);
                }
                jobs.push(Job {
                    at,
                    text,
                    planned,
                    own,
                });
            }
            Ok(None) => {}
            Err(why) => {
                failure = Some(Failure {
                    path: file.clone(),
                    why,
                });
                break;
            }
        }
    }
    if let Some(j) = journal.as_deref_mut() {
        j.moved().changes = jobs
            .iter()
            .flat_map(|job| {
                job.planned
                    .edits
                    .iter()
                    .zip(&job.planned.edit_rows)
                    .map(|(e, row)| Change {
                        file: job.at.clone(),
                        edit: e.clone(),
                        row: *row,
                    })
            })
            .collect();
        j.flush();
    }
    let mut applied: Vec<usize> = Vec::new();
    if failure.is_none() {
        for (k, job) in jobs.iter().enumerate() {
            match gate.edit(
                &job.at,
                &job.planned.base,
                &job.planned.edits,
                &job.planned.expect,
            ) {
                Ok(()) => {
                    applied.push(k);
                    report.edited.push(job.at.clone());
                    report.lines += job.planned.edits.len();
                }
                Err(why) => {
                    failure = Some(Failure {
                        path: job.at.clone(),
                        why,
                    });
                    break;
                }
            }
        }
    }
    if let Some(failure) = failure {
        rollback(rows, &via, &jobs, &applied, gate, files, report);
        report.rolled_back = Some(failure);
        report.edited.clear();
        report.lines = 0;
        // The folders this run made for files that are back are left empty: they go again.
        if report.not_put_back.is_empty() {
            let mut gone = made.clone();
            gone.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
            for dir in gone {
                let _ = files.remove_dir(&dir);
            }
            if let Some(j) = journal.as_deref_mut() {
                j.record.folders.clear();
            }
        }
        if let Some(j) = journal.as_deref_mut() {
            for (i, row) in rows.iter().enumerate() {
                let stuck = report
                    .moved
                    .iter()
                    .any(|(from, _)| same_path(from, &row.from));
                if let Some(f) = j.moved().files.get_mut(i) {
                    f.done = stuck;
                }
            }
            if report.not_put_back.is_empty() {
                j.moved().changes.clear();
            }
            j.flush();
        }
        return;
    }

    // A library file that was rewritten is no longer the file that moved: the record keeps the
    // digest of what it is now, so undo can tell it from a file the owner has changed since.
    if let Some(j) = journal.as_deref_mut() {
        for job in &jobs {
            if let Some(i) = job.own
                && let Some(d) = runs::digest_of(&job.at)
                && let Some(f) = j.moved().files.get_mut(i)
            {
                f.sha256 = d;
            }
        }
        j.flush();
    }

    // The old copy of what crossed a volume, now that the lines name the new one.
    for (i, row) in rows.iter().enumerate() {
        if via[i] != Some(Via::Copied) {
            continue;
        }
        let same = files
            .read(&row.from)
            .is_ok_and(|b| mdshared_app::update::sha256_hex(&b).eq_ignore_ascii_case(&row.sha256));
        if !same {
            report.leftovers.push(Failure {
                path: row.from.clone(),
                why: "it changed while it was being moved; the old copy is kept".to_string(),
            });
        } else if let Err(e) = files.remove_file(&row.from) {
            report.leftovers.push(Failure {
                path: row.from.clone(),
                why: format!("the old copy could not be removed: {e}"),
            });
        }
    }
    for &i in &moved_rows {
        report
            .moved
            .push((rows[i].from.clone(), rows[i].to.clone()));
    }

    // Folders the move left empty: those, and what they were inside while that is allowed. A
    // folder that holds anything is left, because `remove_dir` refuses it.
    let mut candidates: Vec<PathBuf> = Vec::new();
    for &i in &moved_rows {
        if report
            .leftovers
            .iter()
            .any(|f| same_path(&f.path, &rows[i].from))
        {
            continue;
        }
        let mut at = rows[i].from.parent();
        while let Some(dir) = at {
            if !(setup.removable)(dir) {
                break;
            }
            if !candidates.iter().any(|c| same_path(c, dir)) {
                candidates.push(dir.to_path_buf());
            }
            at = dir.parent();
        }
    }
    candidates.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    if let Some(j) = journal.as_deref_mut() {
        j.moved().emptied = candidates.clone();
        j.flush();
    }
    for dir in candidates {
        let real = dir
            .symlink_metadata()
            .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink());
        if real && files.remove_dir(&dir).is_ok() {
            report.emptied.push(dir);
        }
    }
    if let Some(j) = journal {
        j.moved().emptied = report.emptied.clone();
    }
}

/// Moves one file: a rename, or across a volume a copy that is read back and compared. The
/// source of a copy is removed by the caller once the library says where the file is.
fn move_one(row: &ExecRow, files: &mut dyn Files) -> Result<Via, String> {
    if std::fs::symlink_metadata(&row.to).is_ok() {
        return Err("there is already a file called this".to_string());
    }
    match files.rename_new(&row.from, &row.to) {
        Ok(()) => Ok(Via::Renamed),
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            let bytes = files.read(&row.from).map_err(|e| e.to_string())?;
            if !mdshared_app::update::sha256_hex(&bytes).eq_ignore_ascii_case(&row.sha256) {
                return Err("it has changed since the plan was made".to_string());
            }
            files
                .create_new(&row.to, &bytes)
                .map_err(|e| e.to_string())?;
            match files.read(&row.to) {
                Ok(back) if back == bytes => Ok(Via::Copied),
                other => {
                    let _ = files.remove_file(&row.to);
                    Err(match other {
                        Ok(_) => "the copy does not read back the same".to_string(),
                        Err(e) => format!("the copy could not be read back: {e}"),
                    })
                }
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// A library file was refused: the ones written are written back, exactly as they were, and then
/// every file is put back, except those a library file that could not be written back still
/// names. What stays is in `report.moved` and `report.not_put_back`.
fn rollback(
    rows: &[ExecRow],
    via: &[Option<Via>],
    jobs: &[Job],
    applied: &[usize],
    gate: &mut dyn MoveGate,
    files: &mut dyn Files,
    report: &mut Report,
) {
    let mut stuck: BTreeSet<usize> = BTreeSet::new();
    for &k in applied.iter().rev() {
        let job = &jobs[k];
        let reverse: Vec<Edit> = job
            .planned
            .edits
            .iter()
            .map(|e| Edit {
                kind: e.kind,
                sections: e.sections.clone(),
                before: e.after.clone(),
                after: e.before.clone(),
            })
            .collect();
        let expect = Expected {
            entries: Vec::new(),
            home: None,
            exact: Some(job.text.clone()),
        };
        if let Err(why) = gate.edit(&job.at, &job.planned.base, &reverse, &expect) {
            report.not_put_back.push(Failure {
                path: job.at.clone(),
                why: format!("its lines could not be written back: {why}"),
            });
            stuck.extend(job.planned.rows.iter().copied());
        }
    }
    for (i, row) in rows.iter().enumerate() {
        let Some(how) = via[i] else { continue };
        if stuck.contains(&i) {
            report.not_put_back.push(Failure {
                path: row.to.clone(),
                why: "a library file still names it there".to_string(),
            });
            report.moved.push((row.from.clone(), row.to.clone()));
            continue;
        }
        let back = match how {
            Via::Renamed => files
                .rename_new(&row.to, &row.from)
                .map_err(|e| e.to_string()),
            Via::Copied => {
                let same = files.read(&row.to).is_ok_and(|b| {
                    mdshared_app::update::sha256_hex(&b).eq_ignore_ascii_case(&row.sha256)
                });
                if same {
                    files.remove_file(&row.to).map_err(|e| e.to_string())
                } else {
                    Err("the copy is not what was copied".to_string())
                }
            }
        };
        match back {
            Ok(()) => report.put_back.push(row.from.clone()),
            Err(why) => {
                report.not_put_back.push(Failure {
                    path: row.to.clone(),
                    why: format!("it could not be put back: {why}"),
                });
                report.moved.push((row.from.clone(), row.to.clone()));
            }
        }
    }
}

// ---- refusing before the first file moves -------------------------------------------------------

/// The library files a request can touch: those in the library folder, and, for a library move,
/// those in the folder it leaves.
fn files_of(req: &Request) -> Vec<PathBuf> {
    let mut all = library_files(&req.library);
    if let Some(from) = &req.from_library {
        all.extend(library_files(from));
    }
    all
}

/// Everything that can be refused, refused before the first file moves (the audit's lesson, as
/// `plan_preflight` has it for a split): the folders are what the plan says, every file is still
/// the one it was made from, every destination is free and inside the library or the group's
/// folder, no library file involved is open with unsaved changes, and every library file would
/// read back, in the lines it needs, as the notes it lists. `Err` is the reason, as a card says
/// it.
pub fn preflight(req: &Request, gate: &mut dyn MoveGate) -> Result<(), String> {
    let library = &req.library;
    if library.as_os_str().is_empty() || !library.is_absolute() {
        return Err("there is no library folder to move the notes within.".to_string());
    }
    if req.rows.is_empty() {
        return Err("nothing is ticked.".to_string());
    }
    let mut home = PathBuf::new();
    match req.start {
        Start::Group | Start::Headings => {
            let Some(group) = &req.group else {
                return Err("the plan names no group.".to_string());
            };
            home = group.home.clone();
            let (h, l) = (canon(&home), canon(library));
            if home.as_os_str().is_empty()
                || has_parent_dir(&home)
                || same_path(&h, &l)
                || under(&l, &h)
            {
                return Err(format!(
                    "{} is the library folder or holds it, so the notes would be library files.",
                    shown(&home)
                ));
            }
            if req.from_library.is_some() {
                return Err("a group's notes do not move from another library.".to_string());
            }
        }
        Start::Library => {
            let Some(from) = &req.from_library else {
                return Err("the plan does not say which folder the library leaves.".to_string());
            };
            let (f, l) = (canon(from), canon(library));
            if !from.is_absolute() || !from.is_dir() {
                return Err(format!("{} is not a folder.", from.display()));
            }
            if same_path(&f, &l) || under(&f, &l) || under(&l, &f) {
                return Err(
                    "the two library folders are one, or one is inside the other.".to_string(),
                );
            }
        }
    }
    let mut tos: Vec<&Path> = Vec::new();
    for row in &req.rows {
        let name = shown(&row.from);
        // First, because a path past the system's limit has no spelling the disk can be asked about,
        // so it would otherwise read as outside the library.
        if chars(&row.to) > PATH_MAX_CHARS {
            return Err(format!(
                "{name}: the new path would be {} characters long.",
                chars(&row.to)
            ));
        }
        if has_parent_dir(&row.to) || !inside(library, &home, &row.to) {
            return Err(format!(
                "{} is outside the library and the group's folder.",
                row.to.display()
            ));
        }
        let parent = row.to.parent();
        let in_library = parent.is_some_and(|p| same_path(&canon(p), &canon(library)));
        match row.kind {
            FileKind::Note if in_library => {
                return Err(format!("{name} would be a library file, not a note."));
            }
            FileKind::Library if !in_library => {
                return Err(format!(
                    "{name} is a library file and belongs in the library folder."
                ));
            }
            FileKind::Record
                if !parent.is_some_and(|p| {
                    same_path(&canon(p), &canon(&library.join(runs::RUNS_DIR)))
                }) =>
            {
                return Err(format!(
                    "{name} is a record and belongs in the records' folder."
                ));
            }
            _ => {}
        }
        let same_name = row
            .from
            .file_name()
            .zip(row.to.file_name())
            .is_some_and(|(a, b)| same_path(Path::new(a), Path::new(b)));
        if !row.kind.fits(&row.to) || !row.kind.fits(&row.from) || !same_name {
            return Err(format!(
                "{name} would change its name, which a move never does."
            ));
        }
        match is_plain_file(&row.from) {
            Some(Err(v)) => return Err(format!("{name}: {}.", v.text())),
            Some(Ok(_)) => {}
            None => return Err(format!("{name} is not there.")),
        }
        if !runs::digest_of(&row.from).is_some_and(|d| d.eq_ignore_ascii_case(&row.sha256)) {
            return Err(format!(
                "{name} has changed since this plan was made. Open the plan again."
            ));
        }
        if std::fs::symlink_metadata(&row.to).is_ok() || tos.iter().any(|t| same_path(t, &row.to)) {
            return Err(format!(
                "{name}: there is already a file called this in {}.",
                row.to.parent().map(shown).unwrap_or_default()
            ));
        }
        if let Some(blocker) = row.to.parent().and_then(in_the_way) {
            return Err(format!(
                "{blocker} is a file where a folder of {name} would go."
            ));
        }
        tos.push(&row.to);
        gate.free(&row.from)?;
    }
    // The dry run of every library file: the lines as the write will make them.
    let map = Mapping {
        pairs: req
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| (i, r.from.clone(), r.to.clone()))
            .collect(),
        dirs: req
            .from_library
            .as_ref()
            .map(|from| (from.clone(), library.clone())),
    };
    for file in files_of(req) {
        let own = req.rows.iter().position(|r| same_path(&r.from, &file));
        let at = own.map_or_else(|| file.clone(), |i| req.rows[i].to.clone());
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let base_before = file.parent().unwrap_or(Path::new(""));
        let base_after = at.parent().unwrap_or(Path::new(""));
        let planned = plan_edits(&text, base_before, base_after, &map, &[], own)
            .map_err(|why| format!("{}: {why}.", shown(&file)))?;
        if planned.is_some() || own.is_some() {
            gate.free(&file)?;
        }
    }
    Ok(())
}

// ---- a write -----------------------------------------------------------------------------------

/// Moves the ticked rows, with the clock read now.
pub fn write(req: &Request, gate: &mut dyn MoveGate, files: &mut dyn Files) -> Report {
    let (secs, started) = runs::now_utc();
    write_at(req, gate, files, secs, &started)
}

/// [`write`] with the clock given, so a test can know the record's name.
pub fn write_at(
    req: &Request,
    gate: &mut dyn MoveGate,
    files: &mut dyn Files,
    secs: u64,
    started: &str,
) -> Report {
    let mut report = Report::default();
    if let Err(why) = preflight(req, gate) {
        report.start_error = Some(format!("The run could not start: {why}"));
        return report;
    }
    let rows: Vec<ExecRow> = req
        .rows
        .iter()
        .map(|r| ExecRow {
            from: r.from.clone(),
            to: r.to.clone(),
            sha256: r.sha256.clone(),
        })
        .collect();
    let (name, file, home) = req
        .group
        .as_ref()
        .map(|g| (g.name.clone(), g.file.clone(), g.home.clone()))
        .unwrap_or_else(|| ("the library".to_string(), PathBuf::new(), PathBuf::new()));
    // 1. The record, first: every file the plan names, with `done` false, and nothing the run
    // did, because nothing is done yet.
    let record = Record {
        version: runs::RECORD_VERSION,
        kind: Kind::Move,
        started: started.to_string(),
        started_secs: secs,
        state: State::Writing,
        instruction: req.label.clone(),
        sent: None,
        source: runs::Source {
            path: PathBuf::new(),
            len: 0,
            sha256: String::new(),
            recycled: false,
        },
        group: runs::Group {
            name,
            file,
            created: false,
            home,
        },
        folders: Vec::new(),
        notes: Vec::new(),
        pictures: Vec::new(),
        sections_added: Vec::new(),
        moved: Some(Moved {
            start: req.start,
            library_from: req.from_library.clone(),
            files: req
                .rows
                .iter()
                .map(|r| MovedFile {
                    kind: r.kind,
                    from: r.from.clone(),
                    to: r.to.clone(),
                    done: false,
                    len: r.len,
                    sha256: r.sha256.clone(),
                    outside: r.outside,
                    sections: r.sections.clone(),
                })
                .collect(),
            changes: Vec::new(),
            emptied: Vec::new(),
        }),
    };
    let record_path = match record.create(&req.library) {
        Ok(path) => path,
        Err(e) => {
            report.start_error = Some(format!("The run could not start: {e}"));
            return report;
        }
    };
    report.record_path = Some(record_path.clone());
    record.note_done(&record_path, "started");
    let mut journal = Journal {
        record,
        path: record_path,
        library: req.library.clone(),
    };
    let outside_parents: Vec<PathBuf> = req
        .rows
        .iter()
        .filter(|r| r.outside)
        .filter_map(|r| r.from.parent().map(canon))
        .collect();
    let library_c = canon(&req.library);
    let from_c = req.from_library.as_ref().map(|f| canon(f));
    let strictly = |d: &Path, base: &Path| under(d, base) && !same_path(d, base);
    let removable = move |dir: &Path| {
        let d = canon(dir);
        strictly(&d, &library_c)
            || from_c.as_ref().is_some_and(|f| strictly(&d, f))
            || outside_parents.iter().any(|p| same_path(p, &d))
    };
    let setup = Setup {
        dirs: req
            .from_library
            .as_ref()
            .map(|from| (from.clone(), req.library.clone())),
        library_files: files_of(req),
        prefer: Vec::new(),
        removable: &removable,
    };
    run(&rows, &setup, gate, files, Some(&mut journal), &mut report);
    // 7. The record is closed.
    let mut record = journal.record;
    record.state = if report.rolled_back.is_some() {
        if report.not_put_back.is_empty() {
            State::Undone
        } else {
            State::LinksPending
        }
    } else {
        State::Done
    };
    match record.close(&journal.path, &req.library) {
        Ok(()) => {
            report.closed = true;
            record.note_done(&journal.path, record.state.as_str());
        }
        Err(e) => report.close_error = Some(e.to_string()),
    }
    if report.closed {
        runs::prune(&req.library, secs);
    }
    report.record = Some(record);
    report
}

// ---- undo --------------------------------------------------------------------------------------

/// What a move's record may name, checked before anything is read or planned (D-072): the same
/// rule as a split's, with the one thing a move has that a split has not, a place the files came
/// from. Every `to` is inside the library or the group's declared home, and every `from` is too,
/// or is inside the folder a library move left, or was said to be outside by the run. A path
/// with a `..` in it, a file that would change its name, or a kind of file the run does not
/// carry refuses the whole record, and the sentence says which.
fn reach_of(record: &Record, library: &Path) -> Result<Moved, String> {
    if !library.is_absolute() {
        return Err("The library folder is not known, so this run is not undone.".to_string());
    }
    let Some(m) = &record.moved else {
        return Err("This record names no files, so there is nothing to undo.".to_string());
    };
    let home = if record.group.file.as_os_str().is_empty() {
        None
    } else {
        runs::declared_home(&record.group.file, library)
    };
    let reach = Reach {
        library: library.to_path_buf(),
        home,
    };
    let outside = |path: &Path| {
        format!(
            "{} is outside the library and the group's folder, so this run is not undone.",
            path.display()
        )
    };
    let foreign = |path: &Path| {
        format!(
            "{} is not a file a move carries, so this run is not undone.",
            path.display()
        )
    };
    if !record.group.file.as_os_str().is_empty() && !reach.is_library_file(&record.group.file) {
        return Err(outside(&record.group.file));
    }
    if let Some(from) = &m.library_from {
        let (f, l) = (canon(from), canon(library));
        if !from.is_absolute()
            || has_parent_dir(from)
            || same_path(&f, &l)
            || under(&f, &l)
            || under(&l, &f)
        {
            return Err(outside(from));
        }
    } else if m.start == Start::Library {
        return Err("This record does not say which folder the library left.".to_string());
    }
    let from_ok = |path: &Path, said_outside: bool| {
        !has_parent_dir(path)
            && path.is_absolute()
            && (reach.allows(path)
                || m.library_from.as_ref().is_some_and(|f| {
                    let (p, f) = (canon(path), canon(f));
                    under(&p, &f) && !same_path(&p, &f)
                })
                || said_outside)
    };
    for file in &m.files {
        if !reach.allows(&file.to) {
            return Err(outside(&file.to));
        }
        if !from_ok(&file.from, file.outside) {
            return Err(outside(&file.from));
        }
        let same_name = file
            .from
            .file_name()
            .zip(file.to.file_name())
            .is_some_and(|(a, b)| same_path(Path::new(a), Path::new(b)));
        if !file.kind.fits(&file.to) || !file.kind.fits(&file.from) || !same_name {
            return Err(foreign(&file.to));
        }
        match file.kind {
            FileKind::Library => {
                let from_dir_ok = m.library_from.as_ref().is_some_and(|f| {
                    file.from
                        .parent()
                        .is_some_and(|p| same_path(&canon(p), &canon(f)))
                });
                if !reach.is_library_file(&file.to) || !from_dir_ok {
                    return Err(outside(&file.to));
                }
            }
            FileKind::Record => {
                let dir = library.join(runs::RUNS_DIR);
                if !file
                    .to
                    .parent()
                    .is_some_and(|p| same_path(&canon(p), &canon(&dir)))
                {
                    return Err(outside(&file.to));
                }
            }
            FileKind::Note => {}
        }
    }
    for change in &m.changes {
        let dir_ok = change.file.parent().is_some_and(|p| {
            same_path(&canon(p), &canon(library))
                || m.library_from
                    .as_ref()
                    .is_some_and(|f| same_path(&canon(p), &canon(f)))
        });
        if has_parent_dir(&change.file) || !has_extension(&change.file, &["md"]) || !dir_ok {
            return Err(outside(&change.file));
        }
    }
    for folder in &record.folders {
        if !reach.allows(folder) {
            return Err(outside(folder));
        }
    }
    // A folder undo may make again is one that holds the place of a file it puts back.
    for dir in &m.emptied {
        let holds = m.files.iter().any(|f| under(&canon(&f.from), &canon(dir)));
        if has_parent_dir(dir) || !dir.is_absolute() || !holds {
            return Err(outside(dir));
        }
    }
    Ok(m.clone())
}

/// What undo would do to one move, worked out from the record and the disk as they are now.
#[derive(Clone, Debug, Default)]
pub struct UndoPlan {
    /// Why undo cannot run at all.
    pub refusal: Option<String>,
    /// The files that go back, as indexes into the record's files.
    pub rows: Vec<usize>,
    /// Files that stay where they are, and why.
    pub kept: Vec<(PathBuf, String)>,
    /// Files that are not where the record says any more: nothing to do.
    pub gone: Vec<PathBuf>,
    /// Folders the run removed that go back, because a file does.
    pub folders: usize,
    /// Files that return to a place outside the library, because the run said they came from one.
    pub outside: usize,
}

impl UndoPlan {
    /// Whether anything would happen.
    pub fn acts(&self) -> bool {
        self.refusal.is_none() && !self.rows.is_empty()
    }

    /// The question's sentence: what goes back, and what stays.
    pub fn sentence(&self) -> String {
        if let Some(why) = &self.refusal {
            return why.clone();
        }
        let count = |n: usize, one: &str, many: &str| {
            if n == 1 {
                format!("1 {one}")
            } else {
                format!("{n} {many}")
            }
        };
        let mut s = if self.rows.is_empty() {
            "Nothing of this move is left to put back.".to_string()
        } else {
            let mut s = format!(
                "Puts {} back where {} was, and the lines in the library that name {} as they \
                 said",
                count(self.rows.len(), "file", "files"),
                if self.rows.len() == 1 { "it" } else { "they" },
                if self.rows.len() == 1 { "it" } else { "them" },
            );
            if self.folders > 0 {
                s.push_str(&format!(
                    ", and makes {} again",
                    count(self.folders, "folder", "folders")
                ));
            }
            s.push('.');
            s
        };
        if self.outside > 0 {
            s.push_str(&format!(
                " {} go back outside the library, to where they came from.",
                count(self.outside, "file", "files")
            ));
        }
        if !self.kept.is_empty() {
            let mut why: Vec<&str> = self.kept.iter().map(|(_, w)| w.as_str()).collect();
            why.sort_unstable();
            why.dedup();
            s.push_str(&format!(
                " {} stay where {} now: {}.",
                count(self.kept.len(), "file", "files"),
                if self.kept.len() == 1 {
                    "it is"
                } else {
                    "they are"
                },
                why.join("; ")
            ));
        }
        s
    }
}

/// Works out what undo would do, reading the disk and never writing it.
pub fn prepare(record: &Record, library: &Path) -> UndoPlan {
    plan_undo(record, library).0
}

fn plan_undo(record: &Record, library: &Path) -> (UndoPlan, Option<Moved>) {
    let mut plan = UndoPlan::default();
    if record.state == State::Undone {
        plan.refusal = Some("This run is undone already.".to_string());
        return (plan, None);
    }
    if record.kind != Kind::Move {
        plan.refusal = Some("This is not a move.".to_string());
        return (plan, None);
    }
    let m = match reach_of(record, library) {
        Ok(m) => m,
        Err(why) => {
            plan.refusal = Some(why);
            return (plan, None);
        }
    };
    // Every library file as it is now, from the files: the lines that name a file where it is.
    let mut listings = vec![runs::list_library(library)];
    if let Some(from) = &m.library_from {
        listings.push(runs::list_library(from));
    }
    if listings.iter().any(|l| !l.complete) {
        plan.refusal = Some(
            "The library could not be read in full, so there is no telling which lines still \
             name a file. Nothing was moved."
                .to_string(),
        );
        return (plan, None);
    }
    for (i, file) in m.files.iter().enumerate() {
        if !file.done {
            continue;
        }
        match std::fs::symlink_metadata(&file.to) {
            Err(_) => {
                plan.gone.push(file.to.clone());
                continue;
            }
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
                plan.kept
                    .push((file.to.clone(), "it is not a plain file".to_string()));
                continue;
            }
            Ok(_) => {}
        }
        if !runs::digest_of(&file.to).is_some_and(|d| d.eq_ignore_ascii_case(&file.sha256)) {
            plan.kept
                .push((file.to.clone(), "changed since it was moved".to_string()));
            continue;
        }
        if std::fs::symlink_metadata(&file.from).is_ok() {
            plan.kept.push((
                file.to.clone(),
                "there is a file at its old place already".to_string(),
            ));
            continue;
        }
        // The folders on the way to its old place: there, or removed by this run.
        let mut folders_ok = true;
        let mut at = file.from.parent();
        while let Some(dir) = at {
            if dir.symlink_metadata().is_ok() {
                folders_ok &= dir.is_dir();
                break;
            }
            if !m.emptied.iter().any(|e| same_path(e, dir)) {
                folders_ok = false;
                break;
            }
            at = dir.parent();
        }
        if !folders_ok {
            plan.kept.push((
                file.to.clone(),
                "the folder it came from is gone".to_string(),
            ));
            continue;
        }
        // The line that names it: the run's, or the owner's. Another line, or the run's line
        // edited since, keeps the file where its lines say it is (D-070, for a move).
        if file.kind == FileKind::Note {
            let named: Vec<&runs::Listing> = listings
                .iter()
                .flat_map(|l| runs::named_in(l, &file.to))
                .collect();
            let recorded = m
                .changes
                .iter()
                .find(|c| c.row == Some(i) && c.edit.kind == ChangeKind::Note);
            if named.len() > 1 {
                plan.kept.push((
                    file.to.clone(),
                    "another line in the library lists it".to_string(),
                ));
                continue;
            }
            if let (Some(c), Some(line)) = (recorded, named.first())
                && line.line.trim_end() != c.edit.after.trim_end()
                && line.line.trim_end() != c.edit.before.trim_end()
            {
                plan.kept
                    .push((file.to.clone(), "its line was edited since".to_string()));
                continue;
            }
        }
        plan.rows.push(i);
        if file.outside {
            plan.outside += 1;
        }
        if file
            .from
            .parent()
            .is_some_and(|d| d.symlink_metadata().is_err())
        {
            plan.folders += 1;
        }
    }
    (plan, Some(m))
}

/// Undoes one move: the files that are still the ones it moved go back, the lines are put back
/// as they said, and the record is marked "undone". It is the machine of a write run the other
/// way, so it is refused, rolled back and reported the same.
pub fn undo(
    path: &Path,
    record: &Record,
    library: &Path,
    gate: &mut dyn MoveGate,
    files: &mut dyn Files,
) -> Report {
    let mut report = Report::default();
    let (plan, moved) = plan_undo(record, library);
    let Some(m) = moved.filter(|_| plan.refusal.is_none()) else {
        report.start_error = plan.refusal;
        return report;
    };
    let rows: Vec<ExecRow> = plan
        .rows
        .iter()
        .map(|&i| ExecRow {
            from: m.files[i].to.clone(),
            to: m.files[i].from.clone(),
            sha256: m.files[i].sha256.clone(),
        })
        .collect();
    report.kept.extend(plan.kept.iter().cloned());
    // A file that is not there to be put back: the run moved it, and someone has since.
    for gone in &plan.gone {
        report
            .kept
            .push((gone.clone(), "it is not there any more".to_string()));
    }
    let made = record.folders.clone();
    let removable = move |dir: &Path| {
        let d = canon(dir);
        made.iter().any(|f| same_path(&canon(f), &d))
    };
    let mut library_files = library_files(library);
    if let Some(from) = &m.library_from {
        library_files.extend(self::library_files(from));
    }
    let setup = Setup {
        dirs: m
            .library_from
            .as_ref()
            .map(|from| (library.to_path_buf(), from.clone())),
        library_files,
        prefer: m.changes.iter().map(|c| c.edit.clone()).collect(),
        removable: &removable,
    };
    run(&rows, &setup, gate, files, None, &mut report);
    let mut marked = record.clone();
    marked.state = if report.rolled_back.is_some() || !report.failed.is_empty() {
        State::LinksPending
    } else {
        State::Undone
    };
    match marked.close(path, library) {
        Ok(()) => report.closed = true,
        Err(e) => report.close_error = Some(e.to_string()),
    }
    report.record = Some(marked);
    report
}

#[cfg(test)]
#[path = "moves_tests.rs"]
mod tests;
