//! "What is sent" for Write notes as the window hosts it (`sent_card.rs` is its state,
//! `sent_card_draw.rs` its drawing): a card over the task window, which stays as it was under
//! it. While it is open it takes every key, pointer and wheel event the window would. It sends
//! nothing: "Open the text in a tab" does what "Show what is sent" did before the card, and
//! Close (Esc) goes back to the window.
//!
//! Here too is the first-time sentence of pictures plan 13, said by the first window ever of
//! Write notes with a picture and the AI on, and kept as said in the settings.

use super::*;
use crate::keys::KeyName;
use crate::sent_card::{CardHit, LINE_H, SentCard};
use crate::task_ui::{FileKind, TaskKind};

#[cfg(test)]
thread_local! {
    pub(super) static COPIED: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// Puts `text` on the clipboard; true when it is there.
#[cfg(test)]
fn copy_text(text: &str) -> bool {
    COPIED.with(|c| *c.borrow_mut() = text.to_string());
    true
}

#[cfg(not(test))]
fn copy_text(text: &str) -> bool {
    platform::set_clipboard_text(text)
}

impl App {
    /// The card is open over the task window.
    pub(super) fn sent_card_open(&self) -> bool {
        self.task.as_ref().is_some_and(|s| s.sent.is_some())
    }

    /// "Show what is sent" on Write notes: the card, over the window as it is.
    pub(super) fn sent_card_show(&mut self) {
        let headings = self.notes_headings();
        let Some(s) = self.task.as_mut() else { return };
        s.sent = Some(SentCard::of(s, &headings));
        s.options_open = false;
        s.add_open = false;
        s.dest_open = false;
        s.hover = None;
        s.pressed = None;
        s.tick += 1;
        self.set_cursor(CursorIcon::Default);
        self.redraw();
    }

    pub(super) fn sent_card_close(&mut self) {
        if let Some(s) = self.task.as_mut() {
            s.sent = None;
            s.tick += 1;
        }
        self.set_cursor(CursorIcon::Default);
        self.redraw();
    }

    /// The card's state, and a change of what it shows.
    fn sent_card_mut(&mut self) -> Option<&mut SentCard> {
        let s = self.task.as_mut()?;
        s.tick += 1;
        s.sent.as_mut()
    }

    fn sent_card_act(&mut self, hit: CardHit) {
        match hit {
            CardHit::Close => self.sent_card_close(),
            CardHit::OpenTab => {
                self.sent_card_close();
                self.task_sent_tab();
            }
            CardHit::Tab(k) => {
                if let Some(c) = self.sent_card_mut() {
                    c.show(k);
                }
            }
            CardHit::Prev | CardHit::Next => self.sent_card_step(hit == CardHit::Next),
            CardHit::Card | CardHit::Text | CardHit::Track => {}
        }
        self.redraw();
    }

    /// The next request's tab, or the one before; the first and the last stay where they are.
    fn sent_card_step(&mut self, next: bool) {
        if let Some(c) = self.sent_card_mut() {
            let k = if next {
                (c.tab + 1).min(c.tabs.len().saturating_sub(1))
            } else {
                c.tab.saturating_sub(1)
            };
            if k != c.tab {
                c.show(k);
            }
        }
        self.redraw();
    }

    pub(super) fn sent_card_key(&mut self, name: KeyName) {
        let mods = self.mods;
        match name {
            KeyName::Escape => return self.sent_card_close(),
            KeyName::Char('c') | KeyName::Char('C') if mods.ctrl && !mods.alt => {
                let Some(c) = self.task.as_ref().and_then(|s| s.sent.as_ref()) else {
                    return;
                };
                // Nothing selected: the whole request.
                let text = c
                    .selected_text()
                    .map(str::to_string)
                    .unwrap_or_else(|| c.text.clone());
                if text.is_empty() {
                    return;
                }
                let whole = c.selection().is_none();
                if copy_text(&text) {
                    self.docs.set_toast(if whole {
                        "The text of this request is copied."
                    } else {
                        "Copied."
                    });
                } else {
                    self.docs.set_toast("The clipboard could not be written.");
                }
            }
            KeyName::Char('a') | KeyName::Char('A') if mods.ctrl && !mods.alt => {
                if let Some(c) = self.sent_card_mut() {
                    c.select_all();
                }
            }
            KeyName::Tab if mods.ctrl => self.sent_card_step(!mods.shift),
            KeyName::PageDown if mods.ctrl => self.sent_card_step(true),
            KeyName::PageUp if mods.ctrl => self.sent_card_step(false),
            KeyName::Right => self.sent_card_step(true),
            KeyName::Left => self.sent_card_step(false),
            KeyName::Down | KeyName::Up | KeyName::PageDown | KeyName::PageUp => {
                if let Some(c) = self.sent_card_mut() {
                    let page = (c.body.h - 2.0 * LINE_H).max(LINE_H);
                    let dy = match name {
                        KeyName::Down => LINE_H * 3.0,
                        KeyName::Up => -LINE_H * 3.0,
                        KeyName::PageDown => page,
                        _ => -page,
                    };
                    c.scroll_by(dy);
                }
            }
            KeyName::Home | KeyName::End => {
                if let Some(c) = self.sent_card_mut() {
                    c.scroll = if name == KeyName::End {
                        c.max_scroll()
                    } else {
                        0.0
                    };
                }
            }
            _ => {}
        }
        self.redraw();
    }

    pub(super) fn sent_card_pointer(&mut self, x: f32, y: f32) {
        let Some(c) = self.task.as_ref().and_then(|s| s.sent.as_ref()) else {
            return;
        };
        if let Some(grab) = c.grab {
            if let Some((track, thumb)) =
                mdshared_ui::scroll::scrollbar(c.body, c.content, c.body.h, c.scroll)
            {
                let to = mdshared_ui::scroll::offset_for_thumb(
                    track,
                    thumb.h,
                    y - grab,
                    c.content,
                    c.body.h,
                );
                if let Some(c) = self.sent_card_mut() {
                    c.scroll = to.clamp(0.0, c.max_scroll());
                }
            }
            self.redraw();
            return;
        }
        if c.selecting {
            if let Some(c) = self.sent_card_mut() {
                // Past the body's edge the text scrolls under the pointer, a line at a time.
                if y > c.body.bottom() {
                    c.scroll_by(LINE_H);
                } else if y < c.body.y {
                    c.scroll_by(-LINE_H);
                }
                let at = c.offset_at(x, y);
                if let Some((anchor, _)) = c.sel {
                    c.sel = Some((anchor, at));
                }
            }
            self.redraw();
            return;
        }
        let hit = c.hit_at(x, y).filter(|h| *h != CardHit::Card);
        let hovered = c.hover;
        self.set_cursor(if hit == Some(CardHit::Text) {
            CursorIcon::Text
        } else {
            CursorIcon::Default
        });
        if hovered != hit {
            if let Some(c) = self.sent_card_mut() {
                c.hover = hit;
            }
            self.redraw();
        }
    }

    pub(super) fn sent_card_press(&mut self) {
        let (x, y) = self.ui.mouse;
        let shift = self.mods.shift;
        let Some(c) = self.sent_card_mut() else {
            return;
        };
        let hit = c.hit_at(x, y);
        c.pressed = None;
        match hit {
            Some(CardHit::Track) => {
                if let Some((_, thumb)) =
                    mdshared_ui::scroll::scrollbar(c.body, c.content, c.body.h, c.scroll)
                {
                    c.grab = Some(if thumb.contains(x, y) {
                        y - thumb.y
                    } else {
                        thumb.h / 2.0
                    });
                }
            }
            Some(CardHit::Text) => {
                let at = c.offset_at(x, y);
                let anchor = match c.sel {
                    Some((anchor, _)) if shift => anchor,
                    _ => at,
                };
                c.sel = Some((anchor, at));
                c.selecting = true;
            }
            Some(CardHit::Card) | None => {}
            Some(h) => c.pressed = Some(h),
        }
        let grabbed = c.grab.is_some();
        if grabbed {
            self.sent_card_pointer(x, y);
        }
        self.redraw();
    }

    pub(super) fn sent_card_release(&mut self) {
        let (x, y) = self.ui.mouse;
        let Some(c) = self.sent_card_mut() else {
            return;
        };
        c.grab = None;
        c.selecting = false;
        let Some(pressed) = c.pressed.take() else {
            self.redraw();
            return;
        };
        if c.hit_at(x, y) == Some(pressed) {
            self.sent_card_act(pressed);
        }
        self.redraw();
    }

    pub(super) fn sent_card_wheel(&mut self, dy: f32) {
        if let Some(c) = self.sent_card_mut() {
            c.scroll_by(dy);
        }
        self.redraw();
    }

    /// The first window ever of Write notes with a picture and the AI on says, above its
    /// button, that a picture is sent whole; the settings keep that it was said, so no later
    /// window says it again. This window goes on saying it while it is open.
    pub(super) fn sent_whole_once(&mut self) {
        if self.docs.prefs.ai_pictures_told {
            return;
        }
        let Some(s) = self.task.as_mut() else { return };
        let due = s.task == TaskKind::Notes
            && s.ai_on
            && s.files.iter().any(|f| f.kind == FileKind::Picture);
        if !due || s.whole_note {
            return;
        }
        s.whole_note = true;
        s.tick += 1;
        let mut p = self.docs.prefs.clone();
        p.ai_pictures_told = true;
        self.set_prefs(p);
        self.redraw();
    }
}
