//! The tests of `moves`: each rule of `docs/ai-library-plan.md` 5.12 has one that fails when the
//! code that keeps it is taken out, and each failure point of the order of a move has one that
//! forces it and says exactly what is left on disk.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn scratch(tag: &str) -> PathBuf {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mdedit-moves-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    // The temporary folder may be a link (macOS): the plan compares what the disk says.
    mdshared_app::paths::canonical_or_absolute(&dir)
}

fn put(path: &Path, text: &str) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).expect("its folder");
    }
    std::fs::write(path, text).expect("a file");
}

fn text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// A path as a library file writes it.
fn slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// A library with one group, `Work`, whose notes are in the library's own `Work` folder, and a
/// folder the group is pointed at that is outside the library.
struct World {
    dir: PathBuf,
    library: PathBuf,
    work: PathBuf,
    file: PathBuf,
    original: String,
}

fn world(tag: &str) -> World {
    let dir = scratch(tag);
    let library = dir.join("library");
    let work = dir.join("work");
    put(&library.join("Work/Tokyo/a.md"), "# A\n");
    put(&library.join("Work/b.md"), "# B\n");
    let file = library.join("Work.md");
    let original = format!(
        "# Work\n\nhome: {}\n\n## Tokyo\n\n- [A](Work/Tokyo/a.md)\n\n## Food\n\n- [B](Work/b.md)\n",
        slashes(&work)
    );
    put(&file, &original);
    World {
        dir,
        library,
        work,
        file,
        original,
    }
}

fn group_of(w: &World) -> library::Group {
    library::read(&w.library)
        .groups
        .into_iter()
        .find(|g| g.name == "Work")
        .expect("the group")
}

/// The window's gate without a window: the same writers, one atomic write, and a way to refuse.
#[derive(Default)]
struct TestGate {
    open: Vec<PathBuf>,
    fail_edit: Option<usize>,
    fail_restore: bool,
    edits: usize,
    probe: Option<PathBuf>,
    probe_seen: Vec<bool>,
}

impl MoveGate for TestGate {
    fn free(&mut self, file: &Path) -> Result<(), String> {
        if self.open.iter().any(|p| same_path(p, file)) {
            return Err(format!(
                "Save {} first: it has changes that are not saved.",
                shown(file)
            ));
        }
        Ok(())
    }

    fn edit(
        &mut self,
        file: &Path,
        base: &Path,
        edits: &[Edit],
        expect: &Expected,
    ) -> Result<(), String> {
        self.edits += 1;
        if let Some(p) = &self.probe {
            self.probe_seen.push(p.exists());
        }
        if self.fail_restore && expect.exact.is_some() {
            return Err("the library file could not be written".to_string());
        }
        if self.fail_edit == Some(self.edits) {
            return Err("the library file could not be written".to_string());
        }
        self.free(file)?;
        let old = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        let new = apply_edits(&old, edits, expect, base)?;
        mdshared_app::settings::write_atomic(file, &new).map_err(|e| e.to_string())
    }
}

/// The real disk, with a way to make one step fail or cross a volume.
struct Flaky {
    disk: Disk,
    cross: bool,
    renames: usize,
    /// Every rename from this one on fails.
    fail_renames_from: Option<usize>,
    /// A file with this name appears at the destination just before a rename.
    race: Option<(String, String)>,
    /// What `create_new` writes instead of the bytes it is given.
    corrupt: Option<Vec<u8>>,
    /// A file made new is, at once, changed by someone else: this one, to these bytes.
    change_after_copy: Option<(PathBuf, String)>,
    fail_remove: Vec<PathBuf>,
    /// This must exist when the first file moves (the record).
    record_dir: Option<PathBuf>,
    record_seen: Vec<bool>,
    /// How many files the record said were done, each time a file was about to move.
    record_done: Vec<usize>,
}

fn flaky() -> Flaky {
    Flaky {
        disk: Disk::portable(),
        cross: false,
        renames: 0,
        fail_renames_from: None,
        race: None,
        corrupt: None,
        change_after_copy: None,
        fail_remove: Vec::new(),
        record_dir: None,
        record_seen: Vec::new(),
        record_done: Vec::new(),
    }
}

impl Files for Flaky {
    fn rename_new(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        self.renames += 1;
        if let Some(dir) = &self.record_dir {
            let any = std::fs::read_dir(dir).is_ok_and(|d| {
                d.flatten()
                    .any(|e| e.path().extension().is_some_and(|x| x == "json"))
            });
            self.record_seen.push(any);
            let done = std::fs::read_dir(dir)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                .map(|e| text(&e.path()).matches("\"done\": true").count())
                .max()
                .unwrap_or(0);
            self.record_done.push(done);
        }
        if self.fail_renames_from.is_some_and(|n| self.renames >= n) {
            return Err(io::Error::other("the disk refused"));
        }
        if self.cross {
            return Err(io::ErrorKind::CrossesDevices.into());
        }
        if let Some((name, bytes)) = &self.race
            && to.file_name().is_some_and(|n| n.to_string_lossy() == *name)
        {
            put(to, bytes);
        }
        self.disk.rename_new(from, to)
    }

    fn create_new(&mut self, to: &Path, bytes: &[u8]) -> io::Result<()> {
        let bytes = self.corrupt.as_deref().unwrap_or(bytes);
        self.disk.create_new(to, bytes)?;
        if let Some((path, new)) = &self.change_after_copy {
            put(path, new);
        }
        Ok(())
    }

    fn read(&mut self, path: &Path) -> io::Result<Vec<u8>> {
        self.disk.read(path)
    }

    fn remove_file(&mut self, path: &Path) -> io::Result<()> {
        if self.fail_remove.iter().any(|p| same_path(p, path)) {
            return Err(io::Error::other("the disk refused"));
        }
        self.disk.remove_file(path)
    }

    fn create_dir_all(&mut self, path: &Path) -> io::Result<()> {
        self.disk.create_dir_all(path)
    }

    fn remove_dir(&mut self, path: &Path) -> io::Result<()> {
        self.disk.remove_dir(path)
    }
}

fn gate() -> TestGate {
    TestGate::default()
}

/// The plan of a group, as the window opens it.
fn group_plan(w: &World) -> Plan {
    plan_group(&w.library, &group_of(w))
}

fn records(library: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(library.join(runs::RUNS_DIR))
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect()
        })
        .unwrap_or_default()
}

fn do_write(req: &Request, gate: &mut TestGate, files: &mut Flaky) -> Report {
    write_at(req, gate, files, 1_800_000_000, "2027-01-15T08:00:00Z")
}

// ---- the plan -------------------------------------------------------------------------------------

#[test]
fn a_group_plan_has_a_row_per_note_that_is_not_where_its_folder_and_headings_put_it() {
    let w = world("plan");
    let plan = group_plan(&w);
    assert_eq!(plan.blocked, None);
    assert_eq!(plan.rows.len(), 2);
    assert_eq!(
        plan.start,
        Start::Group,
        "the notes are not in the group's folder"
    );
    assert_eq!(plan.rows[0].from, w.library.join("Work/Tokyo/a.md"));
    assert_eq!(plan.rows[0].to, w.work.join("Tokyo/a.md"));
    assert_eq!(plan.rows[1].to, w.work.join("Food/b.md"));
    assert!(
        plan.rows
            .iter()
            .all(|r| r.ticked && r.verdict == Verdict::Ready)
    );
    assert!(plan.rows[0].why.contains("not in the group's folder"));
    assert_eq!(
        plan.rows[0].sha256,
        mdshared_app::update::sha256_hex(b"# A\n")
    );
}

