//! The AI task window with a fourth task, "Write notes", and pictures the model is sent: ready to
//! run, with its options open, while the pictures are looked at, and its limit and error states.
//! The geometry is `task_ui::geo`'s, so what is drawn here is the card as built with one more
//! segment, one more button and a thumbnail on a picture's row.

use super::{Art, caption, chip, footer_buttons, hline, link, thumb, tick};
use crate::task_ui::{self, GROUP_H, ROW_H};
use mdshared_ui::gfx::{Face, Painter, Rect, mix};
use mdshared_ui::glyph;
use mdshared_ui::overlay::{DIALOG_PAD, dialog_button_w, draw_dialog_button, draw_dialog_card};
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};
use mdshared_ui::segments::{SEGMENT_H, draw_segments, switch_width};
use mdshared_ui::textfield::{FieldColors, TextField};
use mdshared_ui::theme::Palette;

const PAD: f32 = DIALOG_PAD;
/// A saved prompt's row: 40, not the 44 of `task_ui::PROMPT_ROW`, so two rows show whole in the
/// 80 px the list has (the plan proposes the same change to the window).
const PROMPT_H: f32 = 40.0;

/// Which state of the window is drawn. (`Face` is the font's name here.)
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Face_ {
    Ready,
    Options,
    Looking,
    Limit,
    Failed,
    NoVision,
    /// Opened from six notes selected in the Notes pane: Rewrite them.
    FromNotes,
    /// Opened empty by the one command, with the Add menu open.
    AddMenu,
    /// File them, the text files copied into the library with their pictures.
    FileCopy,
    /// The same, moved.
    FileMove,
}

/// What a face of the window shows above its list.
struct Spec {
    sub: &'static str,
    task: usize,
    instruction: &'static str,
    placeholder: &'static str,
    prompts: &'static [(&'static str, &'static str)],
    dest: &'static str,
    dest_right: [&'static str; 2],
    /// File them's switch for what happens to the files, and which is chosen.
    mode: Option<usize>,
    options: bool,
    in_place: bool,
}

const SUB: &str =
    "They are read here and sent only to the provider you chose. Nothing is opened in a tab.";
const SUB_NOTES: &str = "6 notes from the Notes pane: Recipes \u{203A} Pasta (4) and 2 of Bread. Nothing is opened in a tab.";
const ASK_NOTES: &str =
    "What should the notes be? For example: describe each and sort them by topic";

fn spec(face: Face_) -> Spec {
    match face {
        Face_::FromNotes => Spec {
            sub: SUB_NOTES,
            task: 3,
            instruction: "put the ingredients in a table, amounts in grams",
            placeholder: "",
            prompts: &REWRITE_PROMPTS,
            dest: "",
            dest_right: ["", ""],
            mode: None,
            options: false,
            in_place: true,
        },
        Face_::AddMenu => Spec {
            sub: SUB,
            task: 0,
            instruction: "",
            placeholder: ASK_NOTES,
            prompts: &PROMPTS,
            dest: "Decide on the plan",
            dest_right: ["", ""],
            mode: None,
            options: true,
            in_place: false,
        },
        Face_::FileCopy | Face_::FileMove => Spec {
            sub: SUB,
            task: 1,
            instruction: "file these under my trips, by country",
            placeholder: "",
            prompts: &FILE_PROMPTS,
            dest: "Travel",
            dest_right: [
                "\u{2192} library\\Travel\\",
                "a folder per section, pictures beside their notes",
            ],
            mode: Some(if face == Face_::FileCopy { 1 } else { 2 }),
            options: false,
            in_place: false,
        },
        _ => Spec {
            sub: SUB,
            task: 0,
            instruction: "categorize and describe the screenshots",
            placeholder: ASK_NOTES,
            prompts: &PROMPTS,
            dest: "A new group: Screenshots",
            dest_right: [
                "\u{2192} library\\Screenshots\\",
                "pictures copied beside the notes, in pictures\\",
            ],
            mode: None,
            options: true,
            in_place: false,
        },
    }
}

const REWRITE_PROMPTS: [(&str, &str); 3] = [
    (
        "Fix and tidy",
        "Fix spelling and headings, keep every word of the meaning",
    ),
    (
        "Translate to Turkish",
        "The whole note, headings too; names and amounts as they are",
    ),
    ("Shorten", "Half the length, nothing of substance dropped"),
];

const FILE_PROMPTS: [(&str, &str); 3] = [
    ("By subject", "A section per subject, at most eight"),
    (
        "By year and month",
        "A section per year, a subsection per month",
    ),
    ("By person", "A section per person the file is about"),
];

/// What a row says at its right, and how.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    /// Takes part: the dim text of what is sent.
    Plain,
    /// Looked at already.
    Seen,
    /// Asked for now.
    Asking,
    /// Not looked at: the request failed.
    Missed,
    /// Left out, with the reason under the name.
    Out,
}

