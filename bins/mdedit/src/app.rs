//! The window and its event loop, modelled on the viewer's `bins/mdv-app/src/app.rs`: a frameless
//! window, regions with signatures, damage-only present, a hidden first frame, timers only for
//! pending work. Also `--shot`.
//!
//! **Frames.** The window is five regions (title row, command bar, Files, text, status) plus
//! overlays (menus, dialogs, toast, tooltip). Each region has a signature: a hash of everything it
//! shows. A frame redraws only the regions whose signature changed, redraws overlays together with
//! every region under them, and sends only the redrawn rectangles to the screen. A caret blink or a
//! caret move that changes nothing else repaints only the line bands involved.
//!
//! **Idle.** The loop waits for events; it wakes on a timer only for something pending (caret
//! blink while the window has focus, a tooltip delay, the toast expiring, drag auto-scroll, saving
//! the window size after a resize, `--exit-after-ms`).

use crate::args::{Args, ShotDialog, ShotMenu};
use crate::chrome::{self, Ctx};
use crate::commands::strings;
use crate::docs::{DocState, Docs, EditOp, Waker};
use crate::keys::Mods;
use crate::minimap;
use crate::overlays::{self, Bubble};
use crate::panes;
use crate::platform;
use crate::session::{Session, ThemeChoice};
use crate::status;
use crate::trace::{FrameTimes, Trace, summary};
use crate::ui::{
    Act, After, Areas, BarField, Dialog, DialogKind, Drag, Hit, PANE_MAX, PANE_MIN, PopupKind,
    REGIONS, Region, Scroller, Ui,
};
use crate::view::{self, DrawState, View, metrics};
use mdedit_highlight::{DocId, HighlightService};
use mdshared_app::paths::canonical_or_absolute;
use mdshared_app::sister::Request;
use mdshared_app::watch::FileWatcher;
use mdshared_text::Bitmap;
use mdshared_ui::gfx::{Gfx, Painter, Rect};
use mdshared_ui::theme::{self, Palette};
use std::hash::{Hash, Hasher};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{CursorIcon, ResizeDirection, Theme as WinTheme, Window, WindowId};

mod commands;
mod compare_events;
#[cfg(test)]
mod files_tests;
mod handoff;
mod input;
pub(crate) mod modal;
mod palette_events;
mod replace_files;

const TIP_DELAY: Duration = Duration::from_millis(500);
const AUTOSCROLL_TICK: Duration = Duration::from_millis(16);
const SIZE_SAVE_DELAY: Duration = Duration::from_millis(800);
/// The hidden first frame waits for the active load until this long after launch.
const LOAD_WAIT: Duration = Duration::from_millis(400);
/// And for the first screen's colours until this long after launch.
const COLOUR_WAIT: Duration = Duration::from_millis(80);
/// `--run` waits this long after the first frame, for the window to be on a screen.
const RUN_DELAY: Duration = Duration::from_millis(500);
/// `--shot` waits this long for colours.
const SHOT_COLOUR_WAIT: Duration = Duration::from_secs(10);
/// Pane wheel step, logical px per notch.
const PANE_WHEEL: f32 = 90.0;
/// Pointer travel that still counts as the same spot for a double click.
const CLICK_SLOP: f32 = 4.0;

/// Wakes the loop from other threads; the data travels on channels.
#[derive(Debug)]
pub enum Wake {
    /// The single-instance listener queued requests.
    Instance,
    /// The viewer that a preview was following is gone.
    Link,
    /// The file watcher saw a change.
    Files,
    /// A load made progress or finished.
    Loaded,
    /// A search finished.
    Found,
    /// The search of a folder has matches or finished.
    Results,
    /// The function list of a document is made.
    Outline,
    /// The palette's list of the folder's files, or of a document's symbols, is made.
    Palette,
    /// The comparison of two files is made.
    Compare,
    /// The highlight worker queued colours.
    Highlight,
    /// An updater thread has an answer.
    Update(crate::updater::Event),
    /// A file dialog (off Windows they run on a helper thread) was answered or cancelled.
    #[cfg(not(windows))]
    Dialog(Picked),
}

/// The result of a file dialog, posted from its helper thread and handled on the UI thread where
/// the blocking call used to return.
#[cfg(not(windows))]
#[derive(Debug)]
pub enum Picked {
    Files(Vec<PathBuf>),
    Folder(Option<PathBuf>),
    /// The file to compare the active one with.
    CompareFile(Vec<PathBuf>),
    /// Save as for document `doc`; `after` is what waits for the save (`After::Nothing` for the
    /// plain command).
    Save {
        doc: DocId,
        to: Option<PathBuf>,
        after: After,
    },
}

/// A minimap's window and band for a document laid out as `g`, in the column `rect`.
fn make_plan(r: &mut crate::docs::Ready, g: &view::Geom, rect: Rect, scale: f32) -> minimap::Plan {
    let scene = view::map_scene(r, g);
    let col_h = (rect.bottom() * scale).round() as i32 - (rect.y * scale).round() as i32;
    let px = minimap::Px::new(scale);
    minimap::Plan::with_range(
        scene.rows,
        scene.first,
        scene.span,
        scene.range,
        px.row_h(),
        col_h,
    )
}

