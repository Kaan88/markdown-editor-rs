use spike_highlight::*;
use std::time::Instant;
use syntect::highlighting::{HighlightState, Highlighter};
use syntect::parsing::{ParseState, SyntaxSet};

const N: usize = 50_000;
const KS: [usize; 4] = [1, 16, 64, 256];
fn reps() -> usize { std::env::var("REPS").ok().and_then(|v| v.parse().ok()).unwrap_or(7) }

fn files() -> Vec<(&'static str, &'static str, Vec<String>)> {
    let f = std::env::var("FILE").unwrap_or_default();
    let mut v = vec![];
    if f.is_empty() || f == "rust" { v.push(("rust", "rs", gen_rust(N))); }
    if f.is_empty() || f == "markdown" { v.push(("markdown", "md", gen_md(N))); }
    v
}

fn checkpoints(lines: &[String], k: usize, e: &Engine, ext: &str, hl: &Highlighter) -> Vec<State> {
    let mut st = State::new(e.syntax(ext), hl);
    let mut ck = Vec::with_capacity(lines.len() / k + 1);
    let mut sink = 0u64;
    for (i, l) in lines.iter().enumerate() {
        if i % k == 0 {
            ck.push(st.clone());
        }
        sink = sink.wrapping_add(do_line(&mut st, l, &e.set, hl));
    }
    std::hint::black_box(sink);
    ck
}

fn cmd_load() {
    let t = Instant::now();
    let set = SyntaxSet::load_defaults_newlines();
    let t_set = ms(t.elapsed());
    let t = Instant::now();
    let themes = syntect::highlighting::ThemeSet::load_defaults();
    let t_theme = ms(t.elapsed());
    let theme = themes.themes.get("base16-ocean.dark").unwrap().clone();
    let e = Engine { set, theme };
    let hl = Highlighter::new(&e.theme);
    let rs = gen_rust(1000);
    let md = gen_md(1000);
    let mut out = format!("set={t_set:.2}ms themes={t_theme:.2}ms");
    for (name, ext, lines) in [("rust", "rs", &rs), ("md", "md", &md)] {
        let mut st = State::new(e.syntax(ext), &hl);
        let t = Instant::now();
        for l in lines.iter() {
            std::hint::black_box(do_line(&mut st, l, &e.set, &hl));
        }
        let first = ms(t.elapsed());
        let mut st = State::new(e.syntax(ext), &hl);
        let t = Instant::now();
        for l in lines.iter() {
            std::hint::black_box(do_line(&mut st, l, &e.set, &hl));
        }
        out += &format!(" {name}_first1000={first:.2}ms {name}_second1000={:.2}ms", ms(t.elapsed()));
    }
    // uncompressed dump of the same set
    let path = std::env::temp_dir().join("spike_hl_syntaxes.bin");
    syntect::dumps::dump_to_uncompressed_file(&e.set, &path).unwrap();
    let size = std::fs::metadata(&path).unwrap().len();
    let data = std::fs::read(&path).unwrap();
    let t = Instant::now();
    let s2: SyntaxSet = syntect::dumps::from_uncompressed_data(&data).unwrap();
    out += &format!(" uncompressed_load={:.2}ms ({} KiB, {} syntaxes)", ms(t.elapsed()), size / 1024, s2.syntaxes().len());
    let _ = std::fs::remove_file(path);
    println!("{out}");
}

