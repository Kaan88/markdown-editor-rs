//! Replace in files, as the results panel shows it. The core is `mdedit_core::replace_files`; this
//! module holds the shell's part of it:
//!
//! - the replacement box and the buttons in the panel's title row (`draw_head`);
//! - the three steps, each on a worker thread (`Job`) with a progress count the panel shows:
//!   a plan, the confirmation (`Confirm`, drawn as a dialog by `draw_confirm`), the apply, and
//!   after it a report (`Outcome`) that replaces the panel's matches and keeps what Undo replace
//!   needs, until the next search, the folder changes, or the panel closes;
//! - the strings and the rows of the report.
//!
//! Nothing here exists until a folder search has run: the state is empty at launch. What the
//! window does with a finished step (open the dialog, reload the tabs) is in `app/replace_files.rs`.

use crate::app::Wake;
use crate::chrome::{Ctx, draw_field, glyph};
use crate::docs::Waker;
use crate::ui::{BarField, DialogKind, Hit, Scroller, Ui};
use mdedit_core::find::Query;
use mdedit_core::load::LoadOptions;
use mdedit_core::replace_files::{
    self, ApplyOptions, Plan, PlanOptions, Report, RevertReport, Source,
};
use mdshared_ui::gfx::{Face, Rect, mix};
use mdshared_ui::overlay::{
    DIALOG_FOOTER, DIALOG_PAD, dialog_button_w, dialog_card, draw_backdrop, draw_dialog_button,
    draw_dialog_card,
};
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};
use mdshared_ui::textfield::TextField;
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The progress wakes the window at most this often.
const WAKE_EVERY: Duration = Duration::from_millis(40);
/// A row of the confirmation's list and of the report.
pub const CONFIRM_ROW: f32 = 30.0;
pub const OUTCOME_ROW: f32 = 25.0;
const OUTCOME_ROW_H: f32 = 23.0;
const PREVIEW_H: f32 = 96.0;
const OPTIONS_H: f32 = 30.0;
/// The most of the title row the controls take, and the least the title keeps.
const BOX_MAX: f32 = 220.0;
const BOX_MIN: f32 = 110.0;
const TITLE_MIN: f32 = 220.0;
const CTRL_H: f32 = 26.0;

/// What the panel searched: enough to plan the same replacement.
#[derive(Clone)]
pub struct Searched {
    pub root: PathBuf,
    pub query: Query,
    pub load: LoadOptions,
}

/// A step running on a worker thread. The window reads its progress and takes its result; the
/// worker wakes the window as it goes.
pub struct Job<T> {
    progress: Arc<(AtomicUsize, AtomicUsize)>,
    cancel: Arc<AtomicBool>,
    done: Arc<Mutex<Option<T>>>,
}

impl<T: Send + 'static> Job<T> {
    fn start(
        wake: Waker,
        work: impl FnOnce(&AtomicBool, &dyn Fn(usize, usize)) -> T + Send + 'static,
    ) -> Job<T> {
        let job = Job {
            progress: Arc::new((AtomicUsize::new(0), AtomicUsize::new(0))),
            cancel: Arc::new(AtomicBool::new(false)),
            done: Arc::new(Mutex::new(None)),
        };
        let (progress, cancel, done) = (job.progress.clone(), job.cancel.clone(), job.done.clone());
        std::thread::spawn(move || {
            let woke = Cell::new(Instant::now());
            let wake_now = wake.clone();
            let report = |at: usize, of: usize| {
                progress.0.store(at, Ordering::Relaxed);
                progress.1.store(of, Ordering::Relaxed);
                if woke.get().elapsed() >= WAKE_EVERY {
                    woke.set(Instant::now());
                    wake_now(Wake::Results);
                }
            };
            let out = work(&cancel, &report);
            *done.lock().unwrap() = Some(out);
            wake(Wake::Results);
        });
        job
    }

    pub fn progress(&self) -> (usize, usize) {
        (
            self.progress.0.load(Ordering::Relaxed),
            self.progress.1.load(Ordering::Relaxed),
        )
    }

    fn take(&self) -> Option<T> {
        self.done.lock().unwrap().take()
    }

    fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

pub enum Phase {
    Idle,
    Planning(Job<Plan>),
    Applying(Job<Report>),
    Reverting(Job<RevertReport>),
}

