//! "What is sent" for Write notes (`docs/ai-pictures-plan.md` sections 5 and 13, shot `sent`):
//! a card over the task window, one tab per request. A look tab shows its pictures as they go,
//! then the request's text word for word; the arrange tab shows its text and says that no
//! picture is in it. The card sends nothing and changes nothing in the window under it.
//!
//! This is its state and the layout that needs no painter: the tabs, the text wrapped to a
//! width, a point turned into a place in the text, and the selection. `sent_card_draw.rs` draws
//! it, `app/sent_card.rs` takes its events. A picture's figures are read from the window's
//! files each time the card is drawn, so one measured while the card is open shows at once.

use crate::task_ui::{FileEntry, TaskScreen};
use mdedit_core::notes_ai;
use mdshared_ui::gfx::Rect;
use std::collections::HashMap;

pub const CARD_W: f32 = 980.0;
pub const CARD_H: f32 = 704.0;
/// A line of the request's text, and the size of its face.
pub const LINE_H: f32 = 18.0;
pub const TEXT_SIZE: f32 = 11.5;
/// A picture's row in the table.
pub const ROW_H: f32 = 44.0;
/// The space inside the text's box, around the lines.
pub const TEXT_PAD: f32 = 12.0;

/// The first-time sentence of section 13, said once ever above the window's button.
pub const WHOLE: &str =
    "A picture is sent whole: everything visible in it, a password or a face included.";

/// The sentence of the arrange tab, over its text.
pub const ARRANGE_HEAD: &str = "No picture is in this request.";
pub const ARRANGE_LINE: &str = "It sends what the looks said of each picture, the text files \
     whole and your instruction. The lines of what each picture shows are filled in from the \
     looks\u{2019} answers when they are back, so they are not in the text below.";

/// One request: a tab of the card.
#[derive(Clone, Debug, PartialEq)]
pub struct SentTab {
    /// "Look 1 of 2", "Arrange".
    pub label: String,
    /// The pictures it sends, by their place in the window, in the order they are attached.
    pub pictures: Vec<usize>,
    pub system: &'static str,
    pub prompt: String,
    pub arrange: bool,
}

impl SentTab {
    /// The request's text, word for word: what "Open the text in a tab" shows of it too.
    pub fn text(&self) -> String {
        format!("SYSTEM:\n{}\n\nPROMPT:\n{}", self.system, self.prompt)
    }
}

/// What a press lands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardHit {
    Card,
    Tab(usize),
    Prev,
    Next,
    /// The request's text: a press there starts a selection.
    Text,
    Track,
    OpenTab,
    Close,
}

/// The card, while it is open.
#[derive(Clone, Debug, Default)]
pub struct SentCard {
    pub tabs: Vec<SentTab>,
    pub tab: usize,
    /// The AI is off: nothing is sent, and the card says so.
    pub off: bool,
    /// The connection and the model, as the window's footer names them.
    pub via: String,
    /// The window's footer line: what goes in how many requests.
    pub summary: String,
    /// The text of the tab shown, and its lines as wrapped (byte ranges, without the line
    /// break), for the width they were wrapped to.
    pub text: String,
    pub lines: Vec<(usize, usize)>,
    pub wrapped: Option<(usize, u32)>,
    /// The width of each character of the text's face, as measured when it was wrapped.
    pub widths: HashMap<char, f32>,
    /// The body's scroll, its height of content and of view, and where it is.
    pub scroll: f32,
    pub content: f32,
    pub body: Rect,
    /// Where the first line of the text is, the body's scroll left out (its top in content
    /// coordinates), and the left of the lines.
    pub text_top: f32,
    pub text_x: f32,
    /// The selection: where it started, and where it is now (byte offsets in `text`).
    pub sel: Option<(usize, usize)>,
    pub selecting: bool,
    /// The scrollbar's thumb is held, this far under its top.
    pub grab: Option<f32>,
    pub hover: Option<CardHit>,
    pub pressed: Option<CardHit>,
    pub hits: Vec<(CardHit, Rect)>,
}

