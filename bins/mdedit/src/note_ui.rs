//! "A Markdown note from a file…" (`docs/ai-library-plan.md` 5.8, phase 5): the card that says
//! what is sent, shows the answer's changes and writes the note. This is its state, its events
//! and the numbers of its layout; `note_draw.rs` draws it and `app/note.rs` is the window's part.
//!
//! Nothing here reads or writes the disk, and nothing of the source file can be written: the note
//! is a new file in the group's folder, created exclusively, and the source is only read.

use crate::keys::Key;
use mdedit_core::ai::ReviewLine;
use mdshared_ui::gfx::Rect;
use mdshared_ui::keys::{KeyName, Mods};
use mdshared_ui::overlay::{DIALOG_FOOTER, dialog_card};
use mdshared_ui::rows::{Nav, Rows};
use mdshared_ui::scroll::offset_for_thumb;
use std::path::PathBuf;

// ---- the numbers of the layout ----------------------------------------------------------------------

pub const CARD_W: f32 = 880.0;
pub const CARD_H: f32 = 620.0;
pub const HEAD_H: f32 = 92.0;
/// A line of the changes, or of the note as it will be written.
pub const LINE_H: f32 = 20.0;

/// The card for a `w` x `h` window.
pub fn card(w: f32, h: f32) -> Rect {
    dialog_card(CARD_W, CARD_H, w, h)
}

/// Where the parts of the card go.
#[derive(Clone, Copy, Debug)]
pub struct Geo {
    pub card: Rect,
    pub header: Rect,
    pub body: Rect,
}

pub fn geo(card: Rect) -> Geo {
    let header = Rect::new(card.x, card.y, card.w, HEAD_H);
    let footer = Rect::new(card.x, card.bottom() - DIALOG_FOOTER, card.w, DIALOG_FOOTER);
    let body = Rect::new(
        card.x,
        header.bottom(),
        card.w,
        (footer.y - header.bottom() - 4.0).max(0.0),
    );
    Geo { card, header, body }
}

// ---- what the card is about --------------------------------------------------------------------------

/// The text file the note is made from, read once when the card opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub path: PathBuf,
    /// The file's name as the owner reads it.
    pub name: String,
    /// The decoded text, which is what a request sends.
    pub text: String,
    /// The bytes on disk: its length and digest are recorded, and compared again before writing.
    pub len: u64,
    pub sha256: String,
    /// Its dominant line ending, which the note uses.
    pub eol: String,
    /// "Windows-1254, written as UTF-8" when it is not UTF-8 already.
    pub encoding: Option<String>,
    /// Its lines, as the card says them.
    pub lines: usize,
}

/// Where the note goes: the group whose row opened the card, and the file it would be. The path is
/// worked out before the answer comes, so the card can say it while it asks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// The group's name, as its library file has it.
    pub group: String,
    /// The group's library file.
    pub file: PathBuf,
    /// The group's folder.
    pub home: PathBuf,
    /// The note's file: `stem.md`, or `stem 2.md` when that name is taken.
    pub path: PathBuf,
    /// The words of the line that lists the note.
    pub title: String,
}

/// What the body shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// The lines removed from the text and added to get the note, unchanged runs folded.
    Changes,
    /// The note as it will be written, with its line numbers.
    Result,
}

/// Where the card is in its work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Nothing is sent yet: the card says what would be.
    Ready,
    /// The request is out.
    Asking,
    /// The answer is in: the changes, or the note, and Write.
    Changes,
}

/// A request out: the engine's id, and how much of the answer came.
#[derive(Clone, Debug)]
pub struct AskState {
    pub id: u64,
    pub chars: usize,
    pub requests: u64,
}

/// What the pointer can be on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hit {
    Card,
    Body,
    Changes,
    Result,
    Thumb,
    Track,
    Cancel,
    /// "Make the note", or "Write the note" once there is one to write.
    Primary,
    /// "Stop" while the request is out.
    Stop,
}

/// What an event asks of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Nothing,
    Close,
    /// Make the note: the request goes out.
    Send,
    /// Write the note: the answer becomes a file.
    Write,
    /// Stop the request; the card stays, with the source as it was.
    Stop,
}