/// A step that finished, for the window to act on.
pub enum Finished {
    Plan(Plan),
    Applied(Report),
    Reverted(RevertReport),
}

/// One row of what the panel shows after a replace or an undo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutRow {
    Heading(String),
    Changed { name: String, count: usize },
    Skipped { name: String, why: String },
    Failed { name: String, why: String },
}

/// What the panel shows in place of the matches after a replace or an undo.
pub struct Outcome {
    pub title: String,
    pub rows: Vec<OutRow>,
    /// Kept for Undo replace; `None` once it was undone (or for the report of an undo).
    pub report: Option<Report>,
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(path)
        .display()
        .to_string()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// " · 2 skipped · 1 failed · stopped" for the title of a report.
fn tail(skipped: usize, failed: usize, stopped: bool) -> String {
    let mut s = String::new();
    if skipped > 0 {
        s.push_str(&format!(" \u{00B7} {skipped} skipped"));
    }
    if failed > 0 {
        s.push_str(&format!(" \u{00B7} {failed} failed"));
    }
    if stopped {
        s.push_str(" \u{00B7} stopped");
    }
    s
}

fn problems(
    rows: &mut Vec<OutRow>,
    root: &Path,
    skipped: &[(PathBuf, String)],
    failed: &[(PathBuf, String)],
) {
    if !skipped.is_empty() {
        rows.push(OutRow::Heading(format!("Skipped ({})", skipped.len())));
        rows.extend(skipped.iter().map(|(p, why)| OutRow::Skipped {
            name: rel(root, p),
            why: why.clone(),
        }));
    }
    if !failed.is_empty() {
        rows.push(OutRow::Heading(format!("Failed ({})", failed.len())));
        rows.extend(failed.iter().map(|(p, why)| OutRow::Failed {
            name: rel(root, p),
            why: why.clone(),
        }));
    }
}

impl Outcome {
    /// The report of an apply. Failures are listed, never left out.
    pub fn applied(report: Report, root: &Path) -> Outcome {
        let title = format!(
            "{}{}",
            if report.changed.is_empty() {
                "Nothing was replaced".to_string()
            } else {
                format!(
                    "Replaced {} in {}",
                    plural(report.replacements, "occurrence", "occurrences"),
                    plural(report.changed.len(), "file", "files")
                )
            },
            tail(
                report.skipped.len(),
                report.failures.len(),
                report.cancelled
            )
        );
        let mut rows = Vec::new();
        if !report.changed.is_empty() {
            rows.push(OutRow::Heading(format!(
                "Changed ({})",
                report.changed.len()
            )));
        }
        rows.extend(report.changed.iter().map(|c| OutRow::Changed {
            name: rel(root, &c.path),
            count: c.replacements,
        }));
        problems(&mut rows, root, &report.skipped, &report.failures);
        Outcome {
            title,
            rows,
            report: Some(report),
        }
    }

    /// The report of an undo.
    pub fn reverted(done: &RevertReport, root: &Path) -> Outcome {
        let title = format!(
            "{}{}",
            if done.restored.is_empty() {
                "Nothing was put back".to_string()
            } else {
                format!("Put back {}", plural(done.restored.len(), "file", "files"))
            },
            tail(done.skipped.len(), done.failures.len(), done.cancelled)
        );
        let mut rows = Vec::new();
        if !done.restored.is_empty() {
            rows.push(OutRow::Heading(format!(
                "Restored ({})",
                done.restored.len()
            )));
        }
        rows.extend(done.restored.iter().map(|p| OutRow::Changed {
            name: rel(root, p),
            count: 0,
        }));
        problems(&mut rows, root, &done.skipped, &done.failures);
        Outcome {
            title,
            rows,
            report: None,
        }
    }

    /// How many files Undo replace can still put back.
    pub fn revertible(&self) -> usize {
        self.report.as_ref().map_or(0, Report::revertible)
    }

