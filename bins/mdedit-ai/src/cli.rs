//! The local agent programs: Claude Code (`claude`) and Codex (`codex`), run once per rewrite
//! with no tools, so they can only answer with text. They use the user's own sign-in.
//!
//! Both run in the temporary directory, so no project's CLAUDE.md or AGENTS.md is read into the
//! rewrite.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;

use crate::Job;
use crate::proto::{Cli, Fail};

impl Cli {
    fn name(self) -> &'static str {
        match self {
            Cli::ClaudeCode => "Claude Code",
            Cli::Codex => "Codex",
        }
    }
    fn program(self) -> &'static str {
        match self {
            Cli::ClaudeCode => "claude",
            Cli::Codex => "codex",
        }
    }
}

/// The places the installers put the programs, looked in after PATH (a program started from the
/// desktop does not always get the shell's PATH).
fn usual_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    if let Some(h) = &home {
        v.push(h.join(".local").join("bin"));
        v.push(h.join(".claude").join("local"));
        v.push(h.join(".npm-global").join("bin"));
        v.push(h.join(".bun").join("bin"));
        v.push(h.join(".volta").join("bin"));
    }
    if cfg!(windows) {
        if let Some(a) = std::env::var_os("APPDATA") {
            v.push(PathBuf::from(a).join("npm"));
        }
    } else {
        v.push("/opt/homebrew/bin".into());
        v.push("/usr/local/bin".into());
        v.push("/usr/bin".into());
    }
    v
}

fn candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    if cfg!(windows) {
        ["exe", "cmd", "bat"]
            .iter()
            .map(|e| dir.join(format!("{name}.{e}")))
            .collect()
    } else {
        vec![dir.join(name)]
    }
}

/// The program to run: the one set in Settings, or the first found on PATH or in the usual
/// places.
pub fn find(cli: Cli, configured: &str) -> Result<PathBuf, Fail> {
    let configured = configured.trim().trim_matches('"');
    if !configured.is_empty() {
        let p = PathBuf::from(configured);
        return if p.is_file() {
            Ok(p)
        } else {
            Err(Fail::new(
                "not_found",
                format!(
                    "{} was not found at {configured}. Check its program in Settings › AI.",
                    cli.name()
                ),
            ))
        };
    }
    let path_dirs = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default();
    for dir in path_dirs.iter().chain(usual_dirs().iter()) {
        for c in candidates(dir, cli.program()) {
            if c.is_file() {
                return Ok(c);
            }
        }
    }
    Err(Fail::new(
        "not_found",
        format!(
            "{} was not found. Install it, or set its program in Settings › AI.",
            cli.name()
        ),
    ))
}

fn command(program: &Path) -> Command {
    let mut c = Command::new(program);
    c.current_dir(std::env::temp_dir());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashes up for a console program.
        c.creation_flags(0x0800_0000);
    }
    c
}

/// The path and version of the program (`--version`).
pub fn detect(cli: Cli, configured: &str) -> Result<(String, String), Fail> {
    let program = find(cli, configured)?;
    let mut child = command(&program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| start_fail(cli, &program, e))?;
    let mut stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(o) = stdout.as_mut() {
            let _ = o.take(64 * 1024).read_to_string(&mut s);
        }
        s
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Fail::new(
                    "cli",
                    format!("{} did not answer --version.", cli.name()),
                ));
            }
        }
    }
    let out = reader.join().unwrap_or_default();
    let version = out
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string();
    Ok((program.to_string_lossy().into_owned(), version))
}

fn start_fail(cli: Cli, program: &Path, e: std::io::Error) -> Fail {
    Fail::new(
        "not_found",
        format!(
            "{} could not be started ({}): {e}",
            cli.name(),
            program.display()
        ),
    )
}

/// What a run of a CLI gave: its JSON lines are read by `line`, which returns the text so far
/// for progress.
fn run(
    cli: Cli,
    program: &str,
    args: &[String],
    stdin_text: &str,
    job: &Job,
    progress: &dyn Fn(usize),
    line: &mut dyn FnMut(&Value) -> usize,
) -> Result<(std::process::ExitStatus, String), Fail> {
    let program = find(cli, program)?;
    let mut child = command(&program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| start_fail(cli, &program, e))?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    if let Ok(mut c) = job.child.lock() {
        *c = Some(child);
    }
    // The text goes in on its own thread, so a large text cannot block against a full stdout.
    let text = stdin_text.to_string();
    let writer = std::thread::spawn(move || {
        if let Some(mut i) = stdin {
            let _ = i.write_all(text.as_bytes());
        }
    });
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = stderr {
            let _ = e.take(256 * 1024).read_to_string(&mut s);
        }
        s
    });
    let mut last = std::time::Instant::now();
    if let Some(out) = stdout {
        for l in BufReader::new(out).lines() {
            let Ok(l) = l else { break };
            if job.is_cancelled() {
                break;
            }
            if let Ok(v) = serde_json::from_str::<Value>(&l) {
                let chars = line(&v);
                if last.elapsed() >= Duration::from_millis(100) {
                    last = std::time::Instant::now();
                    progress(chars);
                }
            }
        }
    }
    let _ = writer.join();
    let stderr = err_reader.join().unwrap_or_default();
    let status = job
        .child
        .lock()
        .ok()
        .and_then(|mut c| c.take())
        .and_then(|mut c| c.wait().ok());
    match status {
        Some(s) => Ok((s, stderr)),
        None => Err(classify(cli, &tail(&stderr))),
    }
}

/// The last lines of what a program wrote to stderr, for a message.
fn tail(stderr: &str) -> String {
    let lines: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let start = lines.len().saturating_sub(3);
    lines[start..].join(" ")
}

