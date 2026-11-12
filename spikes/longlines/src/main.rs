//! Spike "longlines": viewport-only shaping numbers. Throwaway.
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, Layout, LayoutContext, LineHeight,
    OverflowWrap, PositionedLayoutItem, StyleProperty,
};
use std::collections::HashMap;
use std::time::Instant;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Format, Vector};
use swash::FontRef;
use unicode_segmentation::GraphemeCursor;

type Brush = [u8; 4];
const FAMILY: &str = "Consolas";
const SIZE: f32 = 14.0;
const WRAP: f32 = 1200.0;

// ---------- memory ----------
fn mem() -> (usize, usize) {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let mut c: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c as *mut _ as *mut _, c.cb);
        (c.PrivateUsage, c.PeakWorkingSetSize)
    }
}

// ---------- data ----------
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Minified-JS-like: few spaces (after keywords only), lots of punctuation.
fn minified(bytes: usize) -> String {
    const T: &[&str] = &[
        "function(", "return ", "var ", "e", "t", "n", "r", "o", "i", "(", ")", "{", "}", ";", ",",
        ".", "=", "+", "[", "]", "\"str\"", "0", "1", "if(", "else ", "for(", "!", "&&", "||",
        "a.b", "c.d()", "this.x", "typeof ", "new ", "void 0", "===", "=>", "e.length", "-1",
    ];
    let mut r = Rng(0x9E3779B97F4A7C15);
    let mut s = String::with_capacity(bytes + 16);
    while s.len() < bytes {
        s.push_str(T[(r.next() % T.len() as u64) as usize]);
    }
    s.truncate(bytes);
    s
}

/// Source-code-like lines of about `cols` columns.
fn code_line(i: usize, cols: usize) -> String {
    const W: &[&str] = &[
        "let", "result", "items", ".filter(|item|", "item.enabled", "&&", "count", ">", "0)",
        ".map(|x|", "x.id", ")", ".collect::<Vec<_>>();", "if", "self.buffer.len()", "==", "{",
        "return", "Some(value)", "fn", "render_line(&mut self,", "line:", "usize)", "->", "Layout",
    ];
    let mut r = Rng(0x1234_5678 + i as u64 * 7919);
    let mut s = " ".repeat(4 * (r.next() % 3) as usize);
    while s.len() < cols {
        s.push_str(W[(r.next() % W.len() as u64) as usize]);
        s.push(' ');
    }
    s.truncate(cols);
    s
}

// ---------- shaping ----------
struct Shaper {
    fonts: FontContext,
    ctx: LayoutContext<Brush>,
}
#[derive(Clone, Copy, PartialEq)]
enum Wrap {
    No,
    Word,     // wrap at 1200 px, default break opportunities
    Anywhere, // wrap at 1200 px, OverflowWrap::Anywhere
}
impl Shaper {
    fn new() -> Self {
        Shaper { fonts: FontContext::new(), ctx: LayoutContext::new() }
    }
    /// `spans`: colour a span every `n` bytes (highlighter-like), 0 for none.
    fn shape(&mut self, text: &str, wrap: Wrap, spans: usize) -> Layout<Brush> {
        let mut b = self.ctx.ranged_builder(&mut self.fonts, text, 1.0, true);
        b.push_default(StyleProperty::FontFamily(FontFamily::from(FAMILY)));
        b.push_default(StyleProperty::FontSize(SIZE));
        b.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(1.3)));
        b.push_default(StyleProperty::Brush([30, 30, 30, 255]));
        if wrap == Wrap::Anywhere {
            b.push_default(StyleProperty::OverflowWrap(OverflowWrap::Anywhere));
        }
        if spans > 0 {
            let mut i = 0usize;
            let mut k = 0u8;
            while i < text.len() {
                let mut j = (i + spans).min(text.len());
                while !text.is_char_boundary(j) {
                    j += 1;
                }
                if k % 2 == 1 {
                    b.push(StyleProperty::Brush([200, 40, 40 + k, 255]), i..j);
                }
                k = k.wrapping_add(1);
                i = j;
            }
        }
        let mut l = b.build(text);
        match wrap {
            Wrap::No => l.break_all_lines(None),
            _ => l.break_all_lines(Some(WRAP)),
        }
        l.align(Alignment::Start, AlignmentOptions::default());
        l
    }
}

