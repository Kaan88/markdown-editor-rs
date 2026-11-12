//! Throwaway spike: incremental syntect on large files.
use std::time::Instant;
use syntect::highlighting::{HighlightIterator, HighlightState, Highlighter, Theme, ThemeSet};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    pub fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[self.below(v.len() as u64) as usize]
    }
}

const IDENTS: &[&str] = &[
    "buffer", "cursor", "line_count", "offset", "state", "width", "height", "items", "index", "value", "result", "parser",
    "cache", "layout", "run", "span", "token", "scope",
];
const TYPES: &[&str] = &["usize", "u32", "i64", "f32", "String", "Vec<u8>", "Option<usize>", "&str", "bool"];

fn to_lines(parts: Vec<String>, lines: usize) -> Vec<String> {
    let text: String = parts.concat();
    let mut v: Vec<String> = text.split_inclusive('\n').map(|s| s.to_string()).collect();
    v.truncate(lines);
    v
}

/// Rust file with doc comments, attributes, strings, raw strings, balanced block comments.
pub fn gen_rust(lines: usize) -> Vec<String> {
    let mut r = Rng(12345);
    let mut out: Vec<String> = Vec::new();
    out.push("//! Generated test file.\n".into());
    out.push("use std::collections::HashMap;\n".into());
    out.push("use std::fmt;\n".into());
    out.push("\n".into());
    let mut n = 0;
    while out.len() < lines {
        n += 1;
        match r.below(10) {
            0 | 1 => {
                out.push(format!("/// A record number {n} that holds some values.\n"));
                out.push("#[derive(Debug, Clone, PartialEq)]\n".into());
                out.push(format!("pub struct Record{n} {{\n"));
                for _ in 0..(2 + r.below(5)) {
                    out.push(format!("    pub {}: {},\n", r.pick(IDENTS), r.pick(TYPES)));
                }
                out.push("}\n\n".into());
            }
            2 => {
                out.push("/*\n".into());
                out.push(format!(" * Block comment {n}: explains the next function in some detail.\n"));
                out.push(" * It spans a few lines, as old-style comments do.\n".into());
                out.push(" */\n".into());
            }
            _ => {
                out.push(format!("/// Computes the thing number {n}.\n"));
                out.push(format!(
                    "pub fn compute_{n}({}: {}, {}: &mut Vec<u32>) -> Result<u32, String> {{\n",
                    r.pick(IDENTS),
                    r.pick(TYPES),
                    r.pick(IDENTS)
                ));
                for _ in 0..(6 + r.below(30)) {
                    let a = r.pick(IDENTS);
                    let b = r.pick(IDENTS);
                    let k = r.below(40);
                    let s = match r.below(14) {
                        0 => format!("    let {a} = {b}.len() as u32 + {k};\n"),
                        1 => format!("    // adjust {a} when {b} is large, see issue #{k}\n"),
                        2 => format!("    if {a} > {k} {{\n        {b}.push({k});\n    }}\n"),
                        3 => format!("    let msg = format!(\"{{}} items at {{}}\\n\", {a}, {k});\n"),
                        4 => format!("    for i in 0..{k} {{\n        {a} += i * 2;\n    }}\n"),
                        5 => format!("    match {a} {{\n        0 => {{}}\n        1..=9 => println!(\"small {{}}\", {a}),\n        _ => return Err(\"too big\".to_string()),\n    }}\n"),
                        6 => format!("    let raw = r#\"some \"quoted\" text {k}\"#;\n"),
                        7 => format!("    let c = '{}';\n", (b'a' + (k % 26) as u8) as char),
                        8 => format!("    let t = ({a}, {b}, {k}u8, 0x{k:x}, 1.5e3);\n"),
                        9 => format!("    let f = |x: u32| -> u32 {{ x.wrapping_mul({k}) }};\n"),
                        10 => format!("    #[allow(unused)]\n    let {a}: HashMap<String, u32> = HashMap::new();\n"),
                        11 => format!("    let s = \"line {k} with an escape \\t and unicode \\u{{1F600}}\";\n"),
                        12 => format!("    /* inline comment {k} */ let {a} = {b};\n"),
                        _ => format!("    {a}.iter().map(|v| v + {k}).filter(|v| *v % 2 == 0).count();\n"),
                    };
                    out.push(s);
                }
                out.push("    Ok(0)\n".into());
                out.push("}\n\n".into());
            }
        }
    }
    to_lines(out, lines)
}

const WORDS: &[&str] = &[
    "the", "editor", "keeps", "a", "rope", "of", "text", "and", "highlights", "lines", "slowly", "between", "events", "while",
    "typing", "stays", "fast", "because", "only", "visible", "rows", "matter",
];

fn sentence(r: &mut Rng, k: u64) -> String {
    let mut s = String::new();
    for i in 0..k {
        if i > 0 {
            s.push(' ');
        }
        match r.below(12) {
            0 => s.push_str(&format!("**{}**", r.pick(WORDS))),
            1 => s.push_str(&format!("*{}*", r.pick(WORDS))),
            2 => s.push_str(&format!("`{}()`", r.pick(IDENTS))),
            3 => s.push_str(&format!("[{}](https://example.com/{})", r.pick(WORDS), r.pick(IDENTS))),
            _ => s.push_str(r.pick(WORDS)),
        }
    }
    s
}