#[test]
fn a_note_already_where_its_headings_put_it_has_no_row_and_a_disagreeing_one_is_a_headings_move() {
    let w = world("headings");
    // The group's folder is the library's own `Work` folder, and `b.md` sits in the wrong one.
    let text = "# Work\n\nhome: Work\n\n## Tokyo\n\n- [A](Work/Tokyo/a.md)\n\n## Food\n\n- [B](Work/b.md)\n";
    put(&w.file, text);
    let plan = group_plan(&w);
    assert_eq!(plan.start, Start::Headings);
    assert_eq!(plan.rows.len(), 1, "a.md is in Work/Tokyo already");
    assert_eq!(plan.rows[0].from, w.library.join("Work/b.md"));
    assert_eq!(plan.rows[0].to, w.library.join("Work/Food/b.md"));
    assert!(plan.rows[0].why.contains("Food"), "{}", plan.rows[0].why);
}

#[test]
fn a_group_with_no_folder_has_no_plan() {
    let w = world("nohome");
    put(&w.file, "# Work\n\n## Tokyo\n\n- [A](Work/Tokyo/a.md)\n");
    let plan = group_plan(&w);
    assert!(
        plan.blocked
            .as_deref()
            .is_some_and(|b| b.contains("Set the group's folder"))
    );
    assert!(plan.rows.is_empty());
}

#[test]
fn a_folder_that_is_the_library_or_holds_it_has_no_plan() {
    let w = world("homelib");
    put(
        &w.file,
        &format!(
            "# Work\n\nhome: {}\n\n- [A](Work/Tokyo/a.md)\n",
            slashes(&w.library)
        ),
    );
    assert!(group_plan(&w).blocked.is_some());
    put(
        &w.file,
        &format!(
            "# Work\n\nhome: {}\n\n- [A](Work/Tokyo/a.md)\n",
            slashes(&w.dir)
        ),
    );
    assert!(
        group_plan(&w).blocked.is_some(),
        "a folder that holds the library"
    );
}

#[test]
fn a_taken_name_reads_there_is_already_a_file_and_comes_unticked_and_cannot_be_ticked() {
    let w = world("taken");
    put(&w.work.join("Tokyo/a.md"), "# Another A\n");
    let mut plan = group_plan(&w);
    assert_eq!(plan.rows[0].verdict, Verdict::Taken);
    assert_eq!(
        plan.rows[0].verdict.text(),
        "there is already a file called this"
    );
    assert!(!plan.rows[0].ticked);
    plan.set_ticked(0, true);
    assert!(!plan.rows[0].ticked, "a taken name is never ticked");
    plan.tick_all();
    assert!(!plan.rows[0].ticked && plan.rows[1].ticked);
    assert_eq!(
        plan.request().rows.len(),
        1,
        "only the free one is asked for"
    );
    // Never resolved with a number.
    assert!(!w.work.join("Tokyo/a 2.md").exists());
}

#[test]
fn two_notes_with_one_file_name_going_to_one_place_are_not_both_ticked() {
    let w = world("twice");
    put(&w.library.join("Work/Tokyo/x.md"), "# X1\n");
    put(&w.library.join("Work/Other/x.md"), "# X2\n");
    put(
        &w.file,
        &format!(
            "# Work\n\nhome: {}\n\n## Tokyo\n\n- [X1](Work/Tokyo/x.md)\n- [X2](Work/Other/x.md)\n",
            slashes(&w.work)
        ),
    );
    let plan = group_plan(&w);
    assert_eq!(plan.rows[0].verdict, Verdict::Ready);
    assert_eq!(
        plan.rows[1].verdict,
        Verdict::Taken,
        "an earlier row takes the name"
    );
}

#[test]
fn names_that_differ_only_in_case_collide_where_the_disk_folds_case() {
    let w = world("case");
    put(&w.library.join("Work/Tokyo/X.md"), "# X1\n");
    put(&w.library.join("Work/Other/x.md"), "# X2\n");
    put(
        &w.file,
        &format!(
            "# Work\n\nhome: {}\n\n## Tokyo\n\n- [X1](Work/Tokyo/X.md)\n- [X2](Work/Other/x.md)\n",
            slashes(&w.work)
        ),
    );
    let plan = group_plan(&w);
    let second = &plan.rows[1].verdict;
    if mdshared_app::paths::NAMES_IGNORE_CASE {
        assert_eq!(*second, Verdict::Taken);
    } else {
        // `x.md` is in `Other`, so its target section is Tokyo's: another name.
        assert_eq!(*second, Verdict::Ready);
    }
}

#[test]
fn a_note_outside_the_librarys_own_area_comes_unticked_and_says_where_it_lives() {
    let w = world("outside");
    let elsewhere = w.dir.join("Documents");
    put(&elsewhere.join("plan.md"), "# Plan\n");
    put(
        &w.file,
        &format!(
            "# Work\n\nhome: {}\n\n- [Plan]({})\n- [B](Work/b.md)\n",
            slashes(&w.work),
            slashes(&elsewhere.join("plan.md"))
        ),
    );
    let mut plan = group_plan(&w);
    let row = &plan.rows[0];
    assert!(row.outside && !row.ticked && !row.verdict.blocked());
    let said = row.verdict.text();
    assert!(
        said.contains("Documents") && said.contains("outside the library"),
        "{said}"
    );
    plan.tick_all();
    assert!(
        !plan.rows[0].ticked,
        "tick all does not take a file kept elsewhere"
    );
    plan.set_ticked(0, true);
    assert!(plan.rows[0].ticked, "the owner can tick it");
}

#[test]
fn a_symlink_a_missing_file_and_a_list_over_the_limit_are_blocked_not_moved() {
    let w = world("blocked");
    std::fs::remove_file(w.library.join("Work/b.md")).expect("removed");
    let plan = group_plan(&w);
    assert_eq!(plan.rows.len(), 1);
    assert_eq!(
        plan.missing, 1,
        "a note that is not on disk is counted, not a row"
    );
    let long = "n".repeat(PATH_MAX_CHARS);
    let mut g = group_of(&w);
    g.entries[0].path = w.library.join("Work/Tokyo/a.md");
    g.home = Some(w.work.join(&long));
    let plan = plan_group(&w.library, &g);
    assert!(matches!(plan.rows[0].verdict, Verdict::TooLong(n) if n > PATH_MAX_CHARS));
    assert!(!plan.rows[0].ticked);
}

// ---- a write -----------------------------------------------------------------------------------

