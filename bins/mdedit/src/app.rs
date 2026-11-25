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
use crate::commands::CommandId as C;
use crate::commands::strings;
use crate::docs::{DocState, Docs, EditOp, Waker};
use crate::keys::Mods;
use crate::minimap;
use crate::overlays::{self, Bubble};
use crate::panes;
use crate::platform;
use crate::session::{Session, ThemeChoice, Unsaved};
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
use mdshared_ui::corners::Corners;
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
use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{
    CursorIcon, ResizeDirection, Theme as WinTheme, Window, WindowId, WindowLevel,
};

#[cfg(test)]
mod access_tests;
mod ai_events;
#[cfg(test)]
mod ai_kept_tests;
#[cfg(test)]
mod ai_tests;
mod commands;
mod compare_events;
mod completion;
mod crash;
#[cfg(test)]
mod undo_button_tests;
#[cfg(test)]
mod undo_rules_tests;
pub use crash::crash_dir_of_this_launch;
#[cfg(test)]
mod files_tests;
#[cfg(test)]
mod format_tests;
#[cfg(test)]
mod frost_tests;
mod handoff;
#[cfg(test)]
mod ime_tests;
mod import_ai;
#[cfg(test)]
mod import_ai_tests;
#[cfg(test)]
mod import_tests;
mod importing;
mod input;
#[cfg(test)]
mod language_tests;
mod library;
#[cfg(test)]
mod library_ai_tests;
mod library_drag;
#[cfg(test)]
mod library_drag_tests;
#[cfg(test)]
mod library_keys_tests;
#[cfg(test)]
mod library_leak_tests;
#[cfg(test)]
mod library_pane_tests;
#[cfg(test)]
mod library_tests;
mod library_write;
#[cfg(test)]
mod library_write_tests;
pub(crate) mod modal;
#[cfg(test)]
mod motion_tests;
mod moving;
#[cfg(test)]
mod moving_tests;
mod note;
#[cfg(test)]
mod note_tests;
mod notes_demo;
mod notes_plan;
#[cfg(test)]
mod notes_plan_tests;
mod notes_run;
#[cfg(test)]
mod notes_run_tests;
mod notes_write;
#[cfg(test)]
mod notes_write_tests;
mod palette_events;
mod plan;
#[cfg(test)]
mod plan_tests;
mod plan_write;
#[cfg(test)]
mod plan_write_tests;
mod preview;
#[cfg(test)]
mod recycle_tests;
#[cfg(test)]
mod review_tests;
mod visual;
pub(crate) use preview::previewable;
#[cfg(test)]
mod issues_tests;
#[cfg(test)]
mod pin_tests;
mod replace_files;
mod sent_card;
#[cfg(test)]
mod sent_card_tests;
mod split_ai;
#[cfg(test)]
mod split_ai_tests;
mod task;
#[cfg(test)]
mod task_tests;
#[cfg(test)]
mod widget_pixels_tests;

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
/// How wide the east resize band stays over a vertical scrollbar at the right edge, in logical px.
const SCROLLBAR_EDGE: f32 = 3.0;