    /// What the Undo button's tooltip says: how many files can still be undone and how.
    pub fn undo_help(&self) -> String {
        let Some(report) = &self.report else {
            return String::new();
        };
        let changed = report.changed.len();
        let ok = report.revertible();
        let from_backup = report
            .reverts
            .iter()
            .filter(|r| r.original.is_none() && r.backup.is_some())
            .count();
        let mut s = if ok == changed {
            format!(
                "Put {} back as {} before",
                plural(ok, "file", "files"),
                if ok == 1 { "it was" } else { "they were" }
            )
        } else {
            format!(
                "Put back {ok} of the {changed} changed files; the others were too large to keep and have no backup copy"
            )
        };
        if from_backup > 0 {
            s.push_str(&format!(
                ". {} can only come from {}",
                plural(from_backup, "file", "files"),
                if from_backup == 1 {
                    "its backup copy"
                } else {
                    "their backup copies"
                }
            ));
        }
        s.push_str(". A file edited since is left as it is.");
        s
    }
}

/// The confirmation: what the plan found, with a tick for each file.
pub struct Confirm {
    pub plan: Plan,
    pub names: Vec<String>,
    pub ticks: Vec<bool>,
    pub backup: bool,
    pub show_skipped: bool,
    /// The row the keyboard is on.
    pub cursor: usize,
}

impl Confirm {
    pub fn new(plan: Plan, root: &Path) -> Confirm {
        let names = plan.files.iter().map(|f| rel(root, &f.path)).collect();
        let ticks = vec![true; plan.files.len()];
        Confirm {
            plan,
            names,
            ticks,
            backup: false,
            show_skipped: false,
            cursor: 0,
        }
    }

    pub fn toggle(&mut self, i: usize) {
        if let Some(t) = self.ticks.get_mut(i) {
            *t = !*t;
        }
    }

    /// The indexes into the plan's files that are ticked.
    pub fn chosen(&self) -> Vec<usize> {
        (0..self.ticks.len()).filter(|&i| self.ticks[i]).collect()
    }

    /// Occurrences and files in the ticked entries; the count is a lower bound when a file
    /// has more matches than were counted.
    pub fn totals(&self) -> (usize, usize, bool) {
        let mut sum = (0, 0, false);
        for i in self.chosen() {
            let f = &self.plan.files[i];
            sum.0 += f.matches;
            sum.1 += 1;
            sum.2 |= f.capped;
        }
        sum
    }

    pub fn title(&self) -> String {
        let (n, m, more) = self.totals();
        let plus = if more { "+" } else { "" };
        let found = if n == 1 && !more {
            "1 occurrence".to_string()
        } else {
            format!("{n}{plus} occurrences")
        };
        format!("Replace {found} in {}?", plural(m, "file", "files"))
    }

    fn rows(&self) -> usize {
        self.names.len()
            + if self.show_skipped {
                1 + self.plan.skipped.len()
            } else {
                0
            }
    }

    /// Scrolls so the row the keyboard is on shows.
    pub fn reveal(&self, scroll: f32, view: f32) -> f32 {
        let y = self.cursor as f32 * CONFIRM_ROW;
        if y < scroll {
            y
        } else if y + CONFIRM_ROW > scroll + view {
            y + CONFIRM_ROW - view
        } else {
            scroll
        }
    }

    /// The row index the pointer is on in the list, if a file row.
    pub fn row_at(&self, list: Rect, scroll: f32, x: f32, y: f32) -> Option<usize> {
        if !list.contains(x, y) {
            return None;
        }
        let i = ((y - list.y + scroll) / CONFIRM_ROW).floor() as usize;
        (i < self.names.len()).then_some(i)
    }
}

/// The panel's replace state. Empty (idle, no box text, nothing searched) until a folder search
/// has run.
pub struct Replacing {
    pub field: TextField,
    pub searched: Option<Searched>,
    pub phase: Phase,
    pub confirm: Option<Confirm>,
    pub outcome: Option<Outcome>,
    /// Counts every change the panel's look depends on, for its signature.
    pub generation: u64,
}

impl Default for Replacing {
    fn default() -> Replacing {
        Replacing {
            field: TextField::new(),
            searched: None,
            phase: Phase::Idle,
            confirm: None,
            outcome: None,
            generation: 0,
        }
    }
}

impl Replacing {
    /// A search of a folder ran: what it searched is what a replacement will plan. Anything of
    /// an earlier replacement is over.
    pub fn searched(&mut self, root: &Path, query: Query, load: LoadOptions) {
        self.forget();
        self.searched = Some(Searched {
            root: root.to_path_buf(),
            query,
            load,
        });
    }

