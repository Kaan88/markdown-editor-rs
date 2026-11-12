//! Pointer, wheel and keyboard input: hover, drags, clicks, selection and keys. Adapted from the
//! viewer's `bins/mdv-app/src/app/input.rs`; the text area's selection follows DESIGN.md section 8.

use super::*;
use crate::keys::{self, KeyName};
use crate::ui::{BarItem, Popup, SelectUnit, Tip};
use crate::view::offset_for_hthumb;
use mdedit_core::limits::TAB_WIDTH;
use mdedit_core::selection::{self, Motion, Unit};
use mdshared_ui::gfx::Face;
use mdshared_ui::scroll::{offset_for_thumb, scrollbar};
use mdshared_ui::textfield::EditKey;
use winit::event::{KeyEvent, MouseScrollDelta};

/// The text area's vertical and horizontal scrollbars (track, thumb) and its geometry.
type TextBars = (Option<(Rect, Rect)>, Option<(Rect, Rect)>, view::Geom);

fn unit_of(u: SelectUnit) -> Unit {
    match u {
        SelectUnit::Grapheme => Unit::Grapheme,
        SelectUnit::Word => Unit::Word,
        SelectUnit::Line => Unit::Line,
    }
}

impl App {
    // ---- pointer -----------------------------------------------------------------------------

    pub(super) fn pointer_moved(&mut self, x: f32, y: f32) {
        self.ui.mouse = (x, y);
        if let Some(drag) = self.ui.drag {
            self.drag_to(drag, x, y);
            self.redraw();
            return;
        }
        let edge = self.edge_at(x, y);
        let in_menu = self
            .ui
            .popup
            .as_ref()
            .is_some_and(|p| p.menu.contains(x, y));
        if let Some(p) = self.ui.popup.as_mut()
            && p.menu.pointer(x, y)
        {
            self.wants_frame = true;
        }
        let hit = if edge.is_some() || in_menu {
            None
        } else {
            self.hit_at(x, y)
        };
        if hit != self.ui.hover {
            self.ui.hover = hit;
            self.update_tip(hit);
            self.redraw();
        }
        if let Some(d) = self.ui.dialog.as_ref() {
            let row = overlays::dialog_row_at(d, x, y);
            if row != self.ui.dlg_hover_row {
                self.ui.dlg_hover_row = row;
                self.redraw();
            }
        }
        let cursor = match (edge, hit) {
            (Some(dir), _) => CursorIcon::from(dir),
            (None, Some(Hit::Text | Hit::Find)) => CursorIcon::Text,
            (None, Some(Hit::Splitter)) => CursorIcon::ColResize,
            _ => CursorIcon::Default,
        };
        self.set_cursor(cursor);
    }

    pub(super) fn update_tip(&mut self, hit: Option<Hit>) {
        if self.ui.tip_blocked.is_some() && self.ui.tip_blocked != hit {
            self.ui.tip_blocked = None;
        }
        let text = hit
            .filter(|h| Some(*h) != self.ui.tip_blocked)
            .and_then(|h| chrome::tooltip(h, &self.docs, self.maximized));
        self.ui.tip = match (hit, text) {
            (Some(h), Some(text)) => {
                // Moving between neighbouring buttons while a tip shows keeps tips immediate.
                let shown = self.ui.tip.as_ref().is_some_and(|t| t.shown);
                let anchor = self.hit_rect(h).unwrap_or_default();
                Some(Tip {
                    hit: h,
                    text,
                    anchor,
                    due: Instant::now() + if shown { Duration::ZERO } else { TIP_DELAY },
                    shown: false,
                })
            }
            _ => None,
        };
    }

    /// The buffer offset under `(x, y)`, clamped into the text area first.
    pub(super) fn text_offset(&mut self, x: f32, y: f32) -> Option<usize> {
        let region = self.areas.text;
        let i = self.docs.active()?;
        let doc = &mut self.docs.tabs[i];
        let area = view::text_area(doc, region);
        let x = x.clamp(area.x, area.right() - 1.0);
        let y = y.clamp(area.y, (area.bottom() - 1.0).max(area.y));
        self.view.hit(doc, region, self.scale, x, y)
    }

