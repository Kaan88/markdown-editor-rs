//! Write notes' plan card (pictures plan, phase V4): it opens when the plan is ready, and each
//! edit on it is the run's `arranged`, which the write reads: ticks, renames, "Put in…", "Join
//! with…", Esc keeping the plan, Discard, and "Arrange again…" sending no picture.

use super::ai_tests::{engine, wait};
use super::files_tests::{app, scratch};
use super::*;
use crate::keys::{Key, KeyName};
use crate::notes_plan_ui::{Act, Hit, Row, Sel};
use crate::notes_task::Run;
use crate::task_ui::TaskKind;
use mdedit_core::ai::Connection;
use mdedit_core::notes_ai::{self, Arranged, HowMany, Look, Part};

/// The model's answer: the oven and the tiles in Kitchen, the sink and the tap in Bathroom,
/// picture 1 in no note.
fn answer() -> String {
    serde_json::json!({"version": 1, "notes": [
        {"title": "Oven", "sections": ["Kitchen"],
         "body": "Clean it.\n\n![The oven](picture:0)\n\nBuy a scraper."},
        {"title": "Tiles", "sections": ["Kitchen"],
         "body": "Two are cracked.\n\n![Tiles](picture:4)"},
        {"title": "Tap", "sections": ["Bathroom"],
         "body": "## The sink\n\n![Sink](picture:2)\n\n![Tap](picture:3)"}
    ], "unused": [1]})
    .to_string()
}

/// A Write notes window on five pictures with the plan of [`answer`] ready and its card open;
/// nothing is sent (the AI is off).
fn planned(tag: &str) -> App {
    let dir = scratch(tag);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let files: Vec<PathBuf> = ["oven", "list", "sink", "tap", "tiles"]
        .iter()
        .map(|n| {
            let p = dir.join(format!("{n}.png"));
            std::fs::write(&p, b"not read").expect("picture");
            p
        })
        .collect();
    a.task_open(TaskKind::Notes, files);
    let ids: Vec<usize> = (0..5).collect();
    let looks: Vec<Look> = [
        "Kitchen oven",
        "Shop list",
        "Sink",
        "Mixer tap",
        "Blue tiles",
    ]
    .iter()
    .enumerate()
    .map(|(id, t)| Look {
        id,
        title: (*t).to_string(),
        says: format!("{t}, seen close."),
        ..Look::default()
    })
    .collect();
    let s = a.task.as_mut().expect("open");
    s.dest = crate::task_ui::DestChoice::New("House".to_string());
    s.notes_run = Some(Run {
        batches: vec![vec![0, 1, 2, 3], vec![4]],
        answered: vec![0, 1],
        next: 2,
        looks,
        arranged: notes_ai::read_arrange(&answer(), &ids).ok(),
        ..Run::default()
    });
    a.notes_plan_show();
    assert!(a.notes_plan_open());
    a
}

fn plan(a: &App) -> Arranged {
    a.task
        .as_ref()
        .and_then(|s| s.notes_run.as_ref())
        .and_then(|r| r.arranged.clone())
        .expect("a plan")
}

fn key_of(a: &App, title: &str) -> u64 {
    a.notes_plan
        .as_ref()
        .and_then(|c| c.entries.iter().find(|e| e.draft.title == title))
        .map(|e| e.key)
        .expect("that note")
}

fn titles(p: &Arranged) -> Vec<&str> {
    p.notes.iter().map(|n| n.title.as_str()).collect()
}

fn press_key(a: &mut App, k: KeyName, typed: Option<&str>) {
    a.notes_plan_key(Key::Name(k), typed);
}

/// Types `text` into the field that has the keyboard, over what it holds.
fn type_over(a: &mut App, text: &str) {
    a.mods = crate::keys::Mods {
        ctrl: true,
        ..Default::default()
    };
    press_key(a, KeyName::Char('a'), None);
    a.mods = Default::default();
    for c in text.chars() {
        press_key(a, KeyName::Char(c), Some(&c.to_string()));
    }
}

/// Pictures plan step 5: the plan made (here with the AI off, one note per picture) opens its
/// card over the window, on the first note.
#[test]
fn the_card_opens_when_the_plan_is_ready() {
    let dir = scratch("notes-plan-opens");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let p = dir.join("sink.png");
    std::fs::write(&p, b"x").expect("picture");
    a.task_open(TaskKind::Notes, vec![p]);
    if let Some(s) = a.task.as_mut() {
        s.notes.how_many = HowMany::PerPicture;
    }
    a.task_run();
    assert!(a.notes_plan_open(), "the card is over the window");
    let c = a.notes_plan.as_ref().expect("card");
    assert_eq!(c.shown_note(), c.entries.first().map(|e| e.key));
}

