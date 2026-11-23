//! AI rewriting, the parts with no window and no process: the connections and their providers,
//! the prompts sent with a rewrite and the saved prompts file. The review of a result is `hunks`.
//!
//! The work itself is done by the `mdedit-ai` engine, a separate program; the shell talks to it.

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

/// A custom provider of an OpenAI-compatible API, as many as the user adds: its own name, base
/// URL, model and effort. Its key, if it takes one, is kept per URL ([`key_slot`]); a local
/// server (LM Studio, Ollama) takes none and is sent no `authorization` header.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomProvider {
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub effort: String,
}

/// Custom providers kept at most.
pub const CUSTOMS_MAX: usize = 32;

/// The name a custom provider added now gets: "Custom provider N" with the first N no provider
/// in `list` goes by.
pub fn new_custom_name(list: &[CustomProvider]) -> String {
    let taken = |name: &str| {
        list.iter()
            .enumerate()
            .any(|(i, c)| c.label(i).eq_ignore_ascii_case(name))
    };
    (1..)
        .map(|n| format!("Custom provider {n}"))
        .find(|name| !taken(name))
        .expect("a free number")
}

impl CustomProvider {
    /// As the Provider list names it: its name, or a name made from its place in the list.
    pub fn label(&self, index: usize) -> String {
        let name = self.name.trim();
        if name.is_empty() {
            format!("Custom provider {}", index + 1)
        } else {
            name.to_string()
        }
    }
}

pub fn preset_by_key(key: &str) -> &'static Preset {
    PRESETS.iter().find(|p| p.key == key).unwrap_or(&PRESETS[0])
}

/// The credential-store slot of a connection's key; `None` for one that takes no key (a local
/// preset).
///
/// A custom endpoint keeps one key per URL, so a key typed for one server is never sent to
/// another; none while no URL is typed. Its key is optional: with none stored, nothing is sent.
pub fn key_slot(conn: Connection, preset: &str, custom: &str) -> Option<String> {
    match conn {
        Connection::ClaudeApi => Some("claude-api".into()),
        Connection::OpenAi => {
            let p = preset_by_key(preset);
            if p.local {
                None
            } else if p.base_url.is_empty() {
                let url = normalize_url(custom);
                (!url.is_empty()).then(|| format!("openai:custom:{url}"))
            } else {
                Some(format!("openai:{}", p.key))
            }
        }
        _ => None,
    }
}

/// A URL as a key slot names it: trimmed, scheme and host in lower case, no trailing `/`.
fn normalize_url(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    let start = url.find("://").map_or(0, |i| i + 3);
    let end = url[start..].find('/').map_or(url.len(), |i| start + i);
    format!("{}{}", url[..end].to_ascii_lowercase(), &url[end..])
}

/// Why a review's result cannot be applied in place.
pub const BLOCKED_CHANGED: &str = "The text changed while this was being written, so it cannot be applied in place. Copy the result, or run it again.";
pub const BLOCKED_READ_ONLY: &str =
    "This document is read-only, so the result cannot be applied. Copy the result instead.";
pub const BLOCKED_CLOSED: &str =
    "The document was closed, so the result cannot be applied. Copy the result instead.";

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

/// The text of a note made from a text file (`docs/ai-library-plan.md` 5.8): the model's answer
/// with the source file's own line breaks, no blank lines at its end, and exactly one line ending
/// after the last line. Empty for an answer that is blank; the caller refuses that.
pub fn note_body(reply: &str, eol: &str) -> String {
    let body = reply.trim_end_matches(['\n', '\r']);
    let lf = body.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = lf.trim_end();
    if trimmed.is_empty() {
        String::new()
    } else {
        with_eol(&format!("{trimmed}\n"), eol)
    }
}

// ---- saved prompts -----------------------------------------------------------------------------

/// The saved prompts file, beside settings.ini.
pub const PROMPTS_FILE: &str = "ai-prompts.md";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedPrompt {
    pub name: String,
    pub instruction: String,
    /// What the prompt is for: its optional `kind:` line (`docs/ai-library-plan.md` 3.4, 5.6).
    pub kind: PromptKind,
}

/// The kind of work a saved prompt does. A prompt with no `kind:` line is a rewrite, which is
/// what every prompt meant before kinds; a kind this version has no word for is kept apart and
/// offered nowhere, so it can never do another kind of thing than its line says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PromptKind {
    #[default]
    Rewrite,
    Split,
    Other,
}

