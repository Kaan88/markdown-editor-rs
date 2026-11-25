//! AI rewriting as the window hosts it: the commands start a run on the selection (or the whole
//! document), the engine's answers arrive as `Wake::Ai`, and the result is shown for review or
//! applied at once, as one undo step. The two panels (instruction, review) are drawn last and,
//! while open, take every key, pointer and wheel event, as the palette does.

use super::*;
use crate::ai::{Ask, Bar, BarAction, Event, Hit as AiHit, Instruction, Panel, Review, Run, Spec};
use crate::keys::KeyName;
use crate::preview_pane::Pane;
use mdedit_core::ai::{self as core_ai, Connection, PromptKind, SavedPrompt};
use mdedit_core::hunks::Hunks;
use mdshared_ui::textfield::TextField;
use serde_json::json;

/// The most text one rewrite takes: models answer slowly and lose the thread on more.
const AI_TEXT_MAX: usize = 200_000;
/// The saved prompts file is read up to this size.
const PROMPTS_BYTES: u64 = 256 * 1024;
/// Lines the review scrolls by for Up and Down.
const REVIEW_STEP: f32 = crate::hunks_ui::LINE_H * 3.0;

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
        let all = match (path, stamp) {
            (Some(p), Some((_, len))) if len <= PROMPTS_BYTES => crate::ai::read_prompts(&p),
            _ => Vec::new(),
        };
        let of = |kind: PromptKind| -> Vec<SavedPrompt> {
            all.iter().filter(|p| p.kind == kind).cloned().collect()
        };
        self.docs.ai_prompts = of(PromptKind::Rewrite);
        self.docs.ai_split_prompts = of(PromptKind::Split);
        self.docs.ai_file_prompts = of(PromptKind::File);
        self.docs.ai_notes_prompts = of(PromptKind::Notes);
    }

    /// Whether the instruction panel is the split's (5.2).
    fn ai_panel_splits(&self) -> bool {
        matches!(&self.ai.panel, Some(Panel::Instruction(i)) if i.split.is_some())
    }

    /// The saved prompts the open panel lists: the split's for a split, the rewrites otherwise.
    fn ai_panel_prompts(&self) -> &[SavedPrompt] {
        if self.ai_panel_splits() {
            &self.docs.ai_split_prompts
        } else {
            &self.docs.ai_prompts
        }
    }

    /// The instruction panel's Save prompt (or Ctrl+S): what is typed goes at the end of the
    /// saved prompts file under a name made of its first words, and the panel's list shows it.
    /// The file is made from its template first when there is none.
    pub(super) fn ai_save_prompt(&mut self) {
        let Some(text) = self
            .ai
            .instruction()
            .map(|i| i.field.text().trim().to_string())
            .filter(|t| !t.is_empty())
        else {
            return;
        };
        let Some(path) = self.ai_prompts_path() else {
            self.docs
                .set_toast("There is no folder for the saved prompts file.");
            return;
        };
        // Open in a tab with changes not saved: writing the file under them would be lost to
        // the tab's next save (audit 1 round 5).
        if self.docs.tabs.iter().any(|d| {
            d.modified()
                && d.path
                    .as_deref()
                    .is_some_and(|p| mdshared_app::paths::same_path(p, &path))
        }) {
            self.docs.set_toast(&format!(
                "Save or close {} first: it has changes that are not saved.",
                core_ai::PROMPTS_FILE
            ));
            return;
        }
        // The file is made from its template first, so a prompt the template has is found.
        if let Err(e) = crate::ai::ensure_prompts_file(&path) {
            self.docs
                .set_toast(&format!("Could not save the prompt: {e}"));
            return;
        }
        let kind = if self.ai_panel_splits() {
            PromptKind::Split
        } else {
            PromptKind::Rewrite
        };
        self.ai_load_prompts();
        if let Some(p) = self
            .ai_panel_prompts()
            .iter()
            .find(|p| p.instruction.trim() == text)
        {
            self.docs
                .set_toast(&format!("Already saved as \u{201c}{}\u{201d}.", p.name));
            return;
        }
        let file = match std::fs::read_to_string(&path) {
            Ok(file) => file,
            Err(e) => {
                self.docs
                    .set_toast(&format!("Could not save the prompt: {e}"));
                return;
            }
        };
        let mut every = self.docs.ai_prompts.clone();
        every.extend(self.docs.ai_split_prompts.iter().cloned());
        let name = core_ai::prompt_name(&text, &every);
        let new = core_ai::append_prompt_of(&file, &name, &text, kind);
        // Written only when the list reads it back: every prompt there was, and this one.
        if new.len() as u64 > PROMPTS_BYTES {
            self.docs.set_toast(&format!(
                "The prompt was not saved: {} would be over {} KB. Remove some prompts first.",
                core_ai::PROMPTS_FILE,
                PROMPTS_BYTES / 1024
            ));
            return;
        }
        let back = core_ai::parse_prompts(&new);
        let before = core_ai::parse_prompts(&file);
        let reads_back = back.len() == before.len() + 1
            && back[..before.len()] == before[..]
            && back
                .last()
                .is_some_and(|p| p.name == name && p.instruction == text && p.kind == kind);
        if !reads_back {
            self.docs.set_toast(&format!(
                "The prompt was not saved: the end of {} would not read as a prompt (a code block left open?). Edit saved prompts to fix it.",
                core_ai::PROMPTS_FILE
            ));
            return;
        }
        let written = mdshared_app::settings::write_atomic(&path, &new);
        match written {
            Ok(()) => {
                self.ai_load_prompts();
                let k = self.ai_panel_prompts().iter().position(|p| p.name == name);
                if let Some(i) = self.ai.instruction() {
                    i.selected = k;
                }
                self.docs.set_toast(&format!(
                    "Saved as \u{201c}{name}\u{201d}. Edit saved prompts to rename it."
                ));
            }
            Err(e) => self
                .docs
                .set_toast(&format!("Could not save the prompt: {e}")),
        }
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

    pub(super) fn ai_conn(&self) -> Connection {
        self.docs.prefs.ai_connection
    }

    /// The connection, model and effort, as the panels name them.
    pub(super) fn ai_via(&self) -> String {
        let p = &self.docs.prefs;
        let c = p.ai_connection;
        let mut s = match c {
            Connection::OpenAi if let Some(x) = p.ai_custom_chosen() => x.label(p.ai_custom),
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

    /// A saved prompt's Edit (the AI menu's, or a row's in the instruction panel): the panel
    /// opens if it is not open, with the prompt in its box to be changed before Rewrite.
    pub(super) fn ai_edit_saved(&mut self, k: usize) {
        if self.ai.instruction().is_none() {
            self.ai_open_instruction();
        }
        self.ai_put_prompt(k);
    }

    /// Puts saved prompt `k` in the instruction panel's box, the caret at its end, and runs
    /// nothing: Rewrite runs it (under the prompt's name while the text is unchanged).
    fn ai_put_prompt(&mut self, k: usize) {
        let Some(p) = self.ai_panel_prompts().get(k).cloned() else {
            return;
        };
        let Some(i) = self.ai.instruction() else {
            return;
        };
        i.selected = Some(k);
        i.field.set_text(p.instruction.trim());
        self.caret_reset();
        self.ai.tick += 1;
        self.redraw();
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
                self.ai_drop_kept();
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
        self.open_settings_at(super::modal::AI_SERVICE);
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
                    if let Some(t) = self.task.as_mut()
                        && t.asking.as_ref().is_some_and(|a| a.request == id)
                    {
                        if let Some(a) = t.asking.as_mut() {
                            a.chars = chars;
                        }
                        t.tick += 1;
                    } else if let Some(r) = self.ai.run.as_mut().filter(|r| r.id == id) {
                        r.chars = chars;
                        self.ai_status();
                    } else if let Some(r) = self.ai.split.as_mut().filter(|r| r.id == id) {
                        r.chars = chars;
                        self.split_status();
                    } else if let Some(r) = self
                        .note
                        .as_mut()
                        .and_then(|c| c.asking.as_mut())
                        .filter(|r| r.id == id)
                    {
                        r.chars = chars;
                    }
                }
                Event::Done { id, text } => {
                    let ask = self.ai.answered(id, true);
                    if self
                        .task
                        .as_ref()
                        .is_some_and(|t| t.asking.as_ref().is_some_and(|a| a.request == id))
                    {
                        self.task_done(&text);
                    } else if self.ai.split.as_ref().is_some_and(|r| r.id == id) {
                        self.split_done(&text);
                    } else if self.ai.run.as_ref().is_some_and(|r| r.id == id) {
                        let run = self.ai.run.take().expect("checked");
                        self.docs.ai_status = None;
                        self.ai_finish(run, &text);
                    } else if self
                        .note
                        .as_ref()
                        .and_then(|c| c.asking.as_ref())
                        .is_some_and(|r| r.id == id)
                    {
                        self.note_done(&text);
                    } else {
                        match ask {
                            Some(Ask::SortScheme) => self.sort_scheme_done(&text),
                            Some(Ask::SortFiles(_)) => self.sort_batch_done(id, &text),
                            Some(Ask::Look(_)) => self.notes_look_done(id, &text),
                            Some(Ask::Arrange) => self.notes_arrange_done(id, &text),
                            _ => {}
                        }
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
                    Some(Ask::Split) => {
                        if self.ai.split.as_ref().is_some_and(|r| r.id == id) {
                            self.split_failed(&kind, &message);
                        }
                    }
                    Some(Ask::SortScheme | Ask::SortFiles(_)) => {
                        self.sort_request_failed(id, &kind, &message)
                    }
                    Some(Ask::Task) => {
                        if self
                            .task
                            .as_ref()
                            .and_then(|t| t.asking.as_ref())
                            .is_some_and(|a| a.request == id)
                        {
                            self.task_failed(&kind, &message);
                        }
                    }
                    Some(Ask::Note) => {
                        if self
                            .note
                            .as_ref()
                            .and_then(|c| c.asking.as_ref())
                            .is_some_and(|r| r.id == id)
                        {
                            self.note_failed(&kind, &message);
                        }
                    }
                    Some(Ask::Prepare) => self.notes_prepare_failed(id, &kind, &message),
                    Some(Ask::Look(_)) => self.notes_look_failed(id, &kind, &message, retry_after),
                    Some(Ask::Arrange) => self.notes_arrange_failed(id, &kind, &message),
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
                Event::Picture { id, index, info } => {
                    if self.ai.answered(id, false) == Some(Ask::Prepare) {
                        self.notes_picture(id, index, info);
                    }
                }
                Event::Prepared { id } => {
                    if self.ai.answered(id, true) == Some(Ask::Prepare) {
                        self.notes_prepared(id);
                    }
                }
                Event::Sent {
                    id,
                    bytes,
                    text_bytes,
                    ..
                } => {
                    if matches!(self.ai.answered(id, false), Some(Ask::Look(_))) {
                        self.notes_sent(id, bytes, text_bytes);
                    }
                }
                Event::Ended(why) => {
                    if self.task.as_ref().is_some_and(|t| t.asking.is_some()) {
                        self.task_failed("ended", &why);
                    }
                    self.notes_engine_ended(&why);
                    if self.ai.split.is_some() {
                        self.split_failed("ended", &why);
                    }
                    if self.ai.sort.is_some() {
                        self.sort_engine_ended(&why);
                    }
                    let note_out = self.note.as_ref().and_then(|c| c.asking.as_ref()).is_some();
                    if note_out {
                        self.note_failed("ended", &why);
                    }
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
        self.ai_drop_kept();
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
            let to_markdown = self.ai_converts(run.doc, run.start, run.end, &run.spec);
            if self.ai_replace(run.doc, run.start, run.end, &result) {
                if let Some(before) = to_markdown {
                    self.ai_made_markdown(run.doc, Some(before));
                }
                self.docs.set_toast(&format!(
                    "{} done. Undo brings the old text back.",
                    run.spec.title
                ));
                return;
            }
            // Not taken: the result is shown, so it can still be copied.
            blocked = Some(core_ai::BLOCKED_READ_ONLY.into());
        }
        let mut hunks = Hunks::new(&run.original, &result);
        // `--ai-demo`: changes left unticked, before the preview pages are first made.
        for i in self.ai.demo_off.take().unwrap_or_default() {
            hunks.set_ticked(i, false);
        }
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
            hunks,
            list: Default::default(),
            blocked,
            preview,
            panes: None,
            sent: 0,
            due: None,
            grab: None,
            base_dir,
            file: None,
        }));
        if preview {
            self.ai_open_panes();
        }
        self.redraw();
    }

    /// Whether putting the result of `spec` in place of `start..end` of document `doc` makes a
    /// document that is not Markdown Markdown: Format as Markdown or Fix Markdown, over the
    /// whole text (a part formatted leaves the rest as it was). When it does: the undo state and
    /// the language from before, for an undo to take the switch back.
    fn ai_converts(
        &self,
        doc: DocId,
        start: usize,
        end: usize,
        spec: &Spec,
    ) -> Option<(u64, Option<mdedit_highlight::Language>)> {
        let d = self.docs.index_of_id(doc).map(|i| &self.docs.tabs[i])?;
        let r = d.ready()?;
        let whole = start == 0 && end == r.buffer.len_bytes();
        (whole
            && !d.is_markdown()
            && (spec.instruction == core_ai::FORMAT || spec.instruction == core_ai::FIX))
            .then(|| (r.history.state(), r.language))
    }

    /// A document the AI made Markdown is Markdown from now on (its preview, its tools, its
    /// colours), as if chosen in the language list; a file not named so is offered a `.md` name
    /// in a bar. `before`: the undo state and language from before the AI's change; undone back
    /// to that state, the switch and the bar go (`Docs::drop_undone_switches`).
    pub(super) fn ai_made_markdown(
        &mut self,
        doc: DocId,
        before: Option<(u64, Option<mdedit_highlight::Language>)>,
    ) {
        let Some(i) = self.docs.index_of_id(doc) else {
            return;
        };
        let dark = self.dark();
        self.docs
            .set_language(i, crate::languages::markdown(), self.service.as_ref(), dark);
        if let Some(r) = self.docs.tabs[i].ready_mut() {
            r.ai_switch = before;
        }
        if self.docs.tabs[i].path.is_some() {
            self.docs.rename_offer = Some(doc);
        }
        self.relayout_areas();
        self.full = true;
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
    pub(super) fn close_for_ai_panel(&mut self) {
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
            split: None,
            selected: None,
            scope: scope.label,
            via: self.ai_via(),
            scroll: 0.0,
        }));
        self.caret_reset();
        self.ai.tick += 1;
        self.redraw();
    }

    /// Escape on a review: it closes, and its result is kept to be shown again (a stray click
    /// beside it is ignored for the same reason). Discard is the button that throws it away.
    fn ai_keep_review(&mut self) {
        if let Some(Panel::Review(rv)) = self.ai.panel.take() {
            self.ai.kept = Some(rv);
            self.docs.ai_kept = true;
        }
        self.ai_close_panel();
    }

    /// The kept result is dropped: another run began or ended with a result of its own.
    fn ai_drop_kept(&mut self) {
        self.ai.kept = None;
        self.docs.ai_kept = false;
    }

    /// "AI: Show the last result": the review Escape closed, as it was.
    pub(super) fn ai_show_kept(&mut self) {
        let Some(rv) = self.ai.kept.take() else {
            return;
        };
        self.docs.ai_kept = false;
        self.close_for_ai_panel();
        self.ai.panel = Some(Panel::Review(rv));
        self.ai_review_sync();
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
        // The split panel's plan screen comes back.
        self.split_panel_closed();
        self.redraw();
    }

    /// Run what the instruction panel holds: the typed text, or the prompt picked.
    fn ai_submit(&mut self) {
        let Some(i) = self.ai.instruction() else {
            return;
        };
        let typed = i.field.text().trim().to_string();
        let selected = i.selected;
        let again = i.spec.clone().filter(|s| typed == s.instruction.trim());
        let splits = self.ai_panel_splits();
        let picked = selected
            .and_then(|k| self.ai_panel_prompts().get(k))
            .cloned();
        let spec = match picked {
            _ if let Some(s) = again => s,
            Some(p) if typed.is_empty() || typed == p.instruction.trim() => Spec {
                title: p.name,
                instruction: p.instruction,
            },
            _ if !typed.is_empty() => Spec {
                title: if splits { "Split" } else { "Rewrite" }.into(),
                instruction: typed,
            },
            _ => return,
        };
        // A split sends the plan's outline, not a scope of the editor.
        if splits {
            self.split_start(spec);
            return;
        }
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
        let prompts = self.ai_panel_prompts().to_vec();
        let n = prompts.len();
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
        if rv.file.is_some() {
            return;
        }
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
            AiHit::Discard => {
                if self.ai.review().is_some_and(|rv| rv.file.is_some()) {
                    self.task_review_discard();
                } else {
                    self.ai_close_panel()
                }
            }
            AiHit::Copy => {
                let Some((text, file)) = self
                    .ai
                    .review()
                    .map(|rv| (rv.hunks.compose(), rv.file.clone()))
                else {
                    return;
                };
                // The review may be the only place the result is (its document closed): it
                // closes only once the result is on the clipboard (audit 1 round 3, #3).
                if copy_text(&text) {
                    self.docs.confirm_toast("Copied the result.");
                    if let Some(fr) = file {
                        // The file is left as it was, and the run goes on with the next
                        // (audit 4, B3).
                        self.ai.panel = None;
                        self.ai.tick += 1;
                        self.task_review_left(&fr);
                    } else {
                        self.ai_close_panel();
                    }
                } else {
                    self.docs
                        .set_toast("Could not copy to the clipboard. The result is still here.");
                    self.redraw();
                }
            }
            AiHit::Apply => {
                if self.ai.review().is_some_and(|rv| rv.file.is_some()) {
                    self.task_review_apply();
                    return;
                }
                let Some(rv) = self.ai.review() else { return };
                if rv.blocked.is_some() {
                    return;
                }
                // With nothing ticked there is nothing to put in.
                if rv.hunks.ticked() == 0 {
                    self.docs.set_toast("Nothing is ticked.");
                    self.redraw();
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
                    let to_markdown = self.ai_converts(doc, start, end, &rv.spec);
                    if self.ai_replace(doc, start, end, &rv.hunks.compose()) {
                        self.ai_close_panel();
                        if let Some(before) = to_markdown {
                            self.ai_made_markdown(doc, Some(before));
                        }
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
                if rv.file.is_some() {
                    self.task_review_again();
                    return;
                }
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
            AiHit::Prev => self.ai_review_step(false),
            AiHit::Next => self.ai_review_step(true),
            AiHit::Mark(i) => {
                if let Some(rv) = self.ai.review() {
                    rv.list.go_to(i);
                }
                self.ai.tick += 1;
                self.redraw();
            }
            AiHit::TickAll | AiHit::UntickAll => {
                let Some(rv) = self.ai.review() else { return };
                rv.hunks.set_all(hit == AiHit::TickAll);
                self.ai_review_ticked();
            }
            AiHit::List(crate::hunks_ui::Hit::Tick(i)) => {
                let Some(rv) = self.ai.review() else { return };
                rv.hunks.toggle(i);
                rv.list.current = i;
                self.ai_review_ticked();
            }
            AiHit::List(crate::hunks_ui::Hit::Change(i)) => {
                let Some(rv) = self.ai.review() else { return };
                rv.list.current = i;
                self.ai.tick += 1;
                self.redraw();
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
    pub(super) fn ai_open_panes(&mut self) {
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
        rv.sent = 1;
        after.set_text(rv.doc, (0, rv.sent), base, || rv.hunks.compose());
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
        // A tick sent while the page was still drawing the last one waits for its answer.
        if panes.1.held {
            let (doc, sent, base) = (rv.doc, rv.sent, rv.base_dir.clone());
            let text = rv.hunks.compose();
            if let Some(panes) = rv.panes.as_mut() {
                panes.1.set_text(doc, (0, sent), base, || text);
            }
        }
        if a || b {
            self.ai.tick += 1;
            self.redraw();
        }
    }

    /// F7 and Shift+F7, and the bar's arrows: the next or the previous change becomes the current
    /// one and is scrolled into view.
    pub(super) fn ai_review_step(&mut self, forward: bool) {
        let Some(rv) = self.ai.review() else { return };
        let n = rv.hunks.len();
        if n == 0 {
            return;
        }
        let cur = rv.list.current.min(n - 1);
        let next = if forward {
            (cur + 1).min(n - 1)
        } else {
            cur.saturating_sub(1)
        };
        if rv.preview {
            // The preview pages have no list to scroll and the viewer gives no block rows
            // (D-108): stepping here would change the count and move nothing, so it does not.
            return;
        }
        rv.list.go_to(next);
        self.ai.tick += 1;
        self.redraw();
    }

    /// Space: the current change is taken, or left.
    fn ai_review_toggle_current(&mut self) {
        let Some(rv) = self.ai.review() else { return };
        let i = rv.list.current;
        if i < rv.hunks.len() {
            rv.hunks.toggle(i);
            self.ai_review_ticked();
        }
    }

    /// A tick changed: the footer and the list redraw, and the AFTER page is made again 150 ms
    /// after the last one.
    fn ai_review_ticked(&mut self) {
        if let Some(rv) = self.ai.review()
            && rv.panes.is_some()
        {
            rv.due = Some(Instant::now() + crate::compare::DEBOUNCE);
        }
        self.ai.tick += 1;
        self.redraw();
    }

    /// The AFTER page is sent the text with the ticked changes, when the delay has passed.
    /// Returns the time it is next due, if it is not yet.
    pub(super) fn ai_review_due(&mut self, now: Instant) -> Option<Instant> {
        let rv = self.ai.review()?;
        let due = rv.due?;
        if now < due {
            return Some(due);
        }
        rv.due = None;
        rv.sent += 1;
        let (doc, sent, base) = (rv.doc, rv.sent, rv.base_dir.clone());
        let text = rv.hunks.compose();
        if let Some(panes) = rv.panes.as_mut() {
            panes.1.set_text(doc, (0, sent), base, || text);
        }
        self.ai.tick += 1;
        self.wants_frame = true;
        None
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
                KeyName::Char('s') if mods.ctrl => self.ai_save_prompt(),
                KeyName::Down => self.ai_pick(true),
                KeyName::Up => self.ai_pick(false),
                _ => {
                    if i.field.handle_key(&name, mods, typed, true) == Some(true) {
                        i.selected = None;
                    }
                }
            },
            Some(Panel::Review(rv)) => match name {
                KeyName::Escape => self.ai_keep_review(),
                KeyName::Enter => {
                    let hit = if rv.blocked.is_some() {
                        AiHit::Copy
                    } else {
                        AiHit::Apply
                    };
                    self.ai_review_action(hit);
                }
                KeyName::Char('c') if mods.ctrl => self.ai_review_action(AiHit::Copy),
                KeyName::F(7) => self.ai_review_step(!mods.shift),
                KeyName::Space => self.ai_review_toggle_current(),
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
            rv.list.scroll_by(dy);
        }
        self.ai.tick += 1;
        self.redraw();
    }

    /// A drag of the list's scrollbar thumb: the list goes where the thumb is.
    fn ai_drag_thumb(&mut self, y: f32) -> bool {
        let Some(rv) = self.ai.review() else {
            return false;
        };
        let Some(grab) = rv.grab else { return false };
        let area = rv.list.area;
        if let Some((track, thumb)) = crate::hunks_ui::bar(area, &rv.list) {
            let to = mdshared_ui::scroll::offset_for_thumb(
                track,
                thumb.h,
                y - grab,
                rv.list.content,
                rv.list.view,
            );
            rv.list.scroll = to.clamp(0.0, rv.list.max_scroll());
        }
        self.ai.tick += 1;
        self.redraw();
        true
    }

    pub(super) fn ai_pointer(&mut self, x: f32, y: f32) {
        if self.ai_drag_thumb(y) {
            return;
        }
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
        // The list's scrollbar: a press on the thumb takes it, one on the track brings the thumb
        // there and takes it.
        if hit == Some(AiHit::List(crate::hunks_ui::Hit::Track))
            && let Some(rv) = self.ai.review()
            && let Some((_, thumb)) = crate::hunks_ui::bar(rv.list.area, &rv.list)
        {
            rv.grab = Some(if thumb.contains(x, y) {
                y - thumb.y
            } else {
                thumb.h / 2.0
            });
            self.ai_drag_thumb(y);
            self.ai.pressed = None;
        }
        self.ai.tick += 1;
        self.redraw();
    }

    pub(super) fn ai_release(&mut self) {
        let (x, y) = self.ui.mouse;
        if let Some(rv) = self.ai.review() {
            rv.grab = None;
        }
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
                let prompts = self.ai_panel_prompts().to_vec();
                if let Some(i) = self.ai.instruction()
                    && let Some(p) = prompts.get(k)
                {
                    i.selected = Some(k);
                    i.field.set_text(p.instruction.trim());
                }
                self.ai_submit();
            }
            AiHit::EditPrompt(k) => self.ai_put_prompt(k),
            AiHit::Cancel => self.ai_close_panel(),
            AiHit::Rewrite => self.ai_submit(),
            AiHit::SavePrompt => self.ai_save_prompt(),
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
        // `settings-error`: the server did not answer the list (Settings' hardest page).
        let models = if kind == "settings-error" {
            Err("Could not list the models: connection refused (os error 10061). Is the server                  running?"
                .into())
        } else {
            Ok(vec![
                model("claude-opus-5-5", "Claude Opus 5.5"),
                model("claude-sonnet-5-5", "Claude Sonnet 5.5"),
                model("claude-haiku-5-5", "Claude Haiku 5.5"),
            ])
        };
        self.ai.models.insert(key, models);
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
            self.docs.prefs.ai_url(),
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
                // The strip partway across, as it is most of the time.
                self.docs.ai_phase = 14;
            }
            "bar" => self.ai_failed("rate", "Claude API is busy (429).".into(), Some(40)),
            "settings" | "settings-error" => self.ai_open_settings(),
            // A rewrite of the open document by the file `<name>-after.md` beside it, with some
            // changes unticked: the shots of the review's list and its preview (5.5).
            "review-hunks"
            | "review-partial"
            | "review-nothing"
            | "review-long"
            | "review-preview-partial"
            | "review-closed" => {
                let path = self
                    .docs
                    .index_of_id(scope.doc)
                    .and_then(|i| self.docs.tabs[i].path.clone());
                let after = path
                    .map(|p| p.with_file_name(format!("{}-after.md", file_stem(&p))))
                    .and_then(|p| std::fs::read_to_string(p).ok());
                let Some(after) = after else { return };
                self.ai.demo_off = match kind {
                    "review-partial" | "review-preview-partial" => Some(vec![1, 3, 4, 6]),
                    "review-nothing" => Some((0..100).collect()),
                    _ => None,
                };
                self.docs.prefs.ai_review = true;
                self.docs.prefs.ai_review_preview = kind == "review-preview-partial";
                self.ai_finish(run, &after);
                if kind == "review-closed"
                    && let Some(rv) = self.ai.review()
                {
                    rv.blocked = Some(core_ai::BLOCKED_CLOSED.into());
                }
                if let Some(rv) = self.ai.review() {
                    // The change the keys are on: far down a long list; the one with a long
                    // line, to show it wrapped; or the third.
                    let wide = (0..rv.hunks.len()).find(|&i| {
                        rv.hunks
                            .change(i)
                            .is_some_and(|c| c.a.clone().any(|l| rv.hunks.old_line(l).len() > 200))
                    });
                    rv.list.go_to(match kind {
                        "review-long" => 17,
                        "review-hunks" => wide.unwrap_or(2),
                        _ => 2,
                    });
                }
            }
            "review" | "preview" | "stale" | "closed" | "result-kept" => {
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
                        rv.hunks = Hunks::new(&rv.original, &rv.result);
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
                    if kind == "result-kept" {
                        self.ai_keep_review();
                    }
                }
            }
            _ => {}
        }
        self.relayout_areas();
        // The preview panes draw on their own thread: drawn once, they are told their size;
        // then the shot waits for both pages.
        if kind == "preview" || kind == "review-preview-partial" {
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
            &core_ai::base_url(&p.ai_preset, p.ai_url()),
        )
    }
}

#[cfg(test)]
thread_local! {
    /// Tests: what was last copied (the clipboard itself is never touched).
    pub(super) static COPIED: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    /// Tests: the clipboard refuses the text.
    pub(super) static CLIPBOARD_FAILS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Puts `text` on the clipboard; true when it is there.
#[cfg(test)]
fn copy_text(text: &str) -> bool {
    if CLIPBOARD_FAILS.with(|f| f.get()) {
        return false;
    }
    COPIED.with(|c| *c.borrow_mut() = text.to_string());
    true
}

#[cfg(not(test))]
fn copy_text(text: &str) -> bool {
    platform::set_clipboard_text(text)
}

/// A path's file name without its extension, "" when it has none.
fn file_stem(p: &Path) -> String {
    p.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}
