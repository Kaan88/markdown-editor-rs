use super::*;
use crate::notes_ai::{Compose, Part, Place, compose_at};

fn draft(title: &str, sections: &[&str], pictures: &[usize]) -> Draft {
    let mut body = vec![Part::Text("Some words.".to_string())];
    body.extend(pictures.iter().map(|&id| Part::Picture {
        id,
        alt: String::new(),
    }));
    Draft {
        title: title.to_string(),
        sections: sections.iter().map(|s| s.to_string()).collect(),
        body,
    }
}

fn home() -> PathBuf {
    PathBuf::from(if cfg!(windows) {
        r"C:\lib\Shots"
    } else {
        "/lib/Shots"
    })
}

/// Pictures `0..n`, `shot0.png`… of 1,000 bytes each, under a folder of their own.
fn shots(id: usize) -> Option<Original> {
    Some(Original {
        path: home()
            .parent()
            .unwrap_or(Path::new(""))
            .join("desk")
            .join(format!("shot{id}.png")),
        size: 1000,
        pasted: false,
    })
}

fn free(_: &Path, _: &Path) -> Taken {
    Taken::Free
}

fn naming<'a>(looks: &'a [Look], names: &'a HashMap<usize, String>) -> Naming<'a> {
    Naming {
        copy: true,
        rename: false,
        looks,
        names,
    }
}

fn names_of(p: &Planned) -> Vec<String> {
    p.copies.iter().map(Copy::name).collect()
}

/// Section 9: a picture is copied once per folder, into `pictures/` beside the notes that show
/// it: two notes of one section share the copy, a note of another section has its own.
#[test]
fn a_picture_is_copied_once_into_each_folder_that_shows_it() {
    let names = HashMap::new();
    let notes = vec![
        draft("Login", &["Sign-in"], &[0]),
        draft("Logout", &["Sign-in"], &[0]),
        draft("Billing", &["Pay"], &[0]),
    ];
    let p = plan(&notes, &home(), &shots, &naming(&[], &names), &free).expect("plan");
    assert_eq!(p.copies.len(), 2, "{:?}", p.copies);
    assert_eq!(
        p.copies[0].to,
        home().join("Sign-in").join("pictures").join("shot0.png")
    );
    assert_eq!(
        p.copies[1].to,
        home().join("Pay").join("pictures").join("shot0.png")
    );
    assert_eq!(p.notes[0].path, home().join("Sign-in").join("login.md"));
    assert!(p.copy_of(&home().join("Pay"), 0).is_some());
}

/// Section 9: a name that is taken by another file becomes `-2`; one holding the same bytes is
/// used, and not written again.
#[test]
fn a_taken_name_is_numbered_and_the_same_bytes_are_used() {
    let names = HashMap::new();
    let notes = vec![draft("A", &[], &[0, 1])];
    let taken = |to: &Path, from: &Path| match (
        to.file_name().and_then(|n| n.to_str()),
        from.file_name().and_then(|n| n.to_str()),
    ) {
        (Some("shot0.png"), _) => Taken::Other,
        (Some("shot1.png"), Some("shot1.png")) => Taken::Same,
        _ => Taken::Free,
    };
    let p = plan(&notes, &home(), &shots, &naming(&[], &names), &taken).expect("plan");
    assert_eq!(names_of(&p), vec!["shot0-2.png", "shot1.png"]);
    assert!(!p.copies[0].there);
    assert!(p.copies[1].there, "the same bytes are used");
    assert_eq!(p.written().count(), 1);
}

/// Two pictures whose copies would have one name in one folder get two.
#[test]
fn two_copies_in_one_folder_never_share_a_name() {
    let names = HashMap::new();
    let looks = vec![
        Look {
            id: 0,
            title: "Error".to_string(),
            ..Look::default()
        },
        Look {
            id: 1,
            title: "error".to_string(),
            ..Look::default()
        },
    ];
    let mut n = naming(&looks, &names);
    n.rename = true;
    let p = plan(&[draft("A", &[], &[0, 1])], &home(), &shots, &n, &free).expect("plan");
    assert_eq!(names_of(&p), vec!["error.png", "error-2.png"]);
}

