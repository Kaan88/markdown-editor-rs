//! The preview pane in the app: opening and closing it, telling it what changed, and the wheel
//! over it. The pane itself is `crate::preview_pane`.

use super::*;
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
            pane.follow(line as u32 + 1, column as u32);
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

#[cfg(test)]
mod tests {
    use super::super::files_tests::{app, open_now, scratch, type_text};
    use super::*;
    use crate::commands::CommandId as C;
    use crate::ui::Act;
    use std::time::Duration;

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
}
