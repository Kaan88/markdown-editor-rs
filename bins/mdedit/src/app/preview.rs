//! The preview pane in the app: opening and closing it, telling it what changed, and the wheel
//! over it. The pane itself is `crate::preview_pane`.

use super::*;
use crate::keys::KeyName;
use crate::preview_pane::Pane;
use std::time::Duration;

/// The wheel moves the page this many logical px a notch, as three lines of mdview's text.
const NOTCH: f32 = 60.0;

impl App {
    /// Looks for mdview and offers the previews in it (or takes them away) by what it finds;
    /// the pane too in the build that needs mdview for it. A pane that is open stays open.
    pub(super) fn look_for_viewer(&mut self) {
        let found = crate::preview_pane::viewer();
        let had = (self.docs.viewer, self.docs.pane);
        self.docs.viewer = found.is_some();
        self.docs.pane =
            crate::preview_pane::STANDALONE || found.is_some() || self.preview.is_some();
        self.viewer_path = Some(found);
        if had != (self.docs.viewer, self.docs.pane) {
            self.redraw();
        }
    }

    /// Opens the preview pane beside the text, or closes it. A split view gives way to it.
    pub(super) fn toggle_preview_pane(&mut self) {
        if self.preview.take().is_some() {
            self.docs.pane_open = false;
        } else {
            if self.viewer_path.is_none() {
                self.look_for_viewer();
            }
            let viewer = self.viewer_path.clone().flatten();
            if viewer.is_none() && !crate::preview_pane::STANDALONE {
                return;
            }
            self.docs.other = None;
            self.docs.same_file = false;
            self.preview = Some(Pane::open(viewer.as_deref(), self.docs.waker()));
            self.docs.pane_open = true;
            self.trace
                .stage(&format!("preview pane opened ({})", self.kind_of_preview()));
            self.preview_sync();
        }
        self.relayout_areas();
        self.full = true;
        self.redraw();
    }

    /// `--trace`: when the pane's first page arrived, and the memory then.
    pub(super) fn trace_preview_page(&mut self) {
        let Some(p) = self.preview.as_ref() else {
            return;
        };
        if p.generation == 1
            && let Some(at) = p.first_page
        {
            self.trace.line(&format!(
                "preview first page {:.1} ms after the pane opened ({})",
                at.as_secs_f64() * 1000.0,
                p.kind
            ));
            self.trace.memory("preview open");
        }
    }

    /// The timing runs start typing once the pane, when asked for, shows its first page.
    pub(super) fn preview_ready(&self) -> bool {
        self.preview
            .as_ref()
            .is_none_or(|p| p.first_page.is_some() || p.ended.is_some())
    }

    /// `--type-steps` with the pane: how long the pane took to show each new text.
    pub(super) fn preview_summary(&self) -> Option<String> {
        let p = self.preview.as_ref()?;
        let ms =
            |v: &[Duration]| -> Vec<f64> { v.iter().map(|d| d.as_secs_f64() * 1000.0).collect() };
        let (r50, r95, rmax) = crate::trace::summary(&ms(&p.round_trips));
        let (s50, s95, smax) = crate::trace::summary(&ms(&p.server_times));
        Some(format!(
            "preview ({}): {} answers, request to pane pixels p50 {r50:.2} ms, p95 {r95:.2} ms, max {rmax:.2} ms; drawing {} frames p50 {s50:.2} ms, p95 {s95:.2} ms, max {smax:.2} ms",
            p.kind,
            p.round_trips.len(),
            p.server_times.len(),
        ))
    }

