//! The Claude API and OpenAI-compatible APIs, over HTTPS with ureq: streamed answers (SSE) and
//! model lists.

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::proto::{Fail, Model};
use crate::{Job, keys, sse};

/// The Claude API version this speaks.
const ANTHROPIC_VERSION: &str = "2026-10-21";
/// The model used when none is chosen.
pub const CLAUDE_DEFAULT_MODEL: &str = "claude-opus-5-5";
/// Output room for a rewrite; lowered to what the model allows when its list says.
const CLAUDE_MAX_TOKENS: u64 = 64_000;
/// The key slot of the Claude API.
pub const CLAUDE_SLOT: &str = "claude-api";
/// The least time between two progress events.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

/// `ANTHROPIC_BASE_URL` points it elsewhere (a proxy, or the tests' stub server).
fn claude_base() -> String {
    std::env::var("ANTHROPIC_BASE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "https://api.anthropic.com".into())
        .trim_end_matches('/')
        .to_string()
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let tls =
            ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier);
        #[cfg(any(windows, target_os = "macos"))]
        let tls = tls.provider(ureq::tls::TlsProvider::NativeTls);
        let tls = tls.build();
        ureq::Agent::config_builder()
            .tls_config(tls)
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(20)))
            // A long thought before the first token is normal with a high effort.
            .timeout_recv_response(Some(Duration::from_secs(600)))
            // A reader left behind by a cancelled or silent run ends with this at the latest.
            .timeout_recv_body(Some(Duration::from_secs(30 * 60)))
            .user_agent(concat!("mdedit-ai/", env!("CARGO_PKG_VERSION")))
            .build()
            .into()
    })
}

/// The most output tokens of each Claude model, from the last model list.
fn claude_limits() -> &'static Mutex<HashMap<String, u64>> {
    static L: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    L.get_or_init(Mutex::default)
}

/// Progress, at most every [`PROGRESS_EVERY`].
struct Progress<'a> {
    f: &'a dyn Fn(usize),
    last: Instant,
}

impl<'a> Progress<'a> {
    fn new(f: &'a dyn Fn(usize)) -> Self {
        Progress {
            f,
            last: Instant::now(),
        }
    }
    fn tick(&mut self, chars: usize) {
        if self.last.elapsed() >= PROGRESS_EVERY {
            self.last = Instant::now();
            (self.f)(chars);
        }
    }
}

/// Nothing from a streaming answer for this long ends the run.
const IDLE: Duration = Duration::from_secs(180);
/// The longest answer taken.
pub const REPLY_MAX: usize = 2 * 1024 * 1024;

fn too_long() -> Fail {
    Fail::new(
        "length",
        "The answer was too long to use, so it was not used. Try a smaller selection.",
    )
}

/// Reads a streaming answer, handing each event's data to `f` with the text so far, until `f`
/// returns false. A cancel ends at once; a silent server, a huge line or reply end with a failure.
fn stream(
    provider: &str,
    resp: ureq::http::Response<ureq::Body>,
    job: &Job,
    f: &mut dyn FnMut(&str) -> (bool, usize),
) -> Result<(), Fail> {
    let mut long = false;
    sse::read(
        resp.into_body().into_reader(),
        &|| job.is_cancelled(),
        IDLE,
        &mut |ev| {
            let (go, len) = f(&ev.data);
            long = len > REPLY_MAX;
            go && !long
        },
    )
    .map_err(|b| match b {
        sse::Broken::Io(e) => Fail::new(
            "network",
            format!("{provider}: the answer was cut off: {e}"),
        ),
        sse::Broken::Idle => Fail::new(
            "network",
            format!(
                "{provider} stopped sending: nothing came for {} minutes.",
                IDLE.as_secs() / 60
            ),
        ),
        sse::Broken::LineTooLong => too_long(),
    })?;
    if job.is_cancelled() {
        return Err(Fail::new("cancelled", "Cancelled."));
    }
    if long {
        return Err(too_long());
    }
    Ok(())
}

fn network_fail(provider: &str, e: ureq::Error) -> Fail {
    Fail::new("network", format!("{provider} could not be reached: {e}"))
}

/// The message in an error body: `{"error": {"message": …}}`, `{"error": "…"}`,
/// `{"message": …}`, or the start of the text.
fn body_message(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        for m in [
            &v["error"]["message"],
            &v["error"],
            &v["message"],
            &v["detail"],
        ] {
            if let Some(s) = m.as_str()
                && !s.is_empty()
            {
                return s.to_string();
            }
        }
    }
    let t = body.trim();
    let mut s: String = t.chars().take(200).collect();
    if s.len() < t.len() {
        s.push('…');
    }
    s
}