fn sig<T: Hash>(t: T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

/// The part of the window a popup menu may use: all of it below the title row, so that the
/// caption buttons stay uncovered; a menu taller than this scrolls.
fn menu_room(title: Rect, w: f32, h: f32) -> Rect {
    let top = title.bottom();
    Rect::new(0.0, top, w, h - top)
}

fn bits(r: Option<Rect>) -> Option<(u32, u32, u32, u32)> {
    r.map(|r| (r.x.to_bits(), r.y.to_bits(), r.w.to_bits(), r.h.to_bits()))
}

struct App {
    started: Instant,
    args: Args,
    trace: Trace,
    docs: Docs,
    ui: Ui,
    view: View,
    service: Option<HighlightService>,
    /// The document the service was last asked to colour.
    service_doc: Option<DocId>,
    service_dark: Option<bool>,
    window: Option<Rc<Window>>,
    /// The window's drop shadow (Linux: KWin's protocol, or the client-drawn margin where the
    /// compositor has none); `None` elsewhere. With the margin, `size` is the CONTENT size (the
    /// surface minus the margin on every side) and `present` puts the content inside the surface.
    /// Dropping it would remove the KWin shadow, so it lives as long as the window.
    shadow: Option<mdshared_ui::shadow::Shadow>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    incoming: Option<Receiver<Request>>,
    /// When the viewer is next told the caret's line, if an update is held back.
    link_due: Option<Instant>,
    watcher: Option<FileWatcher>,
    session_path: Option<PathBuf>,
    /// Write the session on exit (not in `--shot`).
    save: bool,
    session: Session,
    screen: Bitmap,
    gfx: Gfx,
    pal: Palette,
    system_dark: bool,
    /// The theme came from `--light` / `--dark` and the user has not picked one since: the
    /// session keeps the theme it had.
    theme_from_args: bool,
    scale: f32,
    /// Device px.
    size: (u32, u32),
    maximized: bool,
    focused: bool,
    areas: Areas,
    hits: [Vec<(Hit, Rect)>; 9],
    overlay_hits: Vec<(Hit, Rect)>,
    sigs: [u64; 9],
    /// The minimap's window and band as of the last frame, the lines of its window, and its
    /// drawing.
    /// One of each for the pane with the keyboard and for the second pane.
    map_plan: [Option<minimap::Plan>; 2],
    map_lines: [Option<std::ops::Range<usize>>; 2],
    map_cache: [minimap::Cache; 2],
    caret_sig: u64,
    /// The caret's line at the last text repaint.
    caret_line: Option<usize>,
    overlay_sig: u64,
    overlay_bounds: Vec<Rect>,
    /// Redraw everything next frame (size, scale or theme changed).
    full: bool,
    wants_frame: bool,
    /// A redraw this app asked for is on its way; one without it comes from the system.
    requested: bool,
    expose: bool,
    first: bool,
    exit: bool,
    mods: Mods,
    cursor: CursorIcon,
    last_title: String,
    last_active: Option<PathBuf>,
    toast_until: Option<Instant>,
    size_save_due: Option<Instant>,
    /// A file dialog is open on its helper thread: the window keeps drawing and a second request
    /// is ignored.
    #[cfg(not(windows))]
    dialog_busy: bool,
    exit_due: Option<Instant>,
    /// `--run`: when its command runs.
    run_due: Option<Instant>,
    /// `--scroll-steps`: steps left, the direction, and the frame times taken.
    steps: Option<u32>,
    step_down: bool,
    step_times: Vec<FrameTimes>,
    macros: crate::macros::Macros,
    /// The command palette: empty until it is first opened.
    palette: crate::palette::Host,
    /// `--type-steps`: keystrokes left and typed, each one's time from keystroke to pixels (ms),
    /// and the part of it spent in the edit itself.
    type_steps: Option<u32>,
    type_count: usize,
    type_times: Vec<f64>,
    type_edit_ms: f64,
    /// When the find marks are searched for again after an edit.
    find_due: Option<Instant>,
    /// Two files compared side by side; `None` unless a comparison is on.
    compare: Option<crate::compare::Compare>,
    /// The two files of `--compare`, to be compared once both are read.
    compare_wait: Option<(DocId, DocId)>,
    /// Loading and colouring have gone idle once; `--trace` printed the idle figures.
    settled: bool,
    load_traced: bool,
    colours_traced: bool,
    /// Touchpad deltas below one line or advance, kept until they add up.
    wheel_rest: (f64, f64),
    /// The session state last written (`session_sig`); a change is written at the next pass of
    /// the loop, so an ended process (shutdown, Task Manager) loses nothing.
    saved_sig: u64,
    /// `--goto`, `--select`, `--find`, `--scroll` or `--hscroll` wait for this document to be
    /// ready in a windowed run.
    positions_for: Option<DocId>,
}

impl App {
    /// Brings the shadow in line with the window's scale, focus and the margin in force (0 when
    /// maximized, fullscreen or tiled); cheap (returns at once when none changed) and nothing
    /// without a shadow. `focused` is the new focus when the event says it, otherwise the window's
    /// own. A change of the look is drawn by the next `present` (the margin is repainted for two
    /// frames); it also forces a present even if the content has nothing to redraw.
    fn update_shadow(&mut self, focused: Option<bool>) {
        let mut changed = false;
        if let (Some(shadow), Some(w)) = (self.shadow.as_mut(), self.window.as_ref()) {
            let active = focused.unwrap_or_else(|| w.has_focus());
            changed = shadow.update(w, w.scale_factor(), active);
        }
        if changed {
            self.expose = true;
            self.wants_frame = true;
        }
    }

    /// The content size inside a surface of `w` x `h` device px: the whole of it without a margin.
    fn content_size(&self, w: u32, h: u32) -> (u32, u32) {
        let (w, h) = match &self.shadow {
            Some(s) => s.content_size(w, h),
            None => (w, h),
        };
        (w.max(1), h.max(1))
    }

    fn new(
        args: Args,
        docs: Docs,
        trace: Trace,
        session_path: Option<PathBuf>,
        session: Session,
    ) -> App {
        let system_dark = platform::system_is_dark();
        App {
            started: trace.started,
            steps: args.scroll_steps,
            type_steps: args.type_steps,
            type_count: 0,
            type_times: Vec::new(),
            type_edit_ms: 0.0,
            find_due: None,
            compare: None,
            compare_wait: None,
            theme_from_args: args.dark.is_some(),
            save: args.shot.is_none(),
            args,
            trace,
            docs,
            ui: Ui::new(),
            view: View::new(),
            service: None,
            service_doc: None,
            service_dark: None,
            window: None,
            shadow: None,
            surface: None,
            incoming: None,
            link_due: None,
            watcher: None,
            session_path,
            session,
            screen: Bitmap::new(1, 1),
            gfx: Gfx::new(),
            pal: theme::LIGHT,
            system_dark,
            scale: 1.0,
            size: (1, 1),
            maximized: false,
            focused: true,
            areas: Areas::default(),
            hits: Default::default(),
            overlay_hits: Vec::new(),
            sigs: [0; 9],
            map_plan: [None; 2],
            map_lines: [None, None],
            map_cache: Default::default(),
            caret_sig: 0,
            caret_line: None,
            overlay_sig: 0,
            overlay_bounds: Vec::new(),
            full: true,
            wants_frame: true,
            requested: false,
            expose: false,
            first: true,
            exit: false,
            mods: Mods::default(),
            cursor: CursorIcon::Default,
            last_title: String::new(),
            last_active: None,
            toast_until: None,
            size_save_due: None,
            #[cfg(not(windows))]
            dialog_busy: false,
            exit_due: None,
            run_due: None,
            step_down: true,
            step_times: Vec::new(),
            macros: crate::macros::Macros::default(),
            palette: Default::default(),
            settled: false,
            load_traced: false,
            colours_traced: false,
            wheel_rest: (0.0, 0.0),
            saved_sig: 0,
            positions_for: None,
        }
    }

    fn dark(&self) -> bool {
        self.docs.theme.is_dark(self.system_dark)
    }

    fn redraw(&mut self) {
        self.wants_frame = true;
    }

    fn logical(&self) -> (f32, f32) {
        (
            self.size.0 as f32 / self.scale,
            self.size.1 as f32 / self.scale,
        )
    }

    fn sync_theme(&mut self) {
        let dark = self.dark();
        let pal = if dark { theme::DARK } else { theme::LIGHT };
        if pal != self.pal {
            self.pal = pal;
            self.full = true;
            self.view.clear();
            #[cfg(windows)]
            if let Some(w) = &self.window {
                style(w, dark);
            }
        }
        if self.service_dark != Some(dark) {
            if let Some(s) = &self.service
                && self.service_dark.is_some()
            {
                s.set_dark(dark);
            }
            self.service_dark = Some(dark);
        }
    }

    /// The caret shows: the text has the keyboard and the window has focus.
    fn text_focused(&self) -> bool {
        !self.ui.find_focused
            && self.ui.dialog.is_none()
            && !self.palette_open()
            && self.docs.active_ready().is_some()
    }

    // ---- documents -------------------------------------------------------------------------

    /// Takes in whatever the worker threads have sent. True if anything changed.
    fn drain(&mut self) -> bool {
        let dark = self.dark();
        let applied = self.docs.drain_loads(self.service.as_ref(), dark);
        if applied.active_ready {
            self.trace_load();
        }
        let mut changed = applied.active_ready;
        changed |= self.docs.drain_finds();
        if let Some(s) = &self.service {
            let colored = s.take();
            if !colored.is_empty() {
                changed |= self.docs.colored(colored);
                self.trace_colours();
            }
        }
        changed |= self.apply_positions_due();
        changed
    }

    /// In a windowed run, applies the position flags once their document is ready and the
    /// window has a size. True when they were applied.
    fn apply_positions_due(&mut self) -> bool {
        let Some(id) = self.positions_for else {
            return false;
        };
        let ready = self
            .docs
            .active_doc()
            .is_some_and(|d| d.id == id && d.ready().is_some());
        if !ready || self.window.is_none() {
            return false;
        }
        self.positions_for = None;
        self.apply_positions();
        true
    }

    fn trace_load(&mut self) {
        if self.load_traced || !self.trace.on {
            return;
        }
        let Some(doc) = self.docs.active_doc() else {
            return;
        };
        let Some(r) = doc.ready() else { return };
        self.load_traced = true;
        self.trace.stage(&format!(
            "load done: {} bytes, {} lines, {:.1} ms, {}, {}",
            r.info.file_bytes,
            r.buffer.len_lines(),
            r.info.elapsed.as_secs_f64() * 1000.0,
            r.info.encoding.label(),
            r.info.eol.style().label()
        ));
    }

    fn trace_colours(&mut self) {
        if self.colours_traced
            || !self.trace.on
            || !self
                .docs
                .active_ready()
                .is_some_and(|r| r.view.colors.generation > 0)
        {
            return;
        }
        self.colours_traced = true;
        self.trace.stage("first colours");
    }

    /// Asks the highlight worker for the visible lines and a screen each side, when the view has
    /// left the middle of what was asked last, the theme changed, or another document was asked.
    fn request_colours(&mut self) {
        let dark = self.dark();
        let Some(service) = &self.service else {
            return;
        };
        let asked = self.service_doc;
        let Some(doc) = self.docs.active_doc_mut() else {
            return;
        };
        let (id, revision) = (doc.id, doc.revision);
        let Some(r) = doc.ready_mut() else { return };
        if !r.colourable {
            return;
        }
        let lines = r.buffer.len_lines();
        let vis = view::shown_lines(r);
        let screen = (r.view.view_lines.ceil() as usize).max(1);
        let ok = match &r.view.colors.requested {
            Some((req, d)) if *d == dark && asked == Some(id) => {
                let lo = if req.start == 0 { 0 } else { screen / 2 };
                let hi = if req.end >= lines { 0 } else { screen / 2 };
                vis.start >= req.start + lo && vis.end + hi <= req.end
            }
            _ => false,
        };
        if ok {
            return;
        }
        let want = vis.start.saturating_sub(screen)..(vis.end + screen).min(lines);
        service.view_slot(id, r.view.slot, revision, want.clone());
        r.view.colors.requested = Some((want, dark));
        self.service_doc = Some(id);
    }

    /// The colours of the second pane's lines: asked for beside the active document's.
    fn request_colours_other(&mut self) {
        let dark = self.dark();
        let Some(service) = &self.service else {
            return;
        };
        self.docs.with_pane(1, |doc| {
            let (id, revision) = (doc.id, doc.revision);
            let Some(r) = doc.ready_mut().filter(|r| r.colourable) else {
                return;
            };
            let lines = r.buffer.len_lines();
            let vis = view::shown_lines(r);
            let screen = (r.view.view_lines.ceil() as usize).max(1);
            let ok = matches!(&r.view.colors.requested, Some((req, d))
                if *d == dark && vis.start >= req.start && vis.end <= req.end);
            if ok {
                return;
            }
            let want = vis.start.saturating_sub(screen)..(vis.end + screen).min(lines);
            service.peek_slot(id, r.view.slot, revision, want.clone());
            r.view.colors.requested = Some((want, dark));
        });
    }

    fn minimap_rect(&self, pane: usize) -> Option<Rect> {
        if pane == 0 {
            self.areas.minimap
        } else {
            self.areas.minimap_other
        }
    }

    /// Where a pane's minimap window and band are, from the view's layout of this frame.
    fn minimap_plan(&mut self, pane: usize, geom: Option<view::Geom>) -> Option<minimap::Plan> {
        let rect = self.minimap_rect(pane)?;
        let g = geom?;
        let scale = self.scale;
        self.docs.with_pane(pane, |doc| {
            Some(make_plan(doc.ready_mut()?, &g, rect, scale))
        })?
    }

    /// The colours of a pane's minimap window: asked for apart from the view's, with half a
    /// window more each side, when the window has left what was asked last, the theme changed or
    /// an edit made the answer stale.
    fn request_colours_map(&mut self, pane: usize) {
        self.map_lines[pane] = None;
        let dark = self.dark();
        let Some(plan) = self.map_plan[pane] else {
            return;
        };
        let Some(service) = &self.service else {
            return;
        };
        let asked = self.docs.with_pane(pane, |doc| {
            let (id, revision) = (doc.id, doc.revision);
            let r = doc.ready_mut().filter(|r| r.colourable)?;
            let (first, n) = (plan.first_row(), plan.row_count());
            let win = minimap::lines_of_rows(first..first + n, |row| r.line_at_row(row));
            let ok = matches!(&r.view.map_colors.requested, Some((req, d))
                if *d == dark && win.start >= req.start && win.end <= req.end);
            let want =
                (!ok).then(|| minimap::lines_of_rows(plan.colour_rows(), |row| r.line_at_row(row)));
            if let Some(want) = &want {
                r.view.map_colors.requested = Some((want.clone(), dark));
                service.map_slot(id, r.view.slot, revision, want.clone());
            }
            Some(win)
        });
        self.map_lines[pane] = asked.flatten();
    }

    /// Loading, colouring and searching have nothing left for the active document.
    fn is_settled(&self) -> bool {
        self.settled_with(true)
    }

    /// [`is_settled`](Self::is_settled), with or without the minimap's colours: the first frame
    /// does not wait for them (the minimap draws in the text colour until they come), a `--shot`
    /// does.
    fn settled_with(&self, minimap: bool) -> bool {
        let Some(doc) = self.docs.active_doc() else {
            return true;
        };
        match &doc.state {
            DocState::Pending | DocState::Loading => false,
            DocState::Failed(_) => true,
            DocState::Ready(r) => {
                if doc.is_loading() || r.find.searching {
                    return false;
                }
                if !r.colourable {
                    return true;
                }
                // The second pane's colours too.
                let other = self.docs.pane_view(1);
                // The minimaps' windows too, once they have been laid out.
                let dark = self.dark();
                let mapped = |pane: usize, v: &crate::docs::ViewState| {
                    !minimap
                        || self.map_lines[pane]
                            .as_ref()
                            .is_none_or(|w| v.map_colors.covers(w, dark))
                };
                mapped(0, &r.view)
                    && r.view.colors.covers(&view::shown_lines(r), self.dark())
                    && other.is_none_or(|(_, o, v)| {
                        !o.colourable
                            || (v
                                .colors
                                .covers(&view::shown_lines_in(&o.buffer, v), self.dark())
                                && mapped(1, v))
                    })
            }
        }
    }

    // ---- frame -----------------------------------------------------------------------------

    /// The signature of a pane's minimap: what it draws from the document and the pane's view.
    fn minimap_sig(
        &self,
        common: impl Hash,
        pane: usize,
        doc: &crate::docs::Doc,
        shown: Option<(&crate::docs::Ready, &crate::docs::ViewState)>,
    ) -> u64 {
        let state = shown.map(|(r, v)| {
            sig((
                v.map_colors.generation,
                v.colors.generation,
                v.folds.ranges(),
                r.buffer.len_lines(),
                // What the column marks over the rows.
                (v.sel.range(), v.sel.caret, pane == 0),
                (!v.extra.is_empty()).then(|| sig(&v.extra)),
                (r.find.generation, v.find_current, v.scope()),
                &r.bookmarks,
                v.align.as_ref().map(|a| a.stamp),
            ))
        });
        let plan = self.map_plan[pane].map(|p| {
            (
                p.top_px,
                p.rows,
                p.col_h,
                p.band_first.to_bits(),
                p.band_rows.to_bits(),
            )
        });
        let slot = shown.map_or(0, |(_, v)| v.slot);
        let drag = matches!(self.ui.drag, Some(Drag::Minimap { doc: d, slot: s, .. }) if d == doc.id && s == slot);
        sig((common, doc.id, doc.revision, plan, state, self.dark(), drag))
    }

    fn region_sig(&self, r: Region, rect: Option<Rect>) -> u64 {
        let ui = &self.ui;
        let hov = |h: Option<Hit>| h.filter(|h| h.region() == Some(r) && *h != Hit::Text);
        let common = (
            bits(rect),
            hov(ui.hover),
            hov(ui.pressed),
            self.scale.to_bits(),
        );
        match r {
            Region::Title => {
                let missing: Vec<bool> = self.docs.tabs.iter().map(|d| d.missing).collect();
                let drag = match ui.drag {
                    Some(Drag::Tab { index, moving, .. }) => Some((index, moving)),
                    _ => None,
                };
                sig((
                    common,
                    self.maximized,
                    self.docs.labels(),
                    (self.docs.active(), self.docs.other()),
                    missing,
                    drag,
                ))
            }
            Region::Bar => {
                let has_matches = self
                    .docs
                    .active_ready()
                    .is_some_and(|r| !r.find.matches.is_empty());
                sig((
                    common,
                    self.docs.theme.to_setting(),
                    ui.find.text(),
                    ui.find.caret(),
                    ui.find_focused,
                    (
                        ui.find_focused && ui.caret_on,
                        (ui.replace_open, ui.field),
                        (ui.match_case, ui.whole_word, ui.regex),
                        ui.replace.text(),
                        (
                            ui.replace.caret(),
                            self.docs
                                .active_ready()
                                .map(|r| (r.view.replace_scope.clone(), r.view.scope_on)),
                        ),
                        (ui.goto.text(), ui.goto.caret(), ui.naming),
                    ),
                    chrome::find_label(&self.docs),
                    ui.panes.files,
                    ui.popup_kind(),
                    self.docs.active().is_some(),
                    has_matches,
                    self.docs
                        .updates
                        .bar()
                        .map(|s| crate::updater::Updates::bar_label(&crate::updater::APP, s)),
                ))
            }
            Region::Files if ui.outline.on => {
                let drag = matches!(
                    ui.drag,
                    Some(Drag::Splitter { .. })
                        | Some(Drag::Thumb {
                            target: Scroller::Files,
                            ..
                        })
                );
                sig((
                    common,
                    ui.outline.generation,
                    ui.outline.scroll.to_bits(),
                    crate::outline::caret_row(&self.docs, &ui.outline),
                    self.docs.active_doc().map(|d| d.id),
                    drag,
                ))
            }
            Region::Files => {
                let active = self.docs.active_doc().and_then(|d| d.path.clone());
                let drag = matches!(
                    ui.drag,
                    Some(Drag::Splitter { .. })
                        | Some(Drag::Thumb {
                            target: Scroller::Files,
                            ..
                        })
                );
                let rows: Vec<(u16, bool)> = ui
                    .tree
                    .rows()
                    .iter()
                    .map(|r| (r.depth, r.expanded))
                    .collect();
                sig((
                    common,
                    ui.tree.root(),
                    rows,
                    active,
                    ui.panes.files_scroll.to_bits(),
                    drag,
                ))
            }
            Region::Text | Region::Other => {
                let second = r == Region::Other;
                let shown = if !second {
                    self.docs.active()
                } else {
                    self.docs.pane1()
                };
                let Some(doc) = shown.map(|i| &self.docs.tabs[i]) else {
                    return sig((common, "empty", self.docs.recents.paths()));
                };
                let thumbs = match ui.drag {
                    Some(Drag::Thumb { target, .. }) => Some(target),
                    _ => None,
                };
                let state = match &doc.state {
                    DocState::Pending => sig(0u8),
                    DocState::Loading => {
                        let (done, total) = doc.progress();
                        sig((1u8, done * 100 / total.max(1)))
                    }
                    DocState::Failed(e) => sig((2u8, e)),
                    DocState::Ready(r) => {
                        // The view this pane shows: the document's second one in the second
                        // pane of a document shown twice.
                        let v = match (second && self.docs.same_file, r.twin.as_deref()) {
                            (true, Some(t)) => t,
                            _ => &r.view,
                        };
                        let sel = v.sel.range();
                        sig((
                            3u8,
                            (v.top.to_bits(), v.top_row, r.wrap),
                            v.left.to_bits(),
                            (!sel.is_empty()).then_some((sel.start, sel.end)),
                            (!v.extra.is_empty()).then(|| sig(&v.extra)),
                            (!r.bookmarks.is_empty()).then(|| sig(&r.bookmarks)),
                            (!v.folds.is_empty()).then(|| sig(v.folds.ranges())),
                            r.find.generation,
                            v.colors.generation,
                            (v.widest.to_bits(), r.buffer.len_lines()),
                            (v.brackets(&r.buffer, r.revision), v.scope()),
                            v.align.as_ref().map(|a| (a.stamp, a.fresh)),
                        ))
                    }
                };
                sig((
                    common,
                    doc.id,
                    doc.revision,
                    doc.missing,
                    doc.changed_on_disk,
                    state,
                    self.dark(),
                    thumbs,
                    self.view.zoom,
                    self.docs.whitespace,
                ))
            }
            Region::Minimap | Region::MinimapOther => {
                let pane = usize::from(r == Region::MinimapOther);
                let Some((doc, ready, v)) = self.docs.pane_view(pane) else {
                    let Some(doc) = if pane == 0 {
                        self.docs.active()
                    } else {
                        self.docs.pane1()
                    }
                    .map(|i| &self.docs.tabs[i]) else {
                        return sig((common, "empty"));
                    };
                    return self.minimap_sig(common, pane, doc, None);
                };
                self.minimap_sig(common, pane, doc, Some((ready, v)))
            }
            Region::Status => sig((common, status::fields(&self.docs), self.dark())),
            Region::Results => {
                let drag = matches!(
                    ui.drag,
                    Some(Drag::Thumb {
                        target: Scroller::Results,
                        ..
                    })
                );
                sig((
                    common,
                    ui.results.generation,
                    ui.results.scroll.to_bits(),
                    drag,
                    self.dark(),
                    ui.results.replace.sig(),
                    (ui.find_focused && ui.field == BarField::ResultsReplace)
                        .then_some(ui.caret_on),
                    ui.results.replace.field.caret(),
                ))
            }
        }
    }

    /// What the caret looks like: where it is and whether it shows.
    fn caret_state(&self) -> (u64, Option<usize>) {
        let Some(r) = self.docs.active_ready() else {
            return (0, None);
        };
        let visible = self.text_focused() && self.focused && self.ui.caret_on;
        let line = r.buffer.line_of(r.view.sel.caret);
        (
            sig((r.view.sel.caret, visible, r.view.extra.len())),
            Some(line),
        )
    }

    /// Composes one frame into `screen` and returns the device-px rectangles changed.
    fn compose(&mut self) -> (Vec<(i32, i32, i32, i32)>, FrameTimes) {
        let mut times = FrameTimes::default();
        let t0 = Instant::now();
        let (pw, ph) = (self.size.0.max(1), self.size.1.max(1));
        if self.screen.width != pw || self.screen.height != ph {
            self.screen = Bitmap::new(pw, ph);
            // `Bitmap::new` is all zero, alpha included; a pixel no region paints would stay
            // transparent. The window's blit ignores alpha, the client-drawn shadow's surface
            // does not (the compare view with minimaps once left one column unpainted, as black;
            // the layouts tile now, see `a_split_view_leaves_no_pixel_unpainted`).
            // The colour stays black, as it always was.
            for px in self.screen.pixels.as_chunks_mut::<4>().0 {
                px[3] = 255;
            }
            self.full = true;
        }
        self.sync_theme();
        self.compare_sync();
        self.docs.sync_wrap();
        let (w, h) = self.logical();
        self.relayout_areas();
        // Lay the text out first: it clamps the scroll and says which lines want colours.
        let text = self.areas.text;
        let mut geom = None;
        if let Some(doc) = self.docs.active_doc_mut() {
            geom = self.view.layout(doc, text, self.scale);
        }
        let mut geom_other = None;
        if let Some(rect) = self.areas.other {
            let inner = crate::ui::split_inner(
                rect,
                self.docs.other_left,
                1.0 / self.scale,
                self.areas.minimap_other.is_some(),
            );
            let (view, scale) = (&mut self.view, self.scale);
            geom_other = self
                .docs
                .with_pane(1, |doc| view.layout(doc, inner, scale))
                .flatten();
        }
        self.request_colours();
        self.request_colours_other();
        self.map_plan = [self.minimap_plan(0, geom), self.minimap_plan(1, geom_other)];
        self.request_colours_map(0);
        self.request_colours_map(1);
        self.follow_active_file();
        let waker = self.docs.waker();
        self.ui
            .outline
            .sync(&self.docs, self.areas.files.is_some(), &waker);
        times.layout = t0.elapsed();

        let t1 = Instant::now();
        let mut dirty = [self.full; 9];
        for r in REGIONS {
            let rect = self.areas.region(r);
            let s = self.region_sig(r, rect);
            if s != self.sigs[r.index()] {
                self.sigs[r.index()] = s;
                dirty[r.index()] = true;
            }
        }
        for r in REGIONS {
            if self.areas.region(r).is_none() {
                dirty[r.index()] = false;
                self.hits[r.index()].clear();
            }
        }
        // The caret alone: repaint the lines it left and entered.
        let (caret_sig, caret_line) = self.caret_state();
        let ti = Region::Text.index();
        let mut bands: Option<Vec<usize>> = None;
        if caret_sig != self.caret_sig && !dirty[ti] {
            let mut lines: Vec<usize> = self.caret_line.into_iter().chain(caret_line).collect();
            lines.dedup();
            // With word wrap a line's place depends on the rows above it: the whole text is
            // painted.
            // Several carets are on several lines, and folds move the lines below them: the
            // same.
            let wrapped = self.docs.active_ready().is_some_and(|r| {
                r.wrapped() || !r.view.extra.is_empty() || !r.view.folds.is_empty()
            });
            if lines.is_empty() || wrapped {
                dirty[ti] = true;
            } else {
                bands = Some(lines);
            }
        }
        self.caret_sig = caret_sig;

        // Overlays: lay them out, then redraw every region under them if they or those changed.
        let now = Instant::now();
        let pal = self.pal;
        let mut bubbles = Vec::new();
        {
            let mut p = Painter::new(&mut self.screen, &mut self.gfx, self.scale);
            let page = self.areas.text;
            if let Some(text) = self.docs.toast(now) {
                bubbles.push(Bubble::toast(&mut p, text, page));
            }
            if let Some(tip) = self.ui.tip.as_ref().filter(|t| t.shown) {
                bubbles.push(Bubble::tip(&mut p, &tip.text, tip.anchor, w, h));
            }
            if let Some(popup) = self.ui.popup.as_mut() {
                let room = menu_room(self.areas.title, w, h);
                popup.menu.place_in(&mut p, room);
                if let Some(s) = popup.sub.as_mut() {
                    s.menu.place_in(&mut p, room);
                }
            }
        }
        self.toast_until = self.docs.toast(now).and(self.docs.toast_expires());
        let mut bounds: Vec<Rect> = bubbles.iter().map(Bubble::bounds).collect();
        if let Some(popup) = &self.ui.popup {
            bounds.push(popup.menu.rect().inset(-20.0));
            if let Some(s) = &popup.sub {
                bounds.push(s.menu.rect().inset(-20.0));
            }
        }
        if self.ui.dialog.is_some() || self.ui.settings.is_some() {
            bounds.push(Rect::new(0.0, 0.0, w, h));
        }
        if let Some(p) = self.palette.open.as_mut() {
            // The card's place is needed here, to redraw what it covers; drawing lays it out
            // again.
            let card = p
                .palette
                .layout(Rect::new(0.0, 0.0, w, h), crate::ui::TITLE_H);
            bounds.push(card.inset(-20.0));
        }
        let osig = sig((
            bubbles
                .iter()
                .map(|b| {
                    (
                        b.kind,
                        b.text.as_str(),
                        b.rect.x.to_bits(),
                        b.rect.y.to_bits(),
                    )
                })
                .collect::<Vec<_>>(),
            self.ui.popup.as_ref().map(|p| {
                (
                    p.kind,
                    (p.menu.hover, p.menu.scroll().to_bits()),
                    p.menu.items.len(),
                    p.sub
                        .as_ref()
                        .map(|s| (s.kind, s.menu.hover, s.menu.scroll().to_bits())),
                )
            }),
            self.ui
                .settings
                .as_ref()
                .map(|_| (self.ui.overlay_tick, self.ui.caret_on)),
            self.ui.dialog.as_ref().map(|d| {
                (
                    d.kind,
                    d.scroll.to_bits(),
                    self.ui.dlg_hover_row,
                    self.docs.recents.paths().len(),
                    self.docs.updates.about_label(),
                    self.docs.check_updates,
                    self.ui
                        .results
                        .replace
                        .confirm
                        .as_ref()
                        .map(|c| (c.ticks.clone(), c.backup, c.show_skipped, c.cursor)),
                )
            }),
            self.ui
                .dialog
                .as_ref()
                .map(|_| (self.ui.hover, self.ui.pressed)),
            // The palette does not say what changed inside: every event it handled raised the
            // tick; the caret's blink is the one thing that changes alone.
            self.palette
                .open
                .is_some()
                .then_some((self.palette.tick, self.ui.caret_on)),
        ));
        let overlay_changed = osig != self.overlay_sig;
        self.overlay_sig = osig;
        let mut under: Vec<Rect> = bounds.clone();
        under.extend(self.overlay_bounds.iter().copied());
        if bands.is_some() && under.iter().any(|b| overlaps(*b, self.areas.text)) {
            bands = None;
            dirty[ti] = true;
        }
        let any_under_dirty = REGIONS.iter().any(|&r| {
            dirty[r.index()]
                && self
                    .areas
                    .region(r)
                    .is_some_and(|rr| under.iter().any(|b| overlaps(*b, rr)))
        });
        let draw_overlays = !bounds.is_empty() && (overlay_changed || any_under_dirty || self.full);
        if overlay_changed || any_under_dirty {
            for r in REGIONS {
                if let Some(rr) = self.areas.region(r)
                    && under.iter().any(|b| overlaps(*b, rr))
                {
                    dirty[r.index()] = true;
                }
            }
        }
        if dirty[ti] {
            bands = None;
        }
        self.overlay_bounds = bounds;
        if self.ui.dialog.is_none() {
            self.overlay_hits.clear();
        }

        // Draw.
        let maximized = self.maximized;
        let (hover, pressed) = (self.ui.hover, self.ui.pressed);
        let ds = DrawState {
            dark: self.dark(),
            caret: self.text_focused() && self.focused && self.ui.caret_on,
            band: true,
            whitespace: self.docs.whitespace,
            vbar_active: hover == Some(Hit::ScrollBar(Scroller::Text))
                || matches!(
                    self.ui.drag,
                    Some(Drag::Thumb {
                        target: Scroller::Text,
                        ..
                    })
                ),
            other: false,
            hbar_active: hover == Some(Hit::ScrollBar(Scroller::TextH))
                || matches!(
                    self.ui.drag,
                    Some(Drag::Thumb {
                        target: Scroller::TextH,
                        ..
                    })
                ),
        };
        let mut damage = Vec::new();
        {
            let mut p = Painter::new(&mut self.screen, &mut self.gfx, self.scale);
            for r in REGIONS {
                if !dirty[r.index()] {
                    continue;
                }
                let Some(rect) = self.areas.region(r) else {
                    continue;
                };
                damage.push(p.dev_rect(rect));
                let hits = &mut self.hits[r.index()];
                hits.clear();
                let old = p.clip(rect);
                let mut c = Ctx {
                    p: &mut p,
                    pal: &pal,
                    hits,
                    hover,
                    pressed,
                };
                match r {
                    Region::Title => {
                        chrome::draw_title(&mut c, &self.docs, &self.ui, rect, maximized)
                    }
                    Region::Bar => {
                        let bar = Rect::new(rect.x, rect.y, rect.w, crate::ui::BAR_H);
                        chrome::draw_bar(&mut c, &self.docs, &mut self.ui, bar);
                        let mut y = bar.bottom();
                        if self.ui.replace_open {
                            let strip = Rect::new(rect.x, y, rect.w, crate::ui::REPLACE_H);
                            chrome::draw_replace_strip(&mut c, &self.docs, &mut self.ui, strip);
                            y = strip.bottom();
                        }
                        let under = Rect::new(rect.x, y, rect.w, rect.bottom() - y);
                        if under.h > 0.0 {
                            chrome::draw_update_bar(&mut c, &self.docs, under);
                        }
                    }
                    Region::Files if self.ui.outline.on => {
                        crate::outline::draw(&mut c, &self.docs, &mut self.ui, rect)
                    }
                    Region::Files => panes::draw_files(&mut c, &self.docs, &mut self.ui, rect),
                    Region::Text => match self.docs.active() {
                        None => overlays::draw_empty(&mut c, &self.docs, rect),
                        Some(i) => {
                            let doc = &mut self.docs.tabs[i];
                            times.paint = self.view.draw(&mut c, doc, rect, &ds, None);
                        }
                    },
                    Region::Minimap | Region::MinimapOther => {
                        let pane = usize::from(r == Region::MinimapOther);
                        let hit = if pane == 0 {
                            Hit::Minimap
                        } else {
                            Hit::MinimapOther
                        };
                        let other_left = self.docs.other_left;
                        let (map_cache, plan) = (&mut self.map_cache[pane], &self.map_plan[pane]);
                        let drag = self.ui.drag;
                        let look = |hot: bool| minimap::Look {
                            hot,
                            keyboard: pane == 0,
                            right_line: pane == 1 && other_left,
                            hit,
                        };
                        // The second pane of a document shown twice draws from its second view.
                        let drawn = self.docs.with_pane(pane, |doc| {
                            let hot = hover == Some(hit)
                                || matches!(drag, Some(Drag::Minimap { doc: d, slot, .. })
                                    if d == doc.id
                                        && doc.ready().is_some_and(|r| r.view.slot == slot));
                            let ready = doc.ready().map(|r| (r, doc.id, doc.revision));
                            minimap::draw(&mut c, map_cache, ready, plan.as_ref(), rect, look(hot));
                        });
                        if drawn.is_none() {
                            let hot = hover == Some(hit);
                            minimap::draw(&mut c, map_cache, None, plan.as_ref(), rect, look(hot));
                        }
                    }
                    Region::Status => status::draw(&mut c, &self.docs, rect),
                    Region::Results => crate::results::draw(&mut c, &mut self.ui, rect),
                    Region::Other => {
                        if self.docs.pane1().is_some() {
                            // The pane without the keyboard: no caret, and one hit for all
                            // of it instead of the text's own.
                            let (px, left) = (c.p.px(), self.docs.other_left);
                            let inner = crate::ui::split_inner(
                                rect,
                                left,
                                px,
                                self.areas.minimap_other.is_some(),
                            );
                            let quiet = view::DrawState {
                                caret: false,
                                band: false,
                                vbar_active: hover == Some(Hit::ScrollBar(Scroller::OtherText))
                                    || matches!(
                                        self.ui.drag,
                                        Some(Drag::Thumb {
                                            target: Scroller::OtherText,
                                            ..
                                        })
                                    ),
                                hbar_active: hover == Some(Hit::ScrollBar(Scroller::OtherTextH))
                                    || matches!(
                                        self.ui.drag,
                                        Some(Drag::Thumb {
                                            target: Scroller::OtherTextH,
                                            ..
                                        })
                                    ),
                                other: true,
                                ..ds
                            };
                            let view = &mut self.view;
                            self.docs
                                .with_pane(1, |doc| view.draw(&mut c, doc, inner, &quiet, None));
                            let x = if left { rect.right() - px } else { rect.x };
                            // With a minimap beside it that draws the line, where the panes
                            // part, itself.
                            if self.areas.minimap_other.is_none() || !left {
                                c.p.fill(Rect::new(x, rect.y, px, rect.h), pal.line);
                            }
                            // One hit for the pane, under its scrollbars.
                            c.hits.insert(0, (Hit::Other, rect));
                        }
                    }
                }
                p.restore(old);
            }
            if let (Some(lines), Some(i)) = (&bands, self.docs.active()) {
                let rect = self.areas.text;
                let doc = &mut self.docs.tabs[i];
                let area = view::text_area(doc, rect);
                let m = self.view.metrics(self.scale);
                let top = doc.ready().map_or(0.0, |r| r.view.top);
                for &line in lines {
                    let y0 = (area.y * self.scale).round()
                        + metrics::line_y(line, top, m.line_h).round() as f32;
                    let band = Rect::new(
                        area.x,
                        y0 / self.scale,
                        area.w,
                        m.line_h as f32 / self.scale,
                    );
                    let old = p.clip(band);
                    let clip = p.clip(area);
                    let mut c = Ctx {
                        p: &mut p,
                        pal: &pal,
                        hits: &mut Vec::new(),
                        hover,
                        pressed,
                    };
                    times.paint += self.view.draw(&mut c, doc, rect, &ds, Some(&[line]));
                    let (x0, y0, x1, y1) = p.dev_rect(band);
                    let (ax0, ay0, ax1, ay1) = p.dev_rect(area);
                    let d = (x0.max(ax0), y0.max(ay0), x1.min(ax1), y1.min(ay1));
                    if d.0 < d.2 && d.1 < d.3 {
                        damage.push(d);
                    }
                    p.restore(clip);
                    p.restore(old);
                }
            }
            if draw_overlays {
                let mut c = Ctx {
                    p: &mut p,
                    pal: &pal,
                    hits: &mut self.overlay_hits,
                    hover,
                    pressed,
                };
                c.hits.clear();
                if let Some(d) = self.ui.dialog.as_mut() {
                    match &self.ui.results.replace.confirm {
                        Some(cf) if crate::replace_ui::is_confirm(d.kind) => {
                            crate::replace_ui::draw_confirm(
                                &mut c,
                                cf,
                                d,
                                w,
                                h,
                                self.ui.dlg_hover_row,
                            )
                        }
                        _ => overlays::draw_dialog(
                            &mut c,
                            &self.docs,
                            &self.macros.named,
                            d,
                            w,
                            h,
                            self.ui.dlg_hover_row,
                        ),
                    }
                }
                if let Some(f) = self.ui.settings.as_mut() {
                    f.draw(c.p, &pal, Rect::new(0.0, 0.0, w, h), self.ui.caret_on);
                }
                if let Some(popup) = self.ui.popup.as_mut() {
                    let room = menu_room(self.areas.title, w, h);
                    popup.menu.draw_in(c.p, &pal, room);
                    if let Some(s) = popup.sub.as_mut() {
                        s.menu.draw_in(c.p, &pal, room);
                    }
                }
                for b in &bubbles {
                    b.draw(c.p, &pal);
                }
                // Last, over everything.
                if let Some(p) = self.palette.open.as_mut() {
                    let window = Rect::new(0.0, 0.0, w, h);
                    p.palette
                        .draw(c.p, &pal, window, crate::ui::TITLE_H, self.ui.caret_on);
                }
            }
        }
        if dirty[ti] || bands.is_some() {
            self.caret_line = caret_line;
        }
        self.full = false;
        times.compose = t1.elapsed().saturating_sub(times.paint);
        times.rows = damage.iter().map(|d| (d.3 - d.1).max(0) as u32).sum();
        (damage, times)
    }

    /// Selects the active file in the Files tree when it changes.
    fn follow_active_file(&mut self) {
        let active = self.docs.active_doc().and_then(|d| d.path.clone());
        if active == self.last_active {
            return;
        }
        self.last_active = active.clone();
        if let Some(path) = active
            && let Some(i) = self.ui.tree.reveal(&path)
        {
            let top = i as f32 * panes::ROW;
            let view = self.ui.panes.files_view;
            let s = &mut self.ui.panes.files_scroll;
            if view > 0.0 && (top < *s || top + panes::ROW > *s + view) {
                *s = (top - view / 3.0).max(0.0);
            }
        }
    }

    fn draw(&mut self) {
        let (mut damage, mut times) = self.compose();
        if std::mem::take(&mut self.expose) {
            damage = vec![(0, 0, self.screen.width as i32, self.screen.height as i32)];
        }
        let t = Instant::now();
        if !damage.is_empty() {
            times.copy = self.present(&damage);
        }
        times.present = t.elapsed().saturating_sub(times.copy);
        if !damage.is_empty() {
            self.trace.frame(&times);
        }
        if (self.steps.is_some() || self.type_steps.is_some()) && self.settled && !damage.is_empty()
        {
            self.step_times.push(times);
        }
        let title = strings::window_title(self.docs.active_doc().map(|d| d.title()).as_deref());
        if title != self.last_title {
            if let Some(w) = &self.window {
                w.set_title(&title);
            }
            self.last_title = title;
        }
        if self.first {
            self.first = false;
            self.trace.stage("first frame presented hidden");
            if let Some(w) = &self.window {
                w.set_visible(true);
            }
            self.trace.stage("window shown");
            // The first frame went to a hidden window, which shows nothing: send it again.
            let whole = (0, 0, self.screen.width as i32, self.screen.height as i32);
            self.present(&[whole]);
            self.trace
                .line(&format!("first frame after {:.1} ms", self.trace.since()));
            self.trace.memory("first frame");
            #[cfg(windows)]
            if self.trace.on
                && let Some(w) = &self.window
            {
                self.trace.line(&frame_report(w));
            }
            if let Some(ms) = self.args.exit_after_ms {
                self.exit_due = Some(Instant::now() + Duration::from_millis(ms));
            }
            // Later than the first frame: the desktop has not told the window its screen yet.
            if self.args.run.is_some() {
                self.run_due = Some(Instant::now() + RUN_DELAY);
            }
        }
    }

    /// Copies the damaged rectangles to the window and presents them. Returns the copy time.
    fn present(&mut self, damage: &[(i32, i32, i32, i32)]) -> Duration {
        let Some(surface) = self.surface.as_mut() else {
            return Duration::ZERO;
        };
        let (w, h) = (self.screen.width, self.screen.height);
        // The surface is the content plus the shadow's margin on every side (0 without one:
        // everything below is then as it always was).
        let m = self.shadow.as_ref().map_or(0, |s| s.margin());
        let (sw, sh) = (w + 2 * m, h + 2 * m);
        if surface
            .resize(NonZeroU32::new(sw).unwrap(), NonZeroU32::new(sh).unwrap())
            .is_err()
        {
            return Duration::ZERO;
        }
        let Ok(mut buffer) = surface.buffer_mut() else {
            return Duration::ZERO;
        };
        let t = Instant::now();
        // The back buffer holds the previous frame only when its age is exactly 1. softbuffer's
        // Wayland backend swaps two buffers (age 2 in steady state) and a resized buffer keeps
        // its age, so any other age means stale pixels: copy and present the whole frame. On
        // Windows the age is only 0 or 1, so nothing changes there.
        let age = buffer.age();
        let mut whole = age != 1;
        // The margin is painted when this buffer's one is stale (a new size, scale, focus or
        // margin: both buffers, one frame each); the compositor then gets the whole surface.
        let margin_painted = self
            .shadow
            .as_mut()
            .is_some_and(|s| s.paint_frame(&mut buffer, sw, sh, age));
        let clamp = |d: &(i32, i32, i32, i32)| {
            let x0 = d.0.clamp(0, w as i32) as usize;
            let x1 = d.2.clamp(0, w as i32) as usize;
            let y0 = d.1.clamp(0, h as i32) as usize;
            let y1 = d.3.clamp(0, h as i32) as usize;
            (x0, y0, x1, y1)
        };
        let stride = w as usize;
        let (surface_stride, m) = (sw as usize, m as usize);
        // Both are BGRX in memory. The content's alpha byte is always 255 (the blit to a plain
        // window ignores it; the client-drawn margin needs it). A content row lands `m` rows down
        // and `m` columns in.
        let screen = &self.screen;
        let copy = |buffer: &mut [u32], (x0, y0, x1, y1): (usize, usize, usize, usize)| {
            let bytes: &mut [u8] = bytemuck::cast_slice_mut(buffer);
            for y in y0..y1 {
                let (a, b) = ((y * stride + x0) * 4, (y * stride + x1) * 4);
                let to = ((y + m) * surface_stride + x0 + m) * 4;
                bytes[to..to + (b - a)].copy_from_slice(&screen.pixels[a..b]);
            }
        };
        whole |= margin_painted;
        let rects: Vec<softbuffer::Rect> = if whole {
            // Content: the whole bitmap when the buffer is stale, else only the damage (the buffer
            // holds the last frame); the margin changes only when it was just painted.
            if age != 1 {
                copy(&mut buffer, (0, 0, w as usize, h as usize));
            } else {
                for d in damage {
                    copy(&mut buffer, clamp(d));
                }
            }
            vec![softbuffer::Rect {
                x: 0,
                y: 0,
                width: NonZeroU32::new(sw).unwrap(),
                height: NonZeroU32::new(sh).unwrap(),
            }]
        } else {
            damage
                .iter()
                .filter_map(|d| {
                    let r = clamp(d);
                    copy(&mut buffer, r);
                    Some(softbuffer::Rect {
                        x: (r.0 + m) as u32,
                        y: (r.1 + m) as u32,
                        width: NonZeroU32::new((r.2 - r.0) as u32)?,
                        height: NonZeroU32::new((r.3 - r.1) as u32)?,
                    })
                })
                .collect()
        };
        let copied = t.elapsed();
        let _ = buffer.present_with_damage(&rects);
        copied
    }

    // ---- hit testing -------------------------------------------------------------------------

    fn hit_at(&self, x: f32, y: f32) -> Option<Hit> {
        // The palette does its own hit testing.
        if self.palette_open() {
            return None;
        }
        if self.ui.dialog.is_some() {
            return self
                .overlay_hits
                .iter()
                .rev()
                .find(|(_, r)| r.contains(x, y))
                .map(|(h, _)| *h);
        }
        // The line between two panes is not drawn by a region: it is where the layout says.
        if let (Some(d), Some(_)) = (self.areas.divider, self.areas.other)
            && d.grab(self.areas.text.y, self.areas.text.h).contains(x, y)
        {
            return Some(Hit::Divider);
        }
        for r in REGIONS {
            if let Some(h) = self.hits[r.index()]
                .iter()
                .rev()
                .find(|(_, rr)| rr.contains(x, y))
            {
                return Some(h.0);
            }
        }
        None
    }

    fn hit_rect(&self, hit: Hit) -> Option<Rect> {
        self.hits
            .iter()
            .chain(std::iter::once(&self.overlay_hits))
            .flat_map(|v| v.iter())
            .find(|(h, _)| *h == hit)
            .map(|(_, r)| *r)
    }

    fn edge_at(&self, x: f32, y: f32) -> Option<ResizeDirection> {
        if self.maximized || self.window.is_none() {
            return None;
        }
        let (w, h) = self.logical();
        mdshared_ui::window::edge_at(x, y, w, h)
    }

    fn set_cursor(&mut self, c: CursorIcon) {
        if c != self.cursor {
            self.cursor = c;
            if let Some(w) = &self.window {
                w.set_cursor(c);
            }
        }
    }

    // ---- session -----------------------------------------------------------------------------

    /// What the session file holds, as a hash.
    fn session_sig(&self) -> u64 {
        sig((
            self.docs.paths(),
            (self.docs.active_among_paths(), self.split_state()),
            self.docs.recents.paths(),
            (!self.theme_from_args).then_some(self.docs.theme),
            (self.args.zoom.is_none()).then_some(self.docs.zoom),
            (
                self.docs.whitespace || self.args.whitespace,
                self.docs.wrap || self.args.wrap,
                self.docs.minimap || self.args.minimap,
            ),
            self.ui.panes.files,
            self.ui.panes.files_w.round() as u32,
            &self.docs.folder,
            &self.session.geometry,
            (
                self.docs.check_updates,
                self.session.update_checked,
                self.docs.prefs.serialize(),
            ),
            (
                (self.ui.match_case, self.ui.whole_word, self.ui.regex),
                self.ui.find_recall.items(),
                self.ui.replace_recall.items(),
            ),
        ))
    }

    /// The split view as the session keeps it: the second pane's file (a document without a
    /// file is not kept), its side and the share of the width.
    fn split_state(&self) -> Option<crate::session::Split> {
        let path = self.docs.tabs[self.docs.pane1()?].path.clone()?;
        Some(crate::session::Split::new(
            path,
            self.docs.other_left,
            self.docs.split_ratio,
        ))
    }

    /// Writes the session when it changed since the last write; not during a drag (the Files
    /// pane's width moves with it).
    fn persist_if_changed(&mut self) {
        if !self.save || self.session_path.is_none() || self.ui.drag.is_some() {
            return;
        }
        if self.session_sig() != self.saved_sig {
            self.save_session();
        }
    }

    /// Where unsaved text is kept between runs: `backup` beside the session file.
    pub(crate) fn backup_dir(&self) -> Option<PathBuf> {
        let path = self.session_path.as_deref().filter(|_| self.save)?;
        backup_dir_of(path)
    }

    fn save_session(&mut self) {
        let Some(path) = self.session_path.clone().filter(|_| self.save) else {
            return;
        };
        let split = self.split_state();
        let s = &mut self.session;
        s.tabs = self.docs.paths();
        s.active = self.docs.active_among_paths();
        s.split = split;
        s.recent = self.docs.recents.clone();
        if !self.theme_from_args {
            s.theme = self.docs.theme;
        }
        if self.args.zoom.is_none() {
            s.zoom = self.docs.zoom;
        }
        if !self.args.whitespace {
            s.whitespace = self.docs.whitespace;
        }
        if !self.args.wrap {
            s.wrap = self.docs.wrap;
        }
        if !self.args.minimap {
            s.minimap = self.docs.minimap;
        }
        s.check_updates = self.docs.check_updates;
        s.find_options = (self.ui.match_case, self.ui.whole_word, self.ui.regex);
        s.find_history = self.ui.find_recall.items().to_vec();
        s.replace_history = self.ui.replace_recall.items().to_vec();
        // The function list is not kept: the pane comes back with the Files tree, or not at all.
        s.files = Some(self.ui.panes.files && !self.ui.outline.on);
        s.files_w = Some(self.ui.panes.files_w.round() as u32);
        s.folder = self.docs.folder.clone();
        if let Err(e) = s.save(&path) {
            eprintln!("mdedit: could not save the session: {e}");
        }
        self.saved_sig = self.session_sig();
    }
}

fn backup_dir_of(session: &Path) -> Option<PathBuf> {
    session.parent().map(|d| d.join("backup"))
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

#[cfg(windows)]
fn hwnd(w: &Window) -> Option<isize> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match w.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

#[cfg(windows)]
fn style(w: &Window, dark: bool) {
    if let Some(h) = hwnd(w) {
        platform::style_window(h, dark);
    }
}

#[cfg(windows)]
fn frame_report(w: &Window) -> String {
    hwnd(w).map(platform::frame_report).unwrap_or_default()
}

impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let (w, h, max) = self.session.window_size().unwrap_or((1280, 800, false));
        let title = strings::window_title(self.docs.active_doc().map(|d| d.title()).as_deref());
        let mut attributes = Window::default_attributes()
            .with_title(title)
            .with_decorations(false)
            .with_visible(false)
            .with_inner_size(LogicalSize::new(w.max(520) as f64, h.max(360) as f64))
            .with_maximized(max || self.args.maximize);
        // The window class and icon: off Linux this returns the attributes unchanged.
        attributes = mdshared_ui::window::identify(
            attributes,
            "mdedit",
            Some(include_bytes!("../../../assets/mdedit.png")),
        );
        // Back where it was, if a screen still shows that place: the title row's left end has
        // to be on one, or the window could not be reached.
        if let Some((x, y)) = self.session.window_position() {
            let reachable = event_loop.available_monitors().any(|monitor| {
                let (origin, size) = (monitor.position(), monitor.size());
                let (px, py) = (x + 60, y + 16);
                px >= origin.x
                    && px < origin.x + size.width as i32
                    && py >= origin.y
                    && py < origin.y + size.height as i32
            });
            if reachable {
                attributes = attributes.with_position(PhysicalPosition::new(x, y));
            }
        }
        #[cfg(windows)]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            attributes = attributes.with_undecorated_shadow(true);
        }
        // Transparent, and `with_inner_size` (the content size) grown by the margin, only where
        // the client-drawn shadow will be used (not on KDE, not on Windows): the attributes come
        // back unchanged there.
        let attributes = mdshared_ui::shadow::Shadow::prepare(
            attributes,
            mdshared_ui::shadow::Mode::from_env(),
            event_loop,
        );
        let window = match event_loop.create_window(attributes) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                eprintln!("mdedit: no window: {e}");
                event_loop.exit();
                return;
            }
        };
        self.trace.stage("window created");
        self.scale = window.scale_factor() as f32;
        self.maximized = window.is_maximized();
        self.shadow = mdshared_ui::shadow::Shadow::attach_mode(
            &window,
            mdshared_ui::shadow::Mode::from_env(),
        );
        // The minimum is a content size whatever the route.
        let min = LogicalSize::new(520.0, 360.0);
        match &self.shadow {
            Some(s) => s.set_min_content_size(&window, min),
            None => window.set_min_inner_size(Some(min)),
        }
        let size = window.inner_size();
        self.size = self.content_size(size.width, size.height);
        #[cfg(windows)]
        style(&window, self.dark());
        let surface = softbuffer::Context::new(window.clone())
            .and_then(|context| softbuffer::Surface::new(&context, window.clone()));
        match surface {
            Ok(mut s) => {
                // The client-drawn margin is transparent: Wayland buffers must carry alpha (the
                // content bitmap's alpha is always 255). A no-op elsewhere.
                if self.shadow.as_ref().is_some_and(|s| s.wants_alpha()) {
                    s.set_premultiplied_alpha(true);
                }
                self.surface = Some(s);
            }
            Err(e) => {
                eprintln!("mdedit: no surface: {e}");
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window);
        self.trace.stage("surface");
        // The hidden first frame waits a little for the text and its colours.
        let dark = self.dark();
        let applied = self
            .docs
            .wait_load(self.started + LOAD_WAIT, self.service.as_ref(), dark);
        if applied.active_ready {
            self.trace_load();
        }
        self.drain();
        let colour_deadline = self.started + COLOUR_WAIT;
        loop {
            self.compose_layout_only();
            let want =
                self.docs.active_ready().is_some_and(|r| r.colourable) && !self.settled_with(false);
            if !want || Instant::now() >= colour_deadline {
                break;
            }
            let Some(s) = &self.service else { break };
            let colored = s.wait(colour_deadline);
            if !colored.is_empty() {
                self.docs.colored(colored);
                self.trace_colours();
            }
        }
        self.draw();
        // `--place`, for the launch that opened this window.
        if let Some(rect) = self.args.place {
            self.place(rect);
        }
    }

    fn user_event(&mut self, _: &ActiveEventLoop, wake: Wake) {
        self.woken(wake);
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.request(After::Exit);
                self.redraw();
            }
            WindowEvent::RedrawRequested => {
                self.wants_frame = false;
                if !std::mem::take(&mut self.requested) {
                    self.expose = true;
                }
                self.draw();
            }
            WindowEvent::Moved(position) => {
                if let Some(w) = &self.window
                    && !w.is_maximized()
                    && w.is_minimized() != Some(true)
                    && w.fullscreen().is_none()
                {
                    self.session.set_window_position(position.x, position.y);
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(w) = &self.window {
                    self.maximized = w.is_maximized();
                }
                self.update_shadow(None);
                // The window reports the surface; the app lives in the content inside the margin
                // (the margin in force was just re-read by `update_shadow`).
                self.size = self.content_size(size.width, size.height);
                self.size_save_due = Some(Instant::now() + SIZE_SAVE_DELAY);
                self.redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor as f32;
                self.update_shadow(None);
                if let Some(w) = &self.window {
                    let s = w.inner_size();
                    self.size = self.content_size(s.width, s.height);
                }
                self.full = true;
                self.redraw();
            }
            WindowEvent::ThemeChanged(t) => {
                self.system_dark = t == WinTheme::Dark;
                self.full = true;
                self.redraw();
            }
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                self.update_shadow(Some(focused));
                if focused && self.ui.tree.refresh() {
                    self.redraw();
                }
                if focused {
                    self.caret_reset();
                } else {
                    self.ui.caret_on = true;
                    self.ui.tip = None;
                    self.docs.seal();
                }
                self.redraw();
            }
            WindowEvent::ModifiersChanged(m) => self.mods = Mods::from_winit(m.state()),
            WindowEvent::CursorMoved {
                position: PhysicalPosition { x, y },
                ..
            } => {
                // Surface coordinates to content coordinates (the margin is 0 without the
                // client-drawn shadow): negative or past the content in the outer resize band.
                let (x, y) = match &self.shadow {
                    Some(s) => s.to_content(x, y),
                    None => (x, y),
                };
                self.pointer_moved(x as f32 / self.scale, y as f32 / self.scale);
            }
            WindowEvent::CursorLeft { .. } => {
                if self.ui.drag.is_none() {
                    self.ui.hover = None;
                    self.ui.tip = None;
                    self.redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => match (state, button) {
                (ElementState::Pressed, MouseButton::Left) => self.left_press(),
                (ElementState::Released, MouseButton::Left) => self.left_release(),
                (ElementState::Pressed, MouseButton::Right) => self.right_press(),
                (ElementState::Pressed, MouseButton::Middle) => self.middle_press(),
                _ => {}
            },
            WindowEvent::MouseWheel { delta, .. } => self.wheel(delta),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                if matches!(event.logical_key, Key::Named(NamedKey::Alt)) {
                    return;
                }
                self.key(&event);
            }
            WindowEvent::DroppedFile(path) => self.dropped(path),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.drain() {
            self.redraw();
        }
        if self.exit {
            // The viewer goes back to the files it was showing the text of.
            self.docs.links.finish();
            self.save_size();
            self.save_session();
            self.trace.cpu("exit");
            event_loop.exit();
            return;
        }
        if self.docs.watch_changed {
            self.docs.watch_changed = false;
            if let Some(w) = self.watcher.as_mut() {
                let paths = self.docs.paths();
                w.set_files(paths.iter().map(PathBuf::as_path));
            }
        }
        self.persist_if_changed();
        self.link_due = self.docs.link_poll(Instant::now());
        if !self.settled && !self.first && self.is_settled() {
            self.settled = true;
            self.trace.stage("loading and colouring idle");
            self.trace.memory("idle");
            self.trace.cpu("idle");
        }
        if self.type_steps.is_some() && self.settled {
            self.type_step();
            if self.exit {
                return;
            }
            event_loop.set_control_flow(ControlFlow::Poll);
            return;
        }
        if self.steps.is_some() && self.settled {
            self.step();
            if self.exit {
                return;
            }
            event_loop.set_control_flow(ControlFlow::Poll);
            return;
        }
        let next = self.timers(Instant::now());
        if self.exit {
            return;
        }
        if self.wants_frame {
            self.wants_frame = false;
            if let Some(w) = &self.window {
                self.requested = true;
                w.request_redraw();
            }
        }
        event_loop.set_control_flow(match next {
            Some(t) => ControlFlow::WaitUntil(t),
            None => ControlFlow::Wait,
        });
    }
}

