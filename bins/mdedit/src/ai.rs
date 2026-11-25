//! AI rewriting in the shell: the `mdedit-ai` engine process and what is on screen for it (a
//! run in progress, the instruction and review panels, the bar that reports a problem).
//!
//! The engine is a separate program, started on first use and kept for the session: whatever it
//! does (network, TLS, child processes), it cannot end the editor. It speaks JSON lines; see
//! `bins/mdedit-ai/src/proto.rs`. The text changes only when the user applies a result (or, with
//! the review off, when the result comes back and the text it replaces is unchanged).

use crate::app::Wake;
use crate::docs::Waker;
use mdedit_core::ai::{Connection, SavedPrompt};
use mdedit_core::hunks::Hunks;
use mdedit_highlight::DocId;
use mdshared_ui::gfx::Rect;
use mdshared_ui::textfield::TextField;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How long the engine has to say hello before it counts as broken.
const HELLO: Duration = Duration::from_secs(10);

fn engine_file() -> String {
    format!("mdedit-ai{}", std::env::consts::EXE_SUFFIX)
}

/// Where the engine is, in the order looked at: beside the executable (Windows, the macOS
/// bundle, the AppImage, a cargo build), then `../lib/mdedit/` (the Linux packages).
pub fn engine_paths(exe: Option<&Path>) -> Vec<PathBuf> {
    let Some(dir) = exe.and_then(Path::parent) else {
        return Vec::new();
    };
    let mut at = vec![dir.join(engine_file())];
    if let Some(prefix) = dir.parent().filter(|_| cfg!(target_os = "linux")) {
        at.push(prefix.join("lib/mdedit").join(engine_file()));
    }
    at
}

/// A model a connection offers, as the engine listed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub efforts: Vec<String>,
}

/// What the engine said.
#[derive(Debug)]
pub enum Event {
    Delta {
        id: u64,
        chars: usize,
    },
    Done {
        id: u64,
        text: String,
    },
    Error {
        id: u64,
        kind: String,
        message: String,
        retry_after: Option<u64>,
    },
    Models {
        id: u64,
        list: Vec<ModelInfo>,
    },
    Detected {
        id: u64,
        path: String,
        version: String,
    },
    Key {
        id: u64,
        stored: bool,
    },
    /// What one picture of a `prepare` would be sent as, or why it cannot be (protocol 2).
    Picture {
        id: u64,
        index: usize,
        info: Result<PictureInfo, String>,
    },
    /// Every picture of a `prepare` has been told.
    Prepared {
        id: u64,
    },
    /// What a run with pictures sends, said as it goes: each picture's bytes, and the text's.
    Sent {
        id: u64,
        bytes: u64,
        text_bytes: u64,
    },
    /// The engine ended, could not start or did not answer.
    Ended(String),
}

/// What the engine said a picture would be sent as (`picture` event, protocol 2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureInfo {
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub sha256: String,
    pub original_width: u32,
    pub original_height: u32,
    /// What was done to it, in the engine's words ("made smaller", "details removed").
    pub done: Vec<String>,
    /// A PNG of at most 160 px of what is sent; `None` when the engine made none (a GIF).
    pub thumb: Option<Vec<u8>>,
}

impl PictureInfo {
    /// Whether it goes smaller than it is.
    pub fn smaller(&self) -> bool {
        self.width < self.original_width || self.height < self.original_height
    }
}

