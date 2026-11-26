//! Write notes in the task window (pictures plan, phase V3): the pictures measured by the
//! engine as they are added, the look requests four pictures at a time with as many out at once
//! as the connection takes, and the arrange request. The state is `notes_task.rs`; the window's
//! other tasks are `task.rs`.

use super::*;
use crate::ai::{Ask, PictureInfo};
use crate::notes_task::{self, Measure, Run};
use crate::task_ui::{FileKind, TaskKind};
use mdedit_core::ai::Connection;
use mdedit_core::notes_ai::{self, HowMany};
use serde_json::json;

/// Why a connection is offered no pictures (D-221): Codex has not read one yet.
pub(super) const CODEX_NO_PICTURES: &str = "Codex cannot be sent pictures yet: choose the \
     Claude API, Claude Code or an OpenAI-compatible connection in Settings \u{203A} AI.";

impl App {
    /// Why this connection cannot be sent pictures now, if it cannot.
    pub(super) fn notes_pictures_refused(&self) -> Option<String> {
        if self.ai_conn() == Connection::Codex {
            return Some(CODEX_NO_PICTURES.to_string());
        }
        self.ai.pictures_refused().map(str::to_string)
    }

    /// Measures the window's pictures that are not measured yet, or all of them (`again`, when
    /// what is sent changed): one `prepare` request, nothing sent anywhere. Only on Write notes
    /// with the AI on: with it off nothing goes, so nothing needs measuring.
    pub(super) fn notes_prepare(&mut self, again: bool) {
        let refused = self.notes_pictures_refused();
        let on = self.ai_conn() != Connection::Off;
        let Some(s) = self.task.as_mut() else { return };
        s.pictures_refused = refused;
        if s.task != TaskKind::Notes || !on || s.pictures_refused.is_some() {
            return;
        }
        if again {
            let out: Vec<u64> = s.prepare.drain(..).map(|(id, _)| id).collect();
            for id in out {
                self.ai.cancel_id(id);
            }
        }
        let Some(s) = self.task.as_mut() else { return };
        let measuring: Vec<usize> = s.prepare.iter().flat_map(|(_, ids)| ids.clone()).collect();
        let ids: Vec<usize> = (0..s.files.len())
            .filter(|&i| {
                let f = &s.files[i];
                f.kind == FileKind::Picture
                    && f.why.is_none()
                    && !f.done
                    && (again || f.measure == Measure::Waiting)
                    && !measuring.contains(&i)
            })
            .collect();
        if ids.is_empty() {
            return;
        }
        for &i in &ids {
            s.files[i].measure = Measure::Waiting;
        }
        let images: Vec<_> = ids
            .iter()
            .map(|&i| json!({"path": s.files[i].path.to_string_lossy()}))
            .collect();
        let req = json!({
            "op": "prepare",
            "images": images,
            "long_side": notes_task::long_side(&s.notes),
            "strip": s.notes.strip,
        });
        let wake = self.docs.waker();
        match self.ai.ask(&wake, Ask::Prepare, req) {
            Ok(id) => {
                if let Some(s) = self.task.as_mut() {
                    s.prepare.push((id, ids));
                    s.tick += 1;
                }
            }
            Err(why) => {
                if let Some(s) = self.task.as_mut() {
                    for &i in &ids {
                        s.files[i].measure = Measure::Refused(why.clone());
                    }
                    s.tick += 1;
                }
            }
        }
        self.redraw();
    }

    /// What the engine said one picture of a `prepare` is sent as.
    pub(super) fn notes_picture(
        &mut self,
        id: u64,
        index: usize,
        info: Result<PictureInfo, String>,
    ) {
        let Some(s) = self.task.as_mut() else { return };
        let Some(i) = s
            .prepare
            .iter()
            .find(|(p, _)| *p == id)
            .and_then(|(_, ids)| ids.get(index).copied())
        else {
            return;
        };
        let Some(f) = s.files.get_mut(i) else { return };
        match info {
            Ok(mut info) => {
                if let Some(png) = info.thumb.take() {
                    f.thumb = mdshared_text::decode_png_rgba(&png)
                        .map(|(rgba, w, h)| notes_task::small(&rgba, w, h, notes_task::THUMB_PX));
                }
                f.measure = Measure::Ready(info);
            }
            Err(why) => f.measure = Measure::Refused(why),
        }
        s.tick += 1;
        self.redraw();
    }