impl App {
    /// The height of what shows under the command bar: the replace strip, then the update bar.
    fn notice_h(&self) -> f32 {
        let strip = if self.ui.replace_open {
            crate::ui::REPLACE_H
        } else {
            0.0
        };
        let update = if self.docs.updates.bar().is_some() {
            view::BANNER_H
        } else {
            0.0
        };
        strip + update
    }

    /// Works out where everything goes now (the split's roles and share, the minimap, the
    /// panels) and makes it the current layout; a change repaints everything.
    pub(super) fn relayout_areas(&mut self) {
        let (w, h) = self.logical();
        let areas = Areas::compute(
            w,
            h,
            self.ui.panes.files,
            self.ui.panes.files_w,
            self.notice_h(),
        )
        .with_results(self.ui.results.open);
        // A split whose second tab was closed, or became the active one, is over.
        if self.docs.other.is_some() && self.docs.pane1().is_none() {
            self.docs.other = None;
        }
        let areas = areas
            .with_split(
                self.docs.other.is_some(),
                self.docs.other_left,
                self.docs.split_ratio,
            )
            .with_minimap(self.docs.minimap && self.docs.active().is_some());
        if areas != self.areas {
            self.areas = areas;
            self.full = true;
        }
    }

    /// The layout pass of a frame without drawing: areas, scroll clamping, colour requests.
    fn compose_layout_only(&mut self) {
        self.sync_theme();
        self.compare_sync();
        self.docs.sync_wrap();
        let (w, h) = self.logical();
        self.areas = Areas::compute(
            w,
            h,
            self.ui.panes.files,
            self.ui.panes.files_w,
            self.notice_h(),
        )
        .with_results(self.ui.results.open)
        .with_minimap(self.docs.minimap && self.docs.active().is_some());
        let text = self.areas.text;
        let mut geom = None;
        if let Some(doc) = self.docs.active_doc_mut() {
            geom = self.view.layout(doc, text, self.scale);
        }
        if let Some(rect) = self.areas.other {
            let inner = crate::ui::split_inner(
                rect,
                self.docs.other_left,
                1.0 / self.scale,
                self.areas.minimap_other.is_some(),
            );
            let (view, scale) = (&mut self.view, self.scale);
            self.docs.with_pane(1, |doc| view.layout(doc, inner, scale));
        }
        self.request_colours();
        self.request_colours_other();
        self.map_plan = [self.minimap_plan(0, geom), None];
        self.request_colours_map(0);
        self.request_colours_map(1);
    }

