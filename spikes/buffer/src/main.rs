//! Phase 0 spike "buffer": ropey vs a hand-written piece table vs String + line starts.
//!
//! Modes:
//!   buffer gen <mib> <path>               write a deterministic test file
//!   buffer run <impl> <path> [ops]        one measurement in this process (prints "key value" lines)
//!   buffer bench <runs> <impls> <path> [ops]   spawn `run` many times, print median (min-max)
//! impl is one of: ropey, piece, string

use std::cell::Cell;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::Instant;

// ---------------------------------------------------------------- memory

fn mem() -> (usize, usize) {
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let mut c: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c as *mut _ as *mut _, c.cb);
        (c.PrivateUsage, c.PeakWorkingSetSize)
    }
}

// ---------------------------------------------------------------- rng

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        ((self.next() >> 11) % n as u64) as usize
    }
}

// ---------------------------------------------------------------- generator

const IDS: &[&str] = &[
    "buffer", "index", "line", "offset", "count", "result", "value", "node", "chunk", "width", "height",
    "cursor", "state", "token", "parser", "render", "layout", "config", "path", "name", "size", "text",
    "items", "range", "start", "end", "len", "data", "ctx", "frame",
];
const TYPES: &[&str] = &["Buffer", "Node", "Chunk", "Span", "Token", "Layout", "Config", "Cursor", "State", "Item"];
const WORDS: &[&str] = &[
    "the", "line", "is", "computed", "lazily", "when", "needed", "because", "scrolling", "must", "stay",
    "cheap", "and", "we", "never", "shape", "text", "that", "is", "not", "visible", "cache", "per", "block",
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
            // long line
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

fn gen(mib: usize, path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let target = mib * 1024 * 1024;
    let mut w = BufWriter::with_capacity(1 << 20, File::create(path)?);
    let mut r = Rng(0x9E3779B97F4A7C15);
    let mut s = String::new();
    let (mut written, mut lines) = (0usize, 0usize);
    while written < target {
        s.clear();
        gen_line(&mut r, &mut s);
        s.push('\n');
        w.write_all(s.as_bytes())?;
        written += s.len();
        lines += 1;
    }
    w.flush()?;
    println!("wrote {} bytes, {} lines, {:.1} bytes/line", written, lines, written as f64 / lines as f64);
    Ok(())
}

// ---------------------------------------------------------------- Buf trait

trait Buf: Sized {
    fn load(path: &Path) -> io::Result<Self>;
    fn len(&self) -> usize;
    /// number of lines = number of '\n' + 1
    fn lines(&self) -> usize;
    fn byte_at(&self, p: usize) -> u8;
    fn insert(&mut self, pos: usize, s: &str);
    fn delete(&mut self, pos: usize, len: usize);
    fn line_to_byte(&self, l: usize) -> usize;
    fn byte_to_line(&self, b: usize) -> usize;
    /// text of lines [line, line+n) as a &str; borrowed from the buffer when contiguous, else copied into scratch
    fn window<'a>(&'a self, line: usize, n: usize, scratch: &'a mut String) -> &'a str;
    fn copies(&self) -> u64;
    fn save(&self, path: &Path) -> io::Result<()>;
}

// ---------------------------------------------------------------- (a) ropey

struct Ropey {
    r: ropey::Rope,
    copies: Cell<u64>,
}
impl Buf for Ropey {
    fn load(path: &Path) -> io::Result<Self> {
        let f = File::open(path)?;
        let r = ropey::Rope::from_reader(BufReader::with_capacity(1 << 20, f))?;
        Ok(Ropey { r, copies: Cell::new(0) })
    }
    fn len(&self) -> usize {
        self.r.len_bytes()
    }
    fn lines(&self) -> usize {
        self.r.len_lines()
    }
    fn byte_at(&self, p: usize) -> u8 {
        let (chunk, cb, _, _) = self.r.chunk_at_byte(p);
        chunk.as_bytes()[p - cb]
    }
    fn insert(&mut self, pos: usize, s: &str) {
        let c = self.r.byte_to_char(pos);
        self.r.insert(c, s);
    }
    fn delete(&mut self, pos: usize, len: usize) {
        let a = self.r.byte_to_char(pos);
        let b = self.r.byte_to_char(pos + len);
        self.r.remove(a..b);
    }
    fn line_to_byte(&self, l: usize) -> usize {
        self.r.line_to_byte(l.min(self.r.len_lines()))
    }
    fn byte_to_line(&self, b: usize) -> usize {
        self.r.byte_to_line(b)
    }
    fn window<'a>(&'a self, line: usize, n: usize, scratch: &'a mut String) -> &'a str {
        let nl = self.r.len_lines();
        let a = self.r.line_to_char(line.min(nl));
        let z = self.r.line_to_char((line + n).min(nl));
        let s = self.r.slice(a..z);
        if let Some(t) = s.as_str() {
            return t;
        }
        self.copies.set(self.copies.get() + 1);
        scratch.clear();
        for c in s.chunks() {
            scratch.push_str(c);
        }
        scratch.as_str()
    }
    fn copies(&self) -> u64 {
        self.copies.get()
    }
    fn save(&self, path: &Path) -> io::Result<()> {
        let mut w = BufWriter::with_capacity(1 << 20, File::create(path)?);
        self.r.write_to(&mut w)?;
        w.flush()
    }
}

// ---------------------------------------------------------------- (c) String + line starts

struct Plain {
    t: String,
    l: Vec<usize>, // start offset of every line, l[0] = 0
    copies: Cell<u64>,
}
impl Buf for Plain {
    fn load(path: &Path) -> io::Result<Self> {
        let bytes = std::fs::read(path)?;
        let t = String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let mut l = vec![0usize];
        for (i, b) in t.bytes().enumerate() {
            if b == b'\n' {
                l.push(i + 1);
            }
        }
        Ok(Plain { t, l, copies: Cell::new(0) })
    }
    fn len(&self) -> usize {
        self.t.len()
    }
    fn lines(&self) -> usize {
        self.l.len()
    }
    fn byte_at(&self, p: usize) -> u8 {
        self.t.as_bytes()[p]
    }
    fn insert(&mut self, pos: usize, s: &str) {
        self.t.insert_str(pos, s);
        let idx = self.l.partition_point(|&x| x <= pos);
        let d = s.len();
        for x in &mut self.l[idx..] {
            *x += d;
        }
        let new: Vec<usize> = s.bytes().enumerate().filter(|(_, b)| *b == b'\n').map(|(i, _)| pos + i + 1).collect();
        if !new.is_empty() {
            self.l.splice(idx..idx, new);
        }
    }
    fn delete(&mut self, pos: usize, len: usize) {
        self.t.drain(pos..pos + len);
        let lo = self.l.partition_point(|&x| x <= pos);
        let hi = self.l.partition_point(|&x| x <= pos + len);
        self.l.drain(lo..hi);
        for x in &mut self.l[lo..] {
            *x -= len;
        }
    }
    fn line_to_byte(&self, l: usize) -> usize {
        if l < self.l.len() {
            self.l[l]
        } else {
            self.t.len()
        }
    }
    fn byte_to_line(&self, b: usize) -> usize {
        self.l.partition_point(|&x| x <= b) - 1
    }
    fn window<'a>(&'a self, line: usize, n: usize, _scratch: &'a mut String) -> &'a str {
        &self.t[self.line_to_byte(line)..self.line_to_byte(line + n)]
    }
    fn copies(&self) -> u64 {
        self.copies.get()
    }
    fn save(&self, path: &Path) -> io::Result<()> {
        File::create(path)?.write_all(self.t.as_bytes())
    }
}

// ---------------------------------------------------------------- (b) piece table

const MAXP: usize = 128; // pieces per block; blocks split in half beyond this

#[derive(Clone, Copy)]
struct Piece {
    add: bool,
    start: usize,
    len: usize,
    lf: usize, // '\n' count inside the piece
}
struct Block {
    pieces: Vec<Piece>,
    bytes: usize,
    lf: usize,
}
struct Pt {
    orig: Vec<u8>,
    onl: Vec<usize>, // offsets of '\n' in orig
    add: Vec<u8>,
    anl: Vec<usize>, // offsets of '\n' in add
    blocks: Vec<Block>,
    bytes: usize,
    lf: usize,
    copies: Cell<u64>,
}

impl Pt {
    fn nlv(&self, add: bool) -> &Vec<usize> {
        if add {
            &self.anl
        } else {
            &self.onl
        }
    }
    fn data(&self, add: bool) -> &[u8] {
        if add {
            &self.add
        } else {
            &self.orig
        }
    }
    fn count_nl(&self, add: bool, start: usize, end: usize) -> usize {
        let v = self.nlv(add);
        v.partition_point(|&x| x < end) - v.partition_point(|&x| x < start)
    }
    /// (block, piece, offset in piece). strict: pos must lie inside a piece (not at its end).
    fn locate(&self, pos: usize, strict: bool) -> Option<(usize, usize, usize)> {
        let mut acc = 0;
        for (bi, b) in self.blocks.iter().enumerate() {
            let hit = if strict { pos < acc + b.bytes } else { pos <= acc + b.bytes };
            if hit {
                for (pi, p) in b.pieces.iter().enumerate() {
                    let h = if strict { pos < acc + p.len } else { pos <= acc + p.len };
                    if h {
                        return Some((bi, pi, pos - acc));
                    }
                    acc += p.len;
                }
                unreachable!();
            }
            acc += b.bytes;
        }
        None
    }
    fn maybe_split_block(&mut self, bi: usize) {
        if self.blocks[bi].pieces.len() <= MAXP {
            return;
        }
        let half = self.blocks[bi].pieces.len() / 2;
        let tail = self.blocks[bi].pieces.split_off(half);
        let tb: usize = tail.iter().map(|p| p.len).sum();
        let tl: usize = tail.iter().map(|p| p.lf).sum();
        self.blocks[bi].bytes -= tb;
        self.blocks[bi].lf -= tl;
        self.blocks.insert(bi + 1, Block { pieces: tail, bytes: tb, lf: tl });
    }
    fn slice(&self, p: &Piece, from: usize, to: usize) -> &str {
        // invariant: pieces always cut at char boundaries, so this is valid UTF-8
        unsafe { std::str::from_utf8_unchecked(&self.data(p.add)[p.start + from..p.start + to]) }
    }
}

impl Buf for Pt {
    fn load(path: &Path) -> io::Result<Self> {
        let orig = std::fs::read(path)?;
        std::str::from_utf8(&orig).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let onl: Vec<usize> = orig.iter().enumerate().filter(|(_, b)| **b == b'\n').map(|(i, _)| i).collect();
        let n = orig.len();
        let lf = onl.len();
        let blocks = if n == 0 {
            vec![]
        } else {
            vec![Block { pieces: vec![Piece { add: false, start: 0, len: n, lf }], bytes: n, lf }]
        };
        Ok(Pt { orig, onl, add: Vec::new(), anl: Vec::new(), blocks, bytes: n, lf, copies: Cell::new(0) })
    }
    fn len(&self) -> usize {
        self.bytes
    }
    fn lines(&self) -> usize {
        self.lf + 1
    }
    fn byte_at(&self, p: usize) -> u8 {
        let (bi, pi, off) = self.locate(p, true).unwrap();
        let pc = &self.blocks[bi].pieces[pi];
        self.data(pc.add)[pc.start + off]
    }
    fn insert(&mut self, pos: usize, s: &str) {
        if s.is_empty() {
            return;
        }
        let astart = self.add.len();
        let loc = self.locate(pos, false);
        let nlc = s.bytes().filter(|&b| b == b'\n').count();
        self.add.extend_from_slice(s.as_bytes());
        for (i, b) in s.bytes().enumerate() {
            if b == b'\n' {
                self.anl.push(astart + i);
            }
        }
        let np = Piece { add: true, start: astart, len: s.len(), lf: nlc };
        match loc {
            None => {
                assert!(self.blocks.is_empty() && pos == 0);
                self.blocks.push(Block { pieces: vec![np], bytes: np.len, lf: np.lf });
            }
            Some((bi, pi, off)) => {
                let p = self.blocks[bi].pieces[pi];
                if off == p.len && p.add && p.start + p.len == astart {
                    // typing fast path: extend the piece that ends at the end of the add buffer
                    let q = &mut self.blocks[bi].pieces[pi];
                    q.len += np.len;
                    q.lf += np.lf;
                } else if off == p.len {
                    self.blocks[bi].pieces.insert(pi + 1, np);
                } else if off == 0 {
                    self.blocks[bi].pieces.insert(pi, np);
                } else {
                    let llf = self.count_nl(p.add, p.start, p.start + off);
                    let left = Piece { add: p.add, start: p.start, len: off, lf: llf };
                    let right = Piece { add: p.add, start: p.start + off, len: p.len - off, lf: p.lf - llf };
                    let v = &mut self.blocks[bi].pieces;
                    v[pi] = left;
                    v.insert(pi + 1, np);
                    v.insert(pi + 2, right);
                }
                self.blocks[bi].bytes += np.len;
                self.blocks[bi].lf += np.lf;
                self.maybe_split_block(bi);
            }
        }
        self.bytes += np.len;
        self.lf += np.lf;
    }
    fn delete(&mut self, pos: usize, len: usize) {
        let mut rem = len;
        while rem > 0 {
            let (bi, pi, off) = self.locate(pos, true).expect("delete out of range");
            let p = self.blocks[bi].pieces[pi];
            let take = rem.min(p.len - off);
            let rlf = self.count_nl(p.add, p.start + off, p.start + off + take);
            if off == 0 && take == p.len {
                self.blocks[bi].pieces.remove(pi);
            } else if off == 0 {
                let q = &mut self.blocks[bi].pieces[pi];
                q.start += take;
                q.len -= take;
                q.lf -= rlf;
            } else if off + take == p.len {
                let q = &mut self.blocks[bi].pieces[pi];
                q.len -= take;
                q.lf -= rlf;
            } else {
                let llf = self.count_nl(p.add, p.start, p.start + off);
                let left = Piece { add: p.add, start: p.start, len: off, lf: llf };
                let right = Piece {
                    add: p.add,
                    start: p.start + off + take,
                    len: p.len - off - take,
                    lf: p.lf - llf - rlf,
                };
                let v = &mut self.blocks[bi].pieces;
                v[pi] = left;
                v.insert(pi + 1, right);
            }
            self.blocks[bi].bytes -= take;
            self.blocks[bi].lf -= rlf;
            self.bytes -= take;
            self.lf -= rlf;
            if self.blocks[bi].pieces.is_empty() {
                self.blocks.remove(bi);
            } else {
                self.maybe_split_block(bi);
            }
            rem -= take;
        }
    }
    fn line_to_byte(&self, line: usize) -> usize {
        if line == 0 {
            return 0;
        }
        if line > self.lf {
            return self.bytes;
        }
        let (mut ab, mut al) = (0, 0);
        for b in &self.blocks {
            if al + b.lf >= line {
                for p in &b.pieces {
                    if al + p.lf >= line {
                        let k = line - al; // k-th newline inside this piece, 1-based
                        let v = self.nlv(p.add);
                        let first = v.partition_point(|&x| x < p.start);
                        return ab + (v[first + k - 1] + 1 - p.start);
                    }
                    al += p.lf;
                    ab += p.len;
                }
            }
            al += b.lf;
            ab += b.bytes;
        }
        self.bytes
    }
    fn byte_to_line(&self, off: usize) -> usize {
        let off = off.min(self.bytes);
        let (mut ab, mut al) = (0, 0);
        for b in &self.blocks {
            if off < ab + b.bytes {
                for p in &b.pieces {
                    if off < ab + p.len {
                        return al + self.count_nl(p.add, p.start, p.start + (off - ab));
                    }
                    al += p.lf;
                    ab += p.len;
                }
            }
            al += b.lf;
            ab += b.bytes;
        }
        self.lf
    }
    fn window<'a>(&'a self, line: usize, n: usize, scratch: &'a mut String) -> &'a str {
        let a = self.line_to_byte(line);
        let z = self.line_to_byte(line + n);
        if a >= z {
            return "";
        }
        let (bi, pi, off) = self.locate(a, true).unwrap();
        let p = self.blocks[bi].pieces[pi];
        if off + (z - a) <= p.len {
            return self.slice(&p, off, off + (z - a));
        }
        self.copies.set(self.copies.get() + 1);
        scratch.clear();
        let mut need = z - a;
        let (mut bi, mut pi, mut off) = (bi, pi, off);
        while need > 0 {
            let p = self.blocks[bi].pieces[pi];
            let take = need.min(p.len - off);
            scratch.push_str(self.slice(&p, off, off + take));
            need -= take;
            off = 0;
            pi += 1;
            if pi == self.blocks[bi].pieces.len() {
                pi = 0;
                bi += 1;
            }
        }
        scratch.as_str()
    }
    fn copies(&self) -> u64 {
        self.copies.get()
    }
    fn save(&self, path: &Path) -> io::Result<()> {
        let mut w = BufWriter::with_capacity(1 << 20, File::create(path)?);
        for b in &self.blocks {
            for p in &b.pieces {
                w.write_all(&self.data(p.add)[p.start..p.start + p.len])?;
            }
        }
        w.flush()
    }
}

// ---------------------------------------------------------------- the benchmark

struct Out(Vec<(String, f64)>);
impl Out {
    fn put(&mut self, k: &str, v: f64) {
        println!("{} {}", k, v);
        self.0.push((k.to_string(), v));
    }
}

const INS_CHARS: &[&str] = &["x", "ğ", "漢", "a", "ü"];
const TYPE_CHARS: &[&str] = &["a", "e", " ", "t", "n", "ğ", "o", " ", "i", "ü", "r", "s", "漢", " ", "l"];

fn snap<B: Buf>(b: &B, mut p: usize) -> usize {
    while p < b.len() && (b.byte_at(p) & 0xC0) == 0x80 {
        p += 1;
    }
    p
}
fn utf8_len(lead: u8) -> usize {
    match lead {
        0..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// Untimed: choose n distinct char-boundary positions on the current state, in random order, and turn them
/// into positions valid when applied one after another (Fenwick tree of the size deltas of earlier ops).
fn plan<B: Buf>(b: &B, rng: &mut Rng, n: usize, insert: bool) -> Vec<(usize, &'static str, usize)> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut raw: Vec<(usize, &'static str, usize)> = Vec::new(); // orig pos, text, delta magnitude (or del len)
    while raw.len() < n {
        let p = snap(b, rng.below(b.len()));
        if p >= b.len() || !seen.insert(p) {
            continue;
        }
        if insert {
            let s = INS_CHARS[rng.below(INS_CHARS.len())];
            raw.push((p, s, s.len()));
        } else {
            raw.push((p, "", utf8_len(b.byte_at(p))));
        }
    }
    let mut sorted: Vec<usize> = raw.iter().map(|r| r.0).collect();
    sorted.sort_unstable();
    let mut fen = vec![0i64; n + 1];
    let mut out = Vec::with_capacity(n);
    for (p, s, d) in raw {
        let rank = sorted.binary_search(&p).unwrap(); // 0-based
        let mut sum = 0i64;
        let mut i = rank; // prefix over ranks < rank  => indices 1..=rank
        while i > 0 {
            sum += fen[i];
            i -= i & i.wrapping_neg();
        }
        out.push(((p as i64 + sum) as usize, s, d));
        let delta = if insert { d as i64 } else { -(d as i64) };
        let mut i = rank + 1;
        while i <= n {
            fen[i] += delta;
            i += i & i.wrapping_neg();
        }
    }
    out
}

fn lookups<B: Buf>(b: &B, o: &mut Out, suffix: &str) {
    let mut rng = Rng(12345);
    let lines = b.lines();
    let len = b.len();
    let l: Vec<usize> = (0..100_000).map(|_| rng.below(lines)).collect();
    let t = Instant::now();
    let mut cs = 0usize;
    for &x in &l {
        cs = cs.wrapping_add(b.line_to_byte(x));
    }
    o.put(&format!("l2b_ms{suffix}"), t.elapsed().as_secs_f64() * 1e3);
    o.put(&format!("cs_l2b{suffix}"), (cs % 1_000_000_007) as f64);
    let bs: Vec<usize> = (0..100_000).map(|_| rng.below(len)).collect();
    let t = Instant::now();
    let mut cs = 0usize;
    for &x in &bs {
        cs = cs.wrapping_add(b.byte_to_line(x));
    }
    o.put(&format!("b2l_ms{suffix}"), t.elapsed().as_secs_f64() * 1e3);
    o.put(&format!("cs_b2l{suffix}"), (cs % 1_000_000_007) as f64);
    let ws: Vec<usize> = (0..10_000).map(|_| rng.below(lines.saturating_sub(60).max(1))).collect();
    let mut scratch = String::new();
    let c0 = b.copies();
    let t = Instant::now();
    let mut cs = 0usize;
    for &x in &ws {
        let s = b.window(x, 60, &mut scratch);
        cs = cs.wrapping_add(s.len());
    }
    o.put(&format!("win_ms{suffix}"), t.elapsed().as_secs_f64() * 1e3);
    o.put(&format!("cs_win{suffix}"), (cs % 1_000_000_007) as f64);
    o.put(&format!("win_copied{suffix}"), (b.copies() - c0) as f64);
}

fn fnv(path: &Path) -> io::Result<u64> {
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut h: u64 = 0xcbf29ce484222325;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        for &b in &buf[..n] {
            h = (h ^ b as u64).wrapping_mul(0x100000001b3);
        }
    }
    Ok(h)
}

fn run<B: Buf>(path: &Path, ops: usize) -> io::Result<()> {
    let mut o = Out(vec![]);
    let mb = |x: usize| x as f64 / 1048576.0;
    let (m0, _) = mem();
    let t = Instant::now();
    let mut b = B::load(path)?;
    let load = t.elapsed().as_secs_f64() * 1e3;
    let (m1, _) = mem();
    o.put("file_bytes", std::fs::metadata(path)?.len() as f64);
    o.put("lines", b.lines() as f64);
    o.put("load_ms", load);
    o.put("mem_base_mb", mb(m0));
    o.put("mem_load_mb", mb(m1 - m0.min(m1)));
    let mut scratch = String::new();
    let t = Instant::now();
    let w = b.window(0, 60, &mut scratch).len();
    o.put("first_window_us", t.elapsed().as_secs_f64() * 1e6);
    o.put("first_window_len", w as f64);

    lookups(&b, &mut o, "");

    let mut rng = Rng(777);
    o.put("ops", ops as f64);
    let p = plan(&b, &mut rng, ops, true);
    let t = Instant::now();
    for &(pos, s, _) in &p {
        b.insert(pos, s);
    }
    o.put("rand_ins_ms", t.elapsed().as_secs_f64() * 1e3);

    // typing: one advancing position in the middle
    let mut pos = snap(&b, b.len() / 2);
    let t = Instant::now();
    for i in 0..ops {
        let s = TYPE_CHARS[i % TYPE_CHARS.len()];
        b.insert(pos, s);
        pos += s.len();
    }
    o.put("typ_ins_ms", t.elapsed().as_secs_f64() * 1e3);

    let p = plan(&b, &mut rng, ops, false);
    let t = Instant::now();
    for &(pos, _, len) in &p {
        b.delete(pos, len);
    }
    o.put("del_ms", t.elapsed().as_secs_f64() * 1e3);
    o.put("len_after_edits", b.len() as f64);
    o.put("lines_after_edits", b.lines() as f64);
    let (m2, _) = mem();
    o.put("mem_edit_mb", mb(m2 - m0.min(m2)));

    lookups(&b, &mut o, "_ed");

    let out = path.with_extension("out");
    let t = Instant::now();
    b.save(&out)?;
    o.put("save_ms", t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    File::open(&out)?.sync_all().ok();
    o.put("sync_ms", t.elapsed().as_secs_f64() * 1e3);
    let (_, peak) = mem();
    o.put("peak_ws_mb", mb(peak));
    o.put("out_len", std::fs::metadata(&out)?.len() as f64);
    o.put("out_hash", (fnv(&out)? % 1_000_000_000_000) as f64);
    std::fs::remove_file(&out).ok();
    Ok(())
}

// ---------------------------------------------------------------- bench driver

fn median(v: &mut Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

fn bench(runs: usize, impls: &str, path: &str, ops: Option<&str>) {
    let exe = std::env::current_exe().unwrap();
    let impls: Vec<&str> = impls.split(',').collect();
    let mut all: Vec<Vec<Vec<(String, f64)>>> = vec![];
    for im in &impls {
        let mut rs = vec![];
        for _ in 0..runs {
            let mut c = std::process::Command::new(&exe);
            c.arg("run").arg(im).arg(path);
            if let Some(o) = ops {
                c.arg(o);
            }
            let out = c.output().unwrap();
            if !out.status.success() {
                eprintln!("{} failed: {}", im, String::from_utf8_lossy(&out.stderr));
                std::process::exit(1);
            }
            let txt = String::from_utf8_lossy(&out.stdout);
            let kv: Vec<(String, f64)> = txt
                .lines()
                .filter_map(|l| l.split_once(' ').map(|(k, v)| (k.to_string(), v.parse().unwrap())))
                .collect();
            rs.push(kv);
        }
        all.push(rs);
    }
    println!("| metric | {} |", impls.join(" | "));
    println!("|---|{}", "---|".repeat(impls.len()));
    let keys: Vec<String> = all[0][0].iter().map(|(k, _)| k.clone()).collect();
    for (ki, k) in keys.iter().enumerate() {
        let mut cells = vec![];
        let mut identical = true;
        for rs in &all {
            let mut v: Vec<f64> = rs.iter().map(|r| r[ki].1).collect();
            let (mn, mx) = (v.iter().cloned().fold(f64::MAX, f64::min), v.iter().cloned().fold(f64::MIN, f64::max));
            let m = median(&mut v);
            if mn != mx {
                identical = false;
            }
            cells.push(if mn == mx { format!("{:.2}", m) } else { format!("{:.2} ({:.2}-{:.2})", m, mn, mx) });
        }
        let _ = identical;
        println!("| {} | {} |", k, cells.join(" | "));
    }
    // cross-check
    for k in ["cs_l2b", "cs_b2l", "cs_win", "cs_l2b_ed", "cs_b2l_ed", "cs_win_ed", "out_len", "out_hash", "lines_after_edits"] {
        let ki = keys.iter().position(|x| x == k).unwrap();
        let first = all[0][0][ki].1;
        let ok = all.iter().all(|rs| rs.iter().all(|r| r[ki].1 == first));
        println!("check {} {}", k, if ok { "SAME in all impls and runs" } else { "MISMATCH" });
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(|s| s.as_str()) {
        Some("gen") => gen(a[2].parse().unwrap(), Path::new(&a[3])).unwrap(),
        Some("run") => {
            let ops = a.get(4).map(|s| s.parse().unwrap()).unwrap_or(10_000);
            let p = Path::new(&a[3]);
            match a[2].as_str() {
                "ropey" => run::<Ropey>(p, ops).unwrap(),
                "piece" => run::<Pt>(p, ops).unwrap(),
                "string" => run::<Plain>(p, ops).unwrap(),
                x => panic!("unknown impl {x}"),
            }
        }
        Some("bench") => bench(a[2].parse().unwrap(), &a[3], &a[4], a.get(5).map(|s| s.as_str())),
        _ => eprintln!("usage: gen <mib> <path> | run <impl> <path> [ops] | bench <runs> <impls> <path> [ops]"),
    }
}
