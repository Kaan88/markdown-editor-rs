//! Write notes' plan card as the window hosts it (pictures plan step 5, phase V4). The card is
//! `notes_plan_ui.rs`, its drawing `notes_plan_draw.rs`. It opens over the task window when the
//! plan is ready and takes every event while it is shown. Each edit is applied to the run's
//! `arranged` at once, which is what "Write N notes" writes; Esc and Back keep the plan and bring
//! the window back (D-053), its button opening the card again; Discard throws the plan away.

use super::*;
use crate::keys::{Key, KeyName};
use crate::notes_plan_ui::{self as ui, Act, Bar, Hit, NotesPlan, RowMenu, Sel};
use mdshared_ui::menu::{Item, Menu};
use mdshared_ui::textfield::TextField;

impl App {
    /// The card is on screen over the task window.
    pub(super) fn notes_plan_open(&self) -> bool {
        self.task_is_open() && self.notes_plan.as_ref().is_some_and(|c| c.shown)
    }

    /// The card over the window, on the run's plan: the card kept while it is still of this plan
    /// (so the notes left out can be ticked again), a new one otherwise.
    pub(super) fn notes_plan_show(&mut self) {
        let Some(a) = self
            .task
            .as_ref()
            .and_then(|s| s.notes_run.as_ref())
            .and_then(|r| r.arranged.clone())
        else {
            return;
        };
        if !self.notes_plan.as_ref().is_some_and(|c| c.synced == a) {
            self.notes_plan = Some(NotesPlan::new(&a));
        }
        if let Some(c) = self.notes_plan.as_mut() {
            c.shown = true;
            c.hover = None;
            c.pressed = None;
            c.grab = None;
            c.tick += 1;
        }
        if let Some(s) = self.task.as_mut() {
            s.options_open = false;
            s.dest_open = false;
            s.add_open = false;
            s.hover = None;
            s.pressed = None;
            s.tick += 1;
        }
        self.set_cursor(CursorIcon::Default);
        self.caret_reset();
        self.redraw();
    }

    /// The card's edits are the plan: `run.arranged` is what it shows now.
    fn notes_plan_sync(&mut self) {
        let Some(c) = self.notes_plan.as_mut() else {
            return;
        };
        let a = c.arranged();
        c.synced = a.clone();
        c.tick += 1;
        if let Some(run) = self.task.as_mut().and_then(|s| s.notes_run.as_mut()) {
            run.arranged = Some(a);
        }
        self.redraw();
    }

    /// Esc and Back: the window comes back with the plan kept in the run (D-053); its button
    /// opens the card again as it was.
    pub(super) fn notes_plan_back(&mut self) {
        if let Some(c) = self.notes_plan.as_mut() {
            c.shown = false;
            c.menu = None;
            c.again = None;
            c.edit = None;
            c.grab = None;
        }
        if let Some(s) = self.task.as_mut() {
            s.tick += 1;
        }
        self.docs
            .set_toast("The plan is kept: Review the plan opens it again.");
        self.redraw();
    }

    /// Discard: the plan is thrown away. What the pictures were said to show is kept, so the
    /// window's Send asks for the notes again without sending a picture.
    pub(super) fn notes_plan_discard(&mut self) {
        self.notes_plan = None;
        if let Some(s) = self.task.as_mut() {
            if let Some(run) = s.notes_run.as_mut() {
                run.arranged = None;
                run.copy_names.clear();
            }
            s.tick += 1;
        }
        self.docs
            .set_toast("The plan is discarded. What was looked at is kept.");
        self.redraw();
    }

    pub(super) fn notes_plan_select(&mut self, sel: Sel) {
        let Some(c) = self.notes_plan.as_mut() else {
            return;
        };
        let before = c.shown_note();
        c.sel = Some(sel);
        let now = c.shown_note();
        if now != before {
            c.page_scroll = 0.0;
        }
        c.note = now;
        c.tick += 1;
        self.redraw();
    }

    /// The tick of the selected row, or of `sel`.
    fn notes_plan_toggle(&mut self, sel: &Sel) {
        let changed = self.notes_plan.as_mut().is_some_and(|c| c.toggle(sel));
        if changed {
            self.notes_plan_sync();
        }
    }

