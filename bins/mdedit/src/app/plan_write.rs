//! What "Write the notes" does: the plan becomes files, through the order of 4.3, and the
//! record and undo of 4.4 and 4.5. Every write to a library file goes through the window's own
//! gate ([`App::library_attempt`]): it refuses a file that is open with unsaved changes, reads it
//! again, checks the new text and writes it at once.
//!
//! Nothing here can overwrite, move or rename a file: the notes and the pictures are created by
//! `mdedit_core::runs`, which never replaces a name that is taken, and the source document is
//! read and never touched.

use super::library_write::{Wrote, file_stem_for, note_stem_for};
use super::*;
use crate::commands::strings;
use crate::library_pane::{RowId, key_of};
use crate::plan_ui::PlanScreen;
use crate::replace_ui::Outcome;
use crate::ui::PaneFace;
use mdedit_core::runs::{self, Gate, Group, LinkRemoval, NewNote, NewPicture, Request};
use mdshared_app::files::create_new;
use mdshared_app::library;
use mdshared_app::paths::same_path;

/// The window's half of [`runs::Gate`]: the same library writer every other command uses.
pub(super) struct PlanGate<'a> {
    pub(super) app: &'a mut App,
    pub(super) library: PathBuf,
}

impl Gate for PlanGate<'_> {
    fn add_links(&mut self, file: &Path, links: &[mdedit_core::runs::Link]) -> Result<(), String> {
        let base = self.library.clone();
        // Refused when the group would pass the library's own limit (4.1).
        if let Ok(text) = std::fs::read_to_string(file) {
            let before = library::parse(&text, &base);
            if before.entries.len().saturating_add(links.len()) > library::MAX_ENTRIES {
                return Err(format!(
                    "the group would have more than {} notes",
                    library::MAX_ENTRIES
                ));
            }
        }
        match self.app.library_attempt_at(
            file,
            &base,
            |text| Some(runs::with_links_added(text, links)),
            |old, new| runs::check_links_added(old, new, links, &base),
        ) {
            Wrote::Done => Ok(()),
            Wrote::Nothing => Err("the file was not changed".to_string()),
            Wrote::Refused(why) => Err(why),
        }
    }

    fn remove_links(
        &mut self,
        file: &Path,
        links: &[LinkRemoval],
        headings: &[Vec<String>],
    ) -> Result<(), String> {
        let base = self.library.clone();
        match self.app.library_attempt_at(
            file,
            &base,
            |text| runs::with_removals(text, links, headings, &base),
            |old, new| runs::check_links_removed(old, new, links, &base),
        ) {
            Wrote::Done => Ok(()),
            // Nothing to take out is not success: the notes of those lines stay, and the run
            // says it is not undone.
            Wrote::Nothing => Err("none of the run's lines is in it any more".to_string()),
            Wrote::Refused(why) => Err(why),
        }
    }
}

