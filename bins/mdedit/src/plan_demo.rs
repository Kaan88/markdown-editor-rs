//! `--plan-demo NAME`: a plan screen for a shot, made from a document built by
//! `mdshared_ui::plan::demo`, with no file on disk read or written (the library is a table of what "is there"). The names are those of
//! the shots in `docs/ai-library-plan.md` 5.3, without the `plan-` in front.

use crate::plan_ui::demo::{japan, log, many, recipes};
use crate::plan_ui::{Dest, Found, Init, PlanScreen, Reader, fingerprint, headings_of};
use mdedit_core::buffer::Buffer;
use mdedit_core::split::{Heading, document};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Every name `--plan-demo` takes.
#[cfg(test)]
pub const NAMES: [&str; 8] = [
    "split",
    "split-gaps",
    "split-rest",
    "split-warn",
    "split-again",
    "split-rename",
    "split-many",
    "split-small",
];

fn init<'a>(
    doc: &Buffer,
    name: &'a str,
    headings: &'a [Heading],
    level: Option<u8>,
    dest: Dest,
    root: &Path,
    reader: Reader,
) -> Init<'a> {
    Init {
        doc: document(doc),
        name,
        path: None,
        headings,
        level,
        encoding: None,
        groups: vec![
            Dest::new(root, "Japan 2025", false),
            Dest::new(root, "Recipes", false),
            Dest::new(root, "Work notes", false),
        ],
        dest,
        root: root.to_path_buf(),
        reader,
    }
}

/// A reader that finds nothing: the library is empty.
fn empty() -> Reader {
    Box::new(|_| None)
}

/// A screen for `kind` (one of [`NAMES`]), or `None` for another name. The screen is as it would
/// be when it opened: nothing edited.
pub fn screen(kind: &str) -> Option<PlanScreen> {
    let root = PathBuf::from("library");
    let dest = || Dest::new(&root, "Japan 2025", true);
    let mut s = match kind {
        "split" => {
            let doc = Buffer::from_text(&recipes());
            let h = headings_of(&doc);
            let dest = Dest::new(&root, "Recipes", false);
            let mut s = PlanScreen::new(init(&doc, "Recipes.md", &h, None, dest, &root, empty()));
            s.select(1);
            s
        }
        "split-gaps" | "split-small" | "split-rename" => {
            let doc = Buffer::from_text(&japan());
            let h = headings_of(&doc);
            let mut s = PlanScreen::new(init(
                &doc,
                "Japan trip journal.md",
                &h,
                Some(2),
                dest(),
                &root,
                empty(),
            ));
            s.select(match kind {
                "split-small" => 1,
                "split-rename" => 4,
                _ => 5,
            });
            if kind == "split-rename" {
                s.start_rename();
                if let Some(e) = s.edit.as_mut() {
                    e.field.set_text("Shibuya at night, in the rain");
                }
            }
            s
        }
        "split-rest" => {
            let doc = Buffer::from_text(&japan());
            let h = headings_of(&doc);
            let mut s = PlanScreen::new(init(
                &doc,
                "Japan trip journal.md",
                &h,
                Some(2),
                dest(),
                &root,
                empty(),
            ));
            let rest = s.plan.notes().len();
            s.toggle(rest);
            s.select(rest);
            s
        }
        "split-warn" => {
            let doc = Buffer::from_text(&log());
            let h = headings_of(&doc);
            let mut s = PlanScreen::new(init(
                &doc,
                "Project log.md",
                &h,
                Some(2),
                Dest::new(&root, "Work notes", false),
                &root,
                empty(),
            ));
            // A note asked for inside the code block starts after it.
            s.new_note(40);
            s.select(1);
            s
        }
        "split-again" => {
            let doc = Buffer::from_text(&japan());
            let h = headings_of(&doc);
            let first = PlanScreen::new(init(
                &doc,
                "Japan trip journal.md",
                &h,
                Some(2),
                Dest::new(&root, "Japan 2025", false),
                &root,
                empty(),
            ));
            // What the first split wrote, except one note that is gone and one that was edited.
            let mut table: HashMap<PathBuf, Found> = HashMap::new();
            for (i, t) in first.targets.iter().enumerate() {
                let text = first.plan.note_text(i);
                let hash = fingerprint(text.as_bytes());
                match i {
                    1 => {}
                    2 => {
                        table.insert(
                            t.path.clone(),
                            Found {
                                len: text.len() as u64 + 11,
                                hash: hash ^ 1,
                            },
                        );
                    }
                    _ => {
                        table.insert(
                            t.path.clone(),
                            Found {
                                len: text.len() as u64,
                                hash,
                            },
                        );
                    }
                }
            }
            let reader: Reader = Box::new(move |p| table.get(p).copied());
            let mut s = PlanScreen::new(init(
                &doc,
                "Japan trip journal.md",
                &h,
                Some(2),
                Dest::new(&root, "Japan 2025", false),
                &root,
                reader,
            ));
            s.select(2);
            s
        }
        "split-many" => {
            let doc = Buffer::from_text(&many());
            let h = headings_of(&doc);
            let mut s = PlanScreen::new(init(
                &doc,
                "Notes archive.md",
                &h,
                None,
                Dest::new(&root, "Work notes", false),
                &root,
                empty(),
            ));
            for i in (0..s.plan.notes().len()).filter(|i| i % 11 == 4) {
                s.toggle(i);
            }
            s.select(37);
            s
        }
        _ => return None,
    };
    s.edited = false;
    Some(s)
}