    fn notes_plan_leave_out(&mut self, sel: &Sel) {
        let changed = self.notes_plan.as_mut().is_some_and(|c| c.leave_out(sel));
        if changed {
            self.notes_plan_sync();
        }
    }

    /// "Put in…": the picture into another note.
    pub(super) fn notes_plan_put_in(&mut self, id: usize, key: u64) {
        let changed = self.notes_plan.as_mut().is_some_and(|c| c.put_in(id, key));
        if changed {
            self.notes_plan_sync();
        }
    }

    /// "Join with…": note `from` at the end of note `into`.
    pub(super) fn notes_plan_join(&mut self, into: u64, from: u64) {
        let changed = self.notes_plan.as_mut().is_some_and(|c| c.join(into, from));
        if changed {
            self.notes_plan_sync();
        }
    }

    /// F2: the selected row's name in a field, all of it selected. A picture's is its copy's
    /// name without the extension, which is kept.
    pub(super) fn notes_plan_rename(&mut self, sel: Sel) {
        let text = {
            let Some(c) = self.notes_plan.as_ref() else {
                return;
            };
            match &sel {
                Sel::Note(k) => c.entry(*k).map(|e| e.draft.title.clone()),
                Sel::Section(path) => path.last().cloned(),
                Sel::Picture(id) => self
                    .task
                    .as_ref()
                    .and_then(|s| Some((s, s.notes_run.as_ref()?)))
                    .map(|(s, run)| {
                        let name = ui::copy_name(s, run, *id);
                        match name.rfind('.').filter(|&i| i > 0) {
                            Some(i) => name[..i].to_string(),
                            None => name,
                        }
                    }),
                Sel::Group => None,
            }
        };
        let Some(text) = text else {
            return;
        };
        let mut field = TextField::new();
        field.set_text(&text);
        field.select_all();
        if let Some(c) = self.notes_plan.as_mut() {
            c.menu = None;
            c.edit = Some((sel, field));
            c.tick += 1;
        }
        self.caret_reset();
        self.redraw();
    }

    /// Enter on a name: the note's title, the section's name, or the picture's copy name. One
    /// that cannot be is said, and the field stays to be put right.
    pub(super) fn notes_plan_rename_commit(&mut self) {
        let Some((sel, field)) = self.notes_plan.as_mut().and_then(|c| c.edit.take()) else {
            return;
        };
        let text = field.text().to_string();
        let done = match &sel {
            Sel::Note(k) => self
                .notes_plan
                .as_mut()
                .map_or(Ok(()), |c| c.rename_note(*k, &text)),
            Sel::Section(path) => self
                .notes_plan
                .as_mut()
                .map_or(Ok(()), |c| c.rename_section(path, &text)),
            Sel::Picture(id) => {
                self.notes_plan_copy_name(*id, &text);
                Ok(())
            }
            Sel::Group => Ok(()),
        };
        match done {
            Ok(()) => {
                if let Some(c) = self.notes_plan.as_mut()
                    && let Sel::Section(_) = sel
                {
                    c.tick += 1;
                }
                self.notes_plan_sync();
            }
            Err(why) => {
                self.docs.set_toast(&why);
                if let Some(c) = self.notes_plan.as_mut() {
                    c.edit = Some((sel, field));
                    c.tick += 1;
                }
                self.redraw();
            }
        }
    }

    /// A picture's copy is named by the owner (`Run::copy_names`): the name typed, with the
    /// original's extension; nothing typed, or the name it had anyway, gives it its own back.
    fn notes_plan_copy_name(&mut self, id: usize, typed: &str) {
        let Some(s) = self.task.as_mut() else { return };
        let Some(path) = s.files.get(id).map(|f| f.path.clone()) else {
            return;
        };
        let Some(run) = s.notes_run.as_mut() else {
            return;
        };
        run.copy_names.remove(&id);
        let Some(name) = ui::owner_copy_name(typed, &path) else {
            return;
        };
        let Some(s) = self.task.as_ref() else { return };
        let Some(run) = s.notes_run.as_ref() else {
            return;
        };
        if ui::copy_name(s, run, id) == name {
            return;
        }
        if let Some(run) = self.task.as_mut().and_then(|s| s.notes_run.as_mut()) {
            run.copy_names.insert(id, name);
        }
    }

