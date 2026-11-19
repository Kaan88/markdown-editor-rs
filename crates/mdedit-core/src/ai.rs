//! AI rewriting, the parts with no window and no process: the connections and their providers,
//! the prompts sent with a rewrite, the saved prompts file, and the lines of the review.
//!
//! The work itself is done by the `mdedit-ai` engine, a separate program; the shell talks to it.

use crate::buffer::Buffer;
use crate::diff::{self, DiffOptions, HunkKind};

/// Where rewrites go. `Off` (the default) uses no AI at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Connection {
    #[default]
    Off,
    /// The Claude Code program the user has signed in to.
    ClaudeCode,
    /// The Codex program the user has signed in to.
    Codex,
    /// The Claude API with a key.
    ClaudeApi,
    /// An OpenAI-compatible API: a preset provider or a base URL.
    OpenAi,
}

impl Connection {
    pub const ALL: [Connection; 5] = [
        Connection::Off,
        Connection::ClaudeCode,
        Connection::Codex,
        Connection::ClaudeApi,
        Connection::OpenAi,
    ];

    /// The word in the settings file.
    pub fn key(self) -> &'static str {
        match self {
            Connection::Off => "off",
            Connection::ClaudeCode => "claude-code",
            Connection::Codex => "codex",
            Connection::ClaudeApi => "claude-api",
            Connection::OpenAi => "openai",
        }
    }

    pub fn parse(s: &str) -> Option<Connection> {
        Connection::ALL.into_iter().find(|c| c.key() == s.trim())
    }

    /// As Settings lists it.
    pub fn label(self) -> &'static str {
        match self {
            Connection::Off => "Off",
            Connection::ClaudeCode => "Claude Code (local)",
            Connection::Codex => "Codex (local)",
            Connection::ClaudeApi => "Claude API",
            Connection::OpenAi => "OpenAI-compatible API",
        }
    }

    /// As the review and the instruction dialog name it.
    pub fn short(self) -> &'static str {
        match self {
            Connection::Off => "Off",
            Connection::ClaudeCode => "Claude Code",
            Connection::Codex => "Codex",
            Connection::ClaudeApi => "Claude API",
            Connection::OpenAi => "OpenAI-compatible",
        }
    }

    pub fn is_cli(self) -> bool {
        matches!(self, Connection::ClaudeCode | Connection::Codex)
    }

    /// The effort levels it takes when its model does not say, lowest first.
    pub fn efforts(self) -> &'static [&'static str] {
        match self {
            Connection::Off => &[],
            Connection::ClaudeCode | Connection::ClaudeApi => {
                &["low", "medium", "high", "xhigh", "max"]
            }
            Connection::Codex => &["minimal", "low", "medium", "high", "xhigh"],
            Connection::OpenAi => &["minimal", "low", "medium", "high"],
        }
    }

    /// The models offered without fetching a list: Claude Code's aliases. Empty for the others.
    pub fn aliases(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Connection::ClaudeCode => &[
                ("fable", "Fable"),
                ("opus", "Opus"),
                ("sonnet", "Sonnet"),
                ("haiku", "Haiku"),
            ],
            _ => &[],
        }
    }
}

/// An effort level as Settings shows it.
pub fn effort_label(e: &str) -> String {
    match e {
        "" => "Default".into(),
        "xhigh" => "Extra high".into(),
        _ => {
            let mut c = e.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect(),
                None => String::new(),
            }
        }
    }
}

/// A provider of an OpenAI-compatible API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    /// The word in the settings file and in the key's slot.
    pub key: &'static str,
    pub name: &'static str,
    /// Empty for Custom.
    pub base_url: &'static str,
    /// Runs on the user's own machine and takes no key.
    pub local: bool,
}

const fn preset(key: &'static str, name: &'static str, base_url: &'static str) -> Preset {
    Preset {
        key,
        name,
        base_url,
        local: false,
    }
}