struct Row {
    art: Option<Art>,
    name: &'static str,
    sub: &'static str,
    right: &'static str,
    mark: Mark,
}

const fn pic(art: Art, name: &'static str, sub: &'static str, right: &'static str) -> Row {
    Row {
        art: Some(art),
        name,
        sub,
        right,
        mark: Mark::Plain,
    }
}

const PICS: [Row; 5] = [
    pic(
        Art::Error,
        "Screenshot 2026-11-20 143022.png",
        "1920 \u{00D7} 1080 \u{00B7} 412 KB",
        "sent as it is",
    ),
    pic(
        Art::Error,
        "Screenshot 2026-11-20 143107.png",
        "1920 \u{00D7} 1080 \u{00B7} 388 KB",
        "sent as it is",
    ),
    pic(
        Art::Terminal,
        "Screenshot 2026-11-21 091455.png",
        "3840 \u{00D7} 2160 \u{00B7} 1.6 MB",
        "sent smaller: 1568 \u{00D7} 882 \u{00B7} 204 KB",
    ),
    pic(
        Art::Settings,
        "settings-before.png",
        "2560 \u{00D7} 1440 \u{00B7} 310 KB",
        "sent smaller: 1568 \u{00D7} 882 \u{00B7} 141 KB",
    ),
    pic(
        Art::Photo,
        "IMG_4032.jpg",
        "4032 \u{00D7} 3024 \u{00B7} 6.8 MB",
        "sent smaller: 1568 \u{00D7} 1176 \u{00B7} 372 KB",
    ),
];

/// What the model said of the first four pictures, as a row shows it once it is looked at.
const SEEN: [&str; 4] = [
    "The sign-in window with a red error: \u{201c}Your session has expired\u{201d}",
    "The same window after Retry: the password box is empty again",
    "A terminal: cargo build stops at error E0308 in src/auth.rs",
    "The Display page of Settings, with Night light on and scale at 125%",
];

const TEXT_ROW: Row = Row {
    art: None,
    name: "bug-notes.txt",
    sub: "3.4 KB \u{00B7} read and sent whole",
    right: "",
    mark: Mark::Plain,
};

const PROMPTS: [(&str, &str); 3] = [
    (
        "Describe and sort",
        "Describe each picture and sort them into notes by what they are about",
    ),
    (
        "Read the text in them",
        "Write out the text each picture shows, word for word, under the picture",
    ),
    (
        "A step-by-step guide",
        "One note: the screenshots in order, each with what to do at that step",
    ),
];

