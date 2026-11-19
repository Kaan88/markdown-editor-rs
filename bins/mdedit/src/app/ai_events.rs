//! AI rewriting as the window hosts it: the commands start a run on the selection (or the whole
//! document), the engine's answers arrive as `Wake::Ai`, and the result is shown for review or
//! applied at once, as one undo step. The two panels (instruction, review) are drawn last and,
//! while open, take every key, pointer and wheel event, as the palette does.

use super::*;
use crate::ai::{Ask, Bar, BarAction, Event, Hit as AiHit, Instruction, Panel, Review, Run, Spec};
use crate::keys::KeyName;
use crate::preview_pane::Pane;
use mdedit_core::ai::{self as core_ai, Connection};
use mdshared_ui::textfield::TextField;
use serde_json::json;

/// The most text one rewrite takes: models answer slowly and lose the thread on more.
const AI_TEXT_MAX: usize = 200_000;
/// The saved prompts file is read up to this size.
const PROMPTS_BYTES: u64 = 256 * 1024;
/// Lines the review scrolls by for Up and Down.
const REVIEW_STEP: f32 = crate::ai_ui::LINE_H * 3.0;

/// A run's text and where it came from.
struct Scope {
    doc: DocId,
    start: usize,
    end: usize,
    text: String,
    label: String,
}

impl App {
    // ---- saved prompts -------------------------------------------------------------------------

    fn ai_prompts_path(&self) -> Option<PathBuf> {
        self.snippets_path()
            .map(|p| p.with_file_name(core_ai::PROMPTS_FILE))
    }

