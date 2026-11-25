//! mdedit-ai: mdedit's AI engine.
//!
//! mdedit starts it on the first AI command and talks to it over JSON lines: requests on stdin,
//! events on stdout (`proto`). It reaches four kinds of connection: the Claude Code and Codex
//! command-line programs the user has signed in to (`cli`), the Claude API and OpenAI-compatible
//! APIs (`api`). API keys are kept in the system's credential store (`keys`) and never leave this
//! process.
//!
//! It is a separate program so that nothing here (network, TLS, JSON, child processes) can end
//! the editor: if this process fails, mdedit says so and the text stays as it was. It ends when
//! its stdin closes.

mod api;
mod cli;
mod keys;
mod pictures;
mod proto;
mod reply;
mod resize;
mod sse;

use std::collections::HashMap;
use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use proto::{Fail, Out, Request};

/// A run in progress, so `cancel` can stop it.
#[derive(Clone, Default)]
pub struct Job {
    pub cancelled: Arc<AtomicBool>,
    /// The CLI process of a run, killed on cancel.
    pub child: Arc<Mutex<Option<std::process::Child>>>,
    /// Called once, by the sender, as the request leaves: a run with pictures says `sent` then,
    /// and never for a run that failed before anything went (Fable's review of V1, 17).
    pub on_send: Arc<Mutex<Option<OnSend>>>,
}

pub type OnSend = Box<dyn FnOnce() + Send>;

impl Job {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// The request is leaving: what was waiting for it is told, once.
    pub fn sending(&self) {
        let f = self.on_send.lock().ok().and_then(|mut f| f.take());
        if let Some(f) = f {
            f();
        }
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        if let Ok(mut c) = self.child.lock()
            && let Some(c) = c.as_mut()
        {
            let _ = c.kill();
        }
    }
}

type Jobs = Arc<Mutex<HashMap<u64, Job>>>;