    /// Applies `--goto`, `--select`, `--find`, `--scroll` and `--hscroll` once the active
    /// document is ready and laid out.
    fn apply_positions(&mut self) {
        let a = self.args.clone();
        let line_h = self.view.metrics(self.scale).line_h;
        let scale = self.scale;
        if a.split_same && self.docs.show_in_other_pane() {
            self.docs.other_left = a.split_left;
            self.docs.split_ratio = a.split_ratio.unwrap_or(0.5);
        }
        if let Some(r) = self.docs.active_ready_mut() {
            let at = |r: &crate::docs::Ready, lc: crate::args::LineCol| {
                let pos = mdedit_core::Position {
                    line: lc.line - 1,
                    col: lc.col - 1,
                };
                r.buffer.snap_to_grapheme(r.buffer.offset(pos))
            };
            for &n in &a.bookmarks {
                let line = n.clamp(1, r.buffer.len_lines().max(1)) - 1;
                if let Err(k) = r.bookmarks.binary_search(&line) {
                    r.bookmarks.insert(k, line);
                }
            }
            for &n in &a.folds {
                let line = n.clamp(1, r.buffer.len_lines().max(1)) - 1;
                if let Some(end) =
                    mdedit_core::fold::block_end(&r.buffer, line, crate::live::tab_width())
                {
                    r.view.folds.fold(line, end);
                }
            }
            if let Some(lc) = a.goto {
                let o = at(r, lc);
                r.view.sel = mdedit_core::Selection::at(o);
                view::reveal(r, o, true);
            }
            if let Some((from, to)) = a.select {
                let (f, t) = (at(r, from), at(r, to));
                r.view.sel = mdedit_core::Selection {
                    anchor: f,
                    caret: t,
                    goal_col: None,
                };
                view::reveal(r, t, false);
            }
            if let Some((from, to)) = a.select_column {
                r.select_column(
                    (from.line.saturating_sub(1), from.col.saturating_sub(1)),
                    (to.line.saturating_sub(1), to.col.saturating_sub(1)),
                );
                view::reveal(r, r.view.sel.caret, false);
            }
        }
        if a.replace.is_some() {
            // The strip opens over the selection, as the Replace command would have it.
            self.docs.capture_replace_scope(false);
        }
        if let Some(text) = &a.type_text {
            self.docs.edit(EditOp::Insert(text), self.service.as_ref());
        }
        if let Some(keys) = &a.type_keys {
            for c in keys.chars() {
                let mut buf = [0; 4];
                let op = if c == '\n' {
                    EditOp::Newline
                } else {
                    EditOp::Insert(c.encode_utf8(&mut buf))
                };
                self.docs.edit(op, self.service.as_ref());
            }
        }
        if let Some(q) = &a.find {
            self.ui.find.set_text(q);
            self.ui.match_case = a.match_case;
            self.ui.whole_word = a.whole_word;
            self.ui.regex = a.regex;
            self.docs.find_set(q, self.ui.options());
            if a.find_in_tabs {
                self.find_in_tabs();
            }
        }
        if a.outline {
            self.ui.panes.files = true;
            self.ui.outline.on = true;
            self.ui.outline.fill_now(&self.docs);
        }
        if let Some(with) = &a.replace {
            self.ui.replace.set_text(with);
            self.ui.replace_open = true;
        }
        if let Some(r) = self.docs.active_ready_mut() {
            if let Some(px) = a.scroll {
                view::scroll_rows(r, (px * scale) as f64 / line_h as f64);
            }
            if let Some(px) = a.hscroll {
                r.view.left = px as f64;
            }
        }
        self.apply_other_positions();
        self.compose_layout_only();
    }

