//! Snippets: a trigger word that expands to a block of text with tab stops. Headless.
//!
//! The file (`snippets.toml` beside the settings) is a small subset of TOML, one table per
//! snippet:
//!
//! ```toml
//! [[snippet]]
//! trigger = "todo"
//! types = ["markdown", "text"]   # optional: the file types it is for (a language's name)
//! text = "- [ ] ${1:task}$0"
//! ```
//!
//! `text` is a basic string (`"..."`, with `\n`, `\t`, `\"` and `\\`), a multi-line one
//! (`"""..."""`) or a literal one (`'...'`, `'''...'''`). A line that cannot be read is skipped.
//!
//! In the text, `$1`, `$2`, ... and `${1:default}` are tab stops, visited in number order, and
//! `$0` is where the caret ends (the end of the text when there is none). A number used twice
//! stays a stop at its first place only. `$$` is a dollar sign. Line breaks become the
//! document's, and each line after the first gets the indentation of the line the snippet was
//! typed on (a leading tab becomes one indent unit).

use crate::complete::is_word_char;
use crate::limits::{SNIPPET_TEXT_MAX, SNIPPETS_MAX};
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snippet {
    pub trigger: String,
    /// Lower-case language names ("markdown", "rust", "text" for a plain one); empty is every
    /// type.
    pub types: Vec<String>,
    pub text: String,
}

impl Snippet {
    /// Whether the snippet is for a document of this language (`name` as `Language::name` gives
    /// it, or "" for plain text).
    pub fn applies(&self, language: &str) -> bool {
        if self.types.is_empty() {
            return true;
        }
        let l = language.to_lowercase();
        let l = match l.as_str() {
            "" | "plain text" => "text",
            "md" => "markdown",
            other => other,
        };
        self.types.iter().any(|t| t == l)
    }
}

/// What ships: a few for Markdown.
pub fn builtin() -> Vec<Snippet> {
    let md = |trigger: &str, text: &str| Snippet {
        trigger: trigger.into(),
        types: vec!["markdown".into()],
        text: text.into(),
    };
    vec![
        md(
            "table",
            "| ${1:Column} | ${2:Column} |\n| --- | --- |\n| ${3:Cell} | ${4:Cell} |\n$0",
        ),
        md("code", "```${1:text}\n$0\n```"),
        md("link", "[${1:text}](${2:https://})$0"),
    ]
}

/// The text of a new snippets file: how to write one, and nothing active.
pub const TEMPLATE: &str = "\
# Snippets for mdedit: type the trigger and press Ctrl+Space, then Enter or Tab.
#
# [[snippet]]
# trigger = \"todo\"                 # letters, digits and _ only
# types = [\"markdown\", \"text\"]     # optional: the file types it is for; leave out for all
# text = \"- [ ] ${1:task}$0\"
#
# In the text, $1 $2 ... and ${1:default} are stops that Tab moves between, $0 is where the
# caret ends, and $$ is a dollar sign. A snippet here replaces a built-in one (table, code,
# link) with the same trigger. Save the file: the next completion uses it.
";

struct Cursor<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Cursor<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }

    fn eat(&mut self, p: &str) -> bool {
        if self.rest().starts_with(p) {
            self.i += p.len();
            true
        } else {
            false
        }
    }

    fn skip_blanks(&mut self) {
        let r = self.rest();
        self.i += r.len() - r.trim_start_matches([' ', '\t']).len();
    }

    fn skip_line(&mut self) {
        match self.rest().find('\n') {
            Some(n) => self.i += n + 1,
            None => self.i = self.s.len(),
        }
    }

    /// The rest of the line has only blanks or a comment.
    fn at_line_end(&mut self) -> bool {
        self.skip_blanks();
        let r = self.rest();
        r.is_empty() || r.starts_with(['\n', '\r', '#'])
    }
}

fn unescape_into(out: &mut String, raw: &str) {
    let mut it = raw.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(o) => out.push(o),
            None => out.push('\\'),
        }
    }
}

