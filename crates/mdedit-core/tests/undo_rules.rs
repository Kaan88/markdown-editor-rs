//! The rules that make undo incapable of losing a file the owner wants (audit 3: A2, A3, A4, A5,
//! A9, A10, B2, and D1, D4, D5 of its section 4). Each test forces the real situation on disk and
//! asserts exactly what is left there; each one fails with its fix reverted.

use mdedit_core::runs::{
    self, Gate, Group, Link, LinkRemoval, NewNote, NewPicture, Record, Request, State,
};
use mdshared_app::library;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn scratch(tag: &str) -> PathBuf {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir()
        .join("opencode")
        .join(format!("undo-rules-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    dir
}

fn write_file(path: &Path, text: &str) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).expect("its folder");
    }
    std::fs::write(path, text).expect("a file");
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn digest(text: &str) -> String {
    mdshared_app::update::sha256_hex(text.as_bytes())
}

/// What a hook does after the gate has taken the lines out: the owner, at the wrong moment.
type Hook = Box<dyn FnMut()>;

/// The window's gate without a window: the writers, the checks, one atomic write, and the
/// composition of the removals that the program uses (`runs::with_removals`).
struct FileGate {
    refuse: bool,
    after_remove: Option<Hook>,
}

impl FileGate {
    fn new() -> FileGate {
        FileGate {
            refuse: false,
            after_remove: None,
        }
    }
}

impl Gate for FileGate {
    fn add_links(&mut self, file: &Path, links: &[Link]) -> Result<(), String> {
        if self.refuse {
            return Err("The group's library file has unsaved changes.".to_string());
        }
        let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        let base = file.parent().unwrap_or(Path::new(""));
        let new = runs::with_links_added(&text, links);
        if !runs::check_links_added(
            &library::parse(&text, base),
            &library::parse(&new, base),
            links,
            base,
        ) {
            return Err("the file would not read back as its notes".to_string());
        }
        mdshared_app::settings::write_atomic(file, &new).map_err(|e| e.to_string())
    }

    fn remove_links(
        &mut self,
        file: &Path,
        links: &[LinkRemoval],
        headings: &[Vec<String>],
    ) -> Result<(), String> {
        let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        let base = file.parent().unwrap_or(Path::new(""));
        let new = runs::with_removals(&text, links, headings, base)
            .ok_or_else(|| "the file was not changed".to_string())?;
        if !runs::check_links_removed(
            &library::parse(&text, base),
            &library::parse(&new, base),
            links,
            base,
        ) {
            return Err("the file would not read back as its notes".to_string());
        }
        mdshared_app::settings::write_atomic(file, &new).map_err(|e| e.to_string())?;
        if let Some(hook) = self.after_remove.as_mut() {
            hook();
        }
        Ok(())
    }
}

fn note(library: &Path, rel: &str, title: &str, text: &str, sections: &[&str]) -> NewNote {
    NewNote {
        link_path: PathBuf::from(rel),
        path: library.join(rel),
        text: text.to_string(),
        from_source: text.to_string(),
        title: title.to_string(),
        sections: sections.iter().map(|s| s.to_string()).collect(),
        ranges: vec![(1, 1)],
        heading_shift: 0,
        title_added: false,
        already: false,
    }
}

/// A request as `plan_write::plan_request` builds it: the folders are every note's parent and the
/// group's folder, sorted by depth.
fn request(library: &Path, group: &str, created: bool, notes: Vec<NewNote>) -> Request {
    let source = library.parent().expect("a parent").join("Source.md");
    if !source.exists() {
        write_file(&source, "# One\n\ntext\n");
    }
    let home = library.join(group);
    let mut folders: Vec<PathBuf> = notes
        .iter()
        .filter_map(|n| n.path.parent().map(Path::to_path_buf))
        .collect();
    folders.push(home.clone());
    folders.sort_by_key(|f| f.components().count());
    folders.dedup();
    Request {
        library: library.to_path_buf(),
        source,
        instruction: String::new(),
        sent: None,
        group: Group {
            name: group.to_string(),
            file: library.join(format!("{group}.md")),
            created,
            home,
        },
        folders,
        notes,
        pictures: Vec::new(),
    }
}

const STAMP: &str = "2026-11-21T14:03:11Z";

/// A run of one note `Trip/One.md` into a new group `Trip`, written and read back from disk.
fn one_note_run(tag: &str) -> (PathBuf, Request, PathBuf, Record) {
    let dir = scratch(tag);
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let req = request(
        &library,
        "Trip",
        true,
        vec![note(&library, "Trip/One.md", "One", "# One\n", &[])],
    );
    let mut gate = FileGate::new();
    let report = runs::write_at(&req, &mut gate, 1, STAMP);
    assert_eq!(report.state(), Some(State::Done));
    let path = report.record_path.clone().expect("a record");
    let record = runs::load(&path, &library).expect("the record");
    (library, req, path, record)
}

// ---- A2, D6: both removals in one composition, and nothing written is not success ----------------