    /// "Arrange again…": its box, with the window's instruction in it.
    fn notes_plan_again_open(&mut self) {
        // What the last arrange was asked, or the window's instruction.
        let text = self
            .task
            .as_ref()
            .map(|s| {
                s.notes_run
                    .as_ref()
                    .and_then(|r| r.arrange_instruction.clone())
                    .unwrap_or_else(|| s.instruction.text().to_string())
            })
            .unwrap_or_default();
        let mut field = TextField::new();
        field.set_text(&text);
        field.select_all();
        if let Some(c) = self.notes_plan.as_mut() {
            c.again = Some(field);
            c.menu = None;
            c.edit = None;
            c.tick += 1;
        }
        self.caret_reset();
        self.redraw();
    }

    fn notes_plan_again_go(&mut self) {
        let Some(text) = self
            .notes_plan
            .as_ref()
            .and_then(|c| c.again.as_ref())
            .map(|f| f.text().trim().to_string())
            .filter(|t| !t.is_empty())
        else {
            return;
        };
        self.notes_arrange_again(&text);
    }

    /// The row menu of `sel` at `(x, y)`: Rename, "Put in…", "Join with…", Leave out.
    pub(super) fn notes_plan_menu(&mut self, sel: Sel, x: f32, y: f32) {
        let Some(c) = self.notes_plan.as_mut() else {
            return;
        };
        let others = |c: &NotesPlan, but: Option<u64>| {
            c.entries
                .iter()
                .filter(|e| e.on && Some(e.key) != but)
                .count()
        };
        let items: Vec<Item<Act>> = match &sel {
            Sel::Note(k) => {
                let on = c.entry(*k).is_some_and(|e| e.on);
                let mut items = vec![Item::new(Act::Rename, "Rename\u{2026}").hint("F2")];
                if on {
                    items.push(
                        Item::new(Act::JoinWith, "Join with\u{2026}")
                            .enabled(others(c, Some(*k)) > 0),
                    );
                    items.push(Item::new(Act::LeaveOut, "Leave out").hint("Delete"));
                } else {
                    items.push(Item::new(Act::Keep, "Keep it"));
                }
                items
            }
            Sel::Picture(id) => {
                let holder = c.holder(*id);
                let in_on = holder.is_some_and(|k| c.entry(k).is_some_and(|e| e.on));
                let mut items = vec![
                    Item::new(Act::PutIn, "Put in\u{2026}").enabled(others(c, holder) > 0),
                    Item::new(Act::Rename, "Rename the copy\u{2026}").hint("F2"),
                ];
                if in_on {
                    items.push(Item::new(Act::LeaveOut, "Leave out").hint("Delete"));
                } else if holder.is_none() {
                    items.push(Item::new(Act::Keep, "Put it back").enabled(others(c, None) > 0));
                }
                items
            }
            Sel::Section(_) => vec![Item::new(Act::Rename, "Rename\u{2026}").hint("F2")],
            Sel::Group => return,
        };
        c.edit = None;
        c.menu = Some(RowMenu {
            menu: Menu::new(items, x, y, false),
            sel,
            then: None,
        });
        c.tick += 1;
        self.redraw();
    }

    /// A line of the row menu chosen.
    pub(super) fn notes_plan_choose(&mut self, act: Act) {
        let Some(m) = self.notes_plan.as_mut().and_then(|c| c.menu.take()) else {
            return;
        };
        match act {
            Act::Rename => self.notes_plan_rename(m.sel),
            Act::LeaveOut => self.notes_plan_leave_out(&m.sel),
            Act::Keep => self.notes_plan_toggle(&m.sel),
            Act::PutIn | Act::JoinWith => {
                let Some(c) = self.notes_plan.as_mut() else {
                    return;
                };
                let but = match &m.sel {
                    Sel::Note(k) => Some(*k),
                    Sel::Picture(id) => c.holder(*id),
                    _ => None,
                };
                let items: Vec<Item<Act>> = c
                    .entries
                    .iter()
                    .filter(|e| e.on && Some(e.key) != but)
                    .map(|e| Item::new(Act::Into(e.key), e.draft.title.clone()))
                    .collect();
                if items.is_empty() {
                    return;
                }
                let at = m.menu.rect();
                c.menu = Some(RowMenu {
                    menu: Menu::new(items, at.x, at.y, false),
                    sel: m.sel,
                    then: Some(act),
                });
                c.tick += 1;
                self.redraw();
            }
            Act::Into(key) => match (m.then, &m.sel) {
                (Some(Act::PutIn), Sel::Picture(id)) => self.notes_plan_put_in(*id, key),
                (Some(Act::JoinWith), Sel::Note(from)) => self.notes_plan_join(*from, key),
                _ => {}
            },
        }
        if let Some(c) = self.notes_plan.as_mut() {
            c.tick += 1;
        }
        self.redraw();
    }