pub struct NoteCard {
    pub source: Source,
    pub target: Target,
    /// The request's text, made when the card opens: what "Make the note" sends.
    pub prompt: String,
    /// "Claude Code · Sonnet 5.5 · Default": who the text goes to.
    pub via: String,
    pub phase: Phase,
    pub asking: Option<AskState>,
    /// The note's text, made from the answer.
    pub result: String,
    pub lines: Vec<ReviewLine>,
    pub removed: usize,
    pub added: usize,
    pub view: View,
    pub scroll: f32,
    /// A line about the answer that is not a refusal of the write: "The AI sent back no text."
    pub notice: Option<String>,
    /// Why the note cannot be made or written, as the footer says it.
    pub blocked: Option<String>,
    pub hover: Option<Hit>,
    pub pressed: Option<Hit>,
    /// The scrollbar's thumb is held, this far below its top.
    pub drag: Option<f32>,
    /// Raised by every change of what the card shows: it is in the overlay's signature.
    pub tick: u64,
    pub win: (f32, f32),
    /// A card a shot draws: it sends nothing and writes nothing.
    pub demo: bool,
    /// Rectangles recorded as the card is drawn (buttons whose size is their text's).
    pub spots: Vec<(Hit, Rect)>,
}

impl NoteCard {
    pub fn new(
        source: Source,
        target: Target,
        prompt: String,
        via: String,
        blocked: Option<String>,
    ) -> NoteCard {
        NoteCard {
            source,
            target,
            prompt,
            via,
            phase: Phase::Ready,
            asking: None,
            result: String::new(),
            lines: Vec::new(),
            removed: 0,
            added: 0,
            view: View::Changes,
            scroll: 0.0,
            notice: None,
            blocked,
            hover: None,
            pressed: None,
            drag: None,
            tick: 0,
            win: (0.0, 0.0),
            demo: false,
            spots: Vec::new(),
        }
    }

    pub fn geo(&self) -> Geo {
        geo(card(self.win.0, self.win.1))
    }

    /// How many lines the body holds in the view it is in.
    pub fn count(&self) -> usize {
        match self.phase {
            Phase::Changes => match self.view {
                View::Changes => self.lines.len(),
                View::Result => self.result.lines().count(),
            },
            _ => 0,
        }
    }

    pub fn rows(&self) -> Rows {
        Rows {
            row_h: LINE_H,
            view: self.geo().body.h,
            count: self.count(),
            scroll: self.scroll,
        }
    }

    /// Why the primary button cannot be pressed, or `None` when it can.
    pub fn problem(&self) -> Option<String> {
        if let Some(why) = &self.blocked {
            return Some(why.clone());
        }
        match self.phase {
            Phase::Ready => None,
            Phase::Asking => Some("The request is out.".to_string()),
            Phase::Changes => {
                (self.result.is_empty()).then(|| "There is no note to write.".to_string())
            }
        }
    }

    /// The answer is in: its changes, its counts, and the note's text.
    pub fn set_answer(&mut self, text: String, eol: &str) {
        let result = mdedit_core::ai::note_body(&text, eol);
        let (lines, removed, added) = mdedit_core::ai::review(&self.source.text, &result);
        self.result = result;
        self.lines = lines;
        self.removed = removed;
        self.added = added;
        self.view = View::Changes;
        self.scroll = 0.0;
        self.phase = Phase::Changes;
        self.asking = None;
        self.notice = None;
        self.blocked = None;
        self.tick += 1;
    }

    /// The request is out.
    pub fn set_asking(&mut self, id: u64) {
        self.phase = Phase::Asking;
        self.asking = Some(AskState {
            id,
            chars: 0,
            requests: 1,
        });
        self.notice = None;
        self.tick += 1;
    }

    /// The request stopped or failed: the card goes back to what it shows before a request.
    pub fn set_stopped(&mut self, notice: Option<String>) {
        self.phase = if self.result.is_empty() {
            Phase::Ready
        } else {
            Phase::Changes
        };
        self.asking = None;
        self.notice = notice;
        self.tick += 1;
    }