#[test]
fn the_composition_gives_the_text_when_a_link_goes_and_no_heading_does() {
    let dir = scratch("compose");
    let library = dir.join("library");
    let text = "# Trip\nhome: Trip\n\n- [One](Trip/One.md)\n- [Two](Trip/Two.md)\n";
    let removals = vec![LinkRemoval {
        path: library.join("Trip/One.md"),
        sections: Vec::new(),
        line: "- [One](Trip/One.md)".to_string(),
    }];
    let new = runs::with_removals(text, &removals, &[], &library);
    assert_eq!(
        new.as_deref(),
        Some("# Trip\nhome: Trip\n\n- [Two](Trip/Two.md)\n"),
        "the link goes, though no section heading goes with it"
    );
    // Nothing to take out is `None`, so a gate can tell it from a change.
    assert_eq!(runs::with_removals(text, &[], &[], &library), None);
}

#[test]
fn undo_of_notes_under_no_section_takes_their_lines_out_with_them() {
    let dir = scratch("flat");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let req = request(
        &library,
        "Trip",
        true,
        vec![
            note(&library, "Trip/Asakusa.md", "Asakusa", "# Asakusa\n", &[]),
            note(&library, "Trip/Nara.md", "Nara", "# Nara\n", &[]),
        ],
    );
    let mut gate = FileGate::new();
    let report = runs::write_at(&req, &mut gate, 2, STAMP);
    let path = report.record_path.clone().expect("a record");
    assert!(read(&library.join("Trip.md")).contains("[Asakusa]"));
    let record = runs::load(&path, &library).expect("the record");
    let done = runs::undo(&path, &record, &library, &mut gate);
    assert_eq!(done.removed_links, 2);
    assert_eq!(done.removed_notes.len(), 2, "{done:?}");
    assert!(!library.join("Trip/Asakusa.md").exists());
    assert!(!library.join("Trip/Nara.md").exists());
    assert!(
        !library.join("Trip.md").exists(),
        "the group file the run made goes, with nothing left in it: {:?}",
        read(&library.join("Trip.md"))
    );
    assert_eq!(
        runs::load(&path, &library).expect("it").state,
        State::Undone
    );
}

// ---- D1, A5: a note a library line still lists is never removed ------------------------------------

#[test]
fn a_line_moved_to_another_section_keeps_its_note_and_the_question_says_so() {
    let (library, _req, path, record) = one_note_run("moved");
    let group = library.join("Trip.md");
    let with_section = library::add_section(&read(&group), &[], "Later").expect("a section");
    let line = library::parse(&with_section, &library).entries[0].line;
    let moved = library::move_note(&with_section, line, &["Later".to_string()]).expect("moved");
    write_file(&group, &moved);
    let plan = runs::prepare(&record, &library);
    let words = plan.sentence("Trip");
    let mut gate = FileGate::new();
    let done = runs::undo(&path, &record, &library, &mut gate);
    // What is on disk: the note is there, with the line that lists it.
    assert_eq!(
        read(&library.join("Trip/One.md")),
        "# One\n",
        "the note is where it was"
    );
    assert!(read(&group).contains("[One]"), "its line stays too");
    assert!(
        done.removed_notes.is_empty(),
        "undo removed {:?}",
        done.removed_notes
    );
    // What the question said.
    assert!(
        plan.notes.is_empty(),
        "the note is not to be removed: {:?}",
        plan.notes
    );
    assert_eq!(plan.links_kept.len(), 1);
    assert_eq!(plan.held.len(), 1);
    assert!(
        words.contains("1 line was edited or moved since and is kept, with its note."),
        "{words}"
    );
}

#[test]
fn a_line_that_appears_while_undo_runs_keeps_its_note() {
    // The owner's own list, written between the links step and the removal of the files: the
    // library is read again after the links are out, and the note a library file lists stays.
    let (library, _req, path, record) = one_note_run("race");
    let note_path = library.join("Trip/One.md");
    let other = library.join("Mine.md");
    let mut gate = FileGate::new();
    gate.after_remove = Some(Box::new({
        let other = other.clone();
        let note_path = note_path.clone();
        move || {
            write_file(
                &other,
                &format!(
                    "# Mine\n\n- [One](<{}>)\n",
                    note_path.to_string_lossy().replace('\\', "/")
                ),
            );
        }
    }));
    let done = runs::undo(&path, &record, &library, &mut gate);
    assert!(
        done.removed_notes.is_empty(),
        "undo removed {:?}",
        done.removed_notes
    );
    assert_eq!(done.held_back, vec![note_path.clone()]);
    assert_eq!(
        read(&note_path),
        "# One\n",
        "the note a line lists is still there"
    );
    assert!(read(&other).contains("One"));
}