    /// A button or a row of the card.
    pub(super) fn notes_plan_act(&mut self, hit: Hit) {
        match hit {
            Hit::Row(i) | Hit::Tick(i) => {
                let Some(sel) = self
                    .notes_plan
                    .as_ref()
                    .and_then(|c| c.laid.get(i))
                    .and_then(|r| r.sel())
                else {
                    return;
                };
                self.notes_plan_select(sel.clone());
                if matches!(hit, Hit::Tick(_)) {
                    self.notes_plan_toggle(&sel);
                }
            }
            Hit::View(k) => {
                if let Some(c) = self.notes_plan.as_mut() {
                    c.markdown = k == 1;
                    c.page_scroll = 0.0;
                    c.tick += 1;
                }
                self.redraw();
            }
            Hit::Discard => self.notes_plan_discard(),
            Hit::Back => self.notes_plan_back(),
            Hit::Again => self.notes_plan_again_open(),
            Hit::AgainGo => self.notes_plan_again_go(),
            Hit::AgainClose => {
                if let Some(c) = self.notes_plan.as_mut() {
                    c.again = None;
                    c.tick += 1;
                }
                self.redraw();
            }
            Hit::Write => {
                let blocked = match (self.task.as_ref(), self.notes_plan.as_ref()) {
                    (Some(s), Some(c)) => ui::write_blocked(s, c),
                    _ => Some(String::new()),
                };
                match blocked {
                    None => self.notes_write(),
                    Some(why) if !why.is_empty() => self.docs.set_toast(&why),
                    Some(_) => {}
                }
            }
            Hit::Card | Hit::TreeTrack | Hit::PageTrack | Hit::AgainField => {}
        }
    }

    /// The card's keys: the menu's first, then a name being typed, then the box of "Arrange
    /// again…", then the tree's.
    pub(super) fn notes_plan_key(&mut self, key: Key, typed: Option<&str>) {
        let mods = self.mods;
        let name = match &key {
            Key::Name(n) => Some(n.clone()),
            Key::Menu => None,
            Key::Insert => return,
        };
        let Some(c) = self.notes_plan.as_mut() else {
            return;
        };
        c.tick += 1;
        if let Some(m) = c.menu.as_mut() {
            match name {
                Some(KeyName::Up) => m.menu.step(false),
                Some(KeyName::Down) => m.menu.step(true),
                Some(KeyName::Enter | KeyName::Space) => {
                    if let Some(act) = m.menu.chosen() {
                        self.notes_plan_choose(act);
                    }
                }
                _ => c.menu = None,
            }
            self.redraw();
            return;
        }
        if let Some((_, field)) = c.edit.as_mut() {
            match name {
                Some(KeyName::Escape) => c.edit = None,
                Some(KeyName::Enter) => self.notes_plan_rename_commit(),
                Some(n) => {
                    field.handle_key(&n, mods, typed, true);
                    self.caret_reset();
                }
                None => {}
            }
            self.redraw();
            return;
        }
        if let Some(field) = c.again.as_mut() {
            match name {
                Some(KeyName::Escape) => c.again = None,
                Some(KeyName::Enter) => self.notes_plan_again_go(),
                Some(n) => {
                    field.handle_key(&n, mods, typed, true);
                    self.caret_reset();
                }
                None => {}
            }
            self.redraw();
            return;
        }
        let sel = c.sel.clone();
        let menu_key = name.is_none() || (name == Some(KeyName::F(10)) && mods.shift);
        if menu_key {
            if let Some(sel) = sel {
                let at = self.notes_plan_row_at(&sel);
                self.notes_plan_menu(sel, at.0, at.1);
            }
            return;
        }
        let Some(name) = name else { return };
        match name {
            KeyName::Escape => self.notes_plan_back(),
            KeyName::Up | KeyName::Down | KeyName::Home | KeyName::End => {
                let Some(c) = self.notes_plan.as_ref() else {
                    return;
                };
                let all = c.selectable();
                let at = sel.as_ref().and_then(|s| all.iter().position(|x| x == s));
                let to = match (name, at) {
                    (KeyName::End, _) | (KeyName::Up, None) => all.len().saturating_sub(1),
                    (KeyName::Up, Some(i)) => i.saturating_sub(1),
                    (KeyName::Down, Some(i)) => (i + 1).min(all.len().saturating_sub(1)),
                    _ => 0,
                };
                if let Some(next) = all.get(to).cloned() {
                    self.notes_plan_select(next);
                    self.notes_plan_reveal();
                }
            }
            KeyName::Space => {
                if let Some(sel) = sel {
                    self.notes_plan_toggle(&sel);
                }
            }
            KeyName::Delete => {
                if let Some(sel) = sel {
                    self.notes_plan_leave_out(&sel);
                }
            }
            KeyName::F(2) => {
                if let Some(sel) = sel {
                    self.notes_plan_rename(sel);
                }
            }
            KeyName::PageUp | KeyName::PageDown => {
                if let Some(c) = self.notes_plan.as_mut() {
                    let dy = if name == KeyName::PageDown {
                        c.page_area.h * 0.8
                    } else {
                        -c.page_area.h * 0.8
                    };
                    c.scroll_by(Bar::Page, dy);
                }
            }
            _ => {}
        }
        self.redraw();
    }

