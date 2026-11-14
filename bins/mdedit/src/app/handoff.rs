//! The hand-off with mdview: what the viewer asks of this window ("Edit in mdedit" opens a file at
//! a line), and what this window asks of the viewer ("Preview in mdview"). The protocol is in the
//! shared repository's `docs/SISTER.md`; `crate::link` keeps the viewer scrolled while a preview
//! lasts.

use super::*;
use crate::link;
use mdshared_app::sister::{self, Rect};
use mdshared_ui::place;

/// The right half of `area` for the viewer, once `set` has put this window on the left half;
/// `None` when the area is not known or `set` could not do it.
fn tiled(area: Option<Rect>, set: impl FnOnce(Rect) -> bool) -> Option<Rect> {
    let (left, right) = place::halves(area?);
    set(left).then_some(right)
}

impl App {
    /// What other launches and the viewer asked for since the last wake. A file is opened, shown
    /// at its line and column, and the window is raised, as a second launch always did. A live
    /// update (`follow`) only moves the caret in the file that is the active tab, and raises
    /// nothing. The caret goes where `--goto` puts it: in a same-file split, the pane with the
    /// keyboard.
    pub(super) fn handed_over(&mut self, requests: Vec<Request>) {
        for request in &requests {
            let (line, col) = (
                request.line.map(|l| l as usize),
                request.column.map_or(1, |c| c as usize),
            );
            if !request.follow {
                self.open(&request.path);
                self.ui.find_focused = false;
                if let Some(line) = line {
                    self.docs.go_to_or_wait(line, col);
                }
            } else if let Some(line) = line
                && self.docs.active_doc().is_some_and(|d| {
                    d.path.as_deref() == Some(canonical_or_absolute(&request.path).as_path())
                })
            {
                self.docs.go_to(line, col);
            }
        }
        if let Some(rect) = requests.iter().rev().find_map(|r| r.place) {
            self.place(rect);
        }
        let only_following = !requests.is_empty() && requests.iter().all(|r| r.follow);
        if !only_following && let Some(w) = &self.window {
            w.set_minimized(false);
            w.focus_window();
        }
        self.caret_reset();
    }

    /// Moves and resizes the window to `rect` (physical screen px), or says it cannot be done.
    pub(super) fn place(&mut self, rect: Rect) {
        if let Some(w) = &self.window
            && !place::set(w, rect)
        {
            self.docs.set_toast(strings::NO_PLACING);
        }
    }

    /// "Preview in mdview": shows the active file in the viewer at the caret's line and links the
    /// document, so the viewer follows the caret; or, for a linked document, ends the link.
    /// `side_by_side` also puts this window on the left half of its screen and the viewer's on the
    /// right, and always opens (the viewer comes to the file again).
    pub(super) fn preview(&mut self, side_by_side: bool) {
        let Some(doc) = self.docs.active_doc() else {
            return;
        };
        let (id, path) = (doc.id, doc.path.clone());
        let Some(path) = path else {
            self.docs.set_toast(strings::PREVIEW_NEEDS_FILE);
            return;
        };
        if !side_by_side && self.docs.links.contains(id) {
            self.docs.links.unlink(id);
            return;
        }
        let ready = self.docs.active_ready();
        let line = ready.map(|r| r.buffer.line_of(r.view.sel.caret) as u32 + 1);
        let too_large = ready.is_some_and(|r| r.buffer.len_bytes() > sister::MAX_TEXT);
        let mut request = Request {
            line,
            from: Some(link::FROM.into()),
            ..Request::new(&path)
        };
        if side_by_side {
            request.place = self.tile_left();
        }
        match sister::open(link::VIEWER, &request) {
            Ok(()) => {
                self.docs.links.link(id, path, line, None, Instant::now());
                self.docs.links.start(self.docs.waker());
                if too_large && self.docs.prefs.live_preview {
                    self.docs.set_toast(strings::PREVIEW_TOO_LARGE);
                }
            }
            Err(text) => self.docs.set_toast(&text),
        }
    }

