//! The function list: the headings, functions and types of a document with the line each starts
//! on, found by cheap line-wise heuristics. There is no parser; each line is looked at alone
//! (a few rules look at the next lines), so the list is a guide and not a promise. A false miss
//! is preferred to a false hit. The shell runs [`symbols`] on a worker thread over a rope
//! snapshot.
//!
//! A line of more than 4096 bytes is skipped unread, so one huge line costs nothing. Leading
//! spaces and tabs are the line's indentation (a tab counts as 4 columns). Names are trimmed and
//! cut to [`NAME_MAX`] bytes at a char boundary. Block comments and strings are not tracked,
//! except fenced code in Markdown.

use ropey::Rope;
use std::sync::atomic::{AtomicBool, Ordering};

/// A name is cut to this many bytes (at a char boundary).
pub const NAME_MAX: usize = 120;

/// A line longer than this many bytes is not looked at.
const LINE_MAX: usize = 4096;

/// The cancel flag is checked every this many lines.
const CANCEL_EVERY: usize = 4096;

/// How far the closing line of Markdown front matter is looked for.
const FRONT_MATTER_MAX: usize = 200;

/// What a symbol is, which the list may show with a small mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Heading,
    Function,
    Type,
    Section,
}

/// One entry of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    /// 0-based line the symbol starts on.
    pub line: usize,
    /// Nesting for the list's indent: heading level - 1 for Markdown; for code, 0 at top level
    /// and 1 for something indented (inside a type, impl or class); never more than 5.
    pub depth: u8,
    pub kind: Kind,
    /// What the list shows: the heading text, or the function/type name (for a function, the
    /// name only, without parameters; for Rust `impl` blocks the text after `impl` up to `{`
    /// or `where`, e.g. `Ring<T>` or `fmt::Display for Ring<T>`).
    pub name: String,
}

/// Which set of rules reads a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Flavor {
    Markdown,
    Rust,
    Python,
    CLike,
    Shell,
    Sections,
    Ruby,
    Lua,
    Css,
}

impl Flavor {
    /// From a syntax name as syntect's bundled set gives it ("Rust", "Python", "Markdown",
    /// "C", "C++", "Bourne Again Shell (bash)", "TOML", ...). Matching is case-insensitive and
    /// generous; `None` for plain text and for languages with no rule.
    pub fn for_language(name: &str) -> Option<Flavor> {
        let n = name.trim().to_lowercase();
        let n = n.as_str();
        let starts = |p: &str| n.starts_with(p);
        if n.contains("markdown") {
            Some(Flavor::Markdown)
        } else if starts("rust") {
            Some(Flavor::Rust)
        } else if starts("python") {
            Some(Flavor::Python)
        } else if starts("ruby") {
            Some(Flavor::Ruby)
        } else if starts("lua") {
            Some(Flavor::Lua)
        } else if starts("css") || starts("scss") || n == "less" || starts("less ") {
            Some(Flavor::Css)
        } else if starts("toml") || n == "ini" || starts("ini ") || starts("git config") {
            Some(Flavor::Sections)
        } else if !n.contains("powershell")
            && (n.contains("bash") || n.contains("shell") || n.contains("zsh"))
        {
            Some(Flavor::Shell)
        } else if n == "c"
            || starts("c ")
            || starts("c++")
            || starts("c#")
            || n == "d"
            || n == "go"
            || n == "dart"
            || starts("objective-c")
            || starts("java")
            || starts("typescript")
            || starts("tsx")
            || starts("php")
            || starts("kotlin")
            || starts("swift")
            || starts("scala")
            || starts("groovy")
            || starts("glsl")
        {
            Some(Flavor::CLike)
        } else {
            None
        }
    }
}

/// The symbols of `text`, in line order, at most `max`. The bool is true when there were more
/// or `cancel` was set (checked at least every 4096 lines; a flag set before the call gives an
/// empty list).
pub fn symbols(
    text: &Rope,
    flavor: Flavor,
    max: usize,
    cancel: &AtomicBool,
) -> (Vec<Symbol>, bool) {
    let mut out: Vec<Symbol> = Vec::new();
    let total = text.len_lines();
    let mut buf = String::new();
    let mut scratch = String::new();
    let mut md = Md::default();
    let mut skip_to = None;
    if flavor == Flavor::Markdown {
        skip_to = front_matter_end(text, &mut buf, total);
    }
    for i in 0..total {
        if i % CANCEL_EVERY == 0 && cancel.load(Ordering::Relaxed) {
            return (out, true);
        }
        if skip_to.is_some_and(|end| i <= end) {
            continue;
        }
        if !read_line(text, i, &mut buf) {
            md.prev = None;
            continue;
        }
        let (cols, rest) = indent(&buf);
        let found: Option<(usize, u8, Kind, &str)>;
        let owned: String;
        match flavor {
            Flavor::Markdown => match md.line(i, cols, rest) {
                Some((line, depth, name)) => {
                    owned = name;
                    found = Some((line, depth, Kind::Heading, &owned));
                }
                None => continue,
            },
            _ => {
                let hit = match flavor {
                    Flavor::Rust => rust_line(rest, cols),
                    Flavor::Python => python_line(rest, cols),
                    Flavor::CLike => {
                        let mut allman = || next_is_brace(text, i + 1, total, &mut scratch);
                        clike_line(rest, cols, &mut allman)
                    }
                    Flavor::Shell => shell_line(rest),
                    Flavor::Sections => section_line(rest),
                    Flavor::Ruby => ruby_line(rest, cols),
                    Flavor::Lua => lua_line(rest, cols),
                    Flavor::Css => css_line(rest, cols),
                    Flavor::Markdown => None,
                };
                match hit {
                    Some((depth, kind, name)) => found = Some((i, depth, kind, name)),
                    None => continue,
                }
            }
        }
        if let Some((line, depth, kind, name)) = found {
            let name = clip(name);
            if name.is_empty() {
                continue;
            }
            if out.len() >= max {
                return (out, true);
            }
            out.push(Symbol {
                line,
                depth: depth.min(5),
                kind,
                name,
            });
        }
    }
    (out, false)
}

