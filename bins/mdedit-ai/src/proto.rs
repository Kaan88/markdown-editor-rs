//! The wire between mdedit and this engine: one JSON object per line.
//!
//! Requests (`"op"`): `hello`, `detect`, `models`, `run`, `prepare`, `cancel`, `key_set`,
//! `key_clear`, `key_has`. Events (`"ev"`): `hello`, `sent` (what a run with pictures sends,
//! before it is sent), `delta` (characters received so far, for the status strip), `done` (the
//! whole text), `error`, `models`, `detected`, `key`, `picture` (one per picture of `prepare`)
//! and `prepared` (its end). Every event but `hello` carries the `id` of the request it answers.

use std::io::Write;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{Value, json};

pub use crate::pictures::Picture;

fn yes() -> bool {
    true
}

/// Bumped when a request or event changes shape.
pub const VERSION: u32 = 2;

#[derive(Deserialize, Debug)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Hello,
    Detect {
        id: u64,
        cli: Cli,
        #[serde(default)]
        program: String,
    },
    Models {
        id: u64,
        conn: Conn,
    },
    Run {
        id: u64,
        conn: Conn,
        /// Empty: the provider's or the CLI's default.
        #[serde(default)]
        model: String,
        /// Empty: the model's own default.
        #[serde(default)]
        effort: String,
        system: String,
        prompt: String,
        /// Pictures sent before the text, each with the digest `prepare` gave (version 2).
        #[serde(default)]
        images: Vec<Picture>,
        /// 0: the default long side.
        #[serde(default)]
        long_side: u32,
        #[serde(default = "yes")]
        strip: bool,
    },
    /// The pictures made ready as a run would send them; nothing is sent anywhere.
    Prepare {
        id: u64,
        images: Vec<Picture>,
        #[serde(default)]
        long_side: u32,
        #[serde(default = "yes")]
        strip: bool,
    },
    Cancel {
        id: u64,
    },
    KeySet {
        id: u64,
        slot: String,
        key: String,
    },
    KeyClear {
        id: u64,
        slot: String,
    },
    KeyHas {
        id: u64,
        slot: String,
    },
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Cli {
    ClaudeCode,
    Codex,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Conn {
    /// `program` empty: found on PATH or in the usual places.
    ClaudeCode {
        #[serde(default)]
        program: String,
    },
    Codex {
        #[serde(default)]
        program: String,
    },
    ClaudeApi,
    /// `slot` names the stored key; a local server needs none.
    Openai {
        base_url: String,
        slot: String,
    },
}

/// What went wrong, for the bar mdedit shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fail {
    /// `auth`, `rate`, `declined`, `length`, `not_found`, `network`, `http`, `cli`,
    /// `cancelled`, `keystore`, `bad_request`, `no_vision` (the model reads no pictures),
    /// `image` (a picture could not be made ready; the message names it).
    pub kind: &'static str,
    pub message: String,
    /// Seconds the provider asks to wait (rate limits).
    pub retry_after: Option<u64>,
    /// An HTTP error's status and the server's own words, apart from the host and the status
    /// the message also holds: 0 and empty for any other failure.
    pub status: u16,
    pub said: String,
}

impl Fail {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Fail {
        Fail {
            kind,
            message: message.into(),
            retry_after: None,
            status: 0,
            said: String::new(),
        }
    }

    /// The same failure with `key`, and anything shaped like a key, hidden from its message.
    pub fn hide(mut self, key: Option<&str>) -> Fail {
        self.message = redact(&self.message, key);
        self
    }
}

/// What stands for a hidden key.
pub const HIDDEN: &str = "[key hidden]";

/// Words that start a key of a known provider: such a word of 20 characters or more is hidden.
const KEY_PREFIXES: &[&str] = &[
    "sk-",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "xai-",
    "gsk_",
    "AIza",
];

/// Whether the word after `Bearer` or `x-api-key:` looks like a credential rather than prose:
/// long, and with a digit unless it is very long.
fn looks_like_key(word: &str) -> bool {
    let n = word.chars().count();
    n >= 32 || (n >= 16 && word.chars().any(|c| c.is_ascii_digit()))
}

/// Whether a word that starts like a key has the random part of one: a run of 16 characters or
/// more between separators, or 40 characters in all with capitals, small letters and digits
/// mixed. A model's name (`xai-grok-4-fast-reasoning-production-west`) is short words joined
/// (audit 1 round 3, #12; round 4, #10).
fn random_part(word: &str) -> bool {
    let mixed = word.chars().any(|c| c.is_ascii_uppercase())
        && word.chars().any(|c| c.is_ascii_lowercase())
        && word.chars().any(|c| c.is_ascii_digit());
    word.len() >= 40 && mixed
        || word
            .split(['-', '_', '.'])
            .any(|part| part.chars().count() >= 16)
}