#[test]
fn another_library_file_that_lists_the_note_keeps_it() {
    let (library, _req, path, record) = one_note_run("other-file");
    let note_path = library.join("Trip/One.md");
    write_file(
        &library.join("Mine.md"),
        "# Mine\n\n- [Mine, One](Trip/One.md)\n",
    );
    let plan = runs::prepare(&record, &library);
    let words = plan.sentence("Trip");
    let mut gate = FileGate::new();
    let done = runs::undo(&path, &record, &library, &mut gate);
    // What is on disk: the note stays for the owner's line, which is not touched either.
    assert_eq!(read(&note_path), "# One\n", "the note is where it was");
    assert!(read(&library.join("Mine.md")).contains("[Mine, One]"));
    assert!(
        done.removed_notes.is_empty(),
        "undo removed {:?}",
        done.removed_notes
    );
    // The run's own line is out.
    assert!(!library.join("Trip.md").exists() || !read(&library.join("Trip.md")).contains("[One]"));
    // What the question said.
    assert_eq!(plan.held, vec![note_path.clone()], "{plan:?}");
    assert!(plan.notes.is_empty());
    assert!(
        words.contains("1 note stays, because another line in the library lists it."),
        "{words}"
    );
}

#[test]
fn a_line_in_the_six_hundredth_library_file_keeps_the_note() {
    // The reader keeps 512 library files; undo looks at every one, so a line past them is seen.
    let (library, _req, path, record) = one_note_run("many");
    for i in 0..600 {
        write_file(&library.join(format!("g{i:04}.md")), &format!("# G{i}\n"));
    }
    write_file(
        &library.join("zzz.md"),
        "# Last\n\n- [One again](Trip/One.md)\n",
    );
    assert!(
        library::read(&library).omitted > 0,
        "the reader leaves it out"
    );
    let mut gate = FileGate::new();
    let done = runs::undo(&path, &record, &library, &mut gate);
    assert!(
        done.removed_notes.is_empty(),
        "undo removed {:?}",
        done.removed_notes
    );
    assert_eq!(read(&library.join("Trip/One.md")), "# One\n");
}

// ---- A5: Finish writes the line the record holds ---------------------------------------------------

#[test]
fn a_finished_run_is_undone_with_its_lines_and_its_notes() {
    let dir = scratch("finish");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let req = request(
        &library,
        "Trip",
        true,
        vec![note(&library, "Trip/One.md", "One", "# One\n", &[])],
    );
    let mut refusing = FileGate::new();
    refusing.refuse = true;
    let report = runs::write_at(&req, &mut refusing, 3, STAMP);
    assert_eq!(report.state(), Some(State::LinksPending));
    let path = report.record_path.clone().expect("a record");
    let record = runs::load(&path, &library).expect("the record");
    let mut gate = FileGate::new();
    let finished = runs::finish(&path, &record, &library, &mut gate);
    assert_eq!(finished.added, 1, "{:?}", finished.failures);
    assert!(
        read(&library.join("Trip.md")).contains("- [One](Trip/One.md)"),
        "the line is the one the record holds: {}",
        read(&library.join("Trip.md"))
    );
    let record = runs::load(&path, &library).expect("the record");
    let plan = runs::prepare(&record, &library);
    assert!(plan.links_kept.is_empty(), "{plan:?}");
    let done = runs::undo(&path, &record, &library, &mut gate);
    assert_eq!(done.removed_links, 1);
    assert_eq!(done.removed_notes.len(), 1);
    assert!(!library.join("Trip/One.md").exists());
    assert!(!library.join("Trip.md").exists());
}

// ---- A3: the original ------------------------------------------------------------------------------

#[test]
fn undo_is_refused_when_the_original_is_gone_and_nothing_is_removed() {
    let (library, req, path, record) = one_note_run("gone");
    std::fs::remove_file(&req.source).expect("the owner deletes the original");
    let plan = runs::prepare(&record, &library);
    let why = plan.refusal.clone().expect("refused");
    assert!(why.contains("is not where it was"), "{why}");
    assert!(why.contains("may be the only copy"), "{why}");
    let mut gate = FileGate::new();
    let done = runs::undo(&path, &record, &library, &mut gate);
    assert_eq!(done.refusal.as_deref(), Some(why.as_str()));
    assert!(done.removed_notes.is_empty());
    assert_eq!(
        read(&library.join("Trip/One.md")),
        "# One\n",
        "the only copy is still there"
    );
    assert!(read(&library.join("Trip.md")).contains("[One]"));
    assert_eq!(runs::load(&path, &library).expect("it").state, State::Done);
}

#[test]
fn undo_is_refused_when_the_original_has_changed_since() {
    let (library, req, path, record) = one_note_run("changed");
    write_file(&req.source, "# One\n\ntext, cut down\n");
    let plan = runs::prepare(&record, &library);
    let why = plan.refusal.clone().expect("refused");
    assert!(
        why.contains("is not the file these notes were cut from any more"),
        "{why}"
    );
    // The same length with other bytes is a change as well.
    write_file(&req.source, "# Two\n\ntext\n");
    assert_eq!(
        std::fs::metadata(&req.source).expect("it").len(),
        record.source.len
    );
    let plan = runs::prepare(&record, &library);
    assert!(
        plan.refusal.is_some(),
        "the digest is compared, not only the length"
    );
    let mut gate = FileGate::new();
    let done = runs::undo(&path, &record, &library, &mut gate);
    assert!(done.removed_notes.is_empty());
    assert_eq!(read(&library.join("Trip/One.md")), "# One\n");
    // Put back as it was, undo runs.
    write_file(&req.source, "# One\n\ntext\n");
    let done = runs::undo(&path, &record, &library, &mut gate);
    assert_eq!(done.removed_notes.len(), 1, "{done:?}");
}

