//! Sort a folder by AI (5.7, phase 4) through the window, with the stand-in engine of
//! `ai_tests`: the offer, the scheme, the batches, the answers on the plan, a failed batch, the
//! parallel default of D-080, Stop, the AI off, and the write of what was sorted.

use super::ai_tests::{engine, wait};
use super::files_tests::scratch;
use super::library_write_tests::app_in;
use super::*;
use crate::import_ui::SortStage;
use crate::keys::{Key, KeyName};
use mdedit_core::ai::Connection;
use mdedit_core::import_ai;

struct Lib {
    dir: PathBuf,
    library: PathBuf,
    folder: PathBuf,
}

/// A folder of `files` (its own, or under a subfolder) and a library folder beside it. Each file
/// is `# <stem>` and a paragraph long enough that the request sends only its start.
fn lib(tag: &str, files: &[&str]) -> (App, Lib) {
    engine();
    let dir = scratch(tag);
    let library = dir.join("library");
    std::fs::create_dir_all(&library).unwrap();
    let folder = dir.join("Recipes");
    std::fs::create_dir_all(&folder).unwrap();
    for (i, rel) in files.iter().enumerate() {
        let path = folder.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        std::fs::write(
            &path,
            format!("# {stem}\n\n{}{}", "the body of the note. ".repeat(80), i),
        )
        .unwrap();
    }
    let mut a = app_in(&dir);
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    (
        a,
        Lib {
            dir,
            library,
            folder,
        },
    )
}

/// Opens the card on the folder, waiting for the walk's thread as the event loop would.
fn open_card(a: &mut App, l: &Lib) {
    a.import_folder_picked(l.folder.clone());
    let until = Instant::now() + Duration::from_secs(10);
    while a.importer.is_none() {
        assert!(Instant::now() < until, "the walk did not finish");
        a.import_take();
        std::thread::sleep(Duration::from_millis(2));
    }
    a.import_ai_refresh();
}

/// "Ask the AI…", then the scheme comes.
fn ask(a: &mut App) {
    a.import_sort_ask();
    wait(a, |a| {
        a.importer
            .as_ref()
            .is_some_and(|c| matches!(c.sort, SortStage::Scheme(_) | SortStage::Failed(_)))
    });
}