#[test]
fn a_move_renames_the_files_rewrites_the_lines_and_writes_the_record_first() {
    let w = world("write");
    let plan = group_plan(&w);
    let mut files = flaky();
    files.record_dir = Some(w.library.join(runs::RUNS_DIR));
    let mut g = gate();
    let report = do_write(&plan.request(), &mut g, &mut files);
    assert_eq!(report.start_error, None);
    assert!(
        report.failed.is_empty() && report.rolled_back.is_none(),
        "{report:?}"
    );
    // The files are where the headings put them, byte for byte, and gone from where they were.
    assert_eq!(text(&w.work.join("Tokyo/a.md")), "# A\n");
    assert_eq!(text(&w.work.join("Food/b.md")), "# B\n");
    assert!(!w.library.join("Work/Tokyo/a.md").exists());
    // The lines lead there: the folder is outside the library, so the path is whole.
    let after = text(&w.file);
    assert!(
        after.contains(&format!("- [A]({})", slashes(&w.work.join("Tokyo/a.md")))),
        "{after}"
    );
    assert!(
        after.contains("## Tokyo") && after.contains("home: "),
        "{after}"
    );
    let read = library::read(&w.library);
    assert!(
        read.groups[0].entries.iter().all(|e| !e.missing),
        "no link is left dangling"
    );
    // The folders the move emptied are gone; the library folder itself is not.
    assert!(!w.library.join("Work").exists(), "the empty folders went");
    assert!(w.library.is_dir());
    // The record came first (it existed when the first file moved) and says what happened.
    assert_eq!(files.record_seen.first(), Some(&true));
    assert_eq!(
        files.record_done.first(),
        Some(&2),
        "the record named both files before the first one moved, so a run that dies half-way leaves nothing unnamed"
    );
    let path = report.record_path.clone().expect("a record");
    assert!(
        path.to_string_lossy()
            .ends_with("2027-01-15T08-00-00Z-move.json")
    );
    let record = runs::load(&path, &w.library).expect("it reads");
    assert_eq!(record.kind, Kind::Move);
    assert_eq!(record.state, State::Done);
    let m = record.moved.expect("the move");
    assert!(m.files.iter().all(|f| f.done && !f.sha256.is_empty()));
    assert_eq!(m.changes.len(), 2);
    assert_eq!(
        m.emptied.len(),
        2,
        "Work/Tokyo and Work, and nothing above the library"
    );
    assert!(path.with_extension("done").is_file());
}

#[test]
fn a_folder_that_holds_anything_else_is_left() {
    let w = world("holds");
    put(&w.library.join("Work/Tokyo/photo.png"), "not a note");
    let plan = group_plan(&w);
    let report = do_write(&plan.request(), &mut gate(), &mut flaky());
    assert!(report.failed.is_empty());
    assert!(
        w.library.join("Work/Tokyo/photo.png").is_file(),
        "the picture stays"
    );
    assert!(w.library.join("Work/Tokyo").is_dir(), "so does its folder");
    assert!(!w.library.join("Work/b.md").exists());
}

#[test]
fn a_file_that_appears_at_the_destination_before_the_rename_is_never_replaced() {
    let w = world("race");
    let plan = group_plan(&w);
    let mut files = flaky();
    files.race = Some(("a.md".to_string(), "# The owner's own\n".to_string()));
    let report = do_write(&plan.request(), &mut gate(), &mut files);
    assert_eq!(report.failed.len(), 1, "{report:?}");
    assert_eq!(
        text(&w.work.join("Tokyo/a.md")),
        "# The owner's own\n",
        "never replaced"
    );
    assert_eq!(
        text(&w.library.join("Work/Tokyo/a.md")),
        "# A\n",
        "the note is where it was"
    );
    // The note that did not move keeps its line; the one that did moved with its line.
    let after = text(&w.file);
    assert!(after.contains("(Work/Tokyo/a.md)"), "{after}");
    assert!(
        after.contains(&slashes(&w.work.join("Food/b.md"))),
        "{after}"
    );
}

#[test]
fn nothing_is_overwritten_by_a_request_that_names_a_taken_name() {
    let w = world("forced");
    put(&w.work.join("Tokyo/a.md"), "# Another A\n");
    let mut plan = group_plan(&w);
    plan.rows[0].verdict = Verdict::Ready;
    plan.rows[0].ticked = true;
    plan.rows[0].sha256 = mdshared_app::update::sha256_hex(b"# A\n");
    let report = do_write(&plan.request(), &mut gate(), &mut flaky());
    let why = report
        .start_error
        .expect("refused before the first file moves");
    assert!(why.contains("there is already a file called this"), "{why}");
    assert_eq!(text(&w.work.join("Tokyo/a.md")), "# Another A\n");
    assert_eq!(text(&w.library.join("Work/Tokyo/a.md")), "# A\n");
    assert_eq!(text(&w.file), w.original);
    assert!(records(&w.library).is_empty(), "no record: nothing started");
}

#[test]
fn a_library_file_open_with_unsaved_changes_refuses_the_whole_move_before_it_starts() {
    let w = world("open");
    let plan = group_plan(&w);
    let mut g = gate();
    g.open = vec![w.file.clone()];
    let report = do_write(&plan.request(), &mut g, &mut flaky());
    let why = report.start_error.expect("refused");
    assert!(
        why.contains("Work.md") && why.contains("not saved"),
        "{why}"
    );
    assert!(w.library.join("Work/Tokyo/a.md").is_file() && w.library.join("Work/b.md").is_file());
    assert!(!w.work.exists(), "not even the folder");
    assert_eq!(text(&w.file), w.original);
    assert!(records(&w.library).is_empty());
}

#[test]
fn a_note_open_with_unsaved_changes_refuses_the_move_too() {
    let w = world("opennote");
    let plan = group_plan(&w);
    let mut g = gate();
    g.open = vec![w.library.join("Work/b.md")];
    let report = do_write(&plan.request(), &mut g, &mut flaky());
    assert!(report.start_error.is_some_and(|e| e.contains("b.md")));
    assert!(
        w.library.join("Work/Tokyo/a.md").is_file(),
        "the other note did not go either"
    );
}

#[test]
fn a_source_that_changed_since_the_plan_refuses_before_the_first_file_moves() {
    let w = world("changed");
    let plan = group_plan(&w);
    put(
        &w.library.join("Work/b.md"),
        "# B, edited in another window\n",
    );
    let report = do_write(&plan.request(), &mut gate(), &mut flaky());
    let why = report.start_error.expect("refused");
    assert!(
        why.contains("has changed since this plan was made"),
        "{why}"
    );
    assert!(
        w.library.join("Work/Tokyo/a.md").is_file(),
        "the first note stayed too"
    );
    assert!(records(&w.library).is_empty());
}