/// `text` with no key in it: `key` itself (any key mdedit stores: 4 characters or more), a
/// credential-looking word after `Bearer ` or `x-api-key:`, and a word of 20 characters or more
/// that starts like a known provider's key. Servers sometimes echo what they were sent.
pub fn redact(text: &str, key: Option<&str>) -> String {
    let s = match key
        .map(str::trim)
        .filter(|k| k.chars().count() >= crate::keys::KEY_MIN)
    {
        Some(k) => text.replace(k, HIDDEN),
        None => text.to_string(),
    };
    let tok = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
    let lower = s.to_ascii_lowercase();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut prev_tok = false;
    while i < s.len() {
        let rest = &lower[i..];
        // A marker, then separators, then the token it names.
        let marker = ["bearer ", "x-api-key"]
            .into_iter()
            .find(|m| rest.starts_with(m));
        if let Some(m) = marker.filter(|_| !prev_tok) {
            let mut j = i + m.len();
            let sep = s[j..]
                .find(|c: char| !matches!(c, ' ' | ':' | '=' | '"' | '\''))
                .unwrap_or(s.len() - j);
            j += sep;
            let end = j + s[j..].find(|c: char| !tok(c)).unwrap_or(s.len() - j);
            if looks_like_key(&s[j..end]) && (m == "bearer " || sep > 0) {
                out.push_str(&s[i..j]);
                out.push_str(HIDDEN);
                i = end;
                prev_tok = false;
                continue;
            }
        }
        if !prev_tok && KEY_PREFIXES.iter().any(|p| s[i..].starts_with(p)) {
            let end = i + s[i..].find(|c: char| !tok(c)).unwrap_or(s.len() - i);
            if end - i >= 20 && random_part(&s[i..end]) {
                out.push_str(HIDDEN);
                i = end;
                prev_tok = false;
                continue;
            }
        }
        let c = s[i..].chars().next().unwrap_or(' ');
        out.push(c);
        prev_tok = tok(c);
        i += c.len_utf8();
    }
    out
}

/// Stdout, shared by the threads that answer.
#[derive(Clone)]
pub struct Out(Arc<Mutex<Box<dyn Write + Send>>>);

impl Out {
    pub fn stdout() -> Out {
        Out(Arc::new(Mutex::new(Box::new(std::io::stdout()))))
    }

    pub fn send(&self, v: Value) {
        if let Ok(mut w) = self.0.lock() {
            let _ = writeln!(w, "{v}");
            let _ = w.flush();
        }
    }
}

pub fn hello() -> Value {
    json!({"ev": "hello", "version": VERSION, "engine": env!("CARGO_PKG_VERSION")})
}

pub fn delta(id: u64, chars: usize) -> Value {
    json!({"ev": "delta", "id": id, "chars": chars})
}

pub fn done(id: u64, text: &str) -> Value {
    json!({"ev": "done", "id": id, "text": text})
}

pub fn error(id: u64, f: &Fail) -> Value {
    // Whatever the failure came from, a key-shaped word never leaves the engine.
    let message = redact(&f.message, None);
    let mut v = json!({"ev": "error", "id": id, "kind": f.kind, "message": message});
    if let Some(s) = f.retry_after {
        v["retry_after"] = s.into();
    }
    v
}

/// A model a connection offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub id: String,
    pub name: String,
    /// The effort levels it takes, lowest first; empty when the provider does not say.
    pub efforts: Vec<String>,
    /// The most output tokens it allows; 0 when the provider does not say.
    pub max_tokens: u64,
}

pub fn models(id: u64, list: &[Model]) -> Value {
    let list: Vec<Value> = list
        .iter()
        .map(|m| json!({"id": m.id, "name": m.name, "efforts": m.efforts, "max_tokens": m.max_tokens}))
        .collect();
    json!({"ev": "models", "id": id, "list": list})
}

pub fn detected(id: u64, path: &str, version: &str) -> Value {
    json!({"ev": "detected", "id": id, "path": path, "version": version})
}

pub fn key(id: u64, slot: &str, stored: bool) -> Value {
    json!({"ev": "key", "id": id, "slot": slot, "stored": stored})
}