    /// Every picture of a `prepare` has been told: one the engine said nothing of is refused.
    pub(super) fn notes_prepared(&mut self, id: u64) {
        let Some(s) = self.task.as_mut() else { return };
        let Some(at) = s.prepare.iter().position(|(p, _)| *p == id) else {
            return;
        };
        let (_, ids) = s.prepare.remove(at);
        for i in ids {
            if let Some(f) = s.files.get_mut(i).filter(|f| f.measure == Measure::Waiting) {
                f.measure = Measure::Refused("the engine said nothing of it".to_string());
            }
        }
        s.tick += 1;
        self.redraw();
    }

    /// A `prepare` failed: an engine older than mdedit refuses the pictures, anything else
    /// refuses the ones it was measuring, with why.
    pub(super) fn notes_prepare_failed(&mut self, id: u64, kind: &str, message: &str) {
        let old = self.ai.pictures_refused();
        let Some(s) = self.task.as_mut() else { return };
        let Some(at) = s.prepare.iter().position(|(p, _)| *p == id) else {
            return;
        };
        let (_, ids) = s.prepare.remove(at);
        if kind == "cancelled" {
            return;
        }
        if let Some(why) = old {
            s.pictures_refused = Some(why.to_string());
        }
        for i in ids {
            if let Some(f) = s.files.get_mut(i).filter(|f| f.measure == Measure::Waiting) {
                f.measure = Measure::Refused(message.to_string());
            }
        }
        s.tick += 1;
        self.redraw();
    }

    /// "Send 5 pictures and 1 file": the looks, as many at once as the connection takes; a
    /// picture looked at already in this window is not sent again. With the AI off, or with no
    /// picture to look at, straight to the notes.
    pub(super) fn notes_start(&mut self) {
        let on = self.ai_conn() != Connection::Off;
        let parallel = self.docs.prefs.ai_parallel(self.ai_conn()).max(1);
        let Some(s) = self.task.as_mut() else { return };
        if s.busy() {
            return;
        }
        // A plan kept by Esc on its card is opened again (D-053).
        if s.notes_run.as_ref().is_some_and(|r| r.arranged.is_some()) {
            self.notes_plan_show();
            return;
        }
        s.ai_on = on;
        if let Some(why) = s.notes_blocked() {
            s.blocked = Some(why);
            s.tick += 1;
            self.redraw();
            return;
        }
        s.blocked = None;
        let (pictures, texts) = s.notes_parts();
        let instruction = s.instruction.text().trim().to_string();
        let text_out = s.notes.text_out;
        let kept: Vec<notes_ai::Look> = s
            .notes_run
            .take()
            .filter(|r| r.keeps(&instruction, text_out))
            .map(|r| r.looks)
            .unwrap_or_default()
            .into_iter()
            .filter(|l| pictures.contains(&l.id))
            .collect();
        let to_look: Vec<usize> = pictures
            .iter()
            .copied()
            .filter(|i| on && !kept.iter().any(|l| l.id == *i))
            .collect();
        let batches = notes_task::batches(&to_look);
        let answered = Vec::new();
        s.notes_run = Some(Run {
            batches,
            looks: kept,
            parallel,
            texts,
            answered,
            instruction,
            text_out,
            ..Run::default()
        });
        s.tick += 1;
        self.notes_send_more();
    }

