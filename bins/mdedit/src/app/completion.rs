//! Word completion and snippets in the window. The list is made when it opens (the shortcut, or
//! the third typed letter when the setting is on): words of the active document around the
//! caret and of the other open documents, and the snippets for the file's type
//! (`mdedit_core::complete`, `mdedit_core::snippets`). It is a popup menu at the caret; typing
//! goes on in the text and narrows it, Up and Down move, Enter and Tab accept, Escape closes,
//! any other key closes it and does its own work. With several carets or a selection there is no
//! list. Nothing is read, built or watched until the first time the list is asked for.
//!
//! A snippet puts its first tab stop under the selection; Tab and Shift+Tab move between the
//! stops until the last one (`$0`) is reached. The stops are byte offsets that are shifted by
//! the change of the text's length at each Tab, so they hold while the stop in use is typed in;
//! a caret that has left the stop ends the run and Tab is a Tab again.

use super::*;
use crate::keys::KeyName;
use crate::ui::Popup;
use mdedit_core::Selection;
use mdedit_core::complete::{self as words, Cand, Kind};
use mdedit_core::limits::{
    COMPLETE_OTHER_BYTES, COMPLETE_ROWS, COMPLETE_SCAN_BYTES, SNIPPETS_FILE_BYTES,
};
use mdedit_core::snippets::{self, Snippet};
use mdshared_ui::menu::{Item, Menu};
use std::ops::Range;
use std::time::SystemTime;

/// Letters typed before the list opens by itself.
const AUTO_LETTERS: usize = 3;

pub const NOTHING_TO_COMPLETE: &str = "Nothing to complete here.";
pub const COMPLETE_ONE_CARET: &str = "Completion works with one caret.";

/// The snippets file as last read: when it changed, so it is read again only then.
struct Loaded {
    list: Vec<Snippet>,
    stamp: Option<(Option<SystemTime>, u64)>,
}

struct Open {
    doc: DocId,
    /// Where the word being completed starts.
    start: usize,
    /// Every candidate, and those left for the last prefix (what the rows are taken from).
    full: Vec<Cand>,
    pool: Vec<Cand>,
    prefix: String,
}

/// A snippet being filled in.
struct Run {
    doc: DocId,
    /// The stops in order of use, as offsets of the text at the time `len` was taken.
    stops: Vec<Range<usize>>,
    at: usize,
    len: usize,
}

#[derive(Default)]
pub(super) struct State {
    snippets: Option<Loaded>,
    open: Option<Open>,
    run: Option<Run>,
    /// An automatic list found nothing for this word (document, word start, letters): a longer
    /// prefix of the same word cannot find more, so the text is not read again.
    quiet: Option<(DocId, usize, String)>,
}

impl App {
    /// The snippets file: beside the settings file, or in the application's data folder.
    pub(super) fn snippets_path(&self) -> Option<PathBuf> {
        // A shot never reads the user's own files.
        if self.args.shot.is_some() && self.session_path.is_none() {
            return None;
        }
        let dir = match self.session_path.as_ref().and_then(|p| p.parent()) {
            Some(d) if !d.as_os_str().is_empty() => Some(d.to_path_buf()),
            _ => mdshared_app::settings::data_dir("mdedit"),
        };
        dir.map(|d| d.join("snippets.toml"))
    }