    /// The panel shows something that is not a folder search (open tabs), or is gone.
    pub fn clear(&mut self) {
        self.forget();
        self.searched = None;
    }

    /// Drops the report and the confirmation and stops a plan being made. A replacement being
    /// written is not stopped here (`busy` says so; the window leaves it to finish or to Cancel).
    pub fn forget(&mut self) {
        if let Phase::Planning(job) = &self.phase {
            job.cancel();
            self.phase = Phase::Idle;
        }
        self.confirm = None;
        self.outcome = None;
        self.generation += 1;
    }

    /// Files are being written (a replacement or an undo): the panel stays as it is.
    pub fn busy(&self) -> bool {
        matches!(self.phase, Phase::Applying(_) | Phase::Reverting(_))
    }

    /// Anything is running, a plan included.
    pub fn running(&self) -> bool {
        !matches!(self.phase, Phase::Idle)
    }

    pub fn start_plan(&mut self, wake: Waker, skip: Vec<PathBuf>) -> bool {
        let Some(s) = self.searched.clone() else {
            return false;
        };
        let with = self.field.text().to_string();
        self.outcome = None;
        self.phase = Phase::Planning(Job::start(wake, move |cancel, progress| {
            let opts = PlanOptions {
                load: s.load,
                ..PlanOptions::default()
            };
            replace_files::plan(
                Source::Tree(&s.root),
                &s.query,
                &with,
                &opts,
                &skip,
                cancel,
                progress,
            )
        }));
        self.generation += 1;
        true
    }

    /// Writes the ticked files of the confirmation.
    pub fn start_apply(&mut self, wake: Waker, skip: Vec<PathBuf>) -> bool {
        let Some(c) = self.confirm.take() else {
            return false;
        };
        let (chosen, backup) = (c.chosen(), c.backup);
        let plan = c.plan;
        self.phase = Phase::Applying(Job::start(wake, move |cancel, progress| {
            let opts = ApplyOptions {
                backup,
                skip: &skip,
                ..ApplyOptions::default()
            };
            replace_files::apply(&plan, &chosen, &opts, cancel, progress)
        }));
        self.generation += 1;
        true
    }

    /// Puts the changed files back.
    pub fn start_revert(&mut self, wake: Waker) -> bool {
        let Some(report) = self.outcome.as_mut().and_then(|o| o.report.take()) else {
            return false;
        };
        self.phase = Phase::Reverting(Job::start(wake, move |cancel, progress| {
            replace_files::revert(&report, cancel, progress)
        }));
        self.generation += 1;
        true
    }

    /// Asks the step in progress to stop (between files).
    pub fn cancel(&mut self) {
        match &self.phase {
            Phase::Planning(j) => j.cancel(),
            Phase::Applying(j) => j.cancel(),
            Phase::Reverting(j) => j.cancel(),
            Phase::Idle => {}
        }
    }

    /// Takes the result of a step that finished.
    pub fn finished(&mut self) -> Option<Finished> {
        let out = match &self.phase {
            Phase::Planning(j) => j.take().map(Finished::Plan),
            Phase::Applying(j) => j.take().map(Finished::Applied),
            Phase::Reverting(j) => j.take().map(Finished::Reverted),
            Phase::Idle => None,
        };
        if out.is_some() {
            self.phase = Phase::Idle;
            self.generation += 1;
        }
        out
    }

    /// What the panel says while a step runs.
    pub fn progress_text(&self) -> Option<String> {
        let (word, j) = match &self.phase {
            Phase::Planning(j) => ("Preparing", j.progress()),
            Phase::Applying(j) => ("Replacing", j.progress()),
            Phase::Reverting(j) => ("Restoring", j.progress()),
            Phase::Idle => return None,
        };
        Some(if j.1 == 0 {
            format!("{word}\u{2026}")
        } else {
            format!("{word}\u{2026} {} of {}", j.0, j.1)
        })
    }

