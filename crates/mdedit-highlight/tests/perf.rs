//! Timings and memory of the line highlighter on a generated 50,000-line Rust text. Ignored by
//! default; run with
//! `cargo test --release -p mdedit-highlight --test perf -- --ignored --nocapture --test-threads=1`.
//! The numbers depend on the machine and its load; the spike's are in
//! `spikes/highlight/RESULTS.md`.

use mdedit_highlight::{HighlightService, Language, RopeLines};
use mdshared_highlight::incremental::{LineHighlighter, Lines, checkpoint_spacing};
use ropey::Rope;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Counts live heap bytes.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            LIVE.fetch_add(new_size, Ordering::Relaxed);
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        p
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn generated_rust(lines: usize) -> String {
    let mut s = String::new();
    let mut count = 0;
    let mut n = 0;
    while count < lines {
        let piece = match n % 8 {
            0 => format!("/// Doc for item {n}\n"),
            1 => format!("pub fn item_{n}(a: u32, b: &str) -> u32 {{\n"),
            2 => format!("    let text = \"value {n}\"; // note\n"),
            3 => "    /* a\n       block */\n".to_string(),
            4 => format!("    if a > {n} {{ return a * 2 + b.len() as u32; }}\n"),
            5 => "    let raw = r#\"x \"y\" z\"#;\n    a\n}\n".to_string(),
            6 => format!("struct S{n} {{ field: Vec<u8>, other: Option<String> }}\n\n"),
            _ => format!(
                "impl S{} {{ fn get(&self) -> usize {{ self.field.len() + {n} }} }}\n",
                n - 1
            ),
        };
        count += piece.matches('\n').count();
        s.push_str(&piece);
        n += 1;
    }
    s
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn rust() -> Language {
    Language::by_token("rust").unwrap()
}

#[test]
#[ignore]
fn viewport_after_a_full_parse() {
    let rope = Rope::from_str(&generated_rust(50_000));
    let src = RopeLines(&rope);
    let total = src.len_lines();
    println!("text: {} lines, {} bytes", total, rope.len_bytes());
    let mut h = LineHighlighter::new(rust(), false, total, rope.len_bytes());
    let far = Instant::now() + Duration::from_secs(600);
    // Warm the syntax (lazy regex compilation), then measure.
    h.spans(&src, 0..60, far).unwrap();
    let mut slices = Vec::new();
    let started = Instant::now();
    loop {
        let t = Instant::now();
        let more = h.advance(&src, t + Duration::from_millis(2));
        slices.push(ms(t.elapsed()));
        if !more {
            break;
        }
    }
    let full = ms(started.elapsed());
    let mut sorted = slices.clone();
    let max = sorted.iter().cloned().fold(0.0, f64::max);
    println!(
        "full advance: {full:.0} ms in {} slices of 2 ms; slice median {:.2} ms, max {max:.2} ms",
        slices.len(),
        median(&mut sorted)
    );
    let mut times = Vec::new();
    for _ in 0..31 {
        let t = Instant::now();
        let spans = h.spans(&src, 40_000..40_060, far).unwrap();
        times.push(ms(t.elapsed()));
        assert_eq!(spans.len(), 60);
    }
    println!(
        "viewport 40000..40060 after advance: median {:.2} ms (min {:.2}, max {:.2}), K = {}",
        median(&mut times.clone()),
        times.iter().cloned().fold(f64::MAX, f64::min),
        times.iter().cloned().fold(0.0, f64::max),
        h.spacing()
    );
}

#[test]
#[ignore]
fn first_visible_lines_and_checkpoint_memory() {
    let rope = Rope::from_str(&generated_rust(50_000));
    let total = RopeLines(&rope).len_lines();
    let far = Instant::now() + Duration::from_secs(600);

    // Through the service, in this process: the first request pays syntect's lazy compilation.
    let service = HighlightService::spawn(|| {});
    let t = Instant::now();
    service.open(1, 0, rope.clone(), rust(), false);
    service.view(1, 0, 0..60);
    let got = service.wait(far);
    println!(
        "first 60 lines through the service (open to Colored, cold syntax): {:.1} ms ({} lines)",
        ms(t.elapsed()),
        got[0].lines.len()
    );
    drop(service);

    // Stored states: live bytes before and after a full parse, with the syntax already warm.
    let src = RopeLines(&rope);
    let mut warm = LineHighlighter::new(rust(), false, total, rope.len_bytes());
    warm.spans(&src, 0..60, far).unwrap();
    drop(warm);
    for (lines, name) in [(50_000, "50,000 lines"), (6_000, "6,000 lines")] {
        let text = Rope::from_str(&generated_rust(lines));
        let src = RopeLines(&text);
        let total = src.len_lines();
        let mut h = LineHighlighter::new(rust(), false, total, text.len_bytes());
        h.spans(&src, 0..60, far).unwrap();
        let before = LIVE.load(Ordering::Relaxed);
        while h.advance(&src, far) {}
        let after = LIVE.load(Ordering::Relaxed);
        let held = after.saturating_sub(before);
        let entries = h.checkpoint_count();
        println!(
            "{name}: K = {}, {} checkpoints, {:.2} MiB held ({} bytes each); estimate entries x 1 KB = {:.2} MiB",
            h.spacing(),
            entries,
            held as f64 / (1 << 20) as f64,
            held / entries.max(1),
            (entries * 1024) as f64 / (1 << 20) as f64
        );
        assert_eq!(h.spacing(), checkpoint_spacing(total));
    }
}