/// The tree: the group, each section with its notes, each note's pictures under it (a heading
/// over those that follow one), and the picture no note shows under its own line, unticked.
#[test]
fn the_tree_shows_sections_notes_pictures_and_the_unused() {
    let a = planned("notes-plan-tree");
    let c = a.notes_plan.as_ref().expect("card");
    let (oven, tiles, tap) = (key_of(&a, "Oven"), key_of(&a, "Tiles"), key_of(&a, "Tap"));
    let kitchen = vec!["Kitchen".to_string()];
    let bathroom = vec!["Bathroom".to_string()];
    let pic = |id, key: Option<u64>, on| Row::Picture {
        id,
        key,
        on,
        depth: if key.is_some() { 2 } else { 0 },
    };
    assert_eq!(
        c.rows(),
        vec![
            Row::Group,
            Row::Section {
                path: kitchen.clone()
            },
            Row::Note {
                key: oven,
                on: true,
                depth: 1
            },
            pic(0, Some(oven), true),
            Row::Note {
                key: tiles,
                on: true,
                depth: 1
            },
            pic(4, Some(tiles), true),
            Row::Section {
                path: bathroom.clone()
            },
            Row::Note {
                key: tap,
                on: true,
                depth: 1
            },
            Row::Heading {
                text: "The sink".to_string(),
                depth: 2
            },
            pic(2, Some(tap), true),
            pic(3, Some(tap), true),
            Row::Unused,
            pic(1, None, false),
        ]
    );
}

/// Unticking a note takes it out of `arranged.notes` and lists its pictures unused; ticking it
/// again puts it back where it was, pictures and all.
#[test]
fn a_note_unticked_leaves_the_plan_and_comes_back_ticked() {
    let mut a = planned("notes-plan-note-tick");
    let tiles = Sel::Note(key_of(&a, "Tiles"));
    a.notes_plan_select(tiles.clone());
    press_key(&mut a, KeyName::Space, None);
    let p = plan(&a);
    assert_eq!(titles(&p), vec!["Oven", "Tap"]);
    assert_eq!(p.unused, vec![1, 4], "its picture is no longer shown");
    assert!(
        a.notes_plan
            .as_ref()
            .expect("card")
            .rows()
            .contains(&Row::Note {
                key: key_of(&a, "Tiles"),
                on: false,
                depth: 1
            })
    );
    press_key(&mut a, KeyName::Space, None);
    let p = plan(&a);
    assert_eq!(titles(&p), vec!["Oven", "Tiles", "Tap"]);
    assert_eq!(p.unused, vec![1]);
}

/// A picture unticked leaves its note's body and is listed unused; ticked again it is back at
/// its own place in the body, with its words.
#[test]
fn a_picture_unticked_is_unused_and_comes_back_in_its_place() {
    let mut a = planned("notes-plan-picture-tick");
    let before = plan(&a).notes[2].body.clone();
    a.notes_plan_select(Sel::Picture(2));
    press_key(&mut a, KeyName::Delete, None);
    let p = plan(&a);
    assert_eq!(p.notes[2].pictures(), vec![3]);
    assert_eq!(p.unused, vec![1, 2]);
    press_key(&mut a, KeyName::Space, None);
    assert_eq!(plan(&a).notes[2].body, before);
    assert_eq!(plan(&a).unused, vec![1]);
}

/// "Put in…" from the row menu moves the picture's part from one note's body to the end of
/// another's; one in no note is put in the same way.
#[test]
fn put_in_moves_a_picture_to_another_note() {
    let mut a = planned("notes-plan-put-in");
    let oven = key_of(&a, "Oven");
    a.notes_plan_menu(Sel::Picture(3), 10.0, 10.0);
    a.notes_plan_choose(Act::PutIn);
    let then = a.notes_plan.as_ref().and_then(|c| c.menu.as_ref());
    assert!(
        then.is_some_and(|m| m.menu.items.iter().all(|i| i.label != "Tap")),
        "the list is of the other notes"
    );
    a.notes_plan_choose(Act::Into(oven));
    let p = plan(&a);
    assert_eq!(p.notes[0].pictures(), vec![0, 3]);
    assert_eq!(p.notes[2].pictures(), vec![2]);
    assert_eq!(
        p.notes[0].body.last(),
        Some(&Part::Picture {
            id: 3,
            alt: "Tap".to_string()
        })
    );
    a.notes_plan_menu(Sel::Picture(1), 10.0, 10.0);
    a.notes_plan_choose(Act::PutIn);
    a.notes_plan_choose(Act::Into(oven));
    let p = plan(&a);
    assert_eq!(p.notes[0].pictures(), vec![0, 3, 1]);
    assert!(p.unused.is_empty());
}

