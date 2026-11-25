use super::*;

#[test]
fn claude_runs_with_no_tools_and_no_session() {
    let a = claude_args("sonnet", "high", "Rules.", false);
    let at = |s: &str| a.iter().position(|x| x == s).unwrap();
    assert_eq!(a[at("--tools") + 1], "");
    assert_eq!(a[at("--system-prompt") + 1], "Rules.");
    assert_eq!(a[at("--model") + 1], "sonnet");
    assert_eq!(a[at("--effort") + 1], "high");
    assert!(a.contains(&"--no-session-persistence".to_string()));
    let a = claude_args("", "", "R", false);
    assert!(!a.contains(&"--model".to_string()) && !a.contains(&"--effort".to_string()));
}

#[test]
fn codex_runs_read_only_with_the_prompt_on_stdin() {
    let a = codex_args("gpt-5", "low", &[]);
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

/// Audit 1 round 4, #6: stderr read up to its limit loses the line at the cut, where part of a
/// key could be; what is under the limit is whole.
#[test]
fn stderr_at_its_limit_loses_the_cut_line() {
    let mut b = vec![b'a'; STDERR_MAX - 20];
    b.extend_from_slice(b"\nsk-ant-cut-off-par");
    b.push(b't');
    assert_eq!(b.len(), STDERR_MAX);
    let s = stderr_text(b);
    assert!(!s.contains("sk-ant"), "the cut line stays");
    assert_eq!(s.len(), STDERR_MAX - 20);
    assert_eq!(stderr_text(b"error: no\nsk-x".to_vec()), "error: no\nsk-x");
}

fn ready(bytes: &[u8], mime: &'static str, ext: &'static str) -> Ready {
    Ready {
        bytes: bytes.to_vec(),
        mime,
        ext,
        sha256: crate::pictures::digest(bytes),
    }
}

/// D-213: no MCP server of the user's own configuration is loaded, text run or not; D-221: a run
/// with pictures reads its input as stream-json.
#[test]
fn claude_loads_no_mcp_server_and_takes_pictures_as_stream_json() {
    let a = claude_args("", "", "R", false);
    assert!(a.contains(&"--strict-mcp-config".to_string()));
    assert!(!a.contains(&"--mcp-config".to_string()));
    assert!(!a.contains(&"--input-format".to_string()));
    let a = claude_args("", "", "R", true);
    let at = |s: &str| a.iter().position(|x| x == s).unwrap();
    assert_eq!(a[at("--input-format") + 1], "stream-json");
    assert!(a.contains(&"--strict-mcp-config".to_string()));
}

/// D-221: the pictures go first in one user message, then the text, as one line.
#[test]
fn claude_input_is_one_user_message_with_the_pictures_first() {
    let s = claude_input("describe", &[ready(b"one", "image/png", "png")]);
    assert!(s.ends_with('\n'));
    assert_eq!(s.matches('\n').count(), 1);
    let v: Value = serde_json::from_str(s.trim_end()).unwrap();
    assert_eq!(v["type"], "user");
    assert_eq!(v["message"]["role"], "user");
    let c = &v["message"]["content"];
    assert_eq!(c[0]["type"], "image");
    assert_eq!(c[0]["source"]["media_type"], "image/png");
    assert_eq!(c[0]["source"]["data"], "b25l");
    assert_eq!(c[1]["type"], "text");
    assert_eq!(c[1]["text"], "describe");
}

/// D-221: one `--image` per file, then `--` before the `-` that means "prompt on stdin".
#[test]
fn codex_names_each_picture_and_ends_its_options() {
    let files = [PathBuf::from("/t/1.jpg"), PathBuf::from("/t/2.png")];
    let a = codex_args("", "", &files);
    let n = a.len();
    assert_eq!(
        &a[n - 6..],
        ["--image", "/t/1.jpg", "--image", "/t/2.png", "--", "-"]
    );
}

/// Plan 13: the pictures written for Codex are in a folder of their own, gone when the run is.
#[test]
fn codex_pictures_are_written_and_removed() {
    let p = PictureFiles::write(&[
        ready(b"one", "image/jpeg", "jpg"),
        ready(b"two", "image/png", "png"),
    ])
    .unwrap();
    let dir = p.dir.clone();
    assert!(
        dir.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(PICTURES_DIR)
    );
    assert_eq!(std::fs::read(dir.join("1.jpg")).unwrap(), b"one");
    assert_eq!(std::fs::read(dir.join("2.png")).unwrap(), b"two");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "{mode:o}");
    }
    drop(p);
    assert!(!dir.exists());
}

/// Fable's review of V1, 24: the sweep removes only folders named for a run's pictures and older
/// than the age it is given; a fresh one and any other folder stay.
#[test]
fn the_sweep_removes_only_stale_picture_folders() {
    let temp = std::env::temp_dir().join(format!("mdedit-ai-test-sweep-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let ours = temp.join(format!("{PICTURES_DIR}1-2-0"));
    let other = temp.join("someone-elses");
    std::fs::create_dir_all(&ours).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(ours.join("1.png"), b"x").unwrap();
    sweep(&temp, PICTURES_STALE);
    assert!(ours.is_dir(), "a fresh folder stays");
    sweep(&temp, Duration::ZERO);
    assert!(!ours.exists(), "a stale one goes, with what is in it");
    assert!(other.is_dir(), "another folder stays");
    let _ = std::fs::remove_dir_all(&temp);
}
