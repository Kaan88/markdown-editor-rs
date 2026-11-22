//! Split by AI (`docs/ai-library-plan.md` 5.2, phase 4b), as the window hosts it: "Ask the AI…"
//! on the plan screen opens the instruction panel for a split; what it sends is the outline of
//! the plan's document (`mdedit_core::split::answer`), never its text; the answer, read and
//! checked there, becomes the plan the screen shows. The request is made by the engine, as every
//! AI request is: the engine is told a system text and a prompt, and nothing about splits.
//!
//! One document is one request (D-080's parallel requests have nothing to run in parallel
//! here). An answer that cannot be read is asked for once more with the fault named, then the
//! screen says so with "Run again" and "Split at headings instead".

use super::*;
use crate::ai::{Ask, Instruction, Panel, Spec, SplitAsk, SplitRun};
use crate::plan_ui::{AiAct, AiCut, Notice, PlanScreen};
use mdedit_core::ai::Connection;
use mdedit_core::split::answer::{self, Fault, OUTLINE_MAX, Outline, TooLong};
use mdshared_ui::textfield::TextField;
use serde_json::json;

/// What the line above the list says while the request is out.
fn asking(via: &str) -> Notice {
    Notice {
        text: format!("Asking the AI where to cut it\u{2026} ({via})"),
        warn: false,
        show: None,
        links: vec![(AiAct::Stop, "Stop".to_string())],
    }
}

/// What it says when the answer could not be read, after the one retry (check 2).
pub(super) fn unreadable() -> Notice {
    Notice {
        text: "The AI\u{2019}s answer could not be read.".to_string(),
        warn: true,
        show: None,
        links: vec![
            (AiAct::RunAgain, "Run again".to_string()),
            (AiAct::AtHeadings, "Split at headings instead".to_string()),
        ],
    }
}

/// What it says when the document is too long to be sent even as headings.
pub(super) fn too_long(why: &TooLong) -> Notice {
    Notice {
        text: why.text(),
        warn: true,
        show: None,
        links: Vec::new(),
    }
}

/// What it says when the request failed: the engine's words, and "Try again".
fn failed(message: &str) -> Notice {
    Notice {
        text: message.to_string(),
        warn: true,
        show: None,
        links: vec![(AiAct::RunAgain, "Try again".to_string())],
    }
}

/// A plain warning, with nothing to press.
fn said(text: &str) -> Notice {
    Notice {
        text: text.to_string(),
        warn: true,
        show: None,
        links: Vec::new(),
    }
}

/// The panel's line under its title: what is sent of the document, in how many requests.
pub(super) fn sent_line(ask: &SplitAsk) -> String {
    let kb = crate::plan_ui::size_text(ask.outline.text.len());
    if ask.outline.headings_only {
        format!(
            "Sends only the headings, {kb} in 1 request: with the paragraphs it is over 200 KB."
        )
    } else {
        format!(
            "Sends the headings and the start of each paragraph, {kb} in 1 request; not the text."
        )
    }
}

impl App {
    /// What the plan screen's AI buttons and links ask of the window.
    pub(super) fn plan_ai(&mut self, act: AiAct) {
        match act {
            AiAct::Ask => self.split_open_panel(),
            AiAct::Stop => self.split_stop(),
            AiAct::RunAgain => {
                let spec = self.ai.split.as_ref().map(|r| r.spec.clone());
                let last = spec.or_else(|| self.split_last.clone());
                match last {
                    Some(spec) => self.split_start(spec),
                    None => self.split_open_panel(),
                }
            }
            // The screen cut itself at headings again; a request still out is not wanted.
            AiAct::AtHeadings => {
                self.ai.cancel_split();
                self.docs.ai_status = None;
                if let Some(screen) = self.plan.as_mut() {
                    screen.set_ai_notice(None);
                }
                self.split_recheck();
            }
        }
    }

    /// The outline of the plan's document, or the refusal.
    fn split_outline(screen: &PlanScreen) -> Result<SplitAsk, TooLong> {
        let outline: Outline =
            answer::outline(&**screen.plan.document(), screen.headings(), OUTLINE_MAX)?;
        Ok(SplitAsk {
            name: screen.source.name.clone(),
            outline,
        })
    }

    /// "Ask the AI…": the instruction panel, for a split, over the window; the plan screen
    /// waits behind it and comes back when the panel closes. A document whose outline is too
    /// long is refused on the screen, and no panel opens.
    pub(super) fn split_open_panel(&mut self) {
        let Some(screen) = self.plan.as_mut() else {
            return;
        };
        if self.ai.split.is_some() {
            return;
        }
        let ask = match Self::split_outline(screen) {
            Ok(ask) => ask,
            Err(why) => {
                screen.set_ai_notice(Some(too_long(&why)));
                self.redraw();
                return;
            }
        };
        if self.ai.run.is_some() {
            screen.set_ai_notice(Some(said(
                "An AI rewrite is running. Stop it first (Esc in the editor).",
            )));
            self.redraw();
            return;
        }
        self.ai_load_prompts();
        self.plan_behind = self.plan.take();
        let via = self.ai_via();
        self.ai.panel = Some(Panel::Instruction(Instruction {
            field: TextField::new(),
            spec: None,
            source: (DocId::default(), 0, 0, String::new()),
            changed: false,
            split: Some(ask.clone()),
            selected: None,
            scope: sent_line(&ask),
            via,
            scroll: 0.0,
        }));
        self.ai.hover = None;
        self.ai.pressed = None;
        self.caret_reset();
        self.ai.tick += 1;
        self.redraw();
    }