/// "Join with…": the other note goes at the end of this one under its title as a heading, and
/// is no longer a note of its own.
#[test]
fn join_with_puts_the_other_note_under_this_one() {
    let mut a = planned("notes-plan-join");
    let (oven, tiles) = (key_of(&a, "Oven"), key_of(&a, "Tiles"));
    a.notes_plan_menu(Sel::Note(oven), 10.0, 10.0);
    a.notes_plan_choose(Act::JoinWith);
    a.notes_plan_choose(Act::Into(tiles));
    let p = plan(&a);
    assert_eq!(titles(&p), vec!["Oven", "Tap"]);
    assert_eq!(p.notes[0].pictures(), vec![0, 4]);
    assert!(
        p.notes[0]
            .body
            .iter()
            .any(|part| matches!(part, Part::Text(t) if t.contains("## Tiles")))
    );
}

/// F2 on a note, then Enter: its title is the draft's, and so its file name.
#[test]
fn f2_renames_a_note() {
    let mut a = planned("notes-plan-rename-note");
    a.notes_plan_select(Sel::Note(key_of(&a, "Tap")));
    press_key(&mut a, KeyName::F(2), None);
    type_over(&mut a, "A new tap");
    press_key(&mut a, KeyName::Enter, None);
    let p = plan(&a);
    assert_eq!(p.notes[2].title, "A new tap");
    assert_eq!(p.notes[2].file_name(), "a-new-tap.md");
}

/// A rename that would make two notes one file is refused, and the field stays to be put right.
#[test]
fn a_rename_onto_another_notes_file_is_refused() {
    let mut a = planned("notes-plan-rename-clash");
    a.notes_plan_select(Sel::Note(key_of(&a, "Tiles")));
    press_key(&mut a, KeyName::F(2), None);
    type_over(&mut a, "oven");
    press_key(&mut a, KeyName::Enter, None);
    assert_eq!(titles(&plan(&a)), vec!["Oven", "Tiles", "Tap"]);
    assert!(a.notes_plan.as_ref().is_some_and(|c| c.edit.is_some()));
}

/// F2 on a section renames it in every note under it: their `sections` are the folder.
#[test]
fn f2_renames_a_section_in_every_note_under_it() {
    let mut a = planned("notes-plan-rename-section");
    a.notes_plan_select(Sel::Section(vec!["Kitchen".to_string()]));
    press_key(&mut a, KeyName::F(2), None);
    type_over(&mut a, "Cooking");
    press_key(&mut a, KeyName::Enter, None);
    let p = plan(&a);
    assert_eq!(p.notes[0].sections, vec!["Cooking".to_string()]);
    assert_eq!(p.notes[1].sections, vec!["Cooking".to_string()]);
    assert_eq!(p.notes[2].sections, vec!["Bathroom".to_string()]);
}

/// F2 on a picture names its copy: the name typed with the original's extension, in
/// `run.copy_names`, and the note's Markdown links it so.
#[test]
fn f2_names_a_pictures_copy() {
    let mut a = planned("notes-plan-rename-picture");
    a.notes_plan_select(Sel::Picture(0));
    press_key(&mut a, KeyName::F(2), None);
    let shown = a
        .notes_plan
        .as_ref()
        .and_then(|c| c.edit.as_ref())
        .map(|(_, f)| f.text().to_string());
    assert_eq!(
        shown.as_deref(),
        Some("kitchen-oven"),
        "named after what it shows"
    );
    type_over(&mut a, "Oven door");
    press_key(&mut a, KeyName::Enter, None);
    let s = a.task.as_ref().expect("open");
    let run = s.notes_run.as_ref().expect("run");
    assert_eq!(
        run.copy_names.get(&0).map(String::as_str),
        Some("Oven door.png")
    );
    let md = crate::notes_plan_ui::markdown(s, run, &plan(&a).notes[0]);
    assert!(md.contains("(pictures/Oven%20door.png)"), "{md}");
}