impl App {
    /// "Write the notes": the plan is written, the card closes, the report takes the results
    /// panel and the Notes pane opens on the first new note. Everything that could be refused is
    /// checked first, and the group's file is made before the notes (D2).
    pub(super) fn plan_write(&mut self) {
        let Some(screen) = self.plan.take() else {
            return;
        };
        let mut request = match plan_request(&screen) {
            Ok(request) => request,
            Err(why) => {
                self.plan = Some(screen);
                self.docs
                    .set_toast(&format!("The run could not start: {why}"));
                self.redraw();
                return;
            }
        };
        // D2: the group's file's name is free, the two caps are clear, no library file involved
        // has unsaved changes, the source is still the file the plan was made from, and the links
        // would read back. All of it before a byte is written.
        if let Err(why) = plan_preflight(self, &screen, &request) {
            self.plan = Some(screen);
            self.docs
                .set_toast(&format!("The run could not start: {why}"));
            self.redraw();
            return;
        }
        // The group's file, before the notes: an empty group is a state one can live with, and
        // undo already takes it away. `runs` would make it at step 6, after every note is on
        // disk, so it is made here; the record is told afterwards that this run made it.
        let group_text = if request.group.created {
            let text = match new_group_text(&request) {
                Ok(text) => text,
                Err(why) => return self.plan_write_refused(screen, &why),
            };
            // The library folder itself, when this is its first file: `create_new` makes a file
            // in a folder, never the folder.
            if let Err(e) = std::fs::create_dir_all(&request.library) {
                return self.plan_write_refused(
                    screen,
                    &format!("{} could not be made: {e}", request.library.display()),
                );
            }
            match create_new(&request.group.file, text.as_bytes()) {
                Ok(()) => Some(text),
                Err(e) => {
                    return self.plan_write_refused(
                        screen,
                        &format!("{} could not be made: {e}", shown(&request.group.file)),
                    );
                }
            }
        } else {
            None
        };
        if group_text.is_some() {
            request.group.created = false;
        }
        let library = request.library.clone();
        let mut report = {
            let mut gate = PlanGate {
                app: self,
                library: library.clone(),
            };
            runs::write(&request, &mut gate)
        };
        // The gate's borrow ends here: the window is the window's again.
        if let Some(why) = &report.start_error {
            // Nothing was written, and nothing after step 1 could have been: the card stays
            // open with the plan, every tick, rename and merge in it, and the reason on the bar.
            // The group's file made a moment ago is taken away again, unless it changed.
            if let Some(text) = &group_text {
                remove_own_file(&request.group.file, text);
            }
            self.plan = Some(screen);
            self.docs.set_toast(why);
            self.redraw();
            return;
        }
        if group_text.is_some()
            && let (Some(record), Some(path)) =
                (report.record.as_mut(), report.record_path.as_ref())
        {
            // The record says the run made the group's file, so undo takes it away when the run
            // is undone, and a run left "links pending" still owns it.
            record.group.created = true;
            let _ = record.close(path, &library);
        }
        // 8. The original in the Recycle Bin, when the box was ticked (5.4): last, and only when
        //    every note was written, read back and listed, the record is closed, and the source is
        //    still the file that was read.
        let binned = screen
            .recycle_on()
            .then(|| self.plan_recycle(&mut report, &request, &library));
        self.plan = None;
        self.plan_parked = None;
        let link_failure = report
            .link_failures
            .first()
            .or(report.group_failure.as_ref());
        if let Some(failure) = link_failure {
            let name = failure
                .path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.docs.set_toast(&format!(
                "The notes are written, but {name} could not be updated: {}. Try again / Undo",
                failure.why
            ));
        } else if let Some(Err(why)) = &binned {
            self.docs.set_toast(&format!(
                "The notes are written. {} could not be put in the Recycle Bin: {why}. It is where it was.",
                shown(&request.source)
            ));
        }
        self.docs.check_changed();
        self.ui.tree.refresh();
        self.library_rows();
        let mut outcome = Outcome::run(&report, &library);
        if let Some(result) = &binned {
            // A source that is itself a note of the library: its line now leads nowhere (5.4).
            let listed = if result.is_ok() {
                runs::listed_in(&library, &request.source)
            } else {
                Vec::new()
            };
            outcome.recycled(&request.source, result, &listed);
        }
        self.show_outcome(outcome);
        self.open_notes_of(&report, &request);
        self.redraw();
    }

    /// Step 8 of 4.3: the source of the run in `report` to the Recycle Bin. Its drive is asked
    /// again first, since the card's answer may be a while old. `Err` follows "could not be put in
    /// the Recycle Bin: ", and the source is then where it was.
    fn plan_recycle(
        &mut self,
        report: &mut runs::Report,
        request: &Request,
        library: &Path,
    ) -> Result<(), String> {
        if let Some(why) = self.recycle.refusal(&request.source) {
            return Err(lower_first(&why));
        }
        runs::recycle_source(
            report,
            request,
            library,
            &mut crate::recycle::Through(&mut *self.recycle),
        )
    }