/// Standard base64 with or without padding, as the engine writes a thumbnail; `None` for
/// anything else. Nothing read from the engine may panic (the release build aborts on one).
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let t = text.trim().trim_end_matches('=').as_bytes();
    if t.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(t.len() / 4 * 3 + 2);
    for chunk in t.chunks(4) {
        let mut n = 0u32;
        for (k, &c) in chunk.iter().enumerate() {
            n |= value(c)? << (18 - 6 * k as u32);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Some(out)
}

/// The most a thumbnail of the engine's may be, as base64: past it there is none.
const THUMB_MAX: usize = 256 * 1024;

fn picture_info(v: &Value) -> Option<PictureInfo> {
    let n = |k: &str| v[k].as_u64();
    let px = |k: &str| n(k).and_then(|x| u32::try_from(x).ok());
    Some(PictureInfo {
        format: v["format"].as_str()?.to_string(),
        width: px("width")?,
        height: px("height")?,
        bytes: n("bytes")?,
        sha256: v["sha256"].as_str()?.to_string(),
        original_width: px("original_width").unwrap_or(0),
        original_height: px("original_height").unwrap_or(0),
        done: v["done"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        thumb: v["thumb"]
            .as_str()
            .filter(|t| t.len() <= THUMB_MAX)
            .and_then(base64_decode),
    })
}

/// The first protocol version that sends pictures (`docs/ai-pictures-plan.md` section 6).
pub const PICTURES_VERSION: u32 = 2;

/// Why an engine that greeted with `version` cannot be sent pictures, if it cannot.
pub fn pictures_refused_at(version: u32) -> Option<&'static str> {
    (version < PICTURES_VERSION).then_some(OLD_ENGINE)
}

/// Said where pictures would be sent, when the engine beside mdedit is older than it.
#[allow(dead_code)]
pub const OLD_ENGINE: &str = "The AI engine beside mdedit is older than mdedit and cannot be sent pictures. Reinstall or update.";

/// The protocol version a greeting says, if `line` is the engine's greeting.
pub fn hello_version(line: &str) -> Option<u32> {
    let v: Value = serde_json::from_str(line).ok()?;
    if v["ev"] != "hello" {
        return None;
    }
    Some(
        v["version"]
            .as_u64()
            .map_or(1, |n| u32::try_from(n).unwrap_or(u32::MAX)),
    )
}

/// Reads one line the engine wrote; `None` for the greeting and anything not understood.
pub fn parse_event(line: &str) -> Option<Event> {
    let v: Value = serde_json::from_str(line).ok()?;
    let id = v["id"].as_u64().unwrap_or(0);
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    Some(match v["ev"].as_str()? {
        "delta" => Event::Delta {
            id,
            chars: v["chars"].as_u64().unwrap_or(0) as usize,
        },
        "done" => Event::Done {
            id,
            text: s("text"),
        },
        "error" => Event::Error {
            id,
            kind: s("kind"),
            message: s("message"),
            retry_after: v["retry_after"].as_u64(),
        },
        "models" => Event::Models {
            id,
            list: v["list"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|m| {
                            let id = m["id"].as_str()?.to_string();
                            Some(ModelInfo {
                                name: m["name"].as_str().unwrap_or(&id).to_string(),
                                efforts: m["efforts"]
                                    .as_array()
                                    .map(|e| {
                                        e.iter()
                                            .filter_map(|x| x.as_str().map(str::to_string))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                id,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        },
        "detected" => Event::Detected {
            id,
            path: s("path"),
            version: s("version"),
        },
        "key" => Event::Key {
            id,
            stored: v["stored"].as_bool().unwrap_or(false),
        },
        "picture" => Event::Picture {
            id,
            index: v["index"].as_u64().and_then(|i| usize::try_from(i).ok())?,
            info: match v["error"].as_str() {
                Some(why) => Err(why.to_string()),
                None => picture_info(&v).ok_or_else(|| "the engine's answer was not read".into()),
            },
        },
        "prepared" => Event::Prepared { id },
        "sent" => {
            let images = v["images"].as_array();
            Event::Sent {
                id,
                bytes: images.map_or(0, |a| a.iter().filter_map(|i| i["bytes"].as_u64()).sum()),
                text_bytes: v["text_bytes"].as_u64().unwrap_or(0),
            }
        }
        _ => return None,
    })
}

/// The running engine: a writer thread for requests, a reader thread for events.
pub struct Client {
    input: Sender<String>,
    events: Receiver<Event>,
    child: Arc<Mutex<Child>>,
    /// The protocol version the engine's greeting said; 0 until it came.
    version: Arc<std::sync::atomic::AtomicU32>,
}

impl Client {
    /// Starts the engine found beside mdedit. `wake` is called when an event waits.
    pub fn start(wake: Waker) -> Result<Client, String> {
        let exe = std::env::current_exe().ok();
        // MDEDIT_AI_ENGINE names another engine: for tests.
        let named = std::env::var_os("MDEDIT_AI_ENGINE").map(PathBuf::from);
        let Some(path) = named
            .into_iter()
            .chain(engine_paths(exe.as_deref()))
            .find(|p| p.is_file())
        else {
            return Err(
                "The AI engine (mdedit-ai) is missing from mdedit's folder. An updated copy fetches it                  at its next start; otherwise reinstall mdedit."
                    .into(),
            );
        };
        let mut cmd = Command::new(&path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: the engine is a console program; no console flashes up.
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("The AI engine could not be started: {e}"))?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            return Err("The AI engine could not be started.".into());
        };
        let child = Arc::new(Mutex::new(child));
        let (input, rx) = mpsc::channel::<String>();
        let (tx, events) = mpsc::channel::<Event>();
        std::thread::Builder::new()
            .name("ai-in".into())
            .spawn(move || {
                let mut out = stdin;
                while let Ok(line) = rx.recv() {
                    if writeln!(out, "{line}").and_then(|_| out.flush()).is_err() {
                        break;
                    }
                }
                // Dropping stdin here ends the engine.
            })
            .map_err(|e| e.to_string())?;
        let (hello_tx, hello_rx) = mpsc::channel::<()>();
        let (tx2, wake2, watch) = (tx.clone(), wake.clone(), child.clone());
        std::thread::Builder::new()
            .name("ai-wait".into())
            .spawn(move || {
                if hello_rx.recv_timeout(HELLO).is_err() {
                    if let Ok(mut c) = watch.lock() {
                        let _ = c.kill();
                    }
                    let _ = tx2.send(Event::Ended("The AI engine did not start.".into()));
                    wake2(Wake::Ai);
                }
            })
            .ok();
        let reaper = child.clone();
        let version = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let said = version.clone();
        std::thread::Builder::new()
            .name("ai-out".into())
            .spawn(move || {
                let mut hello = Some(hello_tx);
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if let Some(n) = hello_version(&line) {
                        said.store(n, std::sync::atomic::Ordering::Release);
                        if let Some(h) = hello.take() {
                            let _ = h.send(());
                        }
                        continue;
                    }
                    if let Some(e) = parse_event(&line) {
                        if tx.send(e).is_err() {
                            return;
                        }
                        wake(Wake::Ai);
                    }
                }
                if hello.is_none() {
                    let _ = tx.send(Event::Ended("The AI engine stopped.".into()));
                    wake(Wake::Ai);
                }
                if let Ok(mut c) = reaper.lock() {
                    let _ = c.wait();
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Client {
            input,
            events,
            child,
            version,
        })
    }

    /// The protocol version the engine said; 0 until its greeting came.
    pub fn version(&self) -> u32 {
        self.version.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Why the engine cannot be sent pictures, if it cannot: one older than mdedit. Write notes
    /// asks this where its button is.
    pub fn pictures_refused(&self) -> Option<&'static str> {
        pictures_refused_at(self.version())
    }

    pub fn send(&self, v: Value) -> bool {
        self.input.send(v.to_string()).is_ok()
    }

    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        // The input closes with the writer thread; a run in progress is stopped by the engine.
        // Killed too, in case it is stuck in a request.
        if let Ok(mut c) = self.child.lock() {
            let _ = c.kill();
        }
    }
}

/// What a request was for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    Run,
    /// A split by AI (5.2): the answer goes to the plan screen.
    Split,
    /// The scheme of a folder sort by AI (5.7): the answer goes to the import card.
    SortScheme,
    /// One filing request of a folder sort by AI, by batch number.
    SortFiles(usize),
    /// A request of the AI task window over files (5.6).
    Task,
    /// A Markdown note from a text file (5.8): the answer goes to the note card.
    Note,
    /// Write notes (pictures plan): what each picture of the window would be sent as.
    Prepare,
    /// Write notes: a look request, by batch number.
    Look(usize),
    /// Write notes: the arrange request.
    Arrange,
    /// A model list, under its key in [`Ai::models`].
    Models(String),
    Detect(Connection),
    Key(String),
}

/// What to run: a title for the review and the instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub title: String,
    pub instruction: String,
}

/// A run in progress.
pub struct Run {
    pub id: u64,
    pub doc: DocId,
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub spec: Spec,
    /// "trip-notes.md, whole file" or "4 selected lines".
    pub scope: String,
    /// "Claude API · Claude Opus 5.5 · High".
    pub via: String,
    pub chars: usize,
}

/// A problem, in the bar under the command bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bar {
    pub text: String,
    pub action: Option<BarAction>,
    /// Information (blue), not a warning.
    pub info: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarAction {
    OpenSettings,
    TryAgain,
}

impl BarAction {
    pub fn label(self) -> &'static str {
        match self {
            BarAction::OpenSettings => "Open AI settings",
            BarAction::TryAgain => "Try again",
        }
    }
}

/// The instruction panel.
pub struct Instruction {
    pub field: TextField,
    /// The instruction the box was filled with (Run again): run under its own title while the
    /// box still holds it.
    pub spec: Option<Spec>,
    /// What the panel says will be sent: document, byte range and text. Rewrite sends only that;
    /// if it changed meanwhile the panel shows the new scope and waits for Rewrite again.
    pub source: (DocId, usize, usize, String),
    /// The scope changed since the panel showed it: the line says so.
    pub changed: bool,
    /// The panel asks for a split of the plan's document, not a rewrite (5.2): what it sends.
    pub split: Option<SplitAsk>,
    /// The saved prompt the keyboard is on.
    pub selected: Option<usize>,
    pub scope: String,
    pub via: String,
    pub scroll: f32,
}

/// What the instruction panel sends when it asks for a split (5.2): the document's name and its
/// outline, never its text.
#[derive(Clone, Debug)]
pub struct SplitAsk {
    pub name: String,
    pub outline: mdedit_core::split::answer::Outline,
}

/// A split by AI in flight. Its answer goes to the plan screen that asked for it.
#[derive(Clone, Debug)]
pub struct SplitRun {
    pub id: u64,
    pub spec: Spec,
    pub ask: SplitAsk,
    /// The request's text as first sent; a retry adds the fault to it.
    pub prompt: String,
    pub via: String,
    pub chars: usize,
    /// Requests made for this split: 2 after the retry of check 2.
    pub requests: u64,
}

/// A folder sort by AI in flight (5.7): the scheme request first, then the filing requests in
/// batches, up to the connection's number of requests at once (D-080).
pub struct SortRun {
    /// The scheme, once it has come; `None` while the first request is out.
    pub scheme: Option<mdedit_core::import_ai::Scheme>,
    /// The filing requests' texts, in order; empty until the scheme is taken.
    pub batches: Vec<mdedit_core::import_ai::Batch>,
    /// Each batch's answer, once it comes; `None` for one not answered, or one that failed.
    pub answers: Vec<Option<mdedit_core::import_ai::Answer>>,
    /// The request ids in flight and the batch each is for.
    pub in_flight: Vec<(u64, usize)>,
    /// The next batch to send.
    pub next: usize,
    pub parallel: usize,
    pub via: String,
    /// A request failed: no new one is sent, and those in flight may finish.
    pub stopped: bool,
    /// What stopped it, for the card's line.
    pub stop_reason: Option<String>,
    /// The scheme plus every answer received.
    pub requests: u64,
    /// The scheme plus every batch.
    pub total: u64,
}

/// The review panel.
pub struct Review {
    pub doc: DocId,
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub result: String,
    pub spec: Spec,
    pub subtitle: String,
    /// The changes and which of them are ticked; Apply puts in what they compose.
    pub hunks: Hunks,
    /// The list of changes: its scroll, the change the keys are on.
    pub list: crate::hunks_ui::ListState,
    /// Why the result cannot be applied in place (the text changed meanwhile, the document is
    /// read-only or closed): Apply gives way to Copy result and Run again.
    pub blocked: Option<String>,
    /// The rendered before and after rather than the changes.
    pub preview: bool,
    /// The before and after drawn by mdview's renderer, opened when first shown.
    pub panes: Option<Box<(crate::preview_pane::Pane, crate::preview_pane::Pane)>>,
    /// The AFTER page shows the ticked changes: the version of the text it was sent, and when a
    /// tick not yet sent is due (the delay Compare uses).
    pub sent: u64,
    pub due: Option<std::time::Instant>,
    /// The drag of the list's scrollbar thumb: where in the thumb it was taken.
    pub grab: Option<f32>,
    pub base_dir: Option<PathBuf>,
    /// A review of a file the AI task window is rewriting, not of an open document (5.6).
    pub file: Option<FileReview>,
}

/// What a review over a file rather than a document is: the file Apply writes in place, and the
/// window's entry it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileReview {
    pub path: PathBuf,
    /// The task window the review came from ([`crate::task_ui::TaskScreen::id`]).
    pub window: u64,
    /// The file's place in the task window's files.
    pub index: usize,
    /// The original began with a UTF-8 byte order mark.
    pub bom: bool,
    /// The file's dominant line ending, which the result is written with.
    pub eol: String,
    /// The task window's instruction, for Run again.
    pub instruction: String,
    /// What the window had sent when the answer came: the record of a review applied after its
    /// window closed says it.
    pub sent: mdedit_core::runs::Sent,
}

// One panel is open at a time, kept in an `Option` in `Ai`: boxing the review would only add a
// pointer to every place that reads it.
#[allow(clippy::large_enum_variant)]
pub enum Panel {
    Instruction(Instruction),
    Review(Review),
}

/// What can be clicked on a panel, as last drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hit {
    Card,
    Field,
    Prompt(usize),
    /// A saved prompt's pencil: into the box to be changed, not run.
    EditPrompt(usize),
    Cancel,
    Rewrite,
    /// The instruction panel: save what is typed as a saved prompt.
    SavePrompt,
    Changes,
    Preview,
    Discard,
    Copy,
    Apply,
    RunAgain,
    /// The review's bar: the previous and the next change, the strip's mark for change `i`,
    /// and every change ticked or none.
    Prev,
    Next,
    Mark(usize),
    TickAll,
    UntickAll,
    /// The list of changes.
    List(crate::hunks_ui::Hit),
    /// The changes' or the previews' area, for the wheel.
    Body,
}

#[derive(Default)]
pub struct Ai {
    client: Option<Client>,
    next: u64,
    pending: HashMap<u64, Ask>,
    pub run: Option<Run>,
    /// A split by AI in flight (5.2).
    pub split: Option<SplitRun>,
    /// A folder sort by AI in flight (5.7).
    pub sort: Option<SortRun>,
    pub panel: Option<Panel>,
    /// The review that Escape closed: its result stays until it is shown again and applied or
    /// discarded, replaced by another run, or the program ends.
    pub kept: Option<Review>,
    /// `--ai-demo`: the changes the next review starts with unticked.
    pub demo_off: Option<Vec<usize>>,
    pub bar: Option<Bar>,
    /// The last run asked, for Try again.
    pub last: Option<Spec>,
    /// Model lists fetched this session, per connection (and base URL for OpenAI-compatible).
    pub models: HashMap<String, Result<Vec<ModelInfo>, String>>,
    /// A model list is being fetched for this key.
    pub fetching: Option<String>,
    /// What Check found for a CLI: its path and version, or why not.
    pub detected: HashMap<&'static str, Result<(String, String), String>>,
    /// Whether a key is stored, per slot, as last asked; and the last problem with the store.
    pub keys: HashMap<String, bool>,
    pub key_error: Option<String>,
    /// The panel's clickable places, as last drawn, and the one under the pointer.
    pub hits: Vec<(Hit, Rect)>,
    pub hover: Option<Hit>,
    pub pressed: Option<Hit>,
    /// Raised by every change of what the panels show, for the overlay's repaint.
    pub tick: u64,
    /// The API key typed in Settings and not saved yet. Never written anywhere but the engine.
    pub key_draft: String,
    /// The key slot the draft was typed for: a draft is never saved under another endpoint.
    pub key_draft_slot: Option<String>,
    /// The base URL was edited: its key is looked up once typing pauses.
    pub url_due: Option<std::time::Instant>,
    /// "Other…" was chosen for the model: Settings shows a box for its name.
    pub model_other: bool,
    /// The saved prompts file's time and size when it was last read.
    pub prompts_stamp: Option<Option<(Option<std::time::SystemTime>, u64)>>,
}

impl Ai {
    pub fn running(&self) -> bool {
        self.run.is_some()
            || self.split.is_some()
            || self.sort.is_some()
            || self.pending.values().any(|a| *a == Ask::Note)
    }

    /// Sends a request, starting the engine first if needed. On failure the reason.
    pub fn ask(&mut self, wake: &Waker, ask: Ask, mut req: Value) -> Result<u64, String> {
        if self.client.is_none() {
            self.client = Some(Client::start(wake.clone())?);
        }
        self.next += 1;
        let id = self.next;
        req["id"] = id.into();
        let sent = self.client.as_ref().is_some_and(|c| c.send(req));
        if !sent {
            self.client = None;
            return Err("The AI engine stopped.".into());
        }
        self.pending.insert(id, ask);
        Ok(id)
    }

    /// How many requests are waited for.
    #[cfg(test)]
    pub fn waiting(&self) -> usize {
        self.pending.len()
    }

    /// What request `id` was for, once; `None` for one already answered or cancelled.
    pub fn answered(&mut self, id: u64, last: bool) -> Option<Ask> {
        if last {
            self.pending.remove(&id)
        } else {
            self.pending.get(&id).cloned()
        }
    }

    pub fn poll(&mut self) -> Vec<Event> {
        let events = self.client.as_ref().map(Client::poll).unwrap_or_default();
        if events.iter().any(|e| matches!(e, Event::Ended(_))) {
            self.client = None;
            self.pending.clear();
            self.fetching = None;
        }
        events
    }

    pub fn cancel(&mut self) {
        if let Some(run) = self.run.take() {
            self.pending.remove(&run.id);
            if let Some(c) = &self.client {
                c.send(json!({"op": "cancel", "id": run.id}));
            }
        }
        self.cancel_split();
        self.cancel_sort();
        self.tick += 1;
    }

    /// Stops a folder sort in flight, every request in it cancelled.
    pub fn cancel_sort(&mut self) {
        if let Some(run) = self.sort.take() {
            for (id, _) in run.in_flight {
                self.pending.remove(&id);
                if let Some(c) = &self.client {
                    c.send(json!({"op": "cancel", "id": id}));
                }
            }
        }
        self.tick += 1;
    }

    /// Stops a task window's request; its answer, if it still comes, is not read.
    /// Why the running engine cannot be sent pictures (one older than mdedit), once it has said
    /// hello; `None` while none runs or before it has.
    pub fn pictures_refused(&self) -> Option<&'static str> {
        self.client
            .as_ref()
            .filter(|c| c.version() > 0)
            .and_then(Client::pictures_refused)
    }

    pub fn cancel_task(&mut self, id: u64) {
        self.pending.remove(&id);
        if let Some(c) = &self.client {
            c.send(json!({"op": "cancel", "id": id}));
        }
        self.tick += 1;
    }

    /// Stops a split in flight; its answer, if it still comes, is not read.
    pub fn cancel_split(&mut self) {
        if let Some(run) = self.split.take() {
            self.pending.remove(&run.id);
            if let Some(c) = &self.client {
                c.send(json!({"op": "cancel", "id": run.id}));
            }
        }
        self.tick += 1;
    }

    /// Stops the request `id`, whoever it was for: its answer, if it still comes, is not read.
    pub fn cancel_id(&mut self, id: u64) {
        self.pending.remove(&id);
        if let Some(c) = &self.client {
            c.send(json!({"op": "cancel", "id": id}));
        }
        self.tick += 1;
    }

    /// A key request for `slot` is waiting for its answer.
    pub fn pending_key(&self, slot: &str) -> bool {
        self.pending
            .values()
            .any(|a| matches!(a, Ask::Key(s) if s == slot))
    }

    pub fn instruction(&mut self) -> Option<&mut Instruction> {
        match self.panel.as_mut() {
            Some(Panel::Instruction(i)) => Some(i),
            _ => None,
        }
    }

    pub fn review(&mut self) -> Option<&mut Review> {
        match self.panel.as_mut() {
            Some(Panel::Review(r)) => Some(r),
            _ => None,
        }
    }

    pub fn hit_at(&self, x: f32, y: f32) -> Option<Hit> {
        self.hits
            .iter()
            .rev()
            .find(|(_, r)| r.contains(x, y))
            .map(|(h, _)| *h)
    }
}