    /// `--other-goto`, `--other-select`, `--other-fold` and `--other-scroll`: the second pane's
    /// view of the document, placed as the first's flags place the first.
    fn apply_other_positions(&mut self) {
        let a = self.args.clone();
        let (line_h, scale) = (self.view.metrics(self.scale).line_h, self.scale);
        self.docs.with_pane(1, |doc| {
            let Some(r) = doc.ready_mut() else { return };
            let at = |r: &crate::docs::Ready, lc: crate::args::LineCol| {
                let pos = mdedit_core::Position {
                    line: lc.line - 1,
                    col: lc.col - 1,
                };
                r.buffer.snap_to_grapheme(r.buffer.offset(pos))
            };
            for &n in &a.other_folds {
                let line = n.clamp(1, r.buffer.len_lines().max(1)) - 1;
                if let Some(end) =
                    mdedit_core::fold::block_end(&r.buffer, line, crate::live::tab_width())
                {
                    r.view.folds.fold(line, end);
                }
            }
            if let Some(lc) = a.other_goto {
                let o = at(r, lc);
                r.view.sel = mdedit_core::Selection::at(o);
                view::reveal(r, o, true);
            }
            if let Some((from, to)) = a.other_select {
                let (f, t) = (at(r, from), at(r, to));
                r.view.sel = mdedit_core::Selection {
                    anchor: f,
                    caret: t,
                    goal_col: None,
                };
                view::reveal(r, t, false);
            }
            if let Some(px) = a.other_scroll {
                view::scroll_rows(r, (px * scale) as f64 / line_h as f64);
            }
        });
    }
}