// ---- A4, D4: undo touches only what the run created -------------------------------------------------

#[test]
fn a_note_that_was_there_already_is_not_removed_by_undo_and_its_line_is() {
    let dir = scratch("present");
    let library = dir.join("library");
    // The owner's own file, made before the run, with the bytes the note would have.
    let mine = library.join("Trip/One.md");
    write_file(&mine, "# One\n");
    let req = request(
        &library,
        "Trip",
        true,
        vec![note(&library, "Trip/One.md", "One", "# One\n", &[])],
    );
    let mut gate = FileGate::new();
    let report = runs::write_at(&req, &mut gate, 4, STAMP);
    assert!(report.notes_written.is_empty(), "the run wrote no note");
    assert_eq!(report.notes_present, vec![mine.clone()]);
    let path = report.record_path.clone().expect("a record");
    let record = runs::load(&path, &library).expect("the record");
    let plan = runs::prepare(&record, &library);
    let words = plan.sentence("Trip");
    let done = runs::undo(&path, &record, &library, &mut gate);
    // What is on disk first: the owner's file is where it was, and the run's line is gone.
    assert_eq!(read(&mine), "# One\n", "the owner's file is where it was");
    assert!(
        done.removed_notes.is_empty(),
        "undo removed {:?}",
        done.removed_notes
    );
    assert_eq!(done.removed_links, 1, "the line the run added is taken out");
    assert!(
        !library.join("Trip.md").exists(),
        "and nothing is left in the group's file"
    );
    // And what the record and the question said.
    assert!(
        !record.notes[0].created,
        "the record says it did not make it"
    );
    assert_eq!(plan.before, vec![mine.clone()]);
    assert!(
        words.contains("1 file was there before this run and is kept."),
        "{words}"
    );
}

#[test]
fn undo_removes_the_folders_the_run_made_and_only_those() {
    let dir = scratch("folders");
    let library = dir.join("library");
    // `Trip` was there before the run; `Trip/Japan` and `Trip/Japan/Tokyo` are the run's.
    std::fs::create_dir_all(library.join("Trip")).expect("a folder of the owner's");
    let req = request(
        &library,
        "Trip",
        true,
        vec![note(
            &library,
            "Trip/Japan/Tokyo/Asakusa.md",
            "Asakusa",
            "# Asakusa\n",
            &["Japan", "Tokyo"],
        )],
    );
    // What `plan_request` lists: the note's own folder and the group's, not `Trip/Japan`.
    assert_eq!(
        req.folders,
        vec![library.join("Trip"), library.join("Trip/Japan/Tokyo")]
    );
    let mut gate = FileGate::new();
    let report = runs::write_at(&req, &mut gate, 5, STAMP);
    assert_eq!(report.state(), Some(State::Done));
    let path = report.record_path.clone().expect("a record");
    let record = runs::load(&path, &library).expect("the record");
    assert_eq!(
        record.folders,
        vec![library.join("Trip/Japan"), library.join("Trip/Japan/Tokyo")],
        "the run made these two, and not `Trip`"
    );
    let done = runs::undo(&path, &record, &library, &mut gate);
    assert_eq!(done.removed_notes.len(), 1);
    assert!(
        !library.join("Trip/Japan").exists(),
        "the folder between is the run's"
    );
    assert!(library.join("Trip").is_dir(), "the owner's folder stays");
}

#[test]
fn undo_removes_the_folders_a_run_made_from_nothing_down_to_the_group_folder() {
    let dir = scratch("nested");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let req = request(
        &library,
        "Trip",
        true,
        vec![note(
            &library,
            "Trip/Japan/Tokyo/Asakusa.md",
            "Asakusa",
            "# Asakusa\n",
            &["Japan", "Tokyo"],
        )],
    );
    let mut gate = FileGate::new();
    let report = runs::write_at(&req, &mut gate, 6, STAMP);
    let path = report.record_path.clone().expect("a record");
    let record = runs::load(&path, &library).expect("the record");
    runs::undo(&path, &record, &library, &mut gate);
    assert!(
        !library.join("Trip").exists(),
        "Trip and what is under it are the run's"
    );
    assert!(!library.join("Trip.md").exists());
}