pub fn draw(p: &mut Painter, pal: &Palette, w: f32, h: f32, face: Face_) {
    let r = task_ui::card(w, h);
    let g = task_ui::geo(r);
    let footer = draw_dialog_card(p, pal, r);
    let x = r.x + PAD;
    let inner = r.w - 2.0 * PAD;
    let busy = face == Face_::Looking;
    let sp = spec(face);

    // ---- header -----------------------------------------------------------------------------
    p.text(
        Face::Semibold,
        17.0,
        x,
        r.y + 30.0,
        "AI task on your files",
        pal.text,
        inner - 130.0,
    );
    p.text(
        Face::Ui,
        12.5,
        x,
        r.y + 50.0,
        sp.sub,
        pal.muted,
        inner - 110.0,
    );
    // One button for every way in: a menu, drawn open on the `AddMenu` face.
    let add_w = dialog_button_w(p, "Add") + 18.0;
    let add = Rect::new(r.right() - PAD - add_w, r.y + 19.0, add_w, 30.0);
    draw_dialog_button(p, pal, add, "Add     ", false, false, false);
    p.icon(glyph::DOWN, 12.0, add.right() - 18.0, add.cy(), pal.muted);

    // ---- what to do -------------------------------------------------------------------------
    let labels = ["Write notes", "File them", "Split them", "Rewrite them"];
    let sw_w = switch_width(p, &labels).min(inner);
    let sw = Rect::new(x, g.task.y, sw_w, SEGMENT_H);
    draw_segments(p, pal, sw, &labels, Some(sp.task), None);
    if let Some(mode) = sp.mode {
        // File them: what happens to the files themselves. Beside the task, because it is as
        // much a part of what is done as the task is.
        let modes = ["Link them", "Copy them in", "Move them in"];
        let mw = switch_width(p, &modes);
        let ms = Rect::new(x + inner - mw, g.task.y, mw, SEGMENT_H);
        draw_segments(p, pal, ms, &modes, Some(mode), None);
        p.text_right(
            Face::Ui,
            12.0,
            ms.x - 12.0,
            g.task.cy(),
            "THE FILES",
            pal.faint,
        );
    } else {
        p.text_right(
            Face::Ui,
            12.0,
            x + inner,
            g.task.cy(),
            "WHAT TO DO",
            pal.faint,
        );
    }

    // ---- the instruction --------------------------------------------------------------------
    let field = g.field;
    p.frame(
        field,
        6.0,
        pal.field,
        if busy { pal.line } else { pal.accent },
    );
    if busy {
        p.text(
            Face::Ui,
            12.5,
            field.x + 12.0,
            field.cy(),
            "Looking at the pictures\u{2026} 1 of 2 requests answered \u{00B7} Claude \u{00B7} 14 s",
            pal.muted,
            field.w - 24.0,
        );
    } else {
        let colors = FieldColors {
            text: pal.text,
            placeholder: pal.faint,
            selection: pal.selection,
            caret: pal.text,
        };
        let mut tf = TextField::new();
        tf.set_text(sp.instruction);
        tf.draw(
            p,
            Rect::new(field.x + 12.0, field.y, field.w - 24.0, field.h),
            Face::Ui,
            13.5,
            &colors,
            sp.placeholder,
            true,
            matches!(face, Face_::Ready | Face_::FromNotes),
        );
    }

    // ---- the saved prompts ------------------------------------------------------------------
    let prompts = g.prompts;
    caption(p, pal, x, prompts.y + 8.0, "SAVED PROMPTS", inner);
    let list = Rect::new(prompts.x, prompts.y + 16.0, prompts.w, prompts.h - 16.0);
    let old = p.clip(list);
    for (k, (name, text)) in sp.prompts.iter().enumerate() {
        let row = Rect::new(list.x, list.y + k as f32 * PROMPT_H, list.w, PROMPT_H);
        if k == 0 && !busy && !sp.instruction.is_empty() && sp.task == 0 {
            p.rbox(row.inset(2.0), 6.0, pal.hover);
        }
        p.text(
            Face::Semibold,
            13.0,
            row.x + 12.0,
            row.y + 13.0,
            name,
            if busy { pal.muted } else { pal.text },
            row.w - 40.0,
        );
        p.text(
            Face::Ui,
            11.5,
            row.x + 12.0,
            row.y + 28.0,
            text,
            pal.muted,
            row.w - 40.0,
        );
    }
    p.restore(old);
    if let Some((track, thumb_r)) = scrollbar(list, 3.0 * PROMPT_H, list.h, 0.0) {
        draw_scrollbar(p, pal, track, thumb_r, false);
    }

    // ---- where it goes, and the options -----------------------------------------------------
    caption(p, pal, x, g.dest.y + 17.0, "WHERE IT GOES", 100.0);
    let dest = Rect::new(x + 100.0, g.dest.y + 2.0, 240.0, 30.0);
    let open = face == Face_::Options;
    let mut opt = dest;
    if sp.in_place {
        p.text(
            Face::Ui,
            12.5,
            x + 100.0,
            g.dest.y + 17.0,
            "In place: each note is rewritten where it is, and its original goes to AI runs.",
            pal.muted,
            inner - 100.0,
        );
    } else {
        p.frame(dest, 6.0, pal.field, pal.line);
        p.text(
            Face::Ui,
            13.0,
            dest.x + 10.0,
            dest.cy(),
            sp.dest,
            pal.text,
            dest.w - 30.0,
        );
        p.icon(glyph::DOWN, 12.0, dest.right() - 18.0, dest.cy(), pal.muted);
        if sp.options {
            let opt_w = p.measure(Face::Ui, 13.0, "Options") + 44.0;
            opt = Rect::new(dest.right() + 10.0, dest.y, opt_w, 30.0);
            p.frame(
                opt,
                6.0,
                if open {
                    mix(pal.accent, pal.popup, 0.82)
                } else {
                    pal.field
                },
                if open { pal.accent } else { pal.line },
            );
            p.text(
                Face::Ui,
                13.0,
                opt.x + 10.0,
                opt.cy(),
                "Options",
                pal.text,
                opt.w,
            );
            p.icon(glyph::DOWN, 12.0, opt.right() - 18.0, opt.cy(), pal.muted);
        } else {
            // File them's own field, as built: the section every file goes under.
            let sec = Rect::new(dest.right() + 12.0, dest.y, 180.0, 30.0);
            p.frame(sec, 6.0, pal.field, pal.line);
            p.text(
                Face::Ui,
                12.5,
                sec.x + 10.0,
                sec.cy(),
                "under a section (none)",
                pal.faint,
                sec.w - 20.0,
            );
        }
        p.text_right(
            Face::Ui,
            11.5,
            x + inner,
            dest.cy() - 8.0,
            sp.dest_right[0],
            pal.faint,
        );
        p.text_right(
            Face::Ui,
            11.5,
            x + inner,
            dest.cy() + 8.0,
            sp.dest_right[1],
            pal.faint,
        );
    }

    // ---- the files --------------------------------------------------------------------------
    let rows = rows_of(face);
    let old = p.clip(g.list);
    let mut y = g.list.y + 4.0;
    let mut content = 4.0;
    for item in &rows {
        match item {
            Item::Heading(text, warn) => {
                p.text(
                    Face::Semibold,
                    10.5,
                    g.list.x + PAD,
                    y + GROUP_H / 2.0,
                    text,
                    if *warn { pal.warn } else { pal.muted },
                    g.list.w - 2.0 * PAD,
                );
                y += GROUP_H;
                content += GROUP_H;
            }
            Item::File(row) => {
                let at = Rect::new(g.list.x + 8.0, y + 1.0, g.list.w - 16.0, ROW_H - 2.0);
                file_row(p, pal, at, row, busy);
                y += ROW_H;
                content += ROW_H;
            }
        }
    }
    p.restore(old);
    if face == Face_::Limit {
        // The list is longer than it shows: 104 pictures.
        if let Some((track, thumb_r)) = scrollbar(g.list, 3640.0, g.list.h, 0.0) {
            draw_scrollbar(p, pal, track, thumb_r, false);
        }
    }
    let _ = content;
    if face == Face_::AddMenu {
        p.text_center(
            Face::Ui,
            13.0,
            Rect::new(g.list.x, g.list.y + 150.0, g.list.w, 24.0),
            "Add files, or drop them here from File Explorer. Ctrl+V adds a copied picture.",
            pal.faint,
        );
        add_menu(p, pal, add);
    }

    if open {
        options(p, pal, Rect::new(opt.x, opt.bottom() + 4.0, 520.0, 0.0));
    }

    // ---- footer -----------------------------------------------------------------------------
    let via = "Claude \u{00B7} claude-sonnet-5-5";
    let (buttons, bold, note, warn): (Vec<(&str, bool, bool)>, &str, String, bool) = match face {
        Face_::Ready | Face_::Options => (
            vec![
                ("Cancel", false, true),
                ("Send 5 pictures and 1 file", true, true),
            ],
            "5 pictures, 1.5 MB as sent, and 1 text file \u{00B7} 3 requests",
            format!("{via} \u{00B7} the pictures themselves are sent"),
            false,
        ),
        Face_::Looking => (
            vec![
                ("Stop", false, true),
                ("Send 5 pictures and 1 file", true, false),
            ],
            "4 of 5 pictures looked at \u{00B7} 1.1 MB sent in 2 requests",
            "Stop keeps what was looked at; nothing is written until you have seen the plan."
                .to_string(),
            false,
        ),
        Face_::Limit => (
            vec![("Cancel", false, true), ("Send 104 pictures", true, false)],
            "104 pictures are ticked: one run takes 100.",
            "Untick 4, or send the rest in a second run. Nothing has been sent.".to_string(),
            true,
        ),
        Face_::Failed => (
            vec![
                ("Cancel", false, true),
                ("Go on with 4", false, true),
                ("Try it again", true, true),
            ],
            "Request 2 of 2 failed: Claude said to wait 38 s (rate limit).",
            "4 pictures were looked at and are kept. 1 was not. Nothing is written yet."
                .to_string(),
            true,
        ),
        Face_::FromNotes => (
            vec![("Cancel", false, true), ("Run on 6 notes", true, true)],
            "6 notes \u{00B7} 14.8 KB in 6 requests, 3 at a time",
            format!("{via} \u{00B7} each note is shown as a list of changes before it is written"),
            false,
        ),
        Face_::AddMenu => (
            vec![
                ("Cancel", false, true),
                ("Nothing to send yet", true, false),
            ],
            "No files yet",
            format!("{via} \u{00B7} nothing is sent until you press the button"),
            false,
        ),
        Face_::FileCopy => (
            vec![("Cancel", false, true), ("Run on 4 files", true, true)],
            "3 files copied into library\\Travel\\ with 4 pictures (5.1 MB), 1 linked",
            format!(
                "{via} \u{00B7} a digest of each file is sent, no picture. The originals stay."
            ),
            false,
        ),
        Face_::FileMove => (
            vec![("Cancel", false, true), ("Run on 4 files", true, true)],
            "3 files and 4 pictures leave D:\\Docs\\Trips for library\\Travel\\, 1 linked",
            "Other files that link to them will not find them. Undo puts everything back."
                .to_string(),
            true,
        ),
        Face_::NoVision => (
            vec![
                ("Cancel", false, true),
                ("Open AI settings", false, true),
                ("Send 5 pictures and 1 file", true, false),
            ],
            "This model does not read pictures: llama3.3 at localhost:11434.",
            "Nothing is written. Choose a model that reads pictures.".to_string(),
            true,
        ),
    };
    let left = footer_buttons(p, pal, r, footer, &buttons);
    let link_x = if matches!(
        face,
        Face_::Ready
            | Face_::Options
            | Face_::Limit
            | Face_::FromNotes
            | Face_::FileCopy
            | Face_::FileMove
    ) {
        link(p, pal, left - 16.0, footer.cy(), "Show what is sent")
    } else {
        left
    };
    let room = (link_x - x - 14.0).max(0.0);
    p.text(
        Face::Semibold,
        12.0,
        x,
        footer.cy() - 8.0,
        bold,
        if warn { pal.warn } else { pal.text },
        room,
    );
    p.text(Face::Ui, 11.0, x, footer.cy() + 9.0, &note, pal.muted, room);
    let _ = (hline, chip);
}

