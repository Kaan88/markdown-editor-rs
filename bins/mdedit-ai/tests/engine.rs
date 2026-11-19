//! The engine as mdedit runs it: JSON lines in and out, against a stub HTTP server on localhost
//! and fake `claude` and `codex` programs.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value, json};

/// A request the stub server saw.
#[derive(Debug)]
struct Seen {
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Answers each request with `answer(path)`: status, extra headers, body. Returns its base URL
/// and the requests it saw.
fn stub(
    answer: impl Fn(&str) -> (u16, Vec<(&'static str, String)>, String) + Send + 'static,
) -> (String, mpsc::Receiver<Seen>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(s) = s else { break };
            let Some(seen) = read_request(&s) else {
                continue;
            };
            let (status, headers, body) = answer(&seen.path);
            let mut w = &s;
            let mut head = format!(
                "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n",
                body.len()
            );
            for (k, v) in headers {
                head.push_str(&format!("{k}: {v}\r\n"));
            }
            head.push_str("\r\n");
            let _ = w.write_all(head.as_bytes());
            let _ = w.write_all(body.as_bytes());
            let _ = tx.send(seen);
        }
    });
    (base, rx)
}

fn read_request(s: &TcpStream) -> Option<Seen> {
    let mut r = BufReader::new(s);
    let mut line = String::new();
    r.read_line(&mut line).ok()?;
    let path = line.split_whitespace().nth(1)?.to_string();
    let mut headers = Vec::new();
    let mut len = 0;
    loop {
        line.clear();
        r.read_line(&mut line).ok()?;
        let l = line.trim_end();
        if l.is_empty() {
            break;
        }
        let (k, v) = l.split_once(':')?;
        if k.eq_ignore_ascii_case("content-length") {
            len = v.trim().parse().ok()?;
        }
        headers.push((k.to_string(), v.trim().to_string()));
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body).ok()?;
    Some(Seen {
        path,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn sse(events: &[Value]) -> String {
    events
        .iter()
        .map(|e| format!("event: {}\ndata: {e}\n\n", e["type"].as_str().unwrap_or("")))
        .collect()
}

struct Engine {
    child: Child,
    input: Option<ChildStdin>,
    events: mpsc::Receiver<Value>,
}

impl Engine {
    fn start(env: &[(&str, &str)]) -> Engine {
        let mut c = Command::new(env!("CARGO_BIN_EXE_mdedit-ai"));
        c.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for k in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
        ] {
            c.env_remove(k);
        }
        c.env("NO_PROXY", "*").env("MDEDIT_AI_KEY", "test-key");
        for (k, v) in env {
            c.env(k, v);
        }
        let mut child = c.spawn().unwrap();
        let input = child.stdin.take().unwrap();
        let out: ChildStdout = child.stdout.take().unwrap();
        let (tx, events) = mpsc::channel();
        std::thread::spawn(move || {
            for l in BufReader::new(out).lines() {
                let Ok(l) = l else { break };
                if tx.send(serde_json::from_str::<Value>(&l).unwrap()).is_err() {
                    break;
                }
            }
        });
        let e = Engine {
            child,
            input: Some(input),
            events,
        };
        let hello = e.next();
        assert_eq!(hello["ev"], "hello");
        assert_eq!(hello["version"], 1);
        e
    }

    fn send(&mut self, v: Value) {
        let i = self.input.as_mut().unwrap();
        writeln!(i, "{v}").unwrap();
        i.flush().unwrap();
    }

    fn next(&self) -> Value {
        self.events
            .recv_timeout(Duration::from_secs(20))
            .expect("an event")
    }

    /// The first event that is not progress.
    fn answer(&self) -> Value {
        loop {
            let e = self.next();
            if e["ev"] != "delta" {
                return e;
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn run(conn: Value, model: &str, effort: &str) -> Value {
    json!({"op": "run", "id": 7, "conn": conn, "model": model, "effort": effort,
           "system": "Rewrite as Markdown.", "prompt": "trip notes"})
}

#[test]
fn claude_api_streams_an_answer() {
    let (base, seen) = stub(|path| {
        match path {
        p if p.starts_with("/v1/models") => (
            200,
            vec![],
            json!({"data": [{"id": "claude-opus-5-5", "display_name": "Claude Opus 5.5", "max_tokens": 32000,
                             "capabilities": {"effort": {"high": {"supported": true}}}}],
                   "has_more": false})
            .to_string(),
        ),
        _ => (
            200,
            vec![("content-type", "text/event-stream".into())],
            sse(&[
                json!({"type": "message_start", "message": {}}),
                json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "```markdown\n# Trip"}}),
                json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": " notes\n```"}}),
                json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}}),
                json!({"type": "message_stop"}),
            ]),
        ),
    }
    });
    let mut e = Engine::start(&[("ANTHROPIC_BASE_URL", &base)]);
    e.send(run(json!({"kind": "claude-api"}), "", "high"));
    let a = e.answer();
    assert_eq!(a["ev"], "done", "{a}");
    assert_eq!(a["id"], 7);
    // The fence around the whole answer is taken off.
    assert_eq!(a["text"], "# Trip notes\n");

    // The model list was read first, for the model's output limit.
    let models = seen.recv().unwrap();
    assert!(models.path.starts_with("/v1/models"));
    let msg = seen.recv().unwrap();
    assert_eq!(msg.path, "/v1/messages");
    assert_eq!(msg.header("x-api-key"), Some("test-key"));
    assert_eq!(msg.header("anthropic-version"), Some("2026-10-21"));
    let body: Value = serde_json::from_str(&msg.body).unwrap();
    assert_eq!(body["model"], "claude-opus-5-5");
    assert_eq!(body["max_tokens"], 32000);
    assert_eq!(body["output_config"]["effort"], "high");
    assert_eq!(body["system"], "Rewrite as Markdown.");
    assert_eq!(body["messages"][0]["content"], "trip notes");
}

/// Audit 1 round 4, #7: a run cancelled while the model list is being read sends nothing more:
/// no further page of the list, no rewrite. The first page's body is held back past the cancel
/// and says more pages follow.
#[test]
fn a_cancel_during_the_model_list_asks_for_nothing_more() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let (tx, seen) = mpsc::channel();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(s) = s else { break };
            let Some(req) = read_request(&s) else {
                continue;
            };
            let _ = tx.send(req.path.clone());
            let body = json!({"data": [{"id": "m1", "max_tokens": 1000}],
                              "has_more": true, "last_id": "m1"})
            .to_string();
            let mut w = &s;
            let head = format!(
                "HTTP/1.1 200 OK
content-length: {}
connection: close

",
                body.len()
            );
            let _ = w.write_all(head.as_bytes());
            let _ = w.write_all(&body.as_bytes()[..10]);
            let _ = w.flush();
            std::thread::sleep(Duration::from_millis(1500));
            let _ = w.write_all(&body.as_bytes()[10..]);
        }
    });
    let mut e = Engine::start(&[("ANTHROPIC_BASE_URL", &base)]);
    e.send(run(json!({"kind": "claude-api"}), "m1", ""));
    let first = seen.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(first.starts_with("/v1/models"), "{first}");
    std::thread::sleep(Duration::from_millis(200));
    e.send(json!({"op": "cancel", "id": 7}));
    let a = e.answer();
    assert_eq!(a["kind"], "cancelled", "{a}");
    // The held body goes out; nothing is asked after it, before or after the answer.
    std::thread::sleep(Duration::from_millis(2500));
    let more: Vec<String> = seen.try_iter().collect();
    assert!(more.is_empty(), "asked after the cancel: {more:?}");
}

#[test]
fn claude_api_errors_are_explained() {
    let (base, _seen) = stub(|path| {
        if path.starts_with("/v1/models") {
            (401, vec![], r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#.into())
        } else {
            (
                429,
                vec![("retry-after", "40".into())],
                r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#
                    .into(),
            )
        }
    });
    let mut e = Engine::start(&[("ANTHROPIC_BASE_URL", &base)]);
    e.send(json!({"op": "models", "id": 1, "conn": {"kind": "claude-api"}}));
    let a = e.answer();
    assert_eq!(
        (a["ev"].as_str(), a["kind"].as_str()),
        (Some("error"), Some("auth")),
        "{a}"
    );
    assert_eq!(a["message"], "Claude API refused the key (401).");
    e.send(run(json!({"kind": "claude-api"}), "claude-opus-5-5", ""));
    let a = e.answer();
    assert_eq!(a["kind"], "rate", "{a}");
    assert_eq!(a["retry_after"], 40);
}

#[test]
fn claude_api_refusals_are_not_applied() {
    let (base, _seen) = stub(|path| {
        if path.starts_with("/v1/models") {
            (200, vec![], r#"{"data":[]}"#.into())
        } else {
            (
                200,
                vec![],
                sse(&[
                    json!({"type": "message_delta", "delta": {"stop_reason": "refusal"}}),
                    json!({"type": "message_stop"}),
                ]),
            )
        }
    });
    let mut e = Engine::start(&[("ANTHROPIC_BASE_URL", &base)]);
    e.send(run(json!({"kind": "claude-api"}), "", ""));
    assert_eq!(e.answer()["kind"], "declined");
}

#[test]
fn openai_compatible_streams_and_lists_models() {
    let (base, seen) = stub(|path| match path {
        "/v1/models" => (
            200,
            vec![],
            r#"{"data":[{"id":"llama-4"},{"id":"gemma-3"}]}"#.into(),
        ),
        _ => (
            200,
            vec![],
            [
                r#"{"choices":[{"delta":{"content":"- a"}}]}"#,
                r#"{"choices":[{"delta":{"content":"\n- b"},"finish_reason":"stop"}]}"#,
                "[DONE]",
            ]
            .iter()
            .map(|d| format!("data: {d}\n\n"))
            .collect(),
        ),
    });
    let mut e = Engine::start(&[]);
    let conn =
        json!({"kind": "openai", "base_url": format!("{base}/v1/"), "slot": "openai:ollama"});
    e.send(json!({"op": "models", "id": 2, "conn": conn}));
    let m = e.answer();
    assert_eq!(m["ev"], "models", "{m}");
    assert_eq!(m["list"][0]["id"], "gemma-3");
    e.send(run(conn, "llama-4", "low"));
    let a = e.answer();
    assert_eq!(a["text"], "- a\n- b", "{a}");
    let _ = seen.recv().unwrap();
    let chat = seen.recv().unwrap();
    assert_eq!(chat.path, "/v1/chat/completions");
    assert_eq!(chat.header("authorization"), Some("Bearer test-key"));
    let body: Value = serde_json::from_str(&chat.body).unwrap();
    assert_eq!(body["reasoning_effort"], "low");
    assert_eq!(body["messages"][1]["content"], "trip notes");
}

#[test]
fn a_key_echoed_by_the_server_is_hidden() {
    const KEY: &str = "sk-ant-api03-AbCdEfGhIjKlMnOpQrStUv";
    let (base, _seen) = stub(|path| {
        if path.starts_with("/v1/models") {
            (200, vec![], r#"{"data":[]}"#.into())
        } else if path.contains("messages") {
            (
                200,
                vec![],
                sse(&[
                    json!({"type": "error", "error": {"type": "invalid_request_error",
                    "message": format!("bad header x-api-key: {KEY}")}}),
                ]),
            )
        } else {
            (
                400,
                vec![],
                format!(r#"{{"error":{{"message":"Bearer {KEY} is not valid"}}}}"#),
            )
        }
    });
    let mut e = Engine::start(&[("ANTHROPIC_BASE_URL", &base), ("MDEDIT_AI_KEY", KEY)]);
    e.send(run(json!({"kind": "claude-api"}), "claude-opus-5-5", ""));
    let a = e.answer();
    assert_eq!(a["kind"], "http", "{a}");
    let m = a["message"].as_str().unwrap();
    assert!(!m.contains("AbCdEf") && m.contains("[key hidden]"), "{m}");
    let conn = json!({"kind": "openai", "base_url": format!("{base}/v1"), "slot": "openai:x"});
    e.send(run(conn, "m", ""));
    let a = e.answer();
    let m = a["message"].as_str().unwrap();
    assert!(!m.contains("AbCdEf") && m.contains("[key hidden]"), "{m}");
}

/// Audit 1, round 2: an opaque key echoed where a long plain-text error is cut for the message
/// leaves no fragment; prose about authentication stays readable.
#[test]
fn a_key_at_the_cut_of_a_long_error_leaves_no_fragment() {
    const KEY: &str = "Zq9wPlmX3rT7";
    let (base, _seen) = stub(|path| {
        if path.ends_with("/models") {
            (401, vec![], "Bearer authentication is required.".into())
        } else {
            (
                500,
                vec![],
                format!("{}{KEY} trailing words here", "e".repeat(194)),
            )
        }
    });
    let mut e = Engine::start(&[("MDEDIT_AI_KEY", KEY)]);
    let conn = json!({"kind": "openai", "base_url": format!("{base}/v1"), "slot": "openai:x"});
    e.send(run(conn.clone(), "m", ""));
    let a = e.answer();
    let m = a["message"].as_str().unwrap();
    assert!(!m.contains("Zq9w") && !m.contains("mX3r"), "{m}");
    // The cut falls at a word's end, inside the marker; the key is what must not be there.
    assert!(m.contains("[key"), "{m}");
    e.send(json!({"op": "models", "id": 3, "conn": conn}));
    let a = e.answer();
    assert_eq!(a["kind"], "auth", "{a}");
}

#[test]
fn a_huge_reply_is_not_used() {
    let (base, _seen) = stub(|_| {
        let chunk = "x".repeat(64 * 1024);
        let line = format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{chunk}\"}}}}]}}\n\n");
        (200, vec![], line.repeat(48))
    });
    let mut e = Engine::start(&[]);
    let conn = json!({"kind": "openai", "base_url": format!("{base}/v1"), "slot": "openai:x"});
    e.send(run(conn, "m", ""));
    let a = e.answer();
    assert_eq!(a["kind"], "length", "{a}");
}

#[test]
fn an_unreachable_server_is_a_network_failure() {
    // A port nothing listens on.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut e = Engine::start(&[]);
    let conn =
        json!({"kind": "openai", "base_url": format!("http://127.0.0.1:{port}/v1"), "slot": "x"});
    e.send(run(conn, "m", ""));
    let a = e.answer();
    assert_eq!(a["kind"], "network", "{a}");
}

#[test]
fn a_bad_line_is_answered_and_the_engine_goes_on() {
    let mut e = Engine::start(&[]);
    e.send(json!({"op": "fly"}));
    assert_eq!(e.answer()["kind"], "bad_request");
    e.send(json!({"op": "key_has", "id": 3, "slot": "claude-api"}));
    let k = e.answer();
    assert_eq!(
        (k["ev"].as_str(), k["stored"].as_bool()),
        (Some("key"), Some(true))
    );
}

#[test]
fn the_engine_ends_when_its_input_closes() {
    let mut e = Engine::start(&[]);
    e.input = None;
    let status = e.child.wait().unwrap();
    assert!(status.success());
}

#[cfg(unix)]
mod fake_cli {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A shell script standing in for a CLI: it saves its arguments and stdin beside itself,
    /// then prints `out`.
    fn script(name: &str, out: &str, exit: i32) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mdedit-ai-test-{}-{name}-{exit}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        let body = format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then echo '9.9.9 (fake)'; exit 0; fi\nprintf '%s\\n' \"$@\" > \"{d}/args\"\ncat > \"{d}/stdin\"\ncat <<'EOF'\n{out}\nEOF\nexit {exit}\n",
            d = dir.display()
        );
        std::fs::write(&p, body).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn claude_code_runs_and_answers() {
        let out = [
            json!({"type": "system", "subtype": "init"}),
            json!({"type": "stream_event", "event": {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "# T"}}}),
            json!({"type": "result", "subtype": "success", "is_error": false, "result": "# T"}),
        ]
        .map(|v| v.to_string())
        .join("\n");
        let p = script("claude", &out, 0);
        let mut e = Engine::start(&[]);
        e.send(json!({"op": "detect", "id": 1, "cli": "claude-code", "program": p}));
        let d = e.answer();
        assert_eq!(d["version"], "9.9.9 (fake)", "{d}");
        e.send(run(
            json!({"kind": "claude-code", "program": p}),
            "sonnet",
            "high",
        ));
        let a = e.answer();
        assert_eq!(a["text"], "# T", "{a}");
        let dir = p.parent().unwrap();
        let args = std::fs::read_to_string(dir.join("args")).unwrap();
        assert!(
            args.contains("--system-prompt\nRewrite as Markdown.\n"),
            "{args}"
        );
        assert!(args.contains("--model\nsonnet\n--effort\nhigh"), "{args}");
        assert_eq!(
            std::fs::read_to_string(dir.join("stdin")).unwrap(),
            "trip notes"
        );
    }

    #[test]
    fn claude_code_not_signed_in_is_an_auth_failure() {
        let out = json!({"type": "result", "subtype": "success", "is_error": true, "result": "Invalid API key · Please run /login"}).to_string();
        let p = script("claude", &out, 1);
        let mut e = Engine::start(&[]);
        e.send(run(json!({"kind": "claude-code", "program": p}), "", ""));
        let a = e.answer();
        assert_eq!(a["kind"], "auth", "{a}");
        assert!(
            a["message"]
                .as_str()
                .unwrap()
                .starts_with("Claude Code: Invalid API key")
        );
    }

    #[test]
    fn codex_runs_and_answers() {
        let out = [
            json!({"type": "thread.started", "thread_id": "t"}),
            json!({"type": "item.completed", "item": {"id": "i", "type": "agent_message", "text": "- [ ] Call Ayşe"}}),
            json!({"type": "turn.completed", "usage": {}}),
        ]
        .map(|v| v.to_string())
        .join("\n");
        let p = script("codex", &out, 0);
        let mut e = Engine::start(&[]);
        e.send(run(json!({"kind": "codex", "program": p}), "", "low"));
        let a = e.answer();
        assert_eq!(a["text"], "- [ ] Call Ayşe", "{a}");
        let dir = p.parent().unwrap();
        // The rules go first in the prompt, which comes on stdin.
        assert_eq!(
            std::fs::read_to_string(dir.join("stdin")).unwrap(),
            "Rewrite as Markdown.\n\ntrip notes"
        );
        let args = std::fs::read_to_string(dir.join("args")).unwrap();
        assert!(
            args.ends_with("model_reasoning_effort=\"low\"\n-\n"),
            "{args}"
        );
    }

    #[test]
    fn a_cli_whose_version_fails_is_not_found() {
        let dir = std::env::temp_dir().join(format!("mdedit-ai-test-{}-v1", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("codex");
        std::fs::write(&p, "#!/bin/sh\necho 'not this'\nexit 1\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut e = Engine::start(&[]);
        e.send(json!({"op": "detect", "id": 1, "cli": "codex", "program": p}));
        let a = e.answer();
        assert_eq!(
            (a["ev"].as_str(), a["kind"].as_str()),
            (Some("error"), Some("cli")),
            "{a}"
        );
        assert!(
            a["message"].as_str().unwrap().contains("exited with 1"),
            "{a}"
        );
    }

    #[test]
    fn a_failing_cli_says_why() {
        let p = script("codex", "", 3);
        let mut e = Engine::start(&[]);
        e.send(run(json!({"kind": "codex", "program": p}), "", ""));
        let a = e.answer();
        assert_eq!(a["kind"], "cli", "{a}");
    }

    #[test]
    fn a_run_is_cancelled_at_once() {
        let dir = std::env::temp_dir().join(format!("mdedit-ai-test-{}-slow", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("claude");
        std::fs::write(&p, "#!/bin/sh\ncat > /dev/null\nsleep 30\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut e = Engine::start(&[]);
        e.send(run(json!({"kind": "claude-code", "program": p}), "", ""));
        std::thread::sleep(Duration::from_millis(300));
        let t = std::time::Instant::now();
        e.send(json!({"op": "cancel", "id": 7}));
        let a = e.answer();
        assert_eq!(a["kind"], "cancelled", "{a}");
        assert!(t.elapsed() < Duration::from_secs(5));
        // Nothing more comes for the cancelled run.
        assert!(e.events.recv_timeout(Duration::from_millis(500)).is_err());
    }
}
