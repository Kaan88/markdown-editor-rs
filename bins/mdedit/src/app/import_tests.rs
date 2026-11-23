//! "Add a folder to the library…" through the window (phase 4a): the command opens the card with
//! the folder's files under their subfolders, Add writes one line per ticked file in the group's
//! library file and nothing else on disk changes, a group the library has is joined rather than
//! made again, a file it already lists is left alone, an unsaved library file refuses the write,
//! and the demo writes nothing.

use super::files_tests::{open_now, scratch};
use super::library_write_tests::app_in;
use super::*;
use crate::ui::PaneFace;

/// A folder of recipes and a library folder beside it. The library has no group of that name.
struct Lib {
    dir: PathBuf,
    library: PathBuf,
    folder: PathBuf,
    lasagne: PathBuf,
    notes: PathBuf,
    sourdough: PathBuf,
}

fn lib(tag: &str) -> (App, Lib) {
    let dir = scratch(tag);
    let library = dir.join("library");
    std::fs::create_dir_all(&library).unwrap();
    // The folder is inside the library, as the owner's own `library` folder holds it: its links
    // are then written relative to the library and a library that moves keeps working.
    let folder = library.join("Recipes");
    std::fs::create_dir_all(folder.join("Pasta")).unwrap();
    std::fs::create_dir_all(folder.join("Bread")).unwrap();
    let lasagne = folder.join("Pasta").join("lasagne.md");
    let notes = folder.join("notes.txt");
    let sourdough = folder.join("Bread").join("sourdough.md");
    std::fs::write(&lasagne, "# Lasagne\n\nLayers, and a long bake.\n").unwrap();
    std::fs::write(&notes, "Things to try: a clay pot, a fish kettle\n").unwrap();
    std::fs::write(&sourdough, "# Sourdough\n\nA starter on the counter.\n").unwrap();
    // A file the walk does not take.
    std::fs::write(folder.join("photo.jpg"), "not text\n").unwrap();
    let a = app_in(&dir);
    (
        a,
        Lib {
            dir,
            library,
            folder,
            lasagne,
            notes,
            sourdough,
        },
    )
}

