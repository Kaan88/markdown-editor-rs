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
use crate::pictures::{Ready, base64};
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
            Ok(Some(st)) if st.success() => break,
            Ok(Some(st)) => {
                let how = st
                    .code()
                    .map_or_else(|| "was stopped".into(), |c| format!("exited with {c}"));
                return Err(Fail::new(
                    "cli",
                    format!(
                        "{} did not run (--version {how}). Check the program in Settings › AI.",
                        cli.name()
                    ),
                ));
            }
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
    job.sending();
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
        let mut b = Vec::new();
        if let Some(e) = stderr {
            let _ = e.take(STDERR_MAX as u64).read_to_end(&mut b);
        }
        stderr_text(b)
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

/// The most of a program's stderr that is read.
const STDERR_MAX: usize = 256 * 1024;

/// What a program wrote to stderr as text. When the read stopped at [`STDERR_MAX`], the line at
/// the cut goes: part of a key there could not be found and hidden (audit 1 round 4, #6).
fn stderr_text(mut b: Vec<u8>) -> String {
    if b.len() >= STDERR_MAX {
        b.truncate(b.iter().rposition(|&c| c == b'\n').unwrap_or(0));
    }
    String::from_utf8_lossy(&b).into_owned()
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

/// Claude Code's arguments. `--tools ""` turns off its own tools and `--strict-mcp-config`, with
/// no `--mcp-config`, the MCP servers of the user's configuration (D-213). With pictures the
/// input is one stream-json line (D-221).
pub fn claude_args(model: &str, effort: &str, system: &str, with_images: bool) -> Vec<String> {
    let mut a: Vec<String> = [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--tools",
        "",
        "--strict-mcp-config",
        "--no-session-persistence",
        "--system-prompt",
        system,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if with_images {
        a.extend(["--input-format".into(), "stream-json".into()]);
    }
    if !model.is_empty() {
        a.extend(["--model".into(), model.into()]);
    }
    if !effort.is_empty() {
        a.extend(["--effort".into(), effort.into()]);
    }
    a
}

/// What Claude Code reads on stdin for a run with pictures: one user message whose content is
/// the pictures, then the text.
pub fn claude_input(prompt: &str, images: &[Ready]) -> String {
    let mut content: Vec<Value> = images
        .iter()
        .map(|r| {
            serde_json::json!({"type": "image", "source": {
                "type": "base64", "media_type": r.mime, "data": base64(&r.bytes)}})
        })
        .collect();
    content.push(serde_json::json!({"type": "text", "text": prompt}));
    let line = serde_json::json!({
        "type": "user",
        "message": {"role": "user", "content": content},
    });
    format!("{line}\n")
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

#[allow(clippy::too_many_arguments)]
pub fn run_claude(
    program: &str,
    model: &str,
    effort: &str,
    system: &str,
    prompt: &str,
    images: &[Ready],
    job: &Job,
    progress: &dyn Fn(usize),
) -> Result<String, Fail> {
    let cli = Cli::ClaudeCode;
    let mut lines = ClaudeLines::default();
    let input = if images.is_empty() {
        prompt.to_string()
    } else {
        claude_input(prompt, images)
    };
    let r = run(
        cli,
        program,
        &claude_args(model, effort, system, !images.is_empty()),
        &input,
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

/// Codex's arguments: one `--image` per picture file, and the prompt on stdin (`-`), after `--`
/// when there are pictures, since `--image` takes several values.
pub fn codex_args(model: &str, effort: &str, images: &[PathBuf]) -> Vec<String> {
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
    for p in images {
        a.extend(["--image".into(), p.to_string_lossy().into_owned()]);
    }
    if !images.is_empty() {
        a.push("--".into());
    }
    // The prompt is read from stdin.
    a.push("-".into());
    a
}

/// The start of the name of a folder that holds a Codex run's pictures.
pub const PICTURES_DIR: &str = "mdedit-ai-pictures-";

/// A folder older than this is a run's that never cleaned up (the engine was killed): the
/// longest a run can take is far below it.
pub const PICTURES_STALE: Duration = Duration::from_secs(6 * 3600);

/// The runs' folders of this engine, counted, so two in one tick of the clock are two (Fable's
/// review of V1, 20).
static PICTURES_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A new private folder under the temp folder with a run's pictures in it, as `1.jpg`, `2.png`…;
/// removed when it is dropped.
pub struct PictureFiles {
    pub dir: PathBuf,
    pub files: Vec<PathBuf>,
}

impl PictureFiles {
    pub fn write(images: &[Ready]) -> Result<PictureFiles, Fail> {
        let temp = std::env::temp_dir();
        sweep(&temp, PICTURES_STALE);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let seq = PICTURES_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = temp.join(format!(
            "{PICTURES_DIR}{}-{nanos}-{seq}",
            std::process::id()
        ));
        let fail = |e: std::io::Error| {
            Fail::new(
                "image",
                format!("The pictures could not be written for Codex: {e}"),
            )
        };
        let b = std::fs::DirBuilder::new();
        #[cfg(unix)]
        let b = {
            use std::os::unix::fs::DirBuilderExt;
            let mut b = b;
            b.mode(0o700);
            b
        };
        b.create(&dir).map_err(fail)?;
        let mut me = PictureFiles {
            dir,
            files: Vec::new(),
        };
        if me.dir.to_string_lossy().contains(',') {
            return Err(Fail::new(
                "image",
                "Codex cannot be sent pictures from a temp folder whose path has a comma in it.",
            ));
        }
        for (i, r) in images.iter().enumerate() {
            let f = me.dir.join(format!("{}.{}", i + 1, r.ext));
            std::fs::write(&f, &r.bytes).map_err(fail)?;
            me.files.push(f);
        }
        Ok(me)
    }
}

impl Drop for PictureFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Removes the picture folders runs left behind (an engine that was killed): those under `temp`
/// named for them and older than `stale`. Called before each Codex run with pictures and, on
/// its own thread, when the engine starts (Fable's review of V1, 18: a killed engine's folder
/// waited for a next Codex run that might never come).
pub fn sweep(temp: &Path, stale: Duration) {
    let Ok(rd) = std::fs::read_dir(temp) else {
        return;
    };
    for e in rd.flatten() {
        let stale = e
            .metadata()
            .ok()
            .filter(|m| m.is_dir())
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age >= stale);
        if stale && e.file_name().to_string_lossy().starts_with(PICTURES_DIR) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
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

#[allow(clippy::too_many_arguments)]
pub fn run_codex(
    program: &str,
    model: &str,
    effort: &str,
    system: &str,
    prompt: &str,
    images: &[Ready],
    job: &Job,
    progress: &dyn Fn(usize),
) -> Result<String, Fail> {
    let cli = Cli::Codex;
    let mut lines = CodexLines::default();
    // Codex takes no system prompt on its command line: the rules go first in the prompt.
    let input = format!("{system}\n\n{prompt}");
    // Kept to the end of the run: the folder goes when this does.
    let pictures = if images.is_empty() {
        None
    } else {
        Some(PictureFiles::write(images)?)
    };
    let files = pictures.as_ref().map_or(&[][..], |p| &p.files[..]);
    let r = run(
        cli,
        program,
        &codex_args(model, effort, files),
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