/// Check 10: a picture no kept note shows is not copied; with "Copy the pictures beside the
/// notes" off nothing is.
#[test]
fn only_the_pictures_a_note_shows_are_copied_and_none_with_copy_off() {
    let names = HashMap::new();
    let notes = vec![draft("A", &[], &[1])];
    let p = plan(&notes, &home(), &shots, &naming(&[], &names), &free).expect("plan");
    assert_eq!(names_of(&p), vec!["shot1.png"]);
    let mut off = naming(&[], &names);
    off.copy = false;
    let p = plan(&notes, &home(), &shots, &off, &free).expect("plan");
    assert!(p.copies.is_empty());
    assert_eq!(p.notes.len(), 1);
}

/// A pasted picture lives in a temp folder that goes with the window: it is copied beside its
/// note even with "Copy the pictures beside the notes" off.
#[test]
fn a_pasted_picture_is_copied_even_with_copy_off() {
    let names = HashMap::new();
    let mut off = naming(&[], &names);
    off.copy = false;
    let pasted = |id: usize| {
        shots(id).map(|o| Original {
            pasted: id == 1,
            ..o
        })
    };
    let p = plan(&[draft("A", &[], &[0, 1])], &home(), &pasted, &off, &free).expect("plan");
    assert_eq!(names_of(&p), vec!["shot1.png"]);
}

/// Section 9 and the plan card's renames: the owner's name wins, the look's title is used when
/// "Name the copies after what they show" is on, and the original's otherwise; the extension is
/// always the original's, and the name rule keeps a path out of it.
#[test]
fn a_copy_is_named_by_the_owner_the_look_or_the_original() {
    let looks = vec![Look {
        id: 0,
        title: "Login error".to_string(),
        ..Look::default()
    }];
    let none = HashMap::new();
    let mut n = naming(&looks, &none);
    let shot = Path::new("D:/desk/IMG 0001.PNG");
    assert_eq!(copy_name(0, shot, &n), "IMG 0001.png");
    n.rename = true;
    assert_eq!(copy_name(0, shot, &n), "login-error.png");
    let named = |given: &str| {
        let mut map = HashMap::new();
        map.insert(0, given.to_string());
        copy_name(0, shot, &naming(&looks, &map))
    };
    let name = named("../Sign in.png");
    assert!(!name.contains('/') && !name.contains('\\'), "{name}");
    assert!(name.ends_with("Sign in.png"), "{name}");
    assert_eq!(named("front page.jpg"), "front page.jpg.png");
    assert_eq!(named("front page"), "front page.png");
    assert_eq!(named("Front.PNG"), "Front.png");
}

/// Section 11: 100 notes, 100 copies, 200 MB of copies, 64 MiB a copy; past each the run is
/// refused with both numbers, before anything is written.
#[test]
fn the_limits_of_a_run_are_refused_with_both_numbers() {
    let names = HashMap::new();
    let many: Vec<Draft> = (0..101)
        .map(|i| draft(&format!("N{i}"), &[], &[]))
        .collect();
    let why = plan(&many, &home(), &shots, &naming(&[], &names), &free).expect_err("101");
    assert!(why.contains("101") && why.contains("100"), "{why}");
    let ids: Vec<usize> = (0..101).collect();
    let why = plan(
        &[draft("A", &[], &ids)],
        &home(),
        &shots,
        &naming(&[], &names),
        &free,
    )
    .expect_err("101 copies");
    assert!(why.contains("101 copies"), "{why}");
    let big = |id: usize| {
        shots(id).map(|o| Original {
            size: 60 * 1000 * 1000,
            ..o
        })
    };
    let why = plan(
        &[draft("A", &[], &[0, 1, 2, 3])],
        &home(),
        &big,
        &naming(&[], &names),
        &free,
    )
    .expect_err("240 MB");
    assert!(why.contains("240 MB") && why.contains("200 MB"), "{why}");
    let huge = |id: usize| {
        shots(id).map(|o| Original {
            size: COPY_FILE_MAX + 1,
            ..o
        })
    };
    let why = plan(
        &[draft("A", &[], &[0])],
        &home(),
        &huge,
        &naming(&[], &names),
        &free,
    )
    .expect_err("too large");
    assert!(why.contains("a copy may be"), "{why}");
}