    /// Where the selected row's menu opens: under the row, at its text.
    fn notes_plan_row_at(&self, sel: &Sel) -> (f32, f32) {
        let Some(c) = self.notes_plan.as_ref() else {
            return (0.0, 0.0);
        };
        c.laid
            .iter()
            .position(|r| r.sel().as_ref() == Some(sel))
            .and_then(|i| c.hits.iter().find(|(h, _)| *h == Hit::Row(i)))
            .map_or((c.tree_area.x + 60.0, c.tree_area.y + 40.0), |(_, r)| {
                (r.x + 60.0, r.bottom())
            })
    }

    /// The selected row is brought into the tree's view.
    fn notes_plan_reveal(&mut self) {
        let Some(c) = self.notes_plan.as_mut() else {
            return;
        };
        let rows = c.rows();
        let mut top = 8.0;
        for r in &rows {
            if r.sel() == c.sel {
                let bottom = top + r.height();
                if top < c.tree_scroll {
                    c.tree_scroll = top - 8.0;
                } else if bottom > c.tree_scroll + c.tree_area.h {
                    c.tree_scroll = bottom + 8.0 - c.tree_area.h;
                }
                c.tree_scroll = c.tree_scroll.clamp(0.0, c.max_scroll(Bar::Tree));
                break;
            }
            top += r.height();
        }
    }

    pub(super) fn notes_plan_pointer(&mut self, x: f32, y: f32) {
        let Some(c) = self.notes_plan.as_mut() else {
            return;
        };
        if let Some((bar, grab)) = c.grab {
            let (area, content, view) = match bar {
                Bar::Tree => (c.tree_area, c.tree_content, c.tree_area.h),
                Bar::Page => (c.page_area, c.page_content, c.page_area.h),
            };
            let offset = match bar {
                Bar::Tree => c.tree_scroll,
                Bar::Page => c.page_scroll,
            };
            if let Some((track, thumb)) =
                mdshared_ui::scroll::scrollbar(area, content, view, offset)
            {
                let to =
                    mdshared_ui::scroll::offset_for_thumb(track, thumb.h, y - grab, content, view);
                match bar {
                    Bar::Tree => c.tree_scroll = to,
                    Bar::Page => c.page_scroll = to,
                }
                c.tick += 1;
                self.redraw();
            }
            return;
        }
        if let Some(m) = c.menu.as_mut() {
            if m.menu.pointer(x, y) {
                c.tick += 1;
                self.redraw();
            }
            return;
        }
        let hit = c.hit_at(x, y).filter(|h| *h != Hit::Card);
        let text = hit == Some(Hit::AgainField);
        if c.hover != hit {
            c.hover = hit;
            c.tick += 1;
            self.redraw();
        }
        self.set_cursor(if text {
            CursorIcon::Text
        } else {
            CursorIcon::Default
        });
    }

