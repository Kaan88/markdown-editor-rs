//! Measurements for the phase 1 budgets (release build, Windows):
//!
//! `cargo run --release -p mdedit-core --example bench -- all`
//!   (a) `load_path` of the 100 MiB `lines` file, (b) peak live heap during that load,
//!   (c) `find_all` over it, (d) `chunk_starts` on a 10 MiB single line, (e) 100,000 random
//!   `line_range` and `line_of` calls. Each is the median of 5 runs, with the range.
//!
//! `... bench -- big`
//!   Generates the 100 MiB file under `target/data/`, loads it once and prints the load time and
//!   the process's private bytes before and after, for the plan's memory budget. Run it in a
//!   fresh process (the default), so nothing else is allocated.
//!
//! Unsafe is used here only: a counting allocator and `K32GetProcessMemoryInfo`.

mod common;

use mdedit_core::buffer::Buffer;
use mdedit_core::display::chunk_starts;
use mdedit_core::find::{FindOptions, Query, find_all};
use mdedit_core::limits::{FIND_CAP, TAB_WIDTH};
use mdedit_core::load::{LoadOptions, Loaded, load_path};
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

// ---------------------------------------------------------------------------------------------
// A counting allocator: live bytes and the peak since the last reset.

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let now = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
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
            if new_size >= layout.size() {
                let now = LIVE.fetch_add(new_size - layout.size(), Ordering::Relaxed)
                    + (new_size - layout.size());
                PEAK.fetch_max(now, Ordering::Relaxed);
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        p
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn live() -> usize {
    LIVE.load(Ordering::Relaxed)
}

fn reset_peak() {
    PEAK.store(live(), Ordering::Relaxed);
}

fn peak() -> usize {
    PEAK.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------------------------
// Process memory, as the spikes measured it.

#[cfg(windows)]
#[repr(C)]
struct ProcessMemoryCountersEx {
    cb: u32,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
    private_usage: usize,
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> isize;
    fn K32GetProcessMemoryInfo(
        process: isize,
        counters: *mut ProcessMemoryCountersEx,
        cb: u32,
    ) -> i32;
}

/// (private bytes, peak working set, peak commit).
#[cfg(windows)]
fn process_memory() -> (usize, usize, usize) {
    let mut c = ProcessMemoryCountersEx {
        cb: std::mem::size_of::<ProcessMemoryCountersEx>() as u32,
        page_fault_count: 0,
        peak_working_set_size: 0,
        working_set_size: 0,
        quota_peak_paged_pool_usage: 0,
        quota_paged_pool_usage: 0,
        quota_peak_non_paged_pool_usage: 0,
        quota_non_paged_pool_usage: 0,
        pagefile_usage: 0,
        peak_pagefile_usage: 0,
        private_usage: 0,
    };
    unsafe {
        K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb);
    }
    (
        c.private_usage,
        c.peak_working_set_size,
        c.peak_pagefile_usage,
    )
}

/// The same three figures from /proc: resident set, its peak, and the peak virtual size.
#[cfg(not(windows))]
fn process_memory() -> (usize, usize, usize) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kib = |key: &str| {
        status
            .lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<usize>().ok())
            .unwrap_or(0)
            * 1024
    };
    (kib("VmRSS:"), kib("VmHWM:"), kib("VmPeak:"))
}

const MIB: f64 = 1024.0 * 1024.0;

// ---------------------------------------------------------------------------------------------

fn data_dir() -> PathBuf {
    // target/data next to the workspace's target directory: this file is at
    // crates/mdedit-core/examples/bench.rs.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("data")
}

fn ensure(path: &Path, make: impl FnOnce() -> std::io::Result<()>) {
    if !path.exists() {
        make().expect("generating the test file");
    }
}

fn load(path: &Path) -> Loaded {
    let cancel = AtomicBool::new(false);
    load_path(path, &LoadOptions::default(), &|_, _| {}, &cancel).expect("load")
}

/// Median, min and max of `values`.
fn summary(values: &mut [f64]) -> (f64, f64, f64) {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (
        values[values.len() / 2],
        values[0],
        values[values.len() - 1],
    )
}

fn report(name: &str, unit: &str, values: &mut [f64]) {
    let (med, lo, hi) = summary(values);
    println!(
        "{name}: median {med:.2} {unit} (min {lo:.2}, max {hi:.2}; {} runs)",
        values.len()
    );
}

fn time_ms(f: impl FnOnce()) -> f64 {
    let t = Instant::now();
    f();
    t.elapsed().as_secs_f64() * 1000.0
}