    /// Puts this window on the left half of its screen and returns the right half; where the
    /// desktop does not let a window be placed, says so and returns nothing.
    fn tile_left(&mut self) -> Option<Rect> {
        let window = self.window.clone();
        let right = window
            .as_ref()
            .and_then(|w| tiled(place::work_area(w), |left| place::set(w, left)));
        if right.is_none() {
            self.docs.set_toast(strings::PREVIEW_TILING);
        }
        right
    }
}

#[cfg(test)]
mod tests {
    use super::super::files_tests::{app, open_now, scratch};
    use super::*;
    use crate::commands::CommandId as C;
    use mdedit_core::Position;

    #[test]
    fn the_halves_cover_the_screen_and_the_viewer_gets_the_right_one() {
        let area = Rect {
            x: -1920,
            y: 24,
            w: 1921,
            h: 1056,
        };
        let mut left = None;
        let right = tiled(Some(area), |r| {
            left = Some(r);
            true
        })
        .unwrap();
        let left = left.unwrap();
        // The odd pixel goes to the right; together they are the whole area, side by side.
        assert_eq!(left, Rect { w: 960, ..area });
        assert_eq!(
            right,
            Rect {
                x: -960,
                w: 961,
                ..area
            }
        );
        assert_eq!(left.x + left.w as i32, right.x);
        // A desktop that cannot place the window, or a window with no screen known: no right half.
        assert_eq!(tiled(Some(area), |_| false), None);
        assert_eq!(tiled(None, |_| true), None);
    }

