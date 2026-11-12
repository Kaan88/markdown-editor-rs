//! Writes deterministic test files that are never committed:
//!
//! `cargo run --release -p mdedit-core --example gen -- lines <MiB> <path>`: code-like lines in
//! the buffer spike's mix (indented code, English comments, 3% Turkish and 3% CJK comment lines,
//! 7% blank lines, 4% long lines up to about 600 bytes), LF line endings.
//!
//! `... gen -- long <MiB> <path>`: one line of minified-JavaScript-like text and no line break.
//!
//! `... gen -- crlf <MiB> <path>`: as `lines`, with CRLF line endings.

fn main() {
    todo!()
}