/// Esc keeps the plan and brings the window back (D-053): its button says so and opens the
/// card again, with what was left out still there to tick.
#[test]
fn escape_keeps_the_plan_and_the_button_opens_it_again() {
    let mut a = planned("notes-plan-escape");
    a.notes_plan_select(Sel::Note(key_of(&a, "Oven")));
    press_key(&mut a, KeyName::Space, None);
    press_key(&mut a, KeyName::Escape, None);
    assert!(!a.notes_plan_open());
    assert!(a.task_is_open());
    assert_eq!(titles(&plan(&a)), vec!["Tiles", "Tap"], "the plan is kept");
    assert_eq!(
        a.task.as_ref().expect("open").primary_label(),
        "Review the plan"
    );
    a.task_run();
    assert!(a.notes_plan_open());
    press_key(&mut a, KeyName::Space, None);
    assert_eq!(titles(&plan(&a)), vec!["Oven", "Tiles", "Tap"]);
}

/// Discard throws the plan away; what was looked at is kept.
#[test]
fn discard_throws_the_plan_away() {
    let mut a = planned("notes-plan-discard");
    a.notes_plan_act(Hit::Discard);
    assert!(a.notes_plan.is_none());
    let run = a
        .task
        .as_ref()
        .and_then(|s| s.notes_run.as_ref())
        .expect("run");
    assert!(run.arranged.is_none());
    assert_eq!(run.looks.len(), 5);
}

/// A press on a row's tick, where the card drew it, unticks that row.
#[test]
fn a_click_on_a_tick_unticks_its_row() {
    let mut a = planned("notes-plan-click");
    let mut bmp = mdshared_text::Bitmap::new(1280, 800);
    let mut gfx = mdshared_ui::gfx::Gfx::new();
    let pal = mdshared_ui::theme::LIGHT;
    {
        let mut p = mdshared_ui::gfx::Painter::new(&mut bmp, &mut gfx, 1.0);
        let (Some(c), Some(s)) = (a.notes_plan.as_mut(), a.task.as_ref()) else {
            panic!("open")
        };
        crate::notes_plan_draw::draw(&mut p, &pal, c, s, 1280.0, 800.0, false);
    }
    let c = a.notes_plan.as_ref().expect("card");
    let row = c
        .laid
        .iter()
        .position(|r| matches!(r, Row::Picture { id: 4, .. }))
        .expect("the tiles' picture");
    let at = c
        .hits
        .iter()
        .find(|(h, _)| *h == Hit::Tick(row))
        .map(|(_, r)| *r)
        .expect("its tick");
    a.ui.mouse = (at.cx(), at.cy());
    a.notes_plan_press();
    assert_eq!(plan(&a).unused, vec![1, 4]);
}

/// "Arrange again…" asks the arrange request again with the new instruction and sends no
/// picture: one request more, no look; the card opens on its answer.
#[test]
fn arrange_again_asks_again_without_the_pictures() {
    engine();
    let dir = scratch("notes-plan-again");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    std::fs::create_dir_all(dir.join("library")).expect("library");
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    let files: Vec<PathBuf> = (0..2)
        .map(|i| {
            let p = dir.join(format!("room{i}.png"));
            std::fs::write(&p, b"not read").expect("picture");
            p
        })
        .collect();
    a.task_open(TaskKind::Notes, files);
    wait(&mut a, |a| {
        a.task
            .as_ref()
            .is_some_and(|s| s.prepare.is_empty() && s.notes_blocked().is_none())
    });
    a.task_run();
    wait(&mut a, |a| a.notes_plan_open());
    let requests = a.task_sent.requests;
    let looked = a
        .task
        .as_ref()
        .and_then(|s| s.notes_run.as_ref())
        .map(|r| r.answered.clone());
    a.notes_plan_act(Hit::Again);
    type_over(&mut a, "By room");
    press_key(&mut a, KeyName::Enter, None);
    assert!(!a.notes_plan_open(), "the window shows the request out");
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.instruction.text(), "By room");
    assert!(
        s.notes_run
            .as_ref()
            .is_some_and(|r| r.arrange.is_some() && r.in_flight.is_empty() && r.arranged.is_none())
    );
    wait(&mut a, |a| a.notes_plan_open());
    assert_eq!(a.task_sent.requests, requests + 1, "the arrange alone");
    assert_eq!(
        a.task
            .as_ref()
            .and_then(|s| s.notes_run.as_ref())
            .map(|r| r.answered.clone()),
        looked,
        "no picture is looked at again"
    );
}