    pub fn set_view(&mut self, view: View) {
        if self.view != view {
            self.view = view;
            self.scroll = 0.0;
            self.tick += 1;
        }
    }

    // ---- events ----

    pub fn resize(&mut self, w: f32, h: f32) {
        if self.win != (w, h) {
            self.win = (w, h);
            self.scroll = self.rows().clamp_scroll(self.scroll);
            self.tick += 1;
        }
    }

    pub fn hit_at(&self, x: f32, y: f32) -> Option<Hit> {
        let g = self.geo();
        if !g.card.contains(x, y) {
            return None;
        }
        if let Some((hit, _)) = self.spots.iter().rev().find(|(_, r)| r.contains(x, y)) {
            return Some(*hit);
        }
        if g.body.contains(x, y) {
            return Some(Hit::Body);
        }
        Some(Hit::Card)
    }

    pub fn pointer(&mut self, x: f32, y: f32) {
        if let Some(grab) = self.drag {
            let g = self.geo();
            let rows = self.rows();
            if let Some((track, thumb)) = rows.thumb(g.body) {
                let off = offset_for_thumb(track, thumb.h, y - grab, rows.content(), rows.view);
                self.scroll = rows.clamp_scroll(off);
                self.tick += 1;
            }
            return;
        }
        let hit = self.hit_at(x, y).filter(|h| *h != Hit::Card);
        if hit != self.hover {
            self.hover = hit;
            self.tick += 1;
        }
    }

    pub fn press(&mut self, x: f32, y: f32) {
        self.tick += 1;
        let hit = self.hit_at(x, y);
        self.pressed = hit;
        let g = self.geo();
        let rows = self.rows();
        match hit {
            Some(Hit::Thumb) => {
                if let Some((_, thumb)) = rows.thumb(g.body) {
                    self.drag = Some(y - thumb.y);
                }
            }
            Some(Hit::Track) => {
                if let Some((_, thumb)) = rows.thumb(g.body) {
                    let page = if y < thumb.y { -rows.view } else { rows.view };
                    self.scroll = rows.clamp_scroll(rows.scroll + page);
                }
            }
            _ => {}
        }
    }

    pub fn release(&mut self, x: f32, y: f32) -> Act {
        self.tick += 1;
        self.drag = None;
        let Some(pressed) = self.pressed.take() else {
            return Act::Nothing;
        };
        if self.hit_at(x, y) != Some(pressed) {
            return Act::Nothing;
        }
        match pressed {
            Hit::Cancel => Act::Close,
            Hit::Primary => {
                if self.problem().is_some() {
                    return Act::Nothing;
                }
                if self.phase == Phase::Changes {
                    Act::Write
                } else {
                    Act::Send
                }
            }
            Hit::Stop => Act::Stop,
            Hit::Changes => {
                self.set_view(View::Changes);
                Act::Nothing
            }
            Hit::Result => {
                self.set_view(View::Result);
                Act::Nothing
            }
            _ => Act::Nothing,
        }
    }

    pub fn wheel(&mut self, dy: f32) {
        self.tick += 1;
        let rows = self.rows();
        self.scroll = rows.clamp_scroll(self.scroll + dy);
    }

    pub fn key(&mut self, key: &Key, _mods: Mods) -> Act {
        self.tick += 1;
        let Key::Name(name) = key else {
            return Act::Nothing;
        };
        match name {
            // Esc stops a request and keeps the card; with nothing out it closes, and an answer
            // not yet written goes with it.
            KeyName::Escape => {
                if self.phase == Phase::Asking {
                    Act::Stop
                } else {
                    Act::Close
                }
            }
            KeyName::Enter => {
                if self.problem().is_some() {
                    return Act::Nothing;
                }
                if self.phase == Phase::Changes {
                    Act::Write
                } else if self.phase == Phase::Ready {
                    Act::Send
                } else {
                    Act::Nothing
                }
            }
            KeyName::Tab if self.phase == Phase::Changes => {
                let view = match self.view {
                    View::Changes => View::Result,
                    View::Result => View::Changes,
                };
                self.set_view(view);
                Act::Nothing
            }
            KeyName::Up => self.nav(Nav::Up),
            KeyName::Down => self.nav(Nav::Down),
            KeyName::Home => self.nav(Nav::Home),
            KeyName::End => self.nav(Nav::End),
            KeyName::PageUp => self.nav(Nav::PageUp),
            KeyName::PageDown => self.nav(Nav::PageDown),
            _ => Act::Nothing,
        }
    }

