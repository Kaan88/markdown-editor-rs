//! The preview pane in the app: opening and closing it, telling it what changed, and the wheel
//! over it. The pane itself is `crate::preview_pane`.

use super::*;
use crate::preview_pane::Pane;
use std::time::Duration;

/// The wheel moves the page this many logical px a notch, as three lines of mdview's text.
const NOTCH: f32 = 60.0;

impl App {
    /// Looks for mdview and offers the previews (or takes them away) by what it finds. A pane
    /// that is open stays open: it was started from an mdview that was there.
    pub(super) fn look_for_viewer(&mut self) {
        let found = crate::preview_pane::viewer();
        let had = self.docs.viewer;
        self.docs.viewer = found.is_some() || self.preview.is_some();
        self.viewer_path = Some(found);
        if had != self.docs.viewer {
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
            let Some(Some(viewer)) = self.viewer_path.clone() else {
                return;
            };
            self.docs.other = None;
            self.docs.same_file = false;
            self.preview = Some(Pane::open(&viewer, self.docs.waker()));
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
        changed
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
    fn without_mdview_no_preview_is_offered_anywhere() {
        let dir = scratch("pane-none");
        let md = dir.join("a.md");
        std::fs::write(&md, "# Title\n").unwrap();
        let mut a = app();
        open_now(&mut a, &md);
        a.compose();
        assert!(a.hit_rect(Hit::Bar(crate::ui::BarItem::Preview)).is_some());
        a.docs.viewer = false;
        a.compose();
        assert!(a.hit_rect(Hit::Bar(crate::ui::BarItem::Preview)).is_none());
        assert_eq!(chrome::preview_button(&a.docs), None);
        let view = crate::overlays::view_menu(&a.docs, &a.ui, 0.0, 0.0, false);
        let tab = crate::overlays::tab_menu(&a.docs, 0, 0.0, 0.0);
        let previews = |m: &mdshared_ui::menu::Menu<Act>| {
            m.items.iter().any(|it| {
                matches!(
                    it.id,
                    Act::Cmd(C::PreviewPane | C::PreviewInViewer | C::PreviewSideBySide)
                        | Act::TabCmd(
                            C::PreviewPane | C::PreviewInViewer | C::PreviewSideBySide,
                            _
                        )
                )
            })
        };
        assert!(!previews(&view) && !previews(&tab));
        // Its key and the command do nothing.
        a.command(C::PreviewPane);
        a.command(C::PreviewInViewer);
        assert!(a.preview.is_none() && a.docs.links.is_empty());
    }
}
