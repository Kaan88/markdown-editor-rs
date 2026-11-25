//! `--task-demo notes…`: Write notes for a shot, made from `fixtures/notes-demo/` with no engine
//! and nothing sent. Every figure on it is made up: what the engine would say a picture is sent
//! as, what the model said of it, and the failure.

use super::*;
use crate::ai::PictureInfo;
use crate::notes_task::{self, Measure, Run};
use crate::task_ui::{DestChoice, FileKind, Group, TaskKind, TaskScreen};
use mdedit_core::notes_ai::Look;

/// Pixels across and down.
type Px = (u32, u32);

/// What each made-up picture is sent as: (pixels as sent, bytes, original pixels).
const FIGURES: [(Px, u64, Px); 5] = [
    ((1568, 1176), 372_004, (4032, 3024)),
    ((1568, 882), 204_118, (2880, 1620)),
    ((160, 120), 815, (160, 120)),
    ((1568, 1176), 351_270, (4032, 3024)),
    ((1179, 2556), 512_660, (1179, 2556)),
];

/// What the model said of each, for the shots after the look.
const LOOKS: [(&str, &str); 5] = [
    ("The oven", "An oven with its door open, the light on."),
    (
        "The shop's list",
        "A shop's list of grout colours, \u{201c}blue\u{201d} ringed.",
    ),
    (
        "The sink",
        "A white sink under the window, its tap dripping.",
    ),
    ("The tap", "A chrome mixer tap, the kind with one lever."),
    (
        "Blue tiles",
        "Blue square tiles behind the counter, two of them cracked.",
    ),
];

/// The arrange's answer for the plan card's shots (phase V4): a note per room in two sections,
/// the shop's list in none of them; or, for "One note", one note with a heading per room.
fn plan_answer(one: bool) -> String {
    let notes = if one {
        serde_json::json!([{
            "title": "The house, room by room",
            "sections": [],
            "body": "What each room needs, from five pictures and the shopping list.\n\n\
                     ## Kitchen\n\n![The oven, door open](picture:0)\n\n\
                     The oven glass is brown with old fat: oven cleaner and a scraper.\n\n\
                     ![Blue tiles, two cracked](picture:4)\n\nFour blue tiles and blue grout.\n\n\
                     ## Bathroom\n\n![The sink under the window](picture:2)\n\n\
                     ![A chrome mixer tap](picture:3)\n\n\
                     The tap drips: a mixer with one lever fits the same holes."
        }])
    } else {
        serde_json::json!([
            {
                "title": "The oven needs cleaning",
                "sections": ["Kitchen"],
                "body": "The oven door is open and the light is on: the glass is brown with old \
                         fat.\n\n![The oven, door open](picture:0)\n\n\
                         Buy oven cleaner, the spray kind, and a scraper for the glass."
            },
            {
                "title": "New tiles behind the counter",
                "sections": ["Kitchen"],
                "body": "Two of the blue tiles behind the counter are cracked.\n\n\
                         ![Blue tiles, two cracked](picture:4)\n\n## What to buy\n\n\
                         - Four blue tiles, 10 cm square\n- Grout, blue"
            },
            {
                "title": "A new tap for the sink",
                "sections": ["Bathroom"],
                "body": "The tap over the white sink drips day and night.\n\n\
                         ![The sink under the window](picture:2)\n\n\
                         A chrome mixer with one lever would fit the same holes:\n\n\
                         ![A chrome mixer tap](picture:3)"
            }
        ])
    };
    serde_json::json!({"version": 1, "notes": notes, "unused": [1]}).to_string()
}