/// What an HTTP error status means for the user.
pub fn http_fail(provider: &str, status: u16, retry_after: Option<u64>, body: &str) -> Fail {
    let msg = body_message(body);
    let mut f = match status {
        401 | 403 => Fail::new("auth", format!("{provider} refused the key ({status}).")),
        429 => Fail::new(
            "rate",
            match retry_after {
                Some(s) => format!("Too many requests: {provider} asks to wait {s} seconds."),
                None => format!("Too many requests: {provider} asks to wait."),
            },
        ),
        503 | 529 => Fail::new(
            "rate",
            format!("{provider} is overloaded right now ({status}). Try again in a moment."),
        ),
        404 => Fail::new("not_found", format!("{provider}: {msg}")),
        _ => Fail::new("http", format!("{provider} answered {status}: {msg}")),
    };
    f.retry_after = retry_after;
    f
}

/// Sends a request and returns the response if its status is 2xx, else the failure.
fn checked(
    provider: &str,
    r: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<ureq::http::Response<ureq::Body>, Fail> {
    let mut resp = r.map_err(|e| network_fail(provider, e))?;
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        return Ok(resp);
    }
    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(|s| s.ceil() as u64);
    let mut body = String::new();
    let _ = resp
        .body_mut()
        .as_reader()
        .take(64 * 1024)
        .read_to_string(&mut body);
    Err(http_fail(provider, status, retry_after, &body))
}

fn claude_key() -> Result<String, Fail> {
    keys::get(CLAUDE_SLOT)?.ok_or_else(|| {
        Fail::new(
            "auth",
            "No Claude API key is stored. Enter one in Settings › AI.",
        )
    })
}

/// The body of a Claude API request.
pub fn claude_body(
    model: &str,
    effort: &str,
    system: &str,
    prompt: &str,
    max_tokens: u64,
) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "stream": true,
        "system": system,
        "messages": [{"role": "user", "content": prompt}],
    });
    if !effort.is_empty() {
        body["output_config"] = json!({"effort": effort});
    }
    body
}

pub fn run_claude(
    model: &str,
    effort: &str,
    system: &str,
    prompt: &str,
    job: &Job,
    progress: &dyn Fn(usize),
) -> Result<String, Fail> {
    let key = claude_key()?;
    run_claude_with(&key, model, effort, system, prompt, job, progress)
        .map_err(|f| f.hide(Some(&key)))
}

#[allow(clippy::too_many_arguments)]
fn run_claude_with(
    key: &str,
    model: &str,
    effort: &str,
    system: &str,
    prompt: &str,
    job: &Job,
    progress: &dyn Fn(usize),
) -> Result<String, Fail> {
    const P: &str = "Claude API";
    let model = if model.is_empty() {
        CLAUDE_DEFAULT_MODEL
    } else {
        model
    };
    if claude_limits()
        .lock()
        .map(|l| l.is_empty())
        .unwrap_or(false)
    {
        // Learn the model's output limit once; a failure here shows again in the run itself.
        let _ = models_claude();
    }
    let max_tokens = claude_limits()
        .lock()
        .ok()
        .and_then(|l| l.get(model).copied())
        .filter(|&m| m > 0)
        .map_or(CLAUDE_MAX_TOKENS, |m| m.min(CLAUDE_MAX_TOKENS));
    let body = claude_body(model, effort, system, prompt, max_tokens);
    let resp = checked(
        P,
        agent()
            .post(format!("{}/v1/messages", claude_base()))
            .header("x-api-key", key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .send(body.to_string()),
    )?;
    let mut s = ClaudeStream::default();
    let mut p = Progress::new(progress);
    stream(P, resp, job, &mut |data| {
        let go = s.event(data);
        p.tick(s.chars);
        (go, s.text.len())
    })?;
    s.finish()
}

/// The state of a Claude answer as its events come in.
#[derive(Default)]
pub struct ClaudeStream {
    pub text: String,
    pub chars: usize,
    pub stop: Option<String>,
    pub fail: Option<Fail>,
    pub ended: bool,
}

impl ClaudeStream {
    /// Takes one event's data; false once the answer is over.
    pub fn event(&mut self, data: &str) -> bool {
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            return true;
        };
        match v["type"].as_str().unwrap_or("") {
            "content_block_delta" if v["delta"]["type"] == "text_delta" => {
                let t = v["delta"]["text"].as_str().unwrap_or("");
                self.chars += t.chars().count();
                self.text.push_str(t);
            }
            "message_delta" => {
                if let Some(r) = v["delta"]["stop_reason"].as_str() {
                    self.stop = Some(r.to_string());
                }
            }
            "message_stop" => {
                self.ended = true;
                return false;
            }
            "error" => {
                let kind = v["error"]["type"].as_str().unwrap_or("");
                let msg = v["error"]["message"].as_str().unwrap_or("error");
                self.fail = Some(match kind {
                    "overloaded_error" | "rate_limit_error" => Fail::new(
                        "rate",
                        "Claude API is overloaded right now. Try again in a moment.",
                    ),
                    _ => Fail::new("http", format!("Claude API: {msg}")),
                });
                return false;
            }
            _ => {}
        }
        true
    }

    pub fn finish(self) -> Result<String, Fail> {
        if let Some(f) = self.fail {
            return Err(f);
        }
        match self.stop.as_deref() {
            Some("refusal") => Err(Fail::new(
                "declined",
                "The model declined to rewrite this text.",
            )),
            Some("max_tokens") => Err(Fail::new(
                "length",
                "The answer was cut short by the length limit, so it was not used. Try a smaller selection.",
            )),
            _ if !self.ended => Err(Fail::new(
                "network",
                "Claude API: the answer was cut off before its end.",
            )),
            _ => Ok(self.text),
        }
    }
}