fn cmd_full() {
    let e = Engine::load();
    let hl = Highlighter::new(&e.theme);
    for (name, ext, lines) in files() {
        let bytes: usize = lines.iter().map(|l| l.len()).sum();
        println!("== {name}: {} lines, {} KiB, longest line {} bytes", lines.len(), bytes / 1024, lines.iter().map(|l| l.len()).max().unwrap());
        // warm up (lazy context loading)
        let _ = checkpoints(&lines[..2000], 64, &e, ext, &hl);
        let mut row = |label: &str, f: &mut dyn FnMut() -> f64| {
            let v: Vec<f64> = (0..reps()).map(|_| f()).collect();
            let (m, lo, hi) = stats(v);
            println!("{label:<34} median {m:8.1} ms  range {lo:.1}-{hi:.1}  => {:8.0} lines/s, {:.1} MB/s", N as f64 / (m / 1000.0), bytes as f64 / 1e6 / (m / 1000.0));
        };
        row("parse only (no style)", &mut || {
            let mut ps = ParseState::new(e.syntax(ext));
            let (_, t) = time(|| {
                let mut s = 0;
                for l in &lines {
                    s += parse_only(&mut ps, l, &e.set);
                }
                std::hint::black_box(s)
            });
            t
        });
        row("parse + style, no checkpoints", &mut || {
            let mut st = State::new(e.syntax(ext), &hl);
            let (_, t) = time(|| {
                let mut s = 0u64;
                for l in &lines {
                    s = s.wrapping_add(do_line(&mut st, l, &e.set, &hl));
                }
                std::hint::black_box(s)
            });
            t
        });
        for k in KS {
            row(&format!("parse + style, checkpoint every {k}"), &mut || time(|| checkpoints(&lines, k, &e, ext, &hl)).1);
        }
    }
}

struct EditResult {
    lines: usize,
    converged: bool,
    ms: f64,
}

/// Re-highlight after replacing line `at` with `text`, from the nearest checkpoint at or before it,
/// until the state at a checkpoint boundary past the edit equals the stored one (or EOF).
fn rehl(lines: &[String], at: usize, text: &str, ck: &[State], k: usize, e: &Engine, hl: &Highlighter) -> EditResult {
    let t = Instant::now();
    let c = at / k;
    let mut st = ck[c].clone();
    let mut i = c * k;
    let mut converged = false;
    let mut sink = 0u64;
    while i < lines.len() {
        let line = if i == at { text } else { lines[i].as_str() };
        sink = sink.wrapping_add(do_line(&mut st, line, &e.set, hl));
        i += 1;
        if i % k == 0 && i > at {
            let idx = i / k;
            if idx < ck.len() && st == ck[idx] {
                converged = true;
                break;
            }
        }
    }
    std::hint::black_box(sink);
    EditResult { lines: i - c * k, converged, ms: ms(t.elapsed()) }
}