    /// Reads the saved prompts again when the file is new or has changed.
    pub(super) fn ai_load_prompts(&mut self) {
        let path = self.ai_prompts_path();
        let stamp = path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| (m.modified().ok(), m.len()));
        if self.ai.prompts_stamp == Some(stamp) {
            return;
        }
        self.ai.prompts_stamp = Some(stamp);
        self.docs.ai_prompts = match (path, stamp) {
            (Some(p), Some((_, len))) if len <= PROMPTS_BYTES => crate::ai::read_prompts(&p),
            _ => Vec::new(),
        };
    }

    pub(super) fn ai_edit_prompts(&mut self) {
        let Some(path) = self.ai_prompts_path() else {
            self.docs
                .set_toast("There is no folder for the saved prompts file.");
            return;
        };
        if let Err(e) = crate::ai::ensure_prompts_file(&path) {
            self.docs
                .set_toast(&format!("Could not create the saved prompts file: {e}"));
            return;
        }
        self.ai.panel = None;
        self.open(&path);
    }

    // ---- starting ----------------------------------------------------------------------------

    fn ai_conn(&self) -> Connection {
        self.docs.prefs.ai_connection
    }

    /// The connection, model and effort, as the panels name them.
    fn ai_via(&self) -> String {
        let p = &self.docs.prefs;
        let c = p.ai_connection;
        let mut s = match c {
            Connection::OpenAi => {
                let preset = core_ai::preset_by_key(&p.ai_preset);
                preset
                    .name
                    .split(" (")
                    .next()
                    .unwrap_or(preset.name)
                    .to_string()
            }
            _ => c.short().to_string(),
        };
        let model = p.ai_model(c);
        if !model.is_empty() {
            s.push_str(" · ");
            s.push_str(model);
        }
        let effort = p.ai_effort(c);
        if !effort.is_empty() {
            s.push_str(" · effort ");
            s.push_str(&core_ai::effort_label(effort).to_lowercase());
        }
        s
    }

    /// The selection of the current document, or all of it when nothing is selected.
    fn ai_scope(&self) -> Option<Scope> {
        let i = self.docs.active()?;
        let doc = &self.docs.tabs[i];
        let r = doc.ready()?;
        let sel = r.view.sel.range();
        let (start, end, label) = if sel.is_empty() {
            (0, r.buffer.len_bytes(), "The whole document".to_string())
        } else {
            let first = r.buffer.line_of(sel.start);
            let last = r.buffer.line_of(sel.end.saturating_sub(1).max(sel.start));
            let n = last - first + 1;
            let s = if n == 1 { "" } else { "s" };
            (sel.start, sel.end, format!("The selection, {n} line{s}"))
        };
        let mut text = String::new();
        if end - start <= AI_TEXT_MAX {
            r.buffer.text(start..end, &mut text);
        }
        Some(Scope {
            doc: doc.id,
            start,
            end,
            text,
            label,
        })
    }

    /// Whether a rewrite can start now; if not, says why.
    fn ai_can_start(&mut self) -> bool {
        if self.ai.running() {
            self.docs
                .set_toast("An AI rewrite is running. Esc stops it.");
            self.redraw();
            return false;
        }
        if self.ai_conn() == Connection::Off {
            self.ai_set_bar(Bar {
                text: "AI is off. Choose a connection in Settings › AI to rewrite with it.".into(),
                action: Some(BarAction::OpenSettings),
                info: true,
            });
            return false;
        }
        true
    }

    pub(super) fn ai_builtin(&mut self, instruction: &str, title: &str) {
        self.ai_start(Spec {
            title: title.into(),
            instruction: instruction.into(),
        });
    }

    pub(super) fn ai_saved(&mut self, k: usize) {
        self.ai_load_prompts();
        if let Some(p) = self.docs.ai_prompts.get(k).cloned() {
            self.ai_start(Spec {
                title: p.name,
                instruction: p.instruction,
            });
        }
    }

    fn ai_start(&mut self, spec: Spec) {
        self.ai.panel = None;
        if !self.ai_can_start() {
            return;
        }
        let Some(scope) = self.ai_scope() else {
            return;
        };
        if scope.end - scope.start > AI_TEXT_MAX {
            self.ai_set_bar(Bar {
                text: format!(
                    "The text is too long to rewrite in one go ({} KB at most). Select a part of it.",
                    AI_TEXT_MAX / 1000
                ),
                action: None,
                info: true,
            });
            return;
        }
        if scope.text.trim().is_empty() {
            self.docs.set_toast("There is no text to rewrite.");
            self.redraw();
            return;
        }
        let p = &self.docs.prefs;
        let c = p.ai_connection;
        let req = json!({
            "op": "run",
            "conn": crate::ai::conn_json(c, p),
            "model": p.ai_model(c),
            "effort": p.ai_effort(c),
            "system": core_ai::SYSTEM,
            "prompt": core_ai::prompt(&spec.instruction, &scope.text),
        });
        let via = self.ai_via();
        let wake = self.docs.waker();
        match self.ai.ask(&wake, Ask::Run, req) {
            Err(why) => self.ai_set_bar(Bar {
                text: why,
                action: None,
                info: false,
            }),
            Ok(id) => {
                self.ai.last = Some(spec.clone());
                self.ai.run = Some(Run {
                    id,
                    doc: scope.doc,
                    start: scope.start,
                    end: scope.end,
                    original: scope.text,
                    spec,
                    scope: scope.label,
                    via,
                    chars: 0,
                });
                if self.ai.bar.take().is_some() {
                    self.relayout_areas();
                }
                self.ai_status();
                self.redraw();
            }
        }
    }

    /// The status strip while a rewrite runs.
    fn ai_status(&mut self) {
        self.docs.ai_status = self.ai.run.as_ref().map(|r| {
            let what = if r.chars == 0 {
                format!("AI: {} with {}…", r.spec.title, r.via)
            } else {
                format!("AI: {}… {} characters", r.spec.title, r.chars)
            };
            format!("{what}   Esc stops")
        });
    }

    pub(super) fn ai_cancel(&mut self) {
        if self.ai.running() {
            self.ai.cancel();
            self.docs.ai_status = None;
            self.docs.set_toast("Stopped the AI rewrite.");
            self.redraw();
        }
    }

    /// Esc with no panel open stops a running rewrite first.
    pub(super) fn ai_escape(&mut self) -> bool {
        if self.ai.running() {
            self.ai_cancel();
            return true;
        }
        false
    }

    pub(super) fn ai_open_settings(&mut self) {
        self.ai.panel = None;
        self.open_settings();
        // The form is not laid out yet: it keeps the jump for its first layout.
        if let Some(f) = self.ui.settings.as_mut()
            && let Some(cat) = f.categories().iter().position(|c| *c == "AI")
        {
            f.jump_to_category(cat);
        }
        self.ai_settings_needs();
    }

    // ---- the bar -----------------------------------------------------------------------------

    fn ai_set_bar(&mut self, bar: Bar) {
        self.ai.bar = Some(bar);
        self.relayout_areas();
        self.redraw();
    }

    pub(super) fn ai_bar_close(&mut self) {
        if self.ai.bar.take().is_some() {
            self.relayout_areas();
            self.redraw();
        }
    }

    pub(super) fn ai_bar_go(&mut self) {
        let action = self.ai.bar.as_ref().and_then(|b| b.action);
        self.ai_bar_close();
        match action {
            Some(BarAction::OpenSettings) => self.ai_open_settings(),
            Some(BarAction::TryAgain) => {
                if let Some(spec) = self.ai.last.clone() {
                    self.ai_start(spec);
                }
            }
            None => {}
        }
    }

    /// The bar for a failed run.
    fn ai_failed(&mut self, kind: &str, message: String, retry_after: Option<u64>) {
        let action = match kind {
            "cancelled" => return,
            "auth" | "keystore" | "not_found" | "bad_request" => Some(BarAction::OpenSettings),
            "declined" => None,
            _ => Some(BarAction::TryAgain),
        };
        let mut text = message;
        if let Some(s) = retry_after.filter(|_| kind == "rate") {
            text.push_str(&format!(" Try again in {s} s."));
        }
        self.ai_set_bar(Bar {
            text,
            action,
            info: false,
        });
    }

    // ---- the engine's answers ----------------------------------------------------------------

    pub(super) fn ai_wake(&mut self) {
        let mut settings = false;
        let mut needs = false;
        for e in self.ai.poll() {
            match e {
                Event::Delta { id, chars } => {
                    if let Some(r) = self.ai.run.as_mut().filter(|r| r.id == id) {
                        r.chars = chars;
                        self.ai_status();
                    }
                }
                Event::Done { id, text } => {
                    self.ai.answered(id, true);
                    if self.ai.run.as_ref().is_some_and(|r| r.id == id) {
                        let run = self.ai.run.take().expect("checked");
                        self.docs.ai_status = None;
                        self.ai_finish(run, &text);
                    }
                }
                Event::Error {
                    id,
                    kind,
                    message,
                    retry_after,
                } => match self.ai.answered(id, true) {
                    Some(Ask::Run) => {
                        if self.ai.run.as_ref().is_some_and(|r| r.id == id) {
                            self.ai.run = None;
                            self.docs.ai_status = None;
                            self.ai_failed(&kind, message, retry_after);
                        }
                    }
                    Some(Ask::Models(key)) => {
                        self.ai.models.insert(key, Err(message));
                        self.ai.fetching = None;
                        settings = true;
                    }
                    Some(Ask::Detect(c)) => {
                        self.ai.detected.insert(c.key(), Err(message));
                        settings = true;
                    }
                    Some(Ask::Key(_)) => {
                        self.ai.key_error = Some(message);
                        settings = true;
                    }
                    None => {}
                },
                Event::Models { id, list } => {
                    if let Some(Ask::Models(key)) = self.ai.answered(id, true) {
                        self.ai.models.insert(key, Ok(list));
                        self.ai.fetching = None;
                        settings = true;
                    }
                }
                Event::Detected { id, path, version } => {
                    if let Some(Ask::Detect(c)) = self.ai.answered(id, true) {
                        self.ai.detected.insert(c.key(), Ok((path, version)));
                        settings = true;
                    }
                }
                Event::Key { id, stored } => {
                    if let Some(Ask::Key(slot)) = self.ai.answered(id, true) {
                        self.ai.keys.insert(slot, stored);
                        self.ai.key_error = None;
                        settings = true;
                        // A key found lets the model list be fetched.
                        needs = true;
                    }
                }
                Event::Ended(why) => {
                    if self.ai.run.take().is_some() {
                        self.docs.ai_status = None;
                        self.ai_set_bar(Bar {
                            text: why,
                            action: Some(BarAction::TryAgain),
                            info: false,
                        });
                    }
                    settings = true;
                }
            }
        }
        if needs {
            self.ai_settings_needs();
        }
        if settings {
            self.ai_settings_refresh();
        }
        self.ai.tick += 1;
        self.redraw();
    }

    /// Why `original` at `start..end` of document `doc` cannot be replaced now, if it cannot.
    fn ai_blocked(&self, doc: DocId, start: usize, end: usize, original: &str) -> Option<String> {
        let Some(d) = self.docs.index_of_id(doc).map(|i| &self.docs.tabs[i]) else {
            return Some(core_ai::BLOCKED_CLOSED.into());
        };
        // A reload in flight replaces the text.
        let Some(r) = d.ready().filter(|_| !d.is_loading()) else {
            return Some(core_ai::BLOCKED_CHANGED.into());
        };
        if r.info.lossy || r.read_only {
            return Some(core_ai::BLOCKED_READ_ONLY.into());
        }
        let mut now = String::new();
        if start <= end && end <= r.buffer.len_bytes() {
            r.buffer.text(start..end, &mut now);
        }
        (end > r.buffer.len_bytes() || now != original).then(|| core_ai::BLOCKED_CHANGED.into())
    }

    /// A finished run: shown for review, or applied at once when the setting says so and the
    /// text is still as it was.
    fn ai_finish(&mut self, run: Run, reply: &str) {
        // A document closed (or being read again) meanwhile still gets the review, blocked, so
        // the result can be copied.
        let doc = self.docs.index_of_id(run.doc).map(|i| &self.docs.tabs[i]);
        let fitted = core_ai::fit_end(&run.original, reply);
        let result = match doc.and_then(|d| d.ready()) {
            Some(r) => core_ai::with_eol(&fitted, r.info.eol.dominant()),
            None => fitted,
        };
        if result.trim().is_empty() {
            self.ai_set_bar(Bar {
                text: "The AI sent back no text. Nothing was changed.".into(),
                action: Some(BarAction::TryAgain),
                info: false,
            });
            return;
        }
        if result == run.original {
            self.ai_set_bar(Bar {
                text: format!("{}: the AI changed nothing.", run.spec.title),
                action: None,
                info: true,
            });
            return;
        }
        let base_dir = doc
            .and_then(|d| d.path.as_deref())
            .and_then(|p| p.parent())
            .map(Path::to_path_buf);
        let mut blocked = self.ai_blocked(run.doc, run.start, run.end, &run.original);
        if !self.docs.prefs.ai_review && blocked.is_none() {
            if self.ai_replace(run.doc, run.start, run.end, &result) {
                self.docs.set_toast(&format!(
                    "{} done. Undo brings the old text back.",
                    run.spec.title
                ));
                return;
            }
            // Not taken: the result is shown, so it can still be copied.
            blocked = Some(core_ai::BLOCKED_READ_ONLY.into());
        }
        let (lines, removed, added) = core_ai::review(&run.original, &result);
        let preview = self.docs.prefs.ai_review_preview;
        self.close_for_ai_panel();
        self.ai.panel = Some(Panel::Review(Review {
            doc: run.doc,
            start: run.start,
            end: run.end,
            original: run.original,
            result,
            subtitle: format!("{} · {}", run.scope, run.via),
            spec: run.spec,
            lines,
            removed,
            added,
            blocked,
            preview,
            scroll: 0.0,
            content: 0.0,
            panes: None,
            base_dir,
        }));
        if preview {
            self.ai_open_panes();
        }
        self.redraw();
    }

    /// Puts `text` in place of `start..end` of document `doc`, as one undo step.
    /// Whether the text went in (not in a read-only document).
    fn ai_replace(&mut self, doc: DocId, start: usize, end: usize, text: &str) -> bool {
        let Some(i) = self.docs.index_of_id(doc) else {
            return false;
        };
        if self.docs.active() != Some(i) {
            self.docs.activate(i);
        }
        let changed = self.docs.edit_in(
            i,
            EditOp::ReplaceRange(start, end, text),
            self.service.as_ref(),
        );
        if changed && self.docs.find_stale() {
            self.find_due = Some(Instant::now() + super::commands::REFIND_DELAY);
        }
        self.caret_reset();
        self.redraw();
        changed
    }

    /// What gives way to a panel: the palette, menus, tooltips, the find box's keyboard.
    fn close_for_ai_panel(&mut self) {
        if self.palette_open() {
            self.close_palette();
        }
        self.ui.popup = None;
        self.ui.tip = None;
        self.ui.hover = None;
        self.ui.find_focused = false;
        self.ai.hover = None;
        self.ai.pressed = None;
    }

    // ---- the panels --------------------------------------------------------------------------

    pub(super) fn ai_panel_open(&self) -> bool {
        self.ai.panel.is_some()
    }

    pub(super) fn ai_open_instruction(&mut self) {
        if self.ui.settings.is_some() || !self.ai_can_start() {
            return;
        }
        let Some(scope) = self.ai_scope() else {
            return;
        };
        self.ai_load_prompts();
        self.close_for_ai_panel();
        self.ai.panel = Some(Panel::Instruction(Instruction {
            field: TextField::new(),
            spec: None,
            source: (scope.doc, scope.start, scope.end, scope.text),
            changed: false,
            selected: None,
            scope: scope.label,
            via: self.ai_via(),
            scroll: 0.0,
        }));
        self.caret_reset();
        self.ai.tick += 1;
        self.redraw();
    }

    fn ai_close_panel(&mut self) {
        self.ai.panel = None;
        self.ai.hover = None;
        self.ai.pressed = None;
        self.set_cursor(CursorIcon::Default);
        self.ai.tick += 1;
        self.redraw();
    }

    /// Run what the instruction panel holds: the typed text, or the prompt picked.
    fn ai_submit(&mut self) {
        let Some(i) = self.ai.instruction() else {
            return;
        };
        let typed = i.field.text().trim().to_string();
        let picked = i
            .selected
            .and_then(|k| self.docs.ai_prompts.get(k))
            .cloned();
        let again = i.spec.clone().filter(|s| typed == s.instruction.trim());
        let spec = match picked {
            _ if let Some(s) = again => s,
            Some(p) if typed.is_empty() || typed == p.instruction.trim() => Spec {
                title: p.name,
                instruction: p.instruction,
            },
            _ if !typed.is_empty() => Spec {
                title: "Rewrite".into(),
                instruction: typed,
            },
            _ => return,
        };
        // Only what the panel shows is sent: another tab, another selection or other text since
        // (a file opened or dropped, an edit) is shown first and waits for Rewrite again.
        let now = self.ai_scope();
        let Some(i) = self.ai.instruction() else {
            return;
        };
        match now {
            Some(n)
                if (n.doc, n.start, n.end, &n.text)
                    == (i.source.0, i.source.1, i.source.2, &i.source.3) =>
            {
                self.ai_start(spec)
            }
            Some(n) => {
                i.scope = format!(
                    "Changed since: {}. Press Rewrite again to send it.",
                    n.label
                );
                i.source = (n.doc, n.start, n.end, n.text);
                i.changed = true;
                self.ai.tick += 1;
                self.redraw();
            }
            None => self.ai_close_panel(),
        }
    }

    /// Moves the pick in the saved prompts and puts its instruction in the box.
    fn ai_pick(&mut self, down: bool) {
        let n = self.docs.ai_prompts.len();
        let prompts = self.docs.ai_prompts.clone();
        let Some(i) = self.ai.instruction() else {
            return;
        };
        if n == 0 {
            return;
        }
        let k = match (i.selected, down) {
            (None, true) => 0,
            (None, false) => n - 1,
            (Some(k), true) => (k + 1).min(n - 1),
            (Some(k), false) => k.saturating_sub(1),
        };
        i.selected = Some(k);
        i.field.set_text(prompts[k].instruction.trim());
        i.field.select_all();
    }

    /// A review whose document was closed after it opened says so, before its buttons are drawn
    /// or a key picks one: Copy result and Discard, no Apply (audit 1 round 3, #9).
    pub(super) fn ai_review_sync(&mut self) {
        let Some(Panel::Review(rv)) = self.ai.panel.as_ref() else {
            return;
        };
        let closed = Some(core_ai::BLOCKED_CLOSED);
        if rv.blocked.as_deref() != closed && self.docs.index_of_id(rv.doc).is_none() {
            if let Some(rv) = self.ai.review() {
                rv.blocked = Some(core_ai::BLOCKED_CLOSED.into());
            }
            self.ai.tick += 1;
        }
    }

    pub(super) fn ai_review_action(&mut self, hit: AiHit) {
        self.ai_review_sync();
        match hit {
            AiHit::Discard => self.ai_close_panel(),
            AiHit::Copy => {
                let Some(rv) = self.ai.review() else { return };
                // The review may be the only place the result is (its document closed): it
                // closes only once the result is on the clipboard (audit 1 round 3, #3).
                if copy_text(&rv.result) {
                    self.docs.set_toast("Copied the result.");
                    self.ai_close_panel();
                } else {
                    self.docs
                        .set_toast("Could not copy to the clipboard. The result is still here.");
                    self.redraw();
                }
            }
            AiHit::Apply => {
                let Some(rv) = self.ai.review() else { return };
                if rv.blocked.is_some() {
                    return;
                }
                let (doc, start, end) = (rv.doc, rv.start, rv.end);
                let original = rv.original.clone();
                // The text may have changed since the review opened (an edit elsewhere, a reload).
                let mut blocked = self.ai_blocked(doc, start, end, &original);
                if blocked.is_none() {
                    let Some(Panel::Review(rv)) = self.ai.panel.take() else {
                        return;
                    };
                    if self.ai_replace(doc, start, end, &rv.result) {
                        self.ai_close_panel();
                        return;
                    }
                    self.ai.panel = Some(Panel::Review(rv));
                    blocked = Some(core_ai::BLOCKED_READ_ONLY.into());
                }
                if let Some(rv) = self.ai.review() {
                    rv.blocked = blocked;
                }
                self.ai.tick += 1;
                self.redraw();
            }
            AiHit::RunAgain => {
                let Some(rv) = self.ai.review() else { return };
                let (doc, spec) = (rv.doc, rv.spec.clone());
                // Again on the review's document, never on whichever tab is active now; the
                // panel shows what will be sent and waits for Rewrite. A closed document keeps
                // the review, with its result to copy.
                let Some(i) = self.docs.index_of_id(doc) else {
                    rv.blocked = Some(core_ai::BLOCKED_CLOSED.into());
                    self.ai.tick += 1;
                    self.redraw();
                    return;
                };
                self.ai_close_panel();
                if self.docs.active() != Some(i) {
                    self.docs.activate(i);
                }
                self.ai_open_instruction();
                if let Some(ins) = self.ai.instruction() {
                    ins.field.set_text(spec.instruction.trim());
                    ins.field.select_all();
                    ins.spec = Some(spec);
                }
            }
            AiHit::Changes | AiHit::Preview => {
                let on = hit == AiHit::Preview;
                let Some(rv) = self.ai.review() else { return };
                if rv.preview == on {
                    return;
                }
                rv.preview = on;
                if self.docs.prefs.ai_review_preview != on {
                    let mut p = self.docs.prefs.clone();
                    p.ai_review_preview = on;
                    self.set_prefs(p);
                }
                if on {
                    self.ai_open_panes();
                }
                self.ai.tick += 1;
                self.redraw();
            }
            _ => {}
        }
    }

    /// The review's two preview panes, opened the first time Preview is shown.
    fn ai_open_panes(&mut self) {
        if self.ai.review().is_some_and(|rv| rv.panes.is_some()) {
            return;
        }
        if self.viewer_path.is_none() {
            self.look_for_viewer();
        }
        let viewer = self.viewer_path.clone().flatten();
        if viewer.is_none() && !crate::preview_pane::STANDALONE {
            return;
        }
        let wake = self.docs.waker();
        let Some(rv) = self.ai.review() else { return };
        let mut before = Pane::open(viewer.as_deref(), wake.clone());
        let mut after = Pane::open(viewer.as_deref(), wake);
        let base = rv.base_dir.clone();
        // Each pane shows one text that never changes: any id and version will do.
        before.set_text(rv.doc, (0, 0), base.clone(), || rv.original.clone());
        after.set_text(rv.doc, (0, 1), base, || rv.result.clone());
        rv.panes = Some(Box::new((before, after)));
    }

    /// `Wake::Preview`: the review's panes may have drawn.
    pub(super) fn ai_poll_panes(&mut self) {
        let Some(rv) = self.ai.review() else { return };
        let Some(panes) = rv.panes.as_mut() else {
            return;
        };
        let a = panes.0.poll();
        let b = panes.1.poll();
        if a || b {
            self.ai.tick += 1;
            self.redraw();
        }
    }

    // ---- input while a panel is open ---------------------------------------------------------

    pub(super) fn ai_key(&mut self, name: Option<KeyName>, typed: Option<&str>) {
        let Some(name) = name else { return };
        self.ai_review_sync();
        let mods = self.mods;
        self.ai.tick += 1;
        match self.ai.panel.as_mut() {
            Some(Panel::Instruction(i)) => match name {
                KeyName::Escape => self.ai_close_panel(),
                KeyName::Enter => self.ai_submit(),
                KeyName::Down => self.ai_pick(true),
                KeyName::Up => self.ai_pick(false),
                _ => {
                    if i.field.handle_key(&name, mods, typed, true) == Some(true) {
                        i.selected = None;
                    }
                }
            },
            Some(Panel::Review(rv)) => match name {
                KeyName::Escape => self.ai_close_panel(),
                KeyName::Enter => {
                    let hit = if rv.blocked.is_some() {
                        AiHit::Copy
                    } else {
                        AiHit::Apply
                    };
                    self.ai_review_action(hit);
                }
                KeyName::Char('c') if mods.ctrl => self.ai_review_action(AiHit::Copy),
                KeyName::Down => self.ai_scroll(REVIEW_STEP),
                KeyName::Up => self.ai_scroll(-REVIEW_STEP),
                KeyName::PageDown => self.ai_scroll(400.0),
                KeyName::PageUp => self.ai_scroll(-400.0),
                _ => {}
            },
            None => {}
        }
        self.caret_reset();
        self.redraw();
    }

    fn ai_scroll(&mut self, dy: f32) {
        let Some(rv) = self.ai.review() else { return };
        if rv.preview {
            if let Some(panes) = rv.panes.as_mut() {
                panes.0.scroll(dy);
                panes.1.scroll(dy);
            }
        } else {
            rv.scroll = (rv.scroll + dy).max(0.0);
        }
        self.ai.tick += 1;
        self.redraw();
    }

    pub(super) fn ai_pointer(&mut self, x: f32, y: f32) {
        let hit = self
            .ai
            .hit_at(x, y)
            .filter(|h| !matches!(h, AiHit::Card | AiHit::Body));
        let edge = self.edge_at(x, y).map(CursorIcon::from);
        let over_field = self.ai.hit_at(x, y) == Some(AiHit::Field);
        self.set_cursor(edge.unwrap_or(if over_field {
            CursorIcon::Text
        } else {
            CursorIcon::Default
        }));
        if hit != self.ai.hover {
            self.ai.hover = hit;
            self.ai.tick += 1;
            self.redraw();
        }
    }

    pub(super) fn ai_press(&mut self) {
        let (x, y) = self.ui.mouse;
        let hit = self.ai.hit_at(x, y);
        // A click beside the instruction panel closes it; beside the review it does nothing, so
        // a result is not lost by a stray click.
        if hit.is_none() && matches!(self.ai.panel, Some(Panel::Instruction(_))) {
            self.ai_close_panel();
            return;
        }
        self.ai.pressed = hit.filter(|h| !matches!(h, AiHit::Card | AiHit::Body | AiHit::Field));
        self.ai.tick += 1;
        self.redraw();
    }

    pub(super) fn ai_release(&mut self) {
        let (x, y) = self.ui.mouse;
        let Some(pressed) = self.ai.pressed.take() else {
            return;
        };
        self.ai.tick += 1;
        self.redraw();
        if self.ai.hit_at(x, y) != Some(pressed) {
            return;
        }
        match pressed {
            AiHit::Prompt(k) => {
                let prompts = self.docs.ai_prompts.clone();
                if let Some(i) = self.ai.instruction()
                    && let Some(p) = prompts.get(k)
                {
                    i.selected = Some(k);
                    i.field.set_text(p.instruction.trim());
                }
                self.ai_submit();
            }
            AiHit::Cancel => self.ai_close_panel(),
            AiHit::Rewrite => self.ai_submit(),
            hit => self.ai_review_action(hit),
        }
    }

    /// `dy` in logical px, positive towards the end.
    pub(super) fn ai_wheel(&mut self, dy: f32) {
        let (x, y) = self.ui.mouse;
        let over = self.ai.hit_at(x, y).is_some();
        match self.ai.panel.as_mut() {
            Some(Panel::Instruction(i)) => {
                i.scroll = (i.scroll + dy).max(0.0);
                self.ai.tick += 1;
                self.redraw();
            }
            Some(Panel::Review(_)) if over => self.ai_scroll(dy),
            Some(Panel::Review(_)) => {}
            None => {}
        }
    }

    /// What an input method composes goes to the instruction box.
    pub(super) fn ai_preedit(&mut self, pre: Option<mdshared_ui::textfield::Preedit>) -> bool {
        if self.ai.instruction().is_none() {
            return false;
        }
        self.ai.tick += 1;
        self.ai
            .instruction()
            .is_some_and(|i| i.field.set_preedit(pre))
    }

    #[cfg(test)]
    pub(super) fn ai_start_for_test(&mut self, instruction: &str) {
        self.ai_start(Spec {
            title: "Test".into(),
            instruction: instruction.into(),
        });
    }

    /// `--ai-demo`: a state of the AI feature for a shot, made up, with no engine.
    pub(super) fn ai_demo(&mut self, kind: &str) {
        use crate::ai::ModelInfo;
        let model = |id: &str, name: &str| ModelInfo {
            id: id.into(),
            name: name.into(),
            efforts: ["low", "medium", "high", "xhigh", "max"]
                .map(String::from)
                .to_vec(),
        };
        let key = self.ai_models_key_now();
        self.ai.models.insert(
            key,
            Ok(vec![
                model("claude-opus-5-5", "Claude Opus 5.5"),
                model("claude-sonnet-5-5", "Claude Sonnet 5.5"),
                model("claude-haiku-5-5", "Claude Haiku 5.5"),
            ]),
        );
        for c in [Connection::ClaudeCode, Connection::Codex] {
            self.ai.detected.insert(
                c.key(),
                if c == Connection::ClaudeCode {
                    Ok(("/usr/local/bin/claude".into(), "2.1.328".into()))
                } else {
                    Err("Codex was not found. Install it, or type where it is.".into())
                },
            );
        }
        if let Some(slot) = core_ai::key_slot(
            self.docs.prefs.ai_connection,
            &self.docs.prefs.ai_preset,
            &self.docs.prefs.ai_base_url,
        ) {
            self.ai.keys.insert(slot, true);
        }
        // A shot reads no saved prompts file: the template's prompts stand in.
        self.docs.ai_prompts = core_ai::parse_prompts(core_ai::PROMPTS_TEMPLATE);
        self.ai.prompts_stamp = Some(None);
        let Some(scope) = self.ai_scope() else { return };
        let result = |t: &str| {
            let mut out = String::new();
            for (k, line) in t.lines().enumerate() {
                if k == 0 && !line.starts_with('#') {
                    out.push_str("# ");
                }
                let line = line.trim_start_matches(['*', '-', ' ']);
                if k > 0 && !line.is_empty() && !line.starts_with('#') {
                    out.push_str("- ");
                }
                out.push_str(line);
                out.push('\n');
            }
            out
        };
        let spec = Spec {
            title: "Format as Markdown".into(),
            instruction: core_ai::FORMAT.into(),
        };
        let run = Run {
            id: 0,
            doc: scope.doc,
            start: scope.start,
            end: scope.end,
            original: scope.text.clone(),
            spec: spec.clone(),
            scope: scope.label.clone(),
            via: self.ai_via(),
            chars: 1240,
        };
        match kind {
            "instruction" => self.ai_open_instruction(),
            "working" => {
                self.ai.run = Some(run);
                self.ai_status();
            }
            "bar" => self.ai_failed("rate", "Claude API is busy (429).".into(), Some(40)),
            "settings" => self.ai_open_settings(),
            "review" | "preview" | "stale" | "closed" => {
                let text = result(&scope.text);
                if kind == "stale" {
                    let mut p = self.docs.prefs.clone();
                    p.ai_review = true;
                    self.docs.prefs = p;
                    self.ai_finish(
                        Run {
                            original: format!("{}x", scope.text),
                            ..run
                        },
                        &text,
                    );
                    if let Some(rv) = self.ai.review() {
                        rv.original = scope.text.clone();
                        let (lines, removed, added) = core_ai::review(&rv.original, &rv.result);
                        (rv.lines, rv.removed, rv.added) = (lines, removed, added);
                    }
                } else {
                    self.docs.prefs.ai_review = true;
                    self.docs.prefs.ai_review_preview = kind == "preview";
                    self.ai_finish(run, &text);
                    if kind == "closed"
                        && let Some(rv) = self.ai.review()
                    {
                        rv.blocked = Some(core_ai::BLOCKED_CLOSED.into());
                    }
                }
            }
            _ => {}
        }
        self.relayout_areas();
        // The preview panes draw on their own thread: drawn once, they are told their size;
        // then the shot waits for both pages.
        if kind == "preview" {
            self.compose();
            let until = Instant::now() + Duration::from_secs(10);
            while Instant::now() < until {
                self.ai_poll_panes();
                let drawn = self
                    .ai
                    .review()
                    .and_then(|rv| rv.panes.as_ref())
                    .is_none_or(|p| p.0.frame.width > 1 && p.1.frame.width > 1);
                if drawn {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    fn ai_models_key_now(&self) -> String {
        let p = &self.docs.prefs;
        crate::ai::models_key(
            p.ai_connection,
            &core_ai::base_url(&p.ai_preset, &p.ai_base_url),
        )
    }
}

#[cfg(test)]
thread_local! {
    /// Tests: the clipboard refuses the text.
    pub(super) static CLIPBOARD_FAILS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Puts `text` on the clipboard; true when it is there.
fn copy_text(text: &str) -> bool {
    #[cfg(test)]
    if CLIPBOARD_FAILS.with(|f| f.get()) {
        return false;
    }
    platform::set_clipboard_text(text)
}