fn all() {
    let dir = data_dir();
    let lines = dir.join("lines100.txt");
    let long = dir.join("long10.txt");
    ensure(&lines, || common::gen_lines(100, &lines, "\n"));
    ensure(&long, || common::gen_long(10, &long));
    let file_bytes = std::fs::metadata(&lines).unwrap().len() as f64;

    // (a) and (b): load the 100 MiB file five times. A first load warms the file cache.
    drop(load(&lines));
    let mut ms = Vec::new();
    let mut peaks = Vec::new();
    let mut retained = Vec::new();
    let mut lines_count = 0;
    for _ in 0..5 {
        let base = live();
        reset_peak();
        let l = load(&lines);
        peaks.push((peak() - base) as f64 / file_bytes);
        retained.push((live() - base) as f64 / file_bytes);
        ms.push(l.info.elapsed.as_secs_f64() * 1000.0);
        lines_count = l.buffer.len_lines();
    }
    println!("file: {:.1} MiB, {} lines", file_bytes / MIB, lines_count);
    report("(a) load_path 100 MiB", "ms", &mut ms);
    report(
        "(b) peak live heap during load / file size",
        "x",
        &mut peaks,
    );
    report("(b) live heap after load / file size", "x", &mut retained);

    // (c) find_all over the loaded file.
    let loaded = load(&lines);
    let rope = loaded.buffer.snapshot();
    let cancel = AtomicBool::new(false);
    let queries: &[(&str, &str, bool)] = &[
        ("ASCII, no match", "qzxv_not_in_file", false),
        ("ASCII, first letter common", "elephant_not_here", false),
        ("ASCII, common phrase", "is computed lazily", false),
        ("ASCII, match case", "Merhaba dünya", true),
        ("Turkish, rare start", "çalışma durduruldu", false),
        ("Turkish, dotted capital", "İstanbul", false),
        ("Turkish, no match", "ğüşıöç yok", false),
    ];
    for (name, text, match_case) in queries {
        let query = Query::new(
            text,
            FindOptions {
                match_case: *match_case,
                ..FindOptions::default()
            },
        )
        .unwrap();
        let mut mbs = Vec::new();
        let mut found = 0;
        let mut capped = false;
        for _ in 0..5 {
            let t = Instant::now();
            let m = find_all(&rope, &query, usize::MAX, &cancel);
            let secs = t.elapsed().as_secs_f64();
            found = m.ranges.len();
            capped = m.capped;
            mbs.push(file_bytes / MIB / secs);
        }
        report(
            &format!("(c) find_all {name} ({text:?}): {found} matches, capped {capped}"),
            "MiB/s",
            &mut mbs,
        );
    }
    // The shell's own call: capped at FIND_CAP.
    let query = Query::new("the ", FindOptions::default()).unwrap();
    let mut ms = Vec::new();
    for _ in 0..5 {
        ms.push(time_ms(|| {
            let m = find_all(&rope, &query, FIND_CAP, &cancel);
            assert!(m.capped);
        }));
    }
    report("(c) find_all \"the \", cap 10,000", "ms", &mut ms);

    // (d) chunk_starts on a 10 MiB single line.
    let l = load(&long);
    let mut ms = Vec::new();
    let mut n = 0;
    for _ in 0..5 {
        ms.push(time_ms(|| n = chunk_starts(&l.buffer, 0, TAB_WIDTH).len()));
    }
    println!(
        "(d) one line of {:.1} MiB, longest_line_chars {}, {} chunks",
        l.buffer.len_bytes() as f64 / MIB,
        l.info.longest_line_chars,
        n
    );
    report("(d) chunk_starts, 10 MiB line", "ms", &mut ms);

    // (e) 100,000 random line_range and line_of calls on the 100 MiB buffer.
    let buf: &Buffer = &loaded.buffer;
    let mut rng = common::Rng(12345);
    let line_ids: Vec<usize> = (0..100_000).map(|_| rng.below(buf.len_lines())).collect();
    let offsets: Vec<usize> = (0..100_000).map(|_| rng.below(buf.len_bytes())).collect();
    let mut ms_range = Vec::new();
    let mut ms_of = Vec::new();
    let mut sink = 0usize;
    for _ in 0..5 {
        ms_range.push(time_ms(|| {
            for &i in &line_ids {
                let r = buf.line_range(i);
                sink = sink.wrapping_add(r.end - r.start);
            }
        }));
        ms_of.push(time_ms(|| {
            for &o in &offsets {
                sink = sink.wrapping_add(buf.line_of(o));
            }
        }));
    }
    report("(e) 100,000 random line_range", "ms", &mut ms_range);
    report("(e) 100,000 random line_of", "ms", &mut ms_of);
    println!("(sink {sink})");
}

fn big() {
    let dir = data_dir();
    let path = dir.join("big100.txt");
    ensure(&path, || common::gen_lines(100, &path, "\n"));
    let size = std::fs::metadata(&path).unwrap().len();
    let (private0, _, _) = process_memory();
    let base = live();
    reset_peak();
    let l = load(&path);
    let (private1, peak_ws, peak_commit) = process_memory();
    println!("file {} bytes ({:.1} MiB)", size, size as f64 / MIB);
    println!(
        "load_path {:.1} ms, {} lines, encoding {}, eol {}",
        l.info.elapsed.as_secs_f64() * 1000.0,
        l.buffer.len_lines(),
        l.info.encoding.label(),
        l.info.eol.style().label()
    );
    println!(
        "private bytes: before {:.1} MiB, after {:.1} MiB, difference {:.1} MiB ({:.2}x the file); budget 125 MiB over the small file",
        private0 as f64 / MIB,
        private1 as f64 / MIB,
        (private1 - private0) as f64 / MIB,
        (private1 - private0) as f64 / size as f64
    );
    println!(
        "live heap after load {:.1} MiB, peak live heap during load {:.1} MiB ({:.2}x the file)",
        (live() - base) as f64 / MIB,
        (peak() - base) as f64 / MIB,
        (peak() - base) as f64 / size as f64
    );
    println!(
        "peak working set {:.1} MiB, peak commit {:.1} MiB",
        peak_ws as f64 / MIB,
        peak_commit as f64 / MIB
    );
    std::hint::black_box(&l);
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("all") => all(),
        Some("big") => big(),
        _ => {
            eprintln!("usage: bench all|big");
            std::process::exit(2);
        }
    }
}