/// Renders one state to a PNG without a window (`--shot`).
fn shot(mut app: App, path: &Path) -> i32 {
    let (lw, lh) = app.args.size.unwrap_or((1280, 800));
    app.scale = app.args.scale.unwrap_or_else(platform::system_scale);
    app.size = (
        (lw as f32 * app.scale).round() as u32,
        (lh as f32 * app.scale).round() as u32,
    );
    let dark = app.dark();
    app.sync_theme();
    if app.args.split {
        app.docs.toggle_split();
        if app.docs.other.is_some() {
            app.docs.other_left = app.args.split_left;
            app.docs.split_ratio = app.args.split_ratio.unwrap_or(0.5);
        }
    }
    for i in 0..app.docs.tabs.len() {
        app.docs.load_now(i, app.service.as_ref(), dark);
    }
    app.compose_layout_only();
    app.apply_positions();
    // The mark of a preview, with nothing started or sent.
    if app.args.demo_link
        && let Some((id, path)) = app
            .docs
            .active_doc()
            .and_then(|d| Some((d.id, d.path.clone()?)))
    {
        app.docs.links.link(id, path, None, None, Instant::now());
    }
    app.shot_compare();
    // Wait for the colours of the visible lines.
    let deadline = Instant::now() + SHOT_COLOUR_WAIT;
    loop {
        app.drain();
        app.compose_layout_only();
        if app.is_settled() || Instant::now() >= deadline {
            break;
        }
        if let Some(s) = &app.service {
            let colored = s.wait(deadline.min(Instant::now() + Duration::from_millis(50)));
            app.docs.colored(colored);
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    app.compose();
    if let Some(t) = app.args.macro_prompt.clone() {
        app.ui.naming = true;
        app.ui.focus_field(crate::ui::BarField::Goto);
        app.ui.goto.set_text(&t);
    }
    let text = app.areas.text;
    match app.args.menu {
        Some(ShotMenu::Main) => app.open_popup(PopupKind::Main),
        Some(ShotMenu::MainEnd) => {
            app.open_popup(PopupKind::Main);
            if let Some(p) = app.ui.popup.as_mut() {
                p.menu.scroll_to_end();
            }
        }
        Some(ShotMenu::Recent) => app.open_popup(PopupKind::Recent),
        Some(ShotMenu::MainView) => {
            app.open_popup(PopupKind::Main);
            app.compose();
            if let Some(p) = app.ui.popup.as_mut() {
                p.menu.hover = p.menu.items.iter().position(|i| i.id == Act::ViewMenu);
            }
            app.open_sub(PopupKind::Main, PopupKind::View, true);
        }
        Some(ShotMenu::MainMacro) => {
            app.open_popup(PopupKind::Main);
            app.compose();
            if let Some(p) = app.ui.popup.as_mut() {
                p.menu.hover = p.menu.items.iter().position(|i| i.id == Act::MacroMenu);
            }
            app.open_sub(PopupKind::Main, PopupKind::Macro, true);
        }
        Some(ShotMenu::View) => app.open_popup(PopupKind::View),
        Some(ShotMenu::Macro) => app.open_popup(PopupKind::Macro),
        Some(ShotMenu::MacroRec) => {
            app.macros.recording = Some(Vec::new());
            app.open_popup(PopupKind::Macro);
        }
        Some(ShotMenu::Minimap) => {
            if let Some(r) = app.hit_rect(Hit::Minimap) {
                app.ui.mouse = (r.x + r.w * 0.5, r.y + 120.0);
                app.right_press();
            }
        }
        Some(ShotMenu::Eol) => app.open_popup(PopupKind::Eol),
        Some(ShotMenu::Encoding) => app.open_popup(PopupKind::Encoding),
        Some(ShotMenu::Tab) => {
            if let Some(r) = app.hit_rect(Hit::Tab(0)) {
                app.ui.mouse = (r.x + 30.0, r.cy());
                app.right_press();
            }
        }
        Some(ShotMenu::Text) => {
            app.ui.mouse = (
                text.x + text.w * app.args.menu_at.unwrap_or(0.3),
                text.y + 160.0,
            );
            app.right_press();
        }
        Some(m @ (ShotMenu::TextLines | ShotMenu::TextConvert | ShotMenu::TextMarkdown)) => {
            app.ui.mouse = (
                text.x + text.w * app.args.menu_at.unwrap_or(0.3),
                text.y + 160.0,
            );
            app.right_press();
            app.compose();
            let (row, sub) = match m {
                ShotMenu::TextLines => (7, PopupKind::Lines),
                ShotMenu::TextConvert => (8, PopupKind::Convert),
                _ => (6, PopupKind::Markdown),
            };
            if let Some(p) = app.ui.popup.as_mut() {
                p.menu.hover = Some(row);
            }
            app.open_sub(PopupKind::Text, sub, true);
        }
        Some(ShotMenu::Find) => {
            if let Some(r) = app.hit_rect(Hit::Find) {
                app.ui.mouse = (r.x + 60.0, r.cy());
                app.right_press();
            }
        }
        None => {}
    }
    match app.args.dialog {
        Some(ShotDialog::About) => app.open_dialog(DialogKind::About),
        Some(ShotDialog::Shortcuts) => app.open_dialog(DialogKind::Shortcuts),
        Some(ShotDialog::Recent) => app.open_dialog(DialogKind::Recent),
        Some(ShotDialog::Macros) => app.open_dialog(DialogKind::Macros),
        Some(ShotDialog::Settings) => {
            app.open_settings();
            if let Some(name) = app.args.settings_section.clone() {
                // The form lays its rows out when it is drawn: once, then scroll to the section.
                app.compose();
                if let Some(f) = app.ui.settings.as_mut() {
                    let at = f
                        .categories()
                        .iter()
                        .position(|c| c.eq_ignore_ascii_case(&name));
                    if let Some(at) = at {
                        f.jump_to_category(at);
                    }
                }
            }
        }
        Some(ShotDialog::Unsaved) => {
            if let Some(id) = app.docs.active_doc().map(|d| d.id) {
                app.ask(DialogKind::Unsaved, id, After::Nothing);
            }
        }
        None => {}
    }
    if let Some(q) = app.args.palette.clone() {
        app.open_palette(&q);
        app.settle_palette();
    }
    app.shot_replace();
    if app.ui.dialog.is_some() || app.ui.popup.is_some() || app.palette_open() {
        app.compose();
    }
    if let Some((x, y)) = app.args.hover {
        app.pointer_moved(x, y);
        // What the pointer is over can change what is drawn and hit (the fold chevrons).
        app.compose();
        app.pointer_moved(x, y);
        if let Some(t) = app.ui.tip.as_mut() {
            t.shown = true;
        }
        if let Some(p) = app.ui.popup.as_mut() {
            p.menu.pointer(x, y);
        }
    }
    // The caret in its visible phase, and no toast: a shot is the same on every run.
    app.ui.caret_on = true;
    app.docs.clear_toast();
    app.full = true;
    app.compose();
    match app.screen.write_png(path) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("mdedit: could not write {}: {e}", path.display());
            1
        }
    }
}