fn main() {
    let out = Out::stdout();
    out.send(proto::hello());
    // What a killed engine left of a Codex run's pictures goes now, off the greeting's path.
    spawn(|| cli::sweep(&std::env::temp_dir(), cli::PICTURES_STALE));
    let jobs: Jobs = Arc::default();
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Request>(&line) {
            Ok(req) => handle(req, &out, &jobs),
            Err(e) => out.send(proto::error(
                0,
                &Fail::new(
                    "bad_request",
                    format!("mdedit-ai did not understand a request: {e}"),
                ),
            )),
        }
    }
    // mdedit closed the pipe (it quit or stopped using AI): stop the runs, let the rest (a key
    // being saved) finish for a moment, and end.
    if let Ok(jobs) = jobs.lock() {
        for job in jobs.values() {
            job.cancel();
        }
    }
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while BUSY.load(Ordering::Acquire) > 0 && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Answers being worked on.
static BUSY: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn handle(req: Request, out: &Out, jobs: &Jobs) {
    match req {
        Request::Hello => out.send(proto::hello()),
        Request::Cancel { id } => {
            let job = jobs.lock().ok().and_then(|mut j| j.remove(&id));
            if let Some(job) = job {
                job.cancel();
                out.send(proto::error(id, &Fail::new("cancelled", "Cancelled.")));
            }
        }
        Request::Run {
            id,
            conn,
            model,
            effort,
            system,
            prompt,
            images,
            long_side,
            strip,
        } => {
            let job = Job::default();
            if let Ok(mut j) = jobs.lock() {
                j.insert(id, job.clone());
            }
            let (out, jobs) = (out.clone(), jobs.clone());
            spawn(move || {
                let progress = |chars: usize| out.send(proto::delta(id, chars));
                // The pictures are made ready and checked before anything is sent; `sent` says
                // what goes as it goes: the sender calls `Job::sending` where the request leaves.
                let ready = pictures::for_run(&images, &pictures::options(long_side, strip));
                let r = match ready {
                    Err(f) => Err(f),
                    Ok(_) if job.is_cancelled() => Err(Fail::new("cancelled", "Cancelled.")),
                    Ok(imgs) => {
                        if !imgs.is_empty() {
                            let event = pictures::sent(id, &imgs, system.len() + prompt.len());
                            let out = out.clone();
                            if let Ok(mut f) = job.on_send.lock() {
                                *f = Some(Box::new(move || out.send(event)));
                            }
                        }
                        let (m, e, s, p) = (&model, &effort, &system, &prompt);
                        match &conn {
                            proto::Conn::ClaudeCode { program } => {
                                cli::run_claude(program, m, e, s, p, &imgs, &job, &progress)
                            }
                            proto::Conn::Codex { program } => {
                                cli::run_codex(program, m, e, s, p, &imgs, &job, &progress)
                            }
                            proto::Conn::ClaudeApi => {
                                api::run_claude(m, e, s, p, &imgs, &job, &progress)
                            }
                            proto::Conn::Openai { base_url, slot } => {
                                api::run_openai(base_url, slot, m, e, s, p, &imgs, &job, &progress)
                            }
                        }
                    }
                };
                // A cancelled run was already answered.
                let still = jobs.lock().ok().and_then(|mut j| j.remove(&id)).is_some();
                if !still || job.is_cancelled() {
                    return;
                }
                match r {
                    Ok(text) => out.send(proto::done(id, &reply::unwrap_fence(&text))),
                    Err(f) => out.send(proto::error(id, &f)),
                }
            });
        }
        Request::Prepare {
            id,
            images,
            long_side,
            strip,
        } => {
            let job = Job::default();
            if let Ok(mut j) = jobs.lock() {
                j.insert(id, job.clone());
            }
            let (out, jobs) = (out.clone(), jobs.clone());
            spawn(move || {
                let opts = pictures::options(long_side, strip);
                for (i, p) in images.iter().enumerate() {
                    if job.is_cancelled() {
                        return;
                    }
                    let r = pictures::prepare_one(&p.path, &opts);
                    out.send(pictures::picture_event(id, i, &p.path, &r));
                }
                let still = jobs.lock().ok().and_then(|mut j| j.remove(&id)).is_some();
                if still && !job.is_cancelled() {
                    out.send(proto::prepared(id, images.len()));
                }
            });
        }
        Request::Models { id, conn } => {
            let out = out.clone();
            spawn(move || {
                let r = match &conn {
                    proto::Conn::ClaudeApi => api::models_claude(),
                    proto::Conn::Openai { base_url, slot } => api::models_openai(base_url, slot),
                    proto::Conn::ClaudeCode { .. } | proto::Conn::Codex { .. } => Err(Fail::new(
                        "bad_request",
                        "A command-line program has no model list.",
                    )),
                };
                match r {
                    Ok(list) => out.send(proto::models(id, &list)),
                    Err(f) => out.send(proto::error(id, &f)),
                }
            });
        }
        Request::Detect { id, cli, program } => {
            let out = out.clone();
            spawn(move || match cli::detect(cli, &program) {
                Ok((path, version)) => out.send(proto::detected(id, &path, &version)),
                Err(f) => out.send(proto::error(id, &f)),
            });
        }
        Request::KeySet { id, slot, key } => {
            let out = out.clone();
            spawn(move || match keys::set(&slot, &key) {
                Ok(()) => out.send(proto::key(id, &slot, true)),
                Err(f) => out.send(proto::error(id, &f)),
            });
        }
        Request::KeyClear { id, slot } => {
            let out = out.clone();
            spawn(move || match keys::clear(&slot) {
                Ok(()) => out.send(proto::key(id, &slot, false)),
                Err(f) => out.send(proto::error(id, &f)),
            });
        }
        Request::KeyHas { id, slot } => {
            let out = out.clone();
            spawn(move || match keys::get(&slot) {
                Ok(k) => out.send(proto::key(id, &slot, k.is_some())),
                Err(f) => out.send(proto::error(id, &f)),
            });
        }
    }
}

fn spawn(f: impl FnOnce() + Send + 'static) {
    BUSY.fetch_add(1, Ordering::AcqRel);
    let r = std::thread::Builder::new()
        .name("ai".into())
        .spawn(move || {
            f();
            BUSY.fetch_sub(1, Ordering::AcqRel);
        });
    if r.is_err() {
        BUSY.fetch_sub(1, Ordering::AcqRel);
    }
}