/// The scheme, then "Sort the files", then the answers.
fn sort(a: &mut App) {
    ask(a);
    let c = a.importer.as_ref().expect("the card");
    assert!(
        matches!(c.sort, SortStage::Scheme(_)),
        "the scheme came: {:?}",
        c.sort
    );
    a.import_sort_start();
    wait(a, |a| {
        a.importer
            .as_ref()
            .is_some_and(|c| matches!(c.sort, SortStage::Filed { .. } | SortStage::Failed(_)))
    });
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

#[test]
fn the_scheme_comes_first_and_nothing_is_filed_until_the_owner_sorts() {
    let (mut a, l) = lib(
        "import-ai-scheme",
        &["one.md", "Bread/two.md", "Sauces/three.md"],
    );
    open_card(&mut a, &l);
    let card = a.importer.as_ref().expect("the card");
    assert_eq!(card.problem(), None);
    assert!(card.ai_estimate.requests >= 2);
    ask(&mut a);
    let card = a.importer.as_ref().expect("the card");
    let SortStage::Scheme(scheme) = &card.sort else {
        panic!("the scheme: {:?}", card.sort);
    };
    assert_eq!(scheme.sections.len(), 3);
    assert_eq!(scheme.sections[0].subs, ["Italian"]);
    // The scheme is all that was asked: no filing request went out, and the plan is the folders'.
    assert!(a.ai.sort.is_none(), "the scheme request is done");
    assert_eq!(card.plan.ticked(), 3);
    assert!(card.plan.notes.iter().all(|n| n.confidence.is_none()));
    assert!(!l.library.join("Recipes.md").exists());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_answers_put_the_files_under_the_schemes_sections_and_add_writes_links_only() {
    let (mut a, l) = lib(
        "import-ai-sorted",
        &["one.md", "Bread/two.md", "Sauces/three.md"],
    );
    let before: Vec<(PathBuf, Vec<u8>)> = ["one.md", "Bread/two.md", "Sauces/three.md"]
        .iter()
        .map(|rel| {
            let p = l.folder.join(rel);
            (p.clone(), std::fs::read(p).unwrap())
        })
        .collect();
    open_card(&mut a, &l);
    sort(&mut a);
    let card = a.importer.as_ref().expect("the card");
    let SortStage::Filed {
        placed,
        unsorted,
        stopped,
        ..
    } = &card.sort
    else {
        panic!("filed: {:?}", card.sort);
    };
    assert_eq!((*placed, *unsorted), (3, 0));
    assert!(stopped.is_none());
    // The plan is in the scheme's order: Pasta, Pasta > Italian, Bread.
    let sections: Vec<Vec<String>> = card.plan.notes.iter().map(|n| n.sections.clone()).collect();
    assert_eq!(
        sections,
        vec![
            vec!["Pasta".to_string()],
            vec!["Pasta".to_string(), "Italian".to_string()],
            vec!["Bread".to_string()]
        ]
    );
    assert_eq!(card.plan.notes[0].title, "one");
    assert_eq!(card.plan.notes[0].confidence, Some(0.9));
    assert_eq!(card.plan.notes[0].why, "first of the folder");
    assert_eq!(card.plan.notes[1].confidence, Some(0.4));

    // Tick only the sure ones leaves the 0.4 file out.
    a.importer.as_mut().expect("card").tick_sure();
    assert_eq!(a.importer.as_ref().expect("card").plan.ticked(), 2);
    a.import_write();
    assert!(a.importer.is_none(), "the card closes on the write");
    let group = l.library.join("Recipes.md");
    let text = read(&group);
    assert!(
        text.contains("- [one](") && text.contains("one.md)"),
        "{text}"
    );
    assert!(
        text.contains("- [three](") && text.contains("three.md)"),
        "{text}"
    );
    assert!(
        !text.contains("[two]"),
        "the unsure one was not written: {text}"
    );
    let read_back = mdshared_app::library::read(&l.library);
    let group = read_back
        .groups
        .iter()
        .find(|g| g.name == "Recipes")
        .expect("the group");
    assert_eq!(group.entries.len(), 2);
    let one = group
        .entries
        .iter()
        .find(|e| e.title == "one")
        .expect("one");
    assert_eq!(one.sections, vec!["Pasta".to_string()]);
    let three = group
        .entries
        .iter()
        .find(|e| e.title == "three")
        .expect("three");
    assert_eq!(three.sections, vec!["Bread".to_string()]);
    for (path, bytes) in before {
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "{path:?} changed");
    }
    assert!(toast(&a).contains("2 files are"), "{}", toast(&a));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_section_number_the_scheme_does_not_have_lands_in_unsorted_unticked() {
    let (mut a, l) = lib(
        "import-ai-out",
        &["one.md", "Bread/two.md", "IMPORTOUTOFRANGE.md"],
    );
    open_card(&mut a, &l);
    sort(&mut a);
    let card = a.importer.as_ref().expect("the card");
    let SortStage::Filed { unsorted, .. } = &card.sort else {
        panic!("filed: {:?}", card.sort);
    };
    assert_eq!(*unsorted, 1);
    let unplaced = card
        .plan
        .notes
        .iter()
        .find(|n| n.unsorted)
        .expect("one in Unsorted");
    assert_eq!(unplaced.sections, [import_ai::UNSORTED.to_string()]);
    assert!(!unplaced.ticked);
    // Its row says so, and the card will not write it while it is unticked.
    let rows: Vec<String> = (0..card.plan.rows.len())
        .map(|i| crate::import_ui::row_title(&card.plan, i))
        .collect();
    assert!(rows.contains(&import_ai::UNSORTED.to_string()), "{rows:?}");
    assert_eq!(card.plan.ticked(), 2);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn an_unreadable_scheme_says_so_and_leaves_the_folders() {
    let (mut a, l) = lib("import-ai-junk", &["IMPORTJUNKSCHEME.md", "two.md"]);
    open_card(&mut a, &l);
    ask(&mut a);
    let card = a.importer.as_ref().expect("the card");
    let SortStage::Failed(why) = &card.sort else {
        panic!("failed: {:?}", card.sort);
    };
    assert!(why.contains("could not be read"), "{why}");
    assert_eq!(card.plan.ticked(), 2);
    assert!(card.plan.notes.iter().all(|n| n.confidence.is_none()));
    assert!(a.ai.sort.is_none());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_refused_request_is_said_and_the_ai_off_never_asks() {
    // The engine's own error, from the scheme request.
    let (mut a, l) = lib("import-ai-error", &["FAILPLEASE.md", "two.md"]);
    open_card(&mut a, &l);
    ask(&mut a);
    let why = match &a.importer.as_ref().expect("the card").sort {
        SortStage::Failed(why) => why.clone(),
        other => panic!("failed: {other:?}"),
    };
    assert!(why.contains("could not be asked"), "{why}");
    assert!(why.contains("401"), "{why}");
    let _ = std::fs::remove_dir_all(&l.dir);

    // The AI off: the offer is refused, and no request exists to make.
    let (mut a, l) = lib(
        "import-ai-off",
        &["one.md", "Bread/two.md", "Sauces/three.md"],
    );
    a.docs.prefs.ai_connection = Connection::Off;
    open_card(&mut a, &l);
    let card = a.importer.as_ref().expect("the card");
    assert!(!card.can_ask());
    assert!(card.ai_reason.as_deref().unwrap_or("").contains("off"));
    a.import_sort_ask();
    assert!(a.ai.sort.is_none(), "nothing was started");
    assert!(
        matches!(&a.importer.as_ref().expect("card").sort, SortStage::Failed(w) if w.contains("off"))
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_failed_batch_keeps_what_the_other_requests_sorted() {
    // Forty-five files: two batches, and the second holds a file whose title fails the answer.
    let names: Vec<String> = (0..44).map(|i| format!("n{i:02}.md")).collect();
    let mut files: Vec<&str> = names.iter().map(String::as_str).collect();
    files.push("zz-IMPORTBATCHFAIL.md");
    let (mut a, l) = lib("import-ai-batch-fail", &files);
    open_card(&mut a, &l);
    sort(&mut a);
    let card = a.importer.as_ref().expect("the card");
    let SortStage::Filed {
        placed,
        unsorted,
        stopped,
        ..
    } = &card.sort
    else {
        panic!("filed: {:?}", card.sort);
    };
    assert_eq!(*placed, 3, "the first batch's answer was used");
    assert_eq!(*unsorted, 0, "a failed request places nothing");
    assert!(stopped.is_some(), "the card says it stopped");
    // The files the failed batch held keep their folder and their tick.
    assert_eq!(
        card.plan
            .notes
            .iter()
            .filter(|n| n.sections.is_empty())
            .count(),
        42
    );
    assert_eq!(card.plan.notes.iter().filter(|n| n.ticked).count(), 45);
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// The filing run just started: its batches, the requests in flight, and the next batch to send.
fn started(a: &App) -> (usize, usize, usize) {
    a.ai.sort
        .as_ref()
        .map_or((0, 0, 0), |r| (r.batches.len(), r.in_flight.len(), r.next))
}

/// D-080: the requests go out up to the number the connection allows, and the batches past it
/// wait. Each case has more batches than its number, so it fails with the cap taken out (audit
/// 4, B9): 121 files are four batches.
#[test]
fn requests_go_out_three_at_once_and_one_for_a_local_model() {
    let names: Vec<String> = (0..121).map(|i| format!("n{i:03}.md")).collect();
    let files: Vec<&str> = names.iter().map(String::as_str).collect();
    let (mut a, l) = lib("import-ai-parallel", &files);
    open_card(&mut a, &l);
    ask(&mut a);
    a.import_sort_start();
    assert_eq!(
        started(&a),
        (4, 3, 3),
        "three by default (D-080), the fourth waits"
    );
    a.import_close();
    assert!(a.ai.sort.is_none(), "closing stops it");
    let _ = std::fs::remove_dir_all(&l.dir);

    // The owner's number: two at once.
    let (mut a, l) = lib("import-ai-parallel-two", &files);
    a.docs.prefs.set_ai_parallel(Connection::ClaudeApi, 2);
    open_card(&mut a, &l);
    ask(&mut a);
    a.import_sort_start();
    assert_eq!(started(&a), (4, 2, 2), "two, as set");
    a.import_close();
    let _ = std::fs::remove_dir_all(&l.dir);

    // A local model starts with one, and the second batch waits for it.
    let (mut a, l) = lib("import-ai-local", &files[..41]);
    a.docs.prefs.ai_connection = Connection::OpenAi;
    a.docs.prefs.ai_preset = "lmstudio".to_string();
    open_card(&mut a, &l);
    ask(&mut a);
    a.import_sort_start();
    assert_eq!(started(&a), (2, 1, 1), "a local model starts with one");
    a.import_close();
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// Audit 4, B n4: "Requests at once" is kept per connection, but a number set for a hosted
/// service does not follow the provider to a local model: Settings › AI's provider list puts it
/// at the local model's one, and the sort sends one at a time. Between two of a kind the owner's
/// number stays.
#[test]
fn a_number_set_for_a_hosted_service_does_not_follow_to_a_local_model() {
    use super::modal::AI_PROVIDER;
    use mdshared_ui::settings::Value;
    let names: Vec<String> = (0..81).map(|i| format!("n{i:02}.md")).collect();
    let files: Vec<&str> = names.iter().map(String::as_str).collect();
    let (mut a, l) = lib("import-ai-preset", &files);
    let at = |key: &str| {
        mdedit_core::ai::PRESETS
            .iter()
            .filter(|x| !x.base_url.is_empty())
            .position(|x| x.key == key)
            .expect("a preset")
    };
    let c = Connection::OpenAi;
    a.docs.prefs.ai_connection = c;
    a.docs.prefs.ai_preset = "openai".into();
    a.docs.prefs.set_ai_parallel(c, 5);
    a.ai_setting(AI_PROVIDER, Value::Choice(at("openrouter")));
    assert_eq!(
        a.docs.prefs.ai_parallel(c),
        5,
        "another hosted service keeps it"
    );
    a.ai_setting(AI_PROVIDER, Value::Choice(at("lmstudio")));
    assert_eq!(a.docs.prefs.ai_preset, "lmstudio");
    assert_eq!(
        a.docs.prefs.ai_parallel(c),
        1,
        "a local model starts with one"
    );
    open_card(&mut a, &l);
    ask(&mut a);
    a.import_sort_start();
    assert_eq!(started(&a), (3, 1, 1));
    a.import_close();
    a.docs.prefs.set_ai_parallel(c, 2);
    a.ai_setting(AI_PROVIDER, Value::Choice(at("ollama")));
    assert_eq!(
        a.docs.prefs.ai_parallel(c),
        2,
        "another local model keeps it"
    );
    a.ai_setting(AI_PROVIDER, Value::Choice(at("openai")));
    assert_eq!(
        a.docs.prefs.ai_parallel(c),
        3,
        "a hosted service starts with three"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// The card's plan as it stands: its notes (sections, titles, ticks, order) and its rows.
fn plan_of(
    a: &App,
) -> (
    Vec<mdedit_core::import::Note>,
    Vec<mdedit_core::import::Row>,
) {
    let card = a.importer.as_ref().expect("the card");
    (card.plan.notes.clone(), card.plan.rows.clone())
}

/// Audit 4, B8: after a sort, "Keep the folders" gives back the plan the folders made, with the
/// tick the owner gave before asking; a new request starts from the folders; and a request that
/// fails after a sort leaves the files under their folders, as the card says, so Add writes the
/// folders' headings.
#[test]
fn keep_the_folders_after_a_sort_gives_the_folders_plan_back() {
    let (mut a, l) = lib(
        "import-ai-keep",
        &[
            "one.md",
            "Bread/two.md",
            "Sauces/three.md",
            "Sauces/four.md",
        ],
    );
    open_card(&mut a, &l);
    // The owner unticks a file before asking.
    let four = {
        let card = a.importer.as_ref().expect("the card");
        (0..card.plan.rows.len())
            .find(|&i| crate::import_ui::row_title(&card.plan, i) == "four")
            .expect("four's row")
    };
    a.importer.as_mut().expect("the card").toggle(four);
    let folders = plan_of(&a);
    assert_eq!(folders.0.iter().filter(|n| n.ticked).count(), 3);
    sort(&mut a);
    assert!(matches!(
        a.importer.as_ref().expect("card").sort,
        SortStage::Filed { .. }
    ));
    assert_ne!(plan_of(&a), folders, "the sort is on the plan");
    // Ask again: the scheme comes, and the plan behind it is the folders' again.
    ask(&mut a);
    assert!(matches!(
        a.importer.as_ref().expect("card").sort,
        SortStage::Scheme(_)
    ));
    assert_eq!(plan_of(&a), folders, "a new sort starts from the folders");
    // Escape on the scheme: Keep the folders.
    a.key_pressed(Key::Name(KeyName::Escape), None);
    let card = a.importer.as_ref().expect("card");
    assert!(matches!(card.sort, SortStage::Off), "{:?}", card.sort);
    assert_eq!(plan_of(&a), folders, "Keep the folders");
    // Sorted again, then a request that cannot be made: the files are under their folders.
    sort(&mut a);
    assert_ne!(plan_of(&a), folders);
    a.docs.prefs.ai_connection = Connection::Off;
    a.import_sort_ask();
    assert!(matches!(
        a.importer.as_ref().expect("card").sort,
        SortStage::Failed(_)
    ));
    assert_eq!(
        plan_of(&a),
        folders,
        "the files would still be added under their folders"
    );
    a.import_write();
    assert!(a.importer.is_none(), "written");
    let text = read(&l.library.join("Recipes.md"));
    assert!(
        text.contains("## Bread") && text.contains("## Sauces"),
        "{text}"
    );
    assert!(!text.contains("Pasta"), "none of the AI's headings: {text}");
    assert!(!text.contains("[four]"), "the unticked file: {text}");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn escape_stops_the_requests_and_gives_back_the_edited_scheme() {
    let names: Vec<String> = (0..90).map(|i| format!("n{i:02}.md")).collect();
    let files: Vec<&str> = names.iter().map(String::as_str).collect();
    let (mut a, l) = lib("import-ai-stop", &files);
    open_card(&mut a, &l);
    ask(&mut a);
    a.import_sort_start();
    assert_eq!(a.ai.sort.as_ref().map_or(0, |r| r.in_flight.len()), 3);
    a.key_pressed(Key::Name(KeyName::Escape), None);
    assert!(a.ai.sort.is_none(), "every request was cancelled");
    assert!(a.docs.ai_status.is_none());
    assert!(matches!(
        a.importer.as_ref().expect("card").sort,
        SortStage::Scheme(_)
    ));
    assert!(!l.library.join("Recipes.md").exists());
    let _ = std::fs::remove_dir_all(&l.dir);
}