    /// A file of 300 numbered lines, "line 1" to "line 300".
    fn file(dir: &Path, name: &str) -> PathBuf {
        let text: String = (1..=300).map(|n| format!("line {n}\n")).collect();
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    fn at(line: u32, col: u32) -> Request {
        Request {
            line: Some(line),
            column: Some(col),
            ..Request::default()
        }
    }

    fn caret(a: &App) -> Position {
        let r = a.docs.active_ready().unwrap();
        r.buffer.position(r.view.sel.caret)
    }

    /// The first and the last line the active document shows.
    fn shown(a: &App) -> (f64, f64) {
        let v = &a.docs.active_ready().unwrap().view;
        (v.top, v.top + v.view_lines)
    }

    #[test]
    fn a_request_opens_the_file_and_puts_the_caret_where_goto_does() {
        let dir = scratch("hand-open");
        let (a_md, b_md) = (file(&dir, "a.md"), file(&dir, "b.md"));
        let mut a = app();
        open_now(&mut a, &a_md);
        // Another file, line and column: the tab opens, the caret is there and in view.
        a.handed_over(vec![Request {
            path: b_md.clone(),
            ..at(150, 4)
        }]);
        assert_eq!(a.docs.tabs.len(), 2);
        a.docs
            .wait_load(Instant::now() + Duration::from_secs(5), None, false);
        let path = a.docs.active_doc().unwrap().path.clone().unwrap();
        assert_eq!(path, canonical_or_absolute(&b_md));
        assert_eq!(caret(&a), Position { line: 149, col: 3 });
        let (top, end) = shown(&a);
        assert!(top < 149.0 && 149.0 < end - 1.0, "{top} {end}");
        // The file that is open already is switched to; a line past the end goes to the last.
        a.handed_over(vec![Request {
            path: a_md.clone(),
            ..at(9999, 1)
        }]);
        assert_eq!(a.docs.tabs.len(), 2);
        assert_eq!(
            a.docs.active_doc().unwrap().path,
            Some(canonical_or_absolute(&a_md))
        );
        assert_eq!(caret(&a).line, 300);
        // A request without a line only opens.
        let before = caret(&a);
        a.handed_over(vec![Request::new(&b_md)]);
        assert_eq!(
            a.docs.active_doc().unwrap().path,
            Some(canonical_or_absolute(&b_md))
        );
        assert_eq!(caret(&a), Position { line: 149, col: 3 });
        assert_ne!(before, caret(&a));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_still_being_read_gets_the_caret_when_it_is() {
        let dir = scratch("hand-wait");
        let path = file(&dir, "a.md");
        let mut a = app();
        a.handed_over(vec![Request {
            path: path.clone(),
            ..at(250, 2)
        }]);
        // Nothing to move yet, or already read: either way the caret ends up there.
        a.docs
            .wait_load(Instant::now() + Duration::from_secs(5), None, false);
        assert_eq!(caret(&a), Position { line: 249, col: 1 });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_live_update_only_moves_the_caret_of_the_file_that_is_showing() {
        let dir = scratch("hand-follow");
        let (a_md, b_md) = (file(&dir, "a.md"), file(&dir, "b.md"));
        let mut a = app();
        open_now(&mut a, &a_md);
        let b = open_now(&mut a, &b_md);
        let follow = |path: &Path, line| Request {
            follow: true,
            path: path.to_path_buf(),
            ..at(line, 1)
        };
        a.handed_over(vec![follow(&b_md, 40)]);
        assert_eq!(caret(&a).line, 39);
        // Not the active tab: nothing moves and nothing switches. Not open: nothing opens.
        a.handed_over(vec![follow(&a_md, 90), follow(&dir.join("c.md"), 3)]);
        assert_eq!(caret(&a).line, 39);
        assert_eq!(a.docs.active(), Some(b));
        assert_eq!(a.docs.tabs.len(), 2);
        let first = a.docs.tabs[0].ready().unwrap();
        assert_eq!(first.view.sel.caret, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn in_a_same_file_split_the_pane_with_the_keyboard_moves() {
        let dir = scratch("hand-split");
        let path = file(&dir, "a.md");
        let mut a = app();
        open_now(&mut a, &path);
        a.command(C::ShowInOtherPane);
        let other = |a: &App| {
            a.docs
                .active_ready()
                .unwrap()
                .twin
                .as_ref()
                .unwrap()
                .sel
                .caret
        };
        a.handed_over(vec![Request {
            path: path.clone(),
            ..at(100, 1)
        }]);
        assert_eq!(caret(&a).line, 99);
        assert_eq!(other(&a), 0);
        // The keyboard goes to the other pane: that one moves now, the first stays.
        a.command(C::FocusOther);
        a.handed_over(vec![Request {
            path: path.clone(),
            ..at(200, 1)
        }]);
        assert_eq!(caret(&a).line, 199);
        let r = a.docs.active_ready().unwrap();
        assert_eq!(r.buffer.position(other(&a)).line, 99);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_request_arrives_over_the_instance_port() {
        let dir = scratch("hand-port");
        let path = file(&dir, "a.md");
        let settings = dir.join("settings.ini");
        let mut a = app();
        a.incoming = mdshared_app::single::listen_requests_waking("mdedit", &settings, || {});
        assert!(a.incoming.is_some());
        // What a second launch of `mdedit --goto 20:2 --from mdview a.md` sends.
        let launch = Args::parse(
            ["--goto", "20:2", "--from", "mdview", path.to_str().unwrap()].map(String::from),
        );
        let sent = launch
            .requests()
            .iter()
            .all(|r| mdshared_app::single::forward_request("mdedit", &settings, r));
        assert!(sent);
        // The viewer's own request, with a place.
        let rect = Rect {
            x: 0,
            y: 0,
            w: 800,
            h: 600,
        };
        let viewer = Request {
            line: Some(30),
            place: Some(rect),
            from: Some("mdview".into()),
            ..Request::new(&path)
        };
        assert!(mdshared_app::single::forward_request(
            "mdedit", &settings, &viewer
        ));
        // A live update does not open anything that is not open.
        let nothing = Request {
            follow: true,
            ..Request::new(dir.join("other.md"))
        };
        assert!(mdshared_app::single::forward_request(
            "mdedit", &settings, &nothing
        ));
        let received: Vec<Request> = a.incoming.as_ref().unwrap().try_iter().collect();
        assert_eq!(received.len(), 2, "the file that is not there is skipped");
        assert_eq!(received[0].line, Some(20));
        assert_eq!(received[0].column, Some(2));
        assert_eq!(received[0].from.as_deref(), Some("mdview"));
        assert_eq!(received[1].place, Some(rect));
        a.handed_over(received);
        a.docs
            .wait_load(Instant::now() + Duration::from_secs(5), None, false);
        // The last request wins; with no window there is nothing to place or raise.
        assert_eq!(caret(&a).line, 29);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_handed_to_the_running_window_becomes_its_folder() {
        let dir = scratch("hand-folder");
        let sub = dir.join("proj");
        std::fs::create_dir_all(&sub).unwrap();
        let settings = dir.join("settings.ini");
        let mut a = app();
        a.incoming = mdshared_app::single::listen_requests_waking("mdedit", &settings, || {});
        assert!(a.docs.folder.is_none() && !a.ui.panes.files);
        // `mdedit proj`, and `mdedit --folder proj` (which goes over as one more path).
        let launch = Args::parse(["proj"].map(|p| sub.join("..").join(p).display().to_string()));
        assert!(mdshared_app::single::forward(
            "mdedit",
            &settings,
            &launch.files
        ));
        let received: Vec<Request> = a.incoming.as_ref().unwrap().try_iter().collect();
        assert_eq!(received.len(), 1);
        a.handed_over(received);
        assert_eq!(a.docs.folder, Some(canonical_or_absolute(&sub)));
        assert!(a.ui.panes.files, "the Files pane shows it");
        assert!(a.docs.tabs.is_empty(), "and no tab is made of the folder");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A stand-in for mdview: a script that writes the arguments it was started with.
    #[cfg(unix)]
    fn stand_in(dir: &Path) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let (script, record) = (dir.join("mdview.sh"), dir.join("args.txt"));
        let text = format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done >> '{}'\n",
            record.display()
        );
        std::fs::write(&script, text).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        (script, record)
    }

    #[cfg(unix)]
    fn recorded(record: &Path, lines: usize) -> Vec<String> {
        for _ in 0..500 {
            let text = std::fs::read_to_string(record).unwrap_or_default();
            if text.lines().count() >= lines {
                return text.lines().map(String::from).collect();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        std::fs::read_to_string(record)
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }

    #[cfg(unix)]
    #[test]
    fn preview_starts_the_viewer_at_the_caret_and_links_the_document() {
        let dir = scratch("hand-preview");
        let path = file(&dir, "a.md");
        let (script, record) = stand_in(&dir);
        // SAFETY: no other test reads or writes these variables.
        unsafe {
            std::env::set_var("MD_SISTER_MDVIEW", &script);
            std::env::set_var("MD_SISTER_MDVIEW_SETTINGS", dir.join("none.ini"));
        }
        let mut a = app();
        let i = open_now(&mut a, &path);
        let id = a.docs.tabs[i].id;
        a.docs.go_to(77, 1);
        a.command(C::PreviewInViewer);
        let canonical = canonical_or_absolute(&path).display().to_string();
        assert_eq!(
            recorded(&record, 5),
            ["--line", "77", "--from", "mdedit", &canonical]
        );
        assert!(a.docs.links.contains(id));
        assert!(
            crate::status::fields(&a.docs)
                .1
                .contains(&"Previewing".to_string())
        );
        // The View menu lists it, checked; the tab menu too.
        let view = crate::overlays::view_menu(&a.docs, &a.ui, 0.0, 0.0, false);
        let item = view
            .items
            .iter()
            .find(|it| it.id == Act::Cmd(C::PreviewInViewer));
        assert!(item.is_some_and(|it| it.checked && it.enabled));
        // The same command again stops the link and starts nothing.
        a.command(C::PreviewInViewer);
        assert!(!a.docs.links.contains(id) && a.docs.links.is_empty());
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(recorded(&record, 5).len(), 5);
        // Side by side with no window to measure: the viewer is started without a place, and
        // the hint says what to do instead.
        a.command(C::PreviewSideBySide);
        assert_eq!(recorded(&record, 10).len(), 10);
        assert!(!recorded(&record, 10).contains(&"--place".to_string()));
        assert!(a.docs.links.contains(id));
        // A tab that closes ends its link at the next pass; with nothing linked a pass does
        // nothing and asks for no timer.
        a.docs.close(i, None);
        assert_eq!(a.docs.link_poll(Instant::now()), None);
        assert!(a.docs.links.is_empty());
        // A viewer that listens: the text follows the edits, the line alone follows the caret,
        // and ending the link tells the viewer to go back to the file.
        let viewer = dir.join("viewer.ini");
        let rx = mdshared_app::single::listen_requests_waking("mdview", &viewer, || {}).unwrap();
        // SAFETY: as above.
        unsafe { std::env::set_var("MD_SISTER_MDVIEW_SETTINGS", &viewer) };
        let i = open_now(&mut a, &path);
        let id = a.docs.tabs[i].id;
        a.command(C::PreviewInViewer);
        assert!(a.docs.links.contains(id));
        let t0 = Instant::now();
        let later = |s: u64| t0 + Duration::from_secs(s);
        let got = || rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let quiet = || rx.recv_timeout(Duration::from_millis(300)).is_err();
        // Nothing is sent for a document that was not touched.
        assert_eq!(a.docs.link_poll(later(1)), None);
        assert!(quiet());
        // Moving the caret in it sends the line alone: nothing is unsaved, so the viewer keeps
        // showing the file (no live text, no live dot).
        a.docs.go_to(5, 1);
        a.docs.link_poll(later(1) + Duration::from_millis(500));
        let r = got();
        assert_eq!((r.text, r.line, r.live_off), (None, Some(5), false));
        assert!(quiet());
        a.docs.go_to(1, 1);
        a.docs.link_poll(later(1) + Duration::from_millis(800));
        assert_eq!(got().line, Some(1));
        a.docs.edit(crate::docs::EditOp::Insert("EDITED "), None);
        a.docs.link_poll(later(2));
        let r = got();
        let now_text = a.docs.active_ready().unwrap().buffer.rope().to_string();
        assert!(r.follow && r.text.as_deref() == Some(now_text.as_str()));
        assert!(now_text.starts_with("EDITED line 1") && r.line == Some(1));
        // Looking again at the same text sends nothing; a caret move sends the line alone.
        a.docs.link_poll(later(3));
        assert!(quiet());
        a.docs.go_to(10, 1);
        a.docs.link_poll(later(4));
        let r = got();
        assert_eq!((r.text, r.line), (None, Some(10)));
        // The setting off ends the live text; on again, the next change brings it back.
        a.docs.prefs.live_preview = false;
        a.docs.link_poll(later(5));
        assert!(got().live_off);
        assert_eq!(got().line, Some(10));
        a.docs.prefs.live_preview = true;
        a.docs.edit(crate::docs::EditOp::Insert("AGAIN "), None);
        a.docs.link_poll(later(6));
        assert!(got().text.is_some_and(|t| t.contains("AGAIN line 10")));
        // Undoing back to the saved text ends the live text, once; the caret then moves with
        // the line alone.
        assert!(a.docs.edit(crate::docs::EditOp::Undo, None));
        assert!(a.docs.edit(crate::docs::EditOp::Undo, None));
        assert!(!a.docs.active_doc().unwrap().modified());
        a.docs.link_poll(later(7) - Duration::from_millis(500));
        assert!(got().live_off);
        assert!(got().follow);
        a.docs.link_poll(later(7));
        assert!(quiet());
        a.docs.go_to(12, 1);
        a.docs.link_poll(later(7) + Duration::from_millis(500));
        let r = got();
        assert_eq!((r.text, r.line, r.live_off), (None, Some(12), false));
        // A tab that closes, like the command, ends the live text (none is live now: nothing
        // is sent but the end of the link).
        a.docs.edit(crate::docs::EditOp::Insert("Z"), None);
        a.docs.link_poll(later(8));
        assert!(got().text.is_some());
        a.docs.close(i, None);
        a.docs.link_poll(later(9));
        assert!(got().live_off);
        assert!(a.docs.links.is_empty());
        // A document over the cap is previewed from the file: said once, and no text is sent.
        let big = dir.join("big.md");
        std::fs::write(&big, "word\n".repeat(sister::MAX_TEXT / 5 + 10)).unwrap();
        let i = open_now(&mut a, &big);
        a.command(C::PreviewInViewer);
        assert_eq!(
            a.docs.toast(Instant::now()),
            Some(strings::PREVIEW_TOO_LARGE)
        );
        a.docs.edit(crate::docs::EditOp::Insert("x"), None);
        a.docs.go_to(5, 1);
        a.docs.link_poll(later(8));
        let r = got();
        assert_eq!((r.text, r.line), (None, Some(5)));
        a.command(C::PreviewInViewer);
        assert!(quiet(), "nothing was live, so nothing to end");
        a.docs.close(i, None);
        // No viewer installed: the reason is the toast, and nothing is linked.
        let bare = dir.join("bare");
        std::fs::create_dir_all(&bare).unwrap();
        let (path_was, data_was) = (std::env::var_os("PATH"), std::env::var_os("XDG_DATA_HOME"));
        // SAFETY: as above.
        unsafe {
            std::env::remove_var("MD_SISTER_MDVIEW");
            std::env::set_var("PATH", &bare);
            std::env::set_var("XDG_DATA_HOME", &bare);
        }
        let i = open_now(&mut a, &path);
        a.command(C::PreviewInViewer);
        assert!(a.docs.links.is_empty());
        assert_eq!(
            a.docs.toast(Instant::now()),
            Some("mdview is not installed")
        );
        assert!(a.docs.tabs[i].path.is_some());
        // SAFETY: as above.
        unsafe {
            std::env::remove_var("MD_SISTER_MDVIEW_SETTINGS");
            if let Some(v) = path_was {
                std::env::set_var("PATH", v);
            }
            match data_was {
                Some(v) => std::env::set_var("XDG_DATA_HOME", v),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_preview_commands_need_a_file_and_are_listed_for_markdown() {
        let dir = scratch("hand-menus");
        let (md, rs) = (file(&dir, "a.md"), file(&dir, "b.rs"));
        let mut a = app();
        let (m, r) = (open_now(&mut a, &md), open_now(&mut a, &rs));
        let listed = |a: &App, i: usize| {
            crate::overlays::tab_menu(&a.docs, i, 0.0, 0.0)
                .items
                .iter()
                .any(|it| it.id == Act::TabCmd(C::PreviewInViewer, i))
        };
        // Markdown lists them; another language does not, though the commands work for it.
        assert!(listed(&a, m) && !listed(&a, r));
        assert!(crate::palette::enabled(
            C::PreviewInViewer,
            &a.docs,
            &a.ui,
            false
        ));
        // Linked, a file of any language lists it, so that it can be stopped.
        let id = a.docs.tabs[r].id;
        a.docs
            .links
            .link(id, rs.clone(), None, None, Instant::now());
        assert!(listed(&a, r));
        // A document that has no file cannot be previewed: the command says why.
        a.docs.new_untitled();
        assert!(!crate::palette::enabled(
            C::PreviewInViewer,
            &a.docs,
            &a.ui,
            false
        ));
        a.command(C::PreviewInViewer);
        assert_eq!(
            a.docs.toast(Instant::now()),
            Some(strings::PREVIEW_NEEDS_FILE)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_shortcut_is_free() {
        use crate::keys::{Key, KeyName, Mods};
        let both = Mods {
            ctrl: true,
            shift: true,
            ..Mods::default()
        };
        let key = Key::Name(KeyName::Char('v'));
        assert_eq!(
            crate::keys::command_for(&key, both),
            Some(C::PreviewInViewer)
        );
        let ctrl = Mods {
            ctrl: true,
            ..Mods::default()
        };
        assert_eq!(crate::keys::command_for(&key, ctrl), Some(C::Paste));
    }

    /// What one edit costs the UI thread while a live preview is on: the pass over the links
    /// (`link_poll`), which takes a snapshot of the rope (a clone that shares its nodes) and puts
    /// it in the courier's slot (a mutex and a condition variable). Not run with the others: it
    /// sets the sister's settings variable and prints microseconds.
    ///
    ///   cargo test --release -p mdedit live_send_cost -- --ignored --nocapture --test-threads=1
    #[test]
    #[ignore = "timing; prints the cost of the live preview's send path"]
    fn live_send_cost_on_the_ui_thread() {
        let dir = scratch("hand-cost");
        let path = dir.join("big.md");
        // About 200 KB in 8,400 lines.
        let text: String = (1..=8400)
            .map(|n| format!("line number {n:05} ....\n"))
            .collect();
        assert!(text.len() > 190_000);
        std::fs::write(&path, &text).unwrap();
        // A listener stands in for the viewer, so the sends are real (and nothing reaches a real
        // viewer). SAFETY: this test runs alone (ignored by default), nothing else reads the
        // variable.
        let viewer = dir.join("viewer.ini");
        let rx = mdshared_app::single::listen_requests_waking("mdview", &viewer, || {}).unwrap();
        unsafe { std::env::set_var("MD_SISTER_MDVIEW_SETTINGS", &viewer) };
        let mut a = app();
        let i = open_now(&mut a, &path);
        let id = a.docs.tabs[i].id;
        a.docs.prefs.live_preview = true;
        let t0 = Instant::now();
        let canonical = canonical_or_absolute(&path);
        a.docs.links.link(id, canonical, Some(1), None, t0);
        a.docs.links.start(a.docs.waker());
        let (mut edit, mut send, mut idle) = (vec![], vec![], vec![]);
        for k in 0..300u64 {
            let t = Instant::now();
            a.docs.edit(crate::docs::EditOp::Insert("x"), None);
            edit.push(t.elapsed());
            // 200 ms after the last pass: the rate limit has run out, the update is sent.
            let t = Instant::now();
            a.docs.link_poll(t0 + Duration::from_millis(200 * (k + 1)));
            send.push(t.elapsed());
            // Straight after: the same text again, nothing to send.
            let t = Instant::now();
            a.docs
                .link_poll(t0 + Duration::from_millis(200 * (k + 1) + 1));
            idle.push(t.elapsed());
        }
        let us = |v: &mut Vec<Duration>| {
            v.sort();
            (
                v[v.len() / 2].as_secs_f64() * 1e6,
                v[v.len() * 95 / 100].as_secs_f64() * 1e6,
                v[v.len() - 1].as_secs_f64() * 1e6,
            )
        };
        let len = a.docs.active_ready().unwrap().buffer.len_bytes();
        let (e, s, n) = (us(&mut edit), us(&mut send), us(&mut idle));
        eprintln!(
            "live preview, {len} bytes, 300 edits: edit alone p50 {:.1} us; link_poll with a send p50 {:.1} us, p95 {:.1} us, max {:.1} us; link_poll with nothing due p50 {:.2} us",
            e.0, s.0, s.1, s.2, n.0
        );
        // The sends were real: the viewer's stand-in got updates with the whole text (the courier
        // keeps only the newest of those that pile up, so fewer than 300 arrive).
        let mut got = (0, 0);
        while let Ok(r) = rx.recv_timeout(Duration::from_millis(500)) {
            got = (got.0 + 1, got.1.max(r.text.as_ref().map_or(0, |t| t.len())));
        }
        eprintln!(
            "the viewer's stand-in received {} updates, the longest text {} bytes",
            got.0, got.1
        );
        assert!(got.0 > 0 && got.1 >= len);
        a.docs.links.finish();
    }
}