    fn kind_of_preview(&self) -> &'static str {
        self.preview.as_ref().map_or("none", |p| p.kind)
    }

    /// Hands the pane what changed (its size and theme, the active document's text, the caret's
    /// line) and takes its answers in. True when the pane has something new to show.
    pub(super) fn preview_sync(&mut self) -> bool {
        let dark = self.dark();
        let scale = self.scale;
        let rect = self.areas.preview;
        let visual = self.visual_on();
        let Some(pane) = self.preview.as_mut() else {
            return false;
        };
        let changed = pane.poll();
        if let Some(r) = rect {
            let w = (r.w * scale).round().max(1.0) as u32;
            let h = (r.h * scale).round().max(1.0) as u32;
            // The line at the pane's left edge is the editor's, not the page's.
            pane.set_view(w.saturating_sub(1).max(1), h, scale, dark);
        }
        if let Some(doc) = self.docs.active_doc()
            && let Some(r) = doc.ready()
            && previewable(doc)
        {
            let version = (doc.revision, r.buffer.revision());
            let base = doc
                .path
                .as_deref()
                .and_then(|p| p.parent())
                .map(Path::to_path_buf);
            pane.set_text(doc.id, version, base, || {
                let mut s = String::new();
                r.buffer.text(0..r.buffer.len_bytes(), &mut s);
                s
            });
            let caret = r.view.sel.caret;
            let line = r.buffer.line_of(caret);
            let column = caret - r.buffer.line_range(line).start;
            // In visual editing the page is scrolled by the reader only.
            if !visual {
                pane.follow(line as u32 + 1, column as u32);
            }
        }
        self.preview_copied();
        self.preview_task();
        // The page's answer about the cursor came after the pointer moved: show it now.
        let (mx, my) = self.ui.mouse;
        if self.ui.drag.is_none()
            && self.ui.popup.is_none()
            && self.page_area().is_some_and(|r| r.contains(mx, my))
            && let Some(p) = self.preview.as_ref()
        {
            let cursor = page_cursor(p.pointer);
            self.set_cursor(cursor);
        }
        self.ui.page_selected = self
            .preview
            .as_ref()
            .is_some_and(|p| p.focused && !p.selection.is_empty());
        if visual {
            let before = self.areas;
            self.visual_sync();
            self.relayout_areas();
            return changed | (self.areas != before);
        }
        changed | self.preview_scroll_sync()
    }

    /// Synced scrolling: the page puts the editor's top line at its top, or, after the wheel
    /// turned over the page, the editor puts the page's top line at its top. True when the
    /// editor moved.
    fn preview_scroll_sync(&mut self) -> bool {
        let Some(pane) = self.preview.as_mut() else {
            return false;
        };
        let previewable = self.docs.active_doc().is_some_and(previewable);
        let Some(r) = self.docs.active_ready_mut().filter(|_| previewable) else {
            return false;
        };
        let top = view::source_top(r);
        match pane.take_page_top() {
            Some(want) if (want - top).abs() > 1e-3 => {
                view::set_source_top(r, want);
                self.full = true;
                true
            }
            Some(_) => false,
            None => {
                pane.align(top, view::source_ends(r));
                false
            }
        }
    }

    /// The pane's area when it shows a page that can be selected.
    pub(super) fn page_area(&self) -> Option<Rect> {
        let pane = self.preview.as_ref()?;
        let shown = pane.ended.is_none()
            && pane.frame.width > 1
            && self.docs.active_doc().is_some_and(previewable);
        self.areas.preview.filter(|_| shown)
    }

    /// Window point `x, y` as a point on the page: device pixels from its top left, kept
    /// inside it (the page starts one pixel in, after the line that parts it from the text).
    fn page_point(&self, r: Rect, x: f32, y: f32) -> (f32, f32) {
        let s = self.scale;
        (
            ((x - r.x) * s - 1.0).clamp(0.0, (r.w * s - 2.0).max(0.0)),
            ((y - r.y) * s).clamp(0.0, (r.h * s - 1.0).max(0.0)),
        )
    }

    /// The pointer rests at `x, y` on the page: asks the page for the cursor there, and gives
    /// the one it last answered until the new answer comes.
    pub(super) fn preview_hover(&mut self, x: f32, y: f32) -> CursorIcon {
        let Some(r) = self.page_area() else {
            return CursorIcon::Default;
        };
        let (px, py) = self.page_point(r, x, y);
        match self.preview.as_mut() {
            Some(pane) => {
                pane.hover(px, py);
                page_cursor(pane.pointer)
            }
            None => CursorIcon::Default,
        }
    }

    /// The main button went down at `x, y`: on the page, a selection starts there and the page
    /// takes the keyboard. False when the point is not on the page.
    pub(super) fn preview_press(&mut self, x: f32, y: f32, clicks: u32) -> bool {
        let Some(r) = self.page_area().filter(|r| r.contains(x, y)) else {
            return false;
        };
        let (px, py) = self.page_point(r, x, y);
        if let Some(pane) = self.preview.as_mut() {
            pane.press(px, py, clicks);
        }
        self.ui.find_focused = false;
        self.ui.results.set_focus(false);
        self.ui.drag = Some(Drag::Preview);
        self.redraw();
        true
    }

    /// The pointer moved with the button down after a press on the page. Past its top or
    /// bottom, the auto-scroll timer scrolls it.
    pub(super) fn preview_drag(&mut self, x: f32, y: f32) {
        let Some(r) = self.areas.preview else {
            return;
        };
        let (px, py) = self.page_point(r, x, y);
        if let Some(pane) = self.preview.as_mut() {
            pane.drag(px, py);
        }
        let outside = y < r.y || y >= r.bottom();
        if outside && self.ui.autoscroll_due.is_none() {
            self.ui.autoscroll_due = Some(Instant::now() + AUTOSCROLL_TICK);
        } else if !outside {
            self.ui.autoscroll_due = None;
        }
    }

    pub(super) fn preview_release(&mut self) {
        if let Some(pane) = self.preview.as_mut() {
            pane.release();
        }
    }

    /// A tick of the auto-scroll while a selection on the page is dragged past its edge: the
    /// page moves (and leads synced scrolling), and the selection follows the pointer.
    pub(super) fn preview_autoscroll(&mut self) {
        let Some(r) = self.areas.preview else {
            return;
        };
        let (x, y) = self.ui.mouse;
        let dy = if y < r.y {
            y - r.y
        } else if y >= r.bottom() {
            y - r.bottom() + 1.0
        } else {
            return;
        };
        // As the text scrolls past its edge: faster the farther the pointer is.
        let by = (dy.abs() / 20.0).clamp(0.25, 3.0) * dy.signum() * 20.0 * self.scale;
        if let Some(pane) = self.preview.as_mut() {
            pane.scroll(by);
        }
        self.preview_drag(x, y);
        self.wants_frame = true;
    }

    /// A right-click at `x, y`: on the page, it takes the keyboard (keeping its selection) and
    /// its menu opens. False when the point is not on the page.
    pub(super) fn preview_menu(&mut self, x: f32, y: f32) -> bool {
        if !self.page_area().is_some_and(|r| r.contains(x, y)) {
            return false;
        }
        let Some(pane) = self.preview.as_mut() else {
            return false;
        };
        pane.focus();
        let selected = !pane.selection.is_empty();
        self.ui.find_focused = false;
        self.ui.results.set_focus(false);
        // Opened by the pointer, so the access-key underlines stay hidden until a key is used.
        let cues = self.ui.keyboard;
        self.ui.popup = Some(crate::ui::Popup::with_cues(
            crate::ui::PopupKind::Text,
            crate::overlays::page_menu(selected, x + 2.0, y + 2.0),
            cues,
        ));
        self.redraw();
        true
    }

    /// Whether the page has the keyboard.
    pub(super) fn preview_focused(&self) -> bool {
        self.preview.as_ref().is_some_and(|p| p.focused)
    }

    /// The page gives the keyboard back to the text, its selection cleared.
    pub(super) fn preview_blur(&mut self) {
        if let Some(pane) = self.preview.as_mut()
            && pane.focused
        {
            pane.blur();
            self.redraw();
        }
    }

    /// A key while the page has the keyboard. Copy and Select all act on the page (through
    /// their commands); Escape and any other key give the keyboard back to the text, and the
    /// other keys then do there what they do. True when the key was used.
    pub(super) fn preview_key(&mut self, name: Option<&KeyName>, command: Option<C>) -> bool {
        if !self.preview_focused() {
            return false;
        }
        match (name, command) {
            (_, Some(C::Copy | C::SelectAll)) => false,
            (Some(KeyName::Escape), _) => {
                self.preview_blur();
                true
            }
            (None, None) => false,
            _ => {
                self.preview_blur();
                false
            }
        }
    }

    /// Puts the page's selection on the clipboard when its text has come, as mdview does: the
    /// text and the HTML beside it.
    fn preview_copied(&mut self) {
        let Some(c) = self.preview.as_mut().and_then(|p| p.take_copied()) else {
            return;
        };
        if c.text.is_empty() {
            return;
        }
        #[cfg(test)]
        {
            tests::COPIED.with(|v| *v.borrow_mut() = Some(c.text));
        }
        #[cfg(not(test))]
        if c.html.is_empty() || !platform::set_clipboard_html(&c.html, &c.text) {
            platform::set_clipboard_text(&c.text);
        }
    }

    /// A task box clicked on the page: its mark changes in the text, as an edit that can be
    /// undone; nothing is saved. Only when the text is still the one the page showed.
    fn preview_task(&mut self) {
        let Some((id, version, at, checked)) = self.preview.as_mut().and_then(|p| p.take_task())
        else {
            return;
        };
        let current = self
            .docs
            .active_doc()
            .filter(|d| d.id == id)
            .and_then(|d| d.ready().map(|r| (d.revision, r.buffer.revision())));
        if current == Some(version) {
            self.edit(EditOp::SetTask(at, if checked { 'x' } else { ' ' }));
        }
    }

    /// Any input but the wheel over the page: the editor leads synced scrolling again.
    pub(super) fn editor_leads(&mut self) {
        if let Some(pane) = self.preview.as_mut() {
            pane.editor_leads();
        }
    }

    /// The wheel over the pane: true when it was there.
    pub(super) fn preview_wheel(
        &mut self,
        x: f32,
        y: f32,
        notches: f32,
        pixels: Option<f32>,
    ) -> bool {
        if !self.areas.preview.is_some_and(|r| r.contains(x, y)) {
            return false;
        }
        let by = match pixels {
            Some(py) => -py,
            None => -notches * NOTCH * self.scale,
        };
        if let Some(pane) = self.preview.as_mut() {
            pane.scroll(by);
        }
        true
    }
}