    /// "Write the notes" refused before anything was written: the card stays with the plan, and
    /// the reason is on the bar.
    fn plan_write_refused(&mut self, screen: PlanScreen, why: &str) {
        self.plan = Some(screen);
        self.docs
            .set_toast(&format!("The run could not start: {why}"));
        self.redraw();
    }
}

/// The request a plan makes: every ticked note and the rest, at the names the screen worked out.
fn plan_request(screen: &PlanScreen) -> Result<Request, String> {
    let source = screen
        .source
        .path
        .clone()
        .ok_or_else(|| "this is a demonstration of the plan screen.".to_string())?;
    let root = screen.root().to_path_buf();
    let home = screen.dest.home.clone();
    let (file, created) = if screen.dest.is_new {
        let stem = match file_stem_for(&screen.dest.name) {
            Some(stem) => stem,
            None => return Err("the group has no name.".to_string()),
        };
        (root.join(format!("{stem}.md")), true)
    } else {
        (screen.dest.file.clone(), false)
    };
    let mut notes = Vec::new();
    let mut taken: Vec<PathBuf> = Vec::new();
    for (i, note) in screen.plan.notes().iter().enumerate() {
        if !note.ticked() {
            continue;
        }
        let Some(target) = screen.targets.get(i) else {
            continue;
        };
        let text = screen.plan.note_text(i);
        taken.push(target.path.clone());
        notes.push(NewNote {
            path: target.path.clone(),
            from_source: text.clone(),
            text,
            title: note.title().to_string(),
            link_path: crate::library_pane::link_path(&root, &target.path),
            sections: note.sections().to_vec(),
            ranges: vec![(note.first_line(), note.last_line())],
            heading_shift: note.heading_shift(),
            title_added: note.title_added(),
            already: target.already,
        });
    }
    // The rest, when it is ticked: the document's own name, in the group's folder.
    if screen.plan.rest_ticked() && screen.rest.lines() > 0 {
        let title = screen.rest.title().to_string();
        let stem = match note_stem_for(&title) {
            Some(stem) => stem,
            None => return Err("the note of the rest has no name.".to_string()),
        };
        let folder = library::note_folder(&home, &[]);
        let text = screen.plan.rest_text();
        let (path, already) = free_or_same(&folder, &stem, &text, &taken)?;
        taken.push(path.clone());
        notes.push(NewNote {
            path: path.clone(),
            from_source: text.clone(),
            text,
            title,
            link_path: crate::library_pane::link_path(&root, &path),
            sections: Vec::new(),
            ranges: screen
                .rest
                .runs()
                .iter()
                .map(|r| (r.start.saturating_add(1), r.end))
                .collect(),
            heading_shift: 0,
            title_added: true,
            already,
        });
    }
    if notes.is_empty() {
        return Err("nothing is ticked.".to_string());
    }
    // The folders: the group's, then each note's, parents first.
    let mut folders: Vec<PathBuf> = notes
        .iter()
        .filter_map(|n| n.path.parent().map(Path::to_path_buf))
        .filter(|p| p.starts_with(&home) || p.starts_with(&root))
        .collect();
    if home.starts_with(&root) || home == root {
        folders.push(home.clone());
    }
    folders.sort_by_key(|f| f.components().count());
    folders.dedup();
    // A plan from a model records what it was asked and what was sent (4.4); a cut at headings
    // sent nothing.
    let (instruction, sent) = match &screen.ai_cut {
        Some(cut) => (
            cut.instruction.clone(),
            Some(mdedit_core::runs::Sent {
                connection: cut.connection.clone(),
                model: cut.model.clone(),
                bytes: cut.bytes,
                requests: cut.requests,
            }),
        ),
        None => (String::new(), None),
    };
    Ok(Request {
        library: root,
        source,
        instruction,
        sent,
        group: Group {
            name: screen.dest.name.clone(),
            file,
            created,
            home,
        },
        folders,
        notes,
        pictures: Vec::<NewPicture>::new(),
    })
}

