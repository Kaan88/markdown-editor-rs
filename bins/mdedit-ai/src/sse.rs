//! Server-sent events, as both APIs stream them: `event:` and `data:` lines, a blank line ends
//! an event.

use std::io::{BufRead, BufReader, Read};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// The longest line taken from a stream.
pub const LINE_MAX: usize = 1024 * 1024;
/// How often a waiting reader looks at the cancel flag.
const POLL: Duration = Duration::from_millis(200);

/// One event: its name (empty when it had none) and its data lines joined with newlines.
#[derive(Debug, PartialEq, Eq)]
pub struct Event {
    pub name: String,
    pub data: String,
}

/// Why a stream ended early.
#[derive(Debug)]
pub enum Broken {
    Io(std::io::Error),
    /// Nothing came for the idle time.
    Idle,
    /// A line longer than [`LINE_MAX`].
    LineTooLong,
}

enum Line {
    Text(String),
    End,
    Failed(Broken),
}

/// Reads events from `input` and hands each to `f` until `f` returns false or the stream ends.
///
/// The stream is read on its own thread, so a server that goes silent still lets a cancel
/// (`stop`) end the run within [`POLL`], and nothing for `idle` ends it with [`Broken::Idle`].
/// A reader left waiting on a silent server ends with the connection.
pub fn read(
    input: impl Read + Send + 'static,
    stop: &dyn Fn() -> bool,
    idle: Duration,
    f: &mut dyn FnMut(Event) -> bool,
) -> Result<(), Broken> {
    let (tx, rx) = mpsc::sync_channel::<Line>(64);
    std::thread::spawn(move || {
        let mut input = BufReader::new(input);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let line = match input
                .by_ref()
                .take(LINE_MAX as u64 + 1)
                .read_until(b'\n', &mut buf)
            {
                Ok(0) => Line::End,
                Ok(_) if buf.len() > LINE_MAX => Line::Failed(Broken::LineTooLong),
                Ok(_) => Line::Text(String::from_utf8_lossy(&buf).into_owned()),
                Err(e) => Line::Failed(Broken::Io(e)),
            };
            let last = !matches!(line, Line::Text(_));
            if tx.send(line).is_err() || last {
                return;
            }
        }
    });
    let mut events = Parser::default();
    let mut deadline = Instant::now() + idle;
    loop {
        if stop() {
            return Ok(());
        }
        match rx.recv_timeout(POLL) {
            Ok(Line::Text(l)) => {
                deadline = Instant::now() + idle;
                if let Some(e) = events.line(&l)
                    && !f(e)
                {
                    return Ok(());
                }
            }
            Ok(Line::End) | Err(RecvTimeoutError::Disconnected) => {
                if let Some(e) = events.end() {
                    f(e);
                }
                return Ok(());
            }
            Ok(Line::Failed(b)) => return Err(b),
            Err(RecvTimeoutError::Timeout) if Instant::now() >= deadline => {
                return Err(Broken::Idle);
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

/// Turns lines into events.
#[derive(Default)]
struct Parser {
    name: String,
    data: String,
    has_data: bool,
}

impl Parser {
    /// Takes one line; the event it ends, if any.
    fn line(&mut self, line: &str) -> Option<Event> {
        let l = line.trim_end_matches(['\n', '\r']);
        if l.is_empty() {
            let e = self.end();
            self.name.clear();
            return e;
        }
        if l.starts_with(':') {
            return None;
        }
        let (field, value) = match l.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (l, ""),
        };
        match field {
            "event" => self.name = value.to_string(),
            "data" => {
                if self.has_data {
                    self.data.push('\n');
                }
                self.data.push_str(value);
                self.has_data = true;
            }
            _ => {}
        }
        None
    }

    /// The event being read, if it has data.
    fn end(&mut self) -> Option<Event> {
        if !std::mem::take(&mut self.has_data) {
            return None;
        }
        Some(Event {
            name: std::mem::take(&mut self.name),
            data: std::mem::take(&mut self.data),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_split_on_blank_lines() {
        let s = ": ping\nevent: a\ndata: {\"x\":1}\n\ndata: one\ndata: two\r\n\r\ndata: [DONE]";
        let mut got = Vec::new();
        read(s.as_bytes(), &|| false, Duration::from_secs(5), &mut |e| {
            got.push(e);
            true
        })
        .unwrap();
        assert_eq!(
            got,
            vec![
                Event {
                    name: "a".into(),
                    data: "{\"x\":1}".into()
                },
                Event {
                    name: String::new(),
                    data: "one\ntwo".into()
                },
                Event {
                    name: String::new(),
                    data: "[DONE]".into()
                },
            ]
        );
    }

    /// A server that sends nothing: blocks until the test ends.
    struct Silent;
    impl Read for Silent {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            std::thread::sleep(Duration::from_secs(3600));
            Ok(0)
        }
    }

    #[test]
    fn a_silent_stream_is_cancelled_at_once() {
        let t = Instant::now();
        let stop = || t.elapsed() > Duration::from_millis(300);
        let r = read(Silent, &stop, Duration::from_secs(600), &mut |_| true);
        assert!(r.is_ok());
        assert!(t.elapsed() < Duration::from_secs(1), "{:?}", t.elapsed());
    }

    #[test]
    fn a_silent_stream_ends_after_the_idle_time() {
        let t = Instant::now();
        let r = read(Silent, &|| false, Duration::from_millis(400), &mut |_| true);
        assert!(matches!(r, Err(Broken::Idle)), "{r:?}");
        assert!(t.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_huge_line_is_refused() {
        let s = format!("data: {}\n\n", "x".repeat(LINE_MAX + 10));
        let r = read(
            std::io::Cursor::new(s),
            &|| false,
            Duration::from_secs(5),
            &mut |_| panic!("no event"),
        );
        assert!(matches!(r, Err(Broken::LineTooLong)), "{r:?}");
    }
}