/// A string value at the cursor: basic, literal, or the multi-line forms of each.
fn string(c: &mut Cursor) -> Option<String> {
    for (open, multi, literal) in [
        ("\"\"\"", true, false),
        ("'''", true, true),
        ("\"", false, false),
        ("'", false, true),
    ] {
        if !c.eat(open) {
            continue;
        }
        if multi {
            // A newline right after the opening is not part of the text.
            if !c.eat("\r\n") {
                c.eat("\n");
            }
        }
        let start = c.i;
        let mut i = c.i;
        let bytes = c.s.as_bytes();
        loop {
            if i >= bytes.len() {
                return None;
            }
            if !literal && bytes[i] == b'\\' {
                i += 2;
                continue;
            }
            if c.s[i..].starts_with(open) {
                break;
            }
            if !multi && bytes[i] == b'\n' {
                return None;
            }
            i += 1;
        }
        let raw = c.s.get(start..i)?;
        c.i = i + open.len();
        let mut out = String::new();
        if literal {
            out.push_str(raw);
        } else {
            unescape_into(&mut out, raw);
        }
        return Some(out.replace("\r\n", "\n"));
    }
    None
}

enum Value {
    Text(String),
    List(Vec<String>),
}

fn value(c: &mut Cursor) -> Option<Value> {
    if c.eat("[") {
        let mut v = Vec::new();
        loop {
            c.skip_blanks();
            if c.eat("]") {
                return Some(Value::List(v));
            }
            v.push(string(c)?);
            c.skip_blanks();
            if !c.eat(",") {
                c.skip_blanks();
                return c.eat("]").then_some(Value::List(v));
            }
        }
    }
    string(c).map(Value::Text)
}

/// Reads a snippets file. Never fails: what cannot be read is left out.
pub fn parse(text: &str) -> Vec<Snippet> {
    let mut out = Vec::new();
    let mut cur: Option<(Option<String>, Vec<String>, Option<String>)> = None;
    let mut c = Cursor { s: text, i: 0 };
    let finish = |cur: &mut Option<(Option<String>, Vec<String>, Option<String>)>,
                  out: &mut Vec<Snippet>| {
        if let Some((Some(trigger), types, Some(text))) = cur.take()
            && !trigger.is_empty()
            && trigger.chars().all(is_word_char)
            && trigger.len() <= crate::limits::COMPLETE_WORD_MAX
            && text.len() <= SNIPPET_TEXT_MAX
            && out.len() < SNIPPETS_MAX
        {
            out.push(Snippet {
                trigger,
                types,
                text,
            });
        }
    };
    while c.i < text.len() {
        c.skip_blanks();
        if c.rest().starts_with(['\n', '\r', '#']) {
            c.skip_line();
            continue;
        }
        if c.rest().starts_with('[') {
            let table = c.eat("[[snippet]]") || c.eat("[snippet]");
            finish(&mut cur, &mut out);
            if table && c.at_line_end() {
                cur = Some((None, Vec::new(), None));
            }
            c.skip_line();
            continue;
        }
        let r = c.rest();
        let key_len = r
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
            .unwrap_or(r.len());
        let key = &r[..key_len];
        c.i += key_len;
        c.skip_blanks();
        if key.is_empty() || !c.eat("=") {
            c.skip_line();
            continue;
        }
        c.skip_blanks();
        let v = value(&mut c);
        match (v, cur.as_mut()) {
            (Some(v), Some(e)) if c.at_line_end() => match (key, v) {
                ("trigger", Value::Text(t)) => e.0 = Some(t),
                ("text", Value::Text(t)) => e.2 = Some(t),
                ("types" | "type", Value::List(l)) => {
                    e.1 = l.into_iter().map(|t| t.to_lowercase()).collect()
                }
                ("types" | "type", Value::Text(t)) => e.1 = vec![t.to_lowercase()],
                _ => {}
            },
            _ => {}
        }
        c.skip_line();
    }
    finish(&mut cur, &mut out);
    out
}

/// A snippet's text made ready to insert.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expanded {
    pub text: String,
    /// The stops in the order Tab visits them, as byte ranges of `text` (a default text is a
    /// non-empty range). The last is where the caret ends and always exists.
    pub stops: Vec<Range<usize>>,
}