// ---------- stats ----------
fn stats(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[0], v[v.len() - 1])
}
/// per-iteration ms, `runs` outer repeats of `iters` inner iterations
fn bench<F: FnMut()>(runs: usize, iters: usize, mut f: F) -> (f64, f64, f64) {
    f(); // warm
    let mut v = vec![];
    for _ in 0..runs {
        let t = Instant::now();
        for _ in 0..iters {
            f();
        }
        v.push(t.elapsed().as_secs_f64() * 1000.0 / iters as f64);
    }
    stats(v)
}
fn fmt(s: (f64, f64, f64)) -> String {
    format!("{:.4} ({:.4}-{:.4})", s.0, s.1, s.2)
}
fn iters_for(bytes: usize) -> usize {
    match bytes {
        0..=200 => 200,
        201..=5_000 => 30,
        5_001..=60_000 => 5,
        _ => 1,
    }
}

// ---------- chunking ----------
/// Nearest safe split at or after `target`: ASCII whitespace within 512 bytes, else a grapheme
/// boundary.
fn safe_split(text: &str, target: usize) -> usize {
    if target >= text.len() {
        return text.len();
    }
    let b = text.as_bytes();
    let end = (target + 512).min(b.len());
    for i in target..end {
        if b[i] == b' ' || b[i] == b'\t' {
            return i + 1;
        }
    }
    let mut t = target;
    while !text.is_char_boundary(t) {
        t += 1;
    }
    let mut c = GraphemeCursor::new(t, text.len(), true);
    while t < text.len() && !c.is_boundary(text, 0).unwrap_or(true) {
        t += 1;
        while !text.is_char_boundary(t) {
            t += 1;
        }
        c.set_cursor(t);
    }
    t
}
/// The chunk [s,e) of size ~n containing byte `pos` (boundary search from an estimated start;
/// a real editor keeps a chunk index, this models the cost of one boundary search).
fn chunk_around(text: &str, pos: usize, n: usize) -> (usize, usize) {
    let est = pos / n * n;
    let s = if est == 0 { 0 } else { safe_split(text, est) };
    let s = if s > pos { safe_split(text, est.saturating_sub(n)) } else { s };
    let e = safe_split(text, s + n);
    (s, e)
}