/// Waits for the walk's thread, as the event loop's wake would deliver it.
fn settle(a: &mut App) {
    let until = Instant::now() + Duration::from_secs(10);
    while a.importer.is_none() {
        assert!(Instant::now() < until, "the walk did not finish");
        a.import_take();
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn toast(a: &App) -> String {
    a.docs
        .toast(Instant::now())
        .map(str::to_string)
        .unwrap_or_default()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn open_card(a: &mut App, l: &Lib) {
    a.import_folder_picked(l.folder.clone());
    settle(a);
}

/// The title of every row of the open card, in order.
fn rows(a: &App) -> Vec<String> {
    let card = a.importer.as_ref().expect("the card is open");
    (0..card.plan.rows.len())
        .map(|i| crate::import_ui::row_title(&card.plan, i))
        .collect()
}

#[test]
fn the_command_opens_the_card_with_the_folders_files_under_their_subfolders() {
    let (mut a, l) = lib("import-open");
    open_card(&mut a, &l);
    let card = a.importer.as_ref().expect("the card is open");
    assert_eq!(card.plan.group_name, "Recipes");
    assert!(card.plan.group_new, "the library has no Recipes");
    assert_eq!(card.plan.folder, l.folder);
    assert_eq!(card.plan.group_file, l.library.join("Recipes.md"));
    // The walk takes the three text files and not the picture: the folder's own notes first, then
    // each subfolder's, by name.
    assert_eq!(
        rows(&a),
        vec![
            "notes".to_string(),
            "Bread".to_string(),
            "Sourdough".to_string(),
            "Pasta".to_string(),
            "Lasagne".to_string(),
        ]
    );
    // Every file is ticked: none is in the library.
    assert_eq!(card.plan.ticked(), 3);
    assert_eq!(card.problem(), None);

    // Nothing was written by opening it.
    assert!(!l.library.join("Recipes.md").exists());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn add_writes_one_line_per_ticked_file_and_changes_nothing_else_on_disk() {
    let (mut a, l) = lib("import-add");
    let before: Vec<(PathBuf, Vec<u8>)> = [&l.lasagne, &l.notes, &l.sourdough]
        .into_iter()
        .map(|p| (p.clone(), std::fs::read(p).unwrap()))
        .collect();
    let listing_before: Vec<String> = std::fs::read_dir(&l.folder)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    open_card(&mut a, &l);
    a.import_write();
    assert!(a.importer.is_none(), "the card closes when the write is done");
    let group = l.library.join("Recipes.md");
    let text = read(&group);
    assert!(text.starts_with("# Recipes\nhome: Recipes\n"), "{text}");
    assert!(
        text.contains("- [Lasagne](Recipes/Pasta/lasagne.md)"),
        "{text}"
    );
    assert!(
        text.contains("- [Sourdough](Recipes/Bread/sourdough.md)"),
        "{text}"
    );
    assert!(text.contains("- [notes](Recipes/notes.txt)"), "{text}");
    // The library reads it back as three notes, each under its folder.
    let read_back = mdshared_app::library::read(&l.library);
    let group = read_back
        .groups
        .iter()
        .find(|g| g.name == "Recipes")
        .expect("the group");
    assert_eq!(group.entries.len(), 3);
    let lasagne = group
        .entries
        .iter()
        .find(|e| e.title == "Lasagne")
        .expect("Lasagne");
    assert_eq!(lasagne.sections, vec!["Pasta".to_string()]);
    assert!(mdshared_app::paths::same_path(&lasagne.path, &l.lasagne));
    // Nothing of the owner's changed: the files are the files, and no file was added to the folder.
    for (path, bytes) in before {
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "{path:?} changed");
    }
    let listing_after: Vec<String> = std::fs::read_dir(&l.folder)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(listing_before.len(), listing_after.len());
    // The pane is on the group, and the status says how many went in.
    assert!(a.ui.panes.files && a.ui.library.on, "the Notes pane is open");
    assert_eq!(a.ui.pane_face(), PaneFace::Notes);
    assert!(toast(&a).contains("3 files are"), "{}", toast(&a));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_group_the_library_has_is_joined_and_a_file_it_lists_is_not_written_again() {
    let (mut a, l) = lib("import-join");
    // The library's group is the folder's, and lists the lasagne already.
    let group = l.library.join("Recipes.md");
    std::fs::write(
        &group,
        format!(
            "# Recipes\nhome: {}\n\n## Pasta\n\n- [Lasagne](Recipes/Pasta/lasagne.md)\n",
            l.folder.file_name().unwrap().to_string_lossy()
        ),
    )
    .unwrap();
    open_card(&mut a, &l);
    let card = a.importer.as_ref().expect("the card is open");
    assert!(!card.plan.group_new, "the library has the group");
    assert_eq!(card.plan.entries, 1);
    assert_eq!(card.plan.already(), 1);
    assert_eq!(card.plan.ticked(), 2, "the two files it does not list");
    // Add leaves the group's own line alone and adds the two.
    a.import_write();
    let text = read(&group);
    assert_eq!(text.matches("[Lasagne]").count(), 1, "{text}");
    assert!(text.contains("[notes](Recipes/notes.txt)"), "{text}");
    assert!(
        text.contains("[Sourdough](Recipes/Bread/sourdough.md)"),
        "{text}"
    );
    let read_back = mdshared_app::library::read(&l.library);
    let group = read_back
        .groups
        .iter()
        .find(|g| g.name == "Recipes")
        .expect("the group");
    assert_eq!(group.entries.len(), 3);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_folder_outside_the_library_gets_a_group_whose_home_is_that_folder() {
    let dir = scratch("import-outside");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).unwrap();
    let folder = dir.join("Dishes");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("pie.md"), "# Pie\n").unwrap();
    let mut a = app_in(&dir);
    a.import_folder_picked(folder.clone());
    settle(&mut a);
    let card = a.importer.as_ref().expect("the card is open");
    assert_eq!(card.plan.group_name, "Dishes");
    a.import_write();
    let text = read(&library.join("Dishes.md"));
    // The folder is outside the library, so its `home:` is its whole path and the link too.
    let written = folder.to_string_lossy().replace('\\', "/");
    assert!(text.contains(&format!("home: {written}")), "{text}");
    assert!(
        text.contains(&format!("- [Pie](<{written}/pie.md>)"))
            || text.contains(&format!("- [Pie]({written}/pie.md)")),
        "{text}"
    );
    let read_back = mdshared_app::library::read(&library);
    let group = read_back.groups.first().expect("the group");
    assert_eq!(group.home.as_deref(), Some(folder.as_path()));
    assert!(mdshared_app::paths::same_path(
        &group.entries[0].path,
        &folder.join("pie.md")
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn add_leaves_a_library_file_open_with_unsaved_changes_alone_and_writes_nothing() {
    let (mut a, l) = lib("import-unsaved");
    let group = l.library.join("Recipes.md");
    std::fs::write(&group, "# Recipes\n\n## Pasta\n\n- [Old](Pasta/old.md)\n").unwrap();
    let before = read(&group);
    // The group's file is opened and typed into first, so its text on disk is stale when the card
    // is opened over it.
    open_now(&mut a, &group);
    super::files_tests::type_text(&mut a, "X");
    assert!(
        a.docs.active_doc().is_some_and(|d| d.modified()),
        "the tab has unsaved changes"
    );
    open_card(&mut a, &l);
    a.import_write();
    assert!(a.importer.is_some(), "the card stays open with the reason");
    assert_eq!(read(&group), before, "the file on disk was not written");
    let card = a.importer.as_ref().unwrap();
    let why = card.refusal.clone().unwrap_or_default();
    assert!(why.contains("Save or close"), "{why}");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_group_would_pass_two_thousand_notes_is_refused_with_both_numbers() {
    let (mut a, l) = lib("import-cap");
    // A group of the same name with 2,000 notes in it already.
    let group = l.library.join("Recipes.md");
    let mut text = String::from("# Recipes\n");
    for n in 0..2000 {
        text.push_str(&format!("- [Note {n}](note-{n}.md)\n"));
    }
    std::fs::write(&group, text).unwrap();
    open_card(&mut a, &l);
    let card = a.importer.as_ref().expect("the card is open");
    let why = card.problem().unwrap_or_default();
    assert!(why.contains("2000"), "{why}");
    assert!(a.importer.as_ref().unwrap().plan.blocked.is_none());
    let before = read(&group);
    a.import_write();
    assert!(a.importer.is_some());
    assert_eq!(read(&group), before, "nothing was added");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_folder_with_nothing_to_take_opens_a_card_that_says_so_and_writes_nothing() {
    let dir = scratch("import-empty");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).unwrap();
    let folder = dir.join("Pictures");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.png"), "png\n").unwrap();
    let mut a = app_in(&dir);
    a.import_folder_picked(folder.clone());
    settle(&mut a);
    let card = a.importer.as_ref().expect("the card opens with the reason");
    assert_eq!(
        card.problem().unwrap_or_default(),
        "Every file in it is in the library already."
    );
    assert!(card.plan.rows.is_empty());
    a.import_write();
    assert!(a.importer.is_some(), "the card stays open");
    assert_eq!(std::fs::read_dir(&library).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_library_folder_itself_is_refused() {
    let (mut a, l) = lib("import-self");
    a.import_folder_picked(l.library.clone());
    // Refused before the walk starts: no thread, nothing read.
    assert!(a.importer.is_none() && a.import_job.is_none());
    assert_eq!(toast(&a), crate::commands::strings::LIBRARY_HOME_IS_LIBRARY);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn escape_closes_the_card_and_writes_nothing() {
    let (mut a, l) = lib("import-esc");
    open_card(&mut a, &l);
    a.key_pressed(
        crate::keys::Key::Name(crate::keys::KeyName::Escape),
        None,
    );
    assert!(a.importer.is_none());
    assert!(!l.library.join("Recipes.md").exists());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_demo_draws_the_card_and_writes_nothing() {
    let dir = scratch("import-demo");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).unwrap();
    let mut a = app_in(&dir);
    assert!(a.import_demo("folder"));
    let card = a.importer.as_ref().expect("the card is open");
    assert!(card.demo);
    assert!(card.plan.notes.len() >= 10);
    a.compose();
    a.import_write();
    assert!(a.importer.is_some(), "the demo's card stays open");
    let why = a
        .importer
        .as_ref()
        .and_then(|c| c.refusal.clone())
        .unwrap_or_default();
    assert!(why.contains("writes nothing"), "{why}");
    assert_eq!(std::fs::read_dir(&library).unwrap().count(), 0);
    // The other names are shots, not failures.
    for name in crate::import_ui::NAMES {
        assert!(crate::import_ui::demo(name).is_some(), "{name}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_click_on_a_section_ticks_every_row_under_it_and_redraws_them() {
    let dir = scratch("import-pixels");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).unwrap();
    let mut a = app_in(&dir);
    a.import_demo("folder");
    a.compose();
    let (section, first_file) = {
        let card = a.importer.as_ref().expect("the card");
        let mut section = None;
        let mut file = None;
        for (i, row) in card.plan.rows.iter().enumerate() {
            if let mdedit_core::import::RowKind::Section { path } = &row.kind
                && path.len() == 1
                && section.is_none()
            {
                section = Some(i);
                file = card.plan.note_of(i + 1);
            }
        }
        (section.expect("a top section"), file.expect("a file under it"))
    };
    let g = a.importer.as_ref().unwrap().geo();
    let row = a.importer.as_ref().unwrap().rows().rect(g.list, section);
    let before = a.screen.pixels.clone();
    a.ui.mouse = (row.x + 20.0, row.cy());
    a.plan_press();
    a.plan_release();
    a.compose();
    assert!(
        !a.importer.as_ref().unwrap().plan.notes[first_file].ticked,
        "the note under the heading is unticked"
    );
    // The frame was drawn again, and only where the card is: an overlay repaints its own area,
    // not the editor under it.
    let w = a.screen.width as usize;
    let s = a.scale.max(1.0);
    let mut changed = 0usize;
    for i in 0..a.screen.pixels.len() / 4 {
        if before[i * 4..i * 4 + 4] != a.screen.pixels[i * 4..i * 4 + 4] {
            let (x, y) = (((i % w) as f32) / s, ((i / w) as f32) / s);
            assert!(
                g.card.inset(-2.0).contains(x, y),
                "a pixel changed outside the card at ({x}, {y})"
            );
            changed += 1;
        }
    }
    assert!(changed > 0, "the card was not drawn again");
    let _ = std::fs::remove_dir_all(&dir);
}