/// Starts the editor; the process exit code.
pub fn run() -> i32 {
    let started = Instant::now();
    let args = Args::parse(std::env::args().skip(1));
    let trace = Trace::new(args.trace, started);
    trace.stage("arguments parsed");
    if args.check_update {
        return crate::updater::check_and_print();
    }
    // From an AppImage this writes the desktop entry and icon (on a thread); elsewhere nothing.
    mdshared_ui::desktop_entry::ensure(mdshared_ui::desktop_entry::Spec {
        name: "mdedit",
        display_name: "mdedit",
        comment: "Text editor",
        icon_png: include_bytes!("../../../assets/mdedit.png"),
        mime_types: mdshared_ui::desktop_entry::EDITOR_MIME_TYPES,
        categories: &["Utility", "TextEditor", "Development"],
    });
    let explicit = std::env::var_os("MDEDIT_SETTINGS").map(PathBuf::from);
    // A shot never touches the user's session; it may read a scratch one named by MDEDIT_SETTINGS.
    let session_path = match (args.no_session, args.shot.is_some()) {
        (true, _) => None,
        (false, true) => explicit,
        (false, false) => {
            explicit.or_else(|| mdshared_app::settings::default_settings_path("mdedit"))
        }
    };
    if let Some(settings) = session_path.as_ref().filter(|_| args.shot.is_none()) {
        // A window is already open: hand it the files and quit, before any window work. A launch
        // that asks for a line, a place or a follow hands each file over with that.
        let mut requests = args.requests();
        // `--folder` goes over as one more path: the running window opens it as its folder.
        let mut paths = args.files.clone();
        if let Some(dir) = &args.folder {
            paths.push(dir.clone());
            if !requests.is_empty() {
                requests.push(Request::new(dir));
            }
        }
        let handed = if requests.is_empty() {
            mdshared_app::single::forward("mdedit", settings, &paths)
        } else {
            requests
                .iter()
                .all(|r| mdshared_app::single::forward_request("mdedit", settings, r))
        };
        if handed {
            return 0;
        }
    }
    // A live update is for a window that shows the file already: with none, there is nothing to do.
    if args.follow && args.shot.is_none() {
        return 0;
    }
    let session = session_path
        .as_deref()
        .map(Session::load)
        .unwrap_or_default();
    trace.stage("session read");

    // The proxy exists only once the event loop does; until then wakes are dropped and the
    // loop drains the channels when it starts.
    let proxy: Arc<Mutex<Option<EventLoopProxy<Wake>>>> = Arc::new(Mutex::new(None));
    let cell = proxy.clone();
    let waker: Waker = Arc::new(move |w| {
        if let Ok(p) = cell.lock()
            && let Some(p) = p.as_ref()
        {
            let _ = p.send_event(w);
        }
    });
    let mut docs = Docs::new(platform::code_page(), waker.clone());
    // The preferences: the file's, with any `--pref` of this run over them.
    let mut session = session;
    if !args.prefs.is_empty() {
        let text = format!("{}\n{}", session.prefs.serialize(), args.prefs.join("\n"));
        session.prefs = mdedit_core::prefs::Prefs::parse(&text);
    }
    docs.prefs = session.prefs.clone();
    crate::live::apply(&session.prefs);
    docs.recents = session.recent.clone();
    docs.check_updates = session.check_updates;
    if let Some(stage) = &args.update_bar {
        docs.updates.preview(stage);
    }
    docs.zoom = args.zoom.unwrap_or(session.zoom);
    docs.whitespace = args.whitespace || session.whitespace;
    // The preferences' minimap and wrap are the defaults of a new window and document: the view
    // commands change them for this run only.
    docs.wrap = args.wrap || session.wrap || session.prefs.word_wrap;
    docs.minimap = args.minimap || session.minimap || session.prefs.minimap;
    docs.theme = match args.dark {
        Some(true) => ThemeChoice::Dark,
        Some(false) => ThemeChoice::Light,
        None => session.theme,
    };
    // A folder among the files is the Files pane's folder, as `--folder` is (and as the same
    // argument is when it is handed to a running window); `--folder` wins.
    let dir_arg = args.files.iter().find(|f| f.is_dir());
    docs.folder = args
        .folder
        .as_deref()
        .or(dir_arg.map(PathBuf::as_path))
        .map(canonical_or_absolute)
        .or_else(|| session.folder.clone());
    let mut active = None;
    let restore = session.prefs.restore_session;
    for (i, path) in session.tabs.iter().enumerate().filter(|_| restore) {
        if path.is_file() {
            let k = docs.add_pending(path);
            if i == session.active {
                active = Some(k);
            }
        }
    }
    // Tabs that had unsaved text when the window last closed get it back.
    let mut restored = Vec::new();
    if let Some(dir) = session_path.as_deref().and_then(backup_dir_of) {
        for u in session.unsaved.iter().filter(|_| restore) {
            let file = dir.join(&u.file);
            if !file.is_file() {
                continue;
            }
            match u.path.as_ref().filter(|p| p.is_file()) {
                Some(path) => {
                    let k = docs.add_pending(path);
                    docs.tabs[k].restore = Some(file);
                    restored.push(k);
                }
                // A document that had no file, or whose file is gone since.
                None => {
                    if let Ok(text) = std::fs::read_to_string(&file) {
                        docs.restore_untitled(u.untitled, &text);
                    }
                }
            }
        }
    }
    if active.is_none() && !docs.tabs.is_empty() {
        active = Some(0);
    }
    for (i, f) in args.files.iter().filter(|f| !f.is_dir()).enumerate() {
        let k = docs.add_pending(f);
        if i == 0 {
            active = Some(k);
        }
    }
    // `--compare A B`: both files are tabs, the first active; the comparison starts once they
    // are read.
    let mut compare_ids = None;
    if let Some((x, y)) = &args.compare {
        let (a, b) = (docs.add_pending(x), docs.add_pending(y));
        active = Some(a);
        if a != b {
            compare_ids = Some((docs.tabs[a].id, docs.tabs[b].id));
            docs.other = Some(docs.tabs[b].id);
            docs.other_left = false;
        }
    }
    let w = waker.clone();
    let service = HighlightService::spawn(move || w(Wake::Highlight));
    let mut app = App::new(args, docs, trace, session_path, session);
    app.compare_wait = compare_ids;
    // Macros are read from here when first wanted, not now.
    app.macros.path = app
        .session_path
        .as_deref()
        .filter(|_| app.save)
        .and_then(|p| p.parent())
        .map(|d| d.join("macros.txt"));
    if app.args.demo_macros {
        app.macros.demo();
    }
    // The find options and the boxes' earlier entries come back with the session.
    let (case, word, regex) = app.session.find_options;
    app.ui.match_case = app.args.match_case || case;
    app.ui.whole_word = app.args.whole_word || word;
    app.ui.regex = app.args.regex || regex;
    app.ui.find_recall = crate::recall::Recall::from_items(app.session.find_history.clone());
    app.ui.replace_recall = crate::recall::Recall::from_items(app.session.replace_history.clone());
    app.service = Some(service);
    app.view.zoom = app.docs.zoom;
    app.sync_theme();
    // Read at once, so their tabs show as modified without being visited.
    for k in restored {
        if matches!(app.docs.tabs[k].state, DocState::Pending) {
            app.docs.start_load(k);
        }
    }
    if let Some(i) = active {
        app.docs.activate(i);
        trace.stage("load started");
    }
    // The split view comes back when both its files did (and the second is not the active one).
    if let Some(split) = app.session.split.clone() {
        app.docs
            .restore_split(&split.path, split.left, split.ratio());
    }
    let a = app.args.clone();
    app.ui.panes.files = a.files_pane.or(app.session.files).unwrap_or(false)
        || (a.folder.is_none() && a.files.iter().any(|f| f.is_dir()));
    if let Some(w) = app.session.files_w {
        app.ui.panes.files_w = (w as f32).clamp(PANE_MIN, PANE_MAX);
    }
    app.ui.tree.set_root(app.docs.folder.clone());

    if let Some(path) = a.shot.clone() {
        app.focused = true;
        return shot(app, &path);
    }
    let positions = a.goto.is_some()
        || a.type_text.is_some()
        || a.type_keys.is_some()
        || a.select.is_some()
        || a.select_column.is_some()
        || a.find.is_some()
        || a.scroll.is_some()
        || a.hscroll.is_some();
    if positions {
        app.positions_for = app.docs.active_doc().map(|d| d.id);
    }
    app.saved_sig = app.session_sig();
    let event_loop = match EventLoop::<Wake>::with_user_event().build() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("mdedit: no event loop: {e}");
            return 1;
        }
    };
    let p = event_loop.create_proxy();
    if let Ok(mut slot) = proxy.lock() {
        *slot = Some(p.clone());
    }
    trace.stage("event loop");
    if let Some(settings) = &app.session_path {
        let p = p.clone();
        app.incoming =
            mdshared_app::single::listen_requests_waking("mdedit", settings, move || {
                let _ = p.send_event(Wake::Instance);
            });
    }
    let pw = p.clone();
    app.watcher = FileWatcher::new(move || {
        let _ = pw.send_event(Wake::Files);
    });
    app.updates_at_launch();
    let _ = event_loop.run_app(&mut app);
    0
}