/// The Claude models of the account, newest first as the API lists them.
pub fn models_claude() -> Result<Vec<Model>, Fail> {
    let key = claude_key()?;
    models_claude_with(&key).map_err(|f| f.hide(Some(&key)))
}

fn models_claude_with(key: &str) -> Result<Vec<Model>, Fail> {
    const P: &str = "Claude API";
    let mut list = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let mut url = format!("{}/v1/models?limit=1000", claude_base());
        if let Some(a) = &after {
            url.push_str("&after_id=");
            url.push_str(a);
        }
        let resp = checked(
            P,
            agent()
                .get(&url)
                .header("x-api-key", key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .call(),
        )?;
        let v = read_json(P, resp)?;
        list.extend(parse_claude_models(&v));
        match (v["has_more"].as_bool(), v["last_id"].as_str()) {
            (Some(true), Some(last)) if after.as_deref() != Some(last) => {
                after = Some(last.to_string())
            }
            _ => break,
        }
    }
    if let Ok(mut l) = claude_limits().lock() {
        for m in &list {
            l.insert(m.id.clone(), m.max_tokens);
        }
    }
    Ok(list)
}

/// The effort levels in the Claude API's order.
const CLAUDE_EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

pub fn parse_claude_models(v: &Value) -> Vec<Model> {
    let Some(data) = v["data"].as_array() else {
        return Vec::new();
    };
    data.iter()
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            let name = m["display_name"].as_str().unwrap_or(&id).to_string();
            let efforts = CLAUDE_EFFORTS
                .iter()
                .filter(|e| m["capabilities"]["effort"][**e]["supported"] == true)
                .map(|e| e.to_string())
                .collect();
            Some(Model {
                id,
                name,
                efforts,
                max_tokens: m["max_tokens"].as_u64().unwrap_or(0),
            })
        })
        .collect()
}

fn read_json(provider: &str, resp: ureq::http::Response<ureq::Body>) -> Result<Value, Fail> {
    let mut body = String::new();
    resp.into_body()
        .into_reader()
        .take(16 * 1024 * 1024)
        .read_to_string(&mut body)
        .map_err(|e| Fail::new("network", format!("{provider}: {e}")))?;
    serde_json::from_str(&body).map_err(|_| {
        Fail::new(
            "http",
            format!("{provider} sent something that is not a model list."),
        )
    })
}

/// The provider's name for messages: the host of its base URL.
fn host_of(base: &str) -> String {
    let rest = base.split_once("://").map_or(base, |(_, r)| r);
    let host = rest.split(['/', '?']).next().unwrap_or(rest);
    if host.is_empty() {
        "The server".into()
    } else {
        host.to_string()
    }
}

pub fn openai_body(model: &str, effort: &str, system: &str, prompt: &str) -> Value {
    let mut body = json!({
        "model": model,
        "stream": true,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": prompt},
        ],
    });
    if !effort.is_empty() {
        body["reasoning_effort"] = effort.into();
    }
    body
}