    pub(super) fn drag_to(&mut self, drag: Drag, x: f32, y: f32) {
        match drag {
            Drag::Tab {
                index,
                x: start,
                moving,
            } => {
                let moving = moving || (x - start).abs() > 5.0;
                let mut index = index;
                if moving {
                    let target =
                        self.hits[Region::Title.index()]
                            .iter()
                            .find_map(|(h, r)| match h {
                                Hit::Tab(j) if x >= r.x && x < r.right() => Some(*j),
                                _ => None,
                            });
                    if let Some(t) = target.filter(|&t| t != index)
                        && self.docs.move_tab(index, t)
                    {
                        index = t;
                    }
                }
                self.ui.drag = Some(Drag::Tab {
                    index,
                    x: start,
                    moving,
                });
            }
            Drag::Splitter { x: start, width } => {
                let (w, _) = self.logical();
                let max = PANE_MAX.min(w - crate::ui::PAGE_MIN).max(PANE_MIN);
                self.ui.panes.files_w = (width + (x - start)).clamp(PANE_MIN, max).round();
            }
            Drag::Thumb { target, grab } => self.thumb_to(target, x - grab, y - grab),
            Drag::Select { unit, origin } => {
                let Some(offset) = self.text_offset(x, y) else {
                    return;
                };
                if let Some(r) = self.docs.active_ready_mut() {
                    r.sel =
                        selection::drag_to(&r.buffer, origin.0..origin.1, unit_of(unit), offset);
                    view::reveal(r, r.sel.caret, false);
                }
                let area = self
                    .docs
                    .active_doc()
                    .map_or(self.areas.text, |d| view::text_area(d, self.areas.text));
                let outside = !area.contains(x, y);
                if outside && self.ui.autoscroll_due.is_none() {
                    self.ui.autoscroll_due = Some(Instant::now() + AUTOSCROLL_TICK);
                } else if !outside {
                    self.ui.autoscroll_due = None;
                }
            }
            Drag::FindSelect => {
                if let Some(frame) = self.hit_rect(Hit::Find) {
                    let (area, _) = chrome::find_parts(frame, !self.ui.find.is_empty());
                    let stops = self.stops();
                    self.ui.find.drag(x - area.x, &stops);
                    self.caret_reset();
                }
            }
        }
    }

    /// Scroll geometry of a list scroller: (view area, content, view, offset), logical px.
    pub(super) fn scroll_geom(&self, target: Scroller) -> Option<(Rect, f32, f32, f32)> {
        match target {
            Scroller::Files => {
                let list = panes::files_list(self.areas.files?);
                Some((
                    list,
                    self.ui.panes.files_content,
                    self.ui.panes.files_view,
                    self.ui.panes.files_scroll,
                ))
            }
            Scroller::Dialog => {
                let d = self.ui.dialog.as_ref()?;
                Some((d.list, d.content, d.view, d.scroll))
            }
            Scroller::Text | Scroller::TextH => None,
        }
    }

    pub(super) fn set_scroll(&mut self, target: Scroller, offset: f32) {
        match target {
            Scroller::Files => {
                let cur = self.ui.panes.files_scroll;
                self.ui.panes.scroll_by(offset - cur);
            }
            Scroller::Dialog => {
                if let Some(d) = self.ui.dialog.as_mut() {
                    d.scroll = offset.clamp(0.0, (d.content - d.view).max(0.0)).round();
                }
            }
            Scroller::Text | Scroller::TextH => {}
        }
    }

    /// The text's scroll geometry: (vertical track and thumb, horizontal track and thumb, lines,
    /// view lines, extent, view width).
    fn text_bars(&mut self) -> Option<TextBars> {
        let region = self.areas.text;
        let i = self.docs.active()?;
        let doc = &mut self.docs.tabs[i];
        let g = self.view.layout(doc, region, self.scale)?;
        let r = doc.ready()?;
        Some((g.vbar(r.top), g.hbar(r.left), g))
    }