#[test]
fn every_refusal_a_request_can_hit_leaves_nothing_moved_and_no_record() {
    // Each case edits a good request into a bad one.
    type Break = fn(&World, &mut Request);
    let cases: Vec<(&str, Break, &str)> = vec![
        (
            "a destination outside the library and the group's folder",
            |w, r| r.rows[0].to = w.dir.join("elsewhere/a.md"),
            "outside the library",
        ),
        (
            "a destination with a ..",
            |w, r| r.rows[0].to = w.work.join("../x/a.md"),
            "outside the library",
        ),
        (
            "a note that would become a library file",
            |w, r| r.rows[0].to = w.library.join("a.md"),
            "library file",
        ),
        (
            "a name that would change",
            |w, r| r.rows[0].to = w.work.join("Tokyo/renamed.md"),
            "change its name",
        ),
        (
            "an extension that is not a note's",
            |w, r| r.rows[0].to = w.work.join("Tokyo/a.txt"),
            "change its name",
        ),
        (
            "a folder that is a file",
            |w, r| {
                put(&w.work.join("Tokyo"), "a file where the folder goes");
                let _ = r;
            },
            "is a file where a folder",
        ),
        (
            "a path that is too long",
            |w, r| {
                r.rows[0].to = w.work.join("n".repeat(PATH_MAX_CHARS)).join("a.md");
            },
            "characters long",
        ),
        (
            "no library folder",
            |_, r| r.library = PathBuf::from("relative"),
            "no library folder",
        ),
        ("nothing ticked", |_, r| r.rows.clear(), "nothing is ticked"),
        (
            "the group's folder is the library folder",
            |w, r| {
                if let Some(g) = r.group.as_mut() {
                    g.home = w.library.clone();
                }
            },
            "library folder",
        ),
    ];
    for (name, break_it, expect) in cases {
        let w = world("refuse");
        let mut req = group_plan(&w).request();
        break_it(&w, &mut req);
        let report = do_write(&req, &mut gate(), &mut flaky());
        let why = report
            .start_error
            .unwrap_or_else(|| panic!("{name}: it was not refused"));
        assert!(why.contains(expect), "{name}: {why}");
        assert!(
            w.library.join("Work/Tokyo/a.md").is_file() && w.library.join("Work/b.md").is_file(),
            "{name}: a file moved"
        );
        assert_eq!(text(&w.file), w.original, "{name}: the library changed");
        assert!(
            records(&w.library).is_empty(),
            "{name}: a record was written"
        );
        let _ = std::fs::remove_dir_all(&w.dir);
    }
}

#[test]
fn a_library_file_refused_at_the_write_puts_every_file_back_and_the_library_as_it_was() {
    let w = world("rollback");
    let plan = group_plan(&w);
    let mut g = gate();
    g.fail_edit = Some(1);
    let report = do_write(&plan.request(), &mut g, &mut flaky());
    assert!(report.rolled_back.is_some(), "{report:?}");
    assert_eq!(report.put_back.len(), 2);
    assert!(report.not_put_back.is_empty());
    assert_eq!(text(&w.library.join("Work/Tokyo/a.md")), "# A\n");
    assert_eq!(text(&w.library.join("Work/b.md")), "# B\n");
    assert_eq!(
        text(&w.file),
        w.original,
        "not a byte of the library file differs"
    );
    assert!(
        !w.work.join("Tokyo").exists(),
        "the folders the run made are gone"
    );
    let record = runs::load(&report.record_path.expect("a record"), &w.library).expect("it reads");
    assert_eq!(record.state, State::Undone, "nothing is left to undo");
    assert!(record.moved.expect("move").files.iter().all(|f| !f.done));
}