#[allow(clippy::too_many_arguments)]
pub fn run_openai(
    base: &str,
    slot: &str,
    model: &str,
    effort: &str,
    system: &str,
    prompt: &str,
    job: &Job,
    progress: &dyn Fn(usize),
) -> Result<String, Fail> {
    let p = host_of(base);
    if model.is_empty() {
        return Err(Fail::new(
            "bad_request",
            "No model is chosen. Choose one in Settings › AI.",
        ));
    }
    let url = base.trim().trim_end_matches('/');
    if url.is_empty() {
        return Err(Fail::new(
            "bad_request",
            "No base URL is set in Settings › AI.",
        ));
    }
    let key = keys::get(slot)?;
    let hide = |f: Fail| f.hide(key.as_deref());
    let mut req = agent()
        .post(format!("{url}/chat/completions"))
        .header("content-type", "application/json");
    if let Some(key) = &key {
        req = req.header("authorization", &format!("Bearer {key}"));
    }
    let resp = checked(
        &p,
        req.send(openai_body(model, effort, system, prompt).to_string()),
    )
    .map_err(|f| hide(explain_effort(f, effort)))?;
    let mut s = OpenAiStream::default();
    let mut pr = Progress::new(progress);
    stream(&p, resp, job, &mut |data| {
        let go = s.event(data);
        pr.tick(s.chars);
        (go, s.text.len())
    })
    .map_err(hide)?;
    s.finish(&p).map_err(hide)
}

/// A 400 for a request with an effort is most likely the effort: say so.
fn explain_effort(mut f: Fail, effort: &str) -> Fail {
    if !effort.is_empty() && f.kind == "http" && f.message.contains(" 400:") {
        f.message
            .push_str(" This model may not take an effort setting: set Effort to Default.");
    }
    f
}

#[derive(Default)]
pub struct OpenAiStream {
    pub text: String,
    pub chars: usize,
    pub finish: Option<String>,
    pub fail: Option<String>,
    pub ended: bool,
}

impl OpenAiStream {
    pub fn event(&mut self, data: &str) -> bool {
        if data.trim() == "[DONE]" {
            self.ended = true;
            return false;
        }
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            return true;
        };
        if !v["error"].is_null() {
            self.fail = Some(body_message(data));
            return false;
        }
        let c = &v["choices"][0];
        if let Some(t) = c["delta"]["content"].as_str() {
            self.chars += t.chars().count();
            self.text.push_str(t);
        }
        if let Some(r) = c["finish_reason"].as_str() {
            self.finish = Some(r.to_string());
        }
        true
    }

    pub fn finish(self, provider: &str) -> Result<String, Fail> {
        if let Some(m) = self.fail {
            return Err(Fail::new("http", format!("{provider}: {m}")));
        }
        match self.finish.as_deref() {
            Some("length") => Err(Fail::new(
                "length",
                "The answer was cut short by the length limit, so it was not used. Try a smaller selection.",
            )),
            Some("content_filter") => Err(Fail::new(
                "declined",
                "The model declined to rewrite this text.",
            )),
            // Some servers end without [DONE] but with a finish reason.
            Some(_) => Ok(self.text),
            None if self.ended => Ok(self.text),
            None => Err(Fail::new(
                "network",
                format!("{provider}: the answer was cut off before its end."),
            )),
        }
    }
}

pub fn models_openai(base: &str, slot: &str) -> Result<Vec<Model>, Fail> {
    let p = host_of(base);
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err(Fail::new(
            "bad_request",
            "No base URL is set in Settings › AI.",
        ));
    }
    let key = keys::get(slot)?;
    let hide = |f: Fail| f.hide(key.as_deref());
    let mut req = agent().get(format!("{base}/models"));
    if let Some(key) = &key {
        req = req.header("authorization", &format!("Bearer {key}"));
    }
    let v = read_json(&p, checked(&p, req.call()).map_err(hide)?).map_err(hide)?;
    Ok(parse_openai_models(&v))
}

pub fn parse_openai_models(v: &Value) -> Vec<Model> {
    let data = v["data"].as_array().or_else(|| v["models"].as_array());
    let mut list: Vec<Model> = data
        .map(|d| {
            d.iter()
                .filter_map(|m| {
                    let id = m["id"].as_str().or_else(|| m["name"].as_str())?.to_string();
                    let name = m["name"]
                        .as_str()
                        .filter(|n| !n.is_empty())
                        .unwrap_or(&id)
                        .to_string();
                    Some(Model {
                        id,
                        name,
                        efforts: Vec::new(),
                        max_tokens: 0,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    list.sort_by(|a, b| a.id.cmp(&b.id));
    list.dedup_by(|a, b| a.id == b.id);
    list
}

#[cfg(test)]
mod tests;
