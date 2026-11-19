use super::*;

#[test]
fn claude_runs_with_no_tools_and_no_session() {
    let a = claude_args("sonnet", "high", "Rules.");
    let at = |s: &str| a.iter().position(|x| x == s).unwrap();
    assert_eq!(a[at("--tools") + 1], "");
    assert_eq!(a[at("--system-prompt") + 1], "Rules.");
    assert_eq!(a[at("--model") + 1], "sonnet");
    assert_eq!(a[at("--effort") + 1], "high");
    assert!(a.contains(&"--no-session-persistence".to_string()));
    let a = claude_args("", "", "R");
    assert!(!a.contains(&"--model".to_string()) && !a.contains(&"--effort".to_string()));
}

#[test]
fn codex_runs_read_only_with_the_prompt_on_stdin() {
    let a = codex_args("gpt-5", "low");
    assert_eq!(a[0], "exec");
    assert_eq!(a.last().unwrap(), "-");
    let at = |s: &str| a.iter().position(|x| x == s).unwrap();
    assert_eq!(a[at("--sandbox") + 1], "read-only");
    assert_eq!(a[at("-c") + 1], "model_reasoning_effort=\"low\"");
    assert!(a.contains(&"--ephemeral".to_string()));
}

#[test]
fn claude_lines_give_the_result() {
    let mut l = ClaudeLines::default();
    let v = |s: &str| serde_json::from_str::<Value>(s).unwrap();
    l.line(&v(r#"{"type":"system","subtype":"init"}"#));
    let n = l.line(&v(
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"abc"}}}"#,
    ));
    assert_eq!(n, 3);
    l.line(&v(
        r#"{"type":"result","subtype":"success","is_error":false,"result":"abc"}"#,
    ));
    assert_eq!(l.result, Some(Ok("abc".into())));
    let mut l = ClaudeLines::default();
    l.line(&v(
        r#"{"type":"result","subtype":"success","is_error":true,"result":"Invalid API key · Please run /login"}"#,
    ));
    let Some(Err(m)) = l.result else { panic!() };
    assert_eq!(classify(Cli::ClaudeCode, &m).kind, "auth");
}

#[test]
fn codex_lines_give_the_last_message() {
    let mut l = CodexLines::default();
    let v = |s: &str| serde_json::from_str::<Value>(s).unwrap();
    l.line(&v(r#"{"type":"thread.started","thread_id":"t"}"#));
    l.line(&v(
        r#"{"type":"item.completed","item":{"id":"i0","type":"reasoning","text":"thinking"}}"#,
    ));
    l.line(&v(
        r##"{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"# Done"}}"##,
    ));
    assert_eq!(l.text.as_deref(), Some("# Done"));
    l.line(&v(
        r#"{"type":"turn.failed","error":{"message":"You've hit your usage limit."}}"#,
    ));
    assert_eq!(
        classify(Cli::Codex, l.error.as_deref().unwrap()).kind,
        "rate"
    );
}

#[test]
fn a_set_program_that_is_missing_is_named() {
    let f = find(Cli::Codex, "/no/such/codex").unwrap_err();
    assert_eq!(f.kind, "not_found");
    assert!(f.message.contains("/no/such/codex"));
}