/// Reads line `i` into `buf` without its break; false (and `buf` unusable) when the line is
/// longer than `LINE_MAX` bytes.
fn read_line(text: &Rope, i: usize, buf: &mut String) -> bool {
    buf.clear();
    let line = text.line(i);
    if line.len_bytes() > LINE_MAX {
        return false;
    }
    for chunk in line.chunks() {
        buf.push_str(chunk);
    }
    while buf.ends_with(['\n', '\r']) {
        buf.pop();
    }
    true
}

/// Trims and cuts a name to `NAME_MAX` bytes at a char boundary.
fn clip(s: &str) -> String {
    let s = s.trim();
    let mut end = s.len().min(NAME_MAX);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].trim_end().to_string()
}

/// The indentation in columns (tab = 4) and the rest of the line.
fn indent(s: &str) -> (usize, &str) {
    let mut cols = 0;
    for (i, b) in s.bytes().enumerate() {
        match b {
            b' ' => cols += 1,
            b'\t' => cols += 4,
            _ => return (cols, &s[i..]),
        }
    }
    (cols, "")
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// The leading run of identifier characters.
fn ident(s: &str) -> &str {
    let end = s.find(|c| !is_ident_char(c)).unwrap_or(s.len());
    &s[..end]
}

/// The leading identifier and what follows it.
fn word(s: &str) -> (&str, &str) {
    let w = ident(s);
    (w, &s[w.len()..])
}

/// What follows `kw` when `s` starts with it as a whole word.
fn starts_word<'a>(s: &'a str, kw: &str) -> Option<&'a str> {
    let r = s.strip_prefix(kw)?;
    match r.chars().next() {
        Some(c) if is_ident_char(c) => None,
        _ => Some(r),
    }
}

/// What follows `kw` and the blanks after it, when `kw` is a whole word followed by a blank.
fn kw_rest<'a>(s: &'a str, kw: &str) -> Option<&'a str> {
    let r = starts_word(s, kw)?;
    if r.starts_with([' ', '\t']) {
        Some(r.trim_start())
    } else {
        None
    }
}

/// An identifier that may be qualified with `.` or `::`.
fn qualified(s: &str) -> &str {
    let end = s
        .find(|c: char| !(is_ident_char(c) || c == '.' || c == ':'))
        .unwrap_or(s.len());
    s[..end].trim_end_matches([':', '.'])
}

/// Skips a balanced `<...>` at the start of `s`.
fn skip_angles(s: &str) -> &str {
    if !s.starts_with('<') {
        return s;
    }
    let mut depth = 0usize;
    for (i, b) in s.bytes().enumerate() {
        match b {
            b'<' => depth += 1,
            b'>' => {
                depth -= 1;
                if depth == 0 {
                    return s[i + 1..].trim_start();
                }
            }
            _ => {}
        }
    }
    s
}

/// The byte index of the `)` that closes the `(` at `open`.
fn matching_paren(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, b) in s.bytes().enumerate().skip(open) {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

type Hit<'a> = Option<(u8, Kind, &'a str)>;

fn level(indented: bool) -> u8 {
    u8::from(indented)
}

// ---------------------------------------------------------------------------------------------
// Markdown

/// Where the front matter at the top of the text ends (its closing line), when there is one.
fn front_matter_end(text: &Rope, buf: &mut String, total: usize) -> Option<usize> {
    if !read_line(text, 0, buf) || buf.trim_end() != "---" {
        return None;
    }
    for j in 1..total.min(FRONT_MATTER_MAX) {
        if read_line(text, j, buf) {
            let t = buf.trim_end();
            if t == "---" || t == "..." {
                return Some(j);
            }
        }
    }
    None
}

#[derive(Default)]
struct Md {
    /// The open fence: its character and the length of its opening run.
    fence: Option<(u8, usize)>,
    /// The line before, when it is paragraph text a setext underline could belong to.
    prev: Option<(usize, String)>,
}

/// A run of at least three backticks or tildes: the character and the run's length.
fn fence_run(s: &str) -> Option<(u8, usize)> {
    let c = *s.as_bytes().first()?;
    if c != b'`' && c != b'~' {
        return None;
    }
    let n = s.bytes().take_while(|&b| b == c).count();
    (n >= 3).then_some((c, n))
}