/// Why the plan cannot be written, or `None` when it can: the destination-dependent half of D2,
/// worked out again whenever the group the notes go to changes.
pub(super) fn plan_blocked(app: &App, screen: &PlanScreen) -> Option<String> {
    match plan_request(screen) {
        Ok(request) => plan_preflight(app, screen, &request).err(),
        Err(why) => Some(why),
    }
}

/// Everything D2 checks before the first byte is written: the group's file's name is free, the
/// two caps are clear, no library file involved has unsaved changes, the source is still the
/// file the plan was made from, and a dry run of the link insertion and its check passes against
/// the library file as it reads now. `Err` is the reason, as the card says it.
pub(super) fn plan_preflight(app: &App, screen: &PlanScreen, req: &Request) -> Result<(), String> {
    let library = &req.library;
    // A6: an empty or relative library root makes every path "inside" it. A plan never reaches
    // here without one, because it is refused where it opens.
    if library.as_os_str().is_empty() || !library.is_absolute() {
        return Err("there is no library folder to write the notes into.".to_string());
    }
    // A7: the group's folder must not be the library folder itself: its notes would be files the
    // reader takes for library files.
    if same_path(&req.group.home, library) {
        return Err(format!(
            "{} is the library folder itself, so the notes would be library files. Choose another folder for the group.",
            shown(&req.group.home)
        ));
    }
    for note in &req.notes {
        if note.path.parent().is_some_and(|p| same_path(p, library)) {
            return Err(format!(
                "{} would be a library file, not a note.",
                shown(&note.path)
            ));
        }
    }
    // The group's library file, as it reads now, or as it will read once made.
    let group_text = if req.group.created {
        if req.group.file.exists() {
            return Err(strings::plan_group_taken(&shown(&req.group.file)));
        }
        if library_files(library) >= library::MAX_FILES {
            return Err(format!(
                "the library already holds its {} files.",
                library::MAX_FILES
            ));
        }
        let text = new_group_text(req)?;
        let read = library::parse(&text, library);
        if read.name.as_deref() != Some(req.group.name.trim()) || read.home.is_none() {
            return Err(format!(
                "{} would not read back as its notes.",
                shown(&req.group.file)
            ));
        }
        text
    } else {
        match std::fs::read_to_string(&req.group.file) {
            Ok(text) => text,
            Err(e) => {
                return Err(format!("{} could not be read: {e}", shown(&req.group.file)));
            }
        }
    };
    let parsed = library::parse(&group_text, library);
    // The library's own 2,000.
    if parsed.entries.len().saturating_add(req.notes.len()) > library::MAX_ENTRIES {
        return Err(format!(
            "the group would have more than {} notes.",
            library::MAX_ENTRIES
        ));
    }
    // No library file involved has unsaved changes, at the destination the plan names now (C2).
    app.library_file_free(&req.group.file)?;
    // The source is still the file the plan was made from (A8).
    let bytes = std::fs::read(&req.source)
        .map_err(|e| format!("{} could not be read: {e}", shown(&req.source)))?;
    if bytes.len() as u64 != screen.source.len
        || !mdshared_app::update::sha256_hex(&bytes).eq_ignore_ascii_case(&screen.source.sha256)
    {
        return Err(format!(
            "{} has changed since this plan was made. Save it, then split it again.",
            shown(&req.source)
        ));
    }
    // The dry run: the links exactly as step 6 would write them, against the file as it reads
    // now. C3 is refused here instead of after the notes are on disk.
    let links: Vec<runs::Link> = req.notes.iter().map(runs::Link::new).collect();
    let new = runs::with_links_added(&group_text, &links);
    if !runs::check_links_added(&parsed, &library::parse(&new, library), &links, library) {
        return Err(why_links_would_not_read_back(
            &links,
            &shown(&req.group.file),
        ));
    }
    // The group's folder, as the write will find it: a file where it should be.
    if req.group.home.exists() && !req.group.home.is_dir() {
        return Err(format!(
            "{} is a file, not a folder.",
            req.group.home.display()
        ));
    }
    Ok(())
}

