use spike_highlight::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use syntect::highlighting::Highlighter;
use syntect::parsing::{ParseState, ScopeStack};

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let n = LIVE.fetch_add(l.size(), Relaxed) + l.size();
            PEAK.fetch_max(n, Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        LIVE.fetch_sub(l.size(), Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let q = System.realloc(p, l, new);
        if !q.is_null() {
            if new >= l.size() {
                let n = LIVE.fetch_add(new - l.size(), Relaxed) + new - l.size();
                PEAK.fetch_max(n, Relaxed);
            } else {
                LIVE.fetch_sub(l.size() - new, Relaxed);
            }
        }
        q
    }
}
#[global_allocator]
static A: Counting = Counting;

const N: usize = 50_000;

fn live() -> usize {
    LIVE.load(Relaxed)
}

fn main() {
    let e = Engine::load();
    let hl = Highlighter::new(&e.theme);
    println!(
        "size_of ParseState {} B, HighlightState {} B, ScopeStack {} B",
        std::mem::size_of::<ParseState>(),
        std::mem::size_of::<syntect::highlighting::HighlightState>(),
        std::mem::size_of::<ScopeStack>()
    );
    for (name, ext, lines) in [("rust", "rs", gen_rust(N)), ("markdown", "md", gen_md(N))] {
        let text_bytes: usize = lines.iter().map(|l| l.len()).sum();
        println!("== {name}: {N} lines, text {} KiB", text_bytes / 1024);
        for k in [1usize, 16, 64, 256] {
            // run the file once, keeping one checkpoint per k lines; measure live heap + private bytes delta
            let mut st = State::new(e.syntax(ext), &hl);
            let base_live = live();
            let base_priv = mem_info().private;
            let mut ck: Vec<State> = Vec::new();
            let mut cks: Vec<(ParseState, ScopeStack)> = Vec::new();
            let mut cps: Vec<ParseState> = Vec::new();
            // three storage variants measured in separate passes so each is isolated
            for variant in 0..3 {
                let l0 = live();
                let p0 = mem_info().private;
                let mut st2 = State::new(e.syntax(ext), &hl);
                for (i, l) in lines.iter().enumerate() {
                    if i % k == 0 {
                        match variant {
                            0 => ck.push(st2.clone()),
                            1 => cks.push((st2.ps.clone(), st2.hs.path.clone())),
                            _ => cps.push(st2.ps.clone()),
                        }
                    }
                    do_line(&mut st2, l, &e.set, &hl);
                }
                let dl = live() - l0;
                let dp = mem_info().private as i64 - p0 as i64;
                let (label, count) = match variant {
                    0 => ("ParseState + HighlightState", ck.len()),
                    1 => ("ParseState + ScopeStack", cks.len()),
                    _ => ("ParseState only", cps.len()),
                };
                println!(
                    "   K={k:<4} {label:<30} {count:>6} entries: heap {:8.2} MiB = {:6.0} B/entry; private-bytes delta {:+.2} MiB",
                    dl as f64 / 1048576.0,
                    dl as f64 / count as f64,
                    dp as f64 / 1048576.0
                );
            }
            drop((ck, cks, cps, st));
            let _ = (base_live, base_priv);
        }
        let m = mem_info();
        println!("   process now: private {:.1} MiB, working set {:.1} MiB, peak working set {:.1} MiB; allocator peak live {:.1} MiB", m.private as f64 / 1048576.0, m.working as f64 / 1048576.0, m.peak_working as f64 / 1048576.0, PEAK.load(Relaxed) as f64 / 1048576.0);
    }
    // base cost of the syntax set + theme
    let m = mem_info();
    println!("final private {:.1} MiB (includes syntax set, themes, both files' text)", m.private as f64 / 1048576.0);
}