#[test]
fn a_group_file_that_was_there_is_not_removed_though_it_holds_only_a_heading_and_a_home() {
    let dir = scratch("group");
    let library = dir.join("library");
    write_file(&library.join("Trip.md"), "# Trip\nhome: Trip\n");
    let req = request(
        &library,
        "Trip",
        true,
        vec![note(&library, "Trip/One.md", "One", "# One\n", &[])],
    );
    let mut gate = FileGate::new();
    let report = runs::write_at(&req, &mut gate, 7, STAMP);
    assert!(
        report.group_failure.is_some(),
        "the file is there: the run cannot make it"
    );
    let path = report.record_path.clone().expect("a record");
    let record = runs::load(&path, &library).expect("the record");
    runs::undo(&path, &record, &library, &mut gate);
    assert_eq!(
        read(&library.join("Trip.md")),
        "# Trip\nhome: Trip\n",
        "the group's file was there before the run and is as it was"
    );
    assert!(
        !library.join("Trip/One.md").exists(),
        "the note the run made goes"
    );
    assert!(
        !record.group.created,
        "and the record does not say the run made the file"
    );
}

#[test]
fn a_record_with_no_created_field_removes_no_file() {
    // A record from before the field was written: undo takes the lines out and touches no file.
    let (library, _req, path, record) = one_note_run("old-record");
    let text = std::fs::read_to_string(&path).expect("the record");
    let old = text.replace("\"created\": true, ", "");
    assert_ne!(text, old, "the record has the field");
    write_file(&path, &old);
    let record_old = runs::load(&path, &library).expect("it reads");
    assert!(!record_old.notes[0].created && !record_old.group.created);
    assert_eq!(
        record_old.folders, record.folders,
        "folders are listed as they were"
    );
    let mut gate = FileGate::new();
    let done = runs::undo(&path, &record_old, &library, &mut gate);
    assert!(
        done.removed_notes.is_empty(),
        "undo removed {:?}",
        done.removed_notes
    );
    assert_eq!(read(&library.join("Trip/One.md")), "# One\n");
    assert!(
        library.join("Trip.md").exists(),
        "the group file is not its own either"
    );
}

#[test]
fn a_picture_that_was_there_is_not_removed_and_one_the_run_made_is() {
    let dir = scratch("pictures");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let mut req = request(
        &library,
        "Trip",
        true,
        vec![note(&library, "Trip/One.md", "One", "# One\n", &[])],
    );
    write_file(&dir.join("a.png"), "PNG one");
    write_file(&dir.join("b.png"), "PNG two");
    write_file(&library.join("Trip/media/b.png"), "PNG two");
    req.folders.push(library.join("Trip/media"));
    req.pictures = vec![
        NewPicture {
            source: dir.join("a.png"),
            folder: library.join("Trip/media"),
        },
        NewPicture {
            source: dir.join("b.png"),
            folder: library.join("Trip/media"),
        },
    ];
    let mut gate = FileGate::new();
    let report = runs::write_at(&req, &mut gate, 8, STAMP);
    assert_eq!(report.pictures_written.len(), 2, "{report:?}");
    let path = report.record_path.clone().expect("a record");
    let record = runs::load(&path, &library).expect("the record");
    let made: Vec<bool> = record.pictures.iter().map(|p| p.created).collect();
    assert_eq!(made, vec![true, false]);
    let done = runs::undo(&path, &record, &library, &mut gate);
    assert_eq!(
        done.removed_pictures,
        vec![library.join("Trip/media/a.png")]
    );
    assert_eq!(
        read(&library.join("Trip/media/b.png")),
        "PNG two",
        "the owner's copy stays"
    );
}

// ---- A9, B2: a record is a file format -------------------------------------------------------------

fn hostile_record(_library: &Path, source: &Path, group: &str, notes: &str) -> String {
    let bytes = std::fs::read(source).expect("the source");
    format!(
        "{{\"version\": 1, \"kind\": \"split\", \"started\": \"{STAMP}\", \"state\": \"done\",\n\
         \"source\": {{\"path\": \"{}\", \"len\": {}, \"sha256\": \"{}\"}},\n\
         \"group\": {group},\n\"notes\": [{notes}]}}",
        source.to_string_lossy().replace('\\', "/"),
        bytes.len(),
        mdshared_app::update::sha256_hex(&bytes),
    )
}

fn source_in(dir: &Path) -> PathBuf {
    let source = dir.join("Source.md");
    write_file(&source, "# One\n\ntext\n");
    source
}