/// A content size in device px as the logical size the session keeps (and `with_inner_size`
/// takes back, before `Shadow::prepare` adds the margin).
fn logical_size((w, h): (u32, u32), scale: f64) -> (u32, u32) {
    let size = winit::dpi::PhysicalSize::new(w, h).to_logical::<f64>(scale);
    (size.width.round() as u32, size.height.round() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Key;
    use mdshared_ui::keys::KeyName;

    fn app() -> App {
        let args = Args::parse(["--no-session".to_string()]);
        let mut docs = Docs::new(1254, Arc::new(|_| {}));
        docs.new_untitled();
        let trace = Trace {
            on: false,
            started: Instant::now(),
        };
        App::new(args, docs, trace, None, Session::default())
    }

    /// The saved window size is the content size: a window started at the saved size, grown by
    /// the shadow's margin as `Shadow::prepare` does (logical `EXTENT` each side) and read back
    /// through the margin (device `ceil(EXTENT * scale)` each side), saves the same size, at every
    /// scale and however often it is restarted.
    #[test]
    fn the_saved_size_does_not_grow_by_the_margin() {
        use mdshared_ui::shadow::EXTENT;
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0, 3.0] {
            let mut saved = (1100u32, 700u32);
            for _ in 0..5 {
                let surface = |l: u32| ((l as f64 + 2.0 * EXTENT) * scale).round() as u32;
                let margin = (EXTENT * scale).ceil() as u32;
                let content = (surface(saved.0) - 2 * margin, surface(saved.1) - 2 * margin);
                saved = logical_size(content, scale);
            }
            assert_eq!(saved, (1100, 700), "scale {scale}");
        }
        // Without a margin (Windows, KWin) the content is the surface and nothing changes.
        assert_eq!(logical_size((1650, 1050), 1.5), (1100, 700));
        assert_eq!(app().content_size(1234, 567), (1234, 567));
        assert_eq!(app().content_size(0, 0), (1, 1));
    }

    /// The edge hit test has no lower bound: in the outer band (negative content coordinates with
    /// the client-drawn margin) a resize still starts, on every edge and corner.
    #[test]
    fn the_resize_band_reaches_outside_the_content() {
        use winit::window::ResizeDirection as D;
        let edge = mdshared_ui::window::edge_at;
        assert_eq!(edge(-4.0, 300.0, 800.0, 600.0), Some(D::West));
        assert_eq!(edge(400.0, -5.0, 800.0, 600.0), Some(D::North));
        assert_eq!(edge(-5.0, -5.0, 800.0, 600.0), Some(D::NorthWest));
        assert_eq!(edge(804.0, 603.0, 800.0, 600.0), Some(D::SouthEast));
        assert_eq!(edge(400.0, 300.0, 800.0, 600.0), None);
    }

    /// Every pixel of a composed frame has alpha 255: the client-drawn shadow's surface honours
    /// alpha, so a content pixel below it would show the desktop through the window.
    #[test]
    fn a_composed_frame_is_opaque() {
        let mut app = app();
        app.size = (900, 600);
        app.compose();
        let bad = |a: &App| {
            a.screen
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[3] != 255)
                .count()
        };
        assert_eq!(bad(&app), 0);
        app.size = (700, 500);
        app.scale = 1.5;
        app.compose();
        assert_eq!(bad(&app), 0);
    }

    /// Every layout of a split view tiles the window: with the screen painted in a colour no
    /// region uses first, nothing of it is left after a full compose. The second pane at the
    /// left and at the right, with and without a minimap beside either pane (a pane too narrow
    /// for one goes without), two files and one file in both panes.
    #[test]
    fn a_split_view_leaves_no_pixel_unpainted() {
        const SENTINEL: [u8; 4] = [0xFE, 0x01, 0xFD, 0xFF];
        for same_file in [false, true] {
            for other_left in [false, true] {
                for minimap in [false, true] {
                    for ratio in [0.3, 0.5, 0.7] {
                        for (size, scale) in [((1280, 800), 1.0), ((1000, 700), 1.5)] {
                            let mut app = app();
                            app.docs.new_untitled();
                            let other = app.docs.tabs[0].id;
                            app.docs.minimap = minimap;
                            if same_file {
                                assert!(app.docs.show_in_other_pane());
                            } else {
                                app.docs.set_other(other, other_left);
                            }
                            app.docs.other_left = other_left;
                            app.docs.split_ratio = ratio;
                            app.size = size;
                            app.scale = scale;
                            app.compose();
                            assert!(app.areas.other.is_some());
                            for px in app.screen.pixels.as_chunks_mut::<4>().0 {
                                *px = SENTINEL;
                            }
                            app.full = true;
                            app.compose();
                            let left = app
                                .screen
                                .pixels
                                .as_chunks::<4>()
                                .0
                                .iter()
                                .enumerate()
                                .filter(|(_, p)| **p == SENTINEL)
                                .map(|(i, _)| {
                                    (i % app.screen.width as usize, i / app.screen.width as usize)
                                })
                                .collect::<Vec<_>>();
                            assert!(
                                left.is_empty(),
                                "same_file {same_file}, other_left {other_left}, minimap {minimap}, \
                                 ratio {ratio}, scale {scale}: {} unpainted, from {:?} to {:?}",
                                left.len(),
                                left.first(),
                                left.last()
                            );
                        }
                    }
                }
            }
        }
    }

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            let (key, typed) = match c {
                '\n' => (KeyName::Enter, None),
                '\u{8}' => (KeyName::Backspace, None),
                c => (KeyName::Char(c), Some(c.to_string())),
            };
            app.key_pressed(Key::Name(key), typed.as_deref());
        }
    }

    fn text(app: &App) -> String {
        let r = app.docs.active_ready().unwrap();
        let mut s = String::new();
        r.buffer.text(0..r.buffer.len_bytes(), &mut s);
        s
    }

    #[test]
    fn a_macro_replays_typing_help_to_the_same_result() {
        let mut a = app();
        // Record: a bracket (paired), a letter, then Enter between the pair.
        a.macros.recording = Some(Vec::new());
        type_text(&mut a, "{a");
        a.key_pressed(Key::Name(KeyName::Right), None);
        type_text(&mut a, "\n");
        let recorded = text(&a);
        // A new document ends its lines with the platform's line ending (CRLF on Windows).
        let eol = a
            .docs
            .active_ready()
            .unwrap()
            .info
            .eol
            .dominant()
            .to_string();
        assert_eq!(recorded, format!("{{a}}{eol}"));
        a.macros.finish();
        assert!(!a.macros.steps.is_empty());
        // Replay at the end of the text: the keys do what they did, help included.
        a.docs.edit(crate::docs::EditOp::Insert("\n"), None);
        let before = text(&a);
        a.play_macro(false);
        let after = text(&a);
        assert_eq!(&after[before.len()..], format!("{{a}}{eol}"));
    }

    #[test]
    fn a_setting_applies_to_the_open_documents_and_is_kept_for_the_session_file() {
        use crate::app::modal::{AUTO_PAIR, TRIM_ON_SAVE};
        use mdshared_ui::settings::Value;
        let mut a = app();
        type_text(&mut a, "(");
        assert_eq!(text(&a), "()");
        a.apply_setting(AUTO_PAIR, Value::Bool(false));
        assert!(!a.docs.prefs.auto_pair);
        a.apply_setting(TRIM_ON_SAVE, Value::Bool(true));
        // The open document follows at once: a bracket is a bracket now.
        type_text(&mut a, "[");
        assert_eq!(text(&a), "([)");
        // The session file carries them, and the next start reads them back.
        let file = a.session.serialize();
        assert!(file.contains("[preferences]") && file.contains("auto_pair=0"));
        let back = Session::parse(&file);
        assert!(!back.prefs.auto_pair && back.prefs.trim_on_save);
        // Opening the form builds its rows from them.
        a.open_settings();
        assert!(a.ui.settings.is_some());
        a.close_settings();
        assert!(a.ui.settings.is_none());
    }
}