fn is_list_marker(s: &str) -> bool {
    let b = s.as_bytes();
    match b.first() {
        Some(b'-' | b'*' | b'+') => b.len() == 1 || b[1] == b' ' || b[1] == b'\t',
        Some(b'0'..=b'9') => {
            let d = b.iter().take_while(|c| c.is_ascii_digit()).count();
            d <= 9
                && matches!(b.get(d), Some(b'.' | b')'))
                && (b.len() == d + 1 || b[d + 1] == b' ' || b[d + 1] == b'\t')
        }
        _ => false,
    }
}

impl Md {
    /// One line: the heading it completes, as (line, depth, name).
    fn line(&mut self, i: usize, cols: usize, rest: &str) -> Option<(usize, u8, String)> {
        if let Some((ch, n)) = self.fence {
            if cols < 4
                && let Some((c, k)) = fence_run(rest)
                && c == ch
                && k >= n
                && rest[k..].trim().is_empty()
            {
                self.fence = None;
            }
            self.prev = None;
            return None;
        }
        let t = rest.trim_end();
        if t.is_empty() || cols >= 4 {
            self.prev = None;
            return None;
        }
        if let Some(run) = fence_run(t) {
            self.fence = Some(run);
            self.prev = None;
            return None;
        }
        let hashes = t.bytes().take_while(|&b| b == b'#').count();
        if (1..=6).contains(&hashes) && (t.len() == hashes || t[hashes..].starts_with([' ', '\t']))
        {
            self.prev = None;
            let mut text = t[hashes..].trim();
            let open = text.trim_end_matches('#');
            if open.is_empty() || open.ends_with([' ', '\t']) {
                text = open.trim_end();
            }
            if text.is_empty() {
                return None;
            }
            return Some((i, (hashes - 1) as u8, text.to_string()));
        }
        let all = |c: char| t.chars().all(|x| x == c);
        let underline = if all('=') {
            Some(0)
        } else if t.len() >= 2 && all('-') {
            Some(1)
        } else {
            None
        };
        if let Some(depth) = underline {
            return self.prev.take().map(|(line, name)| (line, depth, name));
        }
        let rule = t
            .chars()
            .all(|c| matches!(c, '-' | '*' | '_' | '=' | ' ' | '\t'));
        if rule || t.starts_with('>') || is_list_marker(t) {
            self.prev = None;
        } else {
            self.prev = Some((i, clip(t)));
        }
        None
    }
}

// ---------------------------------------------------------------------------------------------
// Rust

/// Removes the modifiers in front of an item: `pub`, `pub(...)`, `async`, `const`, `unsafe`,
/// `extern "C"` and `default`.
fn rust_strip(mut s: &str) -> &str {
    'again: loop {
        if let Some(r) = starts_word(s, "pub") {
            s = r;
            if s.starts_with('(') {
                match s.find(')') {
                    Some(i) => s = &s[i + 1..],
                    None => return "",
                }
            }
            s = s.trim_start();
            continue;
        }
        for m in ["async", "const", "unsafe", "default"] {
            if let Some(r) = starts_word(s, m) {
                s = r.trim_start();
                continue 'again;
            }
        }
        if let Some(r) = starts_word(s, "extern") {
            let r = r.trim_start();
            s = if let Some(q) = r.strip_prefix('"') {
                match q.find('"') {
                    Some(i) => q[i + 1..].trim_start(),
                    None => return "",
                }
            } else {
                r
            };
            continue;
        }
        return s;
    }
}

/// The byte index where a `where` clause starts in the text after `impl`.
fn where_clause(s: &str) -> usize {
    for (i, _) in s.match_indices("where") {
        let before = s[..i].chars().next_back();
        let after = s[i + 5..].chars().next();
        if before.is_some_and(char::is_whitespace)
            && after.is_none_or(|c| c.is_whitespace() || c == '{')
        {
            return i;
        }
    }
    s.len()
}

fn rust_line(rest: &str, cols: usize) -> Hit<'_> {
    let indented = cols > 0;
    let d = level(indented);
    let s = rust_strip(rest);
    if let Some(r) = kw_rest(s, "fn") {
        return Some((d, Kind::Function, ident(r)));
    }
    for kw in ["struct", "enum", "trait", "union", "type"] {
        if kw == "type" && indented {
            continue;
        }
        if let Some(r) = kw_rest(s, kw) {
            return Some((d, Kind::Type, ident(r)));
        }
    }
    if let Some(r) = starts_word(s, "impl") {
        let r = skip_angles(r.trim_start());
        let end = r.find('{').unwrap_or(r.len());
        let r = &r[..end];
        let r = &r[..where_clause(r)];
        return Some((d, Kind::Type, r));
    }
    if let Some(r) = kw_rest(s, "mod") {
        let name = ident(r);
        if r[name.len()..].trim_start().starts_with('{') {
            return Some((d, Kind::Section, name));
        }
        return None;
    }
    if let Some(r) = s.strip_prefix("macro_rules!") {
        return Some((d, Kind::Function, ident(r.trim_start())));
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Python and Ruby

fn python_line(rest: &str, cols: usize) -> Hit<'_> {
    let d = (cols / 4).min(5) as u8;
    let s = match starts_word(rest, "async") {
        Some(r) => r.trim_start(),
        None => rest,
    };
    if let Some(r) = kw_rest(s, "def") {
        return Some((d, Kind::Function, ident(r)));
    }
    if let Some(r) = kw_rest(rest, "class") {
        return Some((d, Kind::Type, ident(r)));
    }
    None
}

