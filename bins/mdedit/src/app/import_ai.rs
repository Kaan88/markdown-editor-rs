//! Sort a folder by AI (`docs/ai-library-plan.md` 5.7, phase 4), as the window hosts it: the
//! first request proposes the scheme, the second pass files the files in batches, and the plan
//! card shows and edits both before a line is written. The request is made by the engine, as
//! every AI request is: the engine is told a system text and a prompt, and nothing about the
//! library. Nothing here writes; the card's own Add does, through `importing.rs`.
//!
//! The requests run up to the connection's number in flight (D-080): three by default, one for a
//! local model. The answers are kept by batch and settled in the order of the files, whatever
//! order they come back in. A request that fails stops new ones; those in flight may finish, and
//! the notes they never reached keep their folders, so what did work stays usable.

use super::*;
use crate::ai::{Ask, SortRun};
use crate::import_ui::SortStage;
use mdedit_core::ai::Connection;
use mdedit_core::import_ai;
use serde_json::json;

impl App {
    /// Sets the card's AI state from the window's: whether the offer can be pressed, what it is
    /// called, and what it would send.
    pub(super) fn import_ai_refresh(&mut self) {
        let connection = self.ai_conn();
        let via = self.ai_via();
        let busy = self.ai.run.is_some()
            || self.ai.split.is_some()
            || self.task.as_ref().is_some_and(|t| t.asking.is_some())
            || self.note.as_ref().is_some_and(|c| c.asking.is_some());
        let reason = if connection == Connection::Off {
            Some("The AI is off: choose a connection in Settings \u{203A} AI.".to_string())
        } else if busy {
            Some("An AI run is going. Stop it first.".to_string())
        } else {
            None
        };
        let Some(card) = self.importer.as_mut() else {
            return;
        };
        card.ai_via = via;
        card.ai_estimate = import_ai::estimate(&card.plan);
        card.ai_reason = reason;
        card.tick += 1;
    }

    /// "Ask the AI to sort them…": the scheme request goes out.
    pub(super) fn import_sort_ask(&mut self) {
        if self.ai.run.is_some()
            || self.ai.split.is_some()
            || self.ai.sort.is_some()
            || self.task.as_ref().is_some_and(|t| t.asking.is_some())
            || self.note.as_ref().is_some_and(|c| c.asking.is_some())
        {
            return;
        }
        if self.ai_conn() == Connection::Off {
            if let Some(card) = self.importer.as_mut() {
                card.sort_failed(
                    "The AI connection is off: choose one in Settings \u{203A} AI first."
                        .to_string(),
                );
            }
            self.redraw();
            return;
        }
        let Some(card) = self.importer.as_ref() else {
            return;
        };
        let ask = match import_ai::scheme_ask(&card.plan) {
            Ok(ask) => ask,
            Err(why) => {
                if let Some(card) = self.importer.as_mut() {
                    card.sort_failed(why);
                }
                self.redraw();
                return;
            }
        };
        let via = self.ai_via();
        let parallel = self.docs.prefs.ai_parallel(self.docs.prefs.ai_connection);
        match self.sort_send(&ask.text, None) {
            Ok(id) => {
                let total = 1u64 + ask.files.div_ceil(import_ai::BATCH) as u64;
                self.ai.sort = Some(SortRun {
                    scheme: None,
                    batches: Vec::new(),
                    answers: Vec::new(),
                    in_flight: vec![(id, usize::MAX)],
                    next: 0,
                    parallel,
                    via,
                    stopped: false,
                    stop_reason: None,
                    requests: 0,
                    total,
                });
                if let Some(card) = self.importer.as_mut() {
                    card.sort_asking();
                }
                self.sort_status();
            }
            Err(why) => {
                if let Some(card) = self.importer.as_mut() {
                    card.sort_failed(why);
                }
            }
        }
        self.redraw();
    }

