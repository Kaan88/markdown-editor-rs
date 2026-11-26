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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Key;
    use std::path::PathBuf;

    fn tree(tag: &str, files: usize) -> (App, PathBuf) {
        let dir = super::super::files_tests::scratch(tag);
        let folder = dir.join("a-folder");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("inside.txt"), "inside").unwrap();
        for i in 0..files {
            std::fs::write(dir.join(format!("file-{i:03}.txt")), "outside").unwrap();
        }
        let mut a = super::super::files_tests::app();
        a.ui.tree.set_root(Some(dir.clone()));
        a.ui.tree.refresh();
        shortcut(&mut a);
        a.compose();
        assert_eq!(a.ui.pane_focus.focused(), Some(PaneFace::Files));
        assert_eq!(a.ui.pane_cursor, Some(0));
        (a, dir)
    }

    fn key(a: &mut App, name: KeyName) {
        a.key_pressed(Key::Name(name), None);
    }

    fn shortcut(a: &mut App) {
        a.mods.ctrl = true;
        a.mods.shift = true;
        key(a, KeyName::Char('1'));
        a.mods = Mods::default();
    }

    #[test]
    fn enter_on_a_folder_keeps_focus_but_enter_on_its_file_opens_it_and_blurs() {
        let (mut a, dir) = tree("pane-enter", 1);
        let folder = &a.ui.tree.rows()[0];
        assert!(folder.is_dir && !folder.expanded);
        key(&mut a, KeyName::Enter);
        assert!(a.ui.tree.rows()[0].expanded, "Enter expands the folder");
        assert_eq!(
            a.ui.pane_focus.focused(),
            Some(PaneFace::Files),
            "folder keeps focus"
        );
        key(&mut a, KeyName::Down);
        assert_eq!(a.ui.pane_cursor, Some(1));
        let file = a.ui.tree.rows()[1].path.clone();
        key(&mut a, KeyName::Enter);
        assert_eq!(a.ui.pane_focus.focused(), None, "file gives up focus");
        assert_eq!(
            a.docs.active_doc().and_then(|doc| doc.path.as_ref()),
            Some(&file),
            "Enter opens the selected file"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn left_and_right_expand_enter_return_and_collapse_the_tree() {
        let (mut a, dir) = tree("pane-tree-keys", 1);
        key(&mut a, KeyName::Right);
        assert!(
            a.ui.tree.rows()[0].expanded,
            "Right expands a closed folder"
        );
        assert_eq!(a.ui.pane_cursor, Some(0));
        key(&mut a, KeyName::Right);
        assert_eq!(a.ui.pane_cursor, Some(1), "Right enters an expanded folder");
        key(&mut a, KeyName::Left);
        assert_eq!(
            a.ui.pane_cursor,
            Some(0),
            "Left from a file returns to its parent"
        );
        key(&mut a, KeyName::Left);
        assert!(
            !a.ui.tree.rows()[0].expanded,
            "Left collapses an open folder"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn home_end_and_page_keys_move_and_reveal_the_row() {
        let (mut a, dir) = tree("pane-page-keys", 99);
        let list = panes::files_list(a.areas.files.unwrap());
        let page = (list.h / panes::ROW).floor() as usize;
        let last = a.ui.tree.rows().len() - 1;
        assert!(page > 1 && page * 2 < last);
        for (name, at) in [
            (KeyName::PageDown, page),
            (KeyName::PageDown, 2 * page),
            (KeyName::PageUp, page),
            (KeyName::End, last),
            (KeyName::Home, 0),
        ] {
            let label = format!("{name:?}");
            key(&mut a, name);
            assert_eq!(a.ui.pane_cursor, Some(at), "{label}");
            a.compose();
            let row = a.hit_rect(Hit::FilesRow(at)).unwrap();
            assert!(
                row.y >= list.y && row.bottom() <= list.bottom(),
                "{label} reveals the row"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_click_in_the_editor_gives_up_pane_focus() {
        let (mut a, dir) = tree("pane-click", 1);
        let text = a.areas.text;
        a.pointer_moved(text.cx(), text.cy());
        a.left_press();
        a.left_release();
        assert_eq!(a.ui.pane_focus.focused(), None, "a click gives up focus");
        assert!(a.ui.panes.files);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn pressing_the_same_shortcut_again_closes_the_pane() {
        let (mut a, dir) = tree("pane-close", 1);
        shortcut(&mut a);
        a.compose();
        assert!(!a.ui.panes.files, "the second shortcut closes the pane");
        assert_eq!(a.ui.pane_focus.focused(), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn switching_face_gives_up_the_old_pane_focus() {
        let (mut a, dir) = tree("pane-switch", 1);
        // Activate the switch directly so the pointer press cannot blur first.
        a.activate(Hit::PaneFace(PaneFace::Functions));
        assert!(a.ui.panes.files);
        assert_eq!(a.ui.pane_face(), PaneFace::Functions);
        assert_eq!(
            a.ui.pane_focus.focused(),
            None,
            "switching face gives up focus"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