impl PromptKind {
    /// The word of its `kind:` line.
    pub fn word(self) -> &'static str {
        match self {
            PromptKind::Rewrite => "rewrite",
            PromptKind::Split => "split",
            PromptKind::Other => "other",
        }
    }
}

/// The kind a line names, when it is a `kind:` line: `kind: split`, in any case, blanks around.
fn kind_line(line: &str) -> Option<PromptKind> {
    let (key, value) = line.trim().split_once(':')?;
    if !key.trim().eq_ignore_ascii_case("kind") {
        return None;
    }
    let value = value.trim();
    if value.is_empty() || value.contains(char::is_whitespace) {
        return None;
    }
    Some(match value.to_ascii_lowercase().as_str() {
        "rewrite" => PromptKind::Rewrite,
        "split" => PromptKind::Split,
        _ => PromptKind::Other,
    })
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

/// A line of an instruction split for [`escape_line`] and [`unescape_line`]: its indent, its
/// leading backslashes, and whether what follows them would read as a `## ` heading (at the start
/// of the line only) or a code fence.
fn line_parts(line: &str) -> (usize, usize, bool) {
    let ws = line.len() - line.trim_start().len();
    let rest = &line[ws..];
    let n = rest.len() - rest.trim_start_matches('\\').len();
    let after = &rest[n..];
    let special = (ws == 0 && after.starts_with("## "))
        || after.starts_with("```")
        || after.starts_with("~~~");
    (ws, n, special)
}

/// A line of an instruction as the prompts file holds it: a line that would read as a heading or
/// a fence (and one that already has backslashes before such a start) gets one backslash more,
/// which [`unescape_line`] takes off again, so the instruction reads back as it was typed.
pub fn escape_line(line: &str) -> String {
    let (ws, _, special) = line_parts(line);
    if special {
        format!("{}\\{}", &line[..ws], &line[ws..])
    } else {
        line.to_string()
    }
}

/// The instruction's line an [`escape_line`] line stands for. A line with no backslash before
/// such a start is left as it is: a fence in an instruction written by hand is its text.
pub fn unescape_line(line: &str) -> String {
    let (ws, n, special) = line_parts(line);
    if special && n > 0 {
        format!("{}{}", &line[..ws], &line[ws + 1..])
    } else {
        line.to_string()
    }
}

/// Reads the saved prompts: each `## Name` heading and the text under it to the next one. Empty
/// names and empty instructions are left out; a heading inside a code fence is text. A line
/// with a backslash before a `## ` or a fence reads without it ([`unescape_line`]).
pub fn parse_prompts(text: &str) -> Vec<SavedPrompt> {
    let mut out: Vec<SavedPrompt> = Vec::new();
    let mut cur: Option<(String, Vec<String>)> = None;
    let mut fence: Option<String> = None;
    let flush = |cur: &mut Option<(String, Vec<String>)>, out: &mut Vec<SavedPrompt>| {
        if let Some((name, mut lines)) = cur.take() {
            // The first line with text may say the prompt's kind; it is not the instruction.
            let mut kind = PromptKind::Rewrite;
            if let Some(i) = lines.iter().position(|l| !l.trim().is_empty())
                && let Some(k) = kind_line(&lines[i])
            {
                kind = k;
                lines.drain(..=i);
            }
            let instruction = lines.join("\n").trim().to_string();
            if !name.is_empty() && !instruction.is_empty() {
                out.push(SavedPrompt {
                    name,
                    instruction,
                    kind,
                });
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
            lines.push(if fence.is_none() {
                unescape_line(line)
            } else {
                line.to_string()
            });
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// The name a prompt saved from the instruction box gets: its first words (at most 40
/// characters, whole words), then " 2", " 3"… when a saved prompt already has that name.
pub fn prompt_name(instruction: &str, saved: &[SavedPrompt]) -> String {
    let mut base = String::new();
    for word in instruction.split_whitespace() {
        let next = if base.is_empty() {
            word.to_string()
        } else {
            format!("{base} {word}")
        };
        if next.chars().count() > 40 {
            break;
        }
        base = next;
    }
    if base.is_empty() {
        base = instruction.trim().chars().take(40).collect();
    }
    let base = base
        .trim_end_matches([',', '.', ';', ':', '#'])
        .trim()
        .to_string();
    let base = if base.is_empty() {
        "Saved prompt".to_string()
    } else {
        base
    };
    let taken = |n: &str| saved.iter().any(|p| p.name.eq_ignore_ascii_case(n));
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|k| format!("{base} {k}"))
        .find(|n| !taken(n))
        .unwrap_or(base)
}

/// `file` (the saved prompts file's text) with one more prompt at its end, as [`parse_prompts`]
/// reads it back: a `## name` heading and the instruction under it ([`escape_line`] for each of
/// its lines). The file's text is kept as it is, and what is added takes its line ending (CRLF
/// when the file has one). Whether it reads back is the caller's to check: a file that ends
/// inside a code fence takes the heading as text.
pub fn append_prompt(file: &str, name: &str, instruction: &str) -> String {
    append_prompt_of(file, name, instruction, PromptKind::Rewrite)
}

/// [`append_prompt`] for a prompt of `kind`: a split gets its `kind: split` line under the
/// heading. A rewrite gets none, unless its first line would read as one, when it gets
/// `kind: rewrite` first so that it reads back as typed.
pub fn append_prompt_of(file: &str, name: &str, instruction: &str, kind: PromptKind) -> String {
    let eol = if file.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out = file.to_string();
    if !out.is_empty() {
        if !out.ends_with('\n') {
            out.push_str(eol);
        }
        out.push_str(eol);
    }
    out.push_str("## ");
    out.push_str(name.trim());
    out.push_str(eol);
    out.push_str(eol);
    let first = instruction.trim().lines().next().unwrap_or("");
    if kind != PromptKind::Rewrite || kind_line(first).is_some() {
        out.push_str("kind: ");
        out.push_str(kind.word());
        out.push_str(eol);
        out.push_str(eol);
    }
    for line in instruction.trim().lines() {
        out.push_str(&escape_line(line));
        out.push_str(eol);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;

    #[test]
    fn a_notes_text_takes_the_files_line_endings_and_ends_with_one() {
        let reply = "PASTA\r\n\r\n- boil it\r\n\r\n\r\n";
        assert_eq!(note_body(reply, "\n"), "PASTA\n\n- boil it\n");
        assert_eq!(note_body(reply, "\r\n"), "PASTA\r\n\r\n- boil it\r\n");
        // No line ending in the source's habit does not mean none in the note's.
        assert_eq!(note_body("one line", "\n"), "one line\n");
        // An answer of nothing but blank lines is no note at all.
        assert_eq!(note_body(" \r\n\r\n", "\n"), "");
    }

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
            key_slot(Connection::OpenAi, "openrouter", "http://x").as_deref(),
            Some("openai:openrouter")
        );
        assert_eq!(
            key_slot(Connection::ClaudeApi, "", "").as_deref(),
            Some("claude-api")
        );
        assert_eq!(key_slot(Connection::Codex, "", ""), None);
        // A server on this computer takes no key: none is looked for.
        assert_eq!(key_slot(Connection::OpenAi, "lmstudio", ""), None);
        assert_eq!(key_slot(Connection::OpenAi, "ollama", ""), None);
        assert_eq!(
            CustomProvider::default().label(2),
            "Custom provider 3",
            "an unnamed custom provider"
        );
        // A new one takes the first number no provider goes by, not the count.
        let named = |n: &str| CustomProvider {
            name: n.into(),
            ..CustomProvider::default()
        };
        assert_eq!(new_custom_name(&[]), "Custom provider 1");
        let list = [named("LM Studio"), named("Custom provider 2")];
        assert_eq!(new_custom_name(&list), "Custom provider 1");
        let list = [named("Custom provider 1"), named("Custom provider 2")];
        assert_eq!(new_custom_name(&list), "Custom provider 3");
        // An unnamed one goes by its place.
        let list = [named("Custom provider 2"), CustomProvider::default()];
        assert_eq!(new_custom_name(&list), "Custom provider 1");
        let list = [
            CustomProvider::default(),
            named("x"),
            named("Custom provider 3"),
        ];
        assert_eq!(new_custom_name(&list), "Custom provider 2");
    }

    #[test]
    fn a_custom_endpoint_has_a_key_per_url() {
        let slot = |u| key_slot(Connection::OpenAi, "custom", u);
        assert_eq!(slot("  "), None, "no URL, no key");
        let a = slot("https://a.example/v1").unwrap();
        let b = slot("https://b.example/v1").unwrap();
        assert_ne!(a, b, "a key typed for one server is not sent to another");
        assert_eq!(a, "openai:custom:https://a.example/v1");
        assert_eq!(slot(" HTTPS://A.Example/v1/ ").unwrap(), a);
        // The path keeps its case: servers may tell paths apart by it.
        assert_ne!(slot("https://a.example/V1").unwrap(), a);
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
    fn a_prompt_saved_from_the_box_reads_back() {
        let saved = parse_prompts(PROMPTS_TEMPLATE);
        let name = prompt_name("Make the prices a table, sorted by date please", &saved);
        assert_eq!(name, "Make the prices a table, sorted by date");
        // A name already taken gets a number.
        assert_eq!(prompt_name("Shorten", &saved), "Shorten 2");
        assert_eq!(prompt_name("   ", &saved), "Saved prompt");
        let text = append_prompt(
            PROMPTS_TEMPLATE,
            &name,
            "Make the prices a table\n## not a heading",
        );
        let back = parse_prompts(&text);
        assert_eq!(back.len(), saved.len() + 1);
        let last = back.last().unwrap();
        assert_eq!(last.name, name);
        // Read back exactly as typed (audit 1 round 5).
        assert_eq!(
            last.instruction,
            "Make the prices a table\n## not a heading"
        );
        // Into an empty file.
        let one = parse_prompts(&append_prompt("", "A", "Do it."));
        assert_eq!(
            one,
            [SavedPrompt {
                name: "A".into(),
                instruction: "Do it.".into(),
                kind: PromptKind::Rewrite,
            }]
        );
    }

    /// Audit 1 round 5: what the box saves reads back as it was typed, whatever its lines look
    /// like, and the file's own text and line endings are kept.
    #[test]
    fn a_saved_prompt_reads_back_exactly_and_the_file_is_kept() {
        for typed in [
            "## Literal heading",
            "One\n```\ncode\n```\nTwo",
            "First\n  ~~~ indented fence",
            "\\## already escaped\n\\\\``` twice",
            "Plain \\ backslash and ## mid-line",
        ] {
            for file in [
                "",
                "## A\n\nDo a.\n",
                "## A\r\n\r\nDo a.   \r\n",
                "## A\n\nDo a.",
            ] {
                let out = append_prompt(file, "B", typed);
                assert!(out.starts_with(file), "{file:?}");
                let back = parse_prompts(&out);
                let b = back.iter().find(|p| p.name == "B").unwrap();
                assert_eq!(b.instruction, typed, "{typed:?} in {file:?}");
                if file.contains('\r') {
                    let added = &out[file.len()..];
                    assert!(!added.replace("\r\n", "").contains('\n'), "{added:?}");
                }
            }
        }
        for line in ["## x", "\\## x", "  ```", "plain", "  ## x", "\\\\~~~"] {
            assert_eq!(unescape_line(&escape_line(line)), line);
        }
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

    /// 5.6: a prompt carries its kind on an optional `kind:` line; none is a rewrite, and a kind
    /// this version does not know is kept apart.
    #[test]
    fn a_prompt_carries_its_kind_and_none_is_a_rewrite() {
        let file = "# Prompts\n\n## Chapters\n\nkind: split\n\nOne note per chapter.\n\n## Fix\n\nFix it.\n\n## File\n\n  Kind: File  \nBy topic.\n\n## Says kind\n\nkind: two words\nKeep it.\n";
        let p = parse_prompts(file);
        let kinds: Vec<(&str, PromptKind, &str)> = p
            .iter()
            .map(|p| (p.name.as_str(), p.kind, p.instruction.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("Chapters", PromptKind::Split, "One note per chapter."),
                ("Fix", PromptKind::Rewrite, "Fix it."),
                ("File", PromptKind::Other, "By topic."),
                (
                    "Says kind",
                    PromptKind::Rewrite,
                    "kind: two words\nKeep it."
                ),
            ]
        );
        // The template's prompts are all rewrites.
        assert!(
            parse_prompts(PROMPTS_TEMPLATE)
                .iter()
                .all(|p| p.kind == PromptKind::Rewrite)
        );
    }

    #[test]
    fn a_prompt_is_saved_with_its_kind_and_reads_back_as_typed() {
        let out = append_prompt_of("", "Chapters", "One note per chapter.", PromptKind::Split);
        assert_eq!(out, "## Chapters\n\nkind: split\n\nOne note per chapter.\n");
        let back = parse_prompts(&out);
        assert_eq!(back[0].kind, PromptKind::Split);
        assert_eq!(back[0].instruction, "One note per chapter.");
        // A rewrite whose first line would read as a kind line gets its own kind line first.
        let out = append_prompt("", "Odd", "kind: split\nthe rest");
        let back = parse_prompts(&out);
        assert_eq!(back[0].kind, PromptKind::Rewrite);
        assert_eq!(back[0].instruction, "kind: split\nthe rest");
        // A plain rewrite is written as before, with no kind line.
        assert_eq!(append_prompt("", "A", "Do it."), "## A\n\nDo it.\n");
    }
}