#[test]
fn a_rollback_that_cannot_write_a_file_back_leaves_its_notes_where_it_says_they_are() {
    // Two library files need lines rewritten. The second is refused after the first was written,
    // and the first cannot be written back: its notes stay where its lines now name them.
    let (dir, old, new) = old_library("stuck");
    put(
        &old.join("Home.md"),
        &format!(
            "# Home\n\nhome: Home\n\n- [H](<{}>)\n",
            slashes(&old.join("Home/h.md"))
        ),
    );
    let plan = plan_library(&old, &new);
    let mut g = gate();
    g.fail_edit = Some(2);
    g.fail_restore = true;
    let report = do_write(&plan.request(), &mut g, &mut flaky());
    assert!(report.rolled_back.is_some(), "{report:?}");
    assert!(!report.not_put_back.is_empty(), "{report:?}");
    // Home.md was written first (it sorts first), so its note is where its line says.
    let home = text(&new.join("Home.md"));
    assert!(home.contains("(Home/h.md)"), "{home}");
    assert!(
        new.join("Home/h.md").is_file(),
        "the note stays where the written line leads"
    );
    assert!(
        library::read(&new)
            .groups
            .iter()
            .flat_map(|g| &g.entries)
            .filter(|e| e.title == "H")
            .all(|e| !e.missing),
        "no link of the written file is left dangling"
    );
    let record = runs::load(&report.record_path.expect("a record"), &new).expect("it reads");
    assert_eq!(record.state, State::LinksPending);
    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn a_file_that_cannot_be_put_back_is_reported_and_the_record_still_undoes_it() {
    let w = world("putback");
    let plan = group_plan(&w);
    let mut g = gate();
    g.fail_edit = Some(1);
    let mut files = flaky();
    // The first two renames go forward; every one after that (the put-back) fails.
    files.fail_renames_from = Some(3);
    let report = do_write(&plan.request(), &mut g, &mut files);
    assert!(report.rolled_back.is_some());
    assert_eq!(report.not_put_back.len(), 2, "{report:?}");
    assert!(w.work.join("Tokyo/a.md").is_file(), "it is where it says");
    let path = report.record_path.expect("a record");
    let record = runs::load(&path, &w.library).expect("it reads");
    assert_eq!(record.state, State::LinksPending);
    assert!(
        record
            .moved
            .as_ref()
            .expect("move")
            .files
            .iter()
            .all(|f| f.done)
    );
    // Undo, with a healthy disk, finishes what the put-back could not.
    let back = undo(&path, &record, &w.library, &mut gate(), &mut flaky());
    assert_eq!(back.start_error, None, "{back:?}");
    assert_eq!(text(&w.library.join("Work/Tokyo/a.md")), "# A\n");
    assert_eq!(text(&w.file), w.original);
}

// ---- across a volume -----------------------------------------------------------------------------

#[test]
fn across_a_volume_the_copy_is_read_back_and_the_source_goes_only_after_the_lines_are_written() {
    let w = world("cross");
    let plan = group_plan(&w);
    let mut g = gate();
    g.probe = Some(w.library.join("Work/Tokyo/a.md"));
    let mut files = flaky();
    files.cross = true;
    let report = do_write(&plan.request(), &mut g, &mut files);
    assert!(
        report.failed.is_empty() && report.rolled_back.is_none(),
        "{report:?}"
    );
    assert_eq!(
        g.probe_seen,
        vec![true],
        "the source was still there while the lines were written"
    );
    assert_eq!(text(&w.work.join("Tokyo/a.md")), "# A\n");
    assert!(
        !w.library.join("Work/Tokyo/a.md").exists(),
        "and gone after"
    );
    assert!(report.leftovers.is_empty());
}

#[test]
fn a_copy_that_does_not_read_back_the_same_is_removed_and_the_source_never_touched() {
    let w = world("corrupt");
    let plan = group_plan(&w);
    let mut files = flaky();
    files.cross = true;
    files.corrupt = Some(b"# damaged\n".to_vec());
    let report = do_write(&plan.request(), &mut gate(), &mut files);
    assert_eq!(report.failed.len(), 2, "{report:?}");
    assert!(report.failed[0].why.contains("does not read back the same"));
    assert!(
        !w.work.join("Tokyo/a.md").exists(),
        "the damaged copy was removed"
    );
    assert_eq!(text(&w.library.join("Work/Tokyo/a.md")), "# A\n");
    assert_eq!(
        text(&w.file),
        w.original,
        "no line changed for a note that did not move"
    );
}

#[test]
fn a_refused_library_file_removes_the_copies_and_keeps_every_source() {
    let w = world("crossrefused");
    let plan = group_plan(&w);
    let mut g = gate();
    g.fail_edit = Some(1);
    let mut files = flaky();
    files.cross = true;
    let report = do_write(&plan.request(), &mut g, &mut files);
    assert!(report.rolled_back.is_some());
    assert!(!w.work.join("Tokyo/a.md").exists() && !w.work.join("Food/b.md").exists());
    assert_eq!(text(&w.library.join("Work/Tokyo/a.md")), "# A\n");
    assert_eq!(text(&w.library.join("Work/b.md")), "# B\n");
    assert_eq!(text(&w.file), w.original);
}

#[test]
fn an_old_copy_that_cannot_be_removed_is_reported_and_the_note_is_listed_at_its_new_place() {
    let w = world("leftover");
    let plan = group_plan(&w);
    let mut files = flaky();
    files.cross = true;
    files.fail_remove = vec![w.library.join("Work/Tokyo/a.md")];
    let report = do_write(&plan.request(), &mut gate(), &mut files);
    assert_eq!(report.leftovers.len(), 1);
    assert!(report.leftovers[0].why.contains("could not be removed"));
    assert!(w.library.join("Work/Tokyo/a.md").is_file() && w.work.join("Tokyo/a.md").is_file());
    assert!(text(&w.file).contains(&slashes(&w.work.join("Tokyo/a.md"))));
    assert!(
        w.library.join("Work/Tokyo").is_dir(),
        "its folder holds the old copy, so it stays"
    );
}

#[test]
fn a_source_changed_while_it_was_being_copied_is_kept_and_reported() {
    let w = world("changedcopy");
    let plan = group_plan(&w);
    let mut files = flaky();
    files.cross = true;
    files.change_after_copy = Some((
        w.library.join("Work/Tokyo/a.md"),
        "# A, edited meanwhile\n".to_string(),
    ));
    let report = do_write(&plan.request(), &mut gate(), &mut files);
    assert!(
        report
            .leftovers
            .iter()
            .any(|f| f.why.contains("changed while it was being moved")),
        "{report:?}"
    );
    assert_eq!(
        text(&w.library.join("Work/Tokyo/a.md")),
        "# A, edited meanwhile\n"
    );
}

// ---- outside notes ---------------------------------------------------------------------------------

#[test]
fn a_ticked_note_from_outside_moves_and_only_its_own_empty_folder_goes() {
    let w = world("outmove");
    let elsewhere = w.dir.join("Documents/old");
    put(&elsewhere.join("plan.md"), "# Plan\n");
    put(
        &w.file,
        &format!(
            "# Work\n\nhome: {}\n\n- [Plan]({})\n",
            slashes(&w.work),
            slashes(&elsewhere.join("plan.md"))
        ),
    );
    let mut plan = group_plan(&w);
    plan.set_ticked(0, true);
    let report = do_write(&plan.request(), &mut gate(), &mut flaky());
    assert!(report.failed.is_empty(), "{report:?}");
    assert_eq!(text(&w.work.join("plan.md")), "# Plan\n");
    assert!(!elsewhere.exists(), "the folder it left empty went");
    assert!(w.dir.join("Documents").is_dir(), "but not the one above it");
    let record = runs::load(&report.record_path.expect("a record"), &w.library).expect("record");
    assert!(
        record.moved.expect("move").files[0].outside,
        "the record says it came from outside"
    );
}

// ---- the library moves -------------------------------------------------------------------------------

/// An old library with two groups, notes in folders, a record of an earlier run, and a file that
/// is nobody's note; and a new, empty library folder.
fn old_library(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = scratch(tag);
    let old = dir.join("old");
    let new = dir.join("new");
    put(&old.join("Work/a.md"), "# A\n");
    put(&old.join("Work/Tokyo/t.md"), "# T\n");
    put(&old.join("Home/h.md"), "# H\n");
    put(&old.join("Work/picture.png"), "png");
    put(
        &old.join("Work.md"),
        &format!(
            "# Work\n\nhome: Work\n\n- [A](Work/a.md)\n\n## Tokyo\n\n- [T](<{}>)\n",
            slashes(&old.join("Work/Tokyo/t.md"))
        ),
    );
    put(
        &old.join("Home.md"),
        "# Home\n\nhome: Home\n\n- [H](Home/h.md)\n",
    );
    put(
        &old.join(runs::RUNS_DIR)
            .join("2026-01-01T00-00-00Z-split.json"),
        "{}",
    );
    put(
        &old.join(runs::RUNS_DIR)
            .join("2026-01-01T00-00-00Z-split.done"),
        "x\n",
    );
    std::fs::create_dir_all(&new).expect("new");
    (dir, old, new)
}

#[test]
fn the_library_plan_has_the_library_files_the_notes_inside_and_the_records_and_nothing_outside() {
    let (dir, old, new) = old_library("libplan");
    let outside = dir.join("Elsewhere/o.md");
    put(&outside, "# O\n");
    let mut text = text(&old.join("Home.md"));
    text.push_str(&format!("- [O]({})\n", slashes(&outside)));
    put(&old.join("Home.md"), &text);
    let plan = plan_library(&old, &new);
    assert_eq!(plan.blocked, None);
    assert_eq!(plan.start, Start::Library);
    let kinds = |k: FileKind| plan.rows.iter().filter(|r| r.kind == k).count();
    assert_eq!(kinds(FileKind::Library), 2);
    assert_eq!(kinds(FileKind::Note), 3);
    assert_eq!(kinds(FileKind::Record), 2);
    assert!(
        plan.rows.iter().all(|r| r.from != outside),
        "nothing outside it"
    );
    assert_eq!(plan.others, 1, "the picture stays behind and is counted");
    let t = plan
        .rows
        .iter()
        .find(|r| r.from.ends_with("t.md"))
        .expect("t");
    assert_eq!(t.to, new.join("Work/Tokyo/t.md"));
    assert!(plan.rows.iter().all(|r| r.ticked));
}

#[test]
fn a_library_plan_between_folders_that_hold_one_another_or_are_one_is_blocked() {
    let (dir, old, _) = old_library("libnest");
    assert!(plan_library(&old, &old).blocked.is_some());
    assert!(plan_library(&old, &old.join("inside")).blocked.is_some());
    assert!(plan_library(&old, &dir).blocked.is_some());
    assert!(
        plan_library(&dir.join("missing"), &dir.join("new"))
            .blocked
            .is_some()
    );
}

#[test]
fn a_library_move_takes_the_library_files_the_notes_and_the_records_and_keeps_every_link() {
    let (dir, old, new) = old_library("libwrite");
    let before_work = text(&old.join("Work.md"));
    let plan = plan_library(&old, &new);
    let report = do_write(&plan.request(), &mut gate(), &mut flaky());
    assert_eq!(report.start_error, None);
    assert!(
        report.failed.is_empty() && report.rolled_back.is_none(),
        "{report:?}"
    );
    assert_eq!(text(&new.join("Work/a.md")), "# A\n");
    assert_eq!(text(&new.join("Home/h.md")), "# H\n");
    assert!(new.join("Work.md").is_file() && !old.join("Work.md").exists());
    // A relative link stays as it was written; the absolute one is made relative to the new
    // folder; the group's folder line, relative, is untouched.
    let after = text(&new.join("Work.md"));
    assert!(after.contains("- [A](Work/a.md)"), "{after}");
    assert!(after.contains("(Work/Tokyo/t.md)"), "{after}");
    assert!(
        !after.contains(&slashes(&old)),
        "no path of the old folder is left: {after}"
    );
    assert!(after.contains("home: Work"), "{after}");
    assert_ne!(after, before_work);
    let lib = library::read(&new);
    assert!(
        lib.groups
            .iter()
            .flat_map(|g| &g.entries)
            .all(|e| !e.missing),
        "every link leads to a file"
    );
    // The records of earlier runs went with it, and this run's own is among them.
    assert_eq!(records(&new).len(), 2);
    assert!(
        new.join(runs::RUNS_DIR)
            .join("2026-01-01T00-00-00Z-split.done")
            .is_file()
    );
    // The old folder keeps what was nobody's note, and so does the folder that holds it.
    assert!(old.join("Work/picture.png").is_file());
    assert!(old.join("Work").is_dir());
    assert!(!old.join("Home").exists(), "an empty folder goes");
    assert!(!old.join("Work/Tokyo").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_library_file_whose_name_is_taken_stays_and_its_notes_are_linked_from_where_it_is() {
    let (dir, old, new) = old_library("libtaken");
    put(&new.join("Home.md"), "# Another Home\n");
    let plan = plan_library(&old, &new);
    let row = plan
        .rows
        .iter()
        .find(|r| r.from.ends_with("Home.md"))
        .expect("row");
    assert_eq!(row.verdict, Verdict::Taken);
    assert!(!row.ticked);
    let report = do_write(&plan.request(), &mut gate(), &mut flaky());
    assert!(report.failed.is_empty(), "{report:?}");
    assert_eq!(
        text(&new.join("Home.md")),
        "# Another Home\n",
        "never replaced"
    );
    let stays = text(&old.join("Home.md"));
    assert!(stays.contains(&slashes(&new.join("Home/h.md"))), "{stays}");
    assert!(new.join("Home/h.md").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- undo --------------------------------------------------------------------------------------------

#[test]
fn undo_puts_every_file_back_and_every_line_as_it_said_and_the_folders_it_removed() {
    let w = world("undo");
    let plan = group_plan(&w);
    let report = do_write(&plan.request(), &mut gate(), &mut flaky());
    let path = report.record_path.expect("record");
    let record = runs::load(&path, &w.library).expect("record");
    let question = prepare(&record, &w.library);
    assert!(question.acts() && question.rows.len() == 2);
    assert!(
        question.sentence().contains("2 files back"),
        "{}",
        question.sentence()
    );
    assert!(
        question.sentence().contains("folders again")
            || question.sentence().contains("folder again")
    );
    let back = undo(&path, &record, &w.library, &mut gate(), &mut flaky());
    assert_eq!(back.start_error, None, "{back:?}");
    assert_eq!(text(&w.library.join("Work/Tokyo/a.md")), "# A\n");
    assert_eq!(text(&w.library.join("Work/b.md")), "# B\n");
    assert_eq!(
        text(&w.file),
        w.original,
        "every line is what it said, to the byte"
    );
    assert!(
        !w.work.join("Tokyo").exists() && !w.work.join("Food").exists(),
        "the run's folders went"
    );
    assert_eq!(
        runs::load(&path, &w.library).expect("record").state,
        State::Undone
    );
    // Twice is refused.
    let again = undo(
        &path,
        &runs::load(&path, &w.library).expect("record"),
        &w.library,
        &mut gate(),
        &mut flaky(),
    );
    assert!(
        again
            .start_error
            .is_some_and(|e| e.contains("undone already"))
    );
}

#[test]
fn undo_of_a_library_move_puts_the_library_back_exactly() {
    let (dir, old, new) = old_library("libundo");
    let before: Vec<(PathBuf, String)> = [
        "Work.md",
        "Home.md",
        "Work/a.md",
        "Home/h.md",
        "Work/Tokyo/t.md",
    ]
    .iter()
    .map(|n| (old.join(n), text(&old.join(n))))
    .collect();
    let plan = plan_library(&old, &new);
    let report = do_write(&plan.request(), &mut gate(), &mut flaky());
    let path = report.record_path.expect("record");
    let record = runs::load(&path, &new).expect("record");
    let back = undo(&path, &record, &new, &mut gate(), &mut flaky());
    assert_eq!(back.start_error, None, "{back:?}");
    assert!(
        back.failed.is_empty() && back.rolled_back.is_none(),
        "{back:?}"
    );
    for (file, was) in &before {
        assert_eq!(&text(file), was, "{}", file.display());
    }
    assert!(
        old.join(runs::RUNS_DIR)
            .join("2026-01-01T00-00-00Z-split.json")
            .is_file()
    );
    assert!(!new.join("Work.md").exists() && !new.join("Work").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_keeps_a_note_changed_since_with_its_line_and_puts_the_rest_back() {
    let w = world("undochanged");
    let report = do_write(&group_plan(&w).request(), &mut gate(), &mut flaky());
    let path = report.record_path.expect("record");
    put(
        &w.work.join("Tokyo/a.md"),
        "# A, which I have written more in\n",
    );
    let record = runs::load(&path, &w.library).expect("record");
    let question = prepare(&record, &w.library);
    assert_eq!(question.rows.len(), 1);
    assert!(question.kept[0].1.contains("changed since"));
    let back = undo(&path, &record, &w.library, &mut gate(), &mut flaky());
    assert_eq!(
        text(&w.work.join("Tokyo/a.md")),
        "# A, which I have written more in\n"
    );
    assert_eq!(text(&w.library.join("Work/b.md")), "# B\n");
    let lib = text(&w.file);
    assert!(
        lib.contains(&slashes(&w.work.join("Tokyo/a.md"))),
        "its line still leads to it: {lib}"
    );
    assert!(lib.contains("(Work/b.md)"), "{lib}");
    assert!(back.kept.iter().any(|(p, _)| p.ends_with("a.md")));
}

#[test]
fn undo_keeps_a_note_whose_line_was_edited_or_that_another_line_lists() {
    let w = world("undolines");
    let report = do_write(&group_plan(&w).request(), &mut gate(), &mut flaky());
    let path = report.record_path.expect("record");
    let record = runs::load(&path, &w.library).expect("record");
    // The owner retitled one line, and another library file lists the other note.
    let edited = text(&w.file).replace("- [A](", "- [Asakusa](");
    put(&w.file, &edited);
    put(
        &w.library.join("Money.md"),
        &format!(
            "# Money\n\n- [B again]({})\n",
            slashes(&w.work.join("Food/b.md"))
        ),
    );
    let question = prepare(&record, &w.library);
    assert!(question.rows.is_empty(), "{question:?}");
    let why: Vec<&str> = question.kept.iter().map(|(_, w)| w.as_str()).collect();
    assert!(why.contains(&"its line was edited since"), "{why:?}");
    assert!(
        why.contains(&"another line in the library lists it"),
        "{why:?}"
    );
    let back = undo(&path, &record, &w.library, &mut gate(), &mut flaky());
    assert!(w.work.join("Tokyo/a.md").is_file() && w.work.join("Food/b.md").is_file());
    assert!(back.moved.is_empty());
}

#[test]
fn undo_keeps_a_note_when_something_is_at_its_old_place_or_its_folder_is_gone() {
    let w = world("undoplace");
    let report = do_write(&group_plan(&w).request(), &mut gate(), &mut flaky());
    let path = report.record_path.expect("record");
    let record = runs::load(&path, &w.library).expect("record");
    put(&w.library.join("Work/Tokyo/a.md"), "# somebody else's\n");
    let question = prepare(&record, &w.library);
    assert!(
        question
            .kept
            .iter()
            .any(|(_, w)| w == "there is already a file at its old place"
                || w.contains("old place already"))
    );
    let back = undo(&path, &record, &w.library, &mut gate(), &mut flaky());
    assert_eq!(
        text(&w.library.join("Work/Tokyo/a.md")),
        "# somebody else's\n",
        "never replaced"
    );
    assert!(w.work.join("Tokyo/a.md").is_file());
    assert!(back.kept.len() == 1);
    // A folder the run did not remove, that is gone, is not made again.
    let mut changed = record.clone();
    if let Some(m) = changed.moved.as_mut() {
        m.emptied.clear();
    }
    std::fs::remove_dir_all(w.library.join("Work")).expect("removed");
    let question = prepare(&changed, &w.library);
    assert!(
        question
            .kept
            .iter()
            .any(|(_, w)| w.contains("folder it came from is gone")),
        "{question:?}"
    );
}

#[test]
fn a_record_with_a_path_it_may_not_name_refuses_the_whole_undo() {
    let w = world("hostile");
    let report = do_write(&group_plan(&w).request(), &mut gate(), &mut flaky());
    let path = report.record_path.expect("record");
    let good = runs::load(&path, &w.library).expect("record");
    type Break = fn(&World, &mut Record);
    let cases: Vec<(&str, Break)> = vec![
        ("a destination outside the library", |w, r| {
            r.moved.as_mut().expect("m").files[0].to = w.dir.join("other/a.md");
        }),
        ("a destination with a ..", |w, r| {
            r.moved.as_mut().expect("m").files[0].to = w.work.join("../x/a.md");
        }),
        ("a source outside, not said to be", |w, r| {
            r.moved.as_mut().expect("m").files[0].from = w.dir.join("Documents/a.md");
        }),
        ("a source with a ..", |w, r| {
            r.moved.as_mut().expect("m").files[0].from = w.library.join("Work/../../a.md");
        }),
        ("a file that would change its name", |w, r| {
            r.moved.as_mut().expect("m").files[0].from = w.library.join("Work/Tokyo/z.md");
        }),
        ("a note that is not Markdown", |w, r| {
            let f = &mut r.moved.as_mut().expect("m").files[0];
            f.to = w.work.join("Tokyo/a.exe");
            f.from = w.library.join("Work/Tokyo/a.exe");
        }),
        ("a line in a file that is not a library file", |w, r| {
            r.moved.as_mut().expect("m").changes[0].file = w.dir.join("notes.md");
        }),
        ("a folder that holds no file it puts back", |w, r| {
            r.moved
                .as_mut()
                .expect("m")
                .emptied
                .push(w.dir.join("Documents"));
        }),
        ("a group file that is not a library file", |w, r| {
            r.group.file = w.dir.join("Work.md");
        }),
        ("a library folder that is not absolute", |_, _| {}),
    ];
    for (name, break_it) in cases {
        let mut record = good.clone();
        break_it(&w, &mut record);
        let library = if name.contains("not absolute") {
            PathBuf::from("library")
        } else {
            w.library.clone()
        };
        let plan = prepare(&record, &library);
        assert!(plan.refusal.is_some(), "{name}: it was not refused");
        assert!(plan.rows.is_empty(), "{name}");
        let back = undo(&path, &record, &library, &mut gate(), &mut flaky());
        assert!(back.start_error.is_some(), "{name}");
        assert!(
            w.work.join("Tokyo/a.md").is_file(),
            "{name}: a file was moved"
        );
        assert!(!w.library.join("Work/Tokyo/a.md").exists(), "{name}");
    }
}

#[test]
fn a_library_move_record_that_says_the_library_left_a_folder_that_holds_the_library_is_refused() {
    let (dir, old, new) = old_library("libhostile");
    let report = do_write(
        &plan_library(&old, &new).request(),
        &mut gate(),
        &mut flaky(),
    );
    let path = report.record_path.expect("record");
    let good = runs::load(&path, &new).expect("record");
    for from in [dir.clone(), new.clone(), new.join("inside")] {
        let mut record = good.clone();
        record.moved.as_mut().expect("m").library_from = Some(from);
        assert!(prepare(&record, &new).refusal.is_some());
    }
    let mut record = good.clone();
    record.moved.as_mut().expect("m").library_from = None;
    assert!(
        prepare(&record, &new).refusal.is_some(),
        "a library move names its old folder"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_split_undo_never_acts_on_a_move_and_a_move_is_listed_like_any_run() {
    let w = world("kinds");
    let report = do_write(&group_plan(&w).request(), &mut gate(), &mut flaky());
    let path = report.record_path.expect("record");
    let record = runs::load(&path, &w.library).expect("record");
    let plan = runs::prepare(&record, &w.library);
    assert!(plan.refusal.is_some_and(|r| r.contains("own command")));
    assert!(
        runs::finish(&path, &record, &w.library, &mut NoGate)
            .error
            .is_some()
    );
    let (list, unread) = runs::list_all(&w.library);
    assert_eq!(list.len(), 1);
    assert!(unread.is_empty());
    assert_eq!(list[0].record.kind, Kind::Move);
    assert!(headline(&list[0].record).starts_with("Move \u{00B7} the notes of Work"));
}

struct NoGate;

impl runs::Gate for NoGate {
    fn add_links(&mut self, _: &Path, _: &[runs::Link]) -> Result<(), String> {
        Err("a move has no links to add".to_string())
    }

    fn remove_links(
        &mut self,
        _: &Path,
        _: &[runs::LinkRemoval],
        _: &[Vec<String>],
    ) -> Result<(), String> {
        Err("a move has no links to remove".to_string())
    }
}

#[test]
fn a_kind_this_version_does_not_know_is_listed_and_never_touched_or_deleted() {
    let w = world("newer");
    let dir = w.library.join(runs::RUNS_DIR);
    let newer = dir.join("2020-01-01T00-00-00Z-shuffle.json");
    put(
        &newer,
        "{\"version\": 1, \"kind\": \"shuffle\", \"started\": \"2020-01-01T00:00:00Z\", \"state\": \"done\"}",
    );
    let (list, unread) = runs::list_all(&w.library);
    assert!(list.is_empty());
    assert_eq!(unread.len(), 1);
    assert!(unread[0].newer, "written by a newer version");
    runs::prune(&w.library, 4_000_000_000);
    assert!(newer.is_file(), "never deleted");
}

// ---- the record's file format ---------------------------------------------------------------------

fn sample_record(library: &Path) -> Record {
    let q = library.join("Work/\"q\".md");
    Record {
        version: 1,
        kind: Kind::Move,
        started: "2027-01-15T08:00:00Z".to_string(),
        started_secs: 1_800_000_000,
        state: State::Done,
        instruction: "the notes of \"Work\" into their folder".to_string(),
        sent: None,
        source: runs::Source {
            path: PathBuf::new(),
            len: 0,
            sha256: String::new(),
            recycled: false,
        },
        group: runs::Group {
            name: "Work".to_string(),
            file: library.join("Work.md"),
            created: false,
            home: library.join("Work"),
        },
        folders: vec![library.join("Work/New")],
        notes: Vec::new(),
        pictures: Vec::new(),
        sections_added: Vec::new(),
        moved: Some(Moved {
            start: Start::Group,
            library_from: None,
            files: vec![MovedFile {
                kind: FileKind::Note,
                from: q,
                to: library.join("Work/New/\"q\".md"),
                done: true,
                len: 12,
                sha256: "abc".to_string(),
                outside: true,
                sections: vec!["Tokyo".to_string(), "Food".to_string()],
            }],
            changes: vec![Change {
                file: library.join("Work.md"),
                edit: Edit {
                    kind: ChangeKind::Note,
                    sections: vec!["Tokyo".to_string()],
                    before: "- [Q](Work/\"q\".md)".to_string(),
                    after: "- [Q](<Work/New/\"q\".md>)".to_string(),
                },
                row: Some(0),
            }],
            emptied: vec![library.join("Work/Old")],
        }),
    }
}

#[test]
fn a_moves_record_reads_back_what_was_written_with_quotes_and_without_a_backslash_assumption() {
    let library = scratch("json").join("library");
    let record = sample_record(&library);
    let json = record.to_json(&library);
    assert!(json.contains("\"kind\": \"move\""), "{json}");
    let back = Record::from_json(&json, &library).expect("it reads");
    assert_eq!(back.moved, record.moved);
    assert_eq!(back.group, record.group);
    assert_eq!(back.folders, record.folders);
    assert_eq!(back.instruction, record.instruction);
    // Paths inside the library are stored relative, with `/`, wherever the disk is.
    assert!(json.contains("\"to\": \"Work/New/"), "{json}");
    // A backslash in a name is one character off Windows, and the record must keep it so.
    if !cfg!(windows) {
        let mut odd = record.clone();
        odd.moved.as_mut().expect("m").files[0].to = library.join("Work/a\\b.md");
        let again = Record::from_json(&odd.to_json(&library), &library).expect("reads");
        assert_eq!(
            again.moved.expect("m").files[0].to,
            library.join("Work/a\\b.md")
        );
    }
}

#[test]
fn a_moves_record_with_an_entry_it_cannot_read_is_not_read_at_all() {
    let library = scratch("jsonbad").join("library");
    let json = sample_record(&library).to_json(&library);
    for broken in [
        json.replace("\"kind\": \"note\"", "\"kind\": \"picture\""),
        json.replace("\"from\":", "\"origin\":"),
        json.replace("\"start\": \"group\"", "\"start\": \"sideways\""),
        json.replace("\"before\":", "\"previous\":"),
    ] {
        assert!(Record::from_json(&broken, &library).is_none());
    }
    // An index past the files ties nothing, rather than the wrong file.
    let tied = json.replace("\"row\": 0", "\"row\": 9");
    let back = Record::from_json(&tied, &library).expect("reads");
    assert_eq!(back.moved.expect("m").changes[0].row, None);
}

// ---- the lines ---------------------------------------------------------------------------------------

#[test]
fn edits_replace_exactly_the_lines_named_keep_the_line_ending_and_refuse_a_changed_file() {
    let base = scratch("edits");
    let text =
        "# G\r\n\r\n## S\r\n\r\n- [A](a.md)\r\n- [A](a.md)\r\n\r\n## T\r\n\r\n- [A](a.md)\r\n";
    let edit = |sections: &[&str], after: &str| Edit {
        kind: ChangeKind::Note,
        sections: sections.iter().map(|s| s.to_string()).collect(),
        before: "- [A](a.md)".to_string(),
        after: after.to_string(),
    };
    let new = with_edits(text, &[edit(&["T"], "- [A](b/a.md)")], &base).expect("made");
    assert_eq!(
        new,
        "# G\r\n\r\n## S\r\n\r\n- [A](a.md)\r\n- [A](a.md)\r\n\r\n## T\r\n\r\n- [A](b/a.md)\r\n"
    );
    let twice = with_edits(
        text,
        &[edit(&["S"], "- [A](x.md)"), edit(&["S"], "- [A](y.md)")],
        &base,
    )
    .expect("made");
    assert!(
        twice.contains("(x.md)\r\n- [A](y.md)"),
        "two identical lines are two edits"
    );
    assert!(
        with_edits(
            text,
            &[edit(&["S"], "1"), edit(&["S"], "2"), edit(&["S"], "3")],
            &base
        )
        .is_none()
    );
    assert!(
        with_edits(text, &[edit(&["Nowhere"], "x")], &base).is_none(),
        "wrong section"
    );
    assert!(
        with_edits("# G\n", &[edit(&[], "x")], &base).is_none(),
        "the line is not there"
    );
}

#[test]
fn a_gate_refuses_a_file_that_would_not_read_back_as_the_notes_it_lists() {
    let base = scratch("readback");
    let text = "# G\n\n- [A](a.md)\n- [B](b.md)\n";
    let edits = [Edit {
        kind: ChangeKind::Note,
        sections: Vec::new(),
        before: "- [A](a.md)".to_string(),
        after: "- [A](x/a.md)".to_string(),
    }];
    let want = |a: &str| Expected {
        entries: vec![
            Want {
                title: "A".to_string(),
                sections: Vec::new(),
                path: base.join(a),
            },
            Want {
                title: "B".to_string(),
                sections: Vec::new(),
                path: base.join("b.md"),
            },
        ],
        home: None,
        exact: None,
    };
    assert!(apply_edits(text, &edits, &want("x/a.md"), &base).is_ok());
    let why = apply_edits(text, &edits, &want("y/a.md"), &base).unwrap_err();
    assert!(why.contains("would not read back"), "{why}");
    let exact = Expected {
        exact: Some(text.to_string()),
        ..Expected::default()
    };
    assert!(
        apply_edits(text, &edits, &exact, &base).is_err(),
        "the text must be exactly what was there"
    );
}

#[test]
fn a_note_listed_twice_is_not_moved_because_a_move_could_not_keep_both_links() {
    let w = world("listed");
    let mut text = text(&w.file);
    text.push_str("\n## Again\n\n- [A again](Work/Tokyo/a.md)\n");
    put(&w.file, &text);
    let plan = group_plan(&w);
    let a = plan
        .rows
        .iter()
        .find(|r| r.from.ends_with("a.md"))
        .expect("a");
    assert_eq!(a.verdict, Verdict::Listed(2));
    assert!(!a.ticked);
    assert!(a.verdict.text().contains("2 times"));
}

#[test]
fn the_machine_never_overwrites_with_a_portable_rename_either() {
    let dir = scratch("portable");
    put(&dir.join("a.md"), "one");
    put(&dir.join("b.md"), "two");
    let mut disk = Disk::portable();
    let err = disk
        .rename_new(&dir.join("a.md"), &dir.join("b.md"))
        .unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(text(&dir.join("b.md")), "two");
    assert_eq!(text(&dir.join("a.md")), "one");
}