const PICTURES_HEAD: &str = "PICTURES \u{00B7} SENT TO THE AI, AS EACH ROW SAYS \u{00B7} LOCATION AND CAMERA DETAILS REMOVED";

enum Item {
    Heading(&'static str, bool),
    File(Row),
}

const fn row(name: &'static str, sub: &'static str, right: &'static str, mark: Mark) -> Row {
    Row {
        art: None,
        name,
        sub,
        right,
        mark,
    }
}

const NOTES_HEAD: &str = "NOTES \u{00B7} EACH READ AND SENT WHOLE \u{00B7} REWRITTEN IN PLACE AFTER YOU HAVE SEEN ITS CHANGES";
const COPY_HEAD: &str = "TEXT \u{00B7} A DIGEST IS SENT \u{00B7} COPIED INTO THE GROUP\u{2019}S FOLDERS, WITH THE PICTURES THEY SHOW";
const MOVE_HEAD: &str = "TEXT \u{00B7} A DIGEST IS SENT \u{00B7} MOVED INTO THE GROUP\u{2019}S FOLDERS, WITH THE PICTURES THEY SHOW";

const NOTES: [(&str, &str); 6] = [
    (
        "Carbonara",
        "Recipes \u{203A} Pasta \u{00B7} carbonara.md \u{00B7} 2.1 KB",
    ),
    (
        "Cacio e pepe",
        "Recipes \u{203A} Pasta \u{00B7} cacio-e-pepe.md \u{00B7} 1.4 KB",
    ),
    (
        "Lasagne",
        "Recipes \u{203A} Pasta \u{00B7} lasagne.md \u{00B7} 4.9 KB",
    ),
    (
        "Rag\u{00F9}",
        "Recipes \u{203A} Pasta \u{00B7} ragu.md \u{00B7} 2.6 KB",
    ),
    (
        "Sourdough starter",
        "Recipes \u{203A} Bread \u{00B7} sourdough-starter.md \u{00B7} 1.8 KB",
    ),
    (
        "Rye loaf",
        "Recipes \u{203A} Bread \u{00B7} rye-loaf.md \u{00B7} 2.0 KB",
    ),
];

fn rows_of(face: Face_) -> Vec<Item> {
    let mut out = Vec::new();
    match face {
        Face_::AddMenu => {}
        Face_::FromNotes => {
            out.push(Item::Heading(NOTES_HEAD, false));
            for (name, sub) in NOTES {
                out.push(Item::File(row(name, sub, "read and sent", Mark::Plain)));
            }
        }
        Face_::FileCopy | Face_::FileMove => {
            let moved = face == Face_::FileMove;
            out.push(Item::Heading(
                if moved { MOVE_HEAD } else { COPY_HEAD },
                false,
            ));
            out.push(Item::File(row(
                "japan-2025.md",
                "D:\\Docs\\Trips \u{00B7} 18 KB \u{00B7} shows 3 pictures",
                if moved {
                    "moved in, with 3 pictures"
                } else {
                    "copied in, with 3 pictures"
                },
                Mark::Plain,
            )));
            out.push(Item::File(row(
                "kyoto-temples.md",
                "D:\\Docs\\Trips \u{00B7} 9 KB \u{00B7} shows 2 pictures, 1 of them also in japan-2025.md",
                if moved {
                    "moved in, with 2 pictures"
                } else {
                    "copied in, with 2 pictures"
                },
                Mark::Plain,
            )));
            out.push(Item::File(row(
                "packing-list.txt",
                "D:\\Docs\\Trips \u{00B7} 1 KB",
                if moved { "moved in" } else { "copied in" },
                Mark::Plain,
            )));
            out.push(Item::File(row(
                "lisbon.md",
                "library\\Travel \u{00B7} 6 KB \u{00B7} already inside the library\u{2019}s folder",
                "linked where it is",
                Mark::Plain,
            )));
            out.push(Item::Heading(
                "PICTURES \u{00B7} NOT FILED: A PICTURE IS NOT A NOTE",
                false,
            ));
            out.push(Item::File(Row {
                art: Some(Art::Photo),
                name: "IMG_0412.jpg",
                sub: "Write notes puts it inside a note.",
                right: "Write notes from these instead",
                mark: Mark::Out,
            }));
        }
        Face_::Limit => {
            out.push(Item::Heading(
                "PICTURES \u{00B7} 104 TICKED, AND ONE RUN TAKES 100 \u{00B7} 3 LEFT OUT BELOW",
                true,
            ));
            out.push(Item::File(Row {
                art: Some(Art::Photo),
                name: "IMG_2210.HEIC",
                sub: "HEIC: not a picture this program can read. Save it as JPEG first.",
                right: "left out",
                mark: Mark::Out,
            }));
            out.push(Item::File(Row {
                art: Some(Art::Chart),
                name: "wall-map-scan.png",
                sub: "21000 \u{00D7} 4000: more pixels than can be read here (64 million).",
                right: "left out",
                mark: Mark::Out,
            }));
            out.push(Item::File(Row {
                art: Some(Art::Settings),
                name: "screen-recording.gif",
                sub: "An animation of 5.2 MB: it cannot be made smaller, and 3.5 MB is the most.",
                right: "left out",
                mark: Mark::Out,
            }));
            for f in PICS.iter().take(4) {
                out.push(Item::File(pic(
                    f.art.unwrap_or(Art::Photo),
                    f.name,
                    f.sub,
                    f.right,
                )));
            }
        }
        _ => {
            out.push(Item::Heading(PICTURES_HEAD, false));
            for (k, f) in PICS.iter().enumerate() {
                let (sub, right, mark) = match face {
                    Face_::Looking | Face_::Failed => match k {
                        0 => (SEEN[0], "looked at", Mark::Seen),
                        1 => (SEEN[1], "looked at", Mark::Seen),
                        2 => (SEEN[2], "looked at", Mark::Seen),
                        3 => (SEEN[3], "looked at", Mark::Seen),
                        _ if face == Face_::Looking => {
                            (f.sub, "being looked at\u{2026}", Mark::Asking)
                        }
                        _ => (f.sub, "not looked at", Mark::Missed),
                    },
                    _ => (f.sub, f.right, Mark::Plain),
                };
                out.push(Item::File(Row {
                    art: f.art,
                    name: f.name,
                    sub,
                    right,
                    mark,
                }));
            }
            out.push(Item::Heading("TEXT \u{00B7} READ AND SENT WHOLE", false));
            out.push(Item::File(TEXT_ROW));
        }
    }
    out
}

fn file_row(p: &mut Painter, pal: &Palette, row: Rect, f: &Row, busy: bool) {
    let out = f.mark == Mark::Out;
    let tick_at = Rect::new(row.x + 12.0, row.cy() - 8.0, 16.0, 16.0);
    tick(p, pal, tick_at, !out, !out && !busy);
    let mut name_x = tick_at.right() + 12.0;
    if let Some(art) = f.art {
        let t = Rect::new(name_x, row.cy() - 12.0, 36.0, 24.0);
        thumb(p, pal, t, art);
        if out {
            p.fill_a(t, pal.popup, 0.55);
        }
        name_x = t.right() + 10.0;
    }
    let right_w = p.measure(Face::Ui, 11.5, f.right);
    let room = (row.right() - 24.0 - right_w - name_x).max(80.0);
    p.text(
        Face::Ui,
        13.0,
        name_x,
        row.y + 10.0,
        f.name,
        if out { pal.muted } else { pal.text },
        room,
    );
    p.text(
        Face::Ui,
        11.0,
        name_x,
        row.y + 24.0,
        f.sub,
        if out { pal.faint } else { pal.muted },
        room,
    );
    let colour = match f.mark {
        Mark::Seen => pal.accent,
        Mark::Missed => pal.warn,
        Mark::Out if f.right.starts_with("Write notes") => mix(pal.accent, pal.text, 0.25),
        Mark::Out => pal.faint,
        _ => pal.muted,
    };
    p.text_right(
        Face::Ui,
        11.5,
        row.right() - 12.0,
        row.cy(),
        f.right,
        colour,
    );
}

/// The options of "Write notes", open under their button. `at` gives the corner and the width.
fn options(p: &mut Painter, pal: &Palette, at: Rect) {
    const ROW: f32 = 36.0;
    let items: [(&str, &str, bool, bool); 7] = [
        (
            "Copy the pictures beside the notes",
            "into pictures\\ in each note\u{2019}s folder; the originals stay where they are",
            true,
            true,
        ),
        (
            "Name the copies after what they show",
            "login-error.png, not Screenshot 2026-11-20 143022.png",
            true,
            true,
        ),
        (
            "Describe each picture",
            "a sentence or two under it, and a short text in its brackets",
            true,
            true,
        ),
        (
            "Write out the text in the pictures",
            "word for word, as a quote under the picture: for screenshots and scans",
            false,
            true,
        ),
        (
            "Remove location and camera details before sending",
            "what is sent has no place, date or device in it; the copies keep theirs",
            true,
            true,
        ),
        (
            "Send larger pictures, for small print",
            "2576 px instead of 1568 on the long side: about 2.7 times the cost",
            false,
            true,
        ),
        (
            "Let it add to notes I already have",
            "off in Settings \u{203A} AI: a run writes new notes only",
            false,
            false,
        ),
    ];
    let head = 58.0;
    let h = head + ROW * items.len() as f32 + 8.0;
    let r = Rect::new(at.x, at.y, at.w, h);
    p.shadow(r, 8.0, pal.dark);
    p.frame(r, 8.0, pal.popup, pal.popup_line);
    caption(p, pal, r.x + 16.0, r.y + 29.0, "HOW MANY NOTES", 120.0);
    let labels = ["As many as it takes", "One note", "One per picture"];
    let sw_w = switch_width(p, &labels);
    let sw = Rect::new(r.right() - 16.0 - sw_w, r.y + 15.0, sw_w, SEGMENT_H);
    draw_segments(p, pal, sw, &labels, Some(0), None);
    hline(p, r.x + 1.0, r.y + head - 1.0, r.w - 2.0, pal.popup_line);
    for (k, (label, note, on, enabled)) in items.iter().enumerate() {
        let row = Rect::new(r.x + 4.0, r.y + head + 2.0 + k as f32 * ROW, r.w - 8.0, ROW);
        if k == 3 {
            p.rbox(row.inset(1.0), 5.0, pal.hover);
        }
        tick(
            p,
            pal,
            Rect::new(row.x + 12.0, row.y + 4.0, 16.0, 16.0),
            *on,
            *enabled,
        );
        p.text(
            Face::Ui,
            13.0,
            row.x + 40.0,
            row.y + 12.0,
            label,
            if *enabled { pal.text } else { pal.muted },
            row.w - 52.0,
        );
        p.text(
            Face::Ui,
            11.0,
            row.x + 40.0,
            row.y + 26.0,
            note,
            pal.faint,
            row.w - 52.0,
        );
    }
}

/// The one way in: what "Add" offers, open under its button.
fn add_menu(p: &mut Painter, pal: &Palette, button: Rect) {
    let items: [(&str, &str, bool); 6] = [
        ("This document", "sample.md", false),
        ("The open tabs", "4 files", false),
        ("The notes selected in the Notes pane", "6 notes", true),
        ("Files\u{2026}", "", false),
        ("A folder\u{2026}", "", false),
        ("The picture on the clipboard", "1920 \u{00D7} 1080", false),
    ];
    let w = 380.0;
    let h = 30.0 * items.len() as f32 + 8.0 + 9.0;
    let r = Rect::new(button.right() - w, button.bottom() + 4.0, w, h);
    p.shadow(r, 8.0, pal.dark);
    p.frame(r, 8.0, pal.popup, pal.popup_line);
    let mut y = r.y + 4.0;
    for (k, (label, note, sep_after)) in items.iter().enumerate() {
        let row = Rect::new(r.x + 4.0, y, r.w - 8.0, 30.0);
        if k == 0 {
            p.rbox(row, 5.0, pal.hover);
        }
        p.text(
            Face::Ui,
            13.0,
            row.x + 12.0,
            row.cy(),
            label,
            pal.text,
            260.0,
        );
        p.text_right(
            Face::Ui,
            12.0,
            row.right() - 12.0,
            row.cy(),
            note,
            pal.faint,
        );
        y += 30.0;
        if *sep_after {
            hline(p, r.x + 1.0, y + 4.0, r.w - 2.0, pal.popup_line);
            y += 9.0;
        }
    }
}