/// Wakes the loop from other threads; the data travels on channels.
#[derive(Debug)]
pub enum Wake {
    /// The single-instance listener queued requests.
    Instance,
    /// macOS asked the application to open files (Finder, Open With, the Dock icon).
    Open(Vec<PathBuf>),
    /// An item of the macOS menu bar the application answers.
    Menu(mdshared_ui::menu_bar::AppCommand),
    /// The viewer that a preview was following is gone.
    Link,
    /// The preview pane has an answer.
    Preview,
    /// The AI engine sent something.
    Ai,
    /// The file watcher saw a change.
    Files,
    /// A load made progress or finished.
    Loaded,
    /// A search finished.
    Found,
    /// The folder "Add a folder to the library…" is reading on its own thread is done: the pass
    /// after this opens its plan.
    Import,
    /// The search of a folder has matches or finished.
    Results,
    /// The library read on its own thread has finished: the pass after this takes its result.
    Library,
    /// The function list of a document is made.
    Outline,
    /// The system's wish for less motion changed.
    Motion,
    /// The palette's list of the folder's files, or of a document's symbols, is made.
    Palette,
    /// The comparison of two files is made.
    Compare,
    /// The highlight worker queued colours.
    Highlight,
    /// An updater thread has an answer.
    Update(crate::updater::Answer),
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
    /// The file a note of the notes library was found at (the note it is for goes with it), and
    /// the folder chosen for it.
    LibraryFile(crate::library_pane::Moving, Vec<PathBuf>),
    LibraryFolder(Option<PathBuf>),
    /// A text file to make a Markdown note from (5.8), and the group's library file it is for.
    NoteFile(PathBuf, Vec<PathBuf>),
    /// The folder "Add a folder to the library…" was pointed at.
    ImportFolder(Option<PathBuf>),
    /// The files "Add files." in the AI task window was pointed at.
    TaskFiles(Vec<PathBuf>),
    /// The folder chosen for a group's notes, and the library file it is for.
    GroupHome(PathBuf, Option<PathBuf>),
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

/// How long after a failed write the session is written again.
const SESSION_RETRY: Duration = Duration::from_secs(5);

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
    /// Where the window was before "Preview side by side" put it on the left half, while it is
    /// still there.
    tiling: mdshared_ui::place::Tiling,
    /// The exit has saved the window's size.
    size_saved: bool,
    /// The window's drop shadow (Linux: KWin's protocol, or the client-drawn margin where the
    /// compositor has none); `None` elsewhere. With the margin, `size` is the CONTENT size (the
    /// surface minus the margin on every side) and `present` puts the content inside the surface.
    /// Dropping it would remove the KWin shadow, so it lives as long as the window.
    shadow: Option<mdshared_ui::shadow::Shadow>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    /// The "Window backdrop" setting at work (Windows 11): idle unless the setting is on.
    frost: crate::frost::Frost,
    /// The animator, and whether anything moves.
    pub(crate) motion: crate::motion::Motion,
    incoming: Option<Receiver<Request>>,
    /// When the viewer is next told the caret's line, if an update is held back.
    link_due: Option<Instant>,
    /// The preview pane, while it is open (src/preview_pane.rs).
    pub(crate) preview: Option<crate::preview_pane::Pane>,
    /// Visual editing (Ctrl+E): the document shown as its page, one block at a time edited in
    /// place (src/app/visual.rs).
    pub(crate) visual: Option<visual::Visual>,
    /// The installed mdview, as last looked for; `None` before the first look.
    viewer_path: Option<Option<PathBuf>>,
    /// What the crash report says mdedit was doing, as last set.
    crash_key: crash::CrashKey,
    watcher: Option<FileWatcher>,
    session_path: Option<PathBuf>,
    /// The settings file `MDEDIT_SETTINGS` named for a `--no-session` build: the one thing such a
    /// window may read settings beside, and its library with them. `None` otherwise.
    settings_override: Option<PathBuf>,
    /// Write the session on exit (not in `--shot`).
    save: bool,
    session: Session,
    screen: Bitmap,
    /// The format bar's state for this frame (`None`: not shown), worked out once per frame.
    format: Option<crate::format_bar::State>,
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
    /// The rectangle last given to the snap-layouts flyout, in device px; `None` when it was
    /// withdrawn. Only to skip the call when nothing changed.
    snap_sent: Option<Option<(i32, i32, i32, i32)>>,
    focused: bool,
    areas: Areas,
    hits: [Vec<(Hit, Rect)>; 10],
    overlay_hits: Vec<(Hit, Rect)>,
    sigs: [u64; 10],
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
    /// The file to start once the window is closed: set by a Restart that was carried out.
    restart: Option<PathBuf>,
    mods: Mods,
    /// macOS: the left press held now was made with the Control key, which a Mac takes for a
    /// right click; its release is not a left release.
    control_click: bool,
    /// A trackpad pinch on its way to the next zoom step.
    pinch: mdshared_ui::window::PinchZoom,
    cursor: CursorIcon,
    last_title: String,
    /// The close button's unsaved-changes dot as last set (macOS).
    last_edited: bool,
    last_active: Option<PathBuf>,
    toast_until: Option<Instant>,
    size_save_due: Option<Instant>,
    /// A file dialog is open on its helper thread: the window keeps drawing and a second request
    /// is ignored.
    #[cfg(not(windows))]
    dialog_busy: bool,
    exit_due: Option<Instant>,
    /// Recovery copies of unsaved documents, and when the next one is due.
    recovery: crate::recovery::Recovery,
    recover_due: Option<Instant>,
    /// When the results panel next looks at the tabs whose matches it lists.
    results_due: Option<Instant>,
    /// `--run`: when its command runs.
    run_due: Option<Instant>,
    /// `--scroll-steps`: steps left, the direction, and the frame times taken.
    steps: Option<u32>,
    step_down: bool,
    step_times: Vec<FrameTimes>,
    macros: crate::macros::Macros,
    /// The command palette: empty until it is first opened.
    palette: crate::palette::Host,
    /// AI rewriting: the engine once started, a run, its panels and its bar.
    ai: crate::ai::Ai,
    /// The plan screen (a plan of notes to read and change), and the plan Escape kept.
    plan: Option<crate::plan_ui::PlanScreen>,
    plan_parked: Option<crate::plan_ui::PlanScreen>,
    /// The plan screen while the split panel ("Ask the AI…") is in front of it (5.2).
    plan_behind: Option<crate::plan_ui::PlanScreen>,
    /// The last split asked of the AI, for "Run again".
    split_last: Option<crate::ai::Spec>,
    /// The AI task window over files (5.6), and the copy "Show what is sent" parked.
    task: Option<crate::task_ui::TaskScreen>,
    task_parked: Option<crate::task_ui::TaskScreen>,
    /// The file whose split plan is open from the task window.
    task_plan: Option<usize>,
    /// A rewrite run's record, made when its first file is written.
    task_record: Option<(PathBuf, mdedit_core::runs::Record)>,
    /// The last task window's id ([`crate::task_ui::TaskScreen::id`]).
    task_seq: u64,
    /// Files the task window's run rewrote, over every record it took (a run whose originals
    /// pass one record's size goes on in another, audit 4 B2).
    task_rewritten: usize,
    /// What the task window has sent, for the record.
    task_sent: mdedit_core::runs::Sent,
    /// Write notes' plan card over the task window (phase V4), kept while the window is in front.
    notes_plan: Option<crate::notes_plan_ui::NotesPlan>,
    /// The Recycle Bin (5.4): the system's, or in a test a bin of the test's own, so no test can
    /// reach the system's.
    recycle: Box<dyn crate::recycle::RecycleBin>,
    /// "Move the notes…": the card of a move's plan, while it is open.
    mover: Option<crate::move_ui::MoveCard>,
    /// "A Markdown note from a file…": the card while it is open (5.8).
    note: Option<crate::note_ui::NoteCard>,
    /// "Add a folder to the library…" (5.7): the card of an import, and the folder a walk is
    /// reading on its own thread.
    importer: Option<crate::import_ui::ImportCard>,
    import_job: Option<mdshared_app::job::Job<mdedit_core::import::Walk>>,
    import_dir: Option<PathBuf>,
    /// The library being read again on its own thread after a library file was saved, with at
    /// most one running at a time. The rows on screen stay until the result is taken.
    library_job: Option<mdshared_app::job::Job<Option<mdshared_app::library::Library>>>,
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
    /// The session could not be written: when to try again.
    session_retry: Option<Instant>,
    /// While an AI rewrite runs: when the status bar's strip moves next.
    ai_tick: Option<Instant>,
    /// `--goto`, `--select`, `--find`, `--scroll` or `--hscroll` wait for this document to be
    /// ready in a windowed run.
    positions_for: Option<DocId>,
    /// Word completion and snippets; empty until first used.
    completion: completion::State,
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
        settings_override: Option<PathBuf>,
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
            motion: crate::motion::Motion::new(crate::motion::still_for(&args) || cfg!(test)),
            args,
            trace,
            docs,
            ui: Ui::new(),
            view: View::new(),
            service: None,
            service_doc: None,
            service_dark: None,
            window: None,
            tiling: Default::default(),
            size_saved: false,
            shadow: None,
            surface: None,
            frost: crate::frost::Frost::default(),
            incoming: None,
            link_due: None,
            preview: None,
            visual: None,
            viewer_path: None,
            crash_key: Default::default(),
            watcher: None,
            session_path,
            settings_override,
            session,
            screen: Bitmap::new(1, 1),
            format: None,
            gfx: Gfx::new(),
            pal: theme::LIGHT,
            system_dark,
            scale: 1.0,
            size: (1, 1),
            maximized: false,
            snap_sent: None,
            focused: true,
            areas: Areas::default(),
            hits: Default::default(),
            overlay_hits: Vec::new(),
            sigs: [0; 10],
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
            restart: None,
            mods: Mods::default(),
            control_click: false,
            pinch: Default::default(),
            cursor: CursorIcon::Default,
            last_title: String::new(),
            last_edited: false,
            last_active: None,
            toast_until: None,
            size_save_due: None,
            #[cfg(not(windows))]
            dialog_busy: false,
            exit_due: None,
            recovery: crate::recovery::Recovery::off(),
            recover_due: None,
            results_due: None,
            run_due: None,
            step_down: true,
            step_times: Vec::new(),
            macros: crate::macros::Macros::default(),
            palette: Default::default(),
            ai: Default::default(),
            plan: None,
            plan_parked: None,
            plan_behind: None,
            split_last: None,
            task: None,
            task_parked: None,
            task_plan: None,
            task_record: None,
            task_seq: 0,
            task_rewritten: 0,
            task_sent: Default::default(),
            notes_plan: None,
            #[cfg(not(test))]
            recycle: Box::new(crate::recycle::SystemBin),
            #[cfg(test)]
            recycle: Box::new(crate::recycle::NoBin),
            mover: None,
            note: None,
            importer: None,
            import_job: None,
            import_dir: None,
            library_job: None,
            settled: false,
            load_traced: false,
            colours_traced: false,
            wheel_rest: (0.0, 0.0),
            saved_sig: 0,
            session_retry: None,
            ai_tick: None,
            positions_for: None,
            completion: Default::default(),
        }
    }

    fn dark(&self) -> bool {
        self.docs.theme.is_dark(self.system_dark)
    }

    fn redraw(&mut self) {
        self.wants_frame = true;
    }

    /// Starts reading and watching the system's wish for less motion, once, unless nothing can
    /// move (a shot, a test, the Animations setting off).
    fn watch_motion(&mut self) {
        if self.motion.watching() {
            return;
        }
        self.motion.set_setting(self.docs.prefs.animations);
        let wake = self.docs.waker();
        self.motion.watch(move || wake(Wake::Motion));
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
                style(w, dark, self.session.corners);
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

    /// The "Window backdrop" setting: asks the system when the setting or the focus changed and
    /// repaints everything when the chrome turns frosted or flat. One comparison when it is off.
    fn sync_frost(&mut self) {
        let want = self.docs.prefs.backdrop;
        if !self.frost.concerns(want) {
            return;
        }
        // Without a window (`--shot`) there is nothing to show a backdrop behind.
        let Some(window) = self.window.clone() else {
            return;
        };
        #[cfg(windows)]
        let mut sys = mdshared_ui::frost::WindowSystem(hwnd(&window));
        // KDE Plasma (Wayland): the blur protocol on the window's own connection; elsewhere it
        // says no and the chrome stays flat.
        #[cfg(not(windows))]
        let mut sys = mdshared_ui::frost::BlurSystem(&window);
        if self.frost.sync(want, self.focused, &self.pal, &mut sys) {
            // The buffers carry alpha while the chrome is frosted (or the shadow's margin needs
            // it); a no-op where the surface cannot.
            #[cfg(not(windows))]
            self.sync_surface_alpha();
            // Every mark goes: the regions put theirs back when they are drawn frosted.
            for px in self.screen.pixels.as_chunks_mut::<4>().0 {
                px[3] = 255;
            }
            self.full = true;
            self.expose = true;
            let state = if self.frost.key().is_some() {
                "frosted"
            } else {
                "flat"
            };
            self.trace
                .line(&format!("backdrop {}: chrome {state}", want.key()));
        }
    }

    /// Wayland buffers must carry alpha for a frosted chrome or the client-drawn margin.
    #[cfg(not(windows))]
    fn sync_surface_alpha(&mut self) {
        let alpha =
            self.frost.wants_alpha() || self.shadow.as_ref().is_some_and(|s| s.wants_alpha());
        if let Some(s) = self.surface.as_mut() {
            s.set_premultiplied_alpha(alpha);
        }
    }

    /// The caret shows: the text has the keyboard and the window has focus.
    fn text_focused(&self) -> bool {
        // Visual editing: the text has the keyboard only in an open block.
        !(self.visual.is_some() && self.visual_block().is_none())
            && !self.ui.find_focused
            && !self.preview_focused()
            && !self.ui.results.focused
            && !self.ui.library.focused
            && self.ui.dialog.is_none()
            && !self.palette_open()
            && !self.ai_panel_open()
            && !self.plan_is_open()
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
                    (common, chrome::preview_button(&self.docs), self.format),
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
                        ui.field().preedit().map(|p| (&p.text, p.cursor)),
                    ),
                    chrome::find_label(&self.docs),
                    ui.panes.files,
                    (ui.popup_kind(), self.docs.active().is_some(), has_matches),
                    // New, Save and AI edit (issue #8): Save is lit by an edit, AI edit is there
                    // while the AI is on; the Preview button's tooltip follows its mode.
                    (
                        self.docs.active_doc().is_some_and(|d| d.modified()),
                        self.docs.active_ready().is_some(),
                        self.docs.prefs.ai_connection == mdedit_core::ai::Connection::Off,
                        chrome::button_mode(&self.docs),
                    ),
                    (
                        self.docs
                            .updates
                            .bar()
                            .map(|s| crate::updater::Updates::bar_label(&crate::updater::APP, s)),
                        self.docs.updates.relaunch_target().is_some(),
                        self.docs.crash.is_some(),
                        (self.docs.stale.len(), self.docs.rename_bar()),
                    ),
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
            Region::Files if ui.library.on => {
                let active = self.docs.active_doc().and_then(|d| d.path.clone());
                let drag = matches!(
                    ui.drag,
                    Some(Drag::Splitter { .. })
                        | Some(Drag::Thumb {
                            target: Scroller::Files,
                            ..
                        })
                );
                // A note being dragged: its row, the heading it would go into, and where the
                // pointer is, because the chip is drawn at it and no row changes as it moves.
                let note_drag = match ui.drag {
                    Some(Drag::LibraryNote { row, x, y, moving }) => Some((
                        row,
                        moving,
                        ui.library.drop,
                        x.to_bits(),
                        y.to_bits(),
                        ui.mouse.0.to_bits(),
                        ui.mouse.1.to_bits(),
                    )),
                    _ => None,
                };
                let rows: Vec<(u16, &str, Option<&str>)> = ui
                    .library
                    .rows
                    .iter()
                    .map(|r| (r.depth, r.label.as_str(), r.home.as_deref()))
                    .collect();
                // The name box and the cursor are drawn in the pane: what is typed, where its
                // caret and selection are, whether it blinks, and the keyboard's row.
                let boxed = ui.find_focused && ui.field == crate::ui::BarField::Library;
                let name = ui.library.edit.as_ref().map(|e| {
                    (
                        e.at,
                        e.depth,
                        ui.library.field.text(),
                        ui.library.field.caret(),
                        ui.library.field.preedit().map(|p| (&p.text, p.cursor)),
                        (boxed, boxed && ui.caret_on),
                    )
                });
                sig((
                    (common, rows, active),
                    ui.library.scroll.to_bits(),
                    ui.library.rows.len(),
                    drag,
                    note_drag,
                    name,
                    (ui.library.cur, ui.library.focused),
                    // The "Plan ready" row at the foot: its count, and whether a plan is kept.
                    self.plan_parked.as_ref().map(|p| p.plan.notes().len()),
                    // The row that says how many groups a capped folder shows: the count, and
                    // the groups the reader kept (a file that only names a note is not drawn).
                    (
                        self.docs.library.omitted,
                        self.docs.library_stamp.groups.len(),
                    ),
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
                        let preedit = (!second)
                            .then(|| self.view.preedit.as_ref().map(|p| (&p.text, p.cursor)))
                            .flatten();
                        sig((
                            (3u8, preedit),
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
            Region::Preview => match &self.preview {
                Some(p) => {
                    let drag = matches!(
                        ui.drag,
                        Some(Drag::Thumb {
                            target: Scroller::Page,
                            ..
                        })
                    );
                    sig((common, p.generation, p.ended.as_deref(), self.dark(), drag))
                }
                None => sig((common, "closed")),
            },
            Region::Status => sig((
                common,
                status::fields(&self.docs),
                self.dark(),
                self.docs.ai_status.as_ref().map(|_| self.docs.ai_phase),
            )),
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
                    (
                        ui.find_focused
                            && matches!(ui.field, BarField::ResultsFolder | BarField::ResultsKinds),
                        ui.field,
                        ui.caret_on,
                    ),
                    (
                        ui.results.folder.text(),
                        ui.results.folder.caret(),
                        ui.results.kinds.text(),
                        ui.results.kinds.caret(),
                    ),
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
        self.ai_review_sync();
        let mut times = FrameTimes::default();
        let t0 = Instant::now();
        self.motion.set_setting(self.docs.prefs.animations);
        let frame_at = self.motion.now();
        self.motion.anim.begin_frame(frame_at);
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
        self.sync_frost();
        self.compare_sync();
        self.docs.sync_wrap();
        let (w, h) = self.logical();
        self.drop_undone_switches();
        self.format = crate::format_bar::shown(&self.docs);
        self.sync_side_by_side();
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
        let mut dirty = [self.full; 10];
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
                overlays::fit_modify_room(&mut popup.menu, &mut p);
                popup.menu.place_in(&mut p, room);
                if let Some(s) = popup.sub.as_mut() {
                    overlays::fit_modify_room(&mut s.menu, &mut p);
                    s.menu.place_in(&mut p, room);
                }
            }
        }
        self.toast_until = self.docs.toast(now).and(self.docs.toast_expires());
        // The menus' fade: the system's wish is read once the window settled, or from the
        // first menu that could fade if the setting was off then.
        if !self.motion.watching()
            && self
                .ui
                .popup
                .as_ref()
                .is_some_and(|p| crate::motion::fades(p.kind))
        {
            self.watch_motion();
        }
        let menus = self.motion.menus(self.ui.popup.as_ref());
        let mut bounds: Vec<Rect> = bubbles.iter().map(Bubble::bounds).collect();
        if let Some(popup) = &self.ui.popup {
            // A menu that fades touches its presence bounds (its drop and shadow in them).
            let reach = |m: &mdshared_ui::menu::Menu<Act>| match menus {
                Some(_) => m.presence_bounds(),
                None => m.rect().inset(-20.0),
            };
            bounds.push(reach(&popup.menu));
            if let Some(s) = &popup.sub {
                bounds.push(reach(&s.menu));
            }
        }
        if self.ui.dialog.is_some()
            || self.ui.settings.is_some()
            || self.ai_panel_open()
            || self.plan_is_open()
            || self.task_is_open()
        {
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
                    (p.kind == PopupKind::Ai)
                        .then(|| crate::ai_ui::menu_modify_at(&p.menu, self.ui.mouse)),
                    p.sub.as_ref().map(|s| {
                        (
                            s.kind,
                            s.menu.hover,
                            s.menu.scroll().to_bits(),
                            // A saved prompt's Modify button lit under the pointer (the owner:
                            // it lit only half a second later, with the next other change).
                            (s.kind == PopupKind::Ai)
                                .then(|| crate::ai_ui::menu_modify_at(&s.menu, self.ui.mouse)),
                        )
                    }),
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
                    (
                        self.ui.runs.sel,
                        self.ui.runs.list.len(),
                        self.ui.runs.unread.len(),
                    ),
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
            // The AI panels raise their tick for every change, as the palette does.
            self.ai_panel_open().then_some((
                self.ai.tick,
                self.ai.hover,
                self.ai.pressed,
                self.ui.caret_on,
            )),
            // The plan screen raises its tick for every change of what it shows, and the
            // pointer's place is in its signature; the caret's blink matters only while a name
            // is being typed.
            self.plan
                .as_ref()
                .map(|s| (s.sig(), s.edit.is_some() && self.ui.caret_on)),
            // The move card raises its tick for every change, as the plan screen does.
            self.mover.as_ref().map(|m| m.sig()),
            // The AI task window over files raises its tick for every change too.
            self.task.as_ref().map(|s| {
                (
                    (s.tick, s.hover, s.pressed, self.ui.caret_on),
                    self.notes_plan
                        .as_ref()
                        .map(|c| (c.shown, c.sig(self.ui.caret_on))),
                )
            }),
            // The Markdown note card does too.
            self.note.as_ref().map(|n| n.sig()),
            // So does the import card; the caret blinks in a scheme name being typed.
            self.importer.as_ref().map(|c| c.sig(self.ui.caret_on)),
        ));
        let overlay_changed = osig != self.overlay_sig;
        self.overlay_sig = osig;
        let mut under: Vec<Rect> = bounds.clone();
        under.extend(self.overlay_bounds.iter().copied());
        // What a fading menu's kept pixels cover is under it too.
        if let Some((_, r)) = &self.motion.backing {
            under.push(*r);
        }
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
        // A menu on its way in: drawn alone over what was under it, or with everything else.
        let others = !bubbles.is_empty()
            || self.ui.dialog.is_some()
            || self.ui.settings.is_some()
            || self.ai_panel_open()
            || self.plan.is_some()
            || self.task.is_some()
            || self.mover.is_some()
            || self.importer.is_some()
            || self.note.is_some()
            || self.palette.open.is_some();
        let menu_frame = crate::motion::menu_frame(
            menus,
            self.motion.drawn,
            self.motion.backing.as_ref(),
            self.ui.popup.as_ref().map(crate::motion::popup_bounds),
            overlay_changed || any_under_dirty || self.full || others,
        );
        let overlay_changed = overlay_changed || menu_frame == crate::motion::MenuFrame::Whole;
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
        // Visual editing: the open block's text is drawn over the page, so a new page frame
        // draws the text again.
        let visual_box = self
            .areas
            .preview
            .filter(|_| self.visual_on())
            .and_then(|page| self.visual_box(page));
        if self.visual_on() && dirty[Region::Preview.index()] {
            dirty[ti] = true;
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
            arrow_right: !self.docs.other_left,
            hbar_active: hover == Some(Hit::ScrollBar(Scroller::TextH))
                || matches!(
                    self.ui.drag,
                    Some(Drag::Thumb {
                        target: Scroller::TextH,
                        ..
                    })
                ),
        };
        // The page has a scrollbar when it fills the text area; no other mode draws one.
        let page_bar = self.visual_on().then(|| {
            hover == Some(Hit::ScrollBar(Scroller::Page))
                || matches!(
                    self.ui.drag,
                    Some(Drag::Thumb {
                        target: Scroller::Page,
                        ..
                    })
                )
        });
        let mut damage = Vec::new();
        // Settings opened from the task window hides it until Settings closes.
        let task_shown = self.task_is_open();
        {
            let mut p = Painter::new(&mut self.screen, &mut self.gfx, self.scale);
            // A fade over frosted chrome mixes as the compositor shows it.
            p.set_frost_key(self.frost.key().copied());
            let mut order = REGIONS.to_vec();
            if visual_box.is_some() || self.visual.is_some() {
                order.retain(|&r| r != Region::Preview);
                let at = order.iter().position(|&r| r == Region::Text).unwrap_or(0);
                order.insert(at, Region::Preview);
            }
            for r in order {
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
                        if let Some(state) = self.format {
                            let row = Rect::new(rect.x, y, rect.w, crate::format_bar::FORMAT_H);
                            crate::format_bar::draw(&mut c, &state, row);
                            y = row.bottom();
                        }
                        if self.ui.replace_open {
                            let strip = Rect::new(rect.x, y, rect.w, crate::ui::REPLACE_H);
                            chrome::draw_replace_strip(&mut c, &self.docs, &mut self.ui, strip);
                            y = strip.bottom();
                        }
                        if self.docs.updates.bar().is_some() {
                            let under = Rect::new(rect.x, y, rect.w, view::BANNER_H);
                            chrome::draw_update_bar(&mut c, &self.docs, under);
                            y = under.bottom();
                        }
                        if self.docs.crash.is_some() {
                            let under = Rect::new(rect.x, y, rect.w, view::BANNER_H);
                            chrome::draw_crash_bar(&mut c, under);
                            y = under.bottom();
                        }
                        if !self.docs.stale.is_empty() {
                            let under = Rect::new(rect.x, y, rect.w, view::BANNER_H);
                            chrome::draw_stale_bar(
                                &mut c,
                                under,
                                self.docs.stale.len(),
                                self.docs.prefs.stale_days,
                            );
                            y = under.bottom();
                        }
                        if let Some((name, to)) = self.docs.rename_bar() {
                            let under = Rect::new(rect.x, y, rect.w, view::BANNER_H);
                            let to = to
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            chrome::draw_rename_bar(&mut c, under, &name, &to);
                            y = under.bottom();
                        }
                        if let Some(bar) = &self.ai.bar {
                            let under = Rect::new(rect.x, y, rect.w, view::BANNER_H);
                            chrome::draw_ai_bar(&mut c, bar, under);
                        }
                    }
                    Region::Files if self.ui.outline.on => {
                        crate::outline::draw(&mut c, &self.docs, &mut self.ui, rect)
                    }
                    Region::Files if self.ui.library.on => {
                        let ready = self.plan_parked.as_ref().map(|p| p.plan.notes().len());
                        crate::library_pane::draw(&mut c, &self.docs, &mut self.ui, rect, ready)
                    }
                    Region::Files => panes::draw_files(&mut c, &self.docs, &mut self.ui, rect),
                    Region::Text => match self.docs.active() {
                        None => overlays::draw_empty(&mut c, &self.docs, rect),
                        Some(i) => {
                            let doc = &mut self.docs.tabs[i];
                            times.paint = if visual_box.is_some() {
                                // The box can be scrolled partly out of the page: its text
                                // stays inside the page, off the bars around it.
                                if let Some(page) = self.areas.preview {
                                    c.p.clip(page);
                                }
                                // The open block's text sits on the box's tint, not the page.
                                let boxed = Palette {
                                    page: visual::box_fill(c.pal),
                                    ..*c.pal
                                };
                                let mut bc = chrome::Ctx {
                                    p: &mut *c.p,
                                    pal: &boxed,
                                    hits: &mut *c.hits,
                                    hover: c.hover,
                                    pressed: c.pressed,
                                };
                                self.view.draw(&mut bc, doc, rect, &ds, None)
                            } else {
                                self.view.draw(&mut c, doc, rect, &ds, None)
                            };
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
                    Region::Preview => {
                        crate::preview_pane::draw(
                            &mut c,
                            self.preview.as_ref(),
                            &self.docs,
                            rect,
                            page_bar,
                        );
                        if let Some(b) = visual_box {
                            c.p.frame(b, 6.0, visual::box_fill(&pal), pal.accent);
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
                                arrow_right: left,
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
                // The chrome lets the backdrop through: its pixels are marked for the present.
                if self.frost.key().is_some() && crate::frost::frosted(r) {
                    mdshared_ui::frost::mark(p.bmp, p.dev_rect(rect));
                }
            }
            if self.frost.key().is_some() {
                self.frost.areas.clear();
                for r in REGIONS {
                    if let Some(rect) = self.areas.region(r).filter(|_| crate::frost::frosted(r)) {
                        let d = p.dev_rect(rect);
                        let u = |v: i32| v.max(0) as usize;
                        self.frost.areas.push((u(d.0), u(d.1), u(d.2), u(d.3)));
                    }
                }
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
                // The language list stands above the status bar's language field, when it shows.
                let anchor = self
                    .palette
                    .open
                    .as_ref()
                    .filter(|p| p.anchored)
                    .and_then(|_| {
                        // `hit_rect`, by field: the painter holds the screen.
                        self.hits
                            .iter()
                            .flatten()
                            .find(|(h, _)| *h == Hit::StatusLanguage)
                            .map(|(_, r)| *r)
                    });
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
                            &mut self.ui.runs,
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
                    crate::motion::draw_popup(
                        c.p,
                        &pal,
                        popup,
                        room,
                        menus,
                        self.ui.mouse,
                        &mut self.motion.backing,
                        false,
                    );
                }
                for b in &bubbles {
                    b.draw(c.p, &pal);
                }
                if let Some(plan) = self.plan.as_mut() {
                    plan.draw(c.p, &pal, w, h, self.ui.caret_on);
                }
                if let Some(s) = self.task.as_mut().filter(|_| task_shown) {
                    match self.notes_plan.as_mut().filter(|card| card.shown) {
                        Some(card) => {
                            crate::notes_plan_draw::draw(c.p, &pal, card, s, w, h, self.ui.caret_on)
                        }
                        None => crate::task_draw::draw(c.p, &pal, s, w, h, self.ui.caret_on),
                    }
                }
                if let Some(card) = self.mover.as_mut() {
                    card.draw(c.p, &pal, w, h);
                }
                if let Some(card) = self.importer.as_mut() {
                    card.draw(c.p, &pal, w, h, self.ui.caret_on);
                }
                if let Some(card) = self.note.as_mut() {
                    card.draw(c.p, &pal, w, h);
                }
                // Last, over everything.
                if self.ai.panel.is_some() {
                    let split = matches!(
                        &self.ai.panel,
                        Some(crate::ai::Panel::Instruction(i)) if i.split.is_some()
                    );
                    crate::ai_ui::draw(
                        c.p,
                        &pal,
                        &mut self.ai,
                        if split {
                            &self.docs.ai_split_prompts
                        } else {
                            &self.docs.ai_prompts
                        },
                        w,
                        h,
                        self.ui.caret_on,
                    );
                }
                if let Some(p) = self.palette.open.as_mut() {
                    let (window, title) = match anchor {
                        Some(field) => (
                            crate::palette::anchored_window(
                                &mut p.palette,
                                field,
                                w,
                                crate::ui::TITLE_H + 8.0,
                            ),
                            0.0,
                        ),
                        None => (Rect::new(0.0, 0.0, w, h), crate::ui::TITLE_H),
                    };
                    p.palette.draw(c.p, &pal, window, title, self.ui.caret_on);
                    if let Some(note) = &p.note {
                        crate::palette::draw_note(c.p, &pal, p.palette.rect(), note);
                    }
                }
            } else if menu_frame == crate::motion::MenuFrame::Alone
                && let Some(popup) = self.ui.popup.as_mut()
            {
                // Only the menus moved: what was under them goes back, and they are drawn on it.
                if let Some((under, _)) = &self.motion.backing {
                    under.restore(p.bmp);
                    damage.push(under.rect());
                }
                let room = menu_room(self.areas.title, w, h);
                crate::motion::draw_popup(
                    &mut p,
                    &pal,
                    popup,
                    room,
                    menus,
                    self.ui.mouse,
                    &mut self.motion.backing,
                    true,
                );
            }
        }
        if dirty[ti] || bands.is_some() {
            self.caret_line = caret_line;
        }
        self.motion.drawn = menus;
        if !menus.is_some_and(|m| m.moving()) {
            self.motion.backing = None;
        }
        self.motion.due = self.motion.anim.end_frame();
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

    /// Tells the window where the maximize button is, so that Windows 11 shows the snap-layouts
    /// flyout over it (`None` while a dialog, the palette or the settings cover the title row).
    /// Called after every frame: after a resize, a maximize or restore and a change of scale the
    /// next frame lays the title out again, and nothing is sent when the rectangle is the same.
    fn sync_snap(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let covered = self.ui.dialog.is_some()
            || self.palette_open()
            || self.ai_panel_open()
            || self.plan_is_open()
            || self.ui.settings.is_some();
        let margin = self.shadow.as_ref().map_or(0, |s| s.margin()) as f32 / self.scale;
        let rect = chrome::snap_button_rect(self.areas.title, margin, self.maximized, covered);
        let key = rect.map(|r| {
            (
                (r.x * self.scale).round() as i32,
                (r.y * self.scale).round() as i32,
                (r.w * self.scale).round() as i32,
                (r.h * self.scale).round() as i32,
            )
        });
        if self.snap_sent != Some(key) {
            self.snap_sent = Some(key);
            mdshared_ui::snap::set_maximize_button_rect(window, rect);
        }
    }

    fn draw(&mut self) {
        let (mut damage, mut times) = self.compose();
        self.sync_snap();
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
        self.crash_context();
        let title = strings::window_title(self.docs.active_doc().map(|d| d.title()).as_deref());
        if title != self.last_title {
            if let Some(w) = &self.window {
                w.set_title(&title);
            }
            self.last_title = title;
        }
        // macOS: the dot in the close button while any tab has unsaved changes.
        let edited = mdshared_ui::chrome::NATIVE_FRAME && !self.docs.modified_ids().is_empty();
        if edited != self.last_edited {
            if let Some(w) = &self.window {
                mdshared_ui::window::set_edited(w, edited);
            }
            self.last_edited = edited;
        }
        if self.first {
            self.first = false;
            self.trace.stage("first frame presented hidden");
            if let Some(w) = &self.window {
                w.set_visible(true);
                // Showing a window raises it, so a test run's window is sent under the others
                // again (winit skips a level that has not changed, hence Normal first).
                if self.args.behind {
                    w.set_window_level(WindowLevel::Normal);
                    w.set_window_level(WindowLevel::AlwaysOnBottom);
                }
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
        let frost = &self.frost;
        let copy = |buffer: &mut [u32], (x0, y0, x1, y1): (usize, usize, usize, usize)| {
            let bytes: &mut [u8] = bytemuck::cast_slice_mut(buffer);
            for y in y0..y1 {
                let (a, b) = ((y * stride + x0) * 4, (y * stride + x1) * 4);
                let to = ((y + m) * surface_stride + x0 + m) * 4;
                bytes[to..to + (b - a)].copy_from_slice(&screen.pixels[a..b]);
            }
            // A frosted chrome: the flat colour is taken out of its marked pixels, in the
            // window's copy only (the bitmap keeps what was drawn).
            if let Some(key) = frost.key() {
                for area in &frost.areas {
                    if let Some(r) = mdshared_ui::frost::overlap((x0, y0, x1, y1), *area) {
                        let r = (r.0 + m, r.1 + m, r.2 + m, r.3 + m);
                        mdshared_ui::frost::key_out(key, buffer, surface_stride, r);
                    }
                }
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
        // The rounded corners are cut out of the content just copied: only the corner squares the
        // copied rectangles touch are redone (all four after a whole copy). Nothing without
        // rounding.
        if let Some(s) = self.shadow.as_ref().filter(|s| s.rounds()) {
            let touched: Vec<_> = damage.iter().map(&clamp).collect();
            // A margin painted again (focus, scale) changed the shadow under the corners too.
            let all = age != 1 || margin_painted;
            s.paint_corners(
                &mut buffer,
                sw,
                sh,
                &self.screen.pixels,
                (!all).then_some(&touched[..]),
            );
        }
        let copied = t.elapsed();
        let _ = buffer.present_with_damage(&rects);
        copied
    }

    // ---- hit testing -------------------------------------------------------------------------

    fn hit_at(&self, x: f32, y: f32) -> Option<Hit> {
        // The palette, the AI panels and the plan screen do their own hit testing.
        if self.palette_open() || self.ai_panel_open() || self.plan_is_open() {
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
        // The panel's top edge is a strip across the seam between the text and the panel.
        if let Some(r) = self.areas.results
            && x >= r.x
            && x < r.right()
            && y >= r.y - 2.0
            && y < r.y + 3.0
        {
            return Some(Hit::ResultsEdge);
        }
        // The line between two panes is not drawn by a region: it is where the layout says.
        if let (Some(d), true) = (
            self.areas.divider,
            self.areas.other.is_some() || self.areas.preview.is_some(),
        ) && d.grab(self.areas.text.y, self.areas.text.h).contains(x, y)
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
        // macOS resizes the window itself (`chrome::NATIVE_FRAME`).
        if self.maximized || self.window.is_none() || mdshared_ui::chrome::NATIVE_FRAME {
            return None;
        }
        self.resize_band(x, y)
    }

    /// The resize direction at the point for a restored window, without asking the window.
    ///
    /// The band is 8 px wide, and a vertical scrollbar at the window's right edge (the text, the
    /// second pane or the results panel with no minimap beside it) has its thumb 3 to 11 px from
    /// that edge: the east band over such a track shrinks to the outermost [`SCROLLBAR_EDGE`] px,
    /// which the thumb never covers, so the bar can be grabbed and the edge still resizes. The
    /// corners keep their full band.
    fn resize_band(&self, x: f32, y: f32) -> Option<ResizeDirection> {
        let (w, h) = self.logical();
        let dir = mdshared_ui::window::edge_at(x, y, w, h);
        if dir == Some(ResizeDirection::East)
            && x < w - SCROLLBAR_EDGE
            && matches!(
                self.hit_at(x, y),
                Some(Hit::ScrollBar(
                    Scroller::Text | Scroller::OtherText | Scroller::Results | Scroller::Page
                ))
            )
        {
            return None;
        }
        dir
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
            (self.docs.paths(), self.docs.pins(), &self.session.unsaved),
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
            (self.ui.panes.files_w.round() as u32, self.session.results_h),
            (
                &self.docs.folder,
                self.docs.marks.version,
                self.docs.places.version,
            ),
            &self.session.geometry,
            (
                self.docs.check_updates,
                self.session.update_checked,
                self.docs.prefs.serialize(),
                self.session.corners.to_setting(),
            ),
            (
                (self.ui.match_case, self.ui.whole_word, self.ui.regex),
                self.ui.find_recall.items(),
                self.ui.replace_recall.items(),
                (self.ui.results.folder.text(), self.ui.results.kinds.text()),
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
        // A write that failed is tried again after a pause, not on every pass.
        if self.session_retry.is_some_and(|at| Instant::now() < at) {
            return;
        }
        // Backups of text that was saved, discarded or closed since the start are dropped. Not
        // once the exit has set the list to keep (it is final then).
        if !self.session.unsaved.is_empty() && !self.exit {
            let kept = self.docs.backups_needed(&self.session.unsaved);
            if kept.len() != self.session.unsaved.len() {
                self.session.unsaved = kept;
                self.save_session();
                return;
            }
        }
        // A retry that is due is made even when the state is back to what was last written (an
        // exit that was cancelled puts the old list back): it succeeds and is done, or it is due
        // again later. Never left in the past, where it would wake the loop at once on every pass
        // (audit 1 round 6).
        if self.session_retry.is_some() || self.session_sig() != self.saved_sig {
            self.save_session();
        }
    }

    /// When the event loop must wake next with nothing else to wake it: the timers, the next
    /// recovery look, the folder search, and a session write that failed (tried again then, also
    /// in a window left alone, audit 1 round 5).
    fn next_wake(&mut self, now: Instant) -> Option<Instant> {
        [
            self.timers(now),
            self.recover_due,
            self.results_due,
            self.session_retry,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// Where unsaved text is kept between runs: `backup` beside the session file.
    pub(crate) fn backup_dir(&self) -> Option<PathBuf> {
        let path = self.session_path.as_deref().filter(|_| self.save)?;
        backup_dir_of(path)
    }

    /// Writes the session; false when it could not be written (it is tried again later: the
    /// state written last is not taken as written, audit 1 round 4, #1).
    /// At exit: the session is written, and the recovery copies go, except those of text the
    /// session could not keep (its write failed): those stay for the next start.
    fn finish_session(&mut self) {
        let saved = self.save_session();
        self.recovery.finish(&self.docs, saved);
    }

    fn save_session(&mut self) -> bool {
        let Some(path) = self.session_path.clone().filter(|_| self.save) else {
            return true;
        };
        let split = self.split_state();
        let s = &mut self.session;
        s.tabs = self.docs.paths();
        s.pinned = self.docs.pins();
        s.active = self.docs.active_among_paths();
        s.active_tab = self.docs.active_pin();
        s.last_pinned = self.docs.last_pinned();
        s.seen = self.docs.seen();
        self.docs.focus_flush(Instant::now());
        s.focus = self.docs.focus();
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
        s.format_bar = self.docs.format_bar;
        s.check_updates = self.docs.check_updates;
        s.find_options = (self.ui.match_case, self.ui.whole_word, self.ui.regex);
        s.find_history = self.ui.find_recall.items().to_vec();
        s.replace_history = self.ui.replace_recall.items().to_vec();
        s.files = Some(self.ui.panes.files);
        s.pane = Some(self.ui.pane_face());
        s.files_w = Some(self.ui.panes.files_w.round() as u32);
        s.find_folder = self.ui.results.folder.text().to_string();
        s.find_kinds = self.ui.results.kinds.text().to_string();
        s.marks = self.docs.marks.snapshot(
            self.docs
                .tabs
                .iter()
                .filter_map(|d| Some((d.path.as_deref()?, d.ready()?.bookmarks.as_slice()))),
        );
        s.places = self.docs.places.snapshot(
            self.docs
                .tabs
                .iter()
                .filter_map(|d| Some((d.path.as_deref()?, d.ready()?.place()))),
        );
        s.folder = self.docs.folder.clone();
        if let Err(e) = s.save(&path) {
            eprintln!("mdedit: could not save the session: {e}");
            self.session_retry = Some(Instant::now() + SESSION_RETRY);
            return false;
        }
        self.session_retry = None;
        self.saved_sig = self.session_sig();
        true
    }
}

/// At the start: the recovery copies an earlier run left (`recovery`'s folder) and the backups the
/// last exit kept (`backups`, listed in `unsaved`) come back as tabs. Returns the tabs that read
/// their text once loaded, how many copies came back, and whether an entry of `unsaved` changed
/// (the caller writes the session at once then).
///
/// Which copy covers which backup is settled first ([`crate::recovery::cover`]: by draft id,
/// only when certain, only when the copy is the newer), then the copies come back, then the
/// backups. Nothing depends on the session being written afterwards: a copy and its backup name
/// the same draft id whatever the tabs are called (audit 1 round 4, #1).
fn bring_back(
    docs: &mut Docs,
    recovery: &mut crate::recovery::Recovery,
    unsaved: &mut [Unsaved],
    backups: Option<&Path>,
) -> (Vec<usize>, usize, bool) {
    let found = recovery.scan();
    let time = |u: &Unsaved| {
        let file = backups?.join(&u.file);
        std::fs::metadata(file).and_then(|m| m.modified()).ok()
    };
    let legacy = |u: &Unsaved| crate::recovery::legacy_draft(&backups?.join(&u.file), u);
    // Read only for a copy older than its backup: the same text means one draft, not two.
    let same_text = |f: &crate::recovery::Found, u: &Unsaved| {
        let Some(dir) = backups else { return false };
        let Ok(backup) = std::fs::read(dir.join(&u.file)) else {
            return false;
        };
        matches!(
            mdedit_core::recovery::read_checked(&f.file),
            Ok(Some((_, text))) if text.as_bytes() == backup.as_slice()
        )
    };
    let cover = crate::recovery::cover(&found, unsaved, time, legacy, same_text);
    let recovered = recovery.restore(docs, found);
    let covered: Vec<Option<DocId>> = cover
        .iter()
        .map(|c| c.and_then(|i| recovered.tabs[i]))
        .collect();
    // Unsaved text kept at the last exit comes back whatever "Restore tabs at start" says.
    let (mut restored, remapped) = match backups {
        Some(dir) => restore_unsaved(docs, unsaved, &covered, dir),
        None => (Vec::new(), false),
    };
    restored.extend(recovered.load.iter().copied());
    (restored, recovered.count, remapped)
}

/// Puts back the text that was unsaved when the window last closed, except where a recovery copy
/// has newer text of the same draft. Whatever "Restore tabs at start" says: closing never asks
/// about unsaved text, so it keeps it, and only tabs with nothing unsaved stay closed. Returns
/// the tabs that read their backup once loaded, and whether any entry changed.
///
/// `covered` has, for each entry, the tab of the recovery copy that covers it. That tab keeps the
/// backup as its fallback until the copy's text is in (audit 1 round 4, #2). Each entry is made
/// to describe the tab it stands for (its file, number and draft id), so the session keeps
/// naming its backup until the text is saved or discarded. A file tab already taking a copy's
/// text never has it replaced by a backup: the backup opens as a draft of its own.
fn restore_unsaved(
    docs: &mut Docs,
    unsaved: &mut [Unsaved],
    covered: &[Option<DocId>],
    dir: &Path,
) -> (Vec<usize>, bool) {
    let mut restored = Vec::new();
    let mut changed = false;
    let describe = |u: &mut Unsaved, d: &crate::docs::Doc| {
        let now = (d.path.clone(), d.untitled, Some(d.draft));
        let was = (u.path.clone(), u.untitled, u.draft);
        (u.path, u.untitled, u.draft) = now.clone();
        was != now
    };
    for (i, u) in unsaved.iter_mut().enumerate() {
        let file = dir.join(&u.file);
        if let Some(k) = covered
            .get(i)
            .copied()
            .flatten()
            .and_then(|id| docs.index_of_id(id))
        {
            let d = &mut docs.tabs[k];
            if d.restore.is_some() && d.fallback.is_none() && file.is_file() {
                d.fallback = Some(file);
            }
            changed |= describe(u, &docs.tabs[k]);
            continue;
        }
        if !file.is_file() {
            continue;
        }
        // Read now (a backup no tab waits for): one that cannot be read now stays listed and on
        // disk for the next start (audit 1 round 5, #3).
        let read_now = |docs: &mut Docs, u: &Unsaved, number: u32| match std::fs::read(&file) {
            Ok(bytes) => Some(docs.restore_untitled(number, &String::from_utf8_lossy(&bytes))),
            Err(e) => {
                eprintln!("mdedit: a backup cannot be read now ({e}); kept");
                docs.unread_backups.push(u.clone());
                None
            }
        };
        let k = match u.path.as_ref().filter(|p| p.is_file()) {
            Some(path) => {
                let k = docs.add_pending(path);
                if docs.tabs[k].restore.is_none() {
                    docs.tabs[k].restore = Some(file.clone());
                    restored.push(k);
                    Some(k)
                } else {
                    // The tab already takes other unsaved text of this file (a copy that is not
                    // certainly this draft's): this backup comes back as a draft of its own.
                    read_now(docs, u, 0)
                }
            }
            // A document that had no file, or whose file is gone since.
            None => read_now(docs, u, u.untitled),
        };
        if let Some(k) = k {
            // A backup from before 1.1 takes the id its file gives it, the same at every start.
            if let Some(d) = u.draft.or_else(|| crate::recovery::legacy_draft(&file, u))
                && !docs.tabs.iter().any(|t| t.draft == d)
            {
                docs.tabs[k].draft = d;
            }
            changed |= describe(u, &docs.tabs[k]);
        }
    }
    (restored, changed)
}

/// Whether the session's tab `i` is left out at start: the setting is on and the tab was looked
/// at for less than its seconds in all. The tab that was showing, a pinned one and one with unsaved
/// text always come back, and so does one the session has no count for (from before the count).
fn barely_seen(session: &Session, i: usize) -> bool {
    let p = &session.prefs;
    let Some(path) = session.tabs.get(i) else {
        return false;
    };
    let same = |q: &Path| mdshared_app::paths::same_path(q, path);
    let file = |pin: &crate::session::Pin| matches!(pin, crate::session::Pin::File(q) if same(q));
    // The tab that was showing is kept unless the owner asked for that one to count too.
    let showing =
        !p.brief_last && (i == session.active || session.active_tab.as_ref().is_some_and(file));
    let keep = showing
        || session.pinned.iter().any(file)
        || (session.unsaved.iter()).any(|u| u.path.as_deref().is_some_and(same));
    let counted = session.focus.iter().find(|(q, _)| same(q)).map(|&(_, s)| s);
    mdshared_app::brief::left_out(p.skip_brief, p.brief_secs, counted, keep)
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

/// The dark title bar and the corner rounding (Windows 11 draws it; Windows 10 stays square).
#[cfg(windows)]
fn style(w: &Window, dark: bool, corners: mdshared_ui::corners::Corners) {
    if let Some(h) = hwnd(w) {
        platform::style_window(h, dark);
        mdshared_ui::corners::set_window_corners(h, corners);
    }
}

#[cfg(windows)]
fn frame_report(w: &Window) -> String {
    hwnd(w).map(platform::frame_report).unwrap_or_default()
}

impl App {
    /// The window's events, without the loop they came from: `window_event` itself needs a live
    /// event loop, and a test drives the events here.
    pub(super) fn on_window_event(&mut self, event: WindowEvent) {
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
                    self.motion.refresh();
                    self.focus_gained();
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
            WindowEvent::MouseInput { state, button, .. } => {
                // A viewer closed by hand is found out at the next click if the focus did not.
                if state == ElementState::Pressed {
                    self.docs.links.check_on_input(Instant::now());
                }
                // A press on the page leaves the lead where it is: a selection there does not
                // move it.
                if state == ElementState::Pressed
                    && self.preview.is_some()
                    && !self.on_page(self.ui.mouse.0, self.ui.mouse.1)
                {
                    self.editor_leads();
                }
                match (state, button) {
                    // On a Mac, Control and a click is a right click (`Mods::control` is never
                    // set elsewhere).
                    (ElementState::Pressed, MouseButton::Left) if self.mods.control => {
                        self.control_click = true;
                        self.right_press();
                    }
                    (ElementState::Released, MouseButton::Left)
                        if std::mem::take(&mut self.control_click) => {}
                    (ElementState::Pressed, MouseButton::Left) => self.left_press(),
                    (ElementState::Released, MouseButton::Left) => self.left_release(),
                    (ElementState::Pressed, MouseButton::Right) => self.right_press(),
                    (ElementState::Pressed, MouseButton::Middle) => self.middle_press(),
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // The wheel over the page makes the page lead again (`preview_wheel`).
                self.editor_leads();
                self.wheel(delta)
            }
            // A trackpad: a pinch zooms in steps; the smart-zoom tap (two fingers, twice) is the
            // actual size. winit reports both on macOS only.
            WindowEvent::PinchGesture { delta, phase, .. } => {
                let steps = self.pinch.add(delta);
                for _ in 0..steps.unsigned_abs() {
                    self.command(if steps > 0 { C::ZoomIn } else { C::ZoomOut });
                }
                if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                    self.pinch.end();
                }
            }
            WindowEvent::DoubleTapGesture { .. } => self.command(C::ZoomReset),

            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                if matches!(event.logical_key, Key::Named(NamedKey::Alt)) {
                    return;
                }
                self.editor_leads();
                // A key's press asks, once in a while; its release and its auto-repeat do not.
                self.ask_on_key(event.state, event.repeat);
                self.key(&event);
            }
            // Text an input method made: an accent typed with a dead key or chosen by holding a
            // key, or Chinese, Japanese or Korean input. Input methods are on only on macOS.
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => self.ime_commit(&text),
            // The input method's candidate window goes under the caret.
            // What it is composing shows where it will go, and its candidate window goes under
            // the caret.
            WindowEvent::Ime(winit::event::Ime::Preedit(text, cursor)) => {
                self.ime_preedit(text, cursor)
            }
            WindowEvent::Ime(winit::event::Ime::Enabled) => self.ime_area(),
            WindowEvent::Ime(winit::event::Ime::Disabled) => self.ime_preedit(String::new(), None),
            WindowEvent::DroppedFile(path) => self.dropped(path),
            _ => {}
        }
    }

    /// A key asks after the viewer: its press, not its release, and not its auto-repeat while
    /// the key is held. `check_on_input` keeps the asks two seconds apart.
    fn ask_on_key(&mut self, state: ElementState, repeat: bool) {
        if state == ElementState::Pressed && !repeat {
            self.docs.links.check_on_input(Instant::now());
        }
    }
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
        // A test run: shown without the focus and kept under every other window.
        if self.args.behind {
            attributes = attributes
                .with_active(false)
                .with_window_level(WindowLevel::AlwaysOnBottom);
        }
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
        // Rounded corners need the margin on KDE too (KWin's route is opaque).
        let corners = self.session.corners;
        let attributes = mdshared_ui::shadow::Shadow::prepare_corners(
            attributes,
            mdshared_ui::shadow::Mode::from_env(),
            corners,
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
        mdshared_ui::window::created(&window);
        // macOS: the menu bar, with Quit as the Exit command, which asks about unsaved changes
        // (winit's ends the process at once).
        let wake = self.docs.waker();
        mdshared_ui::menu_bar::install("mdedit", move |c| wake(Wake::Menu(c)));
        self.trace.stage("window created");
        self.scale = window.scale_factor() as f32;
        self.maximized = window.is_maximized();
        self.shadow = mdshared_ui::shadow::Shadow::attach_corners(
            &window,
            mdshared_ui::shadow::Mode::from_env(),
            corners,
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
        style(&window, self.dark(), corners);
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
                // The frost may already be on (a first frame with the setting set).
                #[cfg(not(windows))]
                self.sync_surface_alpha();
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
        self.on_window_event(event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Focused time: the tab in front while the window has the focus.
        let showing = self
            .docs
            .active_doc()
            .map(|d| d.id)
            .filter(|_| self.focused);
        self.docs.focus_tick(showing, Instant::now());
        self.library_tick(Instant::now());
        // A folder read for the library that waited for another card to close (audit 4, C8).
        self.import_take();
        if self.drain() {
            self.redraw();
        }
        if self.exit {
            // The viewer goes back to the files it was showing the text of, and to where its
            // window was.
            self.docs.links.finish();
            // The size is saved once: the loop comes here again before it ends, and a second
            // pass would save the window as it is now, not as it was before a side-by-side
            // preview.
            let first = !std::mem::replace(&mut self.size_saved, true);
            if first && !self.leave_side_by_side() {
                self.save_size();
            }
            self.finish_session();
            self.trace.cpu("exit");
            event_loop.exit();
            if first {
                // The loop reads an exit asked for here only after the next event, and a close
                // that comes from the system alone (the taskbar's "Close window") is followed
                // by none: the window stayed open until the pointer moved. This is that event.
                (self.docs.waker())(Wake::Instance);
            }
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
        self.recover_due = self.recovery.poll(&self.docs, Instant::now());
        self.link_due = self.docs.link_poll(Instant::now());
        // However a link ended in this pass, an open menu no longer checks it.
        self.follow_links();
        if self.preview.is_some() && self.preview_sync() {
            self.trace_preview_page();
            self.redraw();
        }
        self.results_due = self.results_sync(Instant::now());
        // The last preview is over (stopped, its tab closed, the viewer gone): the windows go
        // back to where they were.
        if self.tiling.is_some() && self.docs.links.is_empty() {
            self.end_side_by_side();
        }
        if !self.settled && !self.first && self.is_settled() {
            self.settled = true;
            // Off the path to the first frame: whether mdview is there to offer previews.
            self.look_for_viewer();
            self.look_for_crash();
            // The system's wish for less motion, read and watched before the first menu opens
            // (Linux's portal answers on a helper thread); nothing when motion is off.
            self.watch_motion();
            self.trace.stage("loading and colouring idle");
            self.trace.memory("idle");
            self.trace.cpu("idle");
            if self.args.preview_pane && self.preview.is_none() {
                self.toggle_preview_pane();
            }
        }
        if self.type_steps.is_some() && self.settled && self.preview_ready() {
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
        let next = self.next_wake(Instant::now());
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
    /// The height of what shows under the command bar: the format bar, the replace strip, the
    /// update bar, then the crash bar.
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
        let crash = if self.docs.crash.is_some() {
            view::BANNER_H
        } else {
            0.0
        };
        let format = if self.format.is_some() {
            crate::format_bar::FORMAT_H
        } else {
            0.0
        };
        let stale = if self.docs.stale.is_empty() {
            0.0
        } else {
            view::BANNER_H
        };
        let rename = if self.docs.rename_bar().is_some() {
            view::BANNER_H
        } else {
            0.0
        };
        let ai = if self.ai.bar.is_some() {
            view::BANNER_H
        } else {
            0.0
        };
        format + strip + update + crash + stale + rename + ai
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
        .with_results(self.ui.results.open, self.ui.results.height);
        // A split whose second tab was closed, or became the active one, is over.
        if self.docs.other.is_some() && self.docs.pane1().is_none() {
            self.docs.other = None;
        }
        let visual = self.visual_on();
        let areas = areas
            .with_split(
                self.docs.other.is_some() && !visual,
                self.docs.other_left,
                self.docs.split_ratio,
            )
            .with_preview(self.preview.is_some() && !visual, self.docs.split_ratio)
            .with_minimap(self.docs.minimap && self.docs.active().is_some() && !visual);
        let areas = self.with_visual(areas);
        if areas != self.areas {
            self.areas = areas;
            self.full = true;
        }
    }

    /// A switch to Markdown the AI made, undone since: taken back (with its rename bar).
    fn drop_undone_switches(&mut self) {
        let dark = self.dark();
        if self.docs.drop_undone_switches(self.service.as_ref(), dark) {
            self.full = true;
        }
    }

    /// The layout pass of a frame without drawing: areas, scroll clamping, colour requests.
    fn compose_layout_only(&mut self) {
        self.drop_undone_switches();
        self.format = crate::format_bar::shown(&self.docs);
        self.sync_side_by_side();
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
        .with_results(self.ui.results.open, self.ui.results.height)
        .with_preview(
            self.preview.is_some() && !self.visual_on(),
            self.docs.split_ratio,
        )
        .with_minimap(self.docs.minimap && self.docs.active().is_some() && !self.visual_on());
        self.areas = self.with_visual(self.areas);
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
            self.library_read_abandon();
            self.ui.set_pane_face(crate::ui::PaneFace::Functions);
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
    // The mark of a preview, with nothing started or sent. With an mdview mode chosen for the
    // Preview button, drawn as with mdview found (the same on every machine) and side by side
    // when that is the mode, so that the shot shows the button lit.
    if app.args.demo_link && app.docs.prefs.preview_mode != mdedit_core::prefs::PreviewMode::Window
    {
        app.docs.viewer = true;
    }
    if app.args.demo_link
        && let Some((id, path, name)) = app
            .docs
            .active_doc()
            .map(|d| (d.id, d.path.clone(), d.name()))
    {
        let now = Instant::now();
        match path {
            Some(path) => app.docs.links.link(id, path.into(), None, None, now),
            None => (app.docs.links).link_untitled(id, name, crate::link::Opening::default(), now),
        }
        app.docs.links.side_by_side =
            chrome::preview_mode(&app.docs) == mdedit_core::prefs::PreviewMode::SideBySide;
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
    // A report from last time shows its bar in a picture too.
    app.look_for_crash();
    if app.args.preview_pane {
        app.look_for_viewer();
        app.toggle_preview_pane();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.preview_ready() && Instant::now() < deadline {
            app.compose_layout_only();
            app.preview_sync();
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
    // `--menu-fade-ms`: the menu part way into its entrance, on a clock moved by hand.
    let fade = app.args.menu_fade_ms.filter(|_| app.args.menu.is_some());
    if fade.is_some() {
        app.motion.shot_fade();
    }
    match app.args.menu {
        Some(ShotMenu::Main) => app.open_popup(PopupKind::Main),
        Some(ShotMenu::MainEnd) => {
            app.open_popup(PopupKind::Main);
            if let Some(p) = app.ui.popup.as_mut() {
                p.menu.scroll_to_end();
            }
        }
        Some(ShotMenu::Recent) => app.open_popup(PopupKind::Recent),
        Some(ShotMenu::Preview) => {
            // The picture is the same on every machine: as it is with mdview found.
            app.docs.viewer = true;
            app.compose();
            app.open_popup(PopupKind::Preview);
        }
        Some(ShotMenu::BarAi) => app.open_popup(PopupKind::Ai),
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
        Some(ShotMenu::Complete) => app.open_completion(true),
        Some(ShotMenu::CompleteSnippet) => {
            app.open_completion(true);
            app.completion_accept_first();
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
        Some(
            m @ (ShotMenu::TextLines
            | ShotMenu::TextConvert
            | ShotMenu::TextMarkdown
            | ShotMenu::TextAi),
        ) => {
            app.ui.mouse = (
                text.x + text.w * app.args.menu_at.unwrap_or(0.3),
                text.y + 160.0,
            );
            app.right_press();
            app.compose();
            let sub = match m {
                ShotMenu::TextLines => PopupKind::Lines,
                ShotMenu::TextConvert => PopupKind::Convert,
                ShotMenu::TextAi => PopupKind::Ai,
                _ => PopupKind::Markdown,
            };
            if let Some(p) = app.ui.popup.as_mut() {
                p.menu.hover = p
                    .menu
                    .items
                    .iter()
                    .position(|i| i.id.submenu() == Some(sub));
            }
            app.open_sub(PopupKind::Text, sub, true);
        }
        Some(ShotMenu::Language) => {
            app.compose();
            app.activate(Hit::StatusLanguage);
        }
        Some(ShotMenu::MainLibrary) => {
            app.open_popup(PopupKind::Main);
            app.compose();
            if let Some(p) = app.ui.popup.as_mut() {
                p.menu.hover = p.menu.items.iter().position(|i| i.id == Act::LibraryMenu);
            }
            app.open_sub(PopupKind::Main, PopupKind::Library, true);
        }
        Some(ShotMenu::Library) => app.command(crate::commands::CommandId::Library),
        Some(ShotMenu::LibraryAdd) => app.command(crate::commands::CommandId::LibraryAdd),
        Some(ShotMenu::LibraryPane) => {
            app.command(crate::commands::CommandId::ToggleLibraryPane);
        }
        Some(ShotMenu::LibraryCursor) => {
            app.command(crate::commands::CommandId::ToggleLibraryPane);
            app.compose();
            app.library_focus();
            for _ in 0..4 {
                app.ui.library.step(true);
            }
            app.compose();
        }
        Some(
            m @ (ShotMenu::LibraryRow
            | ShotMenu::LibrarySection
            | ShotMenu::LibraryEmpty
            | ShotMenu::LibraryRename
            | ShotMenu::LibraryNewGroup
            | ShotMenu::LibraryRemove),
        ) => {
            use crate::library_pane::Kind;
            use crate::ui::LibAct;
            app.command(crate::commands::CommandId::ToggleLibraryPane);
            app.compose();
            let first = |app: &App, note: bool| {
                app.ui.library.rows.iter().position(|r| {
                    if note {
                        matches!(r.kind, Kind::Note { .. })
                    } else {
                        r.kind == Kind::Section
                    }
                })
            };
            match m {
                ShotMenu::LibraryEmpty => {
                    if let Some(r) = app.areas.files {
                        app.ui.mouse = (r.cx(), r.bottom() - 40.0);
                        app.right_press();
                    }
                }
                ShotMenu::LibraryRename => {
                    if let Some(i) = first(&app, true) {
                        app.act(Act::Library(LibAct::Rename, i));
                        app.ui.library.field.set_text("Receipts 2026");
                    }
                }
                ShotMenu::LibraryNewGroup => {
                    app.act(Act::Library(LibAct::NewGroup, 0));
                    app.ui.library.field.set_text("Work: 2026");
                }
                ShotMenu::LibraryRemove => {
                    if let Some(i) = first(&app, false) {
                        app.act(Act::Library(LibAct::Remove, i));
                    }
                }
                _ => {
                    let note = m == ShotMenu::LibraryRow;
                    if let Some(r) =
                        first(&app, note).and_then(|i| app.hit_rect(Hit::LibraryRow(i)))
                    {
                        app.ui.mouse = (r.x + 80.0, r.cy());
                        app.right_press();
                    }
                }
            }
        }
        Some(ShotMenu::RenameBar) => {
            if let Some(doc) = app.docs.active_doc().map(|d| d.id) {
                app.ai_made_markdown(doc, None);
            }
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
        Some(ShotDialog::Settings) => app.open_settings(),
        Some(ShotDialog::Unsaved) => {
            if let Some(id) = app.docs.active_doc().map(|d| d.id) {
                app.ask(DialogKind::Unsaved, id, After::Nothing);
            }
        }
        Some(
            ShotDialog::Runs
            | ShotDialog::RunUndo
            | ShotDialog::RunUndoRecycled
            | ShotDialog::RunsNewer
            | ShotDialog::RunUndoOriginal
            | ShotDialog::RunUndoLater
            | ShotDialog::RunUndoneLater
            | ShotDialog::RunUndoMoved
            | ShotDialog::RunUndoneMoved
            | ShotDialog::RunUndoPictures
            | ShotDialog::RunUndonePictures,
        ) => {
            let which = app.args.dialog;
            app.runs_demo(which);
        }
        Some(ShotDialog::LibraryDelete | ShotDialog::LibraryDeleteLong) => {
            app.open_dialog(DialogKind::LibraryDelete);
            if let Some(d) = app.ui.dialog.as_mut() {
                d.subject = if app.args.dialog == Some(ShotDialog::LibraryDeleteLong) {
                    strings::library_delete_body(
                        "Japan 2025, Tokyo, Kyoto and the long way home",
                        "japan-2025-tokyo-kyoto-and-the-long-way-home.md",
                        23,
                    )
                } else {
                    strings::library_delete_body("Japan 2025", "japan-2025.md", 23)
                };
            }
        }
        Some(ShotDialog::LibraryRemoveLong) => {
            app.open_dialog(DialogKind::LibraryRemove);
            if let Some(d) = app.ui.dialog.as_mut() {
                d.subject = strings::library_remove_body(
                    "Japan 2025: Tokyo, Kyoto, Osaka, Nara and the long way home, every stop",
                    23,
                );
            }
        }
        None => {}
    }
    if let Some(q) = app.args.palette.clone() {
        app.open_palette(&q);
        app.settle_palette();
    }
    app.shot_replace();
    if let Some(kind) = app.args.ai_demo.clone() {
        app.ai_demo(&kind);
    }
    // After the AI demonstration, which opens Settings on AI service and fills what the engine
    // would have said: a section named shows its page.
    if let Some(name) = app.args.settings_section.clone() {
        app.show_settings_section(&name);
    }
    if let Some(kind) = app.args.plan_demo.clone() {
        app.plan_demo(&kind);
    }
    if let Some(kind) = app.args.move_demo.clone() {
        app.move_demo(&kind);
    }
    if let Some(kind) = app.args.task_demo.clone() {
        app.task_demo(&kind);
    }
    if let Some(kind) = app.args.note_demo.clone() {
        app.note_demo(&kind);
    }
    if let Some(kind) = app.args.library_demo.clone() {
        app.library_demo(&kind);
    }
    // A plan a demonstration opened, ticked and left: the Notes pane's "Plan ready" row (5.3).
    if app.args.plan_parked && app.plan_is_open() {
        use crate::keys::{Key, KeyName};
        app.command(crate::commands::CommandId::ToggleLibraryPane);
        app.key_pressed(Key::Name(KeyName::Space), None);
        app.key_pressed(Key::Name(KeyName::Escape), None);
        // The pane opened after the first frame: every region is painted again for the shot.
        app.sigs = [0; 10];
    }
    if app.ui.dialog.is_some()
        || app.ui.popup.is_some()
        || app.palette_open()
        || app.plan_is_open()
        || app.task_is_open()
    {
        app.compose();
    }
    if let Some(ms) = fade {
        for step in crate::motion::shot_steps(ms) {
            app.motion.advance_ms(step);
            app.compose();
        }
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
    if app.args.demo_recovered.is_none() {
        app.docs.clear_toast();
    }
    app.full = true;
    app.compose();
    // `--frame`: the window as the client-drawn margin puts it on screen.
    let framed = app
        .args
        .frame
        .map(|c| frame_of(&app.screen, c, app.scale as f64, dark));
    match framed.as_ref().unwrap_or(&app.screen).write_png(path) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("mdedit: could not write {}: {e}", path.display());
            1
        }
    }
}

/// What `--frame` renders: `content` with the shadow margin and `corners`, composited over a flat
/// backdrop (a mid grey, lighter for a dark window so its shadow shows), by the same code the
/// client-drawn window uses (`shadow::Frame`). The picture is larger than the content by the margin.
fn frame_of(content: &Bitmap, corners: Corners, scale: f64, dark: bool) -> Bitmap {
    let frame = mdshared_ui::shadow::Frame::with_radius(scale, true, corners.px(scale));
    let backdrop = if dark {
        [0x5a, 0x5a, 0x5f]
    } else {
        [0xc8, 0xc8, 0xcd]
    };
    let (width, height, pixels) =
        frame.preview(&content.pixels, content.width, content.height, backdrop);
    Bitmap {
        width,
        height,
        pixels,
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
    if args.install_update {
        return mdshared_app::updater::install_and_restart(
            &crate::updater::APP,
            &crate::updater::fetch,
        );
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
    // A `--no-session` window reads no settings at all -- so it has no library, no snippets and
    // no saved prompts -- unless `MDEDIT_SETTINGS` names a settings file for this build. That
    // file is read for settings and its library is used; the session itself is still not read or
    // written (CLAUDE.md rule 9).
    let settings_override = if args.no_session {
        explicit.clone()
    } else {
        None
    };
    // A shot never touches the user's session; it may read a scratch one named by MDEDIT_SETTINGS.
    let mut session_path = match (args.no_session, args.shot.is_some()) {
        (true, _) => None,
        (false, true) => explicit,
        (false, false) => {
            explicit.or_else(|| mdshared_app::settings::default_settings_path("mdedit"))
        }
    };
    // The settings path this window listens on for other launches, kept even when the session
    // itself is not used (`session_path` set to `None` below).
    let instance_path = session_path.clone().filter(|_| args.shot.is_none());
    // Held while the window runs: this launch has the settings folder (mdshared_app::folder_lock).
    let mut _folder_lock = None;
    // Why this window runs without its session, to say once it is open.
    let mut no_session_note: Option<String> = None;
    if let Some(settings) = session_path.clone().filter(|_| args.shot.is_none()) {
        let settings = &settings;
        // Started by a restart: the window that started it is still closing; wait it out, or the
        // hand-over below would go to it and leave no window.
        mdshared_app::relaunch::wait_for_predecessor(std::env::args().skip(1), settings);
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
        let forward = || {
            if requests.is_empty() {
                mdshared_app::single::forward("mdedit", settings, &paths)
            } else {
                requests
                    .iter()
                    .all(|r| mdshared_app::single::forward_request("mdedit", settings, r))
            }
        };
        // A window that is still starting is waited for, not run beside.
        match mdshared_app::folder_lock::claim(settings, forward, mdshared_app::folder_lock::WAIT) {
            mdshared_app::folder_lock::Claim::Own(lock) => _folder_lock = lock,
            mdshared_app::folder_lock::Claim::Handed => return 0,
            mdshared_app::folder_lock::Claim::Busy => {
                session_path = None;
                no_session_note = Some(crate::commands::strings::SESSION_BUSY.to_string());
            }
        }
    }
    // A live update is for a window that shows the file already: with none, there is nothing to do.
    if args.follow && args.shot.is_none() {
        return 0;
    }
    // A session file that is there but cannot be read (another program has it open a moment, a
    // disk error) is tried again for a while; failing that, this window runs without it, so it
    // neither writes over it nor drops the drafts it lists (audit 2).
    let session = match session_path.as_deref().map(Session::read_patiently) {
        None => Session::default(),
        Some(Ok(s)) => s,
        Some(Err(e)) => {
            session_path = None;
            no_session_note = Some(crate::commands::strings::session_unreadable(&e));
            Session::default()
        }
    };
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
    docs.format_bar = session.format_bar;
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
    docs.marks = crate::marks::Marks::from_session(&session.marks);
    docs.places = crate::places::Places::from_session(&session.places);
    let mut active = None;
    let restore = session.prefs.restore_session;
    for (i, path) in session.tabs.iter().enumerate().filter(|_| restore) {
        if path.is_file() && !barely_seen(&session, i) {
            let k = docs.add_pending(path);
            if i == session.active {
                active = Some(k);
            }
        }
    }
    // Text a run that did not end normally left behind: one listing of its folder.
    let mut recovery = crate::recovery::Recovery::new(
        session_path
            .as_deref()
            .filter(|_| args.shot.is_none())
            .and_then(crate::recovery::dir_of),
    );
    let backups = session_path.as_deref().and_then(backup_dir_of);
    let (restored, recovered, remapped) = bring_back(
        &mut docs,
        &mut recovery,
        &mut session.unsaved,
        backups.as_deref(),
    );
    docs.start_day(&mut session);
    docs.start_focus(&session);
    // The pinned tabs are marked now and put first once every index taken here is used.
    if restore {
        docs.mark_pinned(&session.pinned);
        active = docs.start_tab(&session, active);
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
    // `--new`: a document that has no file, in front.
    if args.new_document {
        active = Some(docs.new_untitled());
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
    let mut app = App::new(args, docs, trace, session_path, session, settings_override);
    app.compare_wait = compare_ids;
    app.recovery = recovery;
    let recovered_count = app.args.demo_recovered.unwrap_or(recovered);
    if recovered_count > 0 {
        app.docs
            .set_toast(&crate::commands::strings::recovered(recovered_count));
    }
    if let Some(note) = &no_session_note {
        app.docs.set_toast(note);
    }
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
    let pins = app.session.pinned.clone();
    app.docs.pinned_first(&pins);
    if app.docs.prefs.offer_stale {
        app.docs.offer_stale_tabs();
    }
    // The split view comes back when both its files did (and the second is not the active one).
    if let Some(split) = app.session.split.clone() {
        app.docs
            .restore_split(&split.path, split.left, split.ratio());
    }
    let a = app.args.clone();
    restore_pane(&mut app.ui, &app.session, &a);
    app.ui.results.folder.set_text(&app.session.find_folder);
    app.ui.results.kinds.set_text(&app.session.find_kinds);
    if let Some(h) = app.session.results_h {
        app.ui.results.height = (h as f32).max(crate::results::RESULTS_MIN_H);
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
    // A backup that now names another document is written down before any recovery copy can be
    // written under that name (audit 1 round 3, #5).
    if remapped && !app.save_session() {
        // Not written: the next pass of the loop tries again.
        app.saved_sig = 0;
    }
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
    // macOS hands a running bundle its files as an event, also the files it was launched with.
    let po = p.clone();
    mdshared_ui::open_documents::on_open(move |paths| {
        let _ = po.send_event(Wake::Open(paths));
    });
    trace.stage("event loop");
    if let Some(settings) = &instance_path {
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
    // A restart starts the new version last, with the window gone and the session written. The
    // new process waits for this one to end before it looks for a running instance.
    let restart = app.restart.take();
    drop(app);
    if let Some(target) = restart
        && let Err(e) = mdshared_app::relaunch::spawn(&target)
    {
        eprintln!("mdedit: could not start {}: {e}", target.display());
    }
    0
}

/// A content size in device px as the logical size the session keeps (and `with_inner_size`
/// takes back, before `Shadow::prepare` adds the margin).
/// The left pane at start: shut or open, which face it showed, and how wide. The face comes
/// back unless the command line asked for the pane (`--files`, or a folder to open). Only the
/// flags are set: the library is read when the pane's first frame is due, not here.
fn restore_pane(ui: &mut Ui, session: &Session, a: &Args) {
    let forced = a.folder.is_none() && a.files.iter().any(|f| f.is_dir());
    ui.panes.files = a.files_pane.or(session.files).unwrap_or(false) || forced;
    if ui.panes.files
        && a.files_pane.is_none()
        && !forced
        && let Some(face) = session.pane
    {
        ui.set_pane_face(face);
    }
    if let Some(w) = session.files_w {
        ui.panes.files_w = (w as f32).clamp(PANE_MIN, PANE_MAX);
    }
}

fn logical_size((w, h): (u32, u32), scale: f64) -> (u32, u32) {
    let size = winit::dpi::PhysicalSize::new(w, h).to_logical::<f64>(scale);
    (size.width.round() as u32, size.height.round() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Key;
    use crate::session::TabState;
    use mdshared_ui::keys::KeyName;

    fn app() -> App {
        let args = Args::parse(["--no-session".to_string()]);
        let mut docs = Docs::new(1254, Arc::new(|_| {}));
        docs.new_untitled();
        let trace = Trace {
            on: false,
            started: Instant::now(),
        };
        App::new(args, docs, trace, None, Session::default(), None)
    }

    /// The update bar offers Restart in the installed state only, and only with a file to start.
    #[test]
    fn the_bar_shows_restart_only_when_installed_with_a_file_to_start() {
        let mut app = app();
        app.size = (900, 600);
        for (stage, restart) in [
            ("available", false),
            ("installing", false),
            ("installed-plain", false),
            ("installed", true),
        ] {
            app.docs.updates.preview(stage);
            app.compose();
            assert_eq!(
                app.hit_rect(Hit::UpdateRestart).is_some(),
                restart,
                "{stage}"
            );
            assert_eq!(
                app.hit_rect(Hit::UpdateGo).is_some(),
                stage == "available",
                "{stage}"
            );
        }
    }

    /// Restart is Exit that then starts the new version: with nothing unsaved it closes at once.
    #[test]
    fn restart_closes_like_exit_and_remembers_what_to_start() {
        let mut app = app();
        app.docs.updates.preview("installed");
        app.activate(Hit::UpdateRestart);
        assert!(app.exit);
        assert_eq!(app.restart, Some(PathBuf::from("/preview/app")));
        // Exit itself starts nothing.
        let mut plain = self::app();
        plain.docs.updates.preview("installed");
        plain.request(After::Exit);
        assert!(plain.exit);
        assert_eq!(plain.restart, None);
    }

    /// With unsaved changes and no session to keep them in, Restart asks what Exit asks, and
    /// cancelling it aborts the restart: nothing closes and nothing is remembered to start.
    #[test]
    fn restart_asks_about_unsaved_changes_and_a_cancel_aborts_it() {
        let mut app = app();
        app.docs.updates.preview("installed");
        assert!(app.docs.edit(EditOp::Insert("x"), None));
        app.activate(Hit::UpdateRestart);
        assert!(!app.exit, "asked first");
        assert_eq!(
            app.ui.dialog.as_ref().map(|d| d.kind),
            Some(DialogKind::Unsaved)
        );
        assert_eq!(app.ui.pending.map(|p| p.after), Some(After::Restart));
        // Cancel (the third button).
        app.answer(2);
        assert!(!app.exit && app.restart.is_none());
        assert!(app.ui.pending.is_none());
        // Discard goes on to the restart.
        app.activate(Hit::UpdateRestart);
        app.answer(1);
        assert!(app.exit);
        assert_eq!(app.restart, Some(PathBuf::from("/preview/app")));
    }

    /// Audit 1, finding 1 and round 2: with "Restore tabs at start" off, drafts kept at exit
    /// come back at every start, and the session keeps naming their backups after the first
    /// pruning, also for a document whose file is gone and an untitled number already taken.
    #[test]
    fn kept_drafts_survive_restarts_with_restore_off() {
        let dir = std::env::temp_dir().join(format!("mdedit-kept-drafts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let backups = dir.join("backup");
        std::fs::create_dir_all(&backups).unwrap();
        let file = dir.join("gone.txt");
        std::fs::write(&file, "file text\n").unwrap();
        let fresh = || {
            let mut d = Docs::new(1254, Arc::new(|_| {}));
            d.prefs.restore_session = false;
            d
        };
        // The first run: an untitled draft, a draft of a file, and Exit keeps both.
        let mut docs = fresh();
        docs.new_untitled();
        assert!(docs.edit(EditOp::Insert("draft"), None));
        let i = docs.open(&file);
        docs.load_now(i, None, false);
        docs.activate(i);
        assert!(docs.edit(EditOp::Insert("edited "), None));
        let mut unsaved = docs.write_backups(&backups).unwrap();
        assert_eq!(unsaved.len(), 2);
        // The file goes; the next start has another untitled tab holding number 1.
        std::fs::remove_file(&file).unwrap();
        for run in 0..3 {
            let mut docs = fresh();
            docs.new_untitled();
            restore_unsaved(&mut docs, &mut unsaved, &[], &backups);
            let texts: Vec<String> = docs
                .tabs
                .iter()
                .filter(|d| d.modified())
                .map(|d| d.ready().unwrap().buffer.rope().to_string())
                .collect();
            assert!(texts.contains(&"draft".to_string()), "run {run}: {texts:?}");
            assert!(
                texts.contains(&"edited file text\n".to_string()),
                "run {run}"
            );
            // The session's first pruning keeps both.
            let tabs: Vec<TabState> = docs.tabs.iter().map(|d| d.tab_state()).collect();
            let kept = Unsaved::prune(&unsaved, &tabs);
            assert_eq!(kept.len(), 2, "run {run}: {kept:?}");
            unsaved = docs.write_backups(&backups).unwrap();
            assert_eq!(unsaved.len(), 2, "kept again by exit {run}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 1 round 3, #1 and #5: a draft of a file that is gone and an untitled draft with a
    /// newer recovery copy both came back, and a second interruption before any new copy was
    /// written. Both texts come back at every start, and an exit keeps both; the session entries
    /// name the tabs they stand for, so Save As on one drops only its own entry.
    #[test]
    fn two_drafts_survive_a_second_interruption() {
        use crate::recovery::Recovery;
        use std::time::{Duration, Instant};
        let dir = std::env::temp_dir().join(format!("mdedit-two-drafts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let backups = dir.join("backup");
        std::fs::create_dir_all(&backups).unwrap();
        let file = dir.join("gone.txt");
        std::fs::write(&file, "file text\n").unwrap();
        let fresh = || {
            let mut d = Docs::new(1254, Arc::new(|_| {}));
            d.prefs.restore_session = false;
            d
        };
        let texts = |docs: &Docs| {
            let mut t: Vec<String> = docs
                .tabs
                .iter()
                .filter(|d| d.modified())
                .map(|d| d.ready().unwrap().buffer.rope().to_string())
                .collect();
            t.sort();
            t
        };
        // The first run: a draft of a file and an untitled draft, kept by Exit.
        let mut docs = fresh();
        let i = docs.open(&file);
        docs.load_now(i, None, false);
        docs.activate(i);
        assert!(docs.edit(EditOp::Insert("edited "), None));
        let u = docs.new_untitled();
        docs.activate(u);
        assert!(docs.edit(EditOp::Insert("draft"), None));
        let mut unsaved = docs.write_backups(&backups).unwrap();
        assert_eq!(unsaved.len(), 2);
        // The second run types on in the untitled draft; its copy is written, then it dies.
        let mut rec = Recovery::new(Some(dir.join("recover")));
        assert!(docs.edit(EditOp::Insert(" newer"), None));
        let t0 = Instant::now();
        rec.poll(&docs, t0);
        rec.poll(&docs, t0 + Duration::from_secs(60));
        rec.flush();
        drop(rec);
        std::fs::remove_file(&file).unwrap();
        let want = vec!["draft newer".to_string(), "edited file text\n".to_string()];
        // Two starts that both die before writing anything new, then one that exits.
        for run in 0..3 {
            let mut docs = fresh();
            let mut rec = Recovery::new(Some(dir.join("recover")));
            let (_, _, remapped) = bring_back(&mut docs, &mut rec, &mut unsaved, Some(&backups));
            assert_eq!(texts(&docs), want, "run {run}");
            // The entries name distinct tabs.
            let mut names: Vec<(bool, u32)> = unsaved
                .iter()
                .map(|u| (u.path.is_some(), u.untitled))
                .collect();
            names.dedup();
            assert_eq!(names.len(), 2, "run {run}: {unsaved:?}");
            let tabs: Vec<TabState> = docs.tabs.iter().map(|d| d.tab_state()).collect();
            assert_eq!(Unsaved::prune(&unsaved, &tabs).len(), 2, "run {run}");
            // The file's draft took a new number once, and the session was told.
            assert_eq!(remapped, run == 0, "run {run}");
            if run == 2 {
                // Save As on the recovered draft drops its entry, and only its entry.
                let k = (0..docs.tabs.len())
                    .find(|&k| docs.tabs[k].ready().unwrap().buffer.rope() == "draft newer")
                    .unwrap();
                docs.activate(k);
                docs.save_as(k, &dir.join("saved.txt"), None, false)
                    .unwrap();
                let tabs: Vec<TabState> = docs.tabs.iter().map(|d| d.tab_state()).collect();
                let kept = Unsaved::prune(&unsaved, &tabs);
                assert_eq!(kept.len(), 1, "{kept:?}");
                rec.finish(&docs, true);
                unsaved = docs.write_backups(&backups).unwrap();
                assert_eq!(unsaved.len(), 1);
            }
        }
        // What is left on disk is the file's draft.
        let mut docs = fresh();
        let mut rec = Recovery::new(Some(dir.join("recover")));
        bring_back(&mut docs, &mut rec, &mut unsaved, Some(&backups));
        assert_eq!(texts(&docs), ["edited file text\n"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A start for the round 4 tests: the documents, as `two_drafts_survive_a_second_interruption`.
    fn r4_docs() -> Docs {
        let mut d = Docs::new(1254, Arc::new(|_| {}));
        d.prefs.restore_session = false;
        d
    }

    /// The texts of the modified tabs, sorted; tabs waiting for their text read it first.
    fn r4_texts(docs: &mut Docs) -> Vec<String> {
        for k in 0..docs.tabs.len() {
            if docs.tabs[k].ready().is_none() {
                docs.load_now(k, None, false);
            }
        }
        let mut t: Vec<String> = docs
            .tabs
            .iter()
            .filter(|d| d.modified())
            .map(|d| d.ready().unwrap().buffer.rope().to_string())
            .collect();
        t.sort();
        t
    }

    /// Writes the recovery copies of `docs` as a run that then dies does.
    fn r4_copies(docs: &Docs, rec: &mut crate::recovery::Recovery) {
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        rec.poll(docs, t0);
        rec.poll(docs, t0 + Duration::from_secs(60));
        rec.flush();
    }

    fn r4_dir(name: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("mdedit-r4-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let backups = dir.join("backup");
        std::fs::create_dir_all(&backups).unwrap();
        (dir, backups)
    }

    /// As an exit that writes its session does: the backups, then the removal of the others.
    fn r5_exit(docs: &Docs, backups: &Path) -> Vec<Unsaved> {
        let list = docs.write_backups(backups).unwrap();
        Docs::drop_stale_backups(backups, &list);
        list
    }

    /// A run that edits `file` (its backup kept at exit), then types on and dies once the
    /// recovery copy is written: the session list, the backup's text and the copy's.
    fn r5_backup_and_copy(dir: &Path, backups: &Path, file: &Path) -> Vec<Unsaved> {
        use crate::recovery::Recovery;
        let mut docs = r4_docs();
        let i = docs.open(file);
        docs.load_now(i, None, false);
        docs.activate(i);
        assert!(docs.edit(EditOp::Insert("backup "), None));
        let unsaved = r5_exit(&docs, backups);
        // The copy is written later than the backup (the file times must differ).
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(docs.edit(EditOp::Insert("copy "), None));
        let mut rec = Recovery::new(Some(dir.join("recover")));
        r4_copies(&docs, &mut rec);
        unsaved
    }

    /// Audit 1 round 5, #1: the recovery copy is accepted at the start, then cannot be read
    /// when its tab loads. The tab does not take the older backup in its place; the copy is
    /// kept at exit, and the next start has its text.
    #[test]
    fn a_copy_locked_after_the_start_is_kept_for_the_next() {
        use crate::recovery::{Locked, Recovery};
        let (dir, backups) = r4_dir("r5-locked-copy");
        let file = dir.join("f.txt");
        std::fs::write(&file, "file\n").unwrap();
        let mut unsaved = r5_backup_and_copy(&dir, &backups, &file);
        let mut docs = r4_docs();
        let mut rec = Recovery::new(Some(dir.join("recover")));
        bring_back(&mut docs, &mut rec, &mut unsaved, Some(&backups));
        let k = docs.tabs.iter().position(|d| d.restore.is_some()).unwrap();
        let copy = docs.tabs[k].restore.clone().unwrap();
        assert_eq!(copy.parent(), Some(dir.join("recover").as_path()));
        let Some(lock) = Locked::new(&copy) else {
            let _ = std::fs::remove_dir_all(&dir);
            return;
        };
        docs.load_now(k, None, false);
        let d = &docs.tabs[k];
        assert_eq!(d.ready().unwrap().buffer.rope().to_string(), "file\n");
        assert!(d.modified() && d.restore.is_some() && d.ready().unwrap().read_only);
        drop(lock);
        // A normal exit.
        unsaved = r5_exit(&docs, &backups);
        rec.finish(&docs, true);
        assert!(copy.is_file(), "the copy is kept");
        let mut docs = r4_docs();
        let mut rec = Recovery::new(Some(dir.join("recover")));
        bring_back(&mut docs, &mut rec, &mut unsaved, Some(&backups));
        assert_eq!(r4_texts(&mut docs), ["backup copy file\n"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 1 round 5, #2: a file tab whose backup cannot be read when it loads shows the file
    /// and takes no typing, so nothing typed can be lost behind the backup the exit keeps.
    #[test]
    fn a_tab_waiting_for_its_text_takes_no_typing() {
        use crate::recovery::{Locked, Recovery};
        let (dir, backups) = r4_dir("r5-locked-backup");
        let file = dir.join("f.txt");
        std::fs::write(&file, "file\n").unwrap();
        let mut docs = r4_docs();
        let i = docs.open(&file);
        docs.load_now(i, None, false);
        docs.activate(i);
        assert!(docs.edit(EditOp::Insert("backup "), None));
        let mut unsaved = r5_exit(&docs, &backups);
        let mut docs = r4_docs();
        let mut rec = Recovery::new(Some(dir.join("recover")));
        bring_back(&mut docs, &mut rec, &mut unsaved, Some(&backups));
        let k = docs.tabs.iter().position(|d| d.restore.is_some()).unwrap();
        let backup = docs.tabs[k].restore.clone().unwrap();
        let Some(lock) = Locked::new(&backup) else {
            let _ = std::fs::remove_dir_all(&dir);
            return;
        };
        docs.load_now(k, None, false);
        docs.activate(k);
        assert!(!docs.edit(EditOp::Insert("typed "), None), "no typing");
        let name = docs.tabs[k].name();
        assert_eq!(
            docs.toast(Instant::now()).unwrap(),
            strings::waiting_text_toast(&name)
        );
        drop(lock);
        unsaved = r5_exit(&docs, &backups);
        rec.finish(&docs, true);
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), "backup file\n");
        let mut docs = r4_docs();
        let mut rec = Recovery::new(Some(dir.join("recover")));
        bring_back(&mut docs, &mut rec, &mut unsaved, Some(&backups));
        assert_eq!(r4_texts(&mut docs), ["backup file\n"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 1 round 5, #3: an untitled draft's backup cannot be read at the start. It stays in
    /// the session's list through the pruning and the exit, and on disk, for the next start.
    #[test]
    fn an_untitled_backup_locked_at_the_start_is_kept() {
        use crate::recovery::{Locked, Recovery};
        let (dir, backups) = r4_dir("r5-locked-untitled");
        let mut docs = r4_docs();
        docs.new_untitled();
        assert!(docs.edit(EditOp::Insert("draft"), None));
        let mut unsaved = r5_exit(&docs, &backups);
        assert_eq!(unsaved.len(), 1);
        let backup = backups.join(&unsaved[0].file);
        let lock = Locked::new(&backup);
        if lock.is_none() {
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let mut docs = r4_docs();
        let mut rec = Recovery::new(Some(dir.join("recover")));
        bring_back(&mut docs, &mut rec, &mut unsaved, Some(&backups));
        assert!(docs.tabs.iter().all(|d| !d.modified()));
        // The session's pruning keeps it.
        assert_eq!(docs.backups_needed(&unsaved), unsaved);
        drop(lock);
        // The exit keeps it.
        let kept = r5_exit(&docs, &backups);
        rec.finish(&docs, true);
        assert_eq!(kept, unsaved);
        assert!(backup.is_file());
        let mut docs = r4_docs();
        let mut rec = Recovery::new(Some(dir.join("recover")));
        let mut kept = kept;
        bring_back(&mut docs, &mut rec, &mut kept, Some(&backups));
        assert_eq!(r4_texts(&mut docs), ["draft"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 1 round 5, #4: the session cannot be written at exit. The exit does not go ahead
    /// (the user is asked, as when a backup cannot be written), the backups the old session
    /// names are untouched, and a copy of a tab still unsaved is kept by an exit that follows.
    #[test]
    fn an_exit_whose_session_cannot_be_written_keeps_everything() {
        use crate::recovery::Recovery;
        let (dir, backups) = r4_dir("r5-exit-session");
        // The session file cannot be written: its place is a folder.
        let session = dir.join("session.ini");
        std::fs::create_dir_all(&session).unwrap();
        let mut app = app();
        app.save = true;
        app.session_path = Some(session.clone());
        // The old session names a backup of another draft.
        std::fs::write(backups.join("1.txt"), "older draft").unwrap();
        let old = Unsaved {
            file: "1.txt".into(),
            untitled: 3,
            path: None,
            draft: Some(77),
        };
        app.session.unsaved = vec![old.clone()];
        assert!(app.docs.edit(EditOp::Insert("new draft"), None));
        app.request(After::Exit);
        assert!(!app.exit, "not closed");
        assert_eq!(
            app.ui.dialog.as_ref().map(|d| d.kind),
            Some(DialogKind::Unsaved)
        );
        assert_eq!(app.session.unsaved, [old]);
        assert_eq!(
            std::fs::read_to_string(backups.join("1.txt")).unwrap(),
            "older draft"
        );
        // An exit whose session is not written keeps the copy of a tab with unsaved text.
        let mut rec = Recovery::new(Some(dir.join("recover")));
        r4_copies(&app.docs, &mut rec);
        rec.finish(&app.docs, false);
        assert_eq!(std::fs::read_dir(dir.join("recover")).unwrap().count(), 1);
        rec.finish(&app.docs, true);
        assert!(!dir.join("recover").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 1 round 5: a backup from before 1.1 (no draft id) and a session that cannot be
    /// written. Each start writes a recovery copy and dies; the next has one draft, not one more
    /// each time.
    #[test]
    fn a_legacy_backup_does_not_multiply_when_the_session_is_never_written() {
        use crate::recovery::Recovery;
        let (dir, backups) = r4_dir("r5-legacy");
        std::fs::write(backups.join("1.txt"), "legacy draft").unwrap();
        let session = vec![Unsaved {
            file: "1.txt".into(),
            untitled: 1,
            path: None,
            draft: None,
        }];
        std::thread::sleep(std::time::Duration::from_millis(30));
        for run in 0..4 {
            let mut docs = r4_docs();
            let mut rec = Recovery::new(Some(dir.join("recover")));
            let mut unsaved = session.clone();
            bring_back(&mut docs, &mut rec, &mut unsaved, Some(&backups));
            assert_eq!(r4_texts(&mut docs), ["legacy draft"], "run {run}");
            r4_copies(&docs, &mut rec);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 1 round 5: a session write that failed wakes the window when it is due again, with
    /// nothing else to wake it.
    #[test]
    fn a_failed_session_write_wakes_the_window_to_try_again() {
        let mut app = app();
        let now = Instant::now();
        // Due before anything else would wake the window.
        let at = now + Duration::from_micros(1);
        assert!(app.next_wake(now).is_none_or(|t| t > at));
        app.session_retry = Some(at);
        assert_eq!(app.next_wake(now), Some(at));
    }

    /// Audit 1 round 6: a tab whose recovery copy could not be read is read-only; once the copy
    /// can be read, Reload (Discard asked and taken) puts the copy's text back and the tab is
    /// editable again, so the recovery look keeps its copy and a crash after it loses nothing.
    #[test]
    fn a_reload_that_puts_the_waiting_text_back_keeps_its_copy() {
        use crate::recovery::{Locked, Recovery};
        let (dir, backups) = r4_dir("r6-reload");
        let file = dir.join("f.txt");
        std::fs::write(&file, "file\n").unwrap();
        let mut unsaved = r5_backup_and_copy(&dir, &backups, &file);
        let mut app = app();
        app.docs = r4_docs();
        app.recovery = Recovery::new(Some(dir.join("recover")));
        bring_back(
            &mut app.docs,
            &mut app.recovery,
            &mut unsaved,
            Some(&backups),
        );
        let k = app
            .docs
            .tabs
            .iter()
            .position(|d| d.restore.is_some())
            .unwrap();
        let copy = app.docs.tabs[k].restore.clone().unwrap();
        let Some(lock) = Locked::new(&copy) else {
            let _ = std::fs::remove_dir_all(&dir);
            return;
        };
        app.docs.load_now(k, None, false);
        assert!(app.docs.tabs[k].ready().unwrap().read_only);
        drop(lock);
        // Reload, through its question.
        app.docs.activate(k);
        let id = app.docs.tabs[k].id;
        app.request(After::Reload(id));
        assert_eq!(
            app.ui.dialog.as_ref().map(|d| d.kind),
            Some(DialogKind::Discard)
        );
        app.answer(0);
        let until = Instant::now() + Duration::from_secs(10);
        while app.docs.tabs[k].is_loading() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
            app.docs.drain_loads(None, false);
        }
        let d = &app.docs.tabs[k];
        let r = d.ready().unwrap();
        assert_eq!(r.buffer.rope().to_string(), "backup copy file\n");
        assert!(!r.read_only && d.restore.is_none() && d.modified());
        // The recovery look keeps a copy of it; then the window dies.
        r4_copies(&app.docs, &mut app.recovery);
        drop(app);
        let mut docs = r4_docs();
        let mut rec = Recovery::new(Some(dir.join("recover")));
        bring_back(&mut docs, &mut rec, &mut unsaved, Some(&backups));
        assert_eq!(r4_texts(&mut docs), ["backup copy file\n"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 1 round 6: an exit cancelled after the session could not be written leaves the
    /// state as last written, with a retry due. When it is due, the write is made all the same:
    /// failing, it is due again later (never left in the past to wake the loop at once); written,
    /// it is done.
    #[test]
    fn a_due_retry_is_made_when_nothing_changed_since() {
        let (dir, _) = r4_dir("r6-retry");
        let session = dir.join("session.ini");
        let mut app = app();
        app.save = true;
        app.session_path = Some(session.clone());
        assert!(app.save_session());
        // The session cannot be written now; an exit tries, and is cancelled.
        std::fs::remove_file(&session).unwrap();
        std::fs::create_dir_all(&session).unwrap();
        assert!(app.docs.edit(EditOp::Insert("x"), None));
        app.request(After::Exit);
        assert!(!app.exit && app.session_retry.is_some());
        app.answer(2);
        assert!(!app.exit);
        assert_eq!(
            app.session_sig(),
            app.saved_sig,
            "the state as last written"
        );
        // Due: tried again, and due again later.
        app.session_retry = Some(Instant::now() - Duration::from_millis(1));
        app.persist_if_changed();
        assert!(app.session_retry.is_some_and(|t| t > Instant::now()));
        // Writable again: done.
        std::fs::remove_dir_all(&session).unwrap();
        app.session_retry = Some(Instant::now() - Duration::from_millis(1));
        app.persist_if_changed();
        assert_eq!(app.session_retry, None);
        assert!(session.is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 1 round 4, #1 and #9: the session cannot be written at any start, so every start
    /// reads the entries the first exit wrote, while each start writes recovery copies of its
    /// own before it dies. Both drafts come back once each, every time.
    #[test]
    fn drafts_survive_starts_whose_session_is_never_written() {
        use crate::recovery::Recovery;
        let (dir, backups) = r4_dir("no-session");
        let file = dir.join("gone.txt");
        std::fs::write(&file, "file text\n").unwrap();
        let mut docs = r4_docs();
        let i = docs.open(&file);
        docs.load_now(i, None, false);
        docs.activate(i);
        assert!(docs.edit(EditOp::Insert("edited "), None));
        let u = docs.new_untitled();
        docs.activate(u);
        assert!(docs.edit(EditOp::Insert("draft"), None));
        let first = docs.write_backups(&backups).unwrap();
        assert_eq!(first.len(), 2);
        // The second run types on in the untitled draft, writes its copy, and dies; the file
        // goes, so the next start gives the file's draft another name.
        let mut rec = Recovery::new(Some(dir.join("recover")));
        assert!(docs.edit(EditOp::Insert(" newer"), None));
        r4_copies(&docs, &mut rec);
        drop(rec);
        std::fs::remove_file(&file).unwrap();
        let want = vec!["draft newer".to_string(), "edited file text\n".to_string()];
        for run in 0..4 {
            let mut unsaved = first.clone();
            let mut docs = r4_docs();
            let mut rec = Recovery::new(Some(dir.join("recover")));
            bring_back(&mut docs, &mut rec, &mut unsaved, Some(&backups));
            assert_eq!(r4_texts(&mut docs), want, "run {run}");
            r4_copies(&docs, &mut rec);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 1 round 4, #2: a recovery copy whose header reads but whose text does not (here
    /// not UTF-8) gives way to the backup of the same draft, for a file and for a new document.
    #[test]
    fn a_copy_whose_text_cannot_be_read_gives_way_to_its_backup() {
        use crate::recovery::Recovery;
        let (dir, backups) = r4_dir("bad-copy");
        let file = dir.join("f.txt");
        std::fs::write(&file, "file text\n").unwrap();
        let mut docs = r4_docs();
        let i = docs.open(&file);
        docs.load_now(i, None, false);
        docs.activate(i);
        assert!(docs.edit(EditOp::Insert("backup "), None));
        let u = docs.new_untitled();
        docs.activate(u);
        assert!(docs.edit(EditOp::Insert("draft"), None));
        let mut unsaved = docs.write_backups(&backups).unwrap();
        // A later run types more in both and dies; its copies are then damaged.
        let mut rec = Recovery::new(Some(dir.join("recover")));
        assert!(docs.edit(EditOp::Insert(" newer"), None));
        docs.activate(i);
        assert!(docs.edit(EditOp::Insert("newer "), None));
        r4_copies(&docs, &mut rec);
        drop(rec);
        let mut damaged = 0;
        for e in std::fs::read_dir(dir.join("recover")).unwrap().flatten() {
            let mut data = std::fs::read(e.path()).unwrap();
            *data.last_mut().unwrap() = 0xff;
            std::fs::write(e.path(), data).unwrap();
            damaged += 1;
        }
        assert_eq!(damaged, 2);
        let mut docs = r4_docs();
        let mut rec = Recovery::new(Some(dir.join("recover")));
        bring_back(&mut docs, &mut rec, &mut unsaved, Some(&backups));
        assert_eq!(
            r4_texts(&mut docs),
            ["backup file text\n".to_string(), "draft".to_string()]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 1 round 4, #5: sessions from before 1.1 with two entries both numbered 1 (round 3
    /// could leave them) and one copy numbered 1. Which entry the copy stands for is not
    /// certain, so all three texts come back.
    #[test]
    fn two_old_entries_with_one_number_and_a_copy_all_come_back() {
        use crate::recovery::Recovery;
        let (dir, backups) = r4_dir("same-number");
        std::fs::write(backups.join("1.txt"), "first").unwrap();
        std::fs::write(backups.join("2.txt"), "second").unwrap();
        let entry = |file: &str| Unsaved {
            file: file.into(),
            untitled: 1,
            path: None,
            draft: None,
        };
        let mut unsaved = vec![entry("1.txt"), entry("2.txt")];
        let h = mdedit_core::recovery::Header {
            path: None,
            untitled: 1,
            encoding: "UTF-8".into(),
            eol: "Unix (LF)".into(),
            disk: None,
            draft: None,
            bytes: 0,
        };
        let rope = ropey::Rope::from_str("copy");
        mdedit_core::recovery::write(&dir.join("recover"), "7-1.rec", &h, &rope).unwrap();
        let mut docs = r4_docs();
        let mut rec = Recovery::new(Some(dir.join("recover")));
        bring_back(&mut docs, &mut rec, &mut unsaved, Some(&backups));
        assert_eq!(r4_texts(&mut docs), ["copy", "first", "second"]);
        // Each entry now names a draft of its own.
        let tabs: Vec<TabState> = docs.tabs.iter().map(|d| d.tab_state()).collect();
        assert_eq!(Unsaved::prune(&unsaved, &tabs).len(), 2);
        assert_ne!(unsaved[0].draft, unsaved[1].draft);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 1 round 4, #4: a copy from before 1.1 that is older than the backup of the same
    /// file (a later exit wrote the backup) does not hide the backup's newer text.
    #[test]
    fn an_old_copy_older_than_the_backup_hides_nothing() {
        use crate::recovery::Recovery;
        let (dir, backups) = r4_dir("older-copy");
        let file = dir.join("f.txt");
        std::fs::write(&file, "file\n").unwrap();
        let h = mdedit_core::recovery::Header {
            path: Some(file.clone()),
            untitled: 0,
            encoding: "UTF-8".into(),
            eol: "Unix (LF)".into(),
            disk: None,
            draft: None,
            bytes: 0,
        };
        let rope = ropey::Rope::from_str("older");
        mdedit_core::recovery::write(&dir.join("recover"), "7-1.rec", &h, &rope).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        std::fs::write(backups.join("1.txt"), "newer").unwrap();
        let mut unsaved = vec![Unsaved {
            file: "1.txt".into(),
            untitled: 0,
            path: Some(file.clone()),
            draft: None,
        }];
        let mut docs = r4_docs();
        let mut rec = Recovery::new(Some(dir.join("recover")));
        bring_back(&mut docs, &mut rec, &mut unsaved, Some(&backups));
        assert_eq!(r4_texts(&mut docs), ["newer", "older"]);
        let _ = std::fs::remove_dir_all(&dir);
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

    /// `--frame` puts the shadow margin around the composed content and cuts its corners: the
    /// corner pixel is the backdrop with a shadow, the middle is the content, and the picture is
    /// larger by the margin on each side. Square corners keep the content's corner.
    #[test]
    fn a_frame_shot_cuts_the_corners() {
        let mut content = Bitmap::new(400, 240);
        content.clear([0x20, 0x40, 0x60]);
        for px in content.pixels.as_chunks_mut::<4>().0 {
            px[3] = 255;
        }
        for scale in [1.0, 2.0] {
            let content = if scale == 1.0 {
                content.clone()
            } else {
                let mut c = Bitmap::new(800, 480);
                c.clear([0x20, 0x40, 0x60]);
                c
            };
            let m = (mdshared_ui::shadow::EXTENT * scale).ceil() as u32;
            for dark in [false, true] {
                let round = frame_of(&content, Corners::Round, scale, dark);
                let square = frame_of(&content, Corners::Square, scale, dark);
                assert_eq!(
                    (round.width, round.height),
                    (content.width + 2 * m, content.height + 2 * m)
                );
                let at = |b: &Bitmap, x: u32, y: u32| {
                    let o = ((y * b.width + x) * 4) as usize;
                    [b.pixels[o + 2], b.pixels[o + 1], b.pixels[o]]
                };
                let own = [0x20, 0x40, 0x60];
                // The middle is the content either way; the content's corner pixel is its colour
                // when square and not when round.
                assert_eq!(at(&round, round.width / 2, round.height / 2), own);
                assert_eq!(at(&square, m, m), own);
                assert_ne!(at(&round, m, m), own, "dark {dark} scale {scale}");
                // The straight edge is not touched by the rounding.
                assert_eq!(at(&round, m + 60, m), own);
                assert_eq!(at(&round, m, m + 60), own);
            }
        }
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

    /// In a restored window the vertical scrollbar at the right edge can be grabbed: the resize
    /// band over its track is only the outermost pixels, which the thumb does not cover, and the
    /// rest of the right edge, the corners and the other edges still resize.
    #[test]
    fn the_scrollbar_at_the_right_edge_is_not_covered_by_the_resize_band() {
        use super::files_tests::{app, open_now, scratch};
        use winit::window::ResizeDirection as D;
        let dir = scratch("band");
        let long: String = (0..400)
            .map(|i| {
                format!(
                    "line {i}
"
                )
            })
            .collect();
        std::fs::write(dir.join("long.txt"), long).unwrap();
        let mut a = app();
        open_now(&mut a, &dir.join("long.txt"));
        a.compose();
        let (w, h) = a.logical();
        let (_, track) = {
            let r = a.areas.text;
            (
                r,
                a.hit_rect(Hit::ScrollBar(Scroller::Text))
                    .expect("a scrollbar"),
            )
        };
        assert!(
            (track.right() - w).abs() < 0.5,
            "the bar is at the window's edge"
        );
        let y = track.cy();
        // Where the thumb is drawn (3 to 11 px from the edge) the press goes to the bar.
        for dx in [4.0, 6.0, 7.5, 11.0] {
            assert_eq!(a.resize_band(w - dx, y), None, "{dx} px from the edge");
            assert_eq!(a.hit_at(w - dx, y), Some(Hit::ScrollBar(Scroller::Text)));
        }
        // The outermost pixels still resize, and so do the corners and the other edges.
        assert_eq!(a.resize_band(w - 1.0, y), Some(D::East));
        assert_eq!(a.resize_band(w - 6.0, h - 4.0), Some(D::SouthEast));
        assert_eq!(a.resize_band(w - 6.0, 2.0), Some(D::NorthEast));
        assert_eq!(a.resize_band(2.0, y), Some(D::West));
        assert_eq!(a.resize_band(w / 2.0, h - 2.0), Some(D::South));
        // With no scrollbar beside the point the old band stands.
        let above = a.areas.title.cy();
        assert_eq!(a.resize_band(w - 6.0, above.max(14.0)), Some(D::East));
        let _ = std::fs::remove_dir_all(&dir);
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

    /// Audit 2: Close All, "Don't save" on the first tab while the second's question is open,
    /// then Cancel. In between, the loop turns: the first tab's recovery copy stays, and so does
    /// the backup the session lists for it.
    #[test]
    fn dont_save_then_cancel_keeps_the_first_tabs_copy_and_backup_meanwhile() {
        use crate::recovery::Recovery;
        use std::time::{Duration, Instant};
        let (dir, backups) = r4_dir("a2-discard");
        let session = dir.join("session.ini");
        let mut app = app();
        app.save = true;
        app.session_path = Some(session.clone());
        app.recovery = Recovery::new(Some(dir.join("recover")));
        assert!(app.docs.edit(EditOp::Insert("first"), None));
        let first = app.docs.tabs[0].id;
        let k = app.docs.new_untitled();
        app.docs.activate(k);
        assert!(app.docs.edit(EditOp::Insert("second"), None));
        r4_copies(&app.docs, &mut app.recovery);
        let count = || std::fs::read_dir(dir.join("recover")).map_or(0, |d| d.count());
        assert_eq!(count(), 2);
        app.request(After::CloseAll);
        assert_eq!(
            app.ui.dialog.as_ref().map(|d| d.kind),
            Some(DialogKind::Unsaved)
        );
        app.answer(1);
        assert_eq!(
            app.ui.dialog.as_ref().map(|d| d.kind),
            Some(DialogKind::Unsaved)
        );
        let i = app.docs.index_of_id(first).unwrap();
        let state = app.docs.tabs[i].tab_state();
        assert!(
            state.modified,
            "the session's pruning still sees the first tab's text"
        );
        let t0 = Instant::now();
        app.recovery.poll(&app.docs, t0);
        app.recovery.poll(&app.docs, t0 + Duration::from_secs(60));
        app.recovery.flush();
        assert_eq!(
            count(),
            2,
            "the first tab's copy is kept while the question is open"
        );
        app.answer(2);
        assert_eq!(app.docs.tabs.len(), 2);
        let i = app.docs.index_of_id(first).unwrap();
        assert!(app.docs.tabs[i].modified());
        let _ = backups;
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2: `--exit-after-ms` (and the other timed runs) end as the user's exit does: the
    /// unsaved text is kept as a backup the session lists.
    #[test]
    fn a_timed_exit_keeps_the_unsaved_text() {
        let (dir, backups) = r4_dir("a2-timed");
        let session = dir.join("session.ini");
        let mut app = app();
        app.save = true;
        app.session_path = Some(session.clone());
        assert!(app.docs.edit(EditOp::Insert("typed in a timed run"), None));
        app.exit_due = Some(Instant::now());
        app.timers(Instant::now() + Duration::from_millis(1));
        assert!(app.exit);
        let listed = Session::read(&session).unwrap().unsaved;
        assert_eq!(listed.len(), 1);
        assert_eq!(
            std::fs::read_to_string(backups.join(&listed[0].file)).unwrap(),
            "typed in a timed run"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2: the exit path itself keeps the recovery copy of unsaved text when the session
    /// cannot be written (the round-5 fix was tested on `Recovery::finish` alone).
    #[test]
    fn an_exit_whose_session_write_fails_keeps_the_recovery_copy() {
        use crate::recovery::Recovery;
        let (dir, _backups) = r4_dir("a2-exit-copy");
        // The session's folder is a file: nothing can be written there.
        let blocker = dir.join("blocked");
        std::fs::write(&blocker, "a file").unwrap();
        let mut app = app();
        app.save = true;
        app.session_path = Some(blocker.join("session.ini"));
        app.recovery = Recovery::new(Some(dir.join("recover")));
        assert!(app.docs.edit(EditOp::Insert("only here"), None));
        r4_copies(&app.docs, &mut app.recovery);
        let count = || std::fs::read_dir(dir.join("recover")).map_or(0, |d| d.count());
        assert_eq!(count(), 1);
        app.finish_session();
        assert_eq!(count(), 1, "the copy is the only place the text is");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The owner's setting: tabs looked at for less than n seconds (focused time, added up
    /// across runs) are left out at start; the tab that was showing, pinned ones, ones with
    /// unsaved text and ones with no count from before always come back.
    #[test]
    fn tabs_barely_looked_at_are_left_out_at_start_when_asked() {
        use crate::session::Pin;
        let p = |n: &str| PathBuf::from(format!("C:/t/{n}.txt"));
        let mut s = Session {
            tabs: ["a", "b", "c", "d", "e", "f"].map(p).to_vec(),
            active: 0,
            pinned: vec![Pin::File(p("e"))],
            focus: vec![
                (p("a"), 3),
                (p("b"), 3),
                (p("c"), 50),
                (p("e"), 2),
                (p("f"), 1),
            ],
            unsaved: vec![Unsaved {
                file: "1.txt".into(),
                untitled: 0,
                path: Some(p("f")),
                draft: None,
            }],
            ..Default::default()
        };
        s.prefs.brief_secs = 10;
        let out = |s: &Session| -> Vec<usize> { (0..6).filter(|&i| barely_seen(s, i)).collect() };
        // Off (the default): every tab comes back.
        assert!(out(&s).is_empty());
        s.prefs.skip_brief = true;
        // a showing, c long enough, d no count, e pinned, f unsaved: only b is left out.
        assert_eq!(out(&s), [1]);
        // The count goes through the session file.
        let back = Session::parse(&s.serialize());
        assert_eq!(back.focus, s.focus);
        assert_eq!(out(&back), [1]);
        // With the second switch the tab that was showing counts like the rest: a goes too,
        // while the pinned tab and the one with unsaved text still come back.
        s.prefs.brief_last = true;
        assert_eq!(out(&s), [0, 1]);
        let back = Session::parse(&s.serialize());
        assert!(back.prefs.brief_last, "the switch goes through the session");
        assert_eq!(out(&back), [0, 1]);
        // It does nothing on its own: the rule has to be on.
        s.prefs.skip_brief = false;
        assert!(out(&s).is_empty());
    }

    /// The face the left pane showed is kept in the session and comes back with the pane; the
    /// command line's own ask for the pane (`--files`, a folder to open) wins over it.
    #[test]
    fn the_left_pane_comes_back_at_start_with_the_face_it_had() {
        use crate::ui::PaneFace;
        let (dir, _backups) = r4_dir("pane-face");
        let path = dir.join("session.ini");
        let mut app = app();
        app.save = true;
        app.session_path = Some(path.clone());
        app.ui.set_pane_face(PaneFace::Notes);
        assert!(app.save_session());
        let back = Session::read(&path).unwrap();
        assert_eq!((back.files, back.pane), (Some(true), Some(PaneFace::Notes)));
        // The function list is kept too (it used to leave the pane shut).
        app.ui.set_pane_face(PaneFace::Functions);
        assert!(app.save_session());
        let back = Session::read(&path).unwrap();
        assert_eq!(
            (back.files, back.pane),
            (Some(true), Some(PaneFace::Functions))
        );

        let restored = |session: &Session, args: &Args| {
            let mut ui = Ui::new();
            restore_pane(&mut ui, session, args);
            (ui.panes.files, ui.pane_face())
        };
        let none = Args::default();
        assert_eq!(restored(&back, &none), (true, PaneFace::Functions));
        let notes = Session {
            pane: Some(PaneFace::Notes),
            ..back.clone()
        };
        assert_eq!(restored(&notes, &none), (true, PaneFace::Notes));
        // A pane that was shut stays shut, whatever face it had.
        let shut = Session {
            files: Some(false),
            ..notes.clone()
        };
        assert_eq!(restored(&shut, &none), (false, PaneFace::Files));
        // A session from before the face was kept: the tree.
        let old = Session {
            pane: None,
            ..notes.clone()
        };
        assert_eq!(restored(&old, &none), (true, PaneFace::Files));
        // `--files` asks for the tree.
        let files = Args {
            files_pane: Some(true),
            ..Args::default()
        };
        assert_eq!(restored(&notes, &files), (true, PaneFace::Files));
        // So does a folder given to open.
        let folder = Args {
            files: vec![dir.clone()],
            ..Args::default()
        };
        assert_eq!(restored(&notes, &folder), (true, PaneFace::Files));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The library is not read at start: a pane that comes back showing it reads it on the
    /// first tick of the loop, which is also when its rows are made.
    #[test]
    fn a_library_pane_that_came_back_is_read_on_its_first_tick_not_at_start() {
        use crate::ui::PaneFace;
        let (dir, _backups) = r4_dir("pane-lazy");
        let lib = dir.join("library");
        std::fs::create_dir_all(&lib).unwrap();
        let note = dir.join("n.md");
        std::fs::write(&note, "# N\n").unwrap();
        std::fs::write(
            lib.join("notes.md"),
            format!(
                "# Notes\n\n- {}\n",
                note.to_string_lossy().replace('\\', "/")
            ),
        )
        .unwrap();
        let mut app = app();
        app.session_path = Some(dir.join("settings.ini"));
        let session = Session {
            files: Some(true),
            pane: Some(PaneFace::Notes),
            ..Session::default()
        };
        restore_pane(&mut app.ui, &session, &Args::default());
        assert!(app.ui.library.on);
        assert!(app.docs.library.groups.is_empty(), "nothing read yet");
        assert!(app.ui.library.rows.is_empty());
        app.library_tick(Instant::now());
        assert_eq!(app.ui.library.rows.len(), 2, "the group and its note");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The session written carries each file tab's focused seconds, read back at the next start.
    #[test]
    fn the_session_keeps_each_file_tab_s_focused_time() {
        let (dir, _backups) = r4_dir("focus-save");
        let file = dir.join("f.txt");
        std::fs::write(&file, "text").unwrap();
        let mut app = app();
        app.save = true;
        app.session_path = Some(dir.join("session.ini"));
        let k = app.docs.add_pending(&file);
        app.docs.tabs[k].focus_ms = Some(12_345);
        assert!(app.save_session());
        let back = Session::read(&dir.join("session.ini")).unwrap();
        assert_eq!(back.focus.len(), 1);
        // The tab holds the file's canonical path (/private/var on macOS, a long name on Windows).
        let held = app.docs.tabs[k].path.clone().unwrap();
        assert!(mdshared_app::paths::same_path(&back.focus[0].0, &held));
        assert_eq!(back.focus[0].1, 12);
        let mut docs = r4_docs();
        docs.add_pending(&file);
        docs.start_focus(&back);
        assert_eq!(docs.tabs[0].focus_ms, Some(12_000));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Focused time is counted for the tab in front while the window has the focus, and only
    /// then; a tab from before the count has none until it is shown.
    #[test]
    fn focused_time_counts_only_the_tab_in_front_of_a_focused_window() {
        use std::time::{Duration, Instant};
        let mut docs = r4_docs();
        let a = docs.new_untitled();
        let b = docs.new_untitled();
        let (ida, idb) = (docs.tabs[a].id, docs.tabs[b].id);
        docs.tabs[b].focus_ms = None;
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        docs.focus_tick(Some(ida), at(0));
        docs.focus_tick(Some(ida), at(2));
        // The window loses the focus (another program, minimized): a's clock stops.
        docs.focus_tick(None, at(3));
        docs.focus_tick(None, at(60));
        assert_eq!(docs.tabs[a].focus_ms, Some(3000));
        assert_eq!(docs.tabs[b].focus_ms, None);
        // Back, on b: a session write in between counts b so far.
        docs.focus_tick(Some(idb), at(60));
        docs.focus_flush(at(65));
        assert_eq!(docs.tabs[b].focus_ms, Some(5000));
        docs.focus_tick(Some(ida), at(70));
        assert_eq!(docs.tabs[b].focus_ms, Some(10_000));
        assert_eq!(docs.tabs[a].focus_ms, Some(3000));
    }

    /// Audit 2: an exit that wrote the backups and the session, then stopped before it removed
    /// the recovery copies (a crash, power lost): the next start opens the text once.
    #[test]
    fn a_crash_after_the_backups_were_written_brings_the_text_back_once() {
        use crate::recovery::Recovery;
        let (dir, backups) = r4_dir("a2-dup");
        let mut app = app();
        app.save = true;
        app.session_path = Some(dir.join("session.ini"));
        app.recovery = Recovery::new(Some(dir.join("recover")));
        assert!(app.docs.edit(EditOp::Insert("only here"), None));
        r4_copies(&app.docs, &mut app.recovery);
        // The backup is written after the copy, so it is the newer file.
        std::thread::sleep(std::time::Duration::from_millis(50));
        app.request(After::Exit);
        assert!(app.exit);
        assert_eq!(std::fs::read_dir(dir.join("recover")).unwrap().count(), 1);
        drop(app);
        // The next start.
        let mut session = Session::read(&dir.join("session.ini")).unwrap();
        assert_eq!(session.unsaved.len(), 1);
        let mut docs = r4_docs();
        let mut rec = Recovery::new(Some(dir.join("recover")));
        bring_back(&mut docs, &mut rec, &mut session.unsaved, Some(&backups));
        assert_eq!(r4_texts(&mut docs), ["only here"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 2: while a tab waits for its copy (the copy could not be read yet), the recovery
    /// look leaves the copy alone: it is the only place that text is.
    #[test]
    fn the_recovery_look_leaves_a_waiting_tabs_copy_alone() {
        use crate::recovery::{Locked, Recovery};
        let (dir, backups) = r4_dir("a2-waiting");
        let file = dir.join("f.txt");
        std::fs::write(
            &file, "file
",
        )
        .unwrap();
        let mut unsaved = r5_backup_and_copy(&dir, &backups, &file);
        let mut app = app();
        app.docs = r4_docs();
        app.recovery = Recovery::new(Some(dir.join("recover")));
        bring_back(
            &mut app.docs,
            &mut app.recovery,
            &mut unsaved,
            Some(&backups),
        );
        let k = app
            .docs
            .tabs
            .iter()
            .position(|d| d.restore.is_some())
            .unwrap();
        let copy = app.docs.tabs[k].restore.clone().unwrap();
        let Some(lock) = Locked::new(&copy) else {
            let _ = std::fs::remove_dir_all(&dir);
            return;
        };
        app.docs.load_now(k, None, false);
        assert!(app.docs.tabs[k].restore.is_some());
        r4_copies(&app.docs, &mut app.recovery);
        drop(lock);
        assert!(copy.is_file(), "the waiting tab's copy is kept");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