#[test]
fn a_relative_path_with_dots_in_a_record_reaches_nothing_outside_the_library() {
    let dir = scratch("dots");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let taxes = dir.join("taxes.md");
    write_file(&taxes, "what I owe\n");
    let json = hostile_record(
        &library,
        &source_in(&dir),
        "{\"file\": \"Trip.md\", \"home\": \"Trip\", \"created\": true}",
        &format!(
            "{{\"path\": \"../taxes.md\", \"created\": true, \"sha256\": \"{}\", \"line\": \"- [x](x.md)\"}}",
            digest("what I owe\n")
        ),
    );
    let record = Record::from_json(&json, &library).expect("it reads as a record");
    // With the group's library file gone, the folder cannot be checked, and the sentence says
    // that instead (audit 4, W2). Nothing is acted on either way.
    let plan = runs::prepare(&record, &library);
    let why = plan.refusal.clone().expect("the whole record is refused");
    assert!(
        why.contains("taxes.md") && why.contains("is not there"),
        "{why}"
    );
    // The group's library file is there and declares its home: the dots still reach outside it.
    write_file(&library.join("Trip.md"), "# Trip\nhome: Trip\n");
    let plan = runs::prepare(&record, &library);
    let why = plan.refusal.clone().expect("the whole record is refused");
    assert!(why.contains("taxes.md"), "it says which path: {why}");
    assert!(
        why.ends_with("is outside the library and the group's folder, so this run is not undone."),
        "{why}"
    );
    assert!(plan.notes.is_empty() && plan.links.is_empty());
    let mut gate = FileGate::new();
    let done = runs::undo(&dir.join("r.json"), &record, &library, &mut gate);
    assert!(done.refusal.is_some());
    assert_eq!(
        read(&taxes),
        "what I owe\n",
        "the owner's file is where it was"
    );
}

#[test]
fn the_group_home_of_a_record_is_not_taken_from_the_record() {
    // The record names a folder outside the library as the group's home, and a note in it. The
    // group's library file, which is what a home is, does not say so: the record gets nothing.
    let dir = scratch("home");
    let library = dir.join("library");
    let outside = dir.join("elsewhere");
    let mine = outside.join("mine.md");
    write_file(&mine, "words\n");
    write_file(&library.join("Trip.md"), "# Trip\n");
    let source = source_in(&dir);
    let group = format!(
        "{{\"file\": \"Trip.md\", \"home\": \"{}\", \"created\": true}}",
        outside.to_string_lossy().replace('\\', "/")
    );
    let notes = format!(
        "{{\"path\": \"{}\", \"created\": true, \"sha256\": \"{}\", \"line\": \"- [m](m.md)\"}}",
        mine.to_string_lossy().replace('\\', "/"),
        digest("words\n")
    );
    let json = hostile_record(&library, &source, &group, &notes);
    let record = Record::from_json(&json, &library).expect("a record");
    let why = runs::prepare(&record, &library).refusal.expect("refused");
    assert!(why.contains("mine.md"), "{why}");
    assert_eq!(read(&mine), "words\n");
    // The group's library file declares that home now: then the record may name a note in it.
    write_file(
        &library.join("Trip.md"),
        &format!(
            "# Trip\nhome: {}\n",
            outside.to_string_lossy().replace('\\', "/")
        ),
    );
    let plan = runs::prepare(&record, &library);
    assert_eq!(plan.refusal, None, "{plan:?}");
    assert_eq!(plan.notes, vec![mine.clone()]);
}

#[test]
fn the_groups_file_of_a_record_is_a_md_file_in_the_library_folder() {
    let dir = scratch("groupfile");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let victim = dir.join("diary.md");
    write_file(&victim, "# Diary\n\n- [x](x.md)\n");
    let source = source_in(&dir);
    for file in [
        victim.to_string_lossy().replace('\\', "/"),
        "sub/Trip.md".to_string(),
        "Trip.txt".to_string(),
    ] {
        let group = format!("{{\"file\": \"{file}\", \"home\": \"Trip\", \"created\": true}}");
        let json = hostile_record(&library, &source, &group, "");
        let record = Record::from_json(&json, &library).expect("a record");
        let why = runs::prepare(&record, &library).refusal;
        assert!(why.is_some(), "{file} is not a group's library file");
        let mut gate = FileGate::new();
        let done = runs::undo(&dir.join("r.json"), &record, &library, &mut gate);
        assert!(done.refusal.is_some(), "{file}");
        let finished = runs::finish(&dir.join("r.json"), &record, &library, &mut gate);
        assert!(finished.error.is_some(), "Finish writes there too: {file}");
    }
    assert_eq!(
        read(&victim),
        "# Diary\n\n- [x](x.md)\n",
        "nothing was written to it"
    );
}

#[test]
fn only_the_files_a_run_writes_are_removed_by_extension() {
    let dir = scratch("ext");
    let library = dir.join("library");
    let secret = library.join("Trip/secret.txt");
    write_file(&secret, "not a note\n");
    write_file(&library.join("Trip.md"), "# Trip\nhome: Trip\n");
    let source = source_in(&dir);
    let notes = format!(
        "{{\"path\": \"Trip/secret.txt\", \"created\": true, \"sha256\": \"{}\", \"line\": \"- [x](x.md)\"}}",
        digest("not a note\n")
    );
    let json = hostile_record(
        &library,
        &source,
        "{\"file\": \"Trip.md\", \"home\": \"Trip\", \"created\": true}",
        &notes,
    );
    let record = Record::from_json(&json, &library).expect("a record");
    let why = runs::prepare(&record, &library).refusal.expect("refused");
    assert!(
        why.contains("secret.txt") && why.contains("not a file a run writes"),
        "{why}"
    );
    assert_eq!(read(&secret), "not a note\n");
}