// ---------- rasteriser ----------
struct Entry {
    left: i32,
    top: i32,
    w: u32,
    h: u32,
    data: Box<[u8]>,
}
struct Raster {
    scaler: ScaleContext,
    map: HashMap<(u64, u32, u16, u8), Entry>,
    fb: Vec<u8>,
    fbw: usize,
    hits: u64,
    misses: u64,
}
impl Raster {
    fn new(w: usize, h: usize) -> Self {
        Raster { scaler: ScaleContext::new(), map: HashMap::new(), fb: vec![255; w * h * 4], fbw: w, hits: 0, misses: 0 }
    }
    fn draw(&mut self, layout: &Layout<Brush>, oy: f32) {
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(gr) = item else { continue };
                let run = gr.run();
                let font = run.font();
                let size = run.font_size();
                let coords = run.normalized_coords();
                let brush = gr.style().brush;
                for g in gr.positioned_glyphs() {
                    let (x, y) = (4.0 + g.x, (oy + g.y).round());
                    let fx = x.floor();
                    let sub = (((x - fx) * 4.0) as u8).min(3);
                    let key = (font.data.id(), font.index, g.id as u16, sub);
                    if !self.map.contains_key(&key) {
                        self.misses += 1;
                        let r = FontRef::from_index(font.data.as_ref(), font.index as usize).unwrap();
                        let mut sc = self
                            .scaler
                            .builder(r)
                            .size(size)
                            .hint(false)
                            .normalized_coords(coords.iter().map(|c| c.to_bits()))
                            .build();
                        let img = Render::new(&[
                            Source::ColorOutline(0),
                            Source::ColorBitmap(StrikeWith::BestFit),
                            Source::Outline,
                        ])
                        .format(Format::CustomSubpixel([0.3, 0.0, -0.3]))
                        .offset(Vector::new(sub as f32 / 4.0, 0.0))
                        .render(&mut sc, g.id as u16);
                        let e = match img {
                            Some(i) => {
                                let mut d = Vec::with_capacity((i.placement.width * i.placement.height * 3) as usize);
                                for p in i.data.chunks_exact(4) {
                                    d.extend_from_slice(&p[..3]);
                                }
                                Entry { left: i.placement.left, top: i.placement.top, w: i.placement.width, h: i.placement.height, data: d.into_boxed_slice() }
                            }
                            None => Entry { left: 0, top: 0, w: 0, h: 0, data: Box::new([]) },
                        };
                        self.map.insert(key, e);
                    } else {
                        self.hits += 1;
                    }
                    let e = &self.map[&key];
                    let (x0, y0) = (fx as i32 + e.left, y as i32 - e.top);
                    for row in 0..e.h as i32 {
                        let ry = y0 + row;
                        if ry < 0 {
                            continue;
                        }
                        for col in 0..e.w as i32 {
                            let cx = x0 + col;
                            if cx < 0 || cx as usize >= self.fbw {
                                continue;
                            }
                            let s = &e.data[((row as u32 * e.w + col as u32) * 3) as usize..][..3];
                            let d0 = (ry as usize * self.fbw + cx as usize) * 4;
                            if d0 + 3 >= self.fb.len() {
                                continue;
                            }
                            let d = &mut self.fb[d0..d0 + 4];
                            d[0] = ((brush[2] as u32 * s[2] as u32 + d[0] as u32 * (255 - s[2] as u32) + 127) / 255) as u8;
                            d[1] = ((brush[1] as u32 * s[1] as u32 + d[1] as u32 * (255 - s[1] as u32) + 127) / 255) as u8;
                            d[2] = ((brush[0] as u32 * s[0] as u32 + d[2] as u32 * (255 - s[0] as u32) + 127) / 255) as u8;
                        }
                    }
                }
            }
        }
    }
}

fn mb(b: usize) -> f64 {
    b as f64 / 1048576.0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("all");
    match mode {
        "mem" => mem_mode(&args),
        "check" => check(),
        "sweep" => sweep(),
        "strategies" => strategies(),
        "raster" => raster(),
        "sizes" => sizes(),
        _ => {
            check();
            sizes();
            sweep();
            strategies();
            raster();
        }
    }
}

fn check() {
    let mut sh = Shaper::new();
    let l = sh.shape("0123456789", Wrap::No, 0);
    for line in l.lines() {
        for it in line.items() {
            if let PositionedLayoutItem::GlyphRun(gr) = it {
                println!("font data bytes={} index={}", gr.run().font().data.as_ref().len(), gr.run().font().index);
                let gs: Vec<_> = gr.positioned_glyphs().collect();
                println!("glyph advance px at 14px = {:.3} (Consolas expect 7.70)", gs[1].x - gs[0].x);
            }
        }
    }
    println!("line height {:.2}", l.height());
}

