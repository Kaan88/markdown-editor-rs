//! Server-sent events, as both APIs stream them: `event:` and `data:` lines, a blank line ends
//! an event.

use std::io::{BufRead, BufReader, Read};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// The longest line taken from a stream.
pub const LINE_MAX: usize = 1024 * 1024;
/// The most data one event may gather over its lines.
pub const EVENT_MAX: usize = 1024 * 1024;
/// Events read ahead of the run: with [`EVENT_MAX`], at most 8 MB waits.
const AHEAD: usize = 8;
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
    /// A line longer than [`LINE_MAX`], an event over [`EVENT_MAX`], a body over its limit.
    TooLong,
}

enum Item<T> {
    One(T),
    End,
    Failed(Broken),
}

/// Waits for the reader thread's items, handing each to `f` until it returns false: a cancel
/// (`stop`) is seen within [`POLL`], and nothing for `idle` is [`Broken::Idle`].
fn pump<T>(
    rx: Receiver<Item<T>>,
    stop: &dyn Fn() -> bool,
    idle: Duration,
    f: &mut dyn FnMut(T) -> bool,
) -> Result<(), Broken> {
    let mut deadline = Instant::now() + idle;
    loop {
        if stop() {
            return Ok(());
        }
        match rx.recv_timeout(POLL) {
            Ok(Item::One(t)) => {
                deadline = Instant::now() + idle;
                if !f(t) {
                    return Ok(());
                }
            }
            Ok(Item::End) | Err(RecvTimeoutError::Disconnected) => return Ok(()),
            Ok(Item::Failed(b)) => return Err(b),
            Err(RecvTimeoutError::Timeout) if Instant::now() >= deadline => {
                return Err(Broken::Idle);
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

/// Reads events from `input` and hands each to `f` until `f` returns false or the stream ends.
///
/// The stream is read and parsed on its own thread, so a server that goes silent still lets a
/// cancel (`stop`) end the run within [`POLL`], and nothing for `idle` ends it with
/// [`Broken::Idle`]. The thread stops after an event `last` says ends the answer, dropping the
/// body with its connection; after a cancel or a failure it ends when its read returns (at the
/// latest at the agent's body timeout), holding nothing the engine waits for.
pub fn read(
    input: impl Read + Send + 'static,
    stop: &dyn Fn() -> bool,
    idle: Duration,
    last: fn(&Event) -> bool,
    f: &mut dyn FnMut(Event) -> bool,
) -> Result<(), Broken> {
    let (tx, rx) = mpsc::sync_channel::<Item<Event>>(AHEAD);
    std::thread::spawn(move || {
        let mut input = BufReader::new(input);
        let mut buf = Vec::new();
        let mut events = Parser::default();
        loop {
            buf.clear();
            let read = input
                .by_ref()
                .take(LINE_MAX as u64 + 1)
                .read_until(b'\n', &mut buf);
            let item = match read {
                Ok(0) => match events.end() {
                    Some(e) => {
                        let _ = tx.send(Item::One(e));
                        Item::End
                    }
                    None => Item::End,
                },
                Ok(_) if buf.len() > LINE_MAX => Item::Failed(Broken::TooLong),
                Ok(_) => match events.line(&String::from_utf8_lossy(&buf)) {
                    _ if events.data.len() > EVENT_MAX => Item::Failed(Broken::TooLong),
                    Some(e) => {
                        let done = last(&e);
                        if tx.send(Item::One(e)).is_err() || done {
                            return;
                        }
                        continue;
                    }
                    None => continue,
                },
                Err(e) => Item::Failed(Broken::Io(e)),
            };
            let _ = tx.send(item);
            return;
        }
    });
    pump(rx, stop, idle, f)
}

/// Reads at most `max` bytes of `input` (an error's body) on its own thread, with the same cancel
/// and idle limits as [`read`]. What came before a cancel is returned.
pub fn read_body(
    input: impl Read + Send + 'static,
    stop: &dyn Fn() -> bool,
    idle: Duration,
    max: usize,
) -> Result<String, Broken> {
    let (tx, rx) = mpsc::sync_channel::<Item<Vec<u8>>>(AHEAD);
    std::thread::spawn(move || {
        let mut input = input.take(max as u64);
        let mut chunk = vec![0; 8 * 1024];
        loop {
            let item = match input.read(&mut chunk) {
                Ok(0) => Item::End,
                Ok(n) => Item::One(chunk[..n].to_vec()),
                Err(e) => Item::Failed(Broken::Io(e)),
            };
            let more = matches!(item, Item::One(_));
            if tx.send(item).is_err() || !more {
                return;
            }
        }
    });
    let mut body = Vec::new();
    pump(rx, stop, idle, &mut |c| {
        body.extend_from_slice(&c);
        body.len() < max
    })?;
    Ok(String::from_utf8_lossy(&body).into_owned())
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
        read(
            s.as_bytes(),
            &|| false,
            Duration::from_secs(5),
            |_| false,
            &mut |e| {
                got.push(e);
                true
            },
        )
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
        let r = read(
            Silent,
            &stop,
            Duration::from_secs(600),
            |_| false,
            &mut |_| true,
        );
        assert!(r.is_ok());
        assert!(t.elapsed() < Duration::from_secs(1), "{:?}", t.elapsed());
    }

    #[test]
    fn a_silent_stream_ends_after_the_idle_time() {
        let t = Instant::now();
        let r = read(
            Silent,
            &|| false,
            Duration::from_millis(400),
            |_| false,
            &mut |_| true,
        );
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
            |_| false,
            &mut |_| panic!("no event"),
        );
        assert!(matches!(r, Err(Broken::TooLong)), "{r:?}");
    }

    /// Audit 1, round 2: many short lines of one event are bounded as a long line is.
    #[test]
    fn a_huge_event_of_short_lines_is_refused() {
        let line = format!("data: {}\n", "y".repeat(1000));
        let s = line.repeat(EVENT_MAX / 1000 + 10) + "\n";
        let r = read(
            std::io::Cursor::new(s),
            &|| false,
            Duration::from_secs(5),
            |_| false,
            &mut |_| panic!("no event"),
        );
        assert!(matches!(r, Err(Broken::TooLong)), "{r:?}");
    }

    /// The last event, then a server that keeps the connection and sends nothing.
    struct Then {
        head: std::io::Cursor<Vec<u8>>,
        dropped: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }
    impl Read for Then {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            match self.head.read(b)? {
                0 => {
                    std::thread::sleep(Duration::from_secs(3600));
                    Ok(0)
                }
                n => Ok(n),
            }
        }
    }
    impl Drop for Then {
        fn drop(&mut self) {
            self.dropped
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Audit 1, round 2: after the answer's last event the reader lets go of the body at once.
    #[test]
    fn the_reader_lets_go_after_the_last_event() {
        let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let input = Then {
            head: std::io::Cursor::new(b"data: a\n\ndata: [DONE]\n\n".to_vec()),
            dropped: dropped.clone(),
        };
        let mut got = Vec::new();
        let r = read(
            input,
            &|| false,
            Duration::from_secs(5),
            |e| e.data == "[DONE]",
            &mut |e| {
                got.push(e.data);
                true
            },
        );
        assert!(r.is_ok());
        assert_eq!(got, ["a", "[DONE]"]);
        let t = Instant::now();
        while !dropped.load(std::sync::atomic::Ordering::SeqCst) {
            assert!(
                t.elapsed() < Duration::from_secs(2),
                "the body is still held"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Audit 1, round 2: an error body that never comes does not hold a cancel up.
    #[test]
    fn a_silent_error_body_is_cancelled_at_once() {
        let t = Instant::now();
        let stop = || t.elapsed() > Duration::from_millis(300);
        let r = read_body(Silent, &stop, Duration::from_secs(600), 64 * 1024);
        assert_eq!(r.unwrap(), "");
        assert!(t.elapsed() < Duration::from_secs(1));
    }
}