/// Markdown file: headings, paragraphs, lists, quotes, tables, fenced code of several languages.
pub fn gen_md(lines: usize) -> Vec<String> {
    let mut r = Rng(777);
    let mut out: Vec<String> = Vec::new();
    let mut n = 0;
    while out.len() < lines {
        n += 1;
        match r.below(8) {
            0 => out.push(format!("{} Section {n}\n\n", "#".repeat(1 + r.below(4) as usize))),
            1 => {
                for _ in 0..(2 + r.below(5)) {
                    let k = 3 + r.below(8);
                    out.push(format!("- {}\n", sentence(&mut r, k)));
                }
                out.push("\n".into());
            }
            2 => {
                let k = 3 + r.below(6);
                out.push(format!("> {}\n\n", sentence(&mut r, k)));
            }
            3 => {
                out.push("| name | value | note |\n|------|-------|------|\n".into());
                for _ in 0..(2 + r.below(4)) {
                    out.push(format!("| {} | {} | {} |\n", r.pick(IDENTS), r.below(1000), r.pick(WORDS)));
                }
                out.push("\n".into());
            }
            4 => {
                let lang = r.pick(&["rust", "json", "python", "bash", "js"]);
                out.push(format!("```{lang}\n"));
                for _ in 0..(3 + r.below(10)) {
                    out.push(match lang {
                        "rust" => format!("let {} = {}.len() + {};\n", r.pick(IDENTS), r.pick(IDENTS), r.below(99)),
                        "json" => format!("  \"{}\": [{}, \"{}\", true],\n", r.pick(IDENTS), r.below(99), r.pick(WORDS)),
                        "python" => format!("def {}(x):\n    return x + {}  # note\n", r.pick(IDENTS), r.below(99)),
                        "bash" => format!("echo \"{}\" | grep -n {} # c\n", r.pick(WORDS), r.pick(IDENTS)),
                        _ => format!("const {} = (a) => a * {} + \"{}\";\n", r.pick(IDENTS), r.below(99), r.pick(WORDS)),
                    });
                }
                out.push("```\n\n".into());
            }
            _ => {
                for _ in 0..(1 + r.below(3)) {
                    let k = 12 + r.below(30);
                    out.push(format!("{}\n", sentence(&mut r, k)));
                }
                out.push("\n".into());
            }
        }
    }
    to_lines(out, lines)
}

/// One long line of the given kind, `len` bytes (ends with \n).
pub fn long_line(kind: &str, len: usize) -> String {
    let mut s = String::new();
    let mut i = 0u32;
    match kind {
        "rust-code" => {
            while s.len() < len {
                i += 1;
                s.push_str(&format!("let a{i} = foo(b, \"s{i}\") + {i}; "));
            }
        }
        "json" => {
            s.push('[');
            while s.len() < len {
                i += 1;
                s.push_str(&format!("{{\"id\":{i},\"name\":\"item{i}\",\"ok\":true}},"));
            }
        }
        "md-prose" => {
            while s.len() < len {
                i += 1;
                s.push_str(&format!("word{i} **bold** and `code` [link](http://x.y/{i}) "));
            }
        }
        "blob" => {
            s.push_str("let data = \"");
            while s.len() < len {
                i = i.wrapping_mul(1103515245).wrapping_add(12345);
                s.push((b'A' + ((i >> 16) % 26) as u8) as char);
            }
            s.push_str("\";");
        }
        "minified-js" => {
            while s.len() < len {
                i += 1;
                s.push_str(&format!("function f{i}(a,b){{return a+b*{i}||\"x{i}\"}}var v{i}=[1,2,{i}];"));
            }
        }
        _ => panic!("kind"),
    }
    s.truncate(len);
    s.push('\n');
    s
}

pub struct Engine {
    pub set: SyntaxSet,
    pub theme: Theme,
}
impl Engine {
    pub fn load() -> Engine {
        let set = SyntaxSet::load_defaults_newlines();
        let theme = ThemeSet::load_defaults().themes.remove("base16-ocean.dark").unwrap();
        Engine { set, theme }
    }
    pub fn syntax(&self, ext: &str) -> &SyntaxReference {
        self.set.find_syntax_by_extension(ext).unwrap()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct State {
    pub ps: ParseState,
    pub hs: HighlightState,
}
impl State {
    pub fn new(syn: &SyntaxReference, hl: &Highlighter) -> State {
        State { ps: ParseState::new(syn), hs: HighlightState::new(hl, ScopeStack::new()) }
    }
}

/// Parse + style one line, advance state. Returns a checksum so the work is not elided.
#[inline]
pub fn do_line(st: &mut State, line: &str, set: &SyntaxSet, hl: &Highlighter) -> u64 {
    let ops = st.ps.parse_line(line, set).expect("parse");
    let mut sum = 0u64;
    for (style, text) in HighlightIterator::new(&mut st.hs, &ops, line, hl) {
        sum = sum.wrapping_add(text.len() as u64 * (style.foreground.r as u64 + 1));
    }
    sum
}

/// Parse only (no styling), for comparison.
#[inline]
pub fn parse_only(ps: &mut ParseState, line: &str, set: &SyntaxSet) -> usize {
    ps.parse_line(line, set).expect("parse").len()
}

pub fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// (median, min, max)
pub fn stats(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[0], v[v.len() - 1])
}

pub fn pct(v: &mut Vec<f64>, p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * p) as usize]
}

pub fn time<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let t = Instant::now();
    let r = f();
    (r, ms(t.elapsed()))
}

pub struct MemInfo {
    pub private: usize,
    pub working: usize,
    pub peak_working: usize,
}
pub fn mem_info() -> MemInfo {
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let mut c: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c as *mut _ as *mut _, c.cb);
        MemInfo { private: c.PrivateUsage, working: c.WorkingSetSize, peak_working: c.PeakWorkingSetSize }
    }
}