    pub(super) fn notes_plan_press(&mut self) {
        let (x, y) = self.ui.mouse;
        let Some(c) = self.notes_plan.as_mut() else {
            return;
        };
        c.tick += 1;
        if let Some(m) = c.menu.as_ref() {
            if !m.menu.contains(x, y) {
                c.menu = None;
                self.redraw();
            }
            return;
        }
        let hit = c.hit_at(x, y);
        // A scrollbar: a press on the thumb takes it, one on the track brings it there.
        let bar = match hit {
            Some(Hit::TreeTrack) => Some((Bar::Tree, c.tree_area, c.tree_content, c.tree_scroll)),
            Some(Hit::PageTrack) => Some((Bar::Page, c.page_area, c.page_content, c.page_scroll)),
            _ => None,
        };
        if let Some((which, area, content, offset)) = bar
            && let Some((_, thumb)) = mdshared_ui::scroll::scrollbar(area, content, area.h, offset)
        {
            c.grab = Some((
                which,
                if thumb.contains(x, y) {
                    y - thumb.y
                } else {
                    thumb.h / 2.0
                },
            ));
            self.notes_plan_pointer(x, y);
            return;
        }
        // A name being typed is kept when the press is elsewhere.
        if c.edit.is_some() {
            self.notes_plan_rename_commit();
        }
        let Some(c) = self.notes_plan.as_mut() else {
            return;
        };
        c.pressed = hit.filter(|h| *h != Hit::Card);
        if let Some(h @ (Hit::Row(_) | Hit::Tick(_) | Hit::View(_))) = hit {
            c.pressed = None;
            self.notes_plan_act(h);
        }
        self.redraw();
    }

    pub(super) fn notes_plan_release(&mut self) {
        let (x, y) = self.ui.mouse;
        let Some(c) = self.notes_plan.as_mut() else {
            return;
        };
        c.grab = None;
        if let Some(m) = c.menu.as_ref() {
            if m.menu.contains(x, y)
                && let Some(act) = m.menu.chosen()
            {
                self.notes_plan_choose(act);
            }
            return;
        }
        let Some(pressed) = c.pressed.take() else {
            return;
        };
        c.tick += 1;
        if c.hit_at(x, y) == Some(pressed) {
            self.notes_plan_act(pressed);
        }
        self.redraw();
    }

    pub(super) fn notes_plan_right_press(&mut self) {
        let (x, y) = self.ui.mouse;
        let Some(c) = self.notes_plan.as_mut() else {
            return;
        };
        c.menu = None;
        let sel = match c.hit_at(x, y) {
            Some(Hit::Row(i) | Hit::Tick(i)) => c.laid.get(i).and_then(|r| r.sel()),
            _ => None,
        };
        if c.edit.is_some() {
            self.notes_plan_rename_commit();
        }
        match sel {
            Some(sel) => {
                self.notes_plan_select(sel.clone());
                self.notes_plan_menu(sel, x, y);
            }
            None => self.redraw(),
        }
    }

    pub(super) fn notes_plan_wheel(&mut self, dy: f32) {
        let (x, y) = self.ui.mouse;
        let Some(c) = self.notes_plan.as_mut() else {
            return;
        };
        if let Some(m) = c.menu.as_mut() {
            if m.menu.scroll_by(dy) {
                c.tick += 1;
                self.redraw();
            }
            return;
        }
        if c.tree_area.contains(x, y) {
            c.scroll_by(Bar::Tree, dy);
        } else if c.page_area.contains(x, y) {
            c.scroll_by(Bar::Page, dy);
        }
        self.redraw();
    }

    pub(super) fn notes_plan_preedit(
        &mut self,
        pre: Option<mdshared_ui::textfield::Preedit>,
    ) -> bool {
        let Some(c) = self.notes_plan.as_mut() else {
            return false;
        };
        c.tick += 1;
        if let Some((_, f)) = c.edit.as_mut() {
            return f.set_preedit(pre);
        }
        if let Some(f) = c.again.as_mut() {
            return f.set_preedit(pre);
        }
        false
    }
}