/// The key of a model list in [`Ai::models`].
pub fn models_key(conn: Connection, base_url: &str) -> String {
    match conn {
        Connection::OpenAi => format!("openai {base_url}"),
        _ => conn.key().to_string(),
    }
}

/// The connection as the engine takes it.
pub fn conn_json(conn: Connection, prefs: &mdedit_core::prefs::Prefs) -> Value {
    match conn {
        Connection::ClaudeCode => json!({"kind": "claude-code", "program": prefs.ai_claude_path}),
        Connection::Codex => json!({"kind": "codex", "program": prefs.ai_codex_path}),
        Connection::ClaudeApi => json!({"kind": "claude-api"}),
        Connection::OpenAi | Connection::Off => json!({
            "kind": "openai",
            "base_url": mdedit_core::ai::base_url(&prefs.ai_preset, prefs.ai_url()),
            "slot": mdedit_core::ai::key_slot(Connection::OpenAi, &prefs.ai_preset, prefs.ai_url()).unwrap_or_default(),
        }),
    }
}

/// Reads the saved prompts at `path`; none when the file is missing.
pub fn read_prompts(path: &Path) -> Vec<SavedPrompt> {
    std::fs::read_to_string(path)
        .map(|t| mdedit_core::ai::parse_prompts(&t))
        .unwrap_or_default()
}

