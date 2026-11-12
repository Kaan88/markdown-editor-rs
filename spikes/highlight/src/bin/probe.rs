use spike_highlight::*;
use syntect::highlighting::Highlighter;
use std::time::Instant;
fn main() {
    let e = Engine::load();
    let hl = Highlighter::new(&e.theme);
    let lines = gen_md(20000);
    let mut st = State::new(e.syntax("md"), &hl);
    let mut v = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let t = Instant::now();
        do_line(&mut st, l, &e.set, &hl);
        v.push((t.elapsed().as_secs_f64() * 1e6, i));
    }
    let tot: f64 = v.iter().map(|x| x.0).sum();
    v.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    println!("total {:.0} ms over {} lines", tot / 1000.0, lines.len());
    for (us, i) in v.iter().take(8) { println!("{us:8.0} us  len {:3}  {:?}", lines[*i].len(), &lines[*i][..lines[*i].len().min(70)]); }
    let slow: f64 = v.iter().filter(|x| x.0 > 1000.0).map(|x| x.0).sum();
    println!("lines >1ms: {}, their share of time {:.0}%", v.iter().filter(|x| x.0 > 1000.0).count(), slow / tot * 100.0);
    for (lo, hi) in [(0usize, 60usize), (60, 120), (120, 200), (200, 300), (300, 700)] {
        let sel: Vec<f64> = v.iter().filter(|x| { let n = lines[x.1].len(); n >= lo && n < hi && !lines[x.1].starts_with('|') }).map(|x| x.0).collect();
        if !sel.is_empty() { let mut s = sel.clone(); s.sort_by(|a, b| a.partial_cmp(b).unwrap()); println!("len {lo}-{hi}: n={} mean {:.0} us p50 {:.0} p95 {:.0} max {:.0}", sel.len(), sel.iter().sum::<f64>() / sel.len() as f64, s[s.len() / 2], s[s.len() * 95 / 100], s[s.len() - 1]); }
    }
    // classify by prefix
    for (name, f) in [("table '|'", (|l: &str| l.starts_with('|')) as fn(&str) -> bool), ("list '-'", |l| l.starts_with("- ")), ("quote '>'", |l| l.starts_with("> ")), ("heading '#'", |l| l.starts_with('#'))] {
        let sel: Vec<f64> = v.iter().filter(|x| f(&lines[x.1])).map(|x| x.0).collect();
        if !sel.is_empty() { println!("{name:<12} n={} mean {:.0} us max {:.0} us", sel.len(), sel.iter().sum::<f64>() / sel.len() as f64, sel.iter().cloned().fold(0.0, f64::max)); }
    }
}
#[allow(dead_code)]
fn _unused() {}