/// The text a new group's library file is made from, the same one `runs` makes at step 6.
fn new_group_text(req: &Request) -> Result<String, String> {
    let name = req.group.name.trim();
    let home = written_home(&req.group.home, &req.library);
    library::set_group_home(&format!("# {name}\n"), &home)
        .ok_or_else(|| "the group has no name.".to_string())
}

/// A folder as a library file writes it: relative to the library when it is inside it, so the
/// library can be moved as a whole (`runs::written_home`).
fn written_home(home: &Path, library: &Path) -> PathBuf {
    match home.strip_prefix(library) {
        Ok(inside) if !inside.as_os_str().is_empty() => inside.to_path_buf(),
        _ => home.to_path_buf(),
    }
}

/// How many Markdown files are directly in the library folder: what `library::MAX_FILES` counts.
fn library_files(dir: &Path) -> usize {
    match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("md"))
            })
            .count(),
        Err(_) => 0,
    }
}

/// Why the links would not read back, in the words of the fault that is known: the dry run
/// cannot say more than that the file would not be its old notes plus the new ones.
fn why_links_would_not_read_back(links: &[runs::Link], file: &str) -> String {
    // C3: the shared reader takes a heading's closing '#' off, so `## C#` reads back as `C`.
    if links.iter().any(|l| {
        l.title.trim_end().ends_with('#') || l.sections.iter().any(|s| s.trim_end().ends_with('#'))
    }) {
        return format!(
            "{file} would not read back: a heading that ends in '#' loses its last '#' when the library is read."
        );
    }
    format!("{file} would not read back as its old notes plus the new ones.")
}