/// A sign-in problem in what a CLI said becomes an `auth` failure.
fn classify(cli: Cli, msg: &str) -> Fail {
    let low = msg.to_lowercase();
    let kind = if [
        "/login",
        "log in",
        "login",
        "not logged",
        "api key",
        "authenticat",
        "unauthorized",
    ]
    .iter()
    .any(|k| low.contains(k))
    {
        "auth"
    } else if low.contains("rate limit")
        || low.contains("usage limit")
        || low.contains("overloaded")
    {
        "rate"
    } else {
        "cli"
    };
    let msg = if msg.trim().is_empty() {
        "it stopped without saying why".to_string()
    } else {
        msg.trim().to_string()
    };
    Fail::new(kind, format!("{}: {msg}", cli.name()))
}

pub fn claude_args(model: &str, effort: &str, system: &str) -> Vec<String> {
    let mut a: Vec<String> = [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--tools",
        "",
        "--no-session-persistence",
        "--system-prompt",
        system,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if !model.is_empty() {
        a.extend(["--model".into(), model.into()]);
    }
    if !effort.is_empty() {
        a.extend(["--effort".into(), effort.into()]);
    }
    a
}

/// Claude Code's stream-json lines: text deltas for progress, then one `result` line.
#[derive(Default)]
pub struct ClaudeLines {
    pub streamed: String,
    pub result: Option<Result<String, String>>,
}

impl ClaudeLines {
    pub fn line(&mut self, v: &Value) -> usize {
        match v["type"].as_str().unwrap_or("") {
            "stream_event" => {
                let e = &v["event"];
                if e["type"] == "content_block_delta" && e["delta"]["type"] == "text_delta" {
                    self.streamed
                        .push_str(e["delta"]["text"].as_str().unwrap_or(""));
                }
            }
            "result" => {
                let text = v["result"].as_str().unwrap_or("").to_string();
                let failed = v["is_error"] == true
                    || v["subtype"]
                        .as_str()
                        .is_some_and(|s| s.starts_with("error"));
                self.result = Some(if failed {
                    let errs = v["errors"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|e| e.as_str())
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .unwrap_or_default();
                    Err(if text.is_empty() { errs } else { text })
                } else {
                    Ok(text)
                });
            }
            _ => {}
        }
        self.streamed.chars().count()
    }
}

pub fn run_claude(
    program: &str,
    model: &str,
    effort: &str,
    system: &str,
    prompt: &str,
    job: &Job,
    progress: &dyn Fn(usize),
) -> Result<String, Fail> {
    let cli = Cli::ClaudeCode;
    let mut lines = ClaudeLines::default();
    let r = run(
        cli,
        program,
        &claude_args(model, effort, system),
        prompt,
        job,
        progress,
        &mut |v| lines.line(v),
    );
    if job.is_cancelled() {
        return Err(Fail::new("cancelled", "Cancelled."));
    }
    let (status, stderr) = r?;
    match lines.result {
        Some(Ok(text)) => Ok(text),
        Some(Err(msg)) => Err(classify(cli, &msg)),
        None => Err(classify(cli, &said(status, &stderr))),
    }
}

/// The program's own words on stderr, or how it ended.
fn said(status: std::process::ExitStatus, stderr: &str) -> String {
    let t = tail(stderr);
    if t.is_empty() {
        format!("it ended ({status}) without an answer")
    } else {
        t
    }
}

pub fn codex_args(model: &str, effort: &str) -> Vec<String> {
    let mut a: Vec<String> = [
        "exec",
        "--json",
        "--skip-git-repo-check",
        "--ephemeral",
        "--sandbox",
        "read-only",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if !model.is_empty() {
        a.extend(["-m".into(), model.into()]);
    }
    if !effort.is_empty() {
        a.extend(["-c".into(), format!("model_reasoning_effort=\"{effort}\"")]);
    }
    // The prompt is read from stdin.
    a.push("-".into());
    a
}

/// Codex's `exec --json` lines: `item.completed` items of type `agent_message` carry the answer;
/// `turn.failed` and `error` the failures.
#[derive(Default)]
pub struct CodexLines {
    pub text: Option<String>,
    pub error: Option<String>,
}

impl CodexLines {
    pub fn line(&mut self, v: &Value) -> usize {
        match v["type"].as_str().unwrap_or("") {
            "item.completed" if v["item"]["type"] == "agent_message" => {
                self.text = v["item"]["text"].as_str().map(str::to_string);
            }
            "turn.failed" => {
                self.error = v["error"]["message"].as_str().map(str::to_string);
            }
            "error" => {
                self.error = v["message"].as_str().map(str::to_string);
            }
            _ => {}
        }
        self.text.as_deref().map_or(0, |t| t.chars().count())
    }
}

pub fn run_codex(
    program: &str,
    model: &str,
    effort: &str,
    system: &str,
    prompt: &str,
    job: &Job,
    progress: &dyn Fn(usize),
) -> Result<String, Fail> {
    let cli = Cli::Codex;
    let mut lines = CodexLines::default();
    // Codex takes no system prompt on its command line: the rules go first in the prompt.
    let input = format!("{system}\n\n{prompt}");
    let r = run(
        cli,
        program,
        &codex_args(model, effort),
        &input,
        job,
        progress,
        &mut |v| lines.line(v),
    );
    if job.is_cancelled() {
        return Err(Fail::new("cancelled", "Cancelled."));
    }
    let (status, stderr) = r?;
    if let Some(e) = lines.error {
        return Err(classify(cli, &e));
    }
    match lines.text {
        Some(t) if status.success() => Ok(t),
        _ => Err(classify(cli, &said(status, &stderr))),
    }
}

#[cfg(test)]
mod tests;