/// Whether the pane draws this document: Markdown, or a document that has no file yet.
pub(crate) fn previewable(doc: &crate::docs::Doc) -> bool {
    doc.is_markdown() || doc.path.is_none()
}

/// The cursor the page asks for at a place: a hand over a task box, the text cursor over text.
fn page_cursor(p: mdview_preview::Pointer) -> CursorIcon {
    match p {
        mdview_preview::Pointer::Hand => CursorIcon::Pointer,
        mdview_preview::Pointer::Text => CursorIcon::Text,
        mdview_preview::Pointer::Arrow => CursorIcon::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::super::files_tests::{app, open_now, scratch, type_text};
    use super::*;
    use crate::commands::CommandId as C;
    use crate::ui::Act;
    use std::cell::RefCell;
    use std::time::Duration;

    thread_local! {
        /// What the page's Copy put on the clipboard, in place of the system's in tests.
        pub(super) static COPIED: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    /// The mdview that draws the pane in the default build: `MDEDIT_TEST_MDVIEW`, or the
    /// viewer's release build beside this repository. The embedded build needs none.
    fn viewer() -> Option<PathBuf> {
        if cfg!(feature = "embedded-preview") {
            return Some(PathBuf::from("mdview"));
        }
        let path = std::env::var_os("MDEDIT_TEST_MDVIEW")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../markdown-viewer-rs/target/release")
                    .join(format!("mdview{}", std::env::consts::EXE_SUFFIX))
            });
        path.is_file().then_some(path)
    }

    /// Takes the pane's answers in until `until` holds, or five seconds pass.
    fn pump(a: &mut App, until: impl Fn(&App) -> bool) {
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end {
            a.preview_sync();
            a.compose();
            if until(a) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn generation(a: &App) -> u64 {
        a.preview.as_ref().map_or(0, |p| p.generation)
    }

    /// The pane's pixels as the screen shows them.
    fn pane_pixels(a: &App) -> Vec<u8> {
        let r = a.areas.preview.expect("a pane");
        let s = a.scale;
        let (x0, y0) = ((r.x * s) as usize + 2, (r.y * s) as usize);
        let (w, h) = ((r.w * s) as usize - 4, (r.h * s) as usize);
        let sw = a.screen.width as usize;
        let mut out = Vec::new();
        for y in y0..y0 + h {
            out.extend_from_slice(&a.screen.pixels[(y * sw + x0) * 4..(y * sw + x0 + w) * 4]);
        }
        out
    }

    #[test]
    fn the_pane_shows_the_page_and_follows_the_typing() {
        let Some(viewer) = viewer() else {
            eprintln!("no mdview build to draw the pane: skipped");
            return;
        };
        let dir = scratch("pane");
        let md = dir.join("a.md");
        std::fs::write(&md, "# Title\n\nSome text.\n").unwrap();
        let mut a = app();
        open_now(&mut a, &md);
        a.viewer_path = Some(Some(viewer));
        a.command(C::PreviewPane);
        assert!(a.preview.is_some() && a.docs.pane_open);
        a.compose();
        let pane = a.areas.preview.expect("the pane beside the text");
        assert!(a.areas.text.right() <= pane.x);
        pump(&mut a, |a| generation(a) > 0);
        let ended = &a.preview.as_ref().unwrap().ended;
        assert!(
            generation(&a) > 0 && ended.is_none(),
            "a page arrived: {ended:?}"
        );
        let first = pane_pixels(&a);
        assert!(
            first.chunks(4).any(|p| p != &first[..4]),
            "something is drawn"
        );

        // Typing changes the page.
        let before = generation(&a);
        type_text(&mut a, "More text, and more. ");
        pump(&mut a, |a| generation(a) > before);
        assert!(generation(&a) > before);
        assert!(a.preview.as_ref().unwrap().ended.is_none());
        assert!(pane_pixels(&a) != first);

        // The button closes it again.
        a.command(C::PreviewPane);
        a.compose();
        assert!(a.preview.is_none() && a.areas.preview.is_none());
    }

    #[test]
    fn scrolling_either_side_brings_the_other_to_the_same_line() {
        let Some(viewer) = viewer() else {
            eprintln!("no mdview build to draw the pane: skipped");
            return;
        };
        let dir = scratch("pane-sync");
        let md = dir.join("a.md");
        let text: String = (0..300)
            .map(|i| format!("Paragraph {i}, a few words long.\n\n"))
            .collect();
        std::fs::write(&md, text).unwrap();
        let mut a = app();
        open_now(&mut a, &md);
        a.viewer_path = Some(Some(viewer));
        a.command(C::PreviewPane);
        a.compose();
        pump(&mut a, |a| generation(a) > 0);
        let page_top = |a: &App| a.preview.as_ref().unwrap().top;
        let editor_top = |a: &mut App| view::source_top(a.docs.active_ready_mut().unwrap());
        assert!(page_top(&a) < 1.0);

        // The editor scrolls to line 201 (paragraph 100): the page follows.
        view::scroll_rows(a.docs.active_ready_mut().unwrap(), 200.0);
        a.compose();
        pump(&mut a, |a| a.preview.as_ref().unwrap().top > 1000.0);
        let after_editor = page_top(&a);
        assert!(after_editor > 1000.0, "the page followed: {after_editor}");
        assert!((editor_top(&mut a) - 200.0).abs() < 0.01);

        // The wheel over the page: the editor follows, and the page stays where the wheel put it.
        let r = a.areas.preview.unwrap();
        assert!(a.preview_wheel(r.cx(), r.cy(), -10.0, None));
        pump(&mut a, |a| {
            let r = a.docs.active_ready().unwrap();
            r.view.top > 205.0
        });
        let line = editor_top(&mut a);
        assert!(line > 205.0, "the editor followed: {line}");
        let after_wheel = page_top(&a);
        assert!(
            after_wheel > after_editor + 300.0,
            "{after_editor} {after_wheel}"
        );
        // Nothing goes back and forth: a while later both are where they were, nothing waits.
        let quiet = Instant::now() + Duration::from_millis(300);
        pump(&mut a, |_| Instant::now() > quiet);
        assert_eq!(page_top(&a), after_wheel);
        assert!((editor_top(&mut a) - line).abs() < 1e-6);
        assert_eq!(a.preview.as_ref().unwrap().waiting, 0);

        // A key: the editor leads again, and its own scrolling moves the page.
        a.editor_leads();
        view::scroll_rows(a.docs.active_ready_mut().unwrap(), -100.0);
        a.compose();
        pump(&mut a, |a| {
            a.preview.as_ref().unwrap().top < after_wheel - 1000.0
        });
        assert!(page_top(&a) < after_wheel - 1000.0);
    }

    #[test]
    fn the_editor_at_its_end_brings_the_page_to_its_end_wrapped_or_not() {
        let Some(viewer) = viewer() else {
            eprintln!("no mdview build to draw the pane: skipped");
            return;
        };
        let dir = scratch("pane-end");
        let md = dir.join("a.md");
        // Headings and code are taller on the page than in the text: the page is the longer.
        let text: String = (0..120)
            .map(|i| format!("## Part {i}\n\n```\ncode {i}\n```\n\nA paragraph.\n\n"))
            .collect();
        std::fs::write(&md, text).unwrap();
        let mut a = app();
        open_now(&mut a, &md);
        a.viewer_path = Some(Some(viewer));
        a.command(C::PreviewPane);
        a.compose();
        pump(&mut a, |a| generation(a) > 0);
        let at_bottom = |a: &App| {
            let p = a.preview.as_ref().unwrap();
            p.top > 0.0 && p.top >= p.total - p.frame.height as f32 - 0.5
        };
        for wrap in [false, true] {
            a.docs.active_ready_mut().unwrap().wrap = wrap;
            view::scroll_rows(a.docs.active_ready_mut().unwrap(), -1e6);
            a.compose();
            pump(&mut a, |a| a.preview.as_ref().unwrap().top < 1.0);
            // To the editor's end: the page reaches its own.
            view::scroll_rows(a.docs.active_ready_mut().unwrap(), 1e6);
            a.compose();
            pump(&mut a, at_bottom);
            assert!(at_bottom(&a), "wrap {wrap}: the page at its end");
            // Up a line and down again: the page comes back to its end, not above it.
            view::scroll_rows(a.docs.active_ready_mut().unwrap(), -1.0);
            a.compose();
            pump(&mut a, |a| !at_bottom(a));
            assert!(!at_bottom(&a), "wrap {wrap}: a line up moves the page");
            view::scroll_rows(a.docs.active_ready_mut().unwrap(), 1.0);
            a.compose();
            pump(&mut a, at_bottom);
            assert!(at_bottom(&a), "wrap {wrap}: back at its end");
            // The page leading from there leaves the editor at its end.
            let end = view::source_top(a.docs.active_ready_mut().unwrap());
            let r = a.areas.preview.unwrap();
            assert!(a.preview_wheel(r.cx(), r.cy(), -3.0, None));
            let quiet = Instant::now() + Duration::from_millis(300);
            pump(&mut a, |_| Instant::now() > quiet);
            let now = view::source_top(a.docs.active_ready_mut().unwrap());
            assert!((now - end).abs() < 0.01, "wrap {wrap}: {end} {now}");
            a.editor_leads();
        }
    }

    #[test]
    fn a_click_on_a_task_box_ticks_it_in_the_text_without_saving() {
        let Some(viewer) = viewer() else {
            eprintln!("no mdview build to draw the pane: skipped");
            return;
        };
        let dir = scratch("pane-tasks");
        let md = dir.join("a.md");
        let text = "Tasks:\n\n- [ ] one\n- [x] two\n";
        std::fs::write(&md, text).unwrap();
        let mut a = app();
        open_now(&mut a, &md);
        a.viewer_path = Some(Some(viewer));
        a.command(C::PreviewPane);
        a.compose();
        pump(&mut a, |a| generation(a) > 0);
        let r = a.page_area().expect("a page in the pane");
        let now = |a: &App| {
            let b = &a.docs.active_ready().unwrap().buffer;
            let mut s = String::new();
            b.text(0..b.len_bytes(), &mut s);
            s
        };
        let caret = a.docs.active_ready().unwrap().view.sel;
        // Single clicks over the top left of the page, one after another without waiting: the
        // first on a box ticks it; the rest are about a text that has changed since, or miss.
        'scan: for y in (0..(r.h.min(160.0) as u32)).step_by(3) {
            for x in (0..(r.w.min(80.0) as u32)).step_by(3) {
                a.preview_press(r.x + x as f32, r.y + y as f32, 1);
                a.left_release();
                a.preview_sync();
                if now(&a) != text {
                    break 'scan;
                }
            }
        }
        pump(&mut a, |a| now(a) != text);
        let ticked = now(&a);
        assert!(
            ticked == "Tasks:\n\n- [x] one\n- [x] two\n"
                || ticked == "Tasks:\n\n- [ ] one\n- [ ] two\n",
            "{ticked:?}"
        );
        // An edit like any other: the document is modified, nothing is saved, the caret stays,
        // and Undo takes it back.
        assert!(a.docs.active_doc().unwrap().modified());
        assert_eq!(std::fs::read_to_string(&md).unwrap(), text);
        assert_eq!(a.docs.active_ready().unwrap().view.sel, caret);
        a.command(C::Undo);
        assert_eq!(now(&a), text);
    }

    #[test]
    fn the_cursor_is_a_hand_over_a_box_and_a_tick_leaves_the_page_still() {
        let Some(viewer) = viewer() else {
            eprintln!("no mdview build to draw the pane: skipped");
            return;
        };
        let dir = scratch("pane-tasks-still");
        let md = dir.join("a.md");
        let mut text = String::from("- [ ] first\n- [x] second\n\n");
        text.extend((0..150).map(|i| format!("Paragraph {i}.\n\n")));
        std::fs::write(&md, &text).unwrap();
        let mut a = app();
        open_now(&mut a, &md);
        a.viewer_path = Some(Some(viewer));
        a.command(C::PreviewPane);
        a.compose();
        pump(&mut a, |a| generation(a) > 0);
        let idle = |a: &App| a.preview.as_ref().unwrap().idle();
        // The caret at the end of the text, the page then scrolled back to its top: what the
        // reader sees is the boxes, far from the caret.
        a.docs.active_ready_mut().unwrap().view.sel =
            mdedit_core::selection::Selection::at(text.len());
        a.preview_sync();
        pump(&mut a, |a| a.preview.as_ref().unwrap().top > 0.0);
        a.preview.as_mut().unwrap().scroll(-1e9);
        pump(&mut a, |a| {
            a.preview.as_ref().unwrap().top == 0.0 && idle(a)
        });
        let r = a.page_area().expect("a page in the pane");
        // The cursor over the page, asked for at each place: a hand over a box, the text
        // cursor over words.
        let mut hand = None;
        let mut text_cursor = false;
        'scan: for y in (0..(r.h.min(100.0) as u32)).step_by(4) {
            for x in (0..(r.w.min(120.0) as u32)).step_by(4) {
                let (px, py) = (r.x + x as f32, r.y + y as f32);
                a.pointer_moved(px, py);
                pump(&mut a, idle);
                a.preview_sync();
                text_cursor |= a.cursor == CursorIcon::Text;
                if a.cursor == CursorIcon::Pointer {
                    hand = Some((px, py));
                    if text_cursor {
                        break 'scan;
                    }
                }
            }
        }
        let (px, py) = hand.expect("a hand over a box");
        assert!(text_cursor, "the text cursor over words");
        // A click there ticks the box, and the page stays at its top, though the caret is at
        // the far end.
        a.pointer_moved(px, py);
        a.left_press();
        a.left_release();
        pump(&mut a, |a| now_text(a) != text);
        assert_ne!(now_text(&a), text);
        pump(&mut a, |a| idle(a) && generation(a) > 0);
        a.preview_sync();
        assert_eq!(a.preview.as_ref().unwrap().top, 0.0, "the page stayed");
    }

    fn now_text(a: &App) -> String {
        let b = &a.docs.active_ready().unwrap().buffer;
        let mut s = String::new();
        b.text(0..b.len_bytes(), &mut s);
        s
    }

    #[test]
    fn the_page_selects_with_the_mouse_and_copy_takes_its_text() {
        let Some(viewer) = viewer() else {
            eprintln!("no mdview build to draw the pane: skipped");
            return;
        };
        let dir = scratch("pane-select");
        let md = dir.join("a.md");
        let text: String = (0..80)
            .map(|i| format!("Paragraph {i} of **the** page.\n\n"))
            .collect();
        std::fs::write(&md, &text).unwrap();
        let mut a = app();
        open_now(&mut a, &md);
        a.viewer_path = Some(Some(viewer));
        a.command(C::PreviewPane);
        a.compose();
        pump(&mut a, |a| generation(a) > 0);
        let r = a.page_area().expect("a page in the pane");
        let selected = |a: &App| !a.preview.as_ref().unwrap().selection.is_empty();
        let copied = || COPIED.with(|v| v.borrow_mut().take());
        let caret = a.docs.active_ready().unwrap().view.sel;

        // A drag across the first lines of the page: they show selected, the page has the
        // keyboard, and the text keeps its caret.
        a.pointer_moved(r.x + 4.0, r.y + 4.0);
        a.left_press();
        a.pointer_moved(r.x + r.w - 4.0, r.y + 120.0);
        a.left_release();
        pump(&mut a, selected);
        assert!(selected(&a), "the selection shows");
        assert!(a.preview_focused() && !a.text_focused());
        assert_eq!(a.docs.active_ready().unwrap().view.sel, caret);

        // Copy takes the rendered text, not the Markdown.
        a.command(C::Copy);
        pump(&mut a, |_| COPIED.with(|v| v.borrow().is_some()));
        let got = copied().expect("copied");
        assert!(got.starts_with("Paragraph 0 of the page."), "{got:?}");
        assert!(!got.contains("**"));

        // Select all takes the whole page.
        a.command(C::SelectAll);
        a.command(C::Copy);
        pump(&mut a, |_| COPIED.with(|v| v.borrow().is_some()));
        let all = copied().expect("copied");
        assert!(all.contains("Paragraph 79 of the page."), "{}", all.len());
        assert_eq!(
            a.docs.active_ready().unwrap().view.sel,
            caret,
            "the text is not selected"
        );

        // The right-click menu: Copy and Select all, Copy only with something selected; the
        // palette's Copy follows the page too. Copy from the menu takes the page's text.
        a.preview_sync();
        assert!(crate::palette::enabled(C::Copy, &a.docs, &a.ui, true));
        a.pointer_moved(r.x + 40.0, r.y + 40.0);
        a.right_press();
        let items: Vec<(Act, bool)> = {
            let p = a.ui.popup.as_ref().expect("the page's menu");
            p.menu
                .items
                .iter()
                .map(|i| (i.id.clone(), i.enabled))
                .collect()
        };
        assert_eq!(
            items,
            [(Act::Cmd(C::Copy), true), (Act::Cmd(C::SelectAll), true)]
        );
        assert!(
            a.preview_focused(),
            "a right-click keeps the page's keyboard and selection"
        );
        a.choose(Act::Cmd(C::Copy), false);
        pump(&mut a, |_| COPIED.with(|v| v.borrow().is_some()));
        assert!(copied().is_some_and(|t| t.contains("Paragraph 79")));
        // Cut does nothing to the text while the page has the keyboard.
        let before = a.docs.active_ready().unwrap().buffer.len_bytes();
        a.command(C::SelectAll);
        a.command(C::Cut);
        assert_eq!(a.docs.active_ready().unwrap().buffer.len_bytes(), before);

        // Escape gives the keyboard back and clears the page's selection.
        a.key_pressed(crate::keys::Key::Name(crate::keys::KeyName::Escape), None);
        pump(&mut a, |a| !selected(a));
        assert!(!selected(&a) && !a.preview_focused() && a.text_focused());
        a.preview_sync();
        assert!(!crate::palette::enabled(C::Copy, &a.docs, &a.ui, true));
        // With nothing selected on the page, its menu's Copy is off.
        a.pointer_moved(r.x + 40.0, r.y + 40.0);
        a.right_press();
        let copy_on = a.ui.popup.as_ref().unwrap().menu.items[0].enabled;
        assert!(!copy_on);
        a.ui.popup = None;

        // A press in the text takes the keyboard from the page too.
        a.pointer_moved(r.x + 4.0, r.y + 4.0);
        a.left_press();
        a.left_release();
        assert!(a.preview_focused());
        let t = a.areas.text;
        a.pointer_moved(t.x + 60.0, t.y + 10.0);
        a.left_press();
        a.left_release();
        assert!(!a.preview_focused());
        // Copy is the text's again.
        a.command(C::Copy);
        let quiet = Instant::now() + Duration::from_millis(200);
        pump(&mut a, |_| Instant::now() > quiet);
        assert!(copied().is_none());
    }

    #[test]
    fn without_mdview_only_the_pane_of_the_standalone_build_is_offered() {
        let dir = scratch("pane-none");
        let md = dir.join("a.md");
        std::fs::write(&md, "# Title\n").unwrap();
        let mut a = app();
        open_now(&mut a, &md);
        a.compose();
        assert!(a.hit_rect(Hit::Bar(crate::ui::BarItem::Preview)).is_some());
        // What looking for mdview leaves when there is none.
        a.docs.viewer = false;
        a.docs.pane = crate::preview_pane::STANDALONE;
        a.viewer_path = Some(None);
        a.compose();
        let standalone = crate::preview_pane::STANDALONE;
        assert_eq!(
            a.hit_rect(Hit::Bar(crate::ui::BarItem::Preview)).is_some(),
            standalone
        );
        assert_eq!(chrome::preview_button(&a.docs).is_some(), standalone);
        let view = crate::overlays::view_menu(&a.docs, &a.ui, 0.0, 0.0, false);
        let tab = crate::overlays::tab_menu(&a.docs, 0, 0.0, 0.0);
        let has = |m: &mdshared_ui::menu::Menu<Act>, ids: &[C]| {
            m.items.iter().any(|it| match it.id {
                Act::Cmd(c) | Act::TabCmd(c, _) => ids.contains(&c),
                _ => false,
            })
        };
        let in_mdview = [C::PreviewInViewer, C::PreviewSideBySide];
        assert!(!has(&view, &in_mdview) && !has(&tab, &in_mdview));
        assert_eq!(has(&view, &[C::PreviewPane]), standalone);
        assert_eq!(has(&tab, &[C::PreviewPane]), standalone);
        // The previews in mdview do nothing from their keys; the pane opens only standalone.
        a.command(C::PreviewInViewer);
        assert!(a.docs.links.is_empty());
        a.command(C::PreviewPane);
        assert_eq!(a.preview.is_some(), standalone);
        if standalone {
            pump(&mut a, |a| generation(a) > 0);
            let p = a.preview.as_ref().unwrap();
            assert!(
                p.generation > 0 && p.ended.is_none(),
                "drawn without mdview: {:?}",
                p.ended
            );
        }
    }

    #[test]
    fn visual_editing_opens_a_block_in_place_follows_its_typing_and_puts_it_back() {
        use crate::keys::{Key, KeyName};
        let Some(viewer) = viewer() else {
            eprintln!("no mdview build to draw the pane: skipped");
            return;
        };
        let dir = scratch("visual");
        let md = dir.join("a.md");
        std::fs::write(
            &md,
            "# Title

First paragraph.

- one
- two

Last.
",
        )
        .unwrap();
        let mut a = app();
        open_now(&mut a, &md);
        a.viewer_path = Some(Some(viewer));
        let text_before = {
            a.compose();
            a.areas.text
        };
        a.command(C::VisualEdit);
        assert!(a.visual_on() && a.docs.visual_open);
        a.compose();
        pump(&mut a, |a| generation(a) > 0);
        // The page takes the whole text area; no text view while no block is open.
        let page = a.areas.preview.expect("the page");
        assert_eq!(page, text_before);
        assert_eq!(a.areas.text.w, 0.0);

        // A press near the page's top opens the title.
        a.visual_press(page.cx(), page.y + 50.0);
        pump(&mut a, |a| {
            a.visual_block().is_some() && a.areas.text.w > 0.0
        });
        let block = a.visual_block().expect("a block opened");
        assert_eq!(block.first, 0);
        let boxed = a.areas.text;
        assert!(boxed.y >= page.y && boxed.bottom() <= page.bottom() && boxed.w < page.w);
        let ((first, last), h1) = a.preview.as_ref().unwrap().sent_gap().unwrap();
        assert_eq!((first, last), (1, 1));

        // Typing goes into the document; Enter adds a line to the block and the room grows.
        type_text(&mut a, "My ");
        a.key_pressed(Key::Name(KeyName::Enter), None);
        a.compose();
        pump(&mut a, |a| {
            a.preview
                .as_ref()
                .unwrap()
                .sent_gap()
                .is_some_and(|g| g.0 == (1, 2))
        });
        let r = a.docs.active_ready().unwrap();
        let mut s = String::new();
        r.buffer.text(0..r.buffer.len_bytes(), &mut s);
        assert!(
            s.starts_with(
                "My 
# Title
"
            ),
            "{s:?}"
        );
        let (lines, h2) = a.preview.as_ref().unwrap().sent_gap().unwrap();
        assert_eq!(lines, (1, 2));
        assert!(h2 > h1);

        // Esc puts it back; Ctrl+E again goes back to the text beside the page.
        a.key_pressed(Key::Name(KeyName::Escape), None);
        a.compose();
        assert!(a.visual_block().is_none());
        assert!(a.preview.as_ref().unwrap().sent_gap().is_none());
        a.command(C::VisualEdit);
        a.compose();
        assert!(!a.visual_on() && a.areas.text.w > 0.0);
    }
}
