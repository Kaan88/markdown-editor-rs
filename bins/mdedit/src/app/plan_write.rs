//! What "Write the notes" does: the plan becomes files, through the order of 4.3, and the
//! record and undo of 4.4 and 4.5. Every write to a library file goes through the window's own
//! gate ([`App::library_attempt`]): it refuses a file that is open with unsaved changes, reads it
//! again, checks the new text and writes it at once.
//!
//! Nothing here can overwrite, move or rename a file: the notes and the pictures are created by
//! `mdedit_core::runs`, which never replaces a name that is taken, and the source document is
//! read and never touched.

use super::library_write::{Wrote, file_stem_for};
use super::*;
use crate::library_pane::{RowId, key_of};
use crate::plan_ui::PlanScreen;
use crate::replace_ui::Outcome;
use crate::ui::PaneFace;
use mdedit_core::runs::{self, Gate, Group, LinkRemoval, NewNote, NewPicture, Request};
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
        match self.app.library_attempt(
            file,
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
        match self.app.library_attempt(
            file,
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
    /// panel and the Notes pane opens on the first new note.
    pub(super) fn plan_write(&mut self) {
        let Some(screen) = self.plan.take() else {
            return;
        };
        let request = match self.plan_request(&screen) {
            Ok(request) => request,
            Err(why) => {
                self.plan = Some(screen);
                self.docs
                    .set_toast(&format!("The run could not start: {why}"));
                self.redraw();
                return;
            }
        };
        let library = self.docs.library.dir.clone();
        let mut gate = PlanGate {
            app: self,
            library: library.clone(),
        };
        let report = runs::write(&request, &mut gate);
        // The gate's borrow ends here: the window is the window's again.
        if let Some(why) = &report.start_error {
            // Nothing was written, and nothing after step 1 could have been: the card stays
            // open with the plan, every tick, rename and merge in it, and the reason on the bar.
            self.plan = Some(screen);
            self.docs.set_toast(why);
            self.redraw();
            return;
        }
        self.plan = None;
        self.plan_parked = None;
        if let Some(failure) = report
            .link_failures
            .first()
            .or(report.group_failure.as_ref())
        {
            let name = failure
                .path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.docs.set_toast(&format!(
                "The notes are written, but {name} could not be updated: {}. Try again / Undo",
                failure.why
            ));
        }
        self.docs.check_changed();
        self.ui.tree.refresh();
        self.library_rows();
        let outcome = Outcome::run(&report, &library);
        self.show_outcome(outcome);
        self.open_notes_of(&report, &request);
        self.redraw();
    }

    /// The request a plan makes: every ticked note and the rest, at the names the screen worked
    /// out, and nothing else.
    fn plan_request(&self, screen: &PlanScreen) -> Result<Request, String> {
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
            let group = self
                .docs
                .library
                .groups
                .iter()
                .find(|g| g.name.eq_ignore_ascii_case(&screen.dest.name))
                .ok_or_else(|| format!("there is no group called {}.", screen.dest.name))?;
            (group.file.clone(), false)
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
            let stem = match file_stem_for(&title) {
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
        Ok(Request {
            library: root,
            source,
            instruction: String::new(),
            sent: None,
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

    /// Opens the Notes pane on the group the report names, its cursor on the first note written.
    fn open_notes_of(&mut self, report: &runs::Report, request: &Request) {
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
    /// "AI runs…": the records read now, newest first.
    pub(super) fn open_runs(&mut self) {
        let library = self.docs.library.dir.clone();
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
        let root = Some(self.docs.library.dir.clone())
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
                        plan.sentence(&run.record.group_name()),
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
                    (plan.sentence(&run.record.group_name()), true)
                };
                self.ui.runs.undo = Some((run.path.clone(), sentence, can));
                self.open_dialog(DialogKind::RunUndo);
            }
            _ => self.open_dialog(DialogKind::Runs),
        }
        self.redraw();
    }

    /// "Undo this run" on the report: the question first, then the undo.
    pub(super) fn run_undo_ask(&mut self, path: PathBuf) {
        let library = self.library_dir().unwrap_or_default();
        let Some(record) = runs::load(&path, &library) else {
            self.docs
                .set_toast("The record of that run cannot be read.");
            return;
        };
        let plan = runs::prepare(&record, &library);
        if let Some(why) = &plan.refusal {
            self.docs.set_toast(why);
        }
        self.ui.runs.undo = Some((
            path,
            plan.sentence(&record.group_name()),
            plan.refusal.is_none(),
        ));
        self.open_dialog(DialogKind::RunUndo);
    }

    /// The question's answer: undo the run whose record it holds.
    pub(super) fn run_undo(&mut self, path: PathBuf) {
        let Some(library) = self.library_dir() else {
            return;
        };
        let Some(record) = runs::load(&path, &library) else {
            return;
        };
        let mut gate = PlanGate {
            app: self,
            library: library.clone(),
        };
        let report = runs::undo(&path, &record, &library, &mut gate);
        self.ui.dialog = None;
        self.ui.runs.undo = None;
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