    /// Sends look requests while fewer than the connection takes are out; when every one is
    /// answered, the arrange.
    fn notes_send_more(&mut self) {
        loop {
            let Some(s) = self.task.as_ref() else { return };
            let Some(run) = s.notes_run.as_ref() else {
                return;
            };
            if run.stopped || run.in_flight.len() >= run.parallel {
                break;
            }
            let b = run.next;
            let Some(batch) = run.batches.get(b).cloned() else {
                break;
            };
            let instruction = s.instruction.text().trim().to_string();
            let sent: Vec<notes_ai::Sent> = batch
                .iter()
                .map(|&i| notes_ai::Sent {
                    id: i,
                    name: s.files[i].name.clone(),
                })
                .collect();
            let images: Vec<_> = batch
                .iter()
                .map(|&i| {
                    let f = &s.files[i];
                    let sha = match &f.measure {
                        Measure::Ready(info) => info.sha256.clone(),
                        _ => String::new(),
                    };
                    json!({"path": f.path.to_string_lossy(), "sha256": sha})
                })
                .collect();
            let prompt = notes_ai::look_prompt(&instruction, &sent, s.notes.text_out);
            let p = &self.docs.prefs;
            let c = p.ai_connection;
            let req = json!({
                "op": "run",
                "conn": crate::ai::conn_json(c, p),
                "model": p.ai_model(c),
                "effort": p.ai_effort(c),
                "system": notes_ai::LOOK_SYSTEM,
                "prompt": prompt,
                "images": images,
                "long_side": notes_task::long_side(&s.notes),
                "strip": s.notes.strip,
            });
            let wake = self.docs.waker();
            match self.ai.ask(&wake, Ask::Look(b), req) {
                Ok(id) => {
                    self.task_sent.requests += 1;
                    if let Some(run) = self.task.as_mut().and_then(|s| s.notes_run.as_mut()) {
                        run.in_flight.push((id, b));
                        run.next += 1;
                        run.requests += 1;
                    }
                }
                Err(why) => {
                    if let Some(run) = self.task.as_mut().and_then(|s| s.notes_run.as_mut()) {
                        run.failed.push((b, why));
                        run.next += 1;
                        run.stopped = true;
                    }
                    break;
                }
            }
        }
        if let Some(s) = self.task.as_mut() {
            s.tick += 1;
        }
        self.notes_after_looks();
        self.redraw();
    }

    /// The looks are over: with none failed, the notes are arranged (or made here, one per
    /// picture); with some failed, the window offers to try them again or go on without them.
    fn notes_after_looks(&mut self) {
        let Some(s) = self.task.as_ref() else { return };
        let Some(run) = s.notes_run.as_ref() else {
            return;
        };
        if !run.looks_ended()
            || !run.failed.is_empty()
            || run.fault.is_some()
            || run.arrange.is_some()
            || run.arranged.is_some()
            || run.arrange_failed.is_some()
            || (run.stopped && run.next < run.batches.len())
        {
            return;
        }
        let (pictures, _) = s.notes_parts();
        // With the AI on, a note is made only of a picture that was looked at: one the owner
        // went on without is left out.
        let looked: Vec<usize> = if s.ai_on {
            pictures
                .iter()
                .copied()
                .filter(|&i| run.look(i).is_some())
                .collect()
        } else {
            pictures
        };
        // With the AI off every note is made here; One per picture needs no second request
        // unless there are only text files to arrange.
        let how = s.notes.how_many;
        let no_text = run.texts.is_empty();
        if !s.ai_on || (how == HowMany::PerPicture && !looked.is_empty()) {
            self.notes_made_here(&looked, how);
            return;
        }
        // Nothing looked at and no text: an arrange request could not work, and none is sent.
        if looked.is_empty() && no_text {
            self.task_say("No picture was looked at, so there is nothing to arrange: Send again.");
            return;
        }
        self.notes_arrange();
    }

    /// The notes made here, with no request: one per picture, or (with the AI off) all in one.
    fn notes_made_here(&mut self, pictures: &[usize], how: HowMany) {
        let Some(s) = self.task.as_mut() else { return };
        let names: Vec<(usize, String)> = s
            .files
            .iter()
            .enumerate()
            .map(|(i, f)| (i, f.name.clone()))
            .collect();
        let name = |i: usize| {
            names
                .iter()
                .find(|(k, _)| *k == i)
                .map(|(_, n)| n.clone())
                .unwrap_or_default()
        };
        let Some(run) = s.notes_run.as_mut() else {
            return;
        };
        let notes = notes_ai::per_picture(pictures, &run.looks, &name, s.notes.describe);
        let mut arranged = notes_ai::Arranged {
            notes,
            ..notes_ai::Arranged::default()
        };
        if how == HowMany::One {
            arranged = notes_ai::joined(arranged);
        }
        run.arranged = Some(arranged);
        s.tick += 1;
        self.notes_planned();
    }