fn ruby_line(rest: &str, cols: usize) -> Hit<'_> {
    let d = (cols / 2).min(5) as u8;
    if let Some(r) = kw_rest(rest, "def") {
        let mut end = r
            .find(|c: char| !(is_ident_char(c) || matches!(c, '.' | '?' | '!')))
            .unwrap_or(r.len());
        if r[end..].starts_with("=(") {
            end += 1;
        }
        return Some((d, Kind::Function, &r[..end]));
    }
    for kw in ["class", "module"] {
        if let Some(r) = kw_rest(rest, kw) {
            return Some((d, Kind::Type, qualified(r)));
        }
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Shell, sections, Lua, CSS

fn name_run<'a>(s: &'a str, extra: &[char]) -> &'a str {
    let end = s
        .find(|c: char| !(is_ident_char(c) || extra.contains(&c)))
        .unwrap_or(s.len());
    &s[..end]
}

fn shell_line(rest: &str) -> Hit<'_> {
    const EXTRA: &[char] = &['-', '.', ':', '@'];
    if let Some(r) = kw_rest(rest, "function") {
        return Some((0, Kind::Function, name_run(r, EXTRA)));
    }
    let name = name_run(rest, EXTRA);
    let after = rest[name.len()..].trim_start().strip_prefix('(')?;
    after
        .trim_start()
        .starts_with(')')
        .then_some((0, Kind::Function, name))
}

fn section_line(rest: &str) -> Hit<'_> {
    let (inner, after) = if let Some(r) = rest.strip_prefix("[[") {
        let e = r.find("]]")?;
        (&r[..e], &r[e + 2..])
    } else {
        let r = rest.strip_prefix('[')?;
        let e = r.find(']')?;
        (&r[..e], &r[e + 1..])
    };
    let after = after.trim();
    if !(after.is_empty() || after.starts_with('#') || after.starts_with(';')) {
        return None;
    }
    let name = inner.trim();
    if name.is_empty() || name.contains(',') {
        return None;
    }
    let mut quote = None;
    let mut dots = 0;
    for c in name.chars() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '.') => dots += 1,
            _ => {}
        }
    }
    Some((dots.min(5), Kind::Section, name))
}

fn lua_line(rest: &str, cols: usize) -> Hit<'_> {
    const EXTRA: &[char] = &['.', ':'];
    let d = level(cols > 0);
    let s = kw_rest(rest, "local").unwrap_or(rest);
    if let Some(r) = kw_rest(s, "function") {
        let name = name_run(r, EXTRA);
        return r[name.len()..]
            .trim_start()
            .starts_with('(')
            .then_some((d, Kind::Function, name));
    }
    let name = name_run(s, EXTRA);
    let after = s[name.len()..].trim_start().strip_prefix('=')?;
    if after.starts_with('=') {
        return None;
    }
    starts_word(after.trim_start(), "function").map(|_| (d, Kind::Function, name))
}

fn css_line(rest: &str, cols: usize) -> Hit<'_> {
    if cols > 0 || rest.starts_with('}') {
        return None;
    }
    let sel = rest.trim_end().strip_suffix('{')?;
    Some((0, Kind::Section, sel))
}

// ---------------------------------------------------------------------------------------------
// C-like languages

/// Words that start a statement, never a definition.
const CONTROL: &[&str] = &[
    "if",
    "else",
    "elseif",
    "for",
    "foreach",
    "while",
    "switch",
    "return",
    "do",
    "catch",
    "try",
    "finally",
    "case",
    "new",
    "delete",
    "throw",
    "sizeof",
    "using",
    "goto",
    "go",
    "defer",
    "lock",
    "when",
    "synchronized",
    "await",
    "yield",
    "assert",
];

/// Keywords that are not a function's name either.
const KEYWORDS: &[&str] = &[
    "function",
    "func",
    "fun",
    "def",
    "class",
    "struct",
    "enum",
    "interface",
    "namespace",
    "trait",
    "object",
    "protocol",
];

/// Words in front of a declaration that say nothing about what it is.
const MODIFIERS: &[&str] = &[
    "export",
    "default",
    "public",
    "private",
    "protected",
    "static",
    "final",
    "abstract",
    "sealed",
    "data",
    "open",
    "internal",
    "partial",
    "typedef",
    "async",
    "fileprivate",
    "declare",
];

/// The text before a `//` comment, without trailing blanks.
fn strip_comment(s: &str) -> &str {
    match s.find("//") {
        Some(i) => s[..i].trim_end(),
        None => s.trim_end(),
    }
}

/// `const NAME = function`, `= async (`, `= (...) =>`, `= x =>`.
fn js_function(r: &str) -> Option<&str> {
    let name = ident(r);
    if name.is_empty() {
        return None;
    }
    let after = r[name.len()..].trim_start().strip_prefix('=')?;
    if after.starts_with(['=', '>']) {
        return None;
    }
    let mut rhs = after.trim_start();
    let mut is_async = false;
    if let Some(r) = starts_word(rhs, "async") {
        rhs = r.trim_start();
        is_async = true;
    }
    let yes = if starts_word(rhs, "function").is_some() {
        true
    } else if rhs.starts_with('(') {
        is_async
            || matching_paren(rhs, 0).is_some_and(|c| {
                let tail = rhs[c + 1..].trim_start();
                tail.starts_with("=>") || (tail.starts_with(':') && tail.contains("=>"))
            })
    } else {
        let id = ident(rhs);
        !id.is_empty() && rhs[id.len()..].trim_start().starts_with("=>")
    };
    yes.then_some(name)
}