/// shaping 80-col, 60-line viewport, and fixed sizes with/without wrap
fn sizes() {
    let mut sh = Shaper::new();
    println!("\n## typical line and viewport (ms per op: median (min-max) of 7 runs)");
    let line = code_line(1, 80);
    let n = 7;
    println!("80-col line, plain, no wrap : {}", fmt(bench(n, 200, || { std::hint::black_box(sh.shape(&line, Wrap::No, 0)); })));
    println!("80-col line, 10 colour spans: {}", fmt(bench(n, 200, || { std::hint::black_box(sh.shape(&line, Wrap::No, 8)); })));
    let lines: Vec<String> = (0..60).map(|i| code_line(i, 80)).collect();
    println!("60 lines x 80 col, one layout per line: {}", fmt(bench(n, 5, || { for l in &lines { std::hint::black_box(sh.shape(l, Wrap::No, 0)); } })));
    println!("60 lines x 80 col, per line + spans   : {}", fmt(bench(n, 5, || { for l in &lines { std::hint::black_box(sh.shape(l, Wrap::No, 8)); } })));
    let lines: Vec<String> = (0..60).map(|i| code_line(i, 120)).collect();
    println!("60 lines x 120 col, one layout per line: {}", fmt(bench(n, 5, || { for l in &lines { std::hint::black_box(sh.shape(l, Wrap::No, 0)); } })));

    println!("\n## one minified line, whole-line shape (ms)");
    println!("| size | no wrap | wrap 1200 (UAX14) | wrap 1200 (anywhere) |");
    println!("|---|---|---|---|");
    for (label, bytes) in [("1 KB", 1 << 10), ("10 KB", 10_000), ("100 KB", 100_000), ("1 MB", 1_000_000), ("10 MB", 10_000_000)] {
        let t = minified(bytes);
        let runs = if bytes >= 1_000_000 { 5 } else { 7 };
        let it = iters_for(bytes);
        let a = bench(runs, it, || { std::hint::black_box(sh.shape(&t, Wrap::No, 0)); });
        let b = bench(runs, it, || { std::hint::black_box(sh.shape(&t, Wrap::Word, 0)); });
        let c = bench(runs, it, || { std::hint::black_box(sh.shape(&t, Wrap::Anywhere, 0)); });
        println!("| {label} | {} | {} | {} |", fmt(a), fmt(b), fmt(c));
    }
}

/// (private bytes delta with layout held, delta after drop, peak working set) in a fresh process
fn mem_mode(args: &[String]) {
    let bytes: usize = args[2].parse().unwrap();
    let wrap = match args[3].as_str() { "no" => Wrap::No, "word" => Wrap::Word, _ => Wrap::Anywhere };
    let mut sh = Shaper::new();
    let warm = minified(2000);
    drop(sh.shape(&warm, Wrap::No, 0));
    let t = minified(bytes);
    let (base, _) = mem();
    let t0 = Instant::now();
    let l = sh.shape(&t, wrap, 0);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    let (held, peak) = mem();
    let lines = l.len();
    drop(l);
    let (after, _) = mem();
    println!(
        "MEM bytes={bytes} wrap={} text={:.1}MB held_delta={:.1}MB after_drop_delta={:.1}MB peak_ws={:.1}MB visual_lines={lines} shape_ms={ms:.1} held_per_input_byte={:.1}",
        args[3], mb(t.len()), mb(held.saturating_sub(base)), mb(after.saturating_sub(base)), mb(peak), (held.saturating_sub(base)) as f64 / bytes as f64
    );
}

/// whole-line reshape cost vs length: find the 2 ms and 8 ms crossings
fn sweep() {
    let mut sh = Shaper::new();
    println!("\n## single-line reshape cost vs length (minified JS-like, ms, median of 9 runs)");
    println!("| bytes | no wrap | wrap UAX14 | wrap anywhere | no wrap + spans/8B |");
    println!("|---|---|---|---|---|");
    let sizes: Vec<usize> = [100usize, 300, 1000, 2000, 4000, 8000, 12000, 16000, 24000, 32000, 48000, 64000, 100_000, 200_000, 400_000, 1_000_000].to_vec();
    for &b in &sizes {
        let t = minified(b);
        let it = iters_for(b);
        let a = bench(9, it, || { std::hint::black_box(sh.shape(&t, Wrap::No, 0)); });
        let w = bench(9, it, || { std::hint::black_box(sh.shape(&t, Wrap::Word, 0)); });
        let y = bench(9, it, || { std::hint::black_box(sh.shape(&t, Wrap::Anywhere, 0)); });
        let sp = if b <= 200_000 { fmt(bench(9, it, || { std::hint::black_box(sh.shape(&t, Wrap::No, 8)); })) } else { "-".into() };
        println!("| {b} | {} | {} | {} | {sp} |", fmt(a), fmt(w), fmt(y));
    }
}