    /// A thumb dragged so its start is at `thumb_x` (horizontal) or `thumb_y`.
    pub(super) fn thumb_to(&mut self, target: Scroller, thumb_x: f32, thumb_y: f32) {
        match target {
            Scroller::Text => {
                let Some((Some((track, thumb)), _, g)) = self.text_bars() else {
                    return;
                };
                let top =
                    offset_for_thumb(track, thumb.h, thumb_y, g.lines as f32, g.view_lines as f32);
                if let Some(r) = self.docs.active_ready_mut() {
                    r.top = metrics::clamp_top(top as f64, g.lines, g.view_lines);
                }
            }
            Scroller::TextH => {
                let Some((_, Some((track, thumb)), g)) = self.text_bars() else {
                    return;
                };
                let left = offset_for_hthumb(track, thumb.w, thumb_x, g.extent, g.view_w);
                if let Some(r) = self.docs.active_ready_mut() {
                    r.left = metrics::clamp_left(left as f64, g.extent, g.view_w);
                }
            }
            _ => {
                let Some((area, content, view, offset)) = self.scroll_geom(target) else {
                    return;
                };
                let Some((track, thumb)) = scrollbar(area, content, view, offset) else {
                    return;
                };
                let next = offset_for_thumb(track, thumb.h, thumb_y, content, view);
                self.set_scroll(target, next);
            }
        }
    }

    /// A press on a scrollbar: grabs the thumb, or pages towards the press.
    fn scrollbar_press(&mut self, target: Scroller, x: f32, y: f32) {
        match target {
            Scroller::Text => {
                let Some((Some((_, thumb)), _, g)) = self.text_bars() else {
                    return;
                };
                if y >= thumb.y && y < thumb.bottom() {
                    self.ui.drag = Some(Drag::Thumb {
                        target,
                        grab: y - thumb.y,
                    });
                } else if let Some(r) = self.docs.active_ready_mut() {
                    let step = (g.view_lines - 1.0).max(1.0) * if y < thumb.y { -1.0 } else { 1.0 };
                    r.top = metrics::clamp_top(r.top + step, g.lines, g.view_lines);
                }
            }
            Scroller::TextH => {
                let Some((_, Some((_, thumb)), g)) = self.text_bars() else {
                    return;
                };
                if x >= thumb.x && x < thumb.right() {
                    self.ui.drag = Some(Drag::Thumb {
                        target,
                        grab: x - thumb.x,
                    });
                } else if let Some(r) = self.docs.active_ready_mut() {
                    let step = g.view_w as f64 * 0.9 * if x < thumb.x { -1.0 } else { 1.0 };
                    r.left = metrics::clamp_left(r.left + step, g.extent, g.view_w);
                }
            }
            _ => {
                let Some((area, content, view, offset)) = self.scroll_geom(target) else {
                    return;
                };
                let Some((_, thumb)) = scrollbar(area, content, view, offset) else {
                    return;
                };
                if y >= thumb.y && y < thumb.bottom() {
                    self.ui.drag = Some(Drag::Thumb {
                        target,
                        grab: y - thumb.y,
                    });
                } else {
                    let step = view * 0.9 * if y < thumb.y { -1.0 } else { 1.0 };
                    self.set_scroll(target, offset + step);
                }
            }
        }
    }