    /// Reads the snippets file when it is new or has changed; the built-in snippets stay unless
    /// the file has one with the same trigger for Markdown.
    fn load_snippets(&mut self) {
        let path = self.snippets_path();
        let stamp = path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| (m.modified().ok(), m.len()));
        if self
            .completion
            .snippets
            .as_ref()
            .is_some_and(|l| l.stamp == stamp)
        {
            return;
        }
        let mut list = Vec::new();
        if let (Some(path), Some(_)) = (path.as_ref(), stamp) {
            use std::io::Read;
            let mut text = Vec::new();
            if let Ok(f) = std::fs::File::open(path) {
                let _ = f.take(SNIPPETS_FILE_BYTES as u64).read_to_end(&mut text);
            }
            list = snippets::parse(&String::from_utf8_lossy(&text));
        }
        for b in snippets::builtin() {
            if !list
                .iter()
                .any(|u| u.trigger == b.trigger && u.applies("markdown"))
            {
                list.push(b);
            }
        }
        self.completion.snippets = Some(Loaded { list, stamp });
    }

    /// Creates the snippets file with its instructions when it is not there, and opens it.
    pub(super) fn open_snippets_file(&mut self) {
        let Some(path) = self.snippets_path() else {
            self.docs
                .set_toast("There is no folder for the snippets file.");
            return;
        };
        if !path.exists() {
            let made = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|_| std::fs::write(&path, snippets::TEMPLATE));
            if let Err(e) = made {
                self.docs
                    .set_toast(&format!("Could not create the snippets file: {e}"));
                return;
            }
        }
        self.open(&path);
    }

    /// `--menu complete-snippet`: takes the list's first row, as Enter does.
    pub(super) fn completion_accept_first(&mut self) {
        self.ui.popup = None;
        self.accept_completion(0);
    }

    pub(super) fn completion_showing(&self) -> bool {
        self.ui.popup_kind() == Some(PopupKind::Complete) && self.completion.open.is_some()
    }

    fn close_completion(&mut self) {
        if self.ui.popup_kind() == Some(PopupKind::Complete) {
            self.ui.popup = None;
        }
        self.completion.open = None;
        self.redraw();
    }

    /// Opens the list at the caret. `manual` is the shortcut or the command: it says when there
    /// is nothing to offer, and takes any prefix; the automatic one needs [`AUTO_LETTERS`].
    pub(super) fn open_completion(&mut self, manual: bool) {
        if !self.text_focused() || self.ui.popup.is_some() {
            return;
        }
        let Some(i) = self.docs.active() else { return };
        let (id, active) = (self.docs.tabs[i].id, i);
        let Some(r) = self.docs.tabs[i].ready() else {
            return;
        };
        if r.read_only || r.info.lossy {
            return;
        }
        if !r.view.extra.is_empty() {
            if manual {
                self.docs.set_toast(COMPLETE_ONE_CARET);
            }
            return;
        }
        if !r.view.sel.is_empty() {
            return;
        }
        let caret = r.view.sel.caret;
        let range = words::prefix_range(&r.buffer, caret);
        let mut prefix = String::new();
        r.buffer.text(range.clone(), &mut prefix);
        if !manual && prefix.chars().count() < AUTO_LETTERS {
            return;
        }
        if !manual
            && self.completion.quiet.as_ref().is_some_and(|(d, s, p)| {
                *d == id
                    && *s == range.start
                    && prefix.to_lowercase().starts_with(&p.to_lowercase())
            })
        {
            return;
        }
        let language = r.language.map_or("", |l| l.name()).to_owned();
        let mut found = words::Words::new();
        found.scan(&r.buffer, caret, COMPLETE_SCAN_BYTES);
        let mut left = COMPLETE_OTHER_BYTES;
        for (k, d) in self.docs.tabs.iter().enumerate() {
            if k == active || left == 0 {
                continue;
            }
            if let Some(o) = d.ready() {
                left -= found.scan(&o.buffer, 0, COMPLETE_SCAN_BYTES.min(left));
            }
        }
        let mut full = found.into_cands();
        self.load_snippets();
        if let Some(l) = &self.completion.snippets {
            for (k, s) in l.list.iter().enumerate() {
                if s.applies(&language) {
                    full.push(Cand {
                        text: s.trigger.clone(),
                        kind: Kind::Snippet(k),
                        weight: 0,
                    });
                }
            }
        }
        let pool = words::narrow(&prefix, full.clone());
        if pool.is_empty() {
            if manual {
                self.docs.set_toast(NOTHING_TO_COMPLETE);
            } else {
                self.completion.quiet = Some((id, range.start, prefix));
            }
            return;
        }
        self.completion.quiet = None;
        self.completion.open = Some(Open {
            doc: id,
            start: range.start,
            full,
            pool,
            prefix,
        });
        if !self.show_completion() {
            self.completion.open = None;
        }
    }

    /// Puts the rows of the open list in a popup at the word's start. False when the place is
    /// not on screen.
    fn show_completion(&mut self) -> bool {
        let Some(open) = self.completion.open.as_ref() else {
            return false;
        };
        let region = self.areas.text;
        let (view, scale) = (&mut self.view, self.scale);
        let start = open.start;
        let Some((x, y, lh)) = self
            .docs
            .with_pane(0, |doc| view.point_of(doc, region, scale, start))
            .flatten()
        else {
            return false;
        };
        let items: Vec<Item<Act>> = open
            .pool
            .iter()
            .take(COMPLETE_ROWS)
            .enumerate()
            .map(|(n, c)| {
                let item = Item::new(Act::Complete(n), c.text.clone());
                match c.kind {
                    Kind::Snippet(_) => item.hint("snippet"),
                    Kind::Word => item,
                }
            })
            .collect();
        let need = items.len() as f32 * 32.0 + 12.0;
        let h = self.logical().1;
        let below = h - (y + lh);
        // Its labels start where the word does.
        let mut menu = if below >= need || below >= y - self.areas.title.bottom() {
            Menu::new(items, x - 14.0, y + lh + 2.0, false)
        } else {
            Menu::new(items, x - 14.0, y - 2.0, false).above()
        };
        menu.hover = Some(0);
        self.ui.popup = Some(Popup::new(PopupKind::Complete, menu));
        self.ui.tip = None;
        self.redraw();
        true
    }

    /// The word before the caret changed (typed on, or a letter taken back): narrow the list,
    /// or close it.
    fn refresh_completion(&mut self) {
        let Some(open) = self.completion.open.as_mut() else {
            return;
        };
        let Some(r) = self
            .docs
            .active_doc()
            .filter(|d| d.id == open.doc)
            .and_then(|d| d.ready())
        else {
            self.close_completion();
            return;
        };
        let range = words::prefix_range(&r.buffer, r.view.sel.caret);
        let mut prefix = String::new();
        r.buffer.text(range.clone(), &mut prefix);
        if range.start != open.start || prefix.is_empty() || !r.view.sel.is_empty() {
            self.close_completion();
            return;
        }
        // A longer prefix of the same word narrows what is left; a shorter one starts again.
        let pool = if prefix
            .to_lowercase()
            .starts_with(&open.prefix.to_lowercase())
        {
            std::mem::take(&mut open.pool)
        } else {
            open.full.clone()
        };
        let pool = words::narrow(&prefix, pool);
        if pool.is_empty() {
            self.close_completion();
            return;
        }
        open.pool = pool;
        open.prefix = prefix;
        if !self.show_completion() {
            self.close_completion();
        }
    }

    /// A key while the list is up. False when the key was not for the list: it is closed and the
    /// key goes on to do its own work.
    pub(super) fn completion_key(
        &mut self,
        name: Option<KeyName>,
        mods: Mods,
        typed: Option<&str>,
    ) -> bool {
        let letters = typed
            .filter(|t| !t.is_empty() && t.chars().all(words::is_word_char))
            .filter(|_| mods.ctrl == mods.alt);
        match name {
            Some(KeyName::Up) if !mods.ctrl && !mods.alt => self.step_completion(false),
            Some(KeyName::Down) if !mods.ctrl && !mods.alt => self.step_completion(true),
            Some(KeyName::Enter | KeyName::Tab) if !mods.ctrl && !mods.alt && !mods.shift => {
                let chosen = self.ui.popup.as_ref().and_then(|p| p.menu.chosen());
                self.ui.popup = None;
                match chosen {
                    Some(Act::Complete(n)) => self.accept_completion(n),
                    _ => self.completion.open = None,
                }
                self.redraw();
            }
            Some(KeyName::Escape) => self.close_completion(),
            Some(KeyName::Backspace) if !mods.ctrl && !mods.alt => {
                self.edit(EditOp::Backspace);
                self.refresh_completion();
            }
            _ if letters.is_some() => {
                if let Some(t) = letters {
                    self.edit(EditOp::Insert(t));
                }
                self.refresh_completion();
            }
            _ => {
                self.close_completion();
                return false;
            }
        }
        true
    }

    fn step_completion(&mut self, forward: bool) {
        if let Some(p) = self.ui.popup.as_mut() {
            p.menu.step(forward);
        }
        self.redraw();
    }

    /// After a character typed into the text: with the setting on, the list may open.
    pub(super) fn completion_typed(&mut self, typed: &str) {
        if self.docs.prefs.complete_auto
            && !typed.is_empty()
            && typed.chars().all(words::is_word_char)
            && self.macros.recording.is_none()
        {
            self.open_completion(false);
        }
    }

    /// Row `n` was chosen.
    pub(super) fn accept_completion(&mut self, n: usize) {
        let Some(open) = self.completion.open.take() else {
            return;
        };
        let Some(cand) = open.pool.get(n).cloned() else {
            return;
        };
        if self.docs.active_doc().is_none_or(|d| d.id != open.doc) {
            return;
        }
        let Some(r) = self
            .docs
            .active_ready_mut()
            .filter(|r| r.view.extra.is_empty())
        else {
            return;
        };
        let caret = r.view.sel.caret;
        let start = open.start.min(caret);
        r.view.sel = Selection {
            anchor: start,
            caret,
            goal_col: None,
        };
        r.history.seal();
        match cand.kind {
            Kind::Word => {
                self.edit(EditOp::Insert(&cand.text));
            }
            Kind::Snippet(k) => self.insert_snippet(k, start, open.doc),
        }
        self.docs.seal();
    }

    /// Replaces the selection (the trigger) by snippet `k` and selects its first stop.
    fn insert_snippet(&mut self, k: usize, start: usize, doc: DocId) {
        let Some(snippet) = self
            .completion
            .snippets
            .as_ref()
            .and_then(|l| l.list.get(k))
            .cloned()
        else {
            return;
        };
        let Some(r) = self.docs.active_ready() else {
            return;
        };
        let line = r.buffer.line_range(r.buffer.line_of(start)).start;
        let mut before = String::new();
        r.buffer.text(line..start, &mut before);
        let indent: String = before
            .chars()
            .take_while(|c| matches!(c, ' ' | '\t'))
            .collect();
        let e = snippets::expand(
            &snippet.text,
            &indent,
            &r.assist.indent.text(),
            r.info.eol.dominant(),
        );
        if !self.edit(EditOp::Paste(&e.text)) {
            return;
        }
        let stops: Vec<Range<usize>> = e
            .stops
            .iter()
            .map(|s| start + s.start..start + s.end)
            .collect();
        self.select_stop(&stops[0]);
        self.completion.run = (stops.len() > 1).then(|| Run {
            doc,
            len: self.docs.active_ready().map_or(0, |r| r.buffer.len_bytes()),
            stops,
            at: 0,
        });
    }

    fn select_stop(&mut self, s: &Range<usize>) {
        if let Some(r) = self.docs.active_ready_mut() {
            r.view.sel = Selection {
                anchor: s.start,
                caret: s.end,
                goal_col: None,
            };
            view::reveal(r, s.end, false);
        }
        self.caret_reset();
        self.redraw();
    }

    /// Tab (or Shift+Tab with `back`) while a snippet is being filled in: to the next stop.
    /// False when there is no run, or the caret has left the stop (the run ends).
    pub(super) fn snippet_tab(&mut self, back: bool) -> bool {
        let Some(run) = self.completion.run.as_mut() else {
            return false;
        };
        let Some(r) = self
            .docs
            .active_doc()
            .filter(|d| d.id == run.doc)
            .and_then(|d| d.ready())
            .filter(|r| r.view.extra.is_empty())
        else {
            self.completion.run = None;
            return false;
        };
        let now = r.buffer.len_bytes();
        let delta = now as isize - run.len as isize;
        let cur = run.stops[run.at].clone();
        let end = cur.end as isize + delta;
        let sel = r.view.sel.range();
        if end < cur.start as isize || sel.start < cur.start || sel.end as isize > end {
            self.completion.run = None;
            return false;
        }
        let next = if back {
            run.at.checked_sub(1)
        } else {
            Some(run.at + 1)
        };
        let Some(next) = next.filter(|n| *n < run.stops.len()) else {
            self.completion.run = None;
            return false;
        };
        for (j, s) in run.stops.iter_mut().enumerate() {
            if j != run.at && s.start >= cur.end {
                *s = (s.start as isize + delta) as usize..(s.end as isize + delta) as usize;
            }
        }
        run.stops[run.at].end = end as usize;
        run.at = next;
        run.len = now;
        let to = run.stops[next].clone();
        if next == run.stops.len() - 1 {
            self.completion.run = None;
        }
        if let Some(r) = self.docs.active_ready_mut() {
            r.history.seal();
        }
        self.select_stop(&to);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::files_tests::{app, open_now, scratch};
    use super::*;
    use crate::keys::{Key, KeyName};

    fn press(a: &mut App, key: KeyName, typed: Option<&str>) {
        a.key_pressed(Key::Name(key), typed);
    }

    fn type_text(a: &mut App, text: &str) {
        for ch in text.chars() {
            let s = ch.to_string();
            let key = if ch == ' ' {
                KeyName::Space
            } else {
                KeyName::Char(ch)
            };
            press(a, key, Some(&s));
        }
    }

    fn ctrl_space(a: &mut App) {
        a.mods = Mods {
            ctrl: true,
            ..Mods::default()
        };
        press(a, KeyName::Space, None);
        a.mods = Mods::default();
    }

    /// An app with one file open, the caret at its end, and a settings place of its own so the
    /// snippets file is the test's.
    fn with_text(tag: &str, name: &str, text: &str) -> (App, PathBuf) {
        let dir = scratch(tag);
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        let mut a = app();
        a.session_path = Some(dir.join("settings.ini"));
        open_now(&mut a, &path);
        a.areas.text = mdshared_ui::gfx::Rect::new(0.0, 80.0, 1280.0, 600.0);
        a.areas.title = mdshared_ui::gfx::Rect::new(0.0, 0.0, 1280.0, 40.0);
        let r = a.docs.active_ready_mut().unwrap();
        let end = r.buffer.len_bytes();
        r.view.sel = Selection::at(end);
        (a, dir)
    }

    fn text(a: &App) -> String {
        let r = a.docs.active_ready().unwrap();
        let mut s = String::new();
        r.buffer.text(0..r.buffer.len_bytes(), &mut s);
        s
    }

    fn rows(a: &App) -> Vec<String> {
        a.ui.popup
            .as_ref()
            .map(|p| p.menu.items.iter().map(|i| i.label.clone()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn the_shortcut_lists_words_and_enter_takes_one() {
        let (mut a, _d) = with_text("cw1", "a.txt", "parser parameter\npar");
        assert!(!a.completion_showing());
        ctrl_space(&mut a);
        assert!(a.completion_showing());
        let r = rows(&a);
        assert_eq!(r.len(), 2, "{r:?}");
        assert!(r.contains(&"parser".to_string()) && r.contains(&"parameter".to_string()));
        press(&mut a, KeyName::Down, None);
        let second = a.ui.popup.as_ref().unwrap().menu.chosen();
        assert_eq!(second, Some(Act::Complete(1)));
        press(&mut a, KeyName::Enter, None);
        assert!(!a.completion_showing() && a.ui.popup.is_none());
        let t = text(&a);
        assert!(
            t.ends_with("\nparameter") || t.ends_with("\nparser"),
            "{t:?}"
        );
        assert!(!t.ends_with("parparser"));
        // Undo takes the whole word back to what was typed.
        a.command(crate::commands::CommandId::Undo);
        assert_eq!(text(&a), "parser parameter\npar");
    }

    #[test]
    fn typing_narrows_and_other_keys_close() {
        let (mut a, _d) = with_text("cw2", "a.txt", "parser parameter\npar");
        ctrl_space(&mut a);
        type_text(&mut a, "s");
        assert!(a.completion_showing());
        assert_eq!(rows(&a), ["parser"]);
        assert!(text(&a).ends_with("pars"));
        // Backspace widens it again.
        press(&mut a, KeyName::Backspace, None);
        assert_eq!(rows(&a).len(), 2);
        // A letter with no match closes it, and the letter stays typed.
        type_text(&mut a, "z");
        assert!(!a.completion_showing());
        assert!(text(&a).ends_with("parz"));
        // Escape closes it and leaves the text.
        let (mut a, _d) = with_text("cw2b", "a.txt", "parser\npar");
        ctrl_space(&mut a);
        press(&mut a, KeyName::Escape, None);
        assert!(!a.completion_showing() && a.ui.popup.is_none());
        assert!(text(&a).ends_with("\npar"));
        // A space ends the word: closed, and typed.
        ctrl_space(&mut a);
        type_text(&mut a, " ");
        assert!(!a.completion_showing());
        assert!(text(&a).ends_with("par "));
    }

    #[test]
    fn enter_is_a_plain_enter_when_no_list_is_open() {
        let (mut a, _d) = with_text("cw3", "a.txt", "parser\npar");
        press(&mut a, KeyName::Enter, None);
        assert_eq!(text(&a), "parser\npar\n");
        // And with the setting off, typing never opens a list.
        type_text(&mut a, "par");
        assert!(!a.completion_showing());
    }

    #[test]
    fn the_setting_opens_the_list_after_three_letters() {
        let (mut a, _d) = with_text("cw4", "a.txt", "parser parameter\n");
        a.docs.prefs.complete_auto = true;
        type_text(&mut a, "pa");
        assert!(!a.completion_showing());
        type_text(&mut a, "r");
        assert!(a.completion_showing());
        type_text(&mut a, "a");
        assert_eq!(rows(&a), ["parameter"]);
        press(&mut a, KeyName::Tab, None);
        assert!(text(&a).ends_with("\nparameter"));
        assert!(!a.completion_showing());
        // A word with no match: no list, and a longer one does not look again.
        type_text(&mut a, " zzq");
        assert!(!a.completion_showing());
        type_text(&mut a, "x");
        assert!(a.completion.quiet.is_some());
    }

    #[test]
    fn words_of_other_open_documents_are_offered() {
        let (mut a, d) = with_text("cw5", "a.txt", "wombat\nwom");
        let other = d.join("b.txt");
        std::fs::write(&other, "wombatically wonderful").unwrap();
        open_now(&mut a, &other);
        a.docs.activate(0);
        ctrl_space(&mut a);
        let r = rows(&a);
        assert!(r.contains(&"wombatically".to_string()), "{r:?}");
        assert!(r.contains(&"wombat".to_string()), "{r:?}");
        assert!(!r.contains(&"wonderful".to_string()), "no match: {r:?}");
    }

    #[test]
    fn nothing_to_offer_says_so_and_a_selection_or_several_carets_open_nothing() {
        let (mut a, _d) = with_text("cw6", "a.txt", "alpha\nzz");
        ctrl_space(&mut a);
        assert!(!a.completion_showing());
        assert_eq!(a.docs.toast(Instant::now()), Some(NOTHING_TO_COMPLETE));
        let (mut a, _d) = with_text("cw6b", "a.txt", "alpha alps\nal");
        a.docs.active_ready_mut().unwrap().view.sel = Selection {
            anchor: 0,
            caret: 3,
            goal_col: None,
        };
        ctrl_space(&mut a);
        assert!(!a.completion_showing());
        let r = a.docs.active_ready_mut().unwrap();
        r.view.sel = Selection::at(13);
        r.view.extra = vec![Selection::at(2)];
        ctrl_space(&mut a);
        assert!(!a.completion_showing());
        assert_eq!(a.docs.toast(Instant::now()), Some(COMPLETE_ONE_CARET));
    }

    #[test]
    fn a_click_on_a_row_takes_it() {
        let (mut a, _d) = with_text("cw7", "a.txt", "parser\npar");
        ctrl_space(&mut a);
        assert!(a.completion_showing());
        a.choose(Act::Complete(0), false);
        assert!(text(&a).ends_with("\nparser"));
    }

    fn md_app(tag: &str, text: &str) -> (App, PathBuf) {
        with_text(tag, "a.md", text)
    }

    #[test]
    fn a_markdown_snippet_expands_and_tab_walks_the_stops() {
        let (mut a, _d) = md_app("sn1", "intro\ntable");
        ctrl_space(&mut a);
        assert_eq!(rows(&a), ["table"]);
        assert_eq!(a.ui.popup.as_ref().unwrap().menu.items[0].hint, "snippet");
        press(&mut a, KeyName::Enter, None);
        let eol = a.docs.active_ready().unwrap().info.eol.dominant();
        let want =
            "intro\n| Column | Column |\n| --- | --- |\n| Cell | Cell |\n".replace('\n', eol);
        assert_eq!(text(&a), want);
        // The first stop is selected.
        let r = a.docs.active_ready().unwrap();
        let sel = r.view.sel.range();
        assert_eq!(&text(&a)[sel.clone()], "Column");
        // Typing replaces it; Tab goes to the second, whose default is selected.
        type_text(&mut a, "Name");
        press(&mut a, KeyName::Tab, None);
        let r = a.docs.active_ready().unwrap();
        assert_eq!(&text(&a)[r.view.sel.range()], "Column");
        assert!(text(&a).contains("| Name | Column |"));
        type_text(&mut a, "Longer name");
        press(&mut a, KeyName::Tab, None);
        press(&mut a, KeyName::Tab, None);
        let r = a.docs.active_ready().unwrap();
        assert_eq!(&text(&a)[r.view.sel.range()], "Cell");
        // Shift+Tab goes back.
        a.mods = Mods {
            shift: true,
            ..Mods::default()
        };
        press(&mut a, KeyName::Tab, None);
        a.mods = Mods::default();
        let r = a.docs.active_ready().unwrap();
        assert_eq!(&text(&a)[r.view.sel.range()], "Cell");
        assert!(text(&a).contains("| Name | Longer name |"));
        press(&mut a, KeyName::Tab, None);
        press(&mut a, KeyName::Tab, None);
        // The last stop ($0) ends the run: at the end of the text; Tab is a Tab again.
        let r = a.docs.active_ready().unwrap();
        assert!(r.view.sel.is_empty());
        assert!(a.completion.run.is_none());
        let before = text(&a).len();
        press(&mut a, KeyName::Tab, None);
        assert_eq!(text(&a).len(), before + 1, "a tab character");
    }

    #[test]
    fn a_snippet_is_not_offered_in_other_file_types() {
        let (mut a, _d) = with_text("sn2", "a.txt", "table");
        ctrl_space(&mut a);
        assert!(!a.completion_showing());
    }

    #[test]
    fn leaving_the_stop_ends_the_run() {
        let (mut a, _d) = md_app("sn3", "link");
        ctrl_space(&mut a);
        press(&mut a, KeyName::Enter, None);
        assert_eq!(text(&a), "[text](https://)");
        assert!(a.completion.run.is_some());
        // The caret goes elsewhere: Tab is an ordinary Tab.
        press(&mut a, KeyName::Home, None);
        press(&mut a, KeyName::Tab, None);
        assert!(a.completion.run.is_none());
        assert!(text(&a).starts_with('\t'));
    }

    #[test]
    fn snippets_keep_the_indent_of_their_line_and_the_files_eol() {
        let (mut a, _d) = md_app("sn4", "  - item\r\n  code");
        ctrl_space(&mut a);
        press(&mut a, KeyName::Enter, None);
        let t = text(&a);
        assert!(t.ends_with("  ```text\r\n  \r\n  ```"), "{t:?}");
    }

    #[test]
    fn a_snippets_file_adds_and_replaces() {
        let (mut a, d) = md_app("sn5", "todo link");
        std::fs::write(
            d.join("snippets.toml"),
            "[[snippet]]\ntrigger = \"todo\"\ntypes = [\"markdown\"]\ntext = \"- [ ] ${1:task}$0\"\n\
             [[snippet]]\ntrigger = \"link\"\ntext = \"<$1>\"\n",
        )
        .unwrap();
        // The caret after "todo".
        a.docs.active_ready_mut().unwrap().view.sel = Selection::at(4);
        ctrl_space(&mut a);
        assert_eq!(rows(&a), ["todo"]);
        press(&mut a, KeyName::Enter, None);
        assert!(text(&a).starts_with("- [ ] task link"));
        // The file's "link" replaces the built-in one.
        let n = text(&a).len();
        a.docs.active_ready_mut().unwrap().view.sel = Selection::at(n);
        ctrl_space(&mut a);
        press(&mut a, KeyName::Enter, None);
        assert!(text(&a).ends_with("<>"), "{:?}", text(&a));
    }

    #[test]
    fn the_snippets_file_command_makes_the_file_and_opens_it() {
        let (mut a, d) = md_app("sn6", "x");
        assert!(!d.join("snippets.toml").exists());
        let tabs = a.docs.tabs.len();
        a.command(crate::commands::CommandId::OpenSnippets);
        assert!(d.join("snippets.toml").exists());
        assert_eq!(a.docs.tabs.len(), tabs + 1);
        let on_disk = std::fs::read_to_string(d.join("snippets.toml")).unwrap();
        assert_eq!(on_disk, snippets::TEMPLATE);
        // A second time opens what is there and leaves it alone.
        std::fs::write(d.join("snippets.toml"), "# mine\n").unwrap();
        a.command(crate::commands::CommandId::OpenSnippets);
        assert_eq!(
            std::fs::read_to_string(d.join("snippets.toml")).unwrap(),
            "# mine\n"
        );
    }

    #[test]
    fn a_read_only_text_gets_no_list_and_a_macro_never_holds_it() {
        assert!(!crate::macros::recordable(
            crate::commands::CommandId::CompleteWord
        ));
        let (mut a, _d) = with_text("cw8", "a.txt", "parser\npar");
        a.docs.active_ready_mut().unwrap().read_only = true;
        ctrl_space(&mut a);
        assert!(!a.completion_showing());
    }

    #[test]
    fn both_shortcuts_are_bound() {
        use crate::commands::CommandId as C;
        let ctrl = Mods {
            ctrl: true,
            ..Mods::default()
        };
        let alt = Mods {
            alt: true,
            ..Mods::default()
        };
        assert_eq!(
            crate::keys::command_for(&Key::Name(KeyName::Space), ctrl),
            Some(C::CompleteWord)
        );
        assert_eq!(
            crate::keys::command_for(&Key::Name(KeyName::Char('/')), alt),
            Some(C::CompleteWord)
        );
    }
}
