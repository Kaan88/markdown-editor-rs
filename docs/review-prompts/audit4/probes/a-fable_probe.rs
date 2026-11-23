//! Review probes (not for commit). Each passes on 1d503d8 and asserts the fault at `FAULT`.
include!("fable_base.inc");

struct RefusingBin;
impl runs::Bin for RefusingBin {
    fn recycle(&mut self, _path: &Path) -> Result<(), String> {
        Err("the file is in use".to_string())
    }
}

/// P1: a note the bin refuses stays on disk, its line is gone, the group's file is gone, and the
/// record says "undone": nothing lists the note and undo cannot be asked again.
#[test]
fn probe_a_bin_refusal_orphans_the_note_and_the_run_is_marked_undone() {
    let (library, _req, path, record) = one_note_run("probe-bin");
    let mut gate = FileGate::new();
    let done = runs::undo_with(&path, &record, &library, &mut gate, &mut RefusingBin);
    assert_eq!(done.bin_failures.len(), 1);
    assert!(library.join("Trip/One.md").is_file(), "the note is kept");
    // FAULT: no library file lists it any more.
    assert!(!library.join("Trip.md").exists(), "the group's file went");
    let after = runs::load(&path, &library).expect("the record");
    // FAULT: and the run cannot be undone again.
    assert_eq!(after.state, State::Undone);
    let again = runs::prepare(&after, &library);
    assert_eq!(again.refusal.as_deref(), Some("This run is undone already."));
}

/// P2: Finish adds headings that the record never learns of, so undo leaves them and the group's
/// file with them.
#[test]
fn probe_finish_then_undo_leaves_the_headings_and_the_groups_file() {
    let dir = scratch("probe-finish");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let req = request(
        &library,
        "Trip",
        true,
        vec![note(
            &library,
            "Trip/Tokyo/One.md",
            "One",
            "# One\n",
            &["Tokyo"],
        )],
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
    let record = runs::load(&path, &library).expect("the record");
    // FAULT: the record does not know the heading Finish wrote.
    assert!(record.sections_added.is_empty());
    let done = runs::undo(&path, &record, &library, &mut gate);
    assert_eq!(done.removed_notes.len(), 1);
    // FAULT: the heading and the group's file stay.
    let text = read(&library.join("Trip.md"));
    assert!(text.contains("## Tokyo"), "{text}");
    assert!(done.group_kept);
}

/// P3 (audit 3, C6): a heading the run wrote goes although the owner wrote under it.
#[test]
fn probe_undo_takes_a_heading_the_owner_wrote_under() {
    let text = "# Trip\nhome: Trip\n\n## Tokyo\n\n### My own plans\n\n- [One](Trip/Tokyo/One.md)\n";
    let base = Path::new("C:/lib");
    let links = [LinkRemoval {
        path: PathBuf::from("Trip/Tokyo/One.md"),
        sections: vec!["Tokyo".to_string()],
        line: "- [One](Trip/Tokyo/One.md)".to_string(),
    }];
    let _ = links;
    let out = runs::with_headings_removed(
        "# Trip\nhome: Trip\n\n## Tokyo\n\n### My own plans\n\n",
        &[vec!["Tokyo".to_string()]],
        base,
    );
    let _ = text;
    // FAULT: `## Tokyo` goes and the owner's `### My own plans` is left under nothing.
    assert_eq!(
        out.as_deref(),
        Some("# Trip\nhome: Trip\n\n\n### My own plans\n\n")
    );
}

/// P5: "Move the notes…" of one group offers, ticked, to carry another group's library file out
/// of the library folder, when the group lists that file as a note.
#[test]
fn probe_move_the_notes_offers_to_move_another_groups_library_file() {
    let dir = scratch("probe-move-libfile");
    let lib = dir.join("library");
    write_file(&lib.join("Index.md"), "# Index\nhome: Index\n\n- [Recipes](Recipes.md)\n");
    write_file(
        &lib.join("Recipes.md"),
        "# Recipes\nhome: Recipes\n\n- [Bread](Recipes/bread.md)\n",
    );
    write_file(&lib.join("Recipes/bread.md"), "# Bread\n");
    let read = library::read(&lib);
    let group = read
        .groups
        .iter()
        .find(|g| g.name == "Index")
        .expect("the group")
        .clone();
    let plan = mdedit_core::moves::plan_group(&lib, &group);
    assert_eq!(plan.blocked, None);
    // FAULT: the row is there, ready and ticked: Recipes.md would go to library/Index/Recipes.md,
    // and the group Recipes would leave the pane with every note it lists.
    let row = plan
        .rows
        .iter()
        .find(|r| r.from == lib.join("Recipes.md"))
        .expect("a row for the library file");
    assert!(row.ticked, "{:?}", row.verdict);
    assert_eq!(row.to, lib.join("Index").join("Recipes.md"));
}
