//! Keyboard access to the two non-Notes faces of the left pane.

use super::*;
use crate::keys::KeyName;
use crate::ui::PaneFace;

impl App {
    pub(super) fn focus_pane(&mut self, face: PaneFace) -> bool {
        if face == PaneFace::Notes || !self.ui.panes.files || self.ui.pane_face() != face {
            return false;
        }
        let row = match face {
            PaneFace::Files => self.docs.active_doc().and_then(|doc| {
                let path = doc.path.as_deref()?;
                self.ui
                    .tree
                    .rows()
                    .iter()
                    .position(|row| mdshared_app::paths::same_path(&row.path, path))
            }),
            PaneFace::Functions => crate::outline::caret_row(&self.docs, &self.ui.outline),
            PaneFace::Notes => None,
        };
        self.ui.pane_focus.focus(face);
        self.ui.pane_focus.key_used();
        self.ui.pane_cursor = Some(row.unwrap_or(0));
        self.ui.find_focused = false;
        self.ui.results.set_focus(false);
        self.preview_blur();
        self.reveal_pane_cursor(face);
        self.redraw();
        true
    }

    pub(super) fn pane_key(&mut self, key: &KeyName, mods: Mods) -> bool {
        let Some(face) = self.ui.pane_focus.focused() else {
            return false;
        };
        if mods.ctrl || mods.control || mods.alt || self.ui.find_focused || self.ui.results.focused
        {
            return false;
        }
        let count = match face {
            PaneFace::Files => self.ui.tree.rows().len(),
            PaneFace::Functions => self.ui.outline.symbols.len(),
            PaneFace::Notes => return false,
        };
        let last = count.saturating_sub(1);
        let mut row = self.ui.pane_cursor.unwrap_or(0).min(last);
        let page = self.areas.files.map_or(1, |r| {
            let list = if face == PaneFace::Files {
                panes::files_list(r)
            } else {
                crate::outline::list(r)
            };
            (list.h / panes::ROW).floor().max(1.0) as usize
        });
        match key {
            KeyName::Escape => {
                self.ui.pane_focus.blur();
                self.caret_reset();
                return true;
            }
            KeyName::Up => row = row.saturating_sub(1),
            KeyName::Down => row = row.saturating_add(1).min(last),
            KeyName::Home => row = 0,
            KeyName::End => row = last,
            KeyName::PageUp => row = row.saturating_sub(page),
            KeyName::PageDown => row = row.saturating_add(page).min(last),
            KeyName::Left | KeyName::Right if face == PaneFace::Files => {
                if let Some(item) = self.ui.tree.rows().get(row) {
                    let right = *key == KeyName::Right;
                    if item.is_dir && right != item.expanded {
                        self.ui.tree.toggle(row);
                    } else if !right {
                        let depth = item.depth;
                        if let Some(parent) = self.ui.tree.rows()[..row]
                            .iter()
                            .rposition(|item| item.depth < depth)
                        {
                            row = parent;
                        }
                    } else if item.expanded
                        && row < last
                        && self.ui.tree.rows()[row + 1].depth > item.depth
                    {
                        row += 1;
                    }
                }
            }
            KeyName::Enter => {
                let hit = match face {
                    PaneFace::Files if count == 0 && self.ui.tree.root().is_none() => {
                        Some(Hit::FilesOpenFolder)
                    }
                    PaneFace::Files if count != 0 => Some(Hit::FilesRow(row)),
                    PaneFace::Functions if count != 0 => Some(Hit::OutlineRow(row)),
                    _ => None,
                };
                if let Some(hit) = hit {
                    if !matches!(hit, Hit::FilesRow(i) if self.ui.tree.rows()[i].is_dir) {
                        self.ui.pane_focus.blur();
                    }
                    self.activate(hit);
                    return true;
                }
            }
            // A list has no typing target; editor shortcuts are still routed normally.
            KeyName::Tab
            | KeyName::Char(_)
            | KeyName::Space
            | KeyName::Delete
            | KeyName::Backspace
            | KeyName::Left
            | KeyName::Right => {}
            _ => return false,
        }
        self.ui.pane_focus.key_used();
        self.ui.pane_cursor = Some(row);
        self.reveal_pane_cursor(face);
        true
    }

    fn reveal_pane_cursor(&mut self, face: PaneFace) {
        let Some((row, area)) = self.ui.pane_cursor.zip(self.areas.files) else {
            return;
        };
        let (scroll, height) = match face {
            PaneFace::Files => (self.ui.panes.files_scroll, panes::files_list(area).h),
            PaneFace::Functions => (self.ui.outline.scroll, crate::outline::list(area).h),
            PaneFace::Notes => return,
        };
        let y = row as f32 * panes::ROW;
        self.set_scroll(
            Scroller::Files,
            scroll.clamp((y + panes::ROW - height).max(0.0).min(y), y),
        );
    }
}
