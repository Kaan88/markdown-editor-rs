//! `--trace` output: start-up stages, load and first-frame times, memory, CPU time and frame times.
//! Every line starts `mdedit:` and goes to stderr.

use crate::platform;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub struct Trace {
    pub on: bool,
    pub started: Instant,
}

/// One frame's cost, split as the viewer splits it.
#[derive(Default, Clone, Copy, Debug)]
pub struct FrameTimes {
    pub layout: Duration,
    pub paint: Duration,
    pub compose: Duration,
    /// Copying changed rows into the window's buffer.
    pub copy: Duration,
    /// Handing them to the OS.
    pub present: Duration,
    pub rows: u32,
}

impl FrameTimes {
    /// Everything but the OS blit inside softbuffer's present.
    pub fn own_ms(&self) -> f64 {
        (self.layout + self.paint + self.compose + self.copy).as_secs_f64() * 1000.0
    }

    pub fn total_ms(&self) -> f64 {
        self.own_ms() + self.present.as_secs_f64() * 1000.0
    }
}

pub fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn mib(b: u64) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

impl Trace {
    pub fn new(on: bool, started: Instant) -> Trace {
        Trace { on, started }
    }

    pub fn since(&self) -> f64 {
        ms(self.started.elapsed())
    }

    /// "mdedit:   12.3 ms  what".
    pub fn stage(&self, what: &str) {
        if self.on {
            eprintln!("mdedit: {:7.1} ms  {what}", self.since());
        }
    }

    pub fn line(&self, text: &str) {
        if self.on {
            eprintln!("mdedit: {text}");
        }
    }

    pub fn memory(&self, when: &str) {
        if !self.on {
            return;
        }
        match platform::memory() {
            Some(m) => eprintln!(
                "mdedit: memory {when}: private {:.1} MiB, peak working set {:.1} MiB",
                mib(m.private),
                mib(m.peak_working_set)
            ),
            None => eprintln!("mdedit: memory {when}: not available"),
        }
    }

    pub fn cpu(&self, when: &str) {
        if !self.on {
            return;
        }
        match platform::cpu_time() {
            Some(t) => eprintln!("mdedit: cpu time {when}: {:.1} ms", ms(t)),
            None => eprintln!("mdedit: cpu time {when}: not available"),
        }
    }

    pub fn frame(&self, t: &FrameTimes) {
        if self.on {
            eprintln!(
                "mdedit: frame layout {:.2} paint {:.2} compose {:.2} copy {:.2} present {:.2} ms (total {:.2}), {} rows",
                ms(t.layout),
                ms(t.paint),
                ms(t.compose),
                ms(t.copy),
                ms(t.present),
                t.total_ms(),
                t.rows
            );
        }
    }
}

/// p50, p95 and max of `values` (sorted here); zeros when empty. Nearest rank.
pub fn summary(values: &[f64]) -> (f64, f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let pick = |q: f64| v[(((v.len() as f64) * q).ceil() as usize).clamp(1, v.len()) - 1];
    (pick(0.5), pick(0.95), v[v.len() - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles() {
        assert_eq!(summary(&[]), (0.0, 0.0, 0.0));
        assert_eq!(summary(&[3.0]), (3.0, 3.0, 3.0));
        let v: Vec<f64> = (1..=100).rev().map(f64::from).collect();
        assert_eq!(summary(&v), (50.0, 95.0, 100.0));
        let t = FrameTimes {
            layout: Duration::from_millis(1),
            paint: Duration::from_millis(2),
            present: Duration::from_millis(4),
            ..FrameTimes::default()
        };
        assert!((t.own_ms() - 3.0).abs() < 1e-9);
        assert!((t.total_ms() - 7.0).abs() < 1e-9);
    }
}