    /// The panel's signature: what its controls and progress look like.
    pub fn sig(&self) -> (u64, Option<(usize, usize)>, usize) {
        let progress = match &self.phase {
            Phase::Planning(j) => Some(j.progress()),
            Phase::Applying(j) => Some(j.progress()),
            Phase::Reverting(j) => Some(j.progress()),
            Phase::Idle => None,
        };
        (self.generation, progress, self.field.text().len())
    }

    pub fn tip(&self, hit: Hit) -> Option<String> {
        match hit {
            Hit::ResultsReplaceAll => Some(
                "Replace the text in every file listed, after you have seen and ticked them"
                    .to_string(),
            ),
            Hit::ResultsUndo => self
                .outcome
                .as_ref()
                .map(Outcome::undo_help)
                .filter(|t| !t.is_empty()),
            _ => None,
        }
    }
}

// ---- the panel's title row ---------------------------------------------------------------------

fn small_button(c: &mut Ctx, b: Rect, hit: Hit, label: &str, enabled: bool) {
    let pal = *c.pal;
    let fill = c
        .fill_for(hit, false, false)
        .filter(|_| enabled)
        .map_or(pal.field, |f| mix(pal.field, f, 0.6));
    c.p.frame(b, 6.0, fill, pal.line);
    c.p.text_center(
        Face::Ui,
        12.5,
        b,
        label,
        if enabled { pal.text } else { pal.faint },
    );
    if enabled {
        c.hit(hit, b);
    }
}

/// Draws the controls at the right of the panel's title row `r` and returns the x where they
/// start (the title stops before it). `search_done`: a folder search has finished with files in
/// it.
pub fn draw_head(c: &mut Ctx, ui: &mut Ui, r: Rect, search_done: bool) -> f32 {
    let right = r.right() - 44.0;
    let y = r.y + 4.0;
    let width = |c: &mut Ctx, label: &str| (c.p.measure(Face::Ui, 12.5, label) + 24.0).round();
    let focused = ui.find_focused && ui.field == BarField::ResultsReplace;
    let caret_on = ui.caret_on;
    let rep = &mut ui.results.replace;
    if let Some(text) = rep.progress_text() {
        let w = width(c, "Cancel");
        let b = Rect::new(right - w, y, w, CTRL_H);
        small_button(c, b, Hit::ResultsCancel, "Cancel", true);
        let pal = *c.pal;
        let tw = c.p.measure(Face::Ui, 12.5, &text);
        let x = b.x - 12.0 - tw;
        c.p.text(Face::Ui, 12.5, x, b.cy(), &text, pal.muted, tw + 2.0);
        return x;
    }
    if let Some(o) = &rep.outcome {
        if o.report.is_none() {
            return right;
        }
        let ok = o.revertible() > 0;
        let w = width(c, "Undo replace");
        let b = Rect::new(right - w, y, w, CTRL_H);
        small_button(c, b, Hit::ResultsUndo, "Undo replace", ok);
        return b.x;
    }
    if rep.searched.is_none() || !search_done {
        return right;
    }
    let w = width(c, "Replace all\u{2026}");
    let avail = right - w - 8.0 - (r.x + 16.0 + TITLE_MIN);
    let box_w = avail.min(BOX_MAX);
    if box_w < BOX_MIN {
        return right;
    }
    let b = Rect::new(right - w, y, w, CTRL_H);
    // Replacing with nothing is allowed (it deletes), so the button needs no text.
    small_button(c, b, Hit::ResultsReplaceAll, "Replace all\u{2026}", true);
    let frame = Rect::new(b.x - 6.0 - box_w, y, box_w, CTRL_H);
    draw_field(
        c,
        &mut rep.field,
        focused,
        caret_on,
        frame,
        BarField::ResultsReplace,
        false,
    );
    c.hit(Hit::ResultsReplace, frame);
    frame.x
}

// ---- the report ----------------------------------------------------------------------------------

/// The rows of a report, in the list area `area`, scrolled by `scroll`.
pub fn draw_outcome(c: &mut Ctx, o: &Outcome, panel: Rect, area: Rect, scroll: f32) {
    let pal = *c.pal;
    let quiet = mix(pal.text, pal.muted, 0.35);
    let old = c.p.clip(area);
    let first = (scroll / OUTCOME_ROW).floor() as usize;
    for (i, row) in o.rows.iter().enumerate().skip(first) {
        let y = area.y + i as f32 * OUTCOME_ROW - scroll;
        if y > area.bottom() {
            break;
        }
        let rr = Rect::new(panel.x + 6.0, y, panel.w - 12.0, OUTCOME_ROW_H);
        let cy = rr.cy();
        match row {
            OutRow::Heading(text) => {
                c.p.text(
                    Face::Semibold,
                    11.5,
                    panel.x + 16.0,
                    cy,
                    &text.to_uppercase(),
                    pal.muted,
                    rr.w - 20.0,
                );
            }
            OutRow::Changed { name, count } => {
                c.p.icon(glyph::DOC, 13.0, panel.x + 24.0, cy, pal.muted);
                let w = c.p.text(
                    Face::Semibold,
                    12.5,
                    panel.x + 38.0,
                    cy,
                    name,
                    pal.text,
                    rr.w - 120.0,
                );
                if *count > 0 {
                    let n = plural(*count, "replacement", "replacements");
                    c.p.text(Face::Ui, 12.0, panel.x + 48.0 + w, cy, &n, pal.faint, 140.0);
                }
            }
            OutRow::Skipped { name, why } | OutRow::Failed { name, why } => {
                let failed = matches!(row, OutRow::Failed { .. });
                let tint = if failed { pal.warn } else { pal.muted };
                c.p.icon(glyph::WARN, 13.0, panel.x + 24.0, cy, tint);
                let w = c.p.text(
                    Face::Ui,
                    12.5,
                    panel.x + 38.0,
                    cy,
                    name,
                    if failed { pal.text } else { quiet },
                    (rr.w - 60.0) * 0.5,
                );
                let x = panel.x + 38.0 + w + 12.0;
                c.p.text(Face::Ui, 12.0, x, cy, why, tint, rr.right() - x - 14.0);
            }
        }
    }
    c.p.restore(old);
}

// ---- the confirmation dialog -------------------------------------------------------------------------

/// The card, sized for a `w` x `h` window.
pub fn card(w: f32, h: f32) -> Rect {
    dialog_card(660.0, 620.0, w, h)
}

/// Where the parts of the card go.
pub struct Parts {
    pub list: Rect,
    pub preview: Rect,
    pub options: Rect,
}

pub fn parts(r: Rect) -> Parts {
    let pad = DIALOG_PAD;
    let top = r.y + pad + 58.0;
    let options = Rect::new(
        r.x + pad,
        r.bottom() - DIALOG_FOOTER - OPTIONS_H - 4.0,
        r.w - 2.0 * pad,
        OPTIONS_H,
    );
    let room = options.y - top - 8.0;
    // A low window gives up the preview before it squeezes the list.
    let preview_h = (room - 90.0).clamp(0.0, PREVIEW_H);
    let preview = Rect::new(
        r.x + pad,
        options.y - 6.0 - preview_h,
        r.w - 2.0 * pad,
        preview_h,
    );
    let list_bottom = if preview_h > 0.0 {
        preview.y - 8.0
    } else {
        options.y - 6.0
    };
    Parts {
        list: Rect::new(r.x + 8.0, top, r.w - 16.0, (list_bottom - top).max(0.0)),
        preview,
        options,
    }
}

fn tick_box(c: &mut Ctx, r: Rect, on: bool) {
    let pal = *c.pal;
    if on {
        c.p.rbox(r, 4.0, pal.accent);
        c.p.icon(glyph::CHECK, 11.0, r.cx(), r.cy(), pal.on_accent);
    } else {
        c.p.frame(r, 4.0, pal.field, mix(pal.line, pal.text, 0.3));
    }
}

fn dialog_button(c: &mut Ctx, r: Rect, hit: Hit, label: &str, primary: bool, enabled: bool) {
    let pal = *c.pal;
    if enabled {
        let hovered = c.hover == Some(hit);
        draw_dialog_button(
            c.p,
            &pal,
            r,
            label,
            primary,
            hovered,
            c.pressed == Some(hit),
        );
        c.hit(hit, r);
    } else {
        c.p.frame(r, 6.0, pal.field, pal.line);
        c.p.text_center(Face::Semibold, 13.0, r, label, pal.faint);
    }
}

/// The confirmation as a dialog: the title, the files with their ticks, a few lines of what
/// would change in the file the pointer (or the keyboard) is on, the backup option, the files
/// skipped, and the two buttons. `d` holds the list's scroll like the other dialogs.
pub fn draw_confirm(
    c: &mut Ctx,
    cf: &Confirm,
    d: &mut crate::ui::Dialog,
    w: f32,
    h: f32,
    hover_row: Option<usize>,
) {
    let pal = *c.pal;
    c.hit(Hit::Backdrop, Rect::new(0.0, 0.0, w, h));
    draw_backdrop(c.p, &pal, w, h);
    let r = card(w, h);
    c.hit(Hit::DlgCard, r);
    let footer = draw_dialog_card(c.p, &pal, r);
    let pad = DIALOG_PAD;
    c.p.text(
        Face::Semibold,
        17.0,
        r.x + pad,
        r.y + pad + 10.0,
        &cf.title(),
        pal.text,
        r.w - 2.0 * pad,
    );
    let with = cf.plan.replacement();
    let sub = format!(
        "\u{201C}{}\u{201D} \u{2192} {}",
        cf.plan.query().text(),
        if with.is_empty() {
            "nothing".to_string()
        } else {
            format!("\u{201C}{with}\u{201D}")
        }
    );
    c.p.text(
        Face::Ui,
        12.5,
        r.x + pad,
        r.y + pad + 36.0,
        &sub,
        pal.muted,
        r.w - 2.0 * pad,
    );
    let Parts {
        list,
        preview,
        options,
    } = parts(r);

    // The files.
    d.list = list;
    d.view = list.h;
    d.content = cf.rows() as f32 * CONFIRM_ROW + 8.0;
    d.scroll = d.scroll.clamp(0.0, (d.content - d.view).max(0.0));
    let old = c.p.clip(list);
    let shown = hover_row.unwrap_or(cf.cursor);
    for i in 0..cf.rows() {
        let y = list.y + i as f32 * CONFIRM_ROW - d.scroll;
        if y + CONFIRM_ROW < list.y || y > list.bottom() {
            continue;
        }
        let row = Rect::new(list.x + 8.0, y + 1.0, list.w - 16.0, CONFIRM_ROW - 2.0);
        if i < cf.names.len() {
            if shown == i {
                c.p.rbox(row, 6.0, mix(pal.popup, pal.text, 0.05));
            }
            tick_box(
                c,
                Rect::new(row.x + 10.0, row.cy() - 8.0, 16.0, 16.0),
                cf.ticks[i],
            );
            let f = &cf.plan.files[i];
            let count = if f.capped {
                format!("{}+", f.matches)
            } else {
                f.matches.to_string()
            };
            let cw = c.p.text_right(
                Face::Ui,
                12.5,
                row.right() - 12.0,
                row.cy(),
                &count,
                pal.faint,
            );
            c.p.text(
                Face::Semibold,
                13.0,
                row.x + 38.0,
                row.cy(),
                &cf.names[i],
                if cf.ticks[i] { pal.text } else { pal.muted },
                row.w - 38.0 - cw - 28.0,
            );
            // Only the part in the list takes the pointer.
            let top = row.y.max(list.y);
            let bottom = row.bottom().min(list.bottom());
            if bottom > top {
                c.hit(Hit::DlgOpen(i), Rect::new(row.x, top, row.w, bottom - top));
            }
        } else if i == cf.names.len() {
            c.p.text(
                Face::Semibold,
                11.5,
                row.x + 12.0,
                row.cy(),
                &format!("SKIPPED ({})", cf.plan.skipped_count),
                pal.muted,
                row.w - 24.0,
            );
        } else if let Some((path, why)) = cf.plan.skipped.get(i - cf.names.len() - 1) {
            let name = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            c.p.icon(glyph::WARN, 13.0, row.x + 18.0, row.cy(), pal.muted);
            let nw = c.p.text(
                Face::Ui,
                12.5,
                row.x + 38.0,
                row.cy(),
                &name,
                pal.muted,
                (row.w - 60.0) * 0.45,
            );
            let x = row.x + 38.0 + nw + 12.0;
            c.p.text(
                Face::Ui,
                12.0,
                x,
                row.cy(),
                why,
                pal.faint,
                row.right() - x - 12.0,
            );
        }
    }
    c.p.restore(old);
    if let Some((track, thumb)) = scrollbar(d.list, d.content, d.view, d.scroll) {
        let hit = Hit::ScrollBar(Scroller::Dialog);
        draw_scrollbar(c.p, &pal, track, thumb, c.hover == Some(hit));
        c.hit(hit, track);
    }

    // What would change in one file.
    if preview.h > 0.0 {
        c.p.frame(preview, 6.0, pal.field, pal.line);
        if let Some(f) = cf.plan.files.get(shown) {
            let x = preview.x + 12.0;
            let mut y = preview.y + 15.0;
            c.p.text(
                Face::Ui,
                11.5,
                x,
                y,
                &cf.names[shown],
                pal.faint,
                preview.w - 24.0,
            );
            let half = ((preview.w - 24.0 - 60.0) / 2.0).max(40.0);
            let mid = x + 54.0 + half;
            for p in f.preview.iter().take(3) {
                y += 21.0;
                c.p.text_right(
                    Face::Ui,
                    11.5,
                    x + 40.0,
                    y,
                    &(p.line + 1).to_string(),
                    pal.faint,
                );
                c.p.text(
                    Face::Ui,
                    12.5,
                    x + 54.0,
                    y,
                    &p.before,
                    pal.muted,
                    half - 8.0,
                );
                c.p.icon(glyph::RIGHT, 11.0, mid, y, pal.faint);
                c.p.text(
                    Face::Ui,
                    12.5,
                    mid + 14.0,
                    y,
                    &p.after,
                    pal.text,
                    half - 8.0,
                );
            }
            let more = f.matches.saturating_sub(f.preview.len().min(3));
            if more > 0 {
                y += 21.0;
                c.p.text(
                    Face::Ui,
                    11.5,
                    x + 54.0,
                    y,
                    &format!("and {more} more"),
                    pal.faint,
                    half,
                );
            }
        }
    }

    // The backup option and the skipped files.
    let tick = Rect::new(options.x + 2.0, options.cy() - 8.0, 16.0, 16.0);
    tick_box(c, tick, cf.backup);
    let label = "Keep backup copies (.bak)";
    let lw = c.p.text(
        Face::Ui,
        13.0,
        tick.right() + 10.0,
        options.cy(),
        label,
        pal.text,
        300.0,
    );
    c.hit(
        Hit::DlgButton(2),
        Rect::new(options.x, options.y, tick.w + 10.0 + lw + 8.0, options.h),
    );
    if cf.plan.skipped_count > 0 {
        let text = format!(
            "{}: {}",
            plural(cf.plan.skipped_count, "file skipped", "files skipped"),
            if cf.show_skipped { "hide" } else { "show" }
        );
        let tw = c.p.measure(Face::Ui, 12.5, &text);
        let b = Rect::new(options.right() - tw - 12.0, options.y, tw + 12.0, options.h);
        let color = if c.hover == Some(Hit::DlgButton(3)) {
            pal.accent
        } else {
            pal.muted
        };
        c.p.text(Face::Ui, 12.5, b.x + 6.0, b.cy(), &text, color, tw + 2.0);
        c.hit(Hit::DlgButton(3), b);
    }

    // The answers sit at the right in reading order, the first one the default.
    let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
    let mut x = r.right() - pad;
    for (n, label) in ["Replace", "Cancel"].iter().enumerate().rev() {
        let bw = dialog_button_w(c.p, label);
        x -= bw;
        let on = n == 1 || !cf.chosen().is_empty();
        dialog_button(
            c,
            Rect::new(x, by, bw, 32.0),
            Hit::DlgButton(n),
            label,
            n == 0,
            on,
        );
        x -= 8.0;
    }
}

/// Whether the dialog on screen is this one.
pub fn is_confirm(kind: DialogKind) -> bool {
    kind == DialogKind::ReplaceFiles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plural_and_tail_read_naturally() {
        assert_eq!(plural(1, "file", "files"), "1 file");
        assert_eq!(plural(0, "file", "files"), "0 files");
        assert_eq!(tail(0, 0, false), "");
        assert_eq!(
            tail(2, 1, true),
            " \u{00B7} 2 skipped \u{00B7} 1 failed \u{00B7} stopped"
        );
    }
}