    /// The sections of the group the notes go to, each a path joined with " › ": the arrange
    /// request is given them, and "Show what is sent" shows them.
    pub(super) fn notes_headings(&self) -> Vec<String> {
        self.task
            .as_ref()
            .and_then(|s| s.group())
            .and_then(|g| {
                self.docs
                    .library
                    .groups
                    .iter()
                    .find(|lg| mdshared_app::paths::same_path(&lg.file, &g.file))
            })
            .map(|lg| {
                lg.sections
                    .iter()
                    .map(|path| path.join(" \u{203A} "))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The arrange request: what the looks said, the text files whole, the group's sections.
    fn notes_arrange(&mut self) {
        let Some(s) = self.task.as_ref() else { return };
        let Some(run) = s.notes_run.as_ref() else {
            return;
        };
        let instruction = run
            .arrange_instruction
            .clone()
            .unwrap_or_else(|| s.instruction.text().trim().to_string());
        let mut looks = run.looks.clone();
        looks.sort_by_key(|l| l.id);
        let files: Vec<notes_ai::TextFile> = run
            .texts
            .iter()
            .filter_map(|&i| {
                let f = s.files.get(i)?;
                Some(notes_ai::TextFile {
                    name: f.name.clone(),
                    text: f.text.clone()?,
                })
            })
            .collect();
        let headings = self.notes_headings();
        let prompt =
            notes_ai::arrange_prompt(&instruction, s.notes.how_many, &looks, &files, &headings);
        let p = &self.docs.prefs;
        let c = p.ai_connection;
        let req = json!({
            "op": "run",
            "conn": crate::ai::conn_json(c, p),
            "model": p.ai_model(c),
            "effort": p.ai_effort(c),
            "system": notes_ai::ARRANGE_SYSTEM,
            "prompt": prompt,
        });
        let wake = self.docs.waker();
        let bytes = (notes_ai::ARRANGE_SYSTEM.len() + prompt.len()) as u64;
        match self.ai.ask(&wake, Ask::Arrange, req) {
            Ok(id) => {
                self.task_sent.bytes += bytes;
                self.task_sent.requests += 1;
                if let Some(run) = self.task.as_mut().and_then(|s| s.notes_run.as_mut()) {
                    run.arrange = Some(id);
                    run.requests += 1;
                    run.bytes += bytes;
                }
            }
            Err(why) => self.notes_arrange_lost(Some(why)),
        }
        if let Some(s) = self.task.as_mut() {
            s.tick += 1;
        }
        self.redraw();
    }

    /// What one picture request sent, for the record: a `sent` for a request this window no
    /// longer has (stopped as it left) is not counted (Fable's review of V1 round 2, 7).
    pub(super) fn notes_sent(&mut self, id: u64, bytes: u64, text_bytes: u64) {
        let Some(run) = self.task.as_mut().and_then(|s| s.notes_run.as_mut()) else {
            return;
        };
        if !run.in_flight.iter().any(|(r, _)| *r == id) {
            return;
        }
        run.bytes += bytes + text_bytes;
        self.task_sent.bytes += bytes + text_bytes;
    }

    /// A look answered: each picture's row shows what the model said of it.
    pub(super) fn notes_look_done(&mut self, id: u64, text: &str) {
        let Some(s) = self.task.as_mut() else { return };
        let Some(run) = s.notes_run.as_mut() else {
            return;
        };
        let Some(at) = run.in_flight.iter().position(|(r, _)| *r == id) else {
            return;
        };
        let (_, b) = run.in_flight.remove(at);
        run.answers += 1;
        let ids = run.batches.get(b).cloned().unwrap_or_default();
        match notes_ai::read_look(text, &ids) {
            Ok(looks) => {
                run.looks.retain(|l| !ids.contains(&l.id));
                let (said, missed): (Vec<usize>, Vec<usize>) =
                    ids.iter().partition(|&&i| looks.iter().any(|l| l.id == i));
                run.looks.extend(looks);
                let said_of =
                    |names: &[String]| format!("The answer said nothing of {}.", names.join(", "));
                if missed.is_empty() {
                    run.answered.push(b);
                } else if said.is_empty() {
                    // Nothing was said of any: the batch failed whole.
                    let names: Vec<String> = ids
                        .iter()
                        .filter_map(|&i| s.files.get(i).map(|f| f.name.clone()))
                        .collect();
                    run.failed.push((b, said_of(&names)));
                    run.stopped = true;
                } else {
                    // The batch is split: what was said of is answered, the rest a batch of
                    // its own that failed, to try again or go on without.
                    let names: Vec<String> = missed
                        .iter()
                        .filter_map(|&i| s.files.get(i).map(|f| f.name.clone()))
                        .collect();
                    if let Some(batch) = run.batches.get_mut(b) {
                        *batch = said;
                    }
                    run.answered.push(b);
                    run.batches.push(missed);
                    run.failed.push((run.batches.len() - 1, said_of(&names)));
                    run.stopped = true;
                }
            }
            Err(fault) => {
                run.failed.push((
                    b,
                    format!("The answer could not be read: {}.", fault.text()),
                ));
                run.stopped = true;
            }
        }
        self.notes_send_more();
    }

    /// A look failed. The model reading no pictures stops the run with the settings button;
    /// anything else keeps what was looked at and offers the failed ones again.
    pub(super) fn notes_look_failed(
        &mut self,
        id: u64,
        kind: &str,
        message: &str,
        retry_after: Option<u64>,
    ) {
        let Some(s) = self.task.as_mut() else { return };
        let Some(run) = s.notes_run.as_mut() else {
            return;
        };
        let Some(at) = run.in_flight.iter().position(|(r, _)| *r == id) else {
            return;
        };
        let (_, b) = run.in_flight.remove(at);
        if kind == "cancelled" {
            return;
        }
        run.failures += 1;
        run.stopped = true;
        if kind == "no_vision" {
            run.fault = Some(message.to_string());
            let out: Vec<u64> = run.in_flight.drain(..).map(|(r, _)| r).collect();
            for r in out {
                self.ai.cancel_id(r);
            }
        } else {
            let mut why = message.to_string();
            if let Some(secs) = retry_after.filter(|_| kind == "rate") {
                why.push_str(&format!(" Try again in {secs} s."));
            }
            let changed = kind == "image" && message.contains("changed since it was measured");
            if changed {
                why.push_str(": it is measured again.");
            }
            run.failed.push((b, why));
            if changed {
                // What is sent is measured again; "Try it again" waits for it.
                let ids = run.batches.get(b).cloned().unwrap_or_default();
                for i in ids {
                    if let Some(f) = s.files.get_mut(i) {
                        f.measure = Measure::Waiting;
                    }
                }
                self.notes_prepare(false);
            }
        }
        if let Some(s) = self.task.as_mut() {
            s.tick += 1;
        }
        self.redraw();
    }

    /// The arrange answered: the notes are read, and the plan is ready.
    pub(super) fn notes_arrange_done(&mut self, id: u64, text: &str) {
        let Some(s) = self.task.as_mut() else { return };
        let how_many = s.notes.how_many;
        let Some(run) = s.notes_run.as_mut().filter(|r| r.arrange == Some(id)) else {
            return;
        };
        run.arrange = None;
        run.answers += 1;
        let sent = run.sent_ids();
        match notes_ai::read_arrange(text, &sent) {
            Ok(a) if !a.notes.is_empty() => {
                run.previous = None;
                run.arranged = Some(if how_many == HowMany::One {
                    notes_ai::joined(a)
                } else {
                    a
                });
                s.tick += 1;
                self.notes_planned();
            }
            Ok(_) => self.notes_arrange_lost(Some("The answer placed no note.".to_string())),
            Err(fault) => self.notes_arrange_lost(Some(format!(
                "The answer could not be read: {}.",
                fault.text()
            ))),
        }
    }

    /// The arrange failed (`why`) or was stopped. A plan asked again gives back the one before
    /// it, card and all, saying why; a first plan offers the ways on.
    fn notes_arrange_lost(&mut self, why: Option<String>) {
        let Some(s) = self.task.as_mut() else { return };
        let Some(run) = s.notes_run.as_mut() else {
            return;
        };
        let restored = run.arrange_lost(why.clone());
        s.tick += 1;
        if restored {
            let said = why.map_or_else(
                || "Stopped.".to_string(),
                |w| format!("The plan could not be arranged again: {w}"),
            );
            self.docs
                .set_toast(&format!("{said} The plan you had is kept."));
            self.notes_plan_show();
        }
        self.redraw();
    }

    pub(super) fn notes_arrange_failed(&mut self, id: u64, kind: &str, message: &str) {
        let Some(s) = self.task.as_mut() else { return };
        let Some(run) = s.notes_run.as_mut().filter(|r| r.arrange == Some(id)) else {
            return;
        };
        run.arrange = None;
        if kind != "cancelled" {
            run.failures += 1;
        }
        self.notes_arrange_lost((kind != "cancelled").then(|| message.to_string()));
    }

    /// The plan is ready: its card opens over the window (phase V4, `app/notes_plan.rs`).
    fn notes_planned(&mut self) {
        self.notes_plan_show();
    }

    /// "Arrange again…" on the plan card: the arrange request asked again with `instruction`,
    /// and no picture sent again (D-201). The plan goes; the window shows the request out, and
    /// the card opens on its answer.
    pub(super) fn notes_arrange_again(&mut self, instruction: &str) {
        if self.ai_conn() == Connection::Off {
            return;
        }
        let Some(s) = self.task.as_mut() else { return };
        if s.busy() {
            return;
        }
        let Some(run) = s.notes_run.as_mut() else {
            return;
        };
        run.previous = run.arranged.take();
        run.arrange_instruction = Some(instruction.to_string());
        run.arrange_failed = None;
        s.blocked = None;
        self.notes_plan = None;
        self.notes_arrange();
    }

    /// Stop, Esc or the window's close: every request out is cancelled; what was looked at is
    /// kept, and the button comes back.
    pub(super) fn notes_stop(&mut self) -> bool {
        let stopped = self.notes_cancel();
        if stopped {
            self.task_say("Stopped. What was looked at is kept.");
            self.redraw();
        }
        stopped
    }

    /// Every request of the run out is cancelled; whether there was one.
    fn notes_cancel(&mut self) -> bool {
        let Some(run) = self.task.as_mut().and_then(|s| s.notes_run.as_mut()) else {
            return false;
        };
        if !run.looking() && run.arrange.is_none() {
            return false;
        }
        run.stopped = true;
        let mut out: Vec<u64> = run.in_flight.drain(..).map(|(r, _)| r).collect();
        if let Some(id) = run.arrange.take() {
            // A plan asked again: the one before it is kept.
            run.arrange_lost(None);
            out.push(id);
        }
        for r in out {
            self.ai.cancel_id(r);
        }
        if let Some(s) = self.task.as_mut() {
            s.tick += 1;
        }
        true
    }

    /// The requests of a window that closes go with it, and its plan card, and the pictures
    /// pasted into it: a note that took one has its copy by then.
    pub(super) fn notes_close(&mut self) {
        self.notes_plan = None;
        self.notes_cancel();
        let out: Vec<u64> = self
            .task
            .as_mut()
            .map(|s| s.prepare.drain(..).map(|(id, _)| id).collect())
            .unwrap_or_default();
        for id in out {
            self.ai.cancel_id(id);
        }
        let pasted: Vec<PathBuf> = self
            .task
            .as_ref()
            .map(|s| {
                s.files
                    .iter()
                    .filter(|f| notes_task::is_pasted(&f.path))
                    .map(|f| f.path.clone())
                    .collect()
            })
            .unwrap_or_default();
        for p in pasted {
            let _ = std::fs::remove_file(p);
        }
    }

    /// Cancel or Esc on the window. A window that keeps a plan of Write notes says first that
    /// closing throws the plan away (the plan does not outlive its window); the second closes.
    pub(super) fn task_close_asked(&mut self) {
        let Some(s) = self.task.as_mut() else { return };
        let kept =
            s.task == TaskKind::Notes && s.notes_run.as_ref().is_some_and(|r| r.arranged.is_some());
        if kept && !s.close_warned {
            s.close_warned = true;
            s.tick += 1;
            s.notice = Some(
                "A plan is kept in this window: closing throws it away. Cancel again to close, \
                 or Review the plan."
                    .to_string(),
            );
            self.redraw();
            return;
        }
        self.task_close();
    }

    /// "Try it again": the failed looks go again; "Go on with N": without them.
    pub(super) fn notes_again(&mut self, go_on: bool) {
        let Some(run) = self.task.as_mut().and_then(|s| s.notes_run.as_mut()) else {
            return;
        };
        if run.arrange_failed.is_some() {
            let Some(s) = self.task.as_mut() else { return };
            // The pictures looked at: "One note per picture instead" makes a note of each.
            let looked: Vec<usize> = s
                .notes_parts()
                .0
                .into_iter()
                .filter(|&i| s.notes_run.as_ref().is_some_and(|r| r.look(i).is_some()))
                .collect();
            if go_on && looked.is_empty() {
                return;
            }
            if let Some(run) = s.notes_run.as_mut() {
                run.arrange_failed = None;
            }
            if go_on {
                // "One note per picture instead": made here from the looks, no request, for
                // this plan only. Neither the window's How many nor the setting changes, so a
                // text file keeps its tick and the next Send arranges as before.
                self.notes_made_here(&looked, HowMany::PerPicture);
            } else {
                self.notes_arrange();
            }
            return;
        }
        if run.failed.is_empty() {
            return;
        }
        // Going on with nothing to make notes of is not offered: it would send an arrange
        // that cannot work, then "Try it again" would ask only that again.
        if go_on && !run.can_go_on() {
            return;
        }
        // What is asked changed since the looks: everything not kept is asked anew.
        let Some(s) = self.task.as_ref() else { return };
        let (instruction, text_out) = (s.instruction.text().trim().to_string(), s.notes.text_out);
        let Some(run) = self.task.as_mut().and_then(|s| s.notes_run.as_mut()) else {
            return;
        };
        if !go_on && (run.instruction != instruction || run.text_out != text_out) {
            run.failed.clear();
            run.stopped = true;
            self.notes_start();
            return;
        }
        if go_on {
            run.go_on();
        } else {
            run.again();
        }
        self.notes_send_more();
    }

    /// The engine ended: every request of the run is gone with it.
    pub(super) fn notes_engine_ended(&mut self, why: &str) {
        let Some(s) = self.task.as_mut() else { return };
        s.prepare.clear();
        for f in s.files.iter_mut() {
            if f.measure == Measure::Waiting && f.kind == FileKind::Picture {
                f.measure = Measure::Refused(why.to_string());
            }
        }
        if let Some(run) = s.notes_run.as_mut() {
            let out: Vec<usize> = run.in_flight.drain(..).map(|(_, b)| b).collect();
            for b in out {
                run.failed.push((b, why.to_string()));
            }
            if run.arrange.take().is_some() {
                run.arrange_lost(Some(why.to_string()));
            }
            run.stopped = true;
        }
        s.tick += 1;
    }

    /// One of the Options popover's lines: kept in the window and in the settings, and the
    /// pictures measured again when what is sent changed.
    pub(super) fn notes_option(&mut self, tick: Option<notes_task::Tick>, how: Option<HowMany>) {
        let Some(s) = self.task.as_mut() else { return };
        let before = s.notes;
        let mut o = s.notes;
        if let Some(t) = tick {
            if !t.enabled(&o) {
                return;
            }
            o = t.toggled(o);
        }
        if let Some(h) = how {
            o.how_many = h;
        }
        if o == before {
            return;
        }
        s.notes = o;
        // A plan made with other options is no longer the plan.
        if let Some(run) = s.notes_run.as_mut() {
            run.arranged = None;
        }
        s.tick += 1;
        if self.docs.prefs.ai_notes != o {
            let mut p = self.docs.prefs.clone();
            p.ai_notes = o;
            self.set_prefs(p);
        }
        if o.strip != before.strip || o.larger != before.larger {
            // What a look said of a picture stays: it is the same picture, sent another way.
            self.notes_prepare(true);
        }
        self.redraw();
    }

    /// Ctrl+V in the window with a picture on the clipboard: it is saved to a file of its own
    /// under the temp folder and added, as a screenshot dropped on the window would be.
    pub(super) fn notes_paste(&mut self) -> bool {
        let Some(task) = self.task.as_ref().map(|s| s.task) else {
            return false;
        };
        // Only Write notes takes a picture, and the clipboard's text is asked first.
        if task != TaskKind::Notes
            || !notes_task::paste_is_a_picture(task, crate::platform::clipboard_line().as_deref())
        {
            return false;
        }
        let Some(png) = crate::platform::clipboard_png() else {
            return false;
        };
        let dir = notes_task::pasted_dir();
        for old in notes_task::stale_pasted(&dir, std::time::SystemTime::now()) {
            let _ = std::fs::remove_file(old);
        }
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let path = dir.join(format!("Screenshot {secs}.png"));
        let written = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, &png));
        match written {
            Ok(()) => {
                self.task_add_paths(vec![path]);
                self.task_say("The picture on the clipboard is added.");
            }
            Err(e) => self.task_say(&format!("The picture could not be kept: {e}")),
        }
        self.redraw();
        true
    }
}
