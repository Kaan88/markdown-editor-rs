//! Write notes' write (`docs/ai-pictures-plan.md` phase V5; sections 3 step 6, 9, 10, 11, 15):
//! the plan the card left in the run (`run.arranged`, with the owner's renames in
//! `run.copy_names`) becomes files, in the order of 4.3: the record first, then the folders, the
//! copies of the pictures, the notes, the read-back, the links in the group's file. Every prefix
//! of that order is a state "AI runs…" can finish or undo. Everything that could refuse the run
//! is asked before the record is made: a refused run writes nothing.
//!
//! Nothing here can overwrite, move or rename a file: a copy and a note are created with
//! `create_new`, which never replaces a name that is taken; an original is read and never
//! touched; the group's file is written through the window's own gate (`PlanGate`).
//!
//! The record is the shared record's `notes` kind (plan section 10): every note and every copy
//! the run plans is in it from step 1, each copy beside the original it was made from and that
//! original's digest (none for a pasted picture, whose temp file goes with the window). It has
//! no source. Its undo removes a note only while it is what was written, a copy only while it is
//! what was written, its original is where it was, and no note undo keeps shows it; never an
//! original.

use super::library_write::{library_text, same_folder};
use super::plan_write::PlanGate;
use super::*;
use crate::commands::strings;
use crate::replace_ui::Outcome;
use crate::task_ui::{FileKind, TaskKind};
use mdedit_core::notes_ai::{self, Compose, Draft, Look, Place};
use mdedit_core::notes_write::{self as nw, Taken};
use mdedit_core::runs::{self, Gate};
use mdshared_app::files::create_new;
use mdshared_app::library;

/// The longest record this program reads back (`runs`' own cap): what a record with the group's
/// text before the links kept in it must stay under.
const RECORD_MAX: usize = 4 * 1024 * 1024;

#[cfg(test)]
thread_local! {
    /// Tests only: the step the write stops at, as a process that died there would.
    pub(super) static DIES_AT: std::cell::Cell<Option<&'static str>> =
        const { std::cell::Cell::new(None) };
}

/// Whether a test asked the write to stop at `step`; never in a build of the program.
fn dies_at(step: &str) -> bool {
    #[cfg(test)]
    {
        DIES_AT.with(|d| d.get() == Some(step))
    }
    #[cfg(not(test))]
    {
        let _ = step;
        false
    }
}

/// Everything a write needs, worked out before its first byte.
pub(super) struct Job {
    library: PathBuf,
    group: crate::task_ui::Group,
    /// The folder undo will reach besides the library: the group's `home:` as its file says now.
    reach: PathBuf,
    instruction: String,
    sent: Option<runs::Sent>,
    planned: nw::Planned,
    drafts: Vec<Draft>,
    looks: Vec<Look>,
    text_out: bool,
    copy: bool,
    /// Each picture of the plan, by its place in the window, and its file.
    originals: Vec<(usize, PathBuf)>,
    /// The notes as the write makes them, each composed with every copy made.
    notes: Vec<runs::NewNote>,
}

impl Job {
    fn original(&self, id: usize) -> Option<&Path> {
        self.originals
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, p)| p.as_path())
    }

    /// Inside the library folder (not the folder itself) or the group's folder.
    fn inside(&self, path: &Path) -> bool {
        let under = |dir: &Path| {
            !dir.as_os_str().is_empty() && path.starts_with(dir) && !same_folder(path, dir)
        };
        under(&self.library) || under(&self.reach)
    }

    /// Note `n` as it reads with the copies that failed (`failed`, a picture's place and the
    /// folder of its `pictures`, with why) said where they would have been.
    fn text_of(&self, n: usize, failed: &[(usize, PathBuf, String)]) -> String {
        let (Some(at), Some(draft)) = (
            self.planned.notes.get(n),
            self.planned
                .notes
                .get(n)
                .and_then(|a| self.drafts.get(a.index)),
        ) else {
            return String::new();
        };
        let place = |id: usize| -> Option<Place> {
            if !self.copy && !self.original(id).is_some_and(copied_anyway) {
                let original = self.original(id)?;
                return Some(Place::At(link_to_original(&at.folder, original)));
            }
            if let Some((_, _, why)) = failed
                .iter()
                .find(|(i, dir, _)| *i == id && dir.parent() == Some(at.folder.as_path()))
            {
                let name = self
                    .original(id)
                    .and_then(Path::file_name)
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                return Some(Place::Missing(format!("{name} could not be copied: {why}")));
            }
            let copy = self.planned.copy_of(&at.folder, id)?;
            Some(Place::At(format!(
                "{}/{}",
                notes_ai::PICTURES_DIR,
                copy.name()
            )))
        };
        let c = Compose {
            looks: &self.looks,
            name: &|_| None,
            text_out: self.text_out,
        };
        notes_ai::compose_at(draft, &c, &place)
    }

    /// The request the rest of the window reads a run by (the Notes pane's opening on it).
    fn request(&self) -> runs::Request {
        runs::Request {
            library: self.library.clone(),
            source: PathBuf::new(),
            instruction: self.instruction.clone(),
            sent: self.sent.clone(),
            group: runs::Group {
                name: self.group.name.clone(),
                file: self.group.file.clone(),
                created: false,
                home: self.group.home.clone(),
            },
            folders: Vec::new(),
            notes: self.notes.clone(),
            pictures: Vec::new(),
        }
    }
}