fn cmd_edit() {
    let e = Engine::load();
    let hl = Highlighter::new(&e.theme);
    for (name, ext, lines) in files() {
        println!("== {name}");
        let cks: Vec<(usize, Vec<State>)> = KS.iter().map(|&k| (k, checkpoints(&lines, k, &e, ext, &hl))).collect();
        // typical edit sites: lines of code inside function bodies / inside prose and code fences
        let mut r = Rng(99);
        let typical: Vec<usize> = {
            let mut v = Vec::new();
            while v.len() < 40 {
                let i = 100 + r.below(N as u64 - 200) as usize;
                let l = &lines[i];
                let ok = if ext == "rs" { l.starts_with("    let ") } else { l.len() > 30 && !l.starts_with('|') && !l.starts_with("```") };
                if ok {
                    v.push(i);
                }
            }
            v
        };
        // (label, line index, transform)
        type Tx = Box<dyn Fn(&str) -> String>;
        let mut scen: Vec<(String, Vec<usize>, Tx)> = Vec::new();
        if ext == "rs" {
            scen.push(("typical: insert '_x' after 'let' (40 sites)".into(), typical.clone(), Box::new(|l| l.replacen("let ", "let _x", 1))));
            scen.push(("type '{' at 25000 (unbalanced)".into(), vec![25_000], Box::new(|l| format!("{}{{\n", l.trim_end()))));
            scen.push(("type '\"' at line 25000 (unterminated string)".into(), vec![25_000], Box::new(|l| format!("\"{l}"))));
            scen.push(("type '/*' at line 10".into(), vec![10], Box::new(|l| format!("/*{l}"))));
            scen.push(("type 'r#\"' at line 10".into(), vec![10], Box::new(|l| format!("r#\"{l}"))));
            scen.push(("type r##\" at line 10 (no later \"## in file)".into(), vec![10], Box::new(|l| format!("r##\"{l}"))));
            scen.push(("type '/*' at 25000".into(), vec![25_000], Box::new(|l| format!("/*{l}"))));
        } else {
            scen.push(("typical: insert 'x' at col 5 (40 sites)".into(), typical.clone(), Box::new(|l| format!("{}x{}", &l[..5], &l[5..]))));
            scen.push(("type '```' at line 10".into(), vec![10], Box::new(|_| "```\n".to_string())));
            scen.push(("type '```' at line 25000".into(), vec![25_000], Box::new(|_| "```\n".to_string())));
            scen.push(("type '<!--' at line 10".into(), vec![10], Box::new(|l| format!("<!--{l}"))));
        }
        let only: Option<usize> = std::env::var("SCEN").ok().and_then(|v| v.parse().ok());
        for (si, (label, sites, tx)) in scen.iter().enumerate() {
            if only.is_some() && only != Some(si) { continue; }
            println!("-- {label}");
            for (k, ck) in &cks {
                let mut lines_v = Vec::new();
                let mut ms_v = Vec::new();
                let mut conv = 0;
                for &s in sites {
                    for _ in 0..(if sites.len() == 1 { reps() } else { 1 }) {
                        let text = tx(&lines[s]);
                        let r = rehl(&lines, s, &text, ck, *k, &e, &hl);
                        lines_v.push(r.lines as f64);
                        ms_v.push(r.ms);
                        conv += r.converged as usize;
                    }
                }
                let n = lines_v.len();
                let (lm, llo, lhi) = stats(lines_v);
                let (tm, tlo, thi) = stats(ms_v);
                println!("   K={k:<4} lines median {lm:7.0} (range {llo:.0}-{lhi:.0})  time median {tm:8.3} ms (range {tlo:.3}-{thi:.3})  converged {conv}/{n}");
            }
        }
    }
}

fn cmd_viewport() {
    let e = Engine::load();
    let hl = Highlighter::new(&e.theme);
    for (name, ext, lines) in files() {
        println!("== {name}: highlight a 60-line viewport from the nearest checkpoint (200 random positions, plus line 40000 repeated)");
        for k in KS {
            let ck = checkpoints(&lines, k, &e, ext, &hl);
            let mut r = Rng(5);
            let mut times = Vec::new();
            let mut nl = Vec::new();
            for _ in 0..200 {
                let v = r.below(N as u64 - 60) as usize;
                let c = v / k;
                let t = Instant::now();
                let mut st = ck[c].clone();
                let mut s = 0u64;
                for i in c * k..v + 60 {
                    s = s.wrapping_add(do_line(&mut st, &lines[i], &e.set, &hl));
                }
                std::hint::black_box(s);
                times.push(ms(t.elapsed()));
                nl.push((v + 60 - c * k) as f64);
            }
            let mean_l = nl.iter().sum::<f64>() / nl.len() as f64;
            let p50 = pct(&mut times.clone(), 0.5);
            let p95 = pct(&mut times.clone(), 0.95);
            let mx = pct(&mut times, 1.0);
            println!("   K={k:<4} avg lines {mean_l:6.1}  time p50 {p50:.3} ms  p95 {p95:.3} ms  max {mx:.3} ms");
        }
        // pure 60 lines with a perfect state (lower bound)
        let ck = checkpoints(&lines, 1, &e, ext, &hl);
        let mut times = Vec::new();
        let mut r = Rng(6);
        for _ in 0..200 {
            let v = r.below(N as u64 - 60) as usize;
            let t = Instant::now();
            let mut st = ck[v].clone();
            let mut s = 0u64;
            for i in v..v + 60 {
                s = s.wrapping_add(do_line(&mut st, &lines[i], &e.set, &hl));
            }
            std::hint::black_box(s);
            times.push(ms(t.elapsed()));
        }
        println!("   exact state (60 lines only): p50 {:.3} ms  p95 {:.3} ms  max {:.3} ms", pct(&mut times.clone(), 0.5), pct(&mut times.clone(), 0.95), pct(&mut times, 1.0));
    }
}