/// Writes the file with its template when it is missing.
pub fn ensure_prompts_file(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, mdedit_core::ai::PROMPTS_TEMPLATE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_events_are_read() {
        assert!(parse_event(r#"{"ev":"hello","version":1}"#).is_none());
        // Plan 6: the greeting's version is read; an engine that says none is version 1.
        assert_eq!(
            hello_version(r#"{"ev":"hello","version":2,"engine":"0.9"}"#),
            Some(2)
        );
        assert_eq!(hello_version(r#"{"ev":"hello"}"#), Some(1));
        assert_eq!(
            hello_version(r#"{"ev":"done","id":1,"text":"\"ev\":\"hello\""}"#),
            None
        );
        assert_eq!(hello_version("not json"), None);
        match parse_event(r#"{"ev":"error","id":4,"kind":"rate","message":"m","retry_after":40}"#) {
            Some(Event::Error {
                id: 4,
                retry_after: Some(40),
                ..
            }) => {}
            e => panic!("{e:?}"),
        }
        match parse_event(
            r#"{"ev":"models","id":2,"list":[{"id":"a","name":"A","efforts":["low","high"]},{"name":"no id"}]}"#,
        ) {
            Some(Event::Models { list, .. }) => {
                assert_eq!(list.len(), 1);
                assert_eq!(list[0].efforts, ["low", "high"]);
            }
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn the_engine_is_looked_for_beside_the_exe() {
        let at = engine_paths(Some(Path::new("/usr/bin/mdedit")));
        assert_eq!(at[0], Path::new("/usr/bin").join(engine_file()));
        assert!(engine_paths(None).is_empty());
    }
}