    /// "Sort the files": the scheme's batches go out, up to the connection's number at once.
    pub(super) fn import_sort_start(&mut self) {
        if self.ai.sort.is_some() {
            return;
        }
        // Every sort starts from the folders' plan: the batches number its notes as they are now.
        if let Some(card) = self.importer.as_mut() {
            card.folders_back();
        }
        let Some(card) = self.importer.as_ref() else {
            return;
        };
        let Some(scheme) = card.scheme().cloned() else {
            return;
        };
        if let Some(why) = scheme.problem() {
            if let Some(card) = self.importer.as_mut() {
                card.refusal = Some(why);
                card.tick += 1;
            }
            self.redraw();
            return;
        }
        let batches = import_ai::batches(&card.plan, &scheme);
        if batches.is_empty() {
            if let Some(card) = self.importer.as_mut() {
                card.sort_failed("There is nothing here for the AI to sort.".to_string());
            }
            self.redraw();
            return;
        }
        let total = 1u64 + batches.len() as u64;
        let parallel = self
            .docs
            .prefs
            .ai_parallel(self.docs.prefs.ai_connection)
            .max(1);
        self.ai.sort = Some(SortRun {
            scheme: Some(scheme),
            answers: vec![None; batches.len()],
            batches,
            in_flight: Vec::new(),
            next: 0,
            parallel,
            via: self.ai_via(),
            stopped: false,
            stop_reason: None,
            requests: 1,
            total,
        });
        self.sort_send_more();
        self.redraw();
    }

    /// Sends the next batch while there is room and nothing has failed.
    fn sort_send_more(&mut self) {
        while let Some((batch, prompt)) = self.sort_next_prompt() {
            match self.sort_send(&prompt, Some(batch)) {
                Ok(id) => {
                    if let Some(run) = self.ai.sort.as_mut() {
                        run.in_flight.push((id, batch));
                    }
                }
                Err(why) => {
                    if let Some(run) = self.ai.sort.as_mut() {
                        run.stopped = true;
                        if run.stop_reason.is_none() {
                            run.stop_reason = Some(why);
                        }
                    }
                    break;
                }
            }
        }
        self.sort_progress();
        if self.sort_all_answered() {
            self.sort_finish();
        }
    }

    /// The next batch to send, if one may go now.
    fn sort_next_prompt(&mut self) -> Option<(usize, String)> {
        let run = self.ai.sort.as_mut()?;
        if run.stopped || run.next >= run.batches.len() {
            return None;
        }
        if run.in_flight.len() >= run.parallel.max(1) {
            return None;
        }
        let batch = run.next;
        let prompt = run.batches.get(batch)?.text.clone();
        run.next += 1;
        Some((batch, prompt))
    }

    /// Sends one request to the engine: the system text and the prompt, nothing else.
    fn sort_send(&mut self, prompt: &str, batch: Option<usize>) -> Result<u64, String> {
        let p = &self.docs.prefs;
        let c = p.ai_connection;
        let system = if batch.is_some() {
            import_ai::FILE_SYSTEM
        } else {
            import_ai::SYSTEM
        };
        let req = json!({
            "op": "run",
            "conn": crate::ai::conn_json(c, p),
            "model": p.ai_model(c),
            "effort": p.ai_effort(c),
            "system": system,
            "prompt": prompt,
        });
        let ask = match batch {
            Some(b) => Ask::SortFiles(b),
            None => Ask::SortScheme,
        };
        let wake = self.docs.waker();
        self.ai.ask(&wake, ask, req)
    }

    /// Whether every request that can be answered has been.
    fn sort_all_answered(&self) -> bool {
        self.ai.sort.as_ref().is_some_and(|run| {
            run.in_flight.is_empty() && (run.stopped || run.next >= run.batches.len())
        })
    }