#[test]
fn a_directory_where_a_note_was_is_kept_and_not_removed() {
    let dir = scratch("dirnote");
    let library = dir.join("library");
    write_file(&library.join("Trip/inner.md/x.txt"), "x\n");
    write_file(&library.join("Trip.md"), "# Trip\nhome: Trip\n");
    let source = source_in(&dir);
    let notes = "{\"path\": \"Trip/inner.md\", \"created\": true, \"sha256\": \"\", \"line\": \"- [x](x.md)\"}";
    let json = hostile_record(
        &library,
        &source,
        "{\"file\": \"Trip.md\", \"home\": \"Trip\", \"created\": true}",
        notes,
    );
    let record = Record::from_json(&json, &library).expect("a record");
    let plan = runs::prepare(&record, &library);
    assert!(plan.notes.is_empty(), "undo would remove {:?}", plan.notes);
    assert_eq!(plan.kept, vec![library.join("Trip/inner.md")]);
}

#[test]
fn an_empty_library_folder_is_not_a_folder_that_holds_everything() {
    let dir = scratch("empty-library");
    let note_path = dir.join("Trip/One.md");
    let mut req = request(
        &dir.join("library"),
        "Trip",
        true,
        vec![note(
            &dir.join("library"),
            "Trip/One.md",
            "One",
            "# One\n",
            &[],
        )],
    );
    req.library = PathBuf::new();
    req.group.home = PathBuf::new();
    let mut gate = FileGate::new();
    let report = runs::write_at(&req, &mut gate, 9, STAMP);
    assert!(report.start_error.is_some(), "{report:?}");
    assert!(!note_path.exists());
    assert!(!dir.join("library").join("Trip").exists());
    let (library, _req, path, record) = one_note_run("empty-undo");
    let done = runs::undo(&path, &record, Path::new(""), &mut gate);
    assert!(done.refusal.is_some(), "no library folder, no undo");
    assert_eq!(read(&library.join("Trip/One.md")), "# One\n");
}

#[test]
fn a_note_larger_than_undo_reads_is_kept_unread() {
    let dir = scratch("big");
    let library = dir.join("library");
    write_file(&library.join("Trip.md"), "# Trip\nhome: Trip\n");
    let big = library.join("Trip/Big.md");
    let size = runs::DIGEST_MAX_BYTES as usize + 1;
    let bytes = vec![b'a'; size];
    std::fs::create_dir_all(big.parent().expect("a parent")).expect("a folder");
    std::fs::write(&big, &bytes).expect("a big file");
    let source = source_in(&dir);
    let notes = format!(
        "{{\"path\": \"Trip/Big.md\", \"created\": true, \"sha256\": \"{}\", \"line\": \"- [x](x.md)\"}}",
        mdshared_app::update::sha256_hex(&bytes)
    );
    let json = hostile_record(
        &library,
        &source,
        "{\"file\": \"Trip.md\", \"home\": \"Trip\", \"created\": true}",
        &notes,
    );
    let record = Record::from_json(&json, &library).expect("a record");
    let plan = runs::prepare(&record, &library);
    assert!(
        plan.notes.is_empty(),
        "its digest is not worked out: {plan:?}"
    );
    assert_eq!(plan.kept, vec![big.clone()]);
    assert!(big.exists());
}

#[test]
fn an_original_larger_than_undo_reads_is_not_read_and_undo_is_refused() {
    let (library, _req, path, mut record) = one_note_run("big-source");
    let big = library.parent().expect("a parent").join("Big source.md");
    let size = runs::DIGEST_MAX_BYTES + 1;
    std::fs::File::create(&big)
        .expect("a file")
        .set_len(size)
        .expect("its length");
    record.source.path = big;
    record.source.len = size;
    let plan = runs::prepare(&record, &library);
    let why = plan.refusal.clone().expect("refused");
    assert!(why.contains("too large to be checked"), "{why}");
    let mut gate = FileGate::new();
    let done = runs::undo(&path, &record, &library, &mut gate);
    assert!(done.removed_notes.is_empty());
    assert_eq!(read(&library.join("Trip/One.md")), "# One\n");
}
/// A link at `link` to the file `target`, when this account may make one (Windows asks for a
/// privilege or developer mode); `false` when it may not.
fn make_file_link(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, link);
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(target, link);
    made.is_ok()
}