    /// Moves the list by a key, from the line at the top of the body.
    fn nav(&mut self, nav: Nav) -> Act {
        let rows = self.rows();
        let first = (self.scroll / LINE_H).round().max(0.0) as usize;
        if let Some(to) = rows.nav(Some(first), nav) {
            self.scroll = rows.clamp_scroll(to as f32 * LINE_H);
        }
        Act::Nothing
    }

    /// What the frame of the card depends on besides its tick.
    pub fn sig(&self) -> (u64, Option<Hit>, Option<Hit>) {
        (self.tick, self.hover, self.pressed)
    }
}

// ---- the shots ----------------------------------------------------------------------------------------

/// Every name `--note-demo` takes.
#[cfg(test)]
pub const NAMES: [&str; 5] = ["ready", "asking", "changes", "result", "blocked"];

/// The card the shots draw: a made-up text file, a made-up group, and an answer made in the
/// program. `None` for a name it does not know. Nothing here is read or written.
pub fn demo(name: &str) -> Option<NoteCard> {
    let (source_text, result) = demo_texts();
    let name_ok = name == "ready"
        || name == "asking"
        || name == "changes"
        || name == "result"
        || name == "blocked";
    if !name_ok {
        return None;
    }
    let source = Source {
        path: PathBuf::from(if cfg!(windows) {
            r"D:\Docs\pasta notes.txt"
        } else {
            "/home/me/Docs/pasta notes.txt"
        }),
        name: "pasta notes.txt".to_string(),
        len: source_text.len() as u64,
        sha256: String::new(),
        eol: "\n".to_string(),
        encoding: None,
        lines: source_text.lines().count(),
        text: source_text,
    };
    let home = PathBuf::from(if cfg!(windows) {
        r"C:\Notes\library\Recipes"
    } else {
        "/home/me/Notes/library/Recipes"
    });
    let target = Target {
        group: "Recipes".to_string(),
        file: home.join("../Recipes.md"),
        home: home.clone(),
        path: home.join("pasta-notes.md"),
        title: "pasta notes".to_string(),
    };
    let blocked = (name == "blocked")
        .then(|| "AI is off. Choose a connection in Settings \u{203A} AI first.".to_string());
    let prompt = mdedit_core::ai::prompt(mdedit_core::ai::FORMAT, &source.text);
    let via = "Claude Code \u{00B7} Sonnet 5.5".to_string();
    let mut card = NoteCard::new(source, target, prompt, via, blocked);
    card.demo = true;
    match name {
        "asking" => {
            card.set_asking(0);
            if let Some(ask) = card.asking.as_mut() {
                ask.chars = 1_482;
            }
        }
        "changes" | "result" => {
            card.set_answer(result, "\n");
            if name == "result" {
                card.set_view(View::Result);
            }
        }
        _ => {}
    }
    Some(card)
}

/// The made-up file and answer of the shots.
pub fn demo_texts() -> (String, String) {
    let source = "PASTA NOTES\n\n\
Serves four. 500g of spaghetti, 400g of tomatoes, garlic, olive oil.\n\n\
Put the water on. Salt it well, it should taste of the sea.\n\n\
Fry the garlic slowly, then the tomatoes, twenty minutes at least.\n\n\
Keep a cup of the water before you drain it: it brings the sauce together.\n\n\
Not tried yet: anchovies."
        .to_string();
    let result = "# Pasta notes\n\n\
Serves four. 500g of spaghetti, 400g of tomatoes, garlic, olive oil.\n\n\
- Put the water on.\n\
- Salt it well: it should taste of the sea.\n\n\
## The sauce\n\n\
Fry the garlic slowly, then the tomatoes, twenty minutes at least.\n\n\
Keep a cup of the water before you drain it: it brings the sauce together.\n\n\
> Not tried yet: anchovies.\n"
        .to_string();
    (source, result)
}