impl SentCard {
    /// The card for the window as it is now: Write notes' requests, as "Open the text in a tab"
    /// writes them (`notes_task::request_text`).
    /// `headings` are the group's sections, as the arrange request sends them.
    pub fn of(s: &TaskScreen, headings: &[String]) -> SentCard {
        let mut card = SentCard {
            off: !s.ai_on,
            via: s.via.clone(),
            summary: s.summary(),
            ..SentCard::default()
        };
        if s.ai_on {
            card.tabs = tabs(s, s.instruction.text().trim(), headings);
        }
        card.show(0);
        card
    }

    /// Shows tab `k`: its text, wrapped when next drawn, from its top, nothing selected.
    pub fn show(&mut self, k: usize) {
        self.tab = k.min(self.tabs.len().saturating_sub(1));
        self.text = self
            .tabs
            .get(self.tab)
            .map(SentTab::text)
            .unwrap_or_default();
        self.lines.clear();
        self.wrapped = None;
        self.scroll = 0.0;
        self.sel = None;
        self.selecting = false;
    }

    pub fn max_scroll(&self) -> f32 {
        (self.content - self.body.h).max(0.0)
    }

    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll = (self.scroll + dy).clamp(0.0, self.max_scroll());
    }

    /// Wraps the text to `width` with `char_w` measuring a character, unless it is wrapped to
    /// that width already. Each character is measured once.
    pub fn wrap(&mut self, width: f32, char_w: &mut dyn FnMut(char) -> f32) {
        let key = (self.tab, width.max(0.0).round() as u32);
        if self.wrapped == Some(key) {
            return;
        }
        let widths = &mut self.widths;
        let mut w = |c: char| *widths.entry(c).or_insert_with(|| char_w(c));
        self.lines = wrap(&self.text, width, &mut w);
        self.wrapped = Some(key);
    }

    /// The width of `text[a..b]` as the face measures it, from the widths kept when it was
    /// wrapped.
    pub fn width_of(&self, a: usize, b: usize) -> f32 {
        let fallback = self.widths.get(&'M').copied().unwrap_or(7.0);
        self.text
            .get(a..b)
            .unwrap_or("")
            .chars()
            .map(|c| self.widths.get(&c).copied().unwrap_or(fallback))
            .sum()
    }

    /// The place in the text under the point `(x, y)`, on the screen: the nearest character
    /// boundary of the line under it, the first line above the text and the last below it.
    pub fn offset_at(&self, x: f32, y: f32) -> usize {
        let Some(last) = self.lines.len().checked_sub(1) else {
            return 0;
        };
        let cy = y - self.body.y + self.scroll - self.text_top;
        if cy < 0.0 {
            return self.lines[0].0;
        }
        let k = ((cy / LINE_H).floor() as usize).min(last);
        let (a, b) = self.lines[k];
        if cy >= (last + 1) as f32 * LINE_H {
            return self.lines[last].1;
        }
        let dx = x - self.text_x;
        let mut at = a;
        let mut left = 0.0;
        for (i, c) in self.text.get(a..b).unwrap_or("").char_indices() {
            let cw = self.width_of(a + i, a + i + c.len_utf8());
            if dx < left + cw / 2.0 {
                return a + i;
            }
            left += cw;
            at = a + i + c.len_utf8();
        }
        at
    }

    /// The selection, its start first; `None` when nothing is selected.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let (a, b) = self.sel?;
        let (lo, hi) = (a.min(b), a.max(b));
        (lo < hi).then_some((lo, hi))
    }

    pub fn selected_text(&self) -> Option<&str> {
        let (lo, hi) = self.selection()?;
        self.text.get(lo..hi)
    }

    pub fn select_all(&mut self) {
        self.sel = Some((0, self.text.len()));
    }

    pub fn hit_at(&self, x: f32, y: f32) -> Option<CardHit> {
        self.hits
            .iter()
            .rev()
            .find(|(_, r)| r.contains(x, y))
            .map(|(h, _)| *h)
    }

    /// The tab's pictures' bytes as sent, of those measured.
    pub fn bytes(&self, files: &[FileEntry]) -> u64 {
        self.tabs.get(self.tab).map_or(0, |t| {
            t.pictures
                .iter()
                .filter_map(|&i| match files.get(i).map(|f| &f.measure) {
                    Some(crate::notes_task::Measure::Ready(info)) => Some(info.bytes),
                    _ => None,
                })
                .sum()
        })
    }
}