/// Expands `body`: stops are read, line breaks become `eol` followed by `indent`, a leading tab
/// of a later line becomes `unit`.
pub fn expand(body: &str, indent: &str, unit: &str, eol: &str) -> Expanded {
    let mut text = String::new();
    let mut found: Vec<(usize, Range<usize>)> = Vec::new();
    let mut at_line_start = false;
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;
    let push = |text: &mut String, c: char, at_line_start: &mut bool| match c {
        '\r' => {}
        '\n' => {
            text.push_str(eol);
            text.push_str(indent);
            *at_line_start = true;
        }
        '\t' if *at_line_start => text.push_str(unit),
        _ => {
            text.push(c);
            *at_line_start = false;
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c != '$' {
            push(&mut text, c, &mut at_line_start);
            i += 1;
            continue;
        }
        let next = chars.get(i + 1).copied();
        match next {
            Some('$') => {
                text.push('$');
                at_line_start = false;
                i += 2;
            }
            Some(d) if d.is_ascii_digit() => {
                let mut j = i + 1;
                let mut n = 0usize;
                while j < chars.len() && chars[j].is_ascii_digit() && n < 100_000 {
                    n = n * 10 + chars[j].to_digit(10).unwrap_or(0) as usize;
                    j += 1;
                }
                found.push((n, text.len()..text.len()));
                at_line_start = false;
                i = j;
            }
            Some('{') => {
                // ${N} or ${N:default}
                let mut j = i + 2;
                let mut n = 0usize;
                let mut digits = 0;
                while j < chars.len() && chars[j].is_ascii_digit() && digits < 6 {
                    n = n * 10 + chars[j].to_digit(10).unwrap_or(0) as usize;
                    j += 1;
                    digits += 1;
                }
                let close = chars[j.min(chars.len())..].iter().position(|&ch| ch == '}');
                match (digits > 0, chars.get(j), close) {
                    (true, Some('}'), _) => {
                        found.push((n, text.len()..text.len()));
                        at_line_start = false;
                        i = j + 1;
                    }
                    (true, Some(':'), Some(end)) => {
                        let start = text.len();
                        let end = j + end;
                        for &ch in &chars[j + 1..end] {
                            push(&mut text, ch, &mut at_line_start);
                        }
                        found.push((n, start..text.len()));
                        at_line_start = false;
                        i = end + 1;
                    }
                    _ => {
                        text.push('$');
                        i += 1;
                    }
                }
            }
            _ => {
                text.push('$');
                at_line_start = false;
                i += 1;
            }
        }
    }
    // First place of each number; 1, 2, ... in order, then 0 (or the end).
    let mut seen = Vec::new();
    let mut numbered: Vec<(usize, Range<usize>)> = Vec::new();
    let mut zero = None;
    for (n, r) in found {
        if seen.contains(&n) {
            continue;
        }
        seen.push(n);
        if n == 0 {
            zero = Some(r);
        } else {
            numbered.push((n, r));
        }
    }
    numbered.sort_by_key(|(n, _)| *n);
    let mut stops: Vec<Range<usize>> = numbered.into_iter().map(|(_, r)| r).collect();
    stops.push(zero.unwrap_or(text.len()..text.len()));
    Expanded { text, stops }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_read() {
        let s = parse(
            "# my snippets\n\n[[snippet]]\ntrigger = \"todo\"\ntypes = [\"Markdown\", \"text\"]\n\
             text = \"- [ ] ${1:task}$0\"\n\n[[snippet]]\ntrigger = 'fn1'\ntext = '''\nfn $1() {\n\t$0\n}\n'''\n\
             [[snippet]]\ntrigger = \"esc\"  # why\ntext = \"a\\tb\\n\\\"q\\\" \\\\ $$\"\n",
        );
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].trigger, "todo");
        assert_eq!(s[0].types, ["markdown", "text"]);
        assert_eq!(s[0].text, "- [ ] ${1:task}$0");
        assert_eq!(s[1].trigger, "fn1");
        assert!(s[1].types.is_empty());
        assert_eq!(s[1].text, "fn $1() {\n\t$0\n}\n");
        assert_eq!(s[2].text, "a\tb\n\"q\" \\ $$");
    }

    #[test]
    fn bad_entries_are_skipped() {
        let s = parse(
            "[[snippet]]\ntrigger = \"no text\"\n[[snippet]]\ntrigger = \"bad trigger\"\ntext = \"x\"\n\
             [[snippet]]\ntrigger = \"\"\ntext = \"x\"\n[[snippet]]\ntrigger = \"ok\"\ntext = \"y\"\n\
             [[snippet]]\ntrigger = \"open\"\ntext = \"never closed\n",
        );
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].trigger, "ok");
        assert!(parse("").is_empty());
        assert!(parse("garbage = = =\n[[snippet\n\u{0}").is_empty());
        // Text outside a table is ignored.
        assert!(parse("trigger = \"a\"\ntext = \"b\"\n").is_empty());
    }

    #[test]
    fn the_template_holds_nothing_active() {
        assert!(parse(TEMPLATE).is_empty());
        // Uncommented, its example is a valid snippet.
        let on: String = TEMPLATE
            .lines()
            .filter(|l| {
                l.starts_with("# ")
                    && !l.contains("Snippets for")
                    && !l.contains("In the text")
                    && !l.starts_with("# caret")
                    && !l.starts_with("# link")
            })
            .map(|l| format!("{}\n", &l[2..]))
            .collect();
        let s = parse(&on);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].trigger, "todo");
    }

    #[test]
    fn the_builtin_ones_expand() {
        for s in builtin() {
            assert!(s.applies("Markdown"));
            assert!(!s.applies("Rust"));
            let e = expand(&s.text, "", "\t", "\n");
            assert!(!e.stops.is_empty());
        }
    }

    #[test]
    fn types_decide_where_a_snippet_applies() {
        let s = |types: &[&str]| Snippet {
            trigger: "x".into(),
            types: types.iter().map(|t| (*t).into()).collect(),
            text: String::new(),
        };
        assert!(s(&[]).applies("Rust"));
        assert!(s(&["rust"]).applies("Rust"));
        assert!(!s(&["rust"]).applies(""));
        assert!(s(&["text"]).applies(""));
        assert!(s(&["text"]).applies("Plain Text"));
        assert!(s(&["markdown"]).applies("Markdown"));
    }

    #[test]
    fn stops_are_in_number_order_with_zero_last() {
        let e = expand("a $2 b ${1:one} c $0 d", "", "", "\n");
        assert_eq!(e.text, "a  b one c  d");
        assert_eq!(e.stops, [5..8, 2..2, 11..11]);
        let e = expand("plain", "", "", "\n");
        assert_eq!(e.stops.len(), 1);
        assert_eq!(e.stops[0], 5..5);
        let e = expand("x$0y", "", "", "\n");
        assert_eq!(e.stops.len(), 1);
        assert_eq!(e.stops[0], 1..1);
    }

    #[test]
    fn repeated_numbers_stop_at_the_first_place() {
        let e = expand("$1-${1:again}-$2", "", "", "\n");
        assert_eq!(e.text, "-again-");
        assert_eq!(e.stops, [0..0, 7..7, 7..7]);
    }

    #[test]
    fn dollars_and_odd_forms_stay_text() {
        let e = expand("$$5 $x ${a} ${1 $", "", "", "\n");
        assert_eq!(e.text, "$5 $x ${a} ${1 $");
        assert_eq!(e.stops.len(), 1);
        assert_eq!(e.stops[0], e.text.len()..e.text.len());
    }

    #[test]
    fn line_breaks_take_the_document_eol_and_the_indent() {
        let e = expand("a\n\tb\n$0", "  ", "    ", "\r\n");
        assert_eq!(e.text, "a\r\n      b\r\n  ");
        assert_eq!(e.stops.len(), 1);
        assert_eq!(e.stops[0], e.text.len()..e.text.len());
        let e = expand("a\r\nb", "", "", "\n");
        assert_eq!(e.text, "a\nb");
    }

    #[test]
    fn multibyte_text_keeps_byte_offsets() {
        let e = expand("é${1:ü}$0", "", "", "\n");
        assert_eq!(e.text, "éü");
        assert_eq!(e.stops, [2..4, 4..4]);
    }
}
