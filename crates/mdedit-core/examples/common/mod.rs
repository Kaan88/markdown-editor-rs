//! The deterministic generators shared by the `gen` and `bench` examples. The `lines` mix is the
//! buffer spike's (`spikes/buffer`), so the files match the ones the plan's numbers came from.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }

    pub fn below(&mut self, n: usize) -> usize {
        ((self.next() >> 11) % n as u64) as usize
    }
}

const IDS: &[&str] = &[
    "buffer", "index", "line", "offset", "count", "result", "value", "node", "chunk", "width",
    "height", "cursor", "state", "token", "parser", "render", "layout", "config", "path", "name",
    "size", "text", "items", "range", "start", "end", "len", "data", "ctx", "frame",
];
const TYPES: &[&str] = &[
    "Buffer", "Node", "Chunk", "Span", "Token", "Layout", "Config", "Cursor", "State", "Item",
];
const WORDS: &[&str] = &[
    "the",
    "line",
    "is",
    "computed",
    "lazily",
    "when",
    "needed",
    "because",
    "scrolling",
    "must",
    "stay",
    "cheap",
    "and",
    "we",
    "never",
    "shape",
    "text",
    "that",
    "is",
    "not",
    "visible",
    "cache",
    "per",
    "block",
];
const TR: &[&str] = &[
    "Bu satır dosyanın başlangıcını gösterir",
    "Şimdi imleci bir karakter sağa taşıyoruz",
    "Çıkış değeri boş olabilir, ğüşıöç kontrol edilmeli",
    "İstanbul'da öğle yemeği için çalışma durduruldu",
    "Kullanıcı girdisi doğrulanmadan önce kaçış yapılır",
    "Güncelleme işlemi başarısız oldu, yeniden dene",
];
const CJK_FIXED: &[&str] = &["这是一个测试", "日本語のコメント", "한국어 주석"];

fn pick<'a>(r: &mut Rng, v: &[&'a str]) -> &'a str {
    v[r.below(v.len())]
}

fn ident(r: &mut Rng, s: &mut String) {
    s.push_str(pick(r, IDS));
    if r.below(4) == 0 {
        s.push_str(&r.below(100).to_string());
    }
}

fn gen_line(r: &mut Rng, s: &mut String) {
    let k = r.below(100);
    let indent = r.below(5) * 4;
    if k < 7 {
        return; // blank
    }
    for _ in 0..indent {
        s.push(' ');
    }
    match k {
        7..=29 => {
            s.push_str("// ");
            for i in 0..4 + r.below(9) {
                if i > 0 {
                    s.push(' ');
                }
                s.push_str(pick(r, WORDS));
            }
        }
        30..=32 => {
            s.push_str("// ");
            s.push_str(pick(r, TR));
        }
        33..=35 => {
            s.push_str("// ");
            if r.below(2) == 0 {
                s.push_str(pick(r, CJK_FIXED));
            }
            for _ in 0..4 + r.below(14) {
                s.push(char::from_u32(0x4E00 + r.below(0x9FA5 - 0x4E00) as u32).unwrap());
            }
        }
        96..=99 => {
            // A long line.
            s.push_str("const DATA: &str = \"");
            for i in 0..30 + r.below(60) {
                if i > 0 {
                    s.push(' ');
                }
                s.push_str(pick(r, WORDS));
            }
            s.push_str("\";");
        }
        _ => match r.below(8) {
            0 => {
                s.push_str("let ");
                ident(r, s);
                s.push_str(" = ");
                ident(r, s);
                s.push('.');
                ident(r, s);
                s.push('(');
                ident(r, s);
                s.push_str(", ");
                s.push_str(&r.below(10000).to_string());
                s.push_str(");");
            }
            1 => {
                s.push_str("fn ");
                ident(r, s);
                s.push('(');
                ident(r, s);
                s.push_str(": &str, ");
                ident(r, s);
                s.push_str(": usize) -> Option<");
                s.push_str(pick(r, TYPES));
                s.push_str("> {");
            }
            2 => {
                s.push_str("if ");
                ident(r, s);
                s.push_str(" > ");
                s.push_str(&r.below(1000).to_string());
                s.push_str(" && ");
                ident(r, s);
                s.push_str(".is_empty() {");
            }
            3 => {
                s.push_str("return Ok(");
                ident(r, s);
                s.push_str(");");
            }
            4 => s.push('}'),
            5 => {
                s.push_str("pub struct ");
                s.push_str(pick(r, TYPES));
                s.push_str(" { pub ");
                ident(r, s);
                s.push_str(": Vec<");
                s.push_str(pick(r, TYPES));
                s.push_str(">, }");
            }
            6 => {
                s.push_str("println!(\"Merhaba dünya, şöyle çıktı: {}\", ");
                ident(r, s);
                s.push_str(");");
            }
            _ => {
                s.push_str("for ");
                ident(r, s);
                s.push_str(" in 0..");
                s.push_str(&r.below(5000).to_string());
                s.push_str(" {");
            }
        },
    }
}

fn create(path: &Path) -> io::Result<BufWriter<File>> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    Ok(BufWriter::with_capacity(1 << 20, File::create(path)?))
}

/// Code-like lines up to `mib` MiB, each ended with `eol`.
pub fn gen_lines(mib: usize, path: &Path, eol: &str) -> io::Result<()> {
    let target = mib * 1024 * 1024;
    let mut w = create(path)?;
    let mut r = Rng(0x9E3779B97F4A7C15);
    let mut s = String::new();
    let (mut written, mut lines) = (0usize, 0usize);
    while written < target {
        s.clear();
        gen_line(&mut r, &mut s);
        s.push_str(eol);
        w.write_all(s.as_bytes())?;
        written += s.len();
        lines += 1;
    }
    w.flush()?;
    println!(
        "wrote {} bytes, {} lines, {:.1} bytes/line to {}",
        written,
        lines,
        written as f64 / lines as f64,
        path.display()
    );
    Ok(())
}

const JS_TOKENS: &[&str] = &[
    "function(",
    "return ",
    "e.length",
    "\"str\"",
    "=>",
    "var ",
    "t[n]",
    "if(",
    ")",
    "{",
    "}",
    ";",
    ",",
    "r.push(",
    "this.",
    "void 0",
    "null",
    "e=",
    "n++",
    "i<o",
    "(0,",
    "typeof ",
    "case 1:",
    "break;",
    "new Array(",
    "a.b.c(",
    "!0",
    "!1",
    "[]",
    "{}",
    "\"\\u00e7\"",
    "0x1f",
    "12.5",
    "&&",
    "||",
    "===",
    "else ",
    "for(",
    "while(",
];

/// One line of minified-JavaScript-like text of `mib` MiB and no line break.
pub fn gen_long(mib: usize, path: &Path) -> io::Result<()> {
    let target = mib * 1024 * 1024;
    let mut w = create(path)?;
    let mut r = Rng(0xD1B54A32D192ED03);
    let mut written = 0usize;
    while written < target {
        let t = JS_TOKENS[r.below(JS_TOKENS.len())];
        w.write_all(t.as_bytes())?;
        written += t.len();
    }
    w.flush()?;
    println!("wrote {} bytes in one line to {}", written, path.display());
    Ok(())
}