pub const PRESETS: &[Preset] = &[
    preset("openai", "OpenAI", "https://api.openai.com/v1"),
    preset("openrouter", "OpenRouter", "https://openrouter.ai/api/v1"),
    preset("groq", "Groq", "https://api.groq.com/openai/v1"),
    preset("together", "Together AI", "https://api.together.xyz/v1"),
    preset("deepseek", "DeepSeek", "https://api.deepseek.com/v1"),
    preset("mistral", "Mistral", "https://api.mistral.ai/v1"),
    preset(
        "gemini",
        "Google Gemini",
        "https://generativelanguage.googleapis.com/v1beta/openai",
    ),
    preset("xai", "xAI", "https://api.x.ai/v1"),
    Preset {
        key: "ollama",
        name: "Ollama (this computer)",
        base_url: "http://localhost:11434/v1",
        local: true,
    },
    Preset {
        key: "lmstudio",
        name: "LM Studio (this computer)",
        base_url: "http://localhost:1234/v1",
        local: true,
    },
    preset("custom", "Custom", ""),
];

pub fn preset_by_key(key: &str) -> &'static Preset {
    PRESETS.iter().find(|p| p.key == key).unwrap_or(&PRESETS[0])
}

/// The credential-store slot of a connection's key; `None` for one that takes no key.
pub fn key_slot(conn: Connection, preset: &str) -> Option<String> {
    match conn {
        Connection::ClaudeApi => Some("claude-api".into()),
        Connection::OpenAi => Some(format!("openai:{}", preset_by_key(preset).key)),
        _ => None,
    }
}

/// The base URL a rewrite goes to: the preset's, or the one typed for Custom.
pub fn base_url(preset: &str, custom: &str) -> String {
    let p = preset_by_key(preset);
    if p.base_url.is_empty() {
        custom.trim().to_string()
    } else {
        p.base_url.to_string()
    }
}

/// Who the text goes to, for the line under Connection.
pub fn destination(conn: Connection, preset: &str, custom: &str) -> String {
    match conn {
        Connection::Off => "AI commands are off.".into(),
        Connection::ClaudeCode => {
            "Text you rewrite is sent to Anthropic by your Claude Code.".into()
        }
        Connection::Codex => "Text you rewrite is sent to OpenAI by your Codex.".into(),
        Connection::ClaudeApi => "Text you rewrite is sent to Anthropic.".into(),
        Connection::OpenAi => {
            let p = preset_by_key(preset);
            if p.local {
                format!(
                    "Text you rewrite stays on this computer ({}).",
                    p.name.split(" (").next().unwrap_or(p.name)
                )
            } else if p.base_url.is_empty() {
                let url = custom.trim();
                if url.is_empty() {
                    "Type the base URL of the server.".into()
                } else {
                    format!("Text you rewrite is sent to {}.", host(url))
                }
            } else {
                format!("Text you rewrite is sent to {}.", p.name)
            }
        }
    }
}

fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?']).next().unwrap_or(rest)
}

// ---- prompts -----------------------------------------------------------------------------------

/// The rules every rewrite is sent with.
pub const SYSTEM: &str = "You rewrite text for a Markdown editor. Do what the instruction asks \
with the text between <text> and </text>. Reply with the rewritten text only: no preamble, no \
explanation, no notes, and no code fence around the whole reply. Keep the text's language. Keep \
everything the instruction does not ask to change: the wording, facts, numbers, names, links and \
the content of code blocks. Write CommonMark with GitHub's extensions (tables, task lists, \
strikethrough, footnotes, alerts); math as $…$ and $$…$$, diagrams in ```mermaid blocks.";

/// What the built-in commands ask.
pub const FORMAT: &str = "Format this text as Markdown: headings for its sections, lists, \
tables for tabular data, task lists for things to do, emphasis and code where the text calls for \
them. Do not change the wording.";

pub const FIX: &str = "Fix the Markdown of this text: broken tables, lists and links, unclosed \
code fences and emphasis, skipped heading levels, mixed list markers, missing blank lines around \
blocks. Change nothing else.";