    /// The plan screen the split panel stood in front of comes back.
    pub(super) fn split_panel_closed(&mut self) {
        if let Some(screen) = self.plan_behind.take() {
            self.plan_open(screen);
        }
    }

    /// The split panel's Split: the request goes out, and the plan screen comes back with a
    /// line saying so.
    pub(super) fn split_start(&mut self, spec: Spec) {
        if matches!(&self.ai.panel, Some(Panel::Instruction(i)) if i.split.is_some()) {
            self.ai.panel = None;
            self.split_panel_closed();
        }
        if self.ai_conn() == Connection::Off {
            if let Some(screen) = self.plan.as_mut() {
                screen.set_ai_notice(Some(said(
                    "AI is off. Choose a connection in Settings \u{203A} AI first.",
                )));
            }
            self.redraw();
            return;
        }
        let Some(screen) = self.plan.as_ref() else {
            return;
        };
        let ask = match Self::split_outline(screen) {
            Ok(ask) => ask,
            Err(why) => {
                if let Some(screen) = self.plan.as_mut() {
                    screen.set_ai_notice(Some(too_long(&why)));
                }
                self.redraw();
                return;
            }
        };
        self.ai.cancel_split();
        let prompt = answer::prompt(&spec.instruction, &ask.name, &ask.outline);
        let via = self.ai_via();
        match self.split_send(&prompt) {
            Ok(id) => {
                self.split_last = Some(spec.clone());
                self.ai.split = Some(SplitRun {
                    id,
                    spec,
                    ask,
                    prompt,
                    via: via.clone(),
                    chars: 0,
                    requests: 1,
                });
                if let Some(screen) = self.plan.as_mut() {
                    screen.set_ai_notice(Some(asking(&via)));
                }
                self.split_status();
            }
            Err(why) => {
                if let Some(screen) = self.plan.as_mut() {
                    screen.set_ai_notice(Some(failed(&why)));
                }
            }
        }
        self.redraw();
    }

    /// Sends one split request to the engine: the system text and the prompt, nothing else.
    fn split_send(&mut self, prompt: &str) -> Result<u64, String> {
        let p = &self.docs.prefs;
        let c = p.ai_connection;
        let req = json!({
            "op": "run",
            "conn": crate::ai::conn_json(c, p),
            "model": p.ai_model(c),
            "effort": p.ai_effort(c),
            "system": answer::SYSTEM,
            "prompt": prompt,
        });
        let wake = self.docs.waker();
        self.ai.ask(&wake, Ask::Split, req)
    }

    /// The status strip while a split is out.
    pub(super) fn split_status(&mut self) {
        self.docs.ai_status = self.ai.split.as_ref().map(|r| {
            let what = if r.chars == 0 {
                format!("AI: splitting {} with {}\u{2026}", r.ask.name, r.via)
            } else {
                format!(
                    "AI: splitting {}\u{2026} {} characters",
                    r.ask.name, r.chars
                )
            };
            format!("{what}   Esc stops")
        });
    }

    /// Stop: the request is cancelled and the plan stays as it was.
    pub(super) fn split_stop(&mut self) {
        self.ai.cancel_split();
        self.docs.ai_status = None;
        if let Some(screen) = self.plan.as_mut() {
            screen.set_ai_notice(None);
        }
        self.redraw();
    }

    /// A split's answer came: read and checked (5.2), it becomes the plan on the screen. One
    /// answer that cannot be read is asked for again with its fault named; a second is said on
    /// the screen.
    pub(super) fn split_done(&mut self, text: &str) {
        let Some(mut run) = self.ai.split.take() else {
            return;
        };
        self.docs.ai_status = None;
        match answer::read(text) {
            Err(fault) if run.requests < 2 => self.split_retry(run, fault),
            Err(_) => {
                if let Some(screen) = self.plan.as_mut() {
                    screen.set_ai_notice(Some(unreadable()));
                }
            }
            Ok(a) => {
                run.chars = 0;
                self.split_show(&run, &a);
            }
        }
        self.redraw();
    }

    fn split_retry(&mut self, mut run: SplitRun, fault: Fault) {
        let prompt = answer::retry_prompt(&run.prompt, fault);
        match self.split_send(&prompt) {
            Ok(id) => {
                run.id = id;
                run.requests += 1;
                run.chars = 0;
                self.ai.split = Some(run);
                self.split_status();
            }
            Err(why) => {
                if let Some(screen) = self.plan.as_mut() {
                    screen.set_ai_notice(Some(failed(&why)));
                }
            }
        }
    }

