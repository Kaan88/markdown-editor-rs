//! Cleaning up what a model sends back.

/// A reply wrapped whole in one code fence (```` ```markdown … ``` ````) is unwrapped: models do
/// that despite being told not to. A reply with other text around its fence, or with more than
/// one fence, is left as it is.
pub fn unwrap_fence(text: &str) -> String {
    let t = text.trim_matches(|c| c == '\n' || c == '\r');
    let lines: Vec<&str> = t.lines().collect();
    if lines.len() < 2 {
        return text.to_string();
    }
    let open = lines[0].trim_end();
    let ticks = open.chars().take_while(|&c| c == '`' || c == '~').count();
    let Some(mark) = open.chars().next() else {
        return text.to_string();
    };
    if ticks < 3 || open.chars().take(ticks).any(|c| c != mark) {
        return text.to_string();
    }
    let info = &open[ticks..];
    if !(info.is_empty()
        || info.eq_ignore_ascii_case("markdown")
        || info.eq_ignore_ascii_case("md"))
    {
        return text.to_string();
    }
    let is_close = |l: &str| {
        let l = l.trim_end();
        l.len() >= ticks && l.chars().all(|c| c == mark)
    };
    let last = lines.len() - 1;
    if !is_close(lines[last]) {
        return text.to_string();
    }
    // A closing fence inside means the opening one was not around the whole reply.
    if lines[1..last].iter().any(|l| is_close(l)) {
        return text.to_string();
    }
    let mut s = lines[1..last].join("\n");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrapped_reply_is_unwrapped() {
        assert_eq!(unwrap_fence("```markdown\n# A\n\nb\n```\n"), "# A\n\nb\n");
        assert_eq!(unwrap_fence("```\nx\n```"), "x\n");
        assert_eq!(
            unwrap_fence("````md\n```rust\nfn f(){}\n```\n````"),
            "```rust\nfn f(){}\n```\n"
        );
    }

    #[test]
    fn other_replies_are_left() {
        // Two code blocks, not one around everything.
        let two = "```\na\n```\n\ntext\n\n```\nb\n```";
        assert_eq!(unwrap_fence(two), two);
        // A code block of another language is the content.
        let rust = "```rust\nfn f() {}\n```";
        assert_eq!(unwrap_fence(rust), rust);
        assert_eq!(unwrap_fence("# Plain"), "# Plain");
        let lead = "Here it is:\n```\nx\n```";
        assert_eq!(unwrap_fence(lead), lead);
    }
}