impl App {
    /// The demos: `notes` (the window), `notes-options`, `notes-looking`, `notes-failed`,
    /// `notes-no-vision`, `notes-limit`, `notes-off`, and the plan card, `notes-plan` and
    /// `notes-plan-one`.
    pub(super) fn notes_demo(&mut self, name: &str) -> bool {
        let dir = Path::new("fixtures/notes-demo");
        let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|r| r.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        if names.is_empty() {
            return false;
        }
        names.sort();
        self.task_seq += 1;
        let on = name != "notes-off";
        let via = if on {
            "Claude API \u{00B7} Claude Sonnet 5.5".to_string()
        } else {
            self.ai_via()
        };
        let mut s = TaskScreen::new(
            TaskKind::Notes,
            vec![Group {
                name: "House".to_string(),
                file: PathBuf::from("House.md"),
                home: PathBuf::from("House"),
            }],
            DestChoice::Group(0),
            Vec::new(),
            via,
        );
        s.id = self.task_seq;
        s.win = self.logical();
        s.scale = self.scale;
        s.ai_on = on;
        s.instruction
            .set_text("One note per room, with what to buy for it.");
        self.task = Some(s);
        self.close_for_ai_panel();
        let mut probed: Vec<_> = names.iter().map(|p| self.task_probe(p)).collect();
        // The pictures first, in the window's order, then the text file.
        probed.sort_by_key(|f| (f.kind != FileKind::Picture, f.name.to_lowercase()));
        for (k, f) in probed
            .iter_mut()
            .filter(|f| f.kind == FileKind::Picture)
            .enumerate()
        {
            let ((w, h), bytes, (ow, oh)) = FIGURES[k % FIGURES.len()];
            f.thumb = std::fs::read(&f.path)
                .ok()
                .and_then(|b| mdshared_text::decode_png_rgba(&b))
                .map(|(rgba, w, h)| notes_task::small(&rgba, w, h, notes_task::THUMB_PX));
            f.size = bytes.max(f.size) * 3;
            f.measure = if on {
                Measure::Ready(PictureInfo {
                    format: "jpeg".to_string(),
                    width: w,
                    height: h,
                    bytes,
                    sha256: String::new(),
                    original_width: ow,
                    original_height: oh,
                    done: Vec::new(),
                    thumb: None,
                })
            } else {
                Measure::Waiting
            };
        }
        let Some(s) = self.task.as_mut() else {
            return true;
        };
        s.files = probed;
        if name == "notes-limit" {
            // 104 ticked where a run takes 100, and three left out, each with its reason.
            let first = s.files[0].clone();
            let text = s.files.pop();
            for n in 0..99 {
                let mut f = first.clone();
                f.name = format!("IMG_{:04}.jpg", 4100 + n);
                f.path = PathBuf::from(&f.name);
                s.files.push(f);
            }
            for (n, why) in [
                "too large to read: 71 MB of 64 MB",
                "a HEIC photo: save it as JPEG or PNG first",
                "it could not be read: not a picture",
            ]
            .iter()
            .enumerate()
            {
                let mut f = first.clone();
                f.name = format!("left-out-{}.jpg", n + 1);
                f.measure = Measure::Refused((*why).to_string());
                s.files.push(f);
            }
            s.files.extend(text);
        }
        let pictures: Vec<usize> = (0..s.files.len())
            .filter(|&i| s.files[i].kind == FileKind::Picture)
            .collect();
        let looks = |ids: &[usize]| -> Vec<Look> {
            ids.iter()
                .map(|&i| Look {
                    id: i,
                    title: LOOKS[i % LOOKS.len()].0.to_string(),
                    says: LOOKS[i % LOOKS.len()].1.to_string(),
                    subjects: Vec::new(),
                    kind: "photo".to_string(),
                    text: String::new(),
                })
                .collect()
        };
        let batches = notes_task::batches(&pictures);
        match name {
            "notes-options" => s.options_open = true,
            "notes-looking" => {
                s.notes_run = Some(Run {
                    looks: looks(&batches[0]),
                    answered: vec![0],
                    in_flight: vec![(0, 1)],
                    next: 2,
                    parallel: 3,
                    requests: 2,
                    batches,
                    ..Run::default()
                });
            }
            "notes-failed" => {
                s.notes_run = Some(Run {
                    looks: looks(&batches[0]),
                    answered: vec![0],
                    failed: vec![(1, "Too many requests. Try again in 30 s.".to_string())],
                    next: 2,
                    stopped: true,
                    parallel: 3,
                    requests: 2,
                    batches,
                    ..Run::default()
                });
            }
            "notes-no-vision" => {
                s.notes_run = Some(Run {
                    fault: Some(
                        "This model does not read pictures: llama3.2 at localhost:11434. It said: \
                         model does not support images."
                            .to_string(),
                    ),
                    next: 1,
                    stopped: true,
                    batches,
                    ..Run::default()
                });
            }
            "notes-plan" | "notes-plan-one" => {
                let answer = plan_answer(name == "notes-plan-one");
                let arranged = mdedit_core::notes_ai::read_arrange(&answer, &pictures).ok();
                let texts: Vec<usize> = (0..s.files.len())
                    .filter(|&i| s.files[i].kind == FileKind::Text)
                    .collect();
                s.library = PathBuf::from("library");
                if let Some(g) = s.groups.first_mut() {
                    g.home = s.library.join("House");
                }
                s.notes_run = Some(Run {
                    looks: looks(&pictures),
                    answered: (0..batches.len()).collect(),
                    next: batches.len(),
                    parallel: 3,
                    requests: batches.len() as u64 + 1,
                    batches,
                    texts,
                    arranged,
                    ..Run::default()
                });
            }
            _ => {}
        }
        s.recount_limit();
        s.tick += 1;
        if name.starts_with("notes-plan") {
            self.notes_plan_show();
        }
        self.relayout_areas();
        self.redraw();
        true
    }
}
