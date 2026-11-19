//! Server-sent events, as both APIs stream them: `event:` and `data:` lines, a blank line ends
//! an event.

use std::io::BufRead;

/// One event: its name (empty when it had none) and its data lines joined with newlines.
#[derive(Debug, PartialEq, Eq)]
pub struct Event {
    pub name: String,
    pub data: String,
}

/// Reads events from `input` and hands each to `f` until `f` returns false or the stream ends.
/// `stop` is asked before each line, so a cancelled run ends at the next line.
pub fn read(
    input: &mut impl BufRead,
    stop: &dyn Fn() -> bool,
    f: &mut dyn FnMut(Event) -> bool,
) -> std::io::Result<()> {
    let (mut name, mut data) = (String::new(), String::new());
    let mut has_data = false;
    let mut line = String::new();
    loop {
        if stop() {
            return Ok(());
        }
        line.clear();
        if input.read_line(&mut line)? == 0 {
            if has_data {
                f(Event { name, data });
            }
            return Ok(());
        }
        let l = line.trim_end_matches(['\n', '\r']);
        if l.is_empty() {
            if has_data
                && !f(Event {
                    name: std::mem::take(&mut name),
                    data: std::mem::take(&mut data),
                })
            {
                return Ok(());
            }
            name.clear();
            data.clear();
            has_data = false;
            continue;
        }
        if l.starts_with(':') {
            continue;
        }
        let (field, value) = match l.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (l, ""),
        };
        match field {
            "event" => name = value.to_string(),
            "data" => {
                if has_data {
                    data.push('\n');
                }
                data.push_str(value);
                has_data = true;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_split_on_blank_lines() {
        let s = ": ping\nevent: a\ndata: {\"x\":1}\n\ndata: one\ndata: two\r\n\r\ndata: [DONE]";
        let mut got = Vec::new();
        read(&mut s.as_bytes(), &|| false, &mut |e| {
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
}