pub fn prepared(id: u64, count: usize) -> Value {
    json!({"ev": "prepared", "id": id, "count": count})
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Audit 2: `Fail::hide` hides a key with no known prefix by the key itself.
    #[test]
    fn a_fail_hides_its_own_key() {
        let k = "Zq9wPlmX3rT7uVbN";
        let f = Fail {
            kind: "http",
            message: format!("the value {k} was refused"),
            retry_after: None,
            status: 0,
            said: String::new(),
        }
        .hide(Some(k));
        assert_eq!(f.message, format!("the value {HIDDEN} was refused"));
    }

    #[test]
    fn keys_are_hidden() {
        let k = "abcd1234efgh5678";
        assert_eq!(
            redact(&format!("bad key {k}."), Some(k)),
            "bad key [key hidden]."
        );
        assert_eq!(
            redact("Authorization: Bearer xyzxyzxyz1234567890 refused", None),
            "Authorization: Bearer [key hidden] refused"
        );
        assert_eq!(
            redact(r#"{"x-api-key": "zz12345678901234567"}"#, None),
            r#"{"x-api-key": "[key hidden]"}"#
        );
        assert_eq!(
            redact("got sk-proj-0123456789abcdefghij, and sk-short", None),
            "got [key hidden], and sk-short"
        );
        assert_eq!(
            redact("token ghp_0123456789abcdefghijABCDEFGHIJ1234 leaked", None),
            "token [key hidden] leaked"
        );
        assert_eq!(
            redact("echo: q7Zp-x said no", Some("q7Zp-x")),
            "echo: [key hidden] said no"
        );
        // Audit 1, round 2: guidance about authentication is not a key.
        let prose = "Bearer authentication is required. Send an x-api-key: header.";
        assert_eq!(redact(prose, None), prose);
        // Ordinary words stay.
        let plain = "invalid x-api-key; the bearer of bad news — überall";
        assert_eq!(redact(plain, None), plain);
        assert_eq!(
            redact("task-0123456789abcdefghijkl", None),
            "task-0123456789abcdefghijkl"
        );
        // Audit 1 round 3, #12: a model's name is not a key; a key of that provider is.
        // Audit 1 round 4, #10: long, but short words joined.
        let long = "Unknown model: xai-grok-4-fast-reasoning-production-west";
        assert_eq!(redact(long, None), long);
        let alias = "Unknown model alias: xai-grok-4-fast-reasoning";
        assert_eq!(redact(alias, None), alias);
        assert_eq!(
            redact("key xai-Zq81LmN0pQ7rS2tU9vW4xY6z rejected", None),
            "key [key hidden] rejected"
        );
    }

    #[test]
    fn requests_parse() {
        let r: Request = serde_json::from_str(
            r#"{"op":"run","id":3,"conn":{"kind":"openai","base_url":"http://x/v1","slot":"openai:ollama"},"model":"m","system":"s","prompt":"p"}"#,
        )
        .unwrap();
        match r {
            Request::Run {
                id, conn, effort, ..
            } => {
                assert_eq!(id, 3);
                assert_eq!(effort, "");
                assert!(matches!(conn, Conn::Openai { .. }));
            }
            _ => panic!(),
        }
        let r: Request =
            serde_json::from_str(r#"{"op":"detect","id":1,"cli":"claude-code"}"#).unwrap();
        assert!(matches!(
            r,
            Request::Detect {
                cli: Cli::ClaudeCode,
                ..
            }
        ));
        let r: Request =
            serde_json::from_str(r#"{"op":"models","id":2,"conn":{"kind":"claude-api"}}"#).unwrap();
        assert!(matches!(
            r,
            Request::Models {
                conn: Conn::ClaudeApi,
                ..
            }
        ));
    }

    /// Version 2: a run names its pictures; an older request without them still parses.
    #[test]
    fn a_run_names_its_pictures() {
        assert_eq!(VERSION, 2);
        assert_eq!(hello()["version"], 2);
        let r: Request = serde_json::from_str(
            r#"{"op":"run","id":9,"conn":{"kind":"claude-api"},"system":"s","prompt":"p","images":[{"path":"C:\\a\\IMG.jpg","sha256":"ab"}],"long_side":2576,"strip":false}"#,
        )
        .unwrap();
        let Request::Run {
            images,
            long_side,
            strip,
            ..
        } = r
        else {
            panic!()
        };
        assert_eq!(images[0].path, r"C:\a\IMG.jpg");
        assert_eq!(images[0].sha256, "ab");
        assert_eq!((long_side, strip), (2576, false));
        let r: Request = serde_json::from_str(
            r#"{"op":"run","id":1,"conn":{"kind":"claude-api"},"system":"s","prompt":"p"}"#,
        )
        .unwrap();
        assert!(
            matches!(r, Request::Run { ref images, long_side: 0, strip: true, .. } if images.is_empty())
        );
        let r: Request =
            serde_json::from_str(r#"{"op":"prepare","id":2,"images":[{"path":"x.png"}]}"#).unwrap();
        assert!(matches!(
            r,
            Request::Prepare {
                id: 2,
                strip: true,
                ..
            }
        ));
    }

    #[test]
    fn errors_carry_the_wait() {
        let mut f = Fail::new("rate", "Too many requests.");
        f.retry_after = Some(40);
        let v = error(5, &f);
        assert_eq!(v["retry_after"], 40);
        assert_eq!(v["kind"], "rate");
    }
}