    pub(super) fn left_press(&mut self) {
        let (x, y) = self.ui.mouse;
        self.ui.tip = None;
        self.ui.tip_blocked = self.ui.hover;
        if let Some(dir) = self.edge_at(x, y) {
            if let Some(w) = &self.window {
                let _ = w.drag_resize_window(dir);
            }
            return;
        }
        if let Some(p) = &self.ui.popup {
            if p.menu.contains(x, y) {
                return;
            }
            self.ui.popup = None;
            self.redraw();
            return;
        }
        let Some(hit) = self.hit_at(x, y) else { return };
        // Clicking elsewhere takes focus from the find box, except for its own buttons.
        if !matches!(
            hit,
            Hit::Find | Hit::FindClear | Hit::Bar(BarItem::FindNext | BarItem::FindPrev)
        ) {
            self.ui.find_focused = false;
        }
        let now = Instant::now();
        let clicks = match self.ui.last_press {
            Some((t, (px, py), n))
                if now - t < platform::double_click_time()
                    && (px - x).abs() <= CLICK_SLOP
                    && (py - y).abs() <= CLICK_SLOP =>
            {
                n % 3 + 1
            }
            _ => 1,
        };
        self.ui.last_press = Some((now, (x, y), clicks));
        self.redraw();
        match hit {
            Hit::Backdrop => self.close_dialog(),
            Hit::TitleDrag => {
                if clicks == 2 {
                    self.toggle_maximized();
                } else if let Some(w) = &self.window {
                    let _ = w.drag_window();
                }
            }
            Hit::Tab(i) => {
                self.docs.activate(i);
                self.ui.drag = Some(Drag::Tab {
                    index: i,
                    x,
                    moving: false,
                });
            }
            Hit::Splitter => {
                let width = self.areas.files.map_or(self.ui.panes.files_w, |r| r.w);
                self.ui.drag = Some(Drag::Splitter { x, width });
            }
            Hit::ScrollBar(target) => self.scrollbar_press(target, x, y),
            Hit::Text => {
                let Some(offset) = self.text_offset(x, y) else {
                    return;
                };
                let shift = self.mods.shift;
                let Some(r) = self.docs.active_ready_mut() else {
                    return;
                };
                let (unit, origin) = match clicks {
                    2 => (SelectUnit::Word, selection::word_at(&r.buffer, offset)),
                    3 => (SelectUnit::Line, selection::line_at(&r.buffer, offset)),
                    _ if shift => {
                        let a = r.sel.anchor;
                        (SelectUnit::Grapheme, a..a)
                    }
                    _ => (SelectUnit::Grapheme, offset..offset),
                };
                r.sel = if clicks == 1 && !shift {
                    mdedit_core::Selection::at(offset)
                } else if clicks == 1 {
                    selection::drag_to(&r.buffer, origin.clone(), Unit::Grapheme, offset)
                } else {
                    mdedit_core::Selection {
                        anchor: origin.start,
                        caret: origin.end,
                        goal_col: None,
                    }
                };
                self.ui.drag = Some(Drag::Select {
                    unit,
                    origin: (origin.start, origin.end),
                });
                self.caret_reset();
            }
            Hit::Find => {
                self.ui.find_focused = true;
                let Some(frame) = self.hit_rect(Hit::Find) else {
                    return;
                };
                let (area, _) = chrome::find_parts(frame, !self.ui.find.is_empty());
                let stops = self.stops();
                if clicks == 2 {
                    self.ui.find.select_word_at(x - area.x, &stops);
                } else if clicks == 3 {
                    self.ui.find.select_all();
                } else {
                    self.ui.find.press(x - area.x, &stops, self.mods.shift);
                    self.ui.drag = Some(Drag::FindSelect);
                }
                self.caret_reset();
            }
            Hit::DlgCard => {}
            h if h.is_button() => self.ui.pressed = Some(h),
            _ => {}
        }
    }

    pub(super) fn left_release(&mut self) {
        let (x, y) = self.ui.mouse;
        self.ui.autoscroll_due = None;
        if let Some(drag) = self.ui.drag.take() {
            if matches!(drag, Drag::Splitter { .. }) {
                self.session.files_w = Some(self.ui.panes.files_w.round() as u32);
            }
            self.redraw();
            return;
        }
        if let Some(p) = &self.ui.popup
            && p.menu.contains(x, y)
        {
            let act = p.menu.chosen();
            if let Some(act) = act {
                self.ui.popup = None;
                self.act(act);
            }
            self.redraw();
            return;
        }
        if let Some(pressed) = self.ui.pressed.take() {
            if self.hit_at(x, y) == Some(pressed) {
                self.activate(pressed);
            }
            self.redraw();
        }
    }

