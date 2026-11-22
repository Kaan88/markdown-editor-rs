//! `--plan-demo NAME`: a plan screen for a shot, made from a document built by
//! `mdshared_ui::plan::demo`, with no file on disk read or written (the library is a table of what "is there"). The names are those of
//! the shots in `docs/ai-library-plan.md` 5.3, without the `plan-` in front.

use crate::plan_ui::demo::{japan, log, many, recipes};
use crate::plan_ui::{
    Dest, Found, Init, PlanScreen, RECYCLE_NO_BIN, RECYCLE_ONE_DOCUMENT, Reader, RecycleBox,
    fingerprint, headings_of, recycle_unavailable,
};
use mdedit_core::buffer::Buffer;
use mdedit_core::split::{Heading, document};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Every name `--plan-demo` takes.
#[cfg(test)]
pub const NAMES: [&str; 21] = [
    "split",
    "split-gaps",
    "split-rest",
    "split-warn",
    "split-again",
    "split-rename",
    "split-many",
    "split-small",
    "split-ai",
    "split-ai-dropped",
    "split-ai-unreadable",
    "split-ai-long",
    "split-ai-asking",
    "split-ai-ask",
    "split-bin-offered",
    "split-bin",
    "split-bin-gaps",
    "split-bin-wanted",
    "split-bin-drive",
    "split-bin-several",
    "split-bin-system",
];

/// The report shots of a split that put its original in the Recycle Bin (5.4): the window draws
/// them in the results panel, with no card.
#[cfg(test)]
pub const REPORTS: [&str; 2] = ["split-bin-report", "split-bin-kept"];

/// The answer of the `split-ai-unreadable` shot: what a model says when it will not do as asked.
pub const BAD_ANSWER: &str = "I'm sorry, but I can't split this document without seeing all of it.";

/// What the split shots asked the AI.
pub const INSTRUCTION: &str = "Two chapters to a note, under the city they are in";

/// Saved prompts of the kind "split", for the panel's shot (a shot reads no prompts file).
pub fn split_prompts() -> Vec<mdedit_core::ai::SavedPrompt> {
    mdedit_core::ai::parse_prompts(concat!(
        "## One note per chapter\n\nkind: split\n\n",
        "One note per chapter, under the part it is in.\n\n",
        "## One note per day\n\nkind: split\n\n",
        "One note per day of a journal; a day starts at its date.\n\n",
        "## By topic\n\nkind: split\n\n",
        "Group the paragraphs by topic, one note per topic, in the order they first appear.\n",
    ))
}

/// A plain text of 31,000 lines and no heading: too long to send even as an outline.
fn long_plain() -> String {
    let mut s = String::with_capacity(31_000 * 40);
    for k in 0..15_500usize {
        s.push_str(&format!(
            "Reading {k}: the gauge said {} and nothing moved.\n\n",
            k % 97
        ));
    }
    s
}

/// The answer of the `split-ai` and `split-ai-dropped` shots, made from the document's own
/// headings as a model would: line numbers, titles, sections. In `split-ai` one note starts in
/// the middle of a paragraph and one overlaps the note before; in `split-ai-dropped` there are
/// 240 notes, some of them past the end.
pub fn answer_for(kind: &str, screen: &PlanScreen) -> String {
    let mut notes: Vec<String> = Vec::new();
    match kind {
        "split-ai" => {
            let heads = screen.headings();
            let parts: Vec<(usize, String)> = heads
                .iter()
                .filter(|h| h.level == 1)
                .map(|h| (h.line, heading_words(screen, h.line)))
                .collect();
            let chapters: Vec<usize> = heads
                .iter()
                .filter(|h| h.level == 2)
                .map(|h| h.line)
                .collect();
            let part_of = |line: usize| {
                parts
                    .iter()
                    .rev()
                    .find(|(l, _)| *l < line)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_default()
            };
            let mut prev_to = 0usize;
            for (k, pair) in chapters.chunks(2).enumerate() {
                let first = pair[0];
                let end = chapters
                    .get(k * 2 + 2)
                    .copied()
                    .unwrap_or(screen.plan.lines())
                    .min(
                        parts
                            .iter()
                            .map(|(l, _)| *l)
                            .find(|&l| l > first)
                            .unwrap_or(usize::MAX),
                    );
                // 1-based, inclusive.
                let mut from = first + 1;
                let to = end;
                if k == 2 {
                    // The second line of the chapter's first paragraph: inside it.
                    from = first + 4;
                }
                if k == 4 {
                    from = prev_to.saturating_sub(3);
                }
                prev_to = to;
                notes.push(format!(
                    r#"{{"from":{from},"to":{to},"title":"{} and the next","sections":["{}"]}}"#,
                    heading_words(screen, first),
                    part_of(first)
                ));
            }
        }
        _ => {
            for k in 0..240usize {
                let from = k * 6 + 1;
                notes.push(format!(
                    r#"{{"from":{from},"to":{},"title":"Part {k}","sections":["Archive"]}}"#,
                    from + 3
                ));
            }
        }
    }
    format!(
        "Here is the plan.\n```json\n{{\"version\":1,\"notes\":[{}]}}\n```",
        notes.join(",")
    )
}

/// The words of the heading on `line`.
fn heading_words(screen: &PlanScreen, line: usize) -> String {
    let mut s = String::new();
    screen.plan.document().line_text(line, &mut s);
    s.trim_start_matches('#').trim().replace('"', "'")
}

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
        "split-ai" | "split-ai-unreadable" | "split-ai-asking" | "split-ai-ask" => {
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
            s.ai_offer = true;
            s
        }
        "split-ai-dropped" => {
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
            s.ai_offer = true;
            s
        }
        "split-ai-long" => {
            let doc = Buffer::from_text(&long_plain());
            let h = headings_of(&doc);
            let mut s = PlanScreen::new(init(
                &doc,
                "Field log.txt",
                &h,
                None,
                Dest::new(&root, "Work notes", false),
                &root,
                empty(),
            ));
            s.ai_offer = true;
            s
        }
        // The Recycle Bin box (5.4), on the journal: offered and not ticked; ticked with every line in a note; disabled
        // for lines in no note, with the one-click answer; ticked and then blocked by the rest
        // unticked again (the gaps in the warning colour); and each reason the application gives.
        "split-bin-offered" | "split-bin" | "split-bin-gaps" | "split-bin-wanted"
        | "split-bin-drive" | "split-bin-several" | "split-bin-system" => {
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
            let refused = match kind {
                "split-bin-drive" => Some(RECYCLE_NO_BIN.to_string()),
                "split-bin-several" => Some(RECYCLE_ONE_DOCUMENT.to_string()),
                "split-bin-system" => Some(recycle_unavailable(
                    "a policy on this PC deletes files instead",
                )),
                _ => None,
            };
            s.recycle = Some(RecycleBox {
                wanted: false,
                refused,
            });
            if kind != "split-bin-gaps" {
                s.keep_the_rest();
            }
            if matches!(kind, "split-bin" | "split-bin-wanted") {
                s.toggle_recycle();
            }
            if kind == "split-bin-wanted" {
                let rest = s.plan.notes().len();
                s.toggle(rest);
            }
            s.select(1);
            s
        }
        _ => return None,
    };
    s.edited = false;
    Some(s)
}