/// The message sent with the text.
pub fn prompt(instruction: &str, text: &str) -> String {
    format!("{}\n\n<text>\n{text}\n</text>", instruction.trim())
}

/// The rewritten text made to end as the original did: with a line break if, and only if, the
/// original ended with one (models drop or add the last one at will).
pub fn fit_end(original: &str, result: &str) -> String {
    let body = result.trim_end_matches(['\n', '\r']);
    let eol = if original.ends_with("\r\n") {
        "\r\n"
    } else if original.ends_with('\n') {
        "\n"
    } else if original.ends_with('\r') {
        "\r"
    } else {
        ""
    };
    format!("{body}{eol}")
}

/// The text with its line breaks made `eol`, so a rewrite of a CRLF file stays CRLF.
pub fn with_eol(text: &str, eol: &str) -> String {
    let lf = text.replace("\r\n", "\n").replace('\r', "\n");
    if eol == "\n" {
        lf
    } else {
        lf.replace('\n', eol)
    }
}

// ---- saved prompts -----------------------------------------------------------------------------

/// The saved prompts file, beside settings.ini.
pub const PROMPTS_FILE: &str = "ai-prompts.md";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedPrompt {
    pub name: String,
    pub instruction: String,
}

impl SavedPrompt {
    /// The instruction's first line, shortened, for lists.
    pub fn summary(&self) -> String {
        let first = self.instruction.lines().next().unwrap_or("").trim();
        let mut s: String = first.chars().take(90).collect();
        if s.len() < first.len() || self.instruction.lines().nth(1).is_some() {
            s.push('…');
        }
        s
    }
}

/// The file as it is first written: what it is, and four prompts to start from.
pub const PROMPTS_TEMPLATE: &str = "# Saved prompts for mdedit\n\
\n\
Each prompt is a heading of level 2 (its name, as the palette and the AI menu list it), then the\n\
instruction under it. Save this file and the list is read again. Text before the first prompt,\n\
like this, is left out.\n\
\n\
## Make a table\n\
\n\
Turn the data in this text into a Markdown table with a header row. Align numbers to the right.\n\
\n\
## Tidy headings\n\
\n\
Make the headings consistent: one level-1 heading at the top, no skipped levels, sentence case.\n\
\n\
## Shorten\n\
\n\
Make this text about half as long. Keep every fact and the Markdown structure.\n\
\n\
## Fix spelling and grammar\n\
\n\
Fix the spelling, grammar and punctuation. Change nothing else.\n";

/// Reads the saved prompts: each `## Name` heading and the text under it to the next one. Empty
/// names and empty instructions are left out; a heading inside a code fence is text.
pub fn parse_prompts(text: &str) -> Vec<SavedPrompt> {
    let mut out: Vec<SavedPrompt> = Vec::new();
    let mut cur: Option<(String, Vec<&str>)> = None;
    let mut fence: Option<String> = None;
    let flush = |cur: &mut Option<(String, Vec<&str>)>, out: &mut Vec<SavedPrompt>| {
        if let Some((name, lines)) = cur.take() {
            let instruction = lines.join("\n").trim().to_string();
            if !name.is_empty() && !instruction.is_empty() {
                out.push(SavedPrompt { name, instruction });
            }
        }
    };
    for line in text.lines() {
        let t = line.trim_start();
        let marker: String = t.chars().take_while(|&c| c == '`' || c == '~').collect();
        if marker.len() >= 3 && marker.chars().all(|c| c == marker.chars().next().unwrap()) {
            match &fence {
                Some(open)
                    if t.trim_end()
                        .chars()
                        .all(|c| c == open.chars().next().unwrap())
                        && marker.len() >= open.len() =>
                {
                    fence = None
                }
                None => fence = Some(marker.clone()),
                _ => {}
            }
        } else if fence.is_none()
            && let Some(name) = line.strip_prefix("## ")
        {
            flush(&mut cur, &mut out);
            cur = Some((
                name.trim().trim_end_matches('#').trim().to_string(),
                Vec::new(),
            ));
            continue;
        }
        if let Some((_, lines)) = cur.as_mut() {
            lines.push(line);
        }
    }
    flush(&mut cur, &mut out);
    out
}