fn strategies() {
    let mut sh = Shaper::new();
    println!("\n## chunked shaping of an on-screen window (ms per frame/edit, median (min-max) of 9 runs)");
    println!("Window = chunks covering the bytes on screen. No wrap: ~160 columns so 1-2 chunks. Wrap 1200 px (anywhere): 60 visual rows x ~155 cols = ~9.3 KB.");
    println!("Cost includes finding the chunk boundaries (safe_split) in the long string.");
    println!("| line | chunk N | no wrap, view at start | no wrap, view at middle | wrap, 60 rows at middle |");
    println!("|---|---|---|---|---|");
    for (label, total) in [("1 MB", 1_000_000usize), ("10 MB", 10_000_000)] {
        let text = minified(total);
        for n in [256usize, 1024, 4096, 16384, 65536] {
            let mut cols = vec![];
            for (pos, wrap, win) in [(0usize, Wrap::No, 160usize), (total / 2, Wrap::No, 160), (total / 2, Wrap::Anywhere, 9300)] {
                let mut f = || {
                    let (s, mut e) = chunk_around(&text, pos, n);
                    let mut layouts = vec![];
                    let mut s0 = s;
                    loop {
                        layouts.push(sh.shape(&text[s0..e], wrap, 0));
                        if e >= pos + win || e >= text.len() {
                            break;
                        }
                        s0 = e;
                        e = safe_split(&text, e + n);
                    }
                    std::hint::black_box(layouts);
                };
                cols.push(fmt(bench(9, 5, &mut f)));
            }
            println!("| {label} | {n} | {} | {} | {} |", cols[0], cols[1], cols[2]);
        }
    }
    println!("\nchunk-index build cost (walk the whole line once, split every N=4096 at a safe boundary, no shaping), ms:");
    for (label, total) in [("1 MB", 1_000_000usize), ("10 MB", 10_000_000)] {
        let text = minified(total);
        let r = bench(9, 1, || {
            let mut i = 0;
            let mut c = 0;
            while i < text.len() {
                i = safe_split(&text, i + 4096);
                c += 1;
            }
            std::hint::black_box(c);
        });
        println!("  {label}: {}", fmt(r));
    }
    println!("\nhard cap: shape only the first C bytes, then show an ellipsis marker (cost = whole-line shape of C bytes, no wrap):");
    for c in [4096usize, 8192, 16384, 32768, 65536, 131072] {
        let t = minified(c);
        println!("  C={c}: {}", fmt(bench(9, iters_for(c), || { std::hint::black_box(sh.shape(&t, Wrap::No, 0)); })));
    }
}

fn raster() {
    println!("\n## rasterise a 60 x 120 viewport (swash, subpixel RGB masks, 14 px Consolas, 7200 glyphs)");
    let mut sh = Shaper::new();
    let lines: Vec<String> = (0..60).map(|i| code_line(i + 100, 120)).collect();
    let layouts: Vec<Layout<Brush>> = lines.iter().map(|l| sh.shape(l, Wrap::No, 0)).collect();
    let lh = layouts[0].height();
    let (w, h) = (1000usize, (lh * 60.0) as usize + 20);
    let mut cold = vec![];
    let mut uniq = 0;
    let mut cold_misses = 0;
    for _ in 0..9 {
        let mut r = Raster::new(w, h);
        let t = Instant::now();
        for (i, l) in layouts.iter().enumerate() {
            r.draw(l, i as f32 * lh);
        }
        cold.push(t.elapsed().as_secs_f64() * 1000.0);
        uniq = r.map.len();
        cold_misses = r.misses;
    }
    let mut r = Raster::new(w, h);
    for (i, l) in layouts.iter().enumerate() {
        r.draw(l, i as f32 * lh);
    }
    let bytes: usize = r.map.values().map(|e| e.data.len()).sum();
    let mut warm = vec![];
    for _ in 0..9 {
        let t = Instant::now();
        for _ in 0..10 {
            for (i, l) in layouts.iter().enumerate() {
                r.draw(l, i as f32 * lh);
            }
        }
        warm.push(t.elapsed().as_secs_f64() * 100.0);
    }
    println!("cold (empty cache, new ScaleContext): {} ms, {uniq} unique (glyph,subpixel) rasters ({cold_misses} misses), cache {:.1} KB", fmt(stats(cold)), bytes as f64 / 1024.0);
    println!("warm (all cached, lookup + blit into 1000x{h} buffer): {} ms", fmt(stats(warm)));
}