    /// The plan an answer makes, on the screen: the same document, the group the screen has.
    fn split_show(&mut self, run: &SplitRun, a: &answer::Answer) {
        let known = self.split_known_sections();
        let p = &self.docs.prefs;
        let c = p.ai_connection;
        let cut = AiCut {
            instruction: run.spec.instruction.clone(),
            via: run.via.clone(),
            connection: c.key().to_string(),
            model: p.ai_model(c).to_string(),
            bytes: run.prompt.len() as u64,
            requests: run.requests,
            headings_only: run.ask.outline.headings_only,
            dropped: answer::Dropped::default(),
        };
        let Some(screen) = self.plan.as_mut() else {
            return;
        };
        let (plan, dropped) = answer::plan(
            screen.plan.document().clone(),
            &screen.source.name,
            screen.headings(),
            screen.plan.level(),
            a,
            &known,
        );
        screen.set_ai_plan(plan, AiCut { dropped, ..cut });
        self.split_recheck();
    }

    /// The section paths the group the notes go to already has, so a section the model spells
    /// in another case is the one there is (check 9).
    fn split_known_sections(&self) -> Vec<Vec<String>> {
        let Some(screen) = self.plan.as_ref() else {
            return Vec::new();
        };
        self.docs
            .library
            .groups
            .iter()
            .find(|g| mdshared_app::paths::same_path(&g.file, &screen.dest.file))
            .map(|g| g.sections.clone())
            .unwrap_or_default()
    }

    /// The notes changed under the same group: what blocks the write is worked out again.
    fn split_recheck(&mut self) {
        let Some(screen) = self.plan.as_ref() else {
            return;
        };
        if screen.source.path.is_none() {
            return;
        }
        let blocked = super::plan_write::plan_blocked(self, screen);
        if let Some(screen) = self.plan.as_mut() {
            screen.blocked = blocked;
            screen.blocked_for = Some(screen.dest.clone());
            screen.tick += 1;
        }
    }

    /// A split's request failed, or the engine ended.
    pub(super) fn split_failed(&mut self, kind: &str, message: &str) {
        self.ai.split = None;
        self.docs.ai_status = None;
        if let Some(screen) = self.plan.as_mut() {
            screen.set_ai_notice((kind != "cancelled").then(|| failed(message)));
        }
        self.redraw();
    }

    /// The AI states of `--plan-demo` (`split-ai…`), each reached by the path the window takes,
    /// with a made-up answer and no engine: the answer is read and checked as a real one is.
    pub(super) fn split_demo(&mut self, name: &str) {
        let via = self.ai_via();
        match name {
            "split-ai" | "split-ai-dropped" => {
                let top = name == "split-ai-dropped";
                let Some(screen) = self.plan.as_ref() else {
                    return;
                };
                let text = crate::plan_demo::answer_for(name, screen);
                let Ok(ask) = Self::split_outline(screen) else {
                    return;
                };
                let Ok(a) = answer::read(&text) else {
                    return;
                };
                let spec = Spec {
                    title: "Split".into(),
                    instruction: crate::plan_demo::INSTRUCTION.into(),
                };
                let run = SplitRun {
                    id: 0,
                    prompt: answer::prompt(&spec.instruction, &ask.name, &ask.outline),
                    spec,
                    ask,
                    via,
                    chars: 0,
                    requests: 1,
                };
                self.split_show(&run, &a);
                if let Some(screen) = self.plan.as_mut() {
                    // The row the overlap mark is on, or the first one with a mark.
                    if let Some(i) = screen.marked().last().copied().filter(|_| !top) {
                        screen.select(i);
                    }
                    screen.edited = false;
                }
            }
            "split-ai-unreadable" => {
                if answer::read(crate::plan_demo::BAD_ANSWER).is_err()
                    && let Some(screen) = self.plan.as_mut()
                {
                    screen.set_ai_notice(Some(unreadable()));
                }
            }
            "split-ai-long" => self.split_open_panel(),
            "split-ai-asking" => {
                if let Some(screen) = self.plan.as_mut() {
                    screen.set_ai_notice(Some(asking(&via)));
                }
            }
            "split-ai-ask" => self.split_demo_panel(),
            _ => {}
        }
    }

    /// `--plan-demo split-ai-ask`: the split panel over a plan, with saved prompts of the kind,
    /// and no request.
    fn split_demo_panel(&mut self) {
        self.docs.ai_split_prompts = crate::plan_demo::split_prompts();
        self.ai.prompts_stamp = Some(None);
        self.split_open_panel();
        if let Some(i) = self.ai.instruction() {
            i.field
                .set_text("One note per day of the trip, under the city it was spent in");
            i.selected = None;
        }
    }
}
