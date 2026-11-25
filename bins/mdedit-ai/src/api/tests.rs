use super::*;

fn claude(events: &[&str]) -> Result<String, Fail> {
    let mut s = ClaudeStream::default();
    for e in events {
        if !s.event(e) {
            break;
        }
    }
    s.finish()
}

#[test]
fn a_claude_answer_is_put_together() {
    let r = claude(&[
        r#"{"type":"message_start","message":{"id":"m"}}"#,
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
        r##"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"# Trip"}}"##,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" notes"}}"#,
        r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#,
        r#"{"type":"message_stop"}"#,
    ]);
    assert_eq!(r, Ok("# Trip notes".into()));
}

#[test]
fn refusals_and_cut_answers_are_not_used() {
    let end = |reason: &str| {
        claude(&[
            r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"x"}}"#,
            &format!(r#"{{"type":"message_delta","delta":{{"stop_reason":"{reason}"}}}}"#),
            r#"{"type":"message_stop"}"#,
        ])
    };
    assert_eq!(end("refusal").unwrap_err().kind, "declined");
    assert_eq!(end("max_tokens").unwrap_err().kind, "length");
    // No message_stop: the connection dropped.
    let r = claude(&[r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"x"}}"#]);
    assert_eq!(r.unwrap_err().kind, "network");
    let r =
        claude(&[r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#]);
    assert_eq!(r.unwrap_err().kind, "rate");
}

#[test]
fn the_claude_body_has_effort_only_when_set() {
    let b = claude_body("claude-opus-5-5", "high", "sys", "text", &[], 64000);
    assert_eq!(b["output_config"]["effort"], "high");
    assert_eq!(b["system"], "sys");
    assert_eq!(b["messages"][0]["content"], "text");
    assert_eq!(b["stream"], true);
    let b = claude_body("m", "", "s", "t", &[], 1);
    assert!(b.get("output_config").is_none());
    assert!(b.get("thinking").is_none());
    let o = openai_body("gpt", "", "s", "t", &[]);
    assert!(o.get("reasoning_effort").is_none());
    assert_eq!(o["messages"][0]["role"], "system");
    assert_eq!(
        openai_body("gpt", "low", "s", "t", &[])["reasoning_effort"],
        "low"
    );
}

#[test]
fn claude_models_say_their_efforts() {
    let v: Value = serde_json::from_str(
        r#"{"data":[
            {"id":"claude-opus-5-5","display_name":"Claude Opus 5.5","max_tokens":128000,
             "capabilities":{"effort":{"low":{"supported":true},"medium":{"supported":true},"high":{"supported":true},"xhigh":{"supported":true},"max":{"supported":true}}}},
            {"id":"claude-haiku-5-5","display_name":"Claude Haiku 5.5","max_tokens":64000,
             "capabilities":{"effort":{"low":{"supported":false},"max":{"supported":false}}}},
            {"display_name":"no id"}
        ],"has_more":false}"#,
    )
    .unwrap();
    let m = parse_claude_models(&v);
    assert_eq!(m.len(), 2);
    assert_eq!(m[0].name, "Claude Opus 5.5");
    assert_eq!(m[0].efforts, ["low", "medium", "high", "xhigh", "max"]);
    assert_eq!(m[0].max_tokens, 128000);
    assert!(m[1].efforts.is_empty());
}