/// The keyword forms: types, `function`, `func`, `fun`, `def`, JS arrow functions.
fn clike_keyword<'a>(s: &'a str, code: &str) -> Option<(Kind, &'a str)> {
    let (w, r) = word(s);
    let ws = r.starts_with([' ', '\t']);
    match w {
        "class" | "struct" | "interface" | "enum" | "namespace" | "trait" | "object"
        | "protocol"
            if ws =>
        {
            let mut r = r.trim_start();
            if w == "enum"
                && let (c @ ("class" | "struct"), x) = word(r)
                && !c.is_empty()
                && x.starts_with([' ', '\t'])
            {
                r = x.trim_start();
            }
            let name = qualified(r);
            if name.is_empty() || name == "extends" || name == "implements" || code.ends_with(';') {
                return None;
            }
            let after = &r[name.len()..];
            let t = after.trim_start();
            let ok = t.is_empty()
                || t.starts_with(['{', ':', '<', '['])
                || after.starts_with('(')
                || [
                    "extends",
                    "implements",
                    "where",
                    "with",
                    "permits",
                    "inherits",
                ]
                .iter()
                .any(|k| starts_word(t, k).is_some());
            ok.then_some((Kind::Type, name))
        }
        "type" if ws => {
            let r = r.trim_start();
            let name = ident(r);
            let t = r[name.len()..].trim_start();
            let ok = !name.is_empty()
                && (starts_word(t, "struct").is_some()
                    || starts_word(t, "interface").is_some()
                    || (t.starts_with('=') && !t.starts_with("=="))
                    || t.starts_with('<'));
            ok.then_some((Kind::Type, name))
        }
        "function" if ws || r.starts_with('*') => {
            let r = r.trim_start().trim_start_matches('*').trim_start();
            let name = ident(r);
            (!name.is_empty()).then_some((Kind::Function, name))
        }
        "func" if ws || r.starts_with('(') => {
            let mut r = r.trim_start();
            if r.starts_with('(') {
                r = r[matching_paren(r, 0)? + 1..].trim_start();
            }
            let name = ident(r);
            (!name.is_empty()).then_some((Kind::Function, name))
        }
        "fun" | "def" if ws => {
            let r = skip_angles(r.trim_start());
            let head = r[..r.find(['(', '[', '=', ':']).unwrap_or(r.len())].trim_end();
            let name = &head[head.rfind(|c: char| !is_ident_char(c)).map_or(0, |i| i + 1)..];
            (!name.is_empty()).then_some((Kind::Function, name))
        }
        "const" | "let" | "var" if ws => js_function(r.trim_start()).map(|n| (Kind::Function, n)),
        _ => None,
    }
}

/// The generic C-family definition: `ret NAME(args) {`, or `ret NAME(args)` with `{` alone on
/// the next line. `allman` says whether the next non-blank line is exactly `{`.
fn clike_function<'a>(
    code: &'a str,
    indented: bool,
    allman: &mut dyn FnMut() -> bool,
) -> Option<&'a str> {
    if code.ends_with(';') || code.contains("=>") {
        return None;
    }
    let open = code.find('(')?;
    let close = matching_paren(code, open)?;
    let tail = code[close + 1..].trim();
    if tail.contains(';') {
        return None;
    }
    let head = code[..open].trim_end();
    let eq_part = match head.find("operator") {
        Some(i) => &head[..i],
        None => head,
    };
    if eq_part.contains('=') {
        return None;
    }
    let start = match head.rfind("operator") {
        Some(i) if !head[..i].ends_with(is_ident_char) => i,
        _ => {
            let mut start = head.len();
            for (i, c) in head.char_indices().rev() {
                if is_ident_char(c) || c == ':' || c == '~' {
                    start = i;
                } else {
                    break;
                }
            }
            start
        }
    };
    let name = &head[start..];
    let plain = name.trim_start_matches('~');
    if plain.starts_with("operator") {
        // Any operator spelling is a name.
    } else {
        let first = plain.chars().next()?;
        if !(first.is_alphabetic() || first == '_' || first == '$')
            || !plain.split("::").all(|p| !p.is_empty() && !p.contains(':'))
            || CONTROL.contains(&plain)
            || KEYWORDS.contains(&plain)
        {
            return None;
        }
    }
    let before = &head[..start];
    if before.ends_with('.') || before.ends_with("->") {
        return None;
    }
    let before = before.trim_end();
    if indented && before.is_empty() {
        return None;
    }
    let last = before
        .rsplit(|c: char| !is_ident_char(c))
        .next()
        .unwrap_or("");
    if matches!(
        last,
        "new" | "else" | "return" | "throw" | "delete" | "await"
    ) {
        return None;
    }
    let ends_ok = if tail.is_empty() {
        allman()
    } else {
        tail.ends_with('{')
    };
    ends_ok.then_some(name)
}