#[test]
fn a_link_where_a_note_was_is_not_followed_and_not_removed() {
    let dir = scratch("link");
    let library = dir.join("library");
    write_file(&library.join("Trip.md"), "# Trip\nhome: Trip\n");
    let source = source_in(&dir);
    let json = |digest: &str| {
        hostile_record(
            &library,
            &source,
            "{\"file\": \"Trip.md\", \"home\": \"Trip\", \"created\": true}",
            &format!(
                "{{\"path\": \"Trip/One.md\", \"created\": true, \"sha256\": \"{digest}\", \"line\": \"- [x](x.md)\"}}"
            ),
        )
    };
    // A link inside the library to a note that is inside it: the link is not a regular file.
    let inside = library.join("Trip/real.md");
    write_file(&inside, "# One\n");
    if !make_file_link(&inside, &library.join("Trip/One.md")) {
        eprintln!("skipped: this account may not make a symbolic link");
        return;
    }
    let record = Record::from_json(&json(&digest("# One\n")), &library).expect("a record");
    let plan = runs::prepare(&record, &library);
    assert!(plan.notes.is_empty(), "undo would remove {:?}", plan.notes);
    assert_eq!(plan.kept, vec![library.join("Trip/One.md")]);
    // A link to a file outside the library: the whole record is refused.
    std::fs::remove_file(library.join("Trip/One.md")).expect("the link goes");
    let outside = dir.join("elsewhere.md");
    write_file(&outside, "# One\n");
    assert!(make_file_link(&outside, &library.join("Trip/One.md")));
    let plan = runs::prepare(&record, &library);
    assert!(plan.refusal.is_some(), "{plan:?}");
    assert_eq!(
        read(&outside),
        "# One\n",
        "what the link leads to is where it was"
    );
}
#[test]
fn a_library_too_large_to_read_in_full_is_not_a_library_undo_acts_on() {
    let (library, _req, path, record) = one_note_run("incomplete");
    for i in 0..4100 {
        write_file(&library.join(format!("g{i:05}.md")), "# G\n");
    }
    let plan = runs::prepare(&record, &library);
    let why = plan.refusal.clone().expect("refused");
    assert!(why.contains("could not be read in full"), "{why}");
    let mut gate = FileGate::new();
    let done = runs::undo(&path, &record, &library, &mut gate);
    assert!(done.removed_notes.is_empty());
    assert_eq!(read(&library.join("Trip/One.md")), "# One\n");
}
// ---- A10: records of a later version ---------------------------------------------------------------

#[test]
fn a_record_of_a_later_version_is_neither_read_nor_pruned_and_is_listed_as_newer() {
    let dir = scratch("newer");
    let library = dir.join("library");
    std::fs::create_dir_all(library.join(runs::RUNS_DIR)).expect("the runs folder");
    let now = runs::now_utc().0;
    let (_l, _r, _p, record) = one_note_run("newer-model");
    let make = |secs: u64| Record {
        started: runs::utc(secs),
        started_secs: secs,
        ..record.clone()
    };
    for i in 0..runs::KEEP_RUNS as u64 + 5 {
        make(now - 100 * 86_400 - 60 * (i + 1))
            .create(&library)
            .expect("an old record");
    }
    let newer = library
        .join(runs::RUNS_DIR)
        .join("2026-11-21T14-03-11Z-import.json");
    let text = make(now - 30)
        .to_json(&library)
        .replace("\"kind\": \"split\"", "\"kind\": \"import\"")
        .replace("\"version\": 1", "\"version\": 2");
    write_file(&newer, &text);
    write_file(&newer.with_extension("done"), "started\n");
    // Version 1 with a kind this version has no word for is a later version's as well.
    let kind_only = library
        .join(runs::RUNS_DIR)
        .join("2026-11-21T14-03-12Z-fold.json");
    write_file(
        &kind_only,
        &make(now - 29)
            .to_json(&library)
            .replace("\"kind\": \"split\"", "\"kind\": \"fold\""),
    );
    let broken = library
        .join(runs::RUNS_DIR)
        .join("2026-11-21T14-03-13Z-split.json");
    write_file(&broken, "{ not json");
    assert!(
        runs::load(&newer, &library).is_none(),
        "this version cannot read it"
    );
    runs::prune(&library, now);
    assert!(newer.exists(), "the newer record is not pruned");
    assert!(newer.with_extension("done").exists());
    assert!(kind_only.exists());
    assert!(broken.exists(), "what does not load is never deleted");
    let (list, unread) = runs::list_all(&library);
    assert_eq!(
        list.len(),
        runs::KEEP_RUNS,
        "the newest twenty that load stay"
    );
    let names: Vec<(bool, bool)> = unread
        .iter()
        .map(|u| (u.path == newer || u.path == kind_only, u.newer))
        .collect();
    assert_eq!(unread.len(), 3, "{unread:?}");
    assert!(names.contains(&(true, true)));
    assert!(
        unread.iter().any(|u| u.path == broken && !u.newer),
        "a file that is no record is not called newer"
    );
    let shown = unread.iter().find(|u| u.path == newer).expect("listed");
    assert_eq!(shown.started, "2026-11-21T14:03:11Z");
}

#[test]
fn a_record_of_a_later_version_is_refused_as_a_record() {
    let (library, _req, path, record) = one_note_run("version");
    let text = record
        .to_json(&library)
        .replace("\"version\": 1", "\"version\": 2");
    assert!(Record::from_json(&text, &library).is_none());
    assert!(
        runs::load(&path, &library).is_some(),
        "this version's own record loads"
    );
    let text = record
        .to_json(&library)
        .replace("\"version\": 1", "\"version\": 99999999999");
    assert!(Record::from_json(&text, &library).is_none());
}
