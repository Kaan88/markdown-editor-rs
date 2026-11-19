//! The wire between mdedit and this engine: one JSON object per line.
//!
//! Requests (`"op"`): `hello`, `detect`, `models`, `run`, `cancel`, `key_set`, `key_clear`,
//! `key_has`. Events (`"ev"`): `hello`, `delta` (characters received so far, for the status
//! strip), `done` (the whole text), `error`, `models`, `detected`, `key`. Every event but `hello`
//! carries the `id` of the request it answers.

use std::io::Write;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::{Value, json};

/// Bumped when a request or event changes shape.
pub const VERSION: u32 = 1;

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
    /// `cancelled`, `keystore`, `bad_request`.
    pub kind: &'static str,
    pub message: String,
    /// Seconds the provider asks to wait (rate limits).
    pub retry_after: Option<u64>,
}

impl Fail {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Fail {
        Fail {
            kind,
            message: message.into(),
            retry_after: None,
        }
    }
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
    let mut v = json!({"ev": "error", "id": id, "kind": f.kind, "message": f.message});
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn errors_carry_the_wait() {
        let mut f = Fail::new("rate", "Too many requests.");
        f.retry_after = Some(40);
        let v = error(5, &f);
        assert_eq!(v["retry_after"], 40);
        assert_eq!(v["kind"], "rate");
    }
}