/// The name of a file as a person reads it: its own name, or its whole path when it has none.
fn shown(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// A sentence of the card ("This drive has no Recycle Bin") as the middle of another one.
fn lower_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Takes away a file this function made, when it still holds exactly what was put in it.
fn remove_own_file(path: &Path, text: &str) {
    if std::fs::read(path).is_ok_and(|bytes| bytes == text.as_bytes()) {
        let _ = std::fs::remove_file(path);
    }
}

impl App {
    /// Opens the Notes pane on the group the report names, its cursor on the first note written.
    pub(super) fn open_notes_of(&mut self, report: &runs::Report, request: &Request) {
        if report.notes() == 0 {
            return;
        }
        self.ui.panes.files = true;
        self.ui.set_pane_face(PaneFace::Notes);
        let Some(first) = report
            .notes_written
            .first()
            .or_else(|| report.notes_present.first())
            .cloned()
        else {
            return;
        };
        let section = request
            .notes
            .iter()
            .find(|n| same_path(&n.path, &first))
            .map(|n| n.sections.clone())
            .unwrap_or_default();
        self.ui.library.unfold_to(&request.group.file, &section);
        self.ui.library.build(&self.docs.library);
        self.ui
            .library
            .follow(RowId::Note(key_of(&request.group.file, &section), first));
    }
}

/// `stem.md` in `folder` when that name is free, or the one there with exactly these bytes
/// ("already there", 3.9); `stem 2.md` and on when the name is taken by other bytes.
fn free_or_same(
    folder: &Path,
    stem: &str,
    text: &str,
    taken: &[PathBuf],
) -> Result<(PathBuf, bool), String> {
    for n in 1..=1_000 {
        let name = if n == 1 {
            format!("{stem}.md")
        } else {
            format!("{stem} {n}.md")
        };
        let path = folder.join(name);
        if taken.iter().any(|p| same_path(p, &path)) {
            continue;
        }
        match std::fs::read(&path) {
            Ok(bytes) if bytes == text.as_bytes() => return Ok((path, true)),
            Ok(_) => continue,
            Err(_) => return Ok((path, false)),
        }
    }
    Err("every name for the note of the rest is taken.".to_string())
}

// ---- undo, from the report and from "AI runs…" ------------------------------------------------

impl App {
    /// "AI runs…": the records read now, newest first, out of the one folder every run of this
    /// window is resolved against (A6).
    pub(super) fn open_runs(&mut self) {
        let Some(library) = self.library_dir() else {
            self.docs.set_toast(strings::LIBRARY_NO_FOLDER);
            return;
        };
        (self.ui.runs.list, self.ui.runs.unread) = mdedit_core::runs::list_all(&library);
        self.ui.runs.sel = 0;
        self.ui.runs.undo = None;
        self.open_dialog(DialogKind::Runs);
        self.redraw();
    }

    /// "Finish" on a run that stopped before its links were written.
    pub(super) fn run_finish(&mut self, path: PathBuf) {
        let Some(library) = self.library_dir() else {
            return;
        };
        let Some(record) = runs::load(&path, &library) else {
            self.docs
                .set_toast("The record of that run cannot be read.");
            return;
        };
        let mut gate = PlanGate {
            app: self,
            library: library.clone(),
        };
        let report = runs::finish(&path, &record, &library, &mut gate);
        if let Some(why) = report.error {
            self.docs.set_toast(&why);
        } else if let Some(failure) = report.failures.first() {
            self.docs
                .set_toast(&format!("The links could not be written: {}", failure.why));
        } else if report.added > 0 {
            self.docs
                .set_toast(&format!("The run is finished: {} linked.", report.added));
        }
        self.library_rows();
        self.redraw();
    }

    /// `--dialog runs|run-undo|run-undo-recycled`: "AI runs…" with the runs the shot shows. The
    /// records are made in memory, never on disk, and name the folders a real run would.
    pub(super) fn runs_demo(&mut self, which: Option<crate::args::ShotDialog>) {
        use crate::args::ShotDialog;
        use mdedit_core::runs::{Group, NoteRec, Record, Run, Sent, Source, State};
        // The records are in memory and name folders a real run would; a shot has no library
        // folder, so they sit in a made-up absolute one, and `prepare` reads the disk as it is.
        let root = self
            .library_dir()
            .filter(|d| d.is_absolute())
            .unwrap_or_else(|| {
                std::env::temp_dir()
                    .join("mdedit-runs-demo")
                    .join("library")
            });
        let make =
            |started: &str, secs: u64, state: State, group: &str, notes: usize, recycled: bool| {
                let home = root.join(group);
                let source = PathBuf::from(format!("C:/notes/{}.md", group));
                let items: Vec<NoteRec> = (0..notes)
                    .map(|i| NoteRec {
                        path: home.join(format!("Note {}.md", i + 1)),
                        created: true,
                        sha256: String::new(),
                        ranges: vec![(1 + i * 8, 7 + i * 8)],
                        heading_shift: 1,
                        title_added: false,
                        sections: vec!["Part one".to_string()],
                        line: format!("- [Note {}](Note {}.md)", i + 1, i + 1),
                    })
                    .collect();
                Run {
                    record: Record {
                        version: 1,
                        kind: mdedit_core::runs::Kind::Split,
                        started: started.to_string(),
                        started_secs: secs,
                        state,
                        instruction: String::new(),
                        sent: Some(Sent {
                            connection: "Claude".to_string(),
                            model: "claude-sonnet-4-5".to_string(),
                            bytes: 18_204,
                            requests: 1,
                        }),
                        source: Source {
                            path: source,
                            len: 18_204,
                            sha256: String::new(),
                            recycled,
                        },
                        group: Group {
                            name: group.to_string(),
                            file: root.join(format!("{group}.md")),
                            created: true,
                            home,
                        },
                        folders: Vec::new(),
                        notes: items,
                        pictures: Vec::new(),
                        sections_added: vec![vec!["Part one".to_string()]],
                        moved: None,
                    },
                    path: root.join(".runs").join(format!("{started}-split.json")),
                    library: root.clone(),
                }
            };
        let now = mdedit_core::runs::now_utc();
        let earlier = now.0.saturating_sub(3 * 86_400);
        self.ui.runs.list = vec![
            make(
                &mdedit_core::runs::utc(earlier - 7200),
                earlier - 7200,
                State::Done,
                "Japan 2025",
                23,
                false,
            ),
            make(
                &mdedit_core::runs::utc(earlier),
                earlier,
                State::Undone,
                "Recipes",
                6,
                false,
            ),
            make(&now.1, now.0, State::Done, "Work notes", 3, true),
        ];
        self.ui.runs.sel = 0;
        self.ui.runs.unread = Vec::new();
        match which {
            Some(ShotDialog::RunsNewer) => {
                // One run of this version, so that the files after it are in the window.
                self.ui.runs.list.truncate(1);
                let dir = root.join(".runs");
                self.ui.runs.unread = vec![
                    mdedit_core::runs::Unread {
                        path: dir.join("2026-11-21T09-12-40Z-import.json"),
                        newer: true,
                        started: "2026-11-21T09:12:40Z".to_string(),
                    },
                    mdedit_core::runs::Unread {
                        path: dir.join("notes.json"),
                        newer: false,
                        started: String::new(),
                    },
                ];
                self.open_dialog(DialogKind::Runs);
            }
            Some(
                ShotDialog::RunUndo | ShotDialog::RunUndoRecycled | ShotDialog::RunUndoOriginal,
            ) => {
                let last = self.ui.runs.list.len().saturating_sub(1);
                let pick = if which == Some(ShotDialog::RunUndoRecycled) {
                    last
                } else {
                    0
                };
                self.ui.runs.sel = pick;
                let run = &self.ui.runs.list[pick];
                let (sentence, can) = if matches!(
                    which,
                    Some(ShotDialog::RunUndoRecycled | ShotDialog::RunUndoOriginal)
                ) {
                    // The source is not on disk (the record says it went to the Recycle Bin, or
                    // the owner deleted it): `prepare` reads the disk and refuses, as it would
                    // for a real run.
                    let plan = mdedit_core::runs::prepare(&run.record, &root);
                    (
                        self.undo_sentence(&plan, &run.record.group_name()),
                        plan.refusal.is_none(),
                    )
                } else {
                    // A run whose notes are all still there, as `prepare` would find them.
                    let notes: Vec<PathBuf> = (0..run.record.notes.len())
                        .map(|i| run.record.group.home.join(format!("Note {}.md", i + 1)))
                        .collect();
                    let links: Vec<mdedit_core::runs::LinkRemoval> = run
                        .record
                        .notes
                        .iter()
                        .map(|n| mdedit_core::runs::LinkRemoval {
                            path: n.path.clone(),
                            sections: n.sections.clone(),
                            line: n.line.clone(),
                        })
                        .collect();
                    let plan = mdedit_core::runs::UndoPlan {
                        notes: notes.clone(),
                        links: vec![(run.record.group.file.clone(), links)],
                        folders: vec![
                            run.record.group.home.clone(),
                            run.record.group.home.join("Part one"),
                        ],
                        ..Default::default()
                    };
                    (self.undo_sentence(&plan, &run.record.group_name()), true)
                };
                self.ui.runs.bin = which == Some(ShotDialog::RunUndoRecycled)
                    && run.record.source.recycled
                    && self.recycle.can_open();
                self.ui.runs.undo = Some((run.path.clone(), sentence, can));
                self.open_dialog(DialogKind::RunUndo);
            }
            _ => self.open_dialog(DialogKind::Runs),
        }
        self.redraw();
    }

    /// "Undo this run" on the report: the question first, then the undo.
    pub(super) fn run_undo_ask(&mut self, path: PathBuf) {
        let Some(library) = self.library_dir() else {
            self.docs.set_toast(strings::LIBRARY_NO_FOLDER);
            return;
        };
        let Some(record) = runs::load(&path, &library) else {
            self.docs
                .set_toast("The record of that run cannot be read.");
            return;
        };
        self.ui.runs.bin = false;
        // A move is undone by its own machine (`moves::undo`), under the same gate.
        if record.kind == runs::Kind::Move {
            return self.move_undo_ask(path, &record, &library);
        }
        let plan = runs::prepare(&record, &library);
        if let Some(why) = &plan.refusal {
            self.docs.set_toast(why);
        }
        let sentence = self.undo_sentence(&plan, &record.group_name());
        self.ui.runs.undo = Some((path, sentence, plan.refusal.is_none()));
        // A run that put its original in the Recycle Bin waits for it to be back: the question
        // offers to open the bin, where the system can (5.4).
        self.ui.runs.bin =
            record.source.recycled && plan.refusal.is_some() && self.recycle.can_open();
        self.open_dialog(DialogKind::RunUndo);
    }

    /// The undo question's sentence: what goes and what stays, and where the files go when the
    /// drive has a Recycle Bin (4.5, rule 3).
    fn undo_sentence(&self, plan: &runs::UndoPlan, group: &str) -> String {
        let mut sentence = plan.sentence(group);
        let first = plan.notes.first().or(plan.pictures.first());
        if plan.refusal.is_none() && first.is_some_and(|p| self.recycle.refusal(p).is_none()) {
            sentence.push_str(if plan.pictures.is_empty() {
                " The notes go to the Recycle Bin."
            } else {
                " The notes and pictures go to the Recycle Bin."
            });
        }
        sentence
    }

    /// "Open the Recycle Bin" on the undo question.
    pub(super) fn open_bin(&mut self) {
        if !self.recycle.open() {
            self.docs
                .set_toast("The Recycle Bin could not be opened here.");
        }
    }

    /// The question's answer: undo the run whose record it holds.
    pub(super) fn run_undo(&mut self, path: PathBuf) {
        let Some(library) = self.library_dir() else {
            return;
        };
        let Some(record) = runs::load(&path, &library) else {
            return;
        };
        if record.kind == runs::Kind::Move {
            return self.move_undo(path, &record, &library);
        }
        // Undo's removals go to the Recycle Bin where the drive has one (4.5, rule 3; D-071). The
        // bin is taken out of the window for the length of the undo, since the gate holds it.
        let mut bin = std::mem::replace(&mut self.recycle, Box::new(crate::recycle::Taken));
        let report = {
            let mut gate = PlanGate {
                app: self,
                library: library.clone(),
            };
            runs::undo_with(
                &path,
                &record,
                &library,
                &mut gate,
                &mut crate::recycle::ForUndo(&mut *bin),
            )
        };
        self.recycle = bin;
        self.ui.dialog = None;
        self.ui.runs.undo = None;
        self.ui.runs.bin = false;
        if let Some(why) = &report.refusal {
            self.docs.set_toast(why);
        }
        self.library_rows();
        // The report takes the panel, and it is a new one: a run whose links could not be taken
        // out says so there, with the reason, and stays undone-able.
        self.show_outcome(Outcome::undone(&report, &library));
        self.redraw();
    }

    /// "Show the notes" in "AI runs…": the first note of the run, in a tab, and the pane on it.
    pub(super) fn run_show(&mut self, record_path: &Path) {
        let Some(library) = self.library_dir() else {
            return;
        };
        let Some(record) = runs::load(record_path, &library) else {
            return;
        };
        if record.kind == runs::Kind::Move {
            return self.move_show(&record);
        }
        let Some(note) = record.notes.iter().find(|n| n.path.is_file()) else {
            self.docs.set_toast("None of that run's notes is there.");
            return;
        };
        let path = note.path.clone();
        let sections = note.sections.clone();
        let file = record.group.file.clone();
        self.ui.dialog = None;
        self.open(&path);
        self.ui.panes.files = true;
        self.ui.set_pane_face(PaneFace::Notes);
        self.ui.library.unfold_to(&file, &sections);
        self.ui.library.build(&self.docs.library);
        self.ui
            .library
            .follow(RowId::Note(key_of(&file, &sections), path));
        self.redraw();
    }
}