/// The requests of Write notes as `notes_task::request_text` writes them: a look per four
/// pictures, then the arrange unless there is none.
pub fn tabs(s: &TaskScreen, instruction: &str, headings: &[String]) -> Vec<SentTab> {
    let (pictures, texts) = s.notes_parts();
    let batches = crate::notes_task::batches(&pictures);
    let mut out: Vec<SentTab> = batches
        .iter()
        .enumerate()
        .map(|(k, batch)| {
            let sent: Vec<notes_ai::Sent> = batch
                .iter()
                .map(|&i| notes_ai::Sent {
                    id: i,
                    name: s.files[i].name.clone(),
                })
                .collect();
            SentTab {
                label: format!("Look {} of {}", k + 1, batches.len()),
                pictures: batch.clone(),
                system: notes_ai::LOOK_SYSTEM,
                prompt: notes_ai::look_prompt(instruction, &sent, s.notes.text_out),
                arrange: false,
            }
        })
        .collect();
    if crate::notes_task::requests(pictures.len(), texts.len(), s.notes.how_many) > batches.len() {
        let files: Vec<notes_ai::TextFile> = texts
            .iter()
            .filter_map(|&i| {
                let f = &s.files[i];
                Some(notes_ai::TextFile {
                    name: f.name.clone(),
                    text: f.text.clone()?,
                })
            })
            .collect();
        out.push(SentTab {
            label: "Arrange".to_string(),
            pictures: Vec::new(),
            system: notes_ai::ARRANGE_SYSTEM,
            // "Arrange again…" asks the arrange with its own instruction.
            prompt: notes_ai::arrange_prompt(
                s.notes_run
                    .as_ref()
                    .and_then(|r| r.arrange_instruction.as_deref())
                    .unwrap_or(instruction),
                s.notes.how_many,
                &[],
                &files,
                headings,
            ),
            arrange: true,
        });
    }
    out
}

/// `text` cut into lines no wider than `width`: at its line breaks, then at the last space
/// that fits, or between two characters when no space does. Each line is a byte range without
/// its line break; an empty line is an empty range. A line always takes one character, so a
/// width narrower than one still ends.
pub fn wrap(text: &str, width: f32, char_w: &mut dyn FnMut(char) -> f32) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    for line in text.split('\n') {
        let end = start + line.len();
        let mut a = start;
        let mut used = 0.0;
        // The byte after the last space of the line being made, where it can break.
        let mut space: Option<usize> = None;
        let mut i = start;
        for c in line.chars() {
            let cw = char_w(c);
            if used + cw > width && i > a {
                let cut = space.filter(|&s| s > a).unwrap_or(i);
                out.push((a, cut));
                a = cut;
                used = text.get(a..i).unwrap_or("").chars().map(&mut *char_w).sum();
                space = None;
            }
            used += cw;
            i += c.len_utf8();
            if c == ' ' {
                space = Some(i);
            }
        }
        out.push((a, end));
        start = end + 1;
    }
    out
}

/// Whether the window says [`WHOLE`] above its button now: it is the window that says it, on
/// Write notes with the AI on and a picture in it.
pub fn shows_whole(s: &TaskScreen) -> bool {
    s.whole_note
        && s.ai_on
        && s.task == crate::task_ui::TaskKind::Notes
        && s.files
            .iter()
            .any(|f| f.kind == crate::task_ui::FileKind::Picture)
}

/// The band over the window's footer where [`WHOLE`] is said, and the list made shorter by it.
pub const WHOLE_H: f32 = 26.0;