#[test]
fn openai_models_are_sorted() {
    let v: Value =
        serde_json::from_str(r#"{"object":"list","data":[{"id":"b"},{"id":"a"},{"id":"b"}]}"#)
            .unwrap();
    let ids: Vec<_> = parse_openai_models(&v).into_iter().map(|m| m.id).collect();
    assert_eq!(ids, ["a", "b"]);
}

#[test]
fn an_openai_answer_is_put_together() {
    let mut s = OpenAiStream::default();
    for e in [
        r#"{"choices":[{"delta":{"role":"assistant"}}]}"#,
        r#"{"choices":[{"delta":{"content":"- a"}}]}"#,
        r#"{"choices":[{"delta":{"content":"\n- b"},"finish_reason":"stop"}]}"#,
        "[DONE]",
    ] {
        if !s.event(e) {
            break;
        }
    }
    assert_eq!(s.chars, 7);
    assert_eq!(s.finish("x"), Ok("- a\n- b".into()));
    let mut s = OpenAiStream::default();
    s.event(r#"{"choices":[{"delta":{"content":"a"},"finish_reason":"length"}]}"#);
    assert_eq!(s.finish("x").unwrap_err().kind, "length");
}

#[test]
fn http_errors_say_what_to_do() {
    assert_eq!(http_fail("Claude API", 401, None, "").kind, "auth");
    assert_eq!(
        http_fail("Claude API", 401, None, "").message,
        "Claude API refused the key (401)."
    );
    let f = http_fail("api.groq.com", 429, Some(40), "{}");
    assert_eq!((f.kind, f.retry_after), ("rate", Some(40)));
    assert!(f.message.contains("40 seconds"));
    assert_eq!(http_fail("x", 529, None, "").kind, "rate");
    let f = http_fail(
        "x",
        400,
        None,
        r#"{"type":"error","error":{"type":"invalid_request_error","message":"bad model"}}"#,
    );
    assert_eq!(f.message, "x answered 400: bad model");
    assert!(
        explain_effort(f, "high")
            .message
            .ends_with("set Effort to Default.")
    );
}

#[test]
fn the_host_names_the_provider() {
    assert_eq!(host_of("https://openrouter.ai/api/v1"), "openrouter.ai");
    assert_eq!(host_of("http://localhost:11434/v1"), "localhost:11434");
}

/// Audit 1 round 3, #6: the reader and the answer agree on the last event, whatever the
/// spacing of its JSON, and a field nested deeper is not the event's type.
#[test]
fn the_reader_and_the_answer_agree_on_the_end() {
    let ev = |data: &str| sse::Event {
        name: String::new(),
        data: data.to_string(),
    };
    let spaced = r#"{"type": "message_stop"}"#;
    let nested = r##"{"type":"content_block_delta","delta":{"type":"text_delta","text":"x"},"x":{"type":"message_stop"}}"##;
    for (data, end) in [
        (spaced, true),
        (nested, false),
        (r#"{"type":"error","error":{}}"#, true),
    ] {
        let mut s = ClaudeStream::default();
        assert_eq!(!s.event(data), end, "{data}");
        assert_eq!(claude_last(&ev(data)), end, "{data}");
    }
    for (data, end) in [
        (" [DONE] ", true),
        (r#"{"error": {"code": 1}}"#, true),
        (r#"{"choices":[{"delta":{"content":"[DONE]"}}]}"#, false),
    ] {
        let mut s = OpenAiStream::default();
        assert_eq!(!s.event(data), end, "{data}");
        assert_eq!(openai_last(&ev(data)), end, "{data}");
    }
}

/// Audit 1 round 3, #4: a message cut for length never ends in part of a word, so a key in it
/// is whole (and hidden) or gone; an error body cut at its limit loses the word at the cut.
#[test]
fn a_cut_never_leaves_part_of_a_key() {
    let key = "QWERTYUIOPASDFGHJKLZXCVBNM1234567890";
    let body = format!("{}{key} more", "a ".repeat(90));
    let m = body_message(&body);
    assert!(!m.contains("QWERTY"), "{m}");
    assert!(m.ends_with('…'));
    // One long word: nothing can be shown of it.
    let m = body_message(&"Z".repeat(500));
    assert_eq!(m, "(its message could not be shown)");
    // Short bodies are whole.
    assert_eq!(body_message(" no such model "), "no such model");
}

/// A key with separators in it, which a cut at a word's end can split.
const SPLIT_KEY: &str = "Qx7Lm9:Zp2Rt5:Wv8Yb3:Kd4Hs6";

/// Audit 1 round 4, #6: an error body has the key taken out of all of it before it is cut,
/// and a body over its limit says nothing of itself.
#[test]
fn an_error_body_loses_the_key_before_it_is_cut() {
    let body = format!("{}{SPLIT_KEY} end", "a ".repeat(95));
    let shown = body_message(&error_body(&body, 64 * 1024, Some(SPLIT_KEY)));
    assert!(!shown.contains("Qx7"), "{shown}");
    let long = format!("{}{}", " ".repeat(100), &SPLIT_KEY[..10]);
    assert_eq!(
        error_body(&long, long.len(), Some(SPLIT_KEY)),
        BODY_TOO_LONG
    );
    assert_eq!(error_body("short", 64, None), "short");
}

/// Audit 1 round 4, #6: an error in an OpenAI stream has the run's key taken out before its
/// message is cut.
#[test]
fn a_streamed_error_loses_the_key_before_it_is_cut() {
    let message = format!("{}{SPLIT_KEY} end", "a ".repeat(95));
    let mut s = OpenAiStream {
        key: Some(SPLIT_KEY.into()),
        ..OpenAiStream::default()
    };
    let data = serde_json::json!({"error": {"code": 1}, "detail": message}).to_string();
    assert!(!s.event(&data));
    let m = s.fail.clone().unwrap();
    assert!(!m.contains("Qx7"), "{m}");
}

#[test]
fn an_address_s_user_and_password_are_not_shown() {
    assert_eq!(
        host_of("https://me:secret@llm.example.com/v1"),
        "llm.example.com"
    );
    assert_eq!(host_of("http://127.0.0.1:1234/v1"), "127.0.0.1:1234");
    assert_eq!(host_of("https://"), "The server");
}

fn ready(bytes: &[u8], mime: &'static str) -> Ready {
    Ready {
        bytes: bytes.to_vec(),
        mime,
        ext: "jpg",
        sha256: crate::pictures::digest(bytes),
    }
}

/// Plan 6: the Claude API is sent the pictures first, then the text.
#[test]
fn a_claude_body_with_pictures_puts_them_before_the_text() {
    let imgs = [ready(b"one", "image/jpeg"), ready(b"two", "image/png")];
    let b = claude_body("m", "", "s", "describe", &imgs, 1);
    let c = &b["messages"][0]["content"];
    assert_eq!(c.as_array().map(Vec::len), Some(3));
    assert_eq!(c[0]["type"], "image");
    assert_eq!(c[0]["source"]["type"], "base64");
    assert_eq!(c[0]["source"]["media_type"], "image/jpeg");
    assert_eq!(c[0]["source"]["data"], "b25l");
    assert_eq!(c[1]["source"]["media_type"], "image/png");
    assert_eq!(c[2], json!({"type": "text", "text": "describe"}));
}

/// Plan 6: an OpenAI-style server is sent the text, then one data URL per picture.
#[test]
fn an_openai_body_with_pictures_has_data_urls_after_the_text() {
    let imgs = [ready(b"one", "image/jpeg")];
    let b = openai_body("gpt", "", "s", "describe", &imgs);
    let c = &b["messages"][1]["content"];
    assert_eq!(c[0], json!({"type": "text", "text": "describe"}));
    assert_eq!(c[1]["type"], "image_url");
    assert_eq!(c[1]["image_url"]["url"], "data:image/jpeg;base64,b25l");
    assert_eq!(b["messages"][0]["content"], "s");
}

/// Plan 15: a refusal that speaks of pictures, for a request with pictures, says the model reads
/// none; the same refusal of a text request, or one about something else, is left as it is.
#[test]
fn a_refusal_about_pictures_is_no_vision() {
    let f = http_fail(
        "localhost:11434",
        400,
        None,
        r#"{"error":{"message":"model does not support image input"}}"#,
    );
    let v = no_vision(f.clone(), true, "llama3", "localhost:11434");
    assert_eq!(v.kind, "no_vision");
    assert_eq!(
        v.message,
        "This model does not read pictures: llama3 at localhost:11434. It said: model does not support image input"
    );
    assert_eq!(no_vision(f, false, "llama3", "h").kind, "http");
    let other = http_fail("h", 400, None, r#"{"error":{"message":"bad temperature"}}"#);
    assert_eq!(no_vision(other, true, "m", "h").kind, "http");
    let auth = http_fail("h", 401, None, "image");
    assert_eq!(no_vision(auth, true, "m", "h").kind, "auth");
}

/// Fable's review of V1, 16: only the server's own words of a 400 or 422 say a model reads no
/// pictures. A host or a model named for images, a 500 that quotes "400:", and a refusal of one
/// picture (too large, a format) keep their own message.
#[test]
fn only_the_server_s_words_say_a_model_reads_no_pictures() {
    let f = |status: u16, body: &str| http_fail("images.corp:8080", status, None, body);
    let says = |status: u16, body: &str| {
        no_vision(f(status, body), true, "llama3.2-vision", "images.corp:8080").kind
    };
    let none = r#"{"error":{"message":"This model does not support image inputs."}}"#;
    assert_eq!(says(400, none), "no_vision");
    assert_eq!(says(422, none), "no_vision");
    assert_eq!(
        says(
            400,
            r#"{"error":{"message":"Model does not support images. Please use a model that does."}}"#
        ),
        "no_vision"
    );
    // Not of the model: the pictures themselves, an effort, another status.
    for body in [
        r#"{"error":{"message":"image too large: 9000 x 9000"}}"#,
        r#"{"error":{"message":"unsupported image format image/webp"}}"#,
        r#"{"error":{"message":"reasoning_effort is not supported by llama3.2-vision"}}"#,
        r#"{"error":{"message":"bad request"}}"#,
    ] {
        assert_eq!(says(400, body), "http", "{body}");
    }
    assert_eq!(
        says(
            500,
            r#"{"error":{"message":"upstream said 400: image not supported"}}"#
        ),
        "http"
    );
    assert_eq!(
        no_vision(f(400, none), false, "m", "h").kind,
        "http",
        "no pictures sent"
    );
    // The server's words stay in the message.
    let n = no_vision(f(400, none), true, "llama3", "h:1");
    assert!(
        n.message
            .ends_with("It said: This model does not support image inputs."),
        "{}",
        n.message
    );
}