    /// Puts the run's progress on the card and in the status strip.
    fn sort_progress(&mut self) {
        let showing = self.ai.sort.as_ref().and_then(|run| {
            run.scheme.as_ref().map(|scheme| {
                (
                    scheme.clone(),
                    run.requests as usize,
                    run.total as usize,
                    run.in_flight.len(),
                )
            })
        });
        let Some((scheme, requests, total, in_flight)) = showing else {
            self.sort_status();
            return;
        };
        if let Some(card) = self.importer.as_mut()
            && !matches!(card.sort, SortStage::Filed { .. } | SortStage::Failed(_))
        {
            card.sort_progress(scheme, requests, total, in_flight);
        }
        self.sort_status();
    }

    /// The status strip's line while a sort is out; "Esc stops" because Esc in the card stops it.
    pub(super) fn sort_status(&mut self) {
        let folder = self
            .importer
            .as_ref()
            .and_then(|c| c.plan.folder.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let status = self.ai.sort.as_ref().map(|run| {
            if run.scheme.is_none() {
                format!(
                    "AI: asking {} for the sections of {folder}\u{2026}   Esc stops",
                    run.via
                )
            } else {
                format!(
                    "AI: sorting {folder}\u{2026} request {} of {}   Esc stops",
                    run.requests.max(1),
                    run.total
                )
            }
        });
        self.docs.ai_status = status;
    }

    /// The scheme came: the card shows it, and nothing else is asked until the owner says so.
    pub(super) fn sort_scheme_done(&mut self, text: &str) {
        if self.ai.sort.take().is_none() {
            return;
        }
        self.docs.ai_status = None;
        match import_ai::read_scheme(text) {
            Ok(scheme) => {
                if let Some(card) = self.importer.as_mut() {
                    card.sort_scheme(scheme);
                }
            }
            Err(fault) => {
                // The card's second line says that nothing was written; the reason does not
                // say it again.
                if let Some(card) = self.importer.as_mut() {
                    card.sort_failed(format!(
                        "The AI\u{2019}s answer could not be read: {}.",
                        fault.text()
                    ));
                }
            }
        }
        self.redraw();
    }

    /// One filing answer came. It is kept for its batch; when the first request fails, no new
    /// request goes out and those in flight may finish.
    pub(super) fn sort_batch_done(&mut self, id: u64, text: &str) {
        let Some(pos) = self
            .ai
            .sort
            .as_ref()
            .and_then(|run| run.in_flight.iter().position(|(at, _)| *at == id))
        else {
            return;
        };
        let Some(run) = self.ai.sort.as_mut() else {
            return;
        };
        let (_, batch) = run.in_flight.remove(pos);
        run.requests = run.requests.saturating_add(1);
        let (scheme_len, ids) = match (&run.scheme, run.batches.get(batch)) {
            (Some(scheme), Some(batch)) => (scheme.paths().len(), batch.ids.clone()),
            _ => {
                self.sort_after_answer();
                return;
            }
        };
        match import_ai::read_files(text, &ids, scheme_len) {
            Ok(answer) => {
                if let Some(slot) = run.answers.get_mut(batch) {
                    *slot = Some(answer);
                }
            }
            Err(fault) => {
                run.stopped = true;
                if run.stop_reason.is_none() {
                    run.stop_reason = Some(format!(
                        "The AI\u{2019}s answer to one request could not be read: {}.",
                        fault.text()
                    ));
                }
            }
        }
        self.sort_after_answer();
    }

    /// A request failed: a batch's stops new sends; the scheme's ends the run with nothing
    /// changed.
    pub(super) fn sort_request_failed(&mut self, id: u64, kind: &str, message: &str) {
        let scheme_none = self.ai.sort.as_ref().is_some_and(|r| r.scheme.is_none());
        if scheme_none {
            self.ai.sort = None;
            self.docs.ai_status = None;
            if let Some(card) = self.importer.as_mut() {
                card.sort_failed(format!(
                    "The AI could not be asked: {message}{}",
                    if kind == "cancelled" { "" } else { "." }
                ));
            }
            self.redraw();
            return;
        }
        let Some(run) = self.ai.sort.as_mut() else {
            return;
        };
        run.in_flight.retain(|(at, _)| *at != id);
        run.requests = run.requests.saturating_add(1);
        run.stopped = true;
        if run.stop_reason.is_none() {
            run.stop_reason = Some(if kind == "cancelled" {
                "The AI run was stopped.".to_string()
            } else {
                format!("The AI stopped: {message}")
            });
        }
        self.sort_after_answer();
    }

    /// The engine ended: whatever came is settled, and the card says what was lost.
    pub(super) fn sort_engine_ended(&mut self, why: &str) {
        let scheme_none = self.ai.sort.as_ref().is_some_and(|r| r.scheme.is_none());
        if scheme_none {
            self.ai.sort = None;
            self.docs.ai_status = None;
            if let Some(card) = self.importer.as_mut() {
                card.sort_failed(format!("The AI engine stopped: {why}"));
            }
            self.redraw();
            return;
        }
        let Some(run) = self.ai.sort.as_mut() else {
            return;
        };
        run.in_flight.clear();
        run.stopped = true;
        if run.stop_reason.is_none() {
            run.stop_reason = Some(format!("The AI engine stopped: {why}"));
        }
        self.sort_finish();
        self.redraw();
    }

    /// After an answer or a failure: send what may still go, or finish.
    fn sort_after_answer(&mut self) {
        if self.sort_all_answered() {
            self.sort_finish();
        } else {
            self.sort_send_more();
        }
        self.redraw();
    }

    /// Puts the run's answers on the card's plan and says what the AI did.
    fn sort_finish(&mut self) {
        let Some(run) = self.ai.sort.take() else {
            return;
        };
        self.docs.ai_status = None;
        let Some(scheme) = run.scheme.clone() else {
            if let Some(card) = self.importer.as_mut() {
                card.sort_failed("The AI did not answer.".to_string());
            }
            return;
        };
        let complete = !run.stopped
            && run.next >= run.batches.len()
            && run.answers.iter().all(Option::is_some);
        let Some(card) = self.importer.as_mut() else {
            return;
        };
        // The folders' plan, for "Keep the folders" (audit 4, B8).
        card.keep_folders();
        let sorted = import_ai::settle(
            &mut card.plan,
            &scheme,
            &run.batches,
            &run.answers,
            complete,
        );
        let stopped = if run.stopped {
            Some(
                run.stop_reason
                    .clone()
                    .unwrap_or_else(|| "The AI stopped before every file was sorted.".to_string()),
            )
        } else {
            None
        };
        if sorted.placed == 0 && sorted.unsorted == 0 {
            card.sort_failed(stopped.unwrap_or_else(|| "The AI placed no file.".to_string()));
        } else {
            card.sort_filed(
                run.requests as usize,
                sorted.placed,
                sorted.unsorted,
                stopped,
            );
        }
        self.import_ai_refresh();
    }

    /// Stop: every request in flight is cancelled, and the scheme the owner edited stays.
    pub(super) fn import_sort_stop(&mut self) {
        let scheme = self.ai.sort.as_ref().and_then(|r| r.scheme.clone());
        self.ai.cancel_sort();
        self.docs.ai_status = None;
        if let Some(card) = self.importer.as_mut() {
            match scheme {
                Some(scheme) => card.sort_scheme(scheme),
                None => card.sort_back(),
            }
        }
        self.import_ai_refresh();
        self.redraw();
    }

    /// Back to the folder's own sections: the AI step is dropped and nothing is written.
    pub(super) fn import_sort_back(&mut self) {
        self.ai.cancel_sort();
        self.docs.ai_status = None;
        if let Some(card) = self.importer.as_mut() {
            card.sort_back();
        }
        self.import_ai_refresh();
        self.redraw();
    }
}