    pub(super) fn right_press(&mut self) {
        let (x, y) = self.ui.mouse;
        self.ui.tip = None;
        if self.ui.dialog.is_some() {
            return;
        }
        self.ui.popup = None;
        match self.hit_at(x, y) {
            Some(Hit::Tab(i) | Hit::TabClose(i)) => {
                self.ui.popup = Some(Popup {
                    kind: PopupKind::Tab(i),
                    menu: overlays::tab_menu(&self.docs, i, x, y + 2.0),
                });
            }
            Some(Hit::Text) => {
                // A right-click outside the selection moves the caret there first.
                if let Some(offset) = self.text_offset(x, y)
                    && let Some(r) = self.docs.active_ready_mut()
                {
                    let sel = r.sel.range();
                    if sel.is_empty() || offset < sel.start || offset > sel.end {
                        r.sel = mdedit_core::Selection::at(offset);
                    }
                }
                self.ui.find_focused = false;
                self.ui.popup = Some(Popup {
                    kind: PopupKind::Text,
                    menu: overlays::text_menu(&self.docs, x + 2.0, y + 2.0),
                });
            }
            Some(Hit::Find | Hit::FindClear) => {
                // The box takes the focus and keeps its selection, so Cut and Copy act on it.
                self.ui.find_focused = true;
                self.caret_reset();
                let find = &self.ui.find;
                self.ui.popup = Some(Popup {
                    kind: PopupKind::Find,
                    menu: overlays::find_menu(
                        !find.is_empty(),
                        find.has_selection(),
                        self.ui.match_case,
                        x + 2.0,
                        y + 2.0,
                    ),
                });
            }
            _ => {}
        }
        self.redraw();
    }

    pub(super) fn middle_press(&mut self) {
        let (x, y) = self.ui.mouse;
        if self.ui.dialog.is_some() || self.ui.popup.is_some() {
            return;
        }
        if let Some(Hit::Tab(i) | Hit::TabClose(i)) = self.hit_at(x, y) {
            self.docs.close(i, self.service.as_ref());
            self.redraw();
        }
    }

    pub(super) fn wheel(&mut self, delta: MouseScrollDelta) {
        let (x, y) = self.ui.mouse;
        if self.ui.popup.is_some() {
            self.ui.popup = None;
            self.redraw();
            return;
        }
        // Notches (positive = scroll up / left).
        let (nx, ny, pixels) = match delta {
            MouseScrollDelta::LineDelta(dx, dy) => (dx as f64, dy as f64, None),
            MouseScrollDelta::PixelDelta(p) => (0.0, 0.0, Some((p.x, p.y))),
        };
        if self.ui.dialog.is_some() {
            if let Some(d) = self.ui.dialog.as_ref() {
                let dy = match pixels {
                    Some((_, py)) => py as f32 / self.scale,
                    None => ny as f32 * PANE_WHEEL,
                };
                let next = d.scroll - dy;
                self.set_scroll(Scroller::Dialog, next);
            }
            self.redraw();
            return;
        }
        if self.areas.files.is_some_and(|r| r.contains(x, y)) {
            let dy = match pixels {
                Some((_, py)) => py as f32 / self.scale,
                None => ny as f32 * PANE_WHEEL,
            };
            self.ui.panes.scroll_by(-dy);
            self.redraw();
            return;
        }
        if !self.areas.text.contains(x, y) {
            return;
        }
        let line_h = self.view.metrics(self.scale).line_h as f64;
        let scale = self.scale as f64;
        let shift = self.mods.shift;
        let Some(r) = self.docs.active_ready_mut() else {
            return;
        };
        let adv = r.advance as f64;
        // (lines down, logical px right)
        let (down, right) = match pixels {
            Some((px, py)) => {
                self.wheel_rest.0 += -py / line_h;
                self.wheel_rest.1 += -px / scale;
                let d = std::mem::take(&mut self.wheel_rest);
                if shift {
                    (0.0, d.0 * line_h / scale + d.1)
                } else {
                    d
                }
            }
            None if shift && nx == 0.0 => (0.0, -ny * metrics::WHEEL_COLS * adv),
            None => (-ny * metrics::WHEEL_LINES, -nx * metrics::WHEEL_COLS * adv),
        };
        r.top += down;
        r.left = (r.left + right).max(0.0);
        // The layout pass of the next frame clamps both.
        if let Some(Drag::Select { .. }) = self.ui.drag {
            let drag = self.ui.drag.unwrap();
            self.drag_to(drag, x, y);
        }
        self.redraw();
    }