fn cmd_slice() {
    let e = Engine::load();
    let hl = Highlighter::new(&e.theme);
    for (name, ext, lines) in files() {
        println!("== {name}: fixed-size slices over the whole file (3 passes pooled)");
        let _ = checkpoints(&lines[..2000], 64, &e, ext, &hl);
        for s in [25usize, 50, 100, 200, 400] {
            let mut times = Vec::new();
            for _ in 0..3 {
                let mut st = State::new(e.syntax(ext), &hl);
                for chunk in lines.chunks(s) {
                    let t = Instant::now();
                    let mut sum = 0u64;
                    for l in chunk {
                        sum = sum.wrapping_add(do_line(&mut st, l, &e.set, &hl));
                    }
                    std::hint::black_box(sum);
                    times.push(ms(t.elapsed()));
                }
            }
            println!("   {s:>4} lines/slice: p50 {:.3} ms  p99 {:.3} ms  max {:.3} ms", pct(&mut times.clone(), 0.5), pct(&mut times.clone(), 0.99), pct(&mut times, 1.0));
        }
        // time-boxed slices: check the clock every line, stop at budget
        for budget in [1.0f64, 2.0] {
            let mut times = Vec::new();
            let mut sizes = Vec::new();
            let mut st = State::new(e.syntax(ext), &hl);
            let mut i = 0;
            while i < lines.len() {
                let t = Instant::now();
                let start = i;
                while i < lines.len() && ms(t.elapsed()) < budget {
                    std::hint::black_box(do_line(&mut st, &lines[i], &e.set, &hl));
                    i += 1;
                }
                times.push(ms(t.elapsed()));
                sizes.push((i - start) as f64);
            }
            println!("   time-boxed {budget} ms (clock per line): slice time p50 {:.3} max {:.3} ms; lines/slice p50 {:.0} min {:.0}", pct(&mut times.clone(), 0.5), pct(&mut times, 1.0), pct(&mut sizes.clone(), 0.5), pct(&mut sizes, 0.0));
        }
        // per-line cost distribution
        let mut st = State::new(e.syntax(ext), &hl);
        let mut per = Vec::new();
        for l in &lines {
            let t = Instant::now();
            std::hint::black_box(do_line(&mut st, l, &e.set, &hl));
            per.push(t.elapsed().as_secs_f64() * 1e6);
        }
        let mean = per.iter().sum::<f64>() / per.len() as f64;
        println!("   per-line us: mean {mean:.1} p50 {:.1} p99 {:.1} max {:.1}", pct(&mut per.clone(), 0.5), pct(&mut per.clone(), 0.99), pct(&mut per, 1.0));
    }
}