// ---- review ------------------------------------------------------------------------------------

/// A line of the review: unchanged, removed or added, with its line numbers (1-based), or a fold
/// of unchanged lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewLine {
    Same {
        old: usize,
        new: usize,
        text: String,
    },
    Removed {
        old: usize,
        text: String,
    },
    Added {
        new: usize,
        text: String,
    },
    Fold {
        lines: usize,
    },
}

/// Unchanged lines kept around a change; longer unchanged runs fold.
const CONTEXT: usize = 3;

/// The review of `before` → `after`: removed lines then added lines for each change, unchanged
/// runs folded to [`CONTEXT`] lines on each side. Also the counts of removed and added lines.
pub fn review(before: &str, after: &str) -> (Vec<ReviewLine>, usize, usize) {
    let a = Buffer::from_text(before);
    let b = Buffer::from_text(after);
    let d = diff::diff(&a, &b, DiffOptions::default(), &Default::default());
    let la: Vec<&str> = split_lines(before);
    let lb: Vec<&str> = split_lines(after);
    let line = |v: &[&str], i: usize| v.get(i).copied().unwrap_or("").to_string();
    let hunks = d.hunks();
    let mut out = Vec::new();
    let (mut removed, mut added) = (0, 0);
    for (k, h) in hunks.iter().enumerate() {
        match h.kind {
            HunkKind::Equal => {
                let n = h.a.len();
                let lead = if k == 0 { 0 } else { CONTEXT };
                let tail = if k + 1 == hunks.len() { 0 } else { CONTEXT };
                if n > lead + tail + 1 {
                    for i in 0..lead {
                        out.push(same(&la, h, i, line));
                    }
                    out.push(ReviewLine::Fold {
                        lines: n - lead - tail,
                    });
                    for i in n - tail..n {
                        out.push(same(&la, h, i, line));
                    }
                } else {
                    for i in 0..n {
                        out.push(same(&la, h, i, line));
                    }
                }
            }
            _ => {
                for i in h.a.clone() {
                    removed += 1;
                    out.push(ReviewLine::Removed {
                        old: i + 1,
                        text: line(&la, i),
                    });
                }
                for i in h.b.clone() {
                    added += 1;
                    out.push(ReviewLine::Added {
                        new: i + 1,
                        text: line(&lb, i),
                    });
                }
            }
        }
    }
    (out, removed, added)
}

fn same(
    la: &[&str],
    h: &diff::Hunk,
    i: usize,
    line: impl Fn(&[&str], usize) -> String,
) -> ReviewLine {
    ReviewLine::Same {
        old: h.a.start + i + 1,
        new: h.b.start + i + 1,
        text: line(la, h.a.start + i),
    }
}