/// A copy undo could not remove is never made: only the picture kinds undo knows.
#[test]
fn a_file_undo_would_not_remove_is_not_copied() {
    let names = HashMap::new();
    let heic = |_: usize| {
        Some(Original {
            path: PathBuf::from("D:/desk/photo.heic"),
            size: 10,
            pasted: false,
        })
    };
    let why = plan(
        &[draft("A", &[], &[0])],
        &home(),
        &heic,
        &naming(&[], &names),
        &free,
    )
    .expect_err("heic");
    assert!(why.contains("photo.heic"), "{why}");
}

/// Two notes at one path are refused, not written over each other.
#[test]
fn two_notes_at_one_path_are_refused() {
    let names = HashMap::new();
    let why = plan(
        &[draft("Login", &[], &[]), draft("login", &[], &[])],
        &home(),
        &shots,
        &naming(&[], &names),
        &free,
    )
    .expect_err("one path");
    assert!(why.contains("login.md"), "{why}");
}

/// The folders a write needs, parents first, each once.
#[test]
fn the_folders_come_parents_first_and_once() {
    let names = HashMap::new();
    let notes = vec![draft("A", &["One"], &[0]), draft("B", &["One"], &[1])];
    let p = plan(&notes, &home(), &shots, &naming(&[], &names), &free).expect("plan");
    assert_eq!(
        p.folders(),
        vec![home().join("One"), home().join("One").join("pictures")]
    );
}

/// Section 15: a picture that could not be copied leaves a line saying so where it would have
/// been, its marks escaped; a picture linked where it is has its relative path escaped.
#[test]
fn a_missing_picture_says_so_and_a_linked_one_leads_where_it_is() {
    let note = draft("A", &[], &[0, 1]);
    let c = Compose {
        looks: &[],
        name: &|_| None,
        text_out: false,
    };
    let text = compose_at(&note, &c, &|id| {
        Some(match id {
            0 => Place::Missing("shot_[1].png could not be copied: disk full".to_string()),
            _ => Place::At("../../desk/a b.png".to_string()),
        })
    });
    assert!(
        text.contains("(shot\\_\\[1\\].png could not be copied: disk full)"),
        "{text}"
    );
    assert!(text.contains("![](../../desk/a%20b.png)"), "{text}");
}

/// A run of Write notes is the shared record's `notes` kind; a Markdown record is 5.8's note even
/// when it holds pictures, more than one note, or a picture as its source.
#[test]
fn a_notes_run_is_told_from_a_markdown_note_by_its_kind() {
    let mut r = runs::Record {
        version: runs::RECORD_VERSION,
        kind: runs::Kind::Markdown,
        started: String::new(),
        started_secs: 0,
        state: runs::State::Done,
        instruction: String::new(),
        sent: None,
        source: runs::Source {
            path: PathBuf::from("D:/desk/trip.txt"),
            len: 0,
            sha256: String::new(),
            recycled: false,
        },
        group: runs::Group {
            name: String::new(),
            file: PathBuf::new(),
            created: false,
            home: PathBuf::new(),
        },
        folders: Vec::new(),
        notes: vec![runs::NoteRec {
            path: PathBuf::from("a.md"),
            created: true,
            sha256: String::new(),
            ranges: Vec::new(),
            heading_shift: 0,
            title_added: false,
            sections: Vec::new(),
            line: String::new(),
        }],
        pictures: Vec::new(),
        sections_added: Vec::new(),
        moved: None,
        originals: Vec::new(),
        added: Vec::new(),
        copied: Vec::new(),
        removed: Vec::new(),
    };
    assert!(!is_notes_run(&r), "one note of a text file is 5.8's");
    r.source.path = PathBuf::from("D:/desk/shot.PNG");
    r.notes.push(r.notes[0].clone());
    assert!(!is_notes_run(&r), "a Markdown record is never a notes run");
    r.kind = runs::Kind::Notes;
    assert!(is_notes_run(&r));
    r.kind = runs::Kind::Split;
    assert!(!is_notes_run(&r));
}