fn cmd_eq() {
    let e = Engine::load();
    let hl = Highlighter::new(&e.theme);
    for (name, ext, lines) in files() {
        println!("== {name}: state comparison cost over all {N} line states");
        let ck = checkpoints(&lines, 1, &e, ext, &hl);
        let mut eq_both = Vec::new();
        let mut eq_ps = Vec::new();
        let mut eq_hs = Vec::new();
        let mut clone_t = Vec::new();
        let mut ne_both = Vec::new();
        let mut rebuild_t = Vec::new();
        let mut rebuild_ok = 0;
        const R: u32 = 20;
        for i in (0..N).step_by(5) {
            let a = &ck[i];
            let b = a.clone();
            let t = Instant::now();
            for _ in 0..R {
                std::hint::black_box(a == &b);
            }
            eq_both.push(t.elapsed().as_secs_f64() * 1e9 / R as f64);
            let t = Instant::now();
            for _ in 0..R {
                std::hint::black_box(a.ps == b.ps);
            }
            eq_ps.push(t.elapsed().as_secs_f64() * 1e9 / R as f64);
            let t = Instant::now();
            for _ in 0..R {
                std::hint::black_box(a.hs == b.hs);
            }
            eq_hs.push(t.elapsed().as_secs_f64() * 1e9 / R as f64);
            let t = Instant::now();
            for _ in 0..R {
                std::hint::black_box(a.clone());
            }
            clone_t.push(t.elapsed().as_secs_f64() * 1e9 / R as f64);
            if i + 1 < N {
                let c = &ck[i + 1];
                let t = Instant::now();
                for _ in 0..R {
                    std::hint::black_box(a == c);
                }
                ne_both.push(t.elapsed().as_secs_f64() * 1e9 / R as f64);
            }
            let t = Instant::now();
            let hs2 = HighlightState::new(&hl, a.hs.path.clone());
            rebuild_t.push(t.elapsed().as_secs_f64() * 1e9);
            if hs2 == a.hs {
                rebuild_ok += 1;
            }
        }
        let n = eq_both.len();
        let show = |l: &str, v: &mut Vec<f64>| println!("   {l:<44} p50 {:7.0} ns  p99 {:7.0} ns  max {:8.0} ns", pct(v, 0.5), pct(v, 0.99), pct(v, 1.0));
        show("ParseState+HighlightState == (equal)", &mut eq_both);
        show("ParseState == (equal)", &mut eq_ps);
        show("HighlightState == (equal)", &mut eq_hs);
        show("State == next line's state (mostly unequal)", &mut ne_both);
        show("State clone", &mut clone_t);
        show("HighlightState::new(scope stack) rebuild", &mut rebuild_t);
        println!("   rebuilt HighlightState equal to original: {rebuild_ok}/{n}");
    }
}

fn cmd_long() {
    let e = Engine::load();
    let hl = Highlighter::new(&e.theme);
    println!("per-line cost, lines of one kind in a row (fresh state per rep, {} reps, ~400 KB per rep)", reps());
    for (kind, ext) in [("rust-code", "rs"), ("blob", "rs"), ("json", "json"), ("minified-js", "js"), ("md-prose", "md")] {
        for len in [200usize, 2000, 20000] {
            let count = 400_000 / len;
            let lines: Vec<String> = (0..count).map(|_| long_line(kind, len)).collect();
            let _ = (0..2).map(|_| { let mut st = State::new(e.syntax(ext), &hl); for l in &lines[..count.min(20)] { do_line(&mut st, l, &e.set, &hl); } }).count();
            let mut per_line_us = Vec::new();
            let mut worst = Vec::new();
            for _ in 0..reps() {
                let mut st = State::new(e.syntax(ext), &hl);
                let mut mx = 0f64;
                let t = Instant::now();
                for l in &lines {
                    let t1 = Instant::now();
                    std::hint::black_box(do_line(&mut st, l, &e.set, &hl));
                    mx = mx.max(t1.elapsed().as_secs_f64() * 1000.0);
                }
                per_line_us.push(t.elapsed().as_secs_f64() * 1e6 / count as f64);
                worst.push(mx);
            }
            let (m, lo, hi) = stats(per_line_us);
            let (wm, _, _) = stats(worst);
            println!("   {kind:<12} {len:>6} chars x {count:>5}: {m:9.1} us/line (range {lo:.1}-{hi:.1}) = {:6.1} ns/char; worst single line {wm:.3} ms", m * 1000.0 / len as f64);
        }
    }
}

fn main() {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    match cmd.as_str() {
        "load" => cmd_load(),
        "full" => cmd_full(),
        "edit" => cmd_edit(),
        "viewport" => cmd_viewport(),
        "slice" => cmd_slice(),
        "eq" => cmd_eq(),
        "long" => cmd_long(),
        _ => eprintln!("usage: bench load|full|edit|viewport|slice|eq|long"),
    }
}