/// Lines as the diff counts them: split at LF, CRLF and CR, the line breaks left out.
fn split_lines(s: &str) -> Vec<&str> {
    let mut v = Vec::new();
    let mut start = 0;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                v.push(&s[start..i]);
                start = i + 1;
            }
            b'\r' => {
                v.push(&s[start..i]);
                if bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    v.push(&s[start..]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rewrite_is_one_undo_step() {
        use crate::edit::{replace_range, undo};
        use crate::history::History;
        use crate::selection::Selection;
        let mut buf = Buffer::from_text("keep\nold text\nkeep\n");
        let mut hist = History::default();
        let sel = Selection::at(7);
        let out = replace_range(&mut buf, &mut hist, sel, 5..13, "# New\n\n- one");
        assert_eq!(buf.rope().to_string(), "keep\n# New\n\n- one\nkeep\n");
        assert_eq!((out.sel.anchor, out.sel.caret), (5, 17));
        let back = undo(&mut buf, &mut hist).unwrap();
        assert_eq!(buf.rope().to_string(), "keep\nold text\nkeep\n");
        assert_eq!(back.sel.caret, 7);
    }

    #[test]
    fn connections_round_trip() {
        for c in Connection::ALL {
            assert_eq!(Connection::parse(c.key()), Some(c));
        }
        assert_eq!(Connection::parse("nope"), None);
        assert_eq!(Connection::default(), Connection::Off);
    }

    #[test]
    fn presets_and_slots() {
        assert_eq!(preset_by_key("groq").name, "Groq");
        assert_eq!(preset_by_key("gone").key, "openai");
        assert_eq!(base_url("ollama", "x"), "http://localhost:11434/v1");
        assert_eq!(base_url("custom", " http://h:1/v1 "), "http://h:1/v1");
        assert_eq!(
            key_slot(Connection::OpenAi, "openrouter").as_deref(),
            Some("openai:openrouter")
        );
        assert_eq!(
            key_slot(Connection::ClaudeApi, "").as_deref(),
            Some("claude-api")
        );
        assert_eq!(key_slot(Connection::Codex, ""), None);
        assert_eq!(
            destination(Connection::OpenAi, "ollama", ""),
            "Text you rewrite stays on this computer (Ollama)."
        );
        assert_eq!(
            destination(Connection::OpenAi, "custom", "https://llm.example.com/v1"),
            "Text you rewrite is sent to llm.example.com."
        );
        assert_eq!(effort_label("xhigh"), "Extra high");
        assert_eq!(effort_label(""), "Default");
        assert_eq!(effort_label("low"), "Low");
    }

    #[test]
    fn the_prompt_wraps_the_text() {
        assert_eq!(
            prompt(" Fix it. ", "a\nb"),
            "Fix it.\n\n<text>\na\nb\n</text>"
        );
    }

    #[test]
    fn the_end_of_the_text_is_kept() {
        assert_eq!(fit_end("a\n", "b"), "b\n");
        assert_eq!(fit_end("a", "b\n\n"), "b");
        assert_eq!(fit_end("a\r\n", "b\n"), "b\r\n");
        assert_eq!(with_eol("a\nb\r\nc", "\r\n"), "a\r\nb\r\nc");
        assert_eq!(with_eol("a\r\nb", "\n"), "a\nb");
    }

    #[test]
    fn saved_prompts_are_read() {
        let p = parse_prompts(PROMPTS_TEMPLATE);
        let names: Vec<_> = p.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Make a table",
                "Tidy headings",
                "Shorten",
                "Fix spelling and grammar"
            ]
        );
        assert!(p[0].instruction.starts_with("Turn the data"));
        let t = "## A\n\nDo a.\n\n```\n## not a prompt\n```\n## \n\nno name\n## Empty\n\n## B ##\nDo b.";
        let p = parse_prompts(t);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].instruction, "Do a.\n\n```\n## not a prompt\n```");
        assert_eq!(p[1].name, "B");
        assert_eq!(p[0].summary(), "Do a.…");
    }

    #[test]
    fn the_review_folds_long_unchanged_runs() {
        let before = "a\n1\n2\n3\n4\n5\n6\n7\n8\nz\n";
        let after = "A\n1\n2\n3\n4\n5\n6\n7\n8\nZ\n";
        let (lines, removed, added) = review(before, after);
        assert_eq!((removed, added), (2, 2));
        assert_eq!(
            lines[0],
            ReviewLine::Removed {
                old: 1,
                text: "a".into()
            }
        );
        assert_eq!(
            lines[1],
            ReviewLine::Added {
                new: 1,
                text: "A".into()
            }
        );
        assert_eq!(
            lines[2],
            ReviewLine::Same {
                old: 2,
                new: 2,
                text: "1".into()
            }
        );
        assert!(lines.contains(&ReviewLine::Fold { lines: 2 }));
        assert_eq!(
            lines.last(),
            Some(&ReviewLine::Added {
                new: 10,
                text: "Z".into()
            })
        );
        // Nothing changed: no removed or added lines.
        let (_, r, a) = review("x\n", "x\n");
        assert_eq!((r, a), (0, 0));
    }
}
