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
    let b = claude_body("claude-opus-5-5", "high", "sys", "text", 64000);
    assert_eq!(b["output_config"]["effort"], "high");
    assert_eq!(b["system"], "sys");
    assert_eq!(b["messages"][0]["content"], "text");
    assert_eq!(b["stream"], true);
    let b = claude_body("m", "", "s", "t", 1);
    assert!(b.get("output_config").is_none());
    assert!(b.get("thinking").is_none());
    let o = openai_body("gpt", "", "s", "t");
    assert!(o.get("reasoning_effort").is_none());
    assert_eq!(o["messages"][0]["role"], "system");
    assert_eq!(
        openai_body("gpt", "low", "s", "t")["reasoning_effort"],
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