fn clike_line<'a>(
    rest: &'a str,
    cols: usize,
    allman: &mut dyn FnMut() -> bool,
) -> Option<(u8, Kind, &'a str)> {
    let d = level(cols > 0);
    if rest.is_empty()
        || rest.starts_with("//")
        || rest.starts_with("/*")
        || rest.starts_with('*')
        || rest.starts_with('#')
        || rest.starts_with('}')
    {
        return None;
    }
    let code = strip_comment(rest);
    for kw in ["@interface", "@implementation", "@protocol"] {
        if let Some(r) = code.strip_prefix(kw)
            && r.starts_with([' ', '\t'])
            && !code.ends_with(';')
        {
            return Some((d, Kind::Type, ident(r.trim_start())));
        }
    }
    let mut s = code;
    loop {
        let (w, r) = word(s);
        if r.starts_with([' ', '\t']) && MODIFIERS.contains(&w) {
            s = r.trim_start();
        } else {
            break;
        }
    }
    if let Some((kind, name)) = clike_keyword(s, code) {
        return Some((d, kind, name));
    }
    if CONTROL.contains(&ident(code)) {
        return None;
    }
    clike_function(code, cols > 0, allman).map(|name| (d, Kind::Function, name))
}

/// Whether the first non-blank line from `from` on (within a few lines) is exactly `{`.
fn next_is_brace(text: &Rope, from: usize, total: usize, scratch: &mut String) -> bool {
    for j in from..total.min(from + 4) {
        if !read_line(text, j, scratch) {
            return false;
        }
        let t = scratch.trim();
        if !t.is_empty() {
            return t == "{";
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    type Row = (usize, u8, Kind, String);

    fn run(flavor: Flavor, s: &str) -> Vec<Row> {
        let cancel = AtomicBool::new(false);
        let (v, more) = symbols(&Rope::from_str(s), flavor, usize::MAX, &cancel);
        assert!(!more);
        v.into_iter()
            .map(|s| (s.line, s.depth, s.kind, s.name))
            .collect()
    }

    fn row(line: usize, depth: u8, kind: Kind, name: &str) -> Row {
        (line, depth, kind, name.to_string())
    }

    use Kind::{Function as F, Heading as H, Section as S, Type as T};

    #[test]
    fn markdown_headings_fences_and_setext() {
        let s = "\
# Title
intro
## Sub ##
```
# not heading
```
Setext
======
Other
-----
- item
---
~~~
# no
~~~
### C#
#######  seven
#
";
        assert_eq!(
            run(Flavor::Markdown, s),
            vec![
                row(0, 0, H, "Title"),
                row(2, 1, H, "Sub"),
                row(6, 0, H, "Setext"),
                row(8, 1, H, "Other"),
                row(15, 2, H, "C#"),
            ]
        );
    }

    #[test]
    fn markdown_front_matter_is_not_a_setext_heading() {
        let s = "---\ntitle: x\n---\n# Real\n";
        assert_eq!(run(Flavor::Markdown, s), vec![row(3, 0, H, "Real")]);
    }

    #[test]
    fn rust_items() {
        let s = "\
use std::fmt;
pub struct Ring<T> {
    items: Vec<T>,
}
impl<T> Ring<T> {
    pub fn new() -> Self {
    pub(crate) async fn go(&self) {
    type Item = u8;
}
impl fmt::Display for Ring<T> where T: Copy {
mod inner {
mod x;
macro_rules! sq {
// fn commented() {
pub unsafe extern \"C\" fn ffi() {}
pub enum E { A }
trait Tr {
const X: u32 = 1;
type Alias = u8;
";
        assert_eq!(
            run(Flavor::Rust, s),
            vec![
                row(1, 0, T, "Ring"),
                row(4, 0, T, "Ring<T>"),
                row(5, 1, F, "new"),
                row(6, 1, F, "go"),
                row(9, 0, T, "fmt::Display for Ring<T>"),
                row(10, 0, S, "inner"),
                row(12, 0, F, "sq"),
                row(14, 0, F, "ffi"),
                row(15, 0, T, "E"),
                row(16, 0, T, "Tr"),
                row(18, 0, T, "Alias"),
            ]
        );
    }

    #[test]
    fn python_defs() {
        let s = "import os\nclass A:\n    def f(self):\n        pass\n    async def g(self):\n        def deep():\ndef top():\n";
        assert_eq!(
            run(Flavor::Python, s),
            vec![
                row(1, 0, T, "A"),
                row(2, 1, F, "f"),
                row(4, 1, F, "g"),
                row(5, 2, F, "deep"),
                row(6, 0, F, "top"),
            ]
        );
    }

    #[test]
    fn c_functions_and_control_statements() {
        let s = "\
#include <stdio.h>
static int add(int a, int b) {
    return a + b;
}
int main(void)
{
    if (x) {
    }
    for (i = 0; i < n; i++) {
    while (f(x)) {
    return f(x);
    foo(bar);
    } else if (y) {
}
struct Point {
};
struct Point;
struct Point *make(void) {
typedef struct Node {
int x = f(1);
";
        assert_eq!(
            run(Flavor::CLike, s),
            vec![
                row(1, 0, F, "add"),
                row(4, 0, F, "main"),
                row(14, 0, T, "Point"),
                row(17, 0, F, "make"),
                row(18, 0, T, "Node"),
            ]
        );
    }

    #[test]
    fn cpp_method_and_operator() {
        let s = "void Ring::push(int x) {\nbool Ring::operator==(const Ring& o) const {\nRing::~Ring() {\nclass Ring : public Base {\nenum class Color {\nnamespace a::b {\n";
        assert_eq!(
            run(Flavor::CLike, s),
            vec![
                row(0, 0, F, "Ring::push"),
                row(1, 0, F, "operator=="),
                row(2, 0, F, "Ring::~Ring"),
                row(3, 0, T, "Ring"),
                row(4, 0, T, "Color"),
                row(5, 0, T, "a::b"),
            ]
        );
    }

    #[test]
    fn java_class_with_methods() {
        let s = "\
public class Ring {
    private int n;
    public Ring(int n) {
        this.n = n;
    }
    public void push(int x) throws Exception {
        if (x > 0) {
            list.add(x);
        }
    }
    @Override
    public String toString()
    {
        return \"\";
    }
}
";
        assert_eq!(
            run(Flavor::CLike, s),
            vec![
                row(0, 0, T, "Ring"),
                row(2, 1, F, "Ring"),
                row(5, 1, F, "push"),
                row(11, 1, F, "toString"),
            ]
        );
    }

    #[test]
    fn javascript_and_typescript() {
        let s = "\
export const add = (a, b) => a + b;
const f = async (x) => {
const g = function () {
let h = x => x * 2;
const total = (a + b) * c;
export default class App extends Base {
async function* gen() {
describe('x', () => {
foo(function () {
interface Props {
type Id = string;
const y = 5;
";
        assert_eq!(
            run(Flavor::CLike, s),
            vec![
                row(0, 0, F, "add"),
                row(1, 0, F, "f"),
                row(2, 0, F, "g"),
                row(3, 0, F, "h"),
                row(5, 0, T, "App"),
                row(6, 0, F, "gen"),
                row(9, 0, T, "Props"),
                row(10, 0, T, "Id"),
            ]
        );
    }

    #[test]
    fn go_swift_kotlin_scala() {
        let s = "\
func (r *Ring) Push(x int) {
func main() {
type Ring struct {
fun <T> List<T>.second(): T {
def run(x: Int) = {
object Main {
func greet(name: String) -> String {
";
        assert_eq!(
            run(Flavor::CLike, s),
            vec![
                row(0, 0, F, "Push"),
                row(1, 0, F, "main"),
                row(2, 0, T, "Ring"),
                row(3, 0, F, "second"),
                row(4, 0, F, "run"),
                row(5, 0, T, "Main"),
                row(6, 0, F, "greet"),
            ]
        );
    }

    #[test]
    fn php_functions() {
        let s = "<?php\nclass A {\n    public static function make($x) {\n}\nfunction helper() {\n";
        assert_eq!(
            run(Flavor::CLike, s),
            vec![
                row(1, 0, T, "A"),
                row(2, 1, F, "make"),
                row(4, 0, F, "helper"),
            ]
        );
    }

    #[test]
    fn shell_functions() {
        let s = "#!/bin/bash\nbuild() {\nfunction deploy {\n  clean ()\n{\nfoo-bar() { :; }\nls -l\nfoo();\n";
        assert_eq!(
            run(Flavor::Shell, s),
            vec![
                row(1, 0, F, "build"),
                row(2, 0, F, "deploy"),
                row(3, 0, F, "clean"),
                row(5, 0, F, "foo-bar"),
                row(7, 0, F, "foo"),
            ]
        );
    }

    #[test]
    fn toml_and_ini_sections() {
        let s = "title = \"x\"\n[package]\nname = \"a\"\n[[bin]]\n[dependencies.serde]\n[a.\"b.c\"] # note\narr = [\n  [1, 2],\n]\n[]\n[x] y\n";
        assert_eq!(
            run(Flavor::Sections, s),
            vec![
                row(1, 0, S, "package"),
                row(3, 0, S, "bin"),
                row(4, 1, S, "dependencies.serde"),
                row(5, 1, S, "a.\"b.c\""),
            ]
        );
    }

    #[test]
    fn ruby_defs() {
        let s = "module M\n  class A < B\n    def f\n    def self.g(x)\n    def ok?\n  end\nend\n";
        assert_eq!(
            run(Flavor::Ruby, s),
            vec![
                row(0, 0, T, "M"),
                row(1, 1, T, "A"),
                row(2, 2, F, "f"),
                row(3, 2, F, "self.g"),
                row(4, 2, F, "ok?"),
            ]
        );
    }

    #[test]
    fn lua_functions() {
        let s = "function M.f(a)\nlocal function g()\nh = function(x)\n  function inner()\nlocal t = {}\nx == function\n";
        assert_eq!(
            run(Flavor::Lua, s),
            vec![
                row(0, 0, F, "M.f"),
                row(1, 0, F, "g"),
                row(2, 0, F, "h"),
                row(3, 1, F, "inner"),
            ]
        );
    }

    #[test]
    fn css_rules() {
        let s =
            "body {\n  color: red;\n}\n@media (max-width: 600px) {\n  .a {\n  }\n}\na:hover, b {\n";
        assert_eq!(
            run(Flavor::Css, s),
            vec![
                row(0, 0, S, "body"),
                row(3, 0, S, "@media (max-width: 600px)"),
                row(7, 0, S, "a:hover, b"),
            ]
        );
    }

    #[test]
    fn max_caps_and_reports_more() {
        let s = "fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\nfn e() {}\n";
        let rope = Rope::from_str(s);
        let cancel = AtomicBool::new(false);
        let (v, more) = symbols(&rope, Flavor::Rust, 3, &cancel);
        assert_eq!((v.len(), more), (3, true));
        let (v, more) = symbols(&rope, Flavor::Rust, 5, &cancel);
        assert_eq!((v.len(), more), (5, false));
        let (v, more) = symbols(&rope, Flavor::Rust, 6, &cancel);
        assert_eq!((v.len(), more), (5, false));
        let (v, more) = symbols(&rope, Flavor::Rust, 0, &cancel);
        assert_eq!((v.len(), more), (0, true));
    }

    #[test]
    fn cancel_set_before_the_call() {
        let rope = Rope::from_str("fn a() {}\nfn b() {}\n");
        let cancel = AtomicBool::new(true);
        assert_eq!(symbols(&rope, Flavor::Rust, 10, &cancel), (vec![], true));
    }

    #[test]
    fn a_huge_line_is_skipped_quickly() {
        let big = "x".repeat(1 << 20);
        let s = format!("{big}\nfn real() {{}}\nfn {big}() {{}}\n");
        let t = Instant::now();
        let got = run(Flavor::Rust, &s);
        assert!(t.elapsed().as_millis() < 500, "{:?}", t.elapsed());
        assert_eq!(got, vec![row(1, 0, F, "real")]);
        let s = format!("{big}\n# Head\n");
        assert_eq!(run(Flavor::Markdown, &s), vec![row(1, 0, H, "Head")]);
    }

    #[test]
    fn names_are_cut_at_a_char_boundary() {
        let s = format!("# {}\n", "é".repeat(100));
        let got = run(Flavor::Markdown, &s);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].3.len(), NAME_MAX);
        assert!(got[0].3.chars().all(|c| c == 'é'));
    }

    #[test]
    fn crlf_lines() {
        let s = "# One\r\ntext\r\n===\r\n## Two\r\n";
        assert_eq!(
            run(Flavor::Markdown, s),
            vec![
                row(0, 0, H, "One"),
                row(1, 0, H, "text"),
                row(3, 1, H, "Two")
            ]
        );
    }

    #[test]
    fn language_names() {
        use Flavor::*;
        let cases: &[(&str, Option<Flavor>)] = &[
            ("Rust", Some(Rust)),
            ("Python", Some(Python)),
            ("Markdown", Some(Markdown)),
            ("C", Some(CLike)),
            ("C++", Some(CLike)),
            ("C#", Some(CLike)),
            ("Java", Some(CLike)),
            ("JavaScript", Some(CLike)),
            ("JavaScript (Babel)", Some(CLike)),
            ("Go", Some(CLike)),
            ("PHP", Some(CLike)),
            ("Kotlin", Some(CLike)),
            ("Swift", Some(CLike)),
            ("Scala", Some(CLike)),
            ("D", Some(CLike)),
            ("Objective-C", Some(CLike)),
            ("Bourne Again Shell (bash)", Some(Shell)),
            ("Shell-Unix-Generic", Some(Shell)),
            ("TOML", Some(Sections)),
            ("INI", Some(Sections)),
            ("YAML", None),
            ("Plain Text", None),
            ("PowerShell", None),
            ("Ruby", Some(Ruby)),
            ("Lua", Some(Lua)),
            ("CSS", Some(Css)),
            ("SCSS", Some(Css)),
            ("Less", Some(Css)),
            ("rust", Some(Rust)),
            ("", None),
        ];
        for (name, want) in cases {
            assert_eq!(Flavor::for_language(name), *want, "{name}");
        }
    }

    #[test]
    #[ignore = "timing: run with --ignored --nocapture"]
    fn timing_50_mb_rust_like() {
        let chunk = "use std::fmt;\n\npub struct Ring<T> {\n    items: Vec<T>,\n}\n\nimpl<T> Ring<T> {\n    pub fn new() -> Self {\n        let x = foo(1, 2);\n        Ring { items: Vec::new() }\n    }\n}\n\n// a comment line that is here to pad the text out a little\n";
        let mut s = String::with_capacity(51 << 20);
        while s.len() < 50 << 20 {
            s.push_str(chunk);
        }
        let rope = Rope::from_str(&s);
        let cancel = AtomicBool::new(false);
        let t = Instant::now();
        let (v, more) = symbols(&rope, Flavor::Rust, usize::MAX, &cancel);
        println!(
            "outline: {} lines, {} symbols in {} ms",
            rope.len_lines(),
            v.len(),
            t.elapsed().as_millis()
        );
        assert!(!more);
        let t = Instant::now();
        let (v, _) = symbols(&rope, Flavor::CLike, usize::MAX, &cancel);
        println!(
            "outline (CLike): {} symbols in {} ms",
            v.len(),
            t.elapsed().as_millis()
        );
    }
}
