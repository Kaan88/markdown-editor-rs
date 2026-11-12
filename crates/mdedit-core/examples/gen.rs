//! Writes deterministic test files that are never committed:
//!
//! `cargo run --release -p mdedit-core --example gen -- lines <MiB> <path>`: code-like lines in
//! the buffer spike's mix (indented code, English comments, 3% Turkish and 3% CJK comment lines,
//! 7% blank lines, 4% long lines up to about 600 bytes), LF line endings.
//!
//! `... gen -- long <MiB> <path>`: one line of minified-JavaScript-like text and no line break.
//!
//! `... gen -- crlf <MiB> <path>`: as `lines`, with CRLF line endings.
//!
//! Put the files under `target/data/`.

mod common;

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: gen lines|long|crlf <MiB> <path>";
    let (Some(kind), Some(mib), Some(path)) = (args.first(), args.get(1), args.get(2)) else {
        eprintln!("{usage}");
        std::process::exit(2);
    };
    let Ok(mib) = mib.parse::<usize>() else {
        eprintln!("{usage}");
        std::process::exit(2);
    };
    let path = Path::new(path);
    let result = match kind.as_str() {
        "lines" => common::gen_lines(mib, path, "\n"),
        "crlf" => common::gen_lines(mib, path, "\r\n"),
        "long" => common::gen_long(mib, path),
        _ => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("gen: {e}");
        std::process::exit(1);
    }
}