/// Whether a picture is copied beside its note even with "Copy the pictures beside the notes"
/// off: a pasted one, whose temp file goes with the window, and one on a network share, which no
/// `file:` link from a note leads to (Fable's review of V5, 6).
pub(super) fn copied_anyway(path: &Path) -> bool {
    crate::notes_task::is_pasted(path) || on_share(path)
}

/// A path on a network share: `\\server\share\…`, `\\?\UNC\…`, or `//server/share/…`.
pub(super) fn on_share(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.starts_with(r"\\?\") || s.starts_with(r"\\.\") {
        return s.get(4..8).is_some_and(|u| u.eq_ignore_ascii_case(r"UNC\"));
    }
    s.starts_with(r"\\") || s.starts_with("//")
}

/// A picture linked where it is ("Copy the pictures beside the notes" off, section 9): by a
/// relative path when it is on the note's drive, as image paste links one, else by its address.
fn link_to_original(folder: &Path, original: &Path) -> String {
    match crate::image_paste::relative_path(folder, original) {
        Some(rel) => rel,
        None => format!(
            "file:///{}",
            original
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches('/')
        ),
    }
}

/// A file's bytes, when it is a regular file of at most `max` bytes.
fn read_at_most(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("it is not a file".to_string());
    }
    if meta.len() > max {
        return Err(format!(
            "it is {} MB, more than the {} MB a copy may be",
            meta.len().div_ceil(1_000_000),
            max / 1_000_000
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > max {
        return Err("it grew while it was read".to_string());
    }
    Ok(bytes)
}

/// What is at the name a copy would take: nothing, the original's very bytes, or another file.
fn taken_at(to: &Path, from: &Path) -> Taken {
    let Ok(meta) = to.symlink_metadata() else {
        return Taken::Free;
    };
    let same = meta.is_file()
        && std::fs::metadata(from).is_ok_and(|m| m.len() == meta.len())
        && matches!(
            (read_at_most(to, nw::COPY_FILE_MAX), read_at_most(from, nw::COPY_FILE_MAX)),
            (Ok(a), Ok(b)) if a == b
        );
    if same { Taken::Same } else { Taken::Other }
}

/// One file left out, and why (`runs::Failure`, whose maker is the shared writer's own).
fn failure(path: &Path, why: impl Into<String>) -> runs::Failure {
    runs::Failure {
        path: path.to_path_buf(),
        why: why.into(),
    }
}

fn sha(bytes: &[u8]) -> String {
    mdshared_app::update::sha256_hex(bytes)
}

fn shown(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

impl App {
    /// Why the plan of Write notes cannot be written now, or `None` when it can: every check of
    /// the write, asked of the disk as it is. It reads the disk, so the plan card does not ask it
    /// each frame: its button follows `notes_plan_ui::write_blocked`, and `notes_write` asks
    /// these checks when pressed, refusing with the reason.
    #[cfg(test)]
    pub(super) fn notes_write_blocked(&self) -> Option<String> {
        self.notes_job().err()
    }

    /// Everything checked before anything is written (section 15): the plan, the group and its
    /// file, the limits of section 11, the names, and the links as they would read back.
    fn notes_job(&self) -> Result<Job, String> {
        let s = self
            .task
            .as_ref()
            .ok_or_else(|| "the window is closed.".to_string())?;
        let run = s
            .notes_run
            .as_ref()
            .filter(|_| s.task == TaskKind::Notes)
            .ok_or_else(|| "there is no plan to write.".to_string())?;
        if run.looking() || run.arrange.is_some() {
            return Err("the plan is not ready yet.".to_string());
        }
        let arranged = run
            .arranged
            .as_ref()
            .ok_or_else(|| "there is no plan to write.".to_string())?;
        let group = s
            .group()
            .cloned()
            .ok_or_else(|| "choose the group the notes go to first.".to_string())?;
        let library = self
            .library_dir()
            .filter(|d| d.is_absolute())
            .ok_or_else(|| strings::LIBRARY_NO_FOLDER.to_string())?;
        if !group
            .file
            .parent()
            .is_some_and(|dir| same_folder(dir, &library))
        {
            return Err(format!(
                "{} is not a file of the library folder.",
                shown(&group.file)
            ));
        }
        if !group.file.is_file() {
            return Err(format!("{} is not there any more.", shown(&group.file)));
        }
        // A7: the notes would be files the reader takes for library files.
        if same_folder(&group.home, &library) {
            return Err(strings::LIBRARY_HOME_IS_LIBRARY.to_string());
        }
        if group.home.exists() && !group.home.is_dir() {
            return Err(format!("{} is a file, not a folder.", group.home.display()));
        }
        let originals: Vec<(usize, PathBuf)> = s
            .files
            .iter()
            .enumerate()
            .filter(|(_, f)| f.kind == FileKind::Picture)
            .map(|(i, f)| (i, f.path.clone()))
            .collect();
        let looks = run.looks.clone();
        let naming = nw::Naming {
            copy: s.notes.copy,
            rename: s.notes.rename,
            looks: &looks,
            names: &run.copy_names,
        };
        let of = |id: usize| {
            s.files
                .get(id)
                .filter(|f| f.kind == FileKind::Picture)
                .map(|f| nw::Original {
                    path: f.path.clone(),
                    size: std::fs::metadata(&f.path).map_or(f.size, |m| m.len()),
                    pasted: copied_anyway(&f.path),
                })
        };
        let planned = nw::plan(&arranged.notes, &group.home, &of, &naming, &taken_at)?;
        // The instruction that made the plan: "Arrange again…"'s own, else the window's.
        let instruction = run
            .arrange_instruction
            .clone()
            .unwrap_or_else(|| s.instruction.text().trim().to_string());
        // What was sent, when anything was: the AI off sends nothing.
        let sent = (s.ai_on && run.requests > 0).then(|| {
            let p = &self.docs.prefs;
            let c = p.ai_connection;
            runs::Sent {
                connection: c.key().to_string(),
                model: p.ai_model(c).to_string(),
                bytes: run.bytes,
                requests: run.requests,
            }
        });
        let mut job = Job {
            reach: runs::undo_home(&group.file, &library),
            library: library.clone(),
            group,
            instruction,
            sent,
            planned,
            drafts: arranged.notes.clone(),
            looks,
            text_out: s.notes.text_out,
            copy: s.notes.copy,
            originals,
            notes: Vec::new(),
        };
        // Every path the run writes is one its undo can reach, and none is a library file.
        for n in &job.planned.notes {
            if !job.inside(&n.path) {
                return Err(format!(
                    "{} would be outside the library and the group's folder, where its undo could not reach it.",
                    n.path.display()
                ));
            }
            if n.path.parent().is_some_and(|p| same_folder(p, &library)) {
                return Err(format!(
                    "{} would be a library file, not a note.",
                    shown(&n.path)
                ));
            }
            if n.path.symlink_metadata().is_ok() {
                return Err(format!(
                    "{} is there already. Rename the note on the plan, or move that file.",
                    shown(&n.path)
                ));
            }
        }
        for c in job.planned.written() {
            if !job.inside(&c.to) {
                return Err(format!(
                    "{} would be outside the library and the group's folder, where its undo could not reach it.",
                    c.to.display()
                ));
            }
        }
        // Each note as it will be written, within its limit.
        let mut notes = Vec::new();
        for (n, at) in job.planned.notes.iter().enumerate() {
            let Some(draft) = job.drafts.get(at.index) else {
                continue;
            };
            let text = job.text_of(n, &[]);
            if text.len() > nw::NOTE_BYTES_MAX {
                return Err(format!(
                    "\u{2018}{}\u{2019} would be {} KB, more than the {} KB a note may hold.",
                    draft.title,
                    text.len().div_ceil(1024),
                    nw::NOTE_BYTES_MAX / 1024
                ));
            }
            notes.push(runs::NewNote {
                path: at.path.clone(),
                from_source: text.clone(),
                text,
                title: draft.title.clone(),
                link_path: crate::library_pane::link_path(&library, &at.path),
                sections: draft.sections.clone(),
                ranges: Vec::new(),
                heading_shift: 0,
                title_added: false,
                already: false,
            });
        }
        job.notes = notes;
        // The group's file as it reads now: free of unsaved changes, writable, with room, and
        // taking the links so that it reads back as its old notes plus these.
        if std::fs::metadata(&job.group.file).is_ok_and(|m| m.permissions().readonly()) {
            return Err(format!(
                "{} is read-only, so its links could not be written.",
                shown(&job.group.file)
            ));
        }
        self.library_file_free(&job.group.file)?;
        let (text, _) = library_text(&job.group.file)
            .map_err(|why| format!("{} could not be read: {why}", shown(&job.group.file)))?;
        let parsed = library::parse(&text, &library);
        if parsed.entries.len().saturating_add(job.notes.len()) > library::MAX_ENTRIES {
            return Err(format!(
                "the group would have more than {} notes.",
                library::MAX_ENTRIES
            ));
        }
        let links: Vec<runs::Link> = job.notes.iter().map(runs::Link::new).collect();
        let new = runs::with_links_added(&text, &links);
        if !runs::check_links_added(&parsed, &library::parse(&new, &library), &links, &library) {
            return Err(format!(
                "{} would not read back as its old notes plus the new ones.",
                shown(&job.group.file)
            ));
        }
        Ok(job)
    }

    /// "Write the notes" of Write notes: the checks, then the write, then the report, the Notes
    /// pane on the first new note, that note in a tab with its preview (4.4), and the window
    /// closes. A run refused before its record writes nothing, and the window stays.
    pub(super) fn notes_write(&mut self) {
        let job = match self.notes_job() {
            Ok(job) => job,
            Err(why) => {
                self.docs
                    .set_toast(&format!("The notes could not be written: {why}"));
                self.redraw();
                return;
            }
        };
        let library = job.library.clone();
        let report = {
            let mut gate = PlanGate {
                app: self,
                library: library.clone(),
            };
            write(&job, &mut gate)
        };
        if let Some(why) = &report.start_error {
            self.docs.set_toast(why);
            self.redraw();
            return;
        }
        // The window closes, and the pictures pasted into it go with it, only when a note was
        // written and every pasted picture a note takes was copied: else a pasted screenshot's
        // only copy would go (Fable's review of V5, 2). It stays, and says why.
        if let Some(why) = stays_open(&report) {
            self.docs.check_changed();
            self.ui.tree.refresh();
            self.library_rows();
            self.docs.set_toast(&why);
            self.redraw();
            return;
        }
        self.task_close();
        self.docs.check_changed();
        self.ui.tree.refresh();
        self.library_rows();
        self.docs
            .set_toast(&written_toast(&report, &job.group.name));
        self.show_outcome(Outcome::run(&report, &library));
        self.open_notes_of(&report, &job.request());
        if let Some(first) = report.notes_written.first().cloned() {
            self.open_after_run(&first, true);
        }
        self.redraw();
    }
}

/// Why the window stays open after a write, or `None` when it may close: no note was written, or
/// a pasted picture could not be copied, and its file in the temp folder is its only copy.
fn stays_open(report: &runs::Report) -> Option<String> {
    if report.notes() == 0 {
        let why = report
            .note_failures
            .first()
            .or(report.check_failures.first())
            .map(|f| format!("{}: {}", shown(&f.path), f.why))
            .unwrap_or_else(|| "nothing was left to write".to_string());
        return Some(format!(
            "No note could be written ({why}). The window stays open with its pictures; AI runs\u{2026} can undo what was written."
        ));
    }
    let f = report
        .picture_failures
        .iter()
        .find(|f| crate::notes_task::is_pasted(&f.path))?;
    Some(format!(
        "Wrote {}, but the pasted picture {} {}. The window stays open with it, so it is not \
         lost: to write the notes again, undo this run from AI runs\u{2026} first.",
        count(report.notes(), "note", "notes"),
        shown(&f.path),
        f.why
    ))
}

/// The toast after the write: what was written, or the first thing that went wrong.
fn written_toast(report: &runs::Report, group: &str) -> String {
    if let Some(f) = report
        .link_failures
        .first()
        .or(report.group_failure.as_ref())
    {
        return format!(
            "The notes are written, but {} could not be updated: {}. {}",
            shown(&f.path),
            f.why,
            strings::LINKS_PENDING_NEXT
        );
    }
    if let Some(f) = report.picture_failures.first() {
        let more = report.picture_failures.len() - 1;
        return format!(
            "{} {}{}. Its note is written without it.",
            shown(&f.path),
            f.why,
            if more > 0 {
                format!(" ({} more)", more)
            } else {
                String::new()
            }
        );
    }
    let notes = report.notes();
    let mut s = format!("Wrote {}", count(notes, "note", "notes"));
    if !report.pictures_written.is_empty() {
        s.push_str(&format!(
            " and {}",
            count(report.pictures_written.len(), "picture", "pictures")
        ));
    }
    s.push_str(&format!(" in {group}. AI runs\u{2026} can undo it."));
    if !report.note_failures.is_empty() || !report.check_failures.is_empty() {
        s.push_str(" Some notes were left out: the report says why.");
    }
    s
}

/// The write itself, in the order of 4.3, through `gate` for the group's file.
fn write(job: &Job, gate: &mut dyn Gate) -> runs::Report {
    let mut report = runs::Report::default();
    let library = &job.library;
    let (secs, started) = runs::now_utc();
    // 1. The record, "writing", with every note and every copy planned. A note or a copy whose
    //    name is free now is marked as the run's before it is written, so a run that dies at any
    //    step leaves a record that owns all it made; undo removes one only while it holds what
    //    was planned for it (Fable's review of V5, 1). A copy is planned with the digest of its
    //    original as read now, and with that original, except a pasted picture's (review, 5).
    let mut record = runs::Record {
        version: runs::RECORD_VERSION,
        kind: runs::Kind::Notes,
        started: started.clone(),
        started_secs: secs,
        state: runs::State::Writing,
        instruction: job.instruction.clone(),
        sent: job.sent.clone(),
        // A run of Write notes has no source: no file was cut, every original is only read.
        source: runs::Source {
            path: PathBuf::new(),
            len: 0,
            sha256: String::new(),
            recycled: false,
        },
        group: runs::Group {
            name: job.group.name.clone(),
            file: job.group.file.clone(),
            created: false,
            home: job.group.home.clone(),
        },
        folders: Vec::new(),
        notes: job
            .notes
            .iter()
            .map(|n| runs::NoteRec {
                path: n.path.clone(),
                created: n.path.symlink_metadata().is_err(),
                sha256: sha(n.text.as_bytes()),
                ranges: Vec::new(),
                heading_shift: 0,
                title_added: false,
                sections: n.sections.clone(),
                line: library::link_line(&n.title, &n.link_path),
            })
            .collect(),
        pictures: job
            .planned
            .written()
            .filter_map(|c| {
                let digest = sha(&read_at_most(&c.from, nw::COPY_FILE_MAX).ok()?);
                Some(planned_copy(c, digest))
            })
            .collect(),
        sections_added: Vec::new(),
        moved: None,
        originals: Vec::new(),
        added: Vec::new(),
        copied: Vec::new(),
        removed: Vec::new(),
    };
    let record_path = match record.create(library) {
        Ok(path) => path,
        Err(e) => {
            report.start_error = Some(format!("The run could not start: {e}"));
            return report;
        }
    };
    report.record_path = Some(record_path.clone());
    record.note_done(&record_path, "started");
    if dies_at("after the record") {
        return report;
    }

    // 2. Folders: only the ones that were not there are the run's, and the record says so before
    //    each is made, so a run that dies here leaves none it does not own.
    for folder in job.planned.folders() {
        if !job.inside(&folder) {
            report.folder_failures.push(failure(
                &folder,
                "it is outside the library and the group's folder",
            ));
            continue;
        }
        let mut missing: Vec<PathBuf> = Vec::new();
        let mut at = Some(folder.as_path());
        while let Some(dir) = at {
            if dir.symlink_metadata().is_ok() || !job.inside(dir) {
                break;
            }
            missing.push(dir.to_path_buf());
            at = dir.parent();
        }
        let new: Vec<PathBuf> = missing
            .into_iter()
            .rev()
            .filter(|d| !record.folders.iter().any(|f| f == d))
            .collect();
        if !new.is_empty() {
            record.folders.extend(new.iter().cloned());
            let _ = record.close(&record_path, library);
        }
        let made = std::fs::create_dir_all(&folder);
        // A folder that could not be made is not the run's after all.
        if new.iter().any(|d| !d.is_dir()) {
            record.folders.retain(|f| !new.contains(f) || f.is_dir());
            let _ = record.close(&record_path, library);
        }
        if let Err(e) = made {
            report.folder_failures.push(failure(&folder, e.to_string()));
        }
    }
    if dies_at("after the folders") {
        return report;
    }

    // 3. The copies, each a new file and never over one, each in the record as it will be
    //    before it is written.
    let mut failed: Vec<(usize, PathBuf, String)> = Vec::new();
    let mut copied = 0;
    for copy in &job.planned.copies {
        let dir = copy.to.parent().map(Path::to_path_buf).unwrap_or_default();
        // The run's own entry for this copy, planned at step 1.
        let ours = |r: &runs::PictureRec| r.path == copy.to && r.created;
        let made = read_at_most(&copy.from, nw::COPY_FILE_MAX).and_then(|bytes| {
            if copy.there {
                return match read_at_most(&copy.to, nw::COPY_FILE_MAX) {
                    Ok(there) if there == bytes => Ok((bytes, false)),
                    _ => Err("the file at its name changed since the plan was made".to_string()),
                };
            }
            // The original changed since step 1, or could not be read then: the record holds
            // what will be written before it is.
            let planned = planned_copy(copy, sha(&bytes));
            if !record.pictures.contains(&planned) {
                record.pictures.retain(|r| !ours(r));
                record.pictures.push(planned);
                let _ = record.close(&record_path, library);
            }
            match create_new(&copy.to, &bytes) {
                Ok(()) => Ok((bytes, true)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    match read_at_most(&copy.to, nw::COPY_FILE_MAX) {
                        Ok(there) if there == bytes => Ok((bytes, false)),
                        _ => Err("a file of that name is there with other contents".to_string()),
                    }
                }
                Err(e) => Err(e.to_string()),
            }
        });
        match made {
            Ok((_, true)) => {
                record.note_done(&record_path, &format!("picture {:?}", copy.to));
                report.pictures_written.push(copy.to.clone());
                copied += 1;
                if copied == 1 && dies_at("mid-copies") {
                    return report;
                }
            }
            Ok((bytes, false)) => {
                // There with these bytes already: linked, and never the run's to remove.
                record.pictures.retain(|r| !ours(r));
                record.pictures.push(runs::PictureRec {
                    path: copy.to.clone(),
                    created: false,
                    sha256: sha(&bytes),
                    original: None,
                    original_sha256: String::new(),
                });
                let _ = record.close(&record_path, library);
                report.pictures_written.push(copy.to.clone());
            }
            Err(why) => {
                if record.pictures.iter().any(ours) {
                    record.pictures.retain(|r| !ours(r));
                    let _ = record.close(&record_path, library);
                }
                failed.push((copy.id, dir, why.clone()));
                report
                    .picture_failures
                    .push(failure(&copy.from, format!("could not be copied: {why}")));
            }
        }
    }
    let _ = record.close(&record_path, library);
    if dies_at("before the notes") {
        return report;
    }

    // The notes as they are written: a copy that failed is a line saying so (section 15), and a
    //    note that grew past its limit for it is left out (Fable's review of V5, 9). The record
    //    holds what each will hold before any is written.
    let mut notes = job.notes.clone();
    let mut too_large: Vec<usize> = Vec::new();
    if !failed.is_empty() {
        for (n, note) in notes.iter_mut().enumerate() {
            let text = job.text_of(n, &failed);
            if text.len() > nw::NOTE_BYTES_MAX {
                too_large.push(n);
                report.note_failures.push(failure(
                    &note.path,
                    format!(
                        "with the pictures that could not be copied said in it, it would be {} KB, more than the {} KB a note may hold",
                        text.len().div_ceil(1024),
                        nw::NOTE_BYTES_MAX / 1024
                    ),
                ));
            }
            note.from_source = text.clone();
            note.text = text;
            if let Some(rec) = record.notes.get_mut(n) {
                rec.sha256 = sha(note.text.as_bytes());
                if too_large.contains(&n) {
                    rec.created = false;
                }
            }
        }
        let _ = record.close(&record_path, library);
    }

    // 4. The notes, each at its name and never over a file.
    let mut written: Vec<usize> = Vec::new();
    for (i, note) in notes.iter().enumerate() {
        if too_large.contains(&i) {
            continue;
        }
        if let Some(rec) = record.notes.get_mut(i) {
            rec.created = false;
        }
        if !job.inside(&note.path) {
            report.note_failures.push(failure(
                &note.path,
                "it is outside the library and the group's folder",
            ));
            continue;
        }
        match create_new(&note.path, note.text.as_bytes()) {
            Ok(()) => {
                written.push(i);
                if let Some(rec) = record.notes.get_mut(i) {
                    rec.created = true;
                }
                report.notes_written.push(note.path.clone());
                record.note_done(&record_path, &format!("note {}", shown(&note.path)));
                if written.len() == 1 && dies_at("mid-notes") {
                    return report;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                report.note_failures.push(failure(
                    &note.path,
                    "a file of that name is there with other contents",
                ));
            }
            Err(e) => report
                .note_failures
                .push(failure(&note.path, e.to_string())),
        }
    }

    // 5. The read-back: a note that does not read back is neither linked nor counted.
    let mut sound: Vec<usize> = Vec::new();
    for &i in &written {
        let note = &notes[i];
        match runs::verify(&note.path, &note.from_source) {
            Ok(()) => sound.push(i),
            Err(why) => report.check_failures.push(failure(&note.path, why)),
        }
    }
    let _ = record.close(&record_path, library);
    if dies_at("before the links") {
        return report;
    }

    // 6. The links, in one write of the group's file through the window's gate.
    if !sound.is_empty() {
        let file = &job.group.file;
        let links: Vec<runs::Link> = sound.iter().map(|&i| runs::Link::new(&notes[i])).collect();
        let before = library_text(file).ok().map(|(t, _)| t);
        let had = before.as_deref().map(|t| library::parse(t, library));
        let mut added: Vec<Vec<String>> = Vec::new();
        for link in &links {
            for n in 1..=link.sections.len() {
                let path: Vec<String> = link.sections[..n].to_vec();
                let known = had
                    .as_ref()
                    .is_some_and(|p| p.sections.iter().any(|s| sections_eq(s, &path)));
                if !known && !added.iter().any(|s| sections_eq(s, &path)) {
                    added.push(path);
                }
            }
        }
        match gate.add_links(file, &links) {
            Ok(()) => {
                record.sections_added = added;
                keep_added(&mut record, file, before, &links, library);
                report.linked.push(file.clone());
                record.note_done(
                    &record_path,
                    &format!("links {} in {}", links.len(), shown(file)),
                );
            }
            Err(why) => report.link_failures.push(failure(file, why)),
        }
    }

    // 7. The record is closed.
    record.state = if report.link_failures.is_empty() {
        runs::State::Done
    } else {
        runs::State::LinksPending
    };
    match record.close(&record_path, library) {
        Ok(()) => {
            report.closed = true;
            record.note_done(&record_path, record.state.as_str());
            runs::prune(library, secs);
        }
        Err(e) => report.close_error = Some(e.to_string()),
    }
    report.record = Some(record);
    report
}

/// A copy as the record holds it before it is written: the run's own while its name is free, with
/// the digest of the original's bytes, and that original, except a pasted picture's, whose temp
/// file goes with the window (Fable's review of V5, 5).
fn planned_copy(copy: &nw::Copy, digest: String) -> runs::PictureRec {
    let pasted = crate::notes_task::is_pasted(&copy.from);
    runs::PictureRec {
        path: copy.to.clone(),
        created: copy.to.symlink_metadata().is_err(),
        sha256: digest.clone(),
        original: (!pasted).then(|| copy.from.clone()),
        original_sha256: if pasted { String::new() } else { digest },
    }
}

/// Two section paths alike, as the shared writer compares headings.
fn sections_eq(a: &[String], b: &[String]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.trim().eq_ignore_ascii_case(y.trim()))
}

/// Keeps in the record what the links did to the group's file, as the shared writer keeps it:
/// only when the file now holds exactly those links added to the text it had, and only while the
/// record still reads back with it; undo can then give the file back byte for byte.
fn keep_added(
    record: &mut runs::Record,
    file: &Path,
    before: Option<String>,
    links: &[runs::Link],
    library: &Path,
) {
    let Some(before) = before else { return };
    let Ok((now, _)) = library_text(file) else {
        return;
    };
    if runs::with_links_added(&before, links) != now {
        return;
    }
    let entry = runs::Added {
        file: file.to_path_buf(),
        before,
        written: sha(now.as_bytes()),
    };
    let mut probe = record.clone();
    probe.added.push(entry.clone());
    probe.state = runs::State::LinksPending;
    if probe.to_json(library).len() <= RECORD_MAX {
        record.added.push(entry);
    }
}

impl App {
    /// `--dialog run-undone-pictures`: the report after undoing a run of Write notes whose first
    /// note the owner changed since: it is kept, and so is the copy it shows (plan section 10),
    /// said as kept for that note; the other notes, their copies and their lines went.
    pub(super) fn notes_undone_demo(&mut self, root: &Path) {
        let section = root.join("Screenshots").join("Sign-in");
        let pictures = section.join(notes_ai::PICTURES_DIR);
        let kept_note = section.join("sign-in-fails-after-the-update.md");
        let kept_copy = pictures.join("login-error.png");
        let report = runs::UndoReport {
            removed_notes: vec![
                section.join("the-error-page.md"),
                section.join("two-step-code.md"),
            ],
            removed_pictures: vec![
                pictures.join("error-page.png"),
                pictures.join("code.png"),
                pictures.join("update.png"),
            ],
            removed_links: 2,
            kept: vec![kept_note, kept_copy.clone()],
            shown: vec![kept_copy],
            ..Default::default()
        };
        self.show_outcome(Outcome::undone(&report, root));
        // Scrolled to its end, where the rows kept and why are; the draw clamps it.
        self.ui.results.scroll = f32::MAX;
    }

    /// `--dialog run-undo-pictures`: "AI runs…" with a run of Write notes on top and the question
    /// before its undo, as `prepare` would find a run whose notes and copies are all still there.
    /// The record is made in memory, never on disk.
    pub(super) fn notes_undo_demo(&mut self, root: &Path) {
        let home = root.join("Screenshots");
        let section = home.join("Sign-in");
        let titles = [
            "Sign-in fails after the update",
            "The error page",
            "Two-step code",
        ];
        let notes: Vec<runs::NoteRec> = titles
            .iter()
            .map(|t| {
                let path = section.join(format!("{}.md", mdshared_app::split::note_slug(t)));
                runs::NoteRec {
                    line: library::link_line(t, &crate::library_pane::link_path(root, &path)),
                    path,
                    created: true,
                    sha256: String::new(),
                    ranges: Vec::new(),
                    heading_shift: 0,
                    title_added: false,
                    sections: vec!["Sign-in".to_string()],
                }
            })
            .collect();
        let pictures: Vec<runs::PictureRec> = [
            "login-error.png",
            "error-page.png",
            "code.png",
            "update.png",
        ]
        .iter()
        .map(|n| runs::PictureRec {
            path: section.join(notes_ai::PICTURES_DIR).join(n),
            created: true,
            sha256: String::new(),
            original: Some(PathBuf::from(format!("D:/Desk/Screenshots/{n}"))),
            original_sha256: String::new(),
        })
        .collect();
        let (secs, started) = runs::now_utc();
        let record = runs::Record {
            version: runs::RECORD_VERSION,
            kind: runs::Kind::Notes,
            started: started.clone(),
            started_secs: secs,
            state: runs::State::Done,
            instruction: String::new(),
            sent: None,
            source: runs::Source {
                path: PathBuf::new(),
                len: 0,
                sha256: String::new(),
                recycled: false,
            },
            group: runs::Group {
                name: "Screenshots".to_string(),
                file: root.join("Screenshots.md"),
                created: false,
                home: home.clone(),
            },
            folders: vec![section.clone(), section.join(notes_ai::PICTURES_DIR)],
            notes,
            pictures,
            sections_added: vec![vec!["Sign-in".to_string()]],
            moved: None,
            originals: Vec::new(),
            added: Vec::new(),
            copied: Vec::new(),
            removed: Vec::new(),
        };
        let plan = runs::UndoPlan {
            notes: record.notes.iter().map(|n| n.path.clone()).collect(),
            pictures: record.pictures.iter().map(|p| p.path.clone()).collect(),
            links: vec![(
                record.group.file.clone(),
                record
                    .notes
                    .iter()
                    .map(|n| runs::LinkRemoval {
                        path: n.path.clone(),
                        sections: n.sections.clone(),
                        line: n.line.clone(),
                    })
                    .collect(),
            )],
            folders: record.folders.clone(),
            ..Default::default()
        };
        let sentence = self.undo_sentence(&plan, &record.group_name());
        let path = root
            .join(runs::RUNS_DIR)
            .join(format!("{}-notes.json", started.replace(':', "-")));
        self.ui.runs.list.insert(
            0,
            runs::Run {
                record,
                path: path.clone(),
                library: root.to_path_buf(),
            },
        );
        self.ui.runs.sel = 0;
        self.ui.runs.undo = Some((path, sentence, true));
        self.open_dialog(DialogKind::RunUndo);
    }
}