    pub(super) fn toggle_maximized(&mut self) {
        if let Some(w) = &self.window {
            w.set_maximized(!w.is_maximized());
        }
    }

    // ---- keyboard ----------------------------------------------------------------------------

    pub(super) fn stops(&mut self) -> Vec<f32> {
        let mut p = Painter::new(&mut self.screen, &mut self.gfx, self.scale);
        p.caret_stops(Face::Ui, 13.0, self.ui.find.text())
    }

    pub(super) fn caret_reset(&mut self) {
        self.ui.caret_on = true;
        self.ui.caret_due = platform::caret_blink_time().map(|d| Instant::now() + d);
    }

    pub(super) fn find_changed(&mut self) {
        let q = self.ui.find.text().to_string();
        let case = self.ui.match_case;
        self.docs.find_set(&q, case);
    }

    pub(super) fn key(&mut self, event: &KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }
        #[cfg(any(windows, target_os = "linux"))]
        let bare = {
            use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
            event.key_without_modifiers()
        };
        #[cfg(not(any(windows, target_os = "linux")))]
        let bare = event.logical_key.clone();
        let Some(key) = keys::key_of(&bare).or_else(|| keys::key_of(&event.logical_key)) else {
            return;
        };
        self.key_pressed(key, event.text.as_deref());
    }

    pub(super) fn key_pressed(&mut self, key: keys::Key, typed: Option<&str>) {
        let mods = self.mods;
        self.ui.tip = None;
        self.redraw();
        let name = match &key {
            keys::Key::Name(n) => Some(n.clone()),
            keys::Key::Insert => None,
        };

        // A menu takes the keyboard.
        if let Some(p) = self.ui.popup.as_mut() {
            match name {
                Some(KeyName::Up) => p.menu.step(false),
                Some(KeyName::Down | KeyName::Tab) => p.menu.step(true),
                Some(KeyName::Enter | KeyName::Space) => {
                    if let Some(act) = p.menu.chosen() {
                        self.ui.popup = None;
                        self.act(act);
                    }
                }
                Some(KeyName::Escape) => self.ui.popup = None,
                _ => {}
            }
            return;
        }
        // A dialog too.
        if let Some(d) = self.ui.dialog.as_ref() {
            let scroll = d.scroll;
            match name {
                Some(KeyName::Escape | KeyName::Enter) => self.close_dialog(),
                Some(KeyName::Up) => self.set_scroll(Scroller::Dialog, scroll - 40.0),
                Some(KeyName::Down) => self.set_scroll(Scroller::Dialog, scroll + 40.0),
                Some(KeyName::PageUp) => self.set_scroll(Scroller::Dialog, scroll - 300.0),
                Some(KeyName::PageDown) => self.set_scroll(Scroller::Dialog, scroll + 300.0),
                _ => {}
            }
            return;
        }
        if self.ui.find_focused
            && let Some(n) = &name
            && self.find_key(n, mods, typed)
        {
            return;
        }
        if let Some(id) = keys::command_for(&key, mods) {
            self.command(id);
            return;
        }
        let Some(name) = name else { return };
        self.text_key(&name, mods);
    }

    /// Caret keys for the text area.
    fn text_key(&mut self, name: &KeyName, mods: Mods) {
        let Some(r) = self.docs.active_ready_mut() else {
            return;
        };
        let page = ((r.view_lines.floor() as usize).saturating_sub(1)).max(1);
        let motion = match (name, mods.ctrl) {
            (KeyName::Left, false) => Motion::Left,
            (KeyName::Right, false) => Motion::Right,
            (KeyName::Left, true) => Motion::WordLeft,
            (KeyName::Right, true) => Motion::WordRight,
            (KeyName::Home, false) => Motion::Home,
            (KeyName::End, false) => Motion::End,
            (KeyName::Home, true) => Motion::DocStart,
            (KeyName::End, true) => Motion::DocEnd,
            (KeyName::Up, false) => Motion::Up(1),
            (KeyName::Down, false) => Motion::Down(1),
            (KeyName::PageUp, false) => Motion::Up(page),
            (KeyName::PageDown, false) => Motion::Down(page),
            (KeyName::Up, true) => {
                r.top -= 1.0;
                return;
            }
            (KeyName::Down, true) => {
                r.top += 1.0;
                return;
            }
            _ => return,
        };
        r.sel = selection::apply(&r.buffer, r.sel, motion, mods.shift, TAB_WIDTH);
        if matches!(motion, Motion::Up(n) | Motion::Down(n) if n > 1) {
            // Page keys move the view with the caret.
            let d = page as f64
                * if matches!(motion, Motion::Up(_)) {
                    -1.0
                } else {
                    1.0
                };
            r.top += d;
        }
        view::reveal(r, r.sel.caret, false);
        self.caret_reset();
    }

    /// Keys for the focused find box. True when the key was used.
    pub(super) fn find_key(&mut self, name: &KeyName, mods: Mods, typed: Option<&str>) -> bool {
        let f = &mut self.ui.find;
        let (word, extend) = (mods.ctrl, mods.shift);
        let changed = match (name, mods.ctrl) {
            (KeyName::Enter, _) => {
                self.docs.find_step(!mods.shift);
                return true;
            }
            (KeyName::Escape, _) => {
                // Back to the text; the query and its marks stay.
                self.ui.find_focused = false;
                self.caret_reset();
                return true;
            }
            (KeyName::Left, _) => f.key(EditKey::Left, word, extend),
            (KeyName::Right, _) => f.key(EditKey::Right, word, extend),
            (KeyName::Home, _) => f.key(EditKey::Home, false, extend),
            (KeyName::End, _) => f.key(EditKey::End, false, extend),
            (KeyName::Backspace, _) => f.key(EditKey::Backspace, word, false),
            (KeyName::Delete, _) => f.key(EditKey::Delete, word, false),
            (KeyName::Char('a'), true) if !mods.alt => {
                f.select_all();
                false
            }
            (KeyName::Char('c'), true) if !mods.alt => {
                if f.has_selection() {
                    platform::set_clipboard_text(f.selected_text());
                }
                false
            }
            (KeyName::Char('x'), true) if !mods.alt => {
                if f.has_selection() {
                    platform::set_clipboard_text(f.selected_text());
                }
                f.delete_selection()
            }
            (KeyName::Char('v'), true) if !mods.alt => match platform::clipboard_text() {
                Some(t) => f.insert(t.lines().next().unwrap_or("")),
                None => false,
            },
            (KeyName::Up | KeyName::Down | KeyName::PageUp | KeyName::PageDown, false) => {
                return false;
            }
            _ => {
                // Typed text; Ctrl alone is a shortcut, Ctrl+Alt is AltGr on Windows.
                let typed = typed.filter(|t| !t.chars().any(char::is_control));
                match typed {
                    Some(t) if !(mods.ctrl && !mods.alt) => f.insert(t),
                    _ => return false,
                }
            }
        };
        if changed {
            self.find_changed();
        }
        self.caret_reset();
        true
    }

    /// One auto-scroll step of a selection drag with the pointer outside the text.
    pub(super) fn autoscroll(&mut self) {
        let (x, y) = self.ui.mouse;
        let Some(doc) = self.docs.active_doc() else {
            return;
        };
        let area = view::text_area(doc, self.areas.text);
        let dy = if y < area.y {
            y - area.y
        } else if y >= area.bottom() {
            y - area.bottom() + 1.0
        } else {
            0.0
        };
        let dx = if x < area.x {
            x - area.x
        } else if x >= area.right() {
            x - area.right() + 1.0
        } else {
            0.0
        };
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        if let Some(r) = self.docs.active_ready_mut() {
            r.top += (dy.abs() / 20.0).clamp(0.25, 3.0) as f64 * dy.signum() as f64;
            r.left = (r.left + ((dx.abs() * 0.6).clamp(4.0, 60.0) * dx.signum()) as f64).max(0.0);
        }
        // Clamp through a layout pass, then extend the selection to the pointer.
        let region = self.areas.text;
        if let Some(i) = self.docs.active() {
            self.view.layout(&mut self.docs.tabs[i], region, self.scale);
        }
        if let Some(drag) = self.ui.drag {
            self.drag_to(drag, x, y);
        }
        self.wants_frame = true;
    }
}
