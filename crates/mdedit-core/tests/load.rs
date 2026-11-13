//! Loading: encodings, streaming, cancel and limits, line endings, and the round trip that
//! phase 3's save relies on (decode, then encode with the detected encoding, gives the same
//! bytes).

use mdedit_core::eol::LineEnding;
use mdedit_core::load::{LoadError, LoadOptions, Loaded, load_bytes, load_path, load_reader};
use mdedit_core::{Encoding, Position};
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

fn opts() -> LoadOptions {
    LoadOptions {
        legacy: encoding_rs::WINDOWS_1254,
        max_bytes: 1 << 30,
        forced: None,
    }
}

fn text_of(l: &Loaded) -> String {
    let mut s = String::new();
    l.buffer.text(0..l.buffer.len_bytes(), &mut s);
    s
}

fn no() -> AtomicBool {
    AtomicBool::new(false)
}

/// A reader that returns one to seven bytes at a time.
struct Trickle<'a> {
    data: &'a [u8],
    pos: usize,
    step: usize,
}

impl<'a> Trickle<'a> {
    fn new(data: &'a [u8], seed: usize) -> Trickle<'a> {
        Trickle {
            data,
            pos: 0,
            step: seed,
        }
    }
}

impl Read for Trickle<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        self.step = self.step % 7 + 1;
        let n = self.step.min(out.len()).min(self.data.len() - self.pos);
        out[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

fn trickled(bytes: &[u8], seed: usize) -> Loaded {
    let mut r = Trickle::new(bytes, seed);
    load_reader(&mut r, None, &opts(), &|_, _| {}, &no()).unwrap()
}

fn same(a: &Loaded, b: &Loaded) {
    assert_eq!(text_of(a), text_of(b));
    assert_eq!(a.info.encoding, b.info.encoding);
    assert_eq!(a.info.eol, b.info.eol);
    assert_eq!(a.info.lossy, b.info.lossy);
    assert_eq!(a.info.has_nul, b.info.has_nul);
    assert_eq!(a.info.longest_line_chars, b.info.longest_line_chars);
    assert_eq!(a.info.file_bytes, b.info.file_bytes);
    assert_eq!(a.buffer.len_lines(), b.buffer.len_lines());
}

fn utf16(text: &str, le: bool, bom: bool) -> Vec<u8> {
    let mut out = Vec::new();
    if bom {
        out.extend_from_slice(if le { &[0xFF, 0xFE] } else { &[0xFE, 0xFF] });
    }
    for u in text.encode_utf16() {
        out.extend_from_slice(&if le { u.to_le_bytes() } else { u.to_be_bytes() });
    }
    out
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mdedit-core-tests-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

// ---------------------------------------------------------------------------------------------

#[test]
fn utf8_without_bom() {
    let l = load_bytes("hello ğüşiöç 漢字\nsecond".as_bytes(), &opts());
    assert_eq!(l.info.encoding, Encoding::Utf8);
    assert_eq!(l.info.encoding.label(), "UTF-8");
    assert!(!l.info.lossy);
    assert_eq!(text_of(&l), "hello ğüşiöç 漢字\nsecond");
    assert_eq!(l.buffer.len_lines(), 2);
    assert_eq!(l.info.eol.style(), LineEnding::Lf);
    assert_eq!(l.info.file_bytes, "hello ğüşiöç 漢字\nsecond".len() as u64);
}

#[test]
fn utf8_bom_is_stripped_and_labelled() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice("a\u{11F}b".as_bytes());
    let l = load_bytes(&bytes, &opts());
    assert_eq!(l.info.encoding, Encoding::Utf8Bom);
    assert_eq!(l.info.encoding.label(), "UTF-8 with BOM");
    assert_eq!(text_of(&l), "a\u{11F}b");
    assert!(!l.info.lossy);
    // A BOM and nothing else.
    let l = load_bytes(&[0xEF, 0xBB, 0xBF], &opts());
    assert_eq!(l.info.encoding, Encoding::Utf8Bom);
    assert_eq!(l.buffer.len_bytes(), 0);
}

#[test]
fn utf16_with_a_surrogate_pair() {
    let text = "a\u{1F600}b\r\nğ";
    for le in [true, false] {
        let l = load_bytes(&utf16(text, le, true), &opts());
        let want = if le {
            Encoding::Utf16Le
        } else {
            Encoding::Utf16Be
        };
        assert_eq!(l.info.encoding, want);
        assert_eq!(text_of(&l), text);
        assert!(!l.info.lossy);
        assert_eq!(l.info.eol.crlf, 1);
        assert_eq!(l.buffer.len_lines(), 2);
        assert_eq!(l.buffer.position(5), Position { line: 0, col: 2 });
    }
    assert_eq!(Encoding::Utf16Le.label(), "UTF-16 LE");
    assert_eq!(Encoding::Utf16Be.label(), "UTF-16 BE");
    // FF FE 00 00 is UTF-16 LE with a NUL, not UTF-32.
    let l = load_bytes(&[0xFF, 0xFE, 0, 0], &opts());
    assert_eq!(l.info.encoding, Encoding::Utf16Le);
    assert_eq!(text_of(&l), "\u{0}");
    assert!(l.info.has_nul);
}

#[test]
fn invalid_utf8_without_bom_falls_back_to_windows_1254() {
    let l = load_bytes(&[0xF0, 0xFD, 0xFE], &opts());
    assert_eq!(text_of(&l), "ğış");
    assert_eq!(l.info.encoding, Encoding::Legacy(encoding_rs::WINDOWS_1254));
    assert_eq!(l.info.encoding.label(), "Windows-1254");
    assert!(!l.info.lossy);
    // A multi-byte legacy page keeps its name.
    let sj = LoadOptions {
        legacy: encoding_rs::SHIFT_JIS,
        max_bytes: 1 << 30,
        forced: None,
    };
    let l = load_bytes(&[0x82, 0xA0, 0xFF], &sj);
    assert_eq!(l.info.encoding.label(), "Shift_JIS");
}

/// The Turkish fixture is found to be windows-1254 whatever the system's code page is (the PC
/// it is tested on runs code page 1252).
#[test]
fn legacy_page_is_guessed_from_the_text() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cp1254.txt");
    let bytes = std::fs::read(fixture).unwrap();
    let reference = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/turkish.txt"),
    )
    .unwrap();
    for legacy in [encoding_rs::WINDOWS_1252, encoding_rs::WINDOWS_1254] {
        let o = LoadOptions {
            legacy,
            max_bytes: 1 << 30,
            forced: None,
        };
        let l = load_bytes(&bytes, &o);
        assert_eq!(
            l.info.encoding,
            Encoding::Legacy(encoding_rs::WINDOWS_1254),
            "system page {}",
            legacy.name()
        );
        assert!(!l.info.lossy);
        assert!(text_of(&l).contains("Pijamalı hasta yağız şoföre"));
        // Below its first line, which names its page, the fixture is the UTF-8 fixture's text.
        let text = text_of(&l);
        assert!(text.lines().skip(1).eq(reference.lines()));
    }
    // Twenty copies, the case the checker ran.
    let many = bytes.repeat(20);
    let l = load_bytes(&many, &LoadOptions::default());
    assert_eq!(l.info.encoding, Encoding::Legacy(encoding_rs::WINDOWS_1254));
    // Cyrillic in windows-1251 on a 1252 system.
    let (cyr, _, _) = encoding_rs::WINDOWS_1251
        .encode("Съешь же ещё этих мягких французских булок, да выпей чаю.\n");
    let l = load_bytes(&cyr, &LoadOptions::default());
    assert_eq!(l.info.encoding, Encoding::Legacy(encoding_rs::WINDOWS_1251));
}

#[test]
fn mostly_utf8_is_read_as_utf8_with_replacements() {
    let mut bytes = "ğüş line\n".repeat(50).into_bytes();
    bytes.extend_from_slice(b"bad \xFF\xFE\xC3 bytes\n");
    bytes.extend_from_slice("şöç\n".as_bytes());
    let l = load_bytes(&bytes, &LoadOptions::default());
    assert_eq!(l.info.encoding, Encoding::Utf8);
    assert!(l.info.lossy);
    assert!(text_of(&l).contains("bad \u{FFFD}\u{FFFD}\u{FFFD} bytes\nşöç"));
    // Without a single valid multi-byte sequence the file is legacy, not lossy UTF-8.
    let l = load_bytes(b"ok line\nbad \xFF\xFE\xC3 bytes", &LoadOptions::default());
    assert!(matches!(l.info.encoding, Encoding::Legacy(_)));
    assert!(!l.info.lossy);
}

#[test]
fn legacy_bytes_that_do_not_round_trip_are_lossy() {
    let with = |legacy| LoadOptions {
        legacy,
        max_bytes: 1 << 30,
        forced: None,
    };
    // Each decodes without an error, but the page's encoder writes something else back. The
    // sample is too short for a sure guess, so the system page given is used.
    let cases: [(&'static encoding_rs::Encoding, &[u8]); 3] = [
        // An NEC-selected IBM extension, written back as FA 5C.
        (encoding_rs::SHIFT_JIS, b"ok \xED\x40"),
        // A GB18030 four-byte sequence, which GBK cannot write.
        (encoding_rs::GBK, b"ok \x81\x30\x81\x30"),
        // HKSCS, written back as numeric character references.
        (encoding_rs::BIG5, b"ok \x88\x62"),
    ];
    for (enc, bytes) in cases {
        let l = load_bytes(bytes, &with(enc));
        assert_eq!(l.info.encoding, Encoding::Legacy(enc), "{}", enc.name());
        assert!(l.info.lossy, "{}", enc.name());
    }
    // Text that does round trip in the same pages is not lossy.
    for (enc, text) in [
        (encoding_rs::SHIFT_JIS, "日本語のテキスト"),
        (encoding_rs::GBK, "中文文本"),
        (encoding_rs::BIG5, "繁體中文"),
    ] {
        let (bytes, _, unmappable) = enc.encode(text);
        assert!(!unmappable);
        let l = load_bytes(&bytes, &with(enc));
        assert_eq!(l.info.encoding, Encoding::Legacy(enc), "{}", enc.name());
        assert!(!l.info.lossy, "{}", enc.name());
        assert_eq!(text_of(&l), text);
    }
}

#[test]
fn late_invalid_byte_in_utf8_text_is_replaced() {
    // Valid UTF-8 for well over one read chunk, then a lone 0xFD: mostly UTF-8, so the file is
    // UTF-8 with one replacement, not a legacy page.
    let mut bytes = Vec::new();
    while bytes.len() < 200_000 {
        bytes.extend_from_slice("valid ğüşiöç line\r\n".as_bytes());
    }
    let valid_len = bytes.len();
    bytes.extend_from_slice(b"tail \xFD end");
    let l = load_bytes(&bytes, &opts());
    assert_eq!(l.info.encoding, Encoding::Utf8);
    assert!(l.info.lossy);
    let text = text_of(&l);
    assert!(text.ends_with("tail \u{FFFD} end"));
    assert!(text.starts_with("valid ğ"));
    assert_eq!(text.len(), valid_len + "tail \u{FFFD} end".len());
    // The same through every trickle size.
    for seed in 0..7 {
        let t = trickled(&bytes, seed);
        same(&l, &t);
    }
    // And through a file.
    let p = tmp("late-invalid.txt");
    std::fs::write(&p, &bytes).unwrap();
    let f = load_path(&p, &opts(), &|_, _| {}, &no()).unwrap();
    same(&l, &f);
    assert_eq!(f.info.file_bytes, bytes.len() as u64);
}

#[test]
fn invalid_utf8_after_a_bom_is_lossy() {
    let l = load_bytes(&[0xEF, 0xBB, 0xBF, b'a', 0xFF, b'b', 0xE2, 0x82], &opts());
    assert_eq!(l.info.encoding, Encoding::Utf8Bom);
    assert!(l.info.lossy);
    assert_eq!(text_of(&l), "a\u{FFFD}b\u{FFFD}");
    // A bad continuation after a lead byte, split across pieces.
    let bytes = [0xEF, 0xBB, 0xBF, 0xE2, 0x82, b'x', b'y'];
    let l = load_bytes(&bytes, &opts());
    assert_eq!(text_of(&l), "\u{FFFD}xy");
    for seed in 0..7 {
        same(&l, &trickled(&bytes, seed));
    }
    // Valid text after a BOM is not lossy.
    let l = load_bytes(&[0xEF, 0xBB, 0xBF, b'o', b'k'], &opts());
    assert!(!l.info.lossy);
}

#[test]
fn utf16_invalid_input_is_lossy() {
    // An unpaired high surrogate followed by 'a', LE.
    let l = load_bytes(&[0xFF, 0xFE, 0x00, 0xD8, 0x61, 0x00], &opts());
    assert!(l.info.lossy);
    assert_eq!(text_of(&l), "\u{FFFD}a");
    // An unpaired low surrogate, BE.
    let l = load_bytes(&[0xFE, 0xFF, 0xDC, 0x00, 0x00, 0x61], &opts());
    assert!(l.info.lossy);
    assert_eq!(text_of(&l), "\u{FFFD}a");
    // An odd final byte.
    let l = load_bytes(&[0xFF, 0xFE, 0x61, 0x00, 0x62], &opts());
    assert!(l.info.lossy);
    assert_eq!(text_of(&l), "a\u{FFFD}");
    // A high surrogate at the very end.
    let l = load_bytes(&[0xFF, 0xFE, 0x00, 0xD8], &opts());
    assert!(l.info.lossy);
    assert_eq!(text_of(&l), "\u{FFFD}");
    // Streaming gives the same.
    for bytes in [
        &[0xFF, 0xFE, 0x00, 0xD8, 0x61, 0x00][..],
        &[0xFF, 0xFE, 0x61, 0x00, 0x62][..],
    ] {
        let a = load_bytes(bytes, &opts());
        for seed in 0..7 {
            same(&a, &trickled(bytes, seed));
        }
    }
}

#[test]
fn empty_file() {
    let l = load_bytes(b"", &opts());
    assert_eq!(l.info.encoding, Encoding::Utf8);
    assert_eq!(l.buffer.len_bytes(), 0);
    assert_eq!(l.buffer.len_lines(), 1);
    assert_eq!(l.info.eol.style(), LineEnding::None);
    assert_eq!(l.info.longest_line_chars, 0);
    assert!(!l.info.lossy && !l.info.has_nul);
    let p = tmp("empty.txt");
    std::fs::write(&p, b"").unwrap();
    let f = load_path(&p, &opts(), &|_, _| {}, &no()).unwrap();
    assert_eq!(f.info.encoding, Encoding::Utf8);
    assert_eq!(f.buffer.len_lines(), 1);
    assert_eq!(f.info.file_bytes, 0);
}

#[test]
fn short_files_near_the_bom_length() {
    for bytes in [&b"a"[..], b"ab", b"\xEF", b"\xEF\xBB", b"\xFF", b"\xFE"] {
        let a = load_bytes(bytes, &opts());
        for seed in 0..7 {
            same(&a, &trickled(bytes, seed));
        }
    }
    // 0xEF 0xBB alone is not UTF-8: windows-1254 reads it.
    let l = load_bytes(b"\xEF\xBB", &opts());
    assert_eq!(l.info.encoding, Encoding::Legacy(encoding_rs::WINDOWS_1254));
    assert_eq!(text_of(&l), "ï»");
}

#[test]
fn nul_and_longest_line() {
    let l = load_bytes(b"ab\0cd\n\tx\n12345678901\r\nz", &opts());
    assert!(l.info.has_nul);
    // Line 0: 5 chars; line 1: tab (4) + x = 5; line 2: 11; line 3: 1.
    assert_eq!(l.info.longest_line_chars, 11);
    assert_eq!(l.buffer.len_lines(), 4);
    let l = load_bytes("\t\t\tx".as_bytes(), &opts());
    assert_eq!(l.info.longest_line_chars, 13);
    assert!(!l.info.has_nul);
    // Characters, not bytes; the last line has no break.
    let l = load_bytes("ğğğ\nabcd".as_bytes(), &opts());
    assert_eq!(l.info.longest_line_chars, 4);
    // Wide characters count two advances, combining marks none.
    let l = load_bytes("漢字漢字漢字".as_bytes(), &opts());
    assert_eq!(l.info.longest_line_chars, 12);
    let l = load_bytes("e\u{301}e\u{301}x".as_bytes(), &opts());
    assert_eq!(l.info.longest_line_chars, 3);
    // A CR LF pair split across pieces ends one line.
    let l = load_bytes(b"abc\r\n\r\nde", &opts());
    assert_eq!(l.info.longest_line_chars, 3);
}

#[test]
fn longest_line_across_pieces() {
    // Lines longer than one read chunk, with tabs, CRLF, and a long line last.
    let mut text = String::new();
    text.push_str(&"a".repeat(100_000));
    text.push_str("\r\n");
    text.push_str(&"\tb".repeat(40_000));
    text.push_str("\r\n");
    text.push_str(&"ğ".repeat(250_000));
    let l = load_bytes(text.as_bytes(), &opts());
    assert_eq!(l.info.longest_line_chars, 250_000);
    let p = tmp("long-lines.txt");
    std::fs::write(&p, text.as_bytes()).unwrap();
    let f = load_path(&p, &opts(), &|_, _| {}, &no()).unwrap();
    assert_eq!(f.info.longest_line_chars, 250_000);
    let l = load_bytes(&text.as_bytes()[..text.len() - 500_000], &opts());
    assert_eq!(l.info.longest_line_chars, 40_000 * 5);
    for seed in 0..7 {
        same(
            &l,
            &trickled(&text.as_bytes()[..text.len() - 500_000], seed),
        );
    }
}

#[test]
fn line_endings_in_loaded_files() {
    let l = load_bytes(b"a\r\nb\r\nc", &opts());
    assert_eq!(l.info.eol.style(), LineEnding::Crlf);
    assert_eq!((l.info.eol.crlf, l.info.eol.lf, l.info.eol.cr), (2, 0, 0));
    let l = load_bytes(b"a\nb\n", &opts());
    assert_eq!(l.info.eol.style(), LineEnding::Lf);
    assert_eq!(l.buffer.len_lines(), 3);
    let l = load_bytes(b"a\rb\rc", &opts());
    assert_eq!(l.info.eol.style(), LineEnding::Cr);
    let l = load_bytes(b"a\r\nb\nc\rd", &opts());
    assert_eq!(l.info.eol.style(), LineEnding::Mixed);
    assert_eq!(l.buffer.len_lines(), 4);
    let l = load_bytes(b"abc", &opts());
    assert_eq!(l.info.eol.style(), LineEnding::None);
    // CR at the end of one piece, LF at the start of the next, at every size.
    let bytes = b"one\r\ntwo\r\nthree\r\nfour\r\n";
    for seed in 0..7 {
        let l = trickled(bytes, seed);
        assert_eq!((l.info.eol.crlf, l.info.eol.lf, l.info.eol.cr), (4, 0, 0));
        assert_eq!(l.buffer.len_lines(), 5);
    }
    // A CR at the very end of the file is a lone CR.
    let l = load_bytes(b"a\r", &opts());
    assert_eq!((l.info.eol.crlf, l.info.eol.cr), (0, 1));
    assert_eq!(l.buffer.len_lines(), 2);
}

#[test]
fn crlf_split_at_a_read_chunk_boundary() {
    // The CR is the last byte of the first 64 KiB read, the LF the first of the next.
    let mut bytes = vec![b'x'; 64 * 1024 - 1];
    bytes.extend_from_slice(b"\r\nsecond\r\n");
    let p = tmp("split-crlf.txt");
    std::fs::write(&p, &bytes).unwrap();
    let f = load_path(&p, &opts(), &|_, _| {}, &no()).unwrap();
    assert_eq!((f.info.eol.crlf, f.info.eol.lf, f.info.eol.cr), (2, 0, 0));
    assert_eq!(f.buffer.len_lines(), 3);
    assert_eq!(f.info.longest_line_chars, 64 * 1024 - 1);
    assert_eq!(f.buffer.line_range(0), 0..64 * 1024 - 1);
}

#[test]
fn streaming_equals_whole() {
    let samples: Vec<Vec<u8>> = vec![
        "plain ASCII\nwith lines\r\nand a \u{1F600} emoji ğüşiöç 漢字\r\n"
            .as_bytes()
            .to_vec(),
        [&[0xEF, 0xBB, 0xBF][..], "bom ğ漢\u{1F600}\n".as_bytes()].concat(),
        utf16("utf16 \u{1F600} ğ\r\nline", true, true),
        utf16("utf16 \u{1F600} ğ\r\nline", false, true),
        vec![b'a', b'\r', b'\n', 0xF0, 0xFD, 0xFE, b'\n'],
        {
            // A multi-byte char at every offset mod 7.
            let mut v = Vec::new();
            for i in 0..40 {
                v.extend_from_slice("x".repeat(i % 7).as_bytes());
                v.extend_from_slice("漢ğ\u{1F600}".as_bytes());
            }
            v
        },
        b"nul\0inside\0\r\n".to_vec(),
    ];
    for bytes in &samples {
        let whole = load_bytes(bytes, &opts());
        for seed in 0..7 {
            same(&whole, &trickled(bytes, seed));
        }
    }
}

#[test]
fn cancel_limits_and_errors() {
    let p = tmp("small.txt");
    std::fs::write(&p, "some text\n").unwrap();
    // Cancelled before the first read.
    let flag = AtomicBool::new(true);
    assert_eq!(
        load_path(&p, &opts(), &|_, _| {}, &flag).err(),
        Some(LoadError::Cancelled)
    );
    let mut r: &[u8] = b"abc";
    assert_eq!(
        load_reader(&mut r, None, &opts(), &|_, _| {}, &flag).err(),
        Some(LoadError::Cancelled)
    );
    // Too large by max_bytes.
    let small = LoadOptions {
        max_bytes: 5,
        ..opts()
    };
    assert_eq!(
        load_path(&p, &small, &|_, _| {}, &no()).err(),
        Some(LoadError::TooLarge { bytes: 10, max: 5 })
    );
    // A file that is exactly the limit is fine.
    let exact = LoadOptions {
        max_bytes: 10,
        ..opts()
    };
    assert!(load_path(&p, &exact, &|_, _| {}, &no()).is_ok());
    // Not found, and a folder.
    let missing = tmp("does-not-exist.txt");
    assert_eq!(
        load_path(&missing, &opts(), &|_, _| {}, &no()).err(),
        Some(LoadError::NotFound)
    );
    let dir = p.parent().unwrap();
    assert_eq!(
        load_path(dir, &opts(), &|_, _| {}, &no()).err(),
        Some(LoadError::IsDirectory)
    );
    // A reader that fails.
    struct Bad;
    impl Read for Bad {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("disk on fire"))
        }
    }
    let err = load_reader(&mut Bad, None, &opts(), &|_, _| {}, &no()).err();
    assert!(matches!(err, Some(LoadError::Io(m)) if m.contains("disk on fire")));
    // A reader that is longer than the limit.
    let mut long: &[u8] = b"0123456789";
    assert!(matches!(
        load_reader(&mut long, None, &small, &|_, _| {}, &no()),
        Err(LoadError::TooLarge { max: 5, .. })
    ));
}

#[test]
fn cancel_between_chunks() {
    // The flag is set by the progress callback, which runs between chunks.
    let p = tmp("cancel-mid.txt");
    let data = vec![b'a'; 9 << 20];
    std::fs::write(&p, &data).unwrap();
    let flag = AtomicBool::new(false);
    let calls = AtomicUsize::new(0);
    let r = load_path(
        &p,
        &opts(),
        &|_, _| {
            calls.fetch_add(1, Ordering::Relaxed);
            flag.store(true, Ordering::Relaxed);
        },
        &flag,
    );
    assert_eq!(r.err(), Some(LoadError::Cancelled));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn progress_reports_every_step() {
    let p = tmp("progress.txt");
    let mut data = Vec::new();
    while data.len() < 10 << 20 {
        data.extend_from_slice(b"some line of text for the progress test\n");
    }
    std::fs::write(&p, &data).unwrap();
    let seen = std::sync::Mutex::new(Vec::new());
    let l = load_path(
        &p,
        &opts(),
        &|done, total| seen.lock().unwrap().push((done, total)),
        &no(),
    )
    .unwrap();
    let seen = seen.into_inner().unwrap();
    assert_eq!(l.info.file_bytes, data.len() as u64);
    // 10 MiB at a 4 MiB step: two reports, each at least one step apart.
    assert_eq!(seen.len(), 2, "{seen:?}");
    for w in seen.windows(2) {
        assert!(w[1].0 - w[0].0 >= 4 << 20);
    }
    for &(done, total) in &seen {
        assert_eq!(total, data.len() as u64);
        assert!(done <= total);
    }
    // A small file reports nothing.
    let q = tmp("progress-small.txt");
    std::fs::write(&q, b"tiny").unwrap();
    let n = AtomicUsize::new(0);
    load_path(
        &q,
        &opts(),
        &|_, _| {
            n.fetch_add(1, Ordering::Relaxed);
        },
        &no(),
    )
    .unwrap();
    assert_eq!(n.load(Ordering::Relaxed), 0);
}

#[test]
fn load_path_fills_the_stamp_and_time() {
    let p = tmp("stamp.txt");
    std::fs::write(&p, "abc").unwrap();
    let l = load_path(&p, &opts(), &|_, _| {}, &no()).unwrap();
    assert_eq!(l.info.stamp, mdshared_app::files::stamp(&p));
    assert!(matches!(
        l.info.stamp,
        mdshared_app::files::FileStamp::Present { size: 3, .. }
    ));
    assert_eq!(l.info.file_bytes, 3);
    assert!(l.info.elapsed.as_secs() < 5);
    // load_reader has no stamp.
    let mut r: &[u8] = b"abc";
    let l = load_reader(&mut r, Some(3), &opts(), &|_, _| {}, &no()).unwrap();
    assert_eq!(l.info.stamp, mdshared_app::files::FileStamp::Missing);
}

#[test]
fn error_messages_are_plain_sentences() {
    for e in [
        LoadError::NotFound,
        LoadError::IsDirectory,
        LoadError::TooLarge {
            bytes: 3 << 30,
            max: 1 << 30,
        },
        LoadError::StillChanging,
        LoadError::Cancelled,
        LoadError::Io("access denied".into()),
    ] {
        let s = e.to_string();
        assert!(
            s.ends_with('.') && s.chars().next().unwrap().is_uppercase(),
            "{s}"
        );
        assert!(!s.contains('\n'));
    }
    let s = LoadError::TooLarge {
        bytes: 3 << 30,
        max: 1 << 30,
    }
    .to_string();
    assert!(s.contains("3.0 GB") && s.contains("1.0 GB"), "{s}");
}

// ---------------------------------------------------------------------------------------------
// Round trip: decode, then encode with the detected encoding and the line endings the buffer
// kept, gives back the same bytes.

fn encode_back(l: &Loaded) -> Vec<u8> {
    let text = text_of(l);
    match l.info.encoding {
        Encoding::Utf8 => text.into_bytes(),
        Encoding::Utf8Bom => [&[0xEF, 0xBB, 0xBF][..], text.as_bytes()].concat(),
        Encoding::Utf16Le => utf16(&text, true, true),
        Encoding::Utf16Be => utf16(&text, false, true),
        Encoding::Legacy(e) => {
            let (bytes, _, unmappable) = e.encode(&text);
            assert!(!unmappable);
            bytes.into_owned()
        }
    }
}

#[test]
fn round_trip_every_encoding_and_line_ending() {
    let bodies = [
        "one\r\ntwo\r\nthree ğüşiöç 漢字 \u{1F600}\r\n",
        "one\ntwo\nthree ğüşiöç 漢字 \u{1F600}\n",
        "one\rtwo\rthree ğüşiöç\r",
        "mixed\r\nline\nendings\rhere\n\r\n\r\r\n",
        "no break at all ğ",
        "\n",
        "",
        "tabs\tand\0nul\u{1}\u{7F}\r\n",
    ];
    for body in bodies {
        let mut inputs: Vec<Vec<u8>> = vec![
            body.as_bytes().to_vec(),
            [&[0xEF, 0xBB, 0xBF][..], body.as_bytes()].concat(),
            utf16(body, true, true),
            utf16(body, false, true),
        ];
        // Windows-1254 text: take the body's characters that exist in the page, as bytes.
        let (bytes, _, unmappable) = encoding_rs::WINDOWS_1254.encode(body);
        if !unmappable {
            inputs.push(bytes.into_owned());
        }
        // Every byte value in Windows-1254, between line breaks.
        let mut all = Vec::new();
        for b in 0u8..=255 {
            all.push(b);
            if b % 40 == 0 {
                all.extend_from_slice(b"\r\n");
            }
        }
        inputs.push(all);
        for bytes in inputs {
            let l = load_bytes(&bytes, &opts());
            if bytes.is_empty() {
                assert_eq!(encode_back(&l), bytes);
                continue;
            }
            assert!(!l.info.lossy, "{bytes:?}");
            assert_eq!(encode_back(&l), bytes, "encoding {:?}", l.info.encoding);
            // The same through a file and through trickled reads.
            let t = trickled(&bytes, 3);
            assert_eq!(encode_back(&t), bytes);
        }
    }
}

#[test]
fn round_trip_through_a_file() {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("utf8-crlf.txt", "a\r\nb\r\nğ\r\n".as_bytes().to_vec()),
        (
            "utf8-bom-lf.txt",
            [&[0xEF, 0xBB, 0xBF][..], "a\nb\n".as_bytes()].concat(),
        ),
        ("utf16le.txt", utf16("a\r\nb\u{1F600}\r\n", true, true)),
        ("utf16be.txt", utf16("a\nb\u{1F600}\n", false, true)),
        ("cp1254-mixed.txt", b"a\xF0\r\nb\xFD\nc\xFE\r".to_vec()),
    ];
    for (name, bytes) in cases {
        let p = tmp(name);
        std::fs::write(&p, &bytes).unwrap();
        let l = load_path(&p, &opts(), &|_, _| {}, &no()).unwrap();
        assert_eq!(encode_back(&l), bytes, "{name}");
        assert_eq!(l.info.file_bytes, bytes.len() as u64);
    }
}

// ---- a chosen encoding

fn forced(enc: Encoding) -> LoadOptions {
    LoadOptions {
        forced: Some(enc),
        ..opts()
    }
}

#[test]
fn forced_utf8_on_a_windows_1254_file_is_lossy() {
    // "ğış" in windows-1254: F0 FD FE are all invalid UTF-8.
    let bytes = [b'a', 0xF0, 0xFD, 0xFE, b'z'];
    let l = load_bytes(&bytes, &forced(Encoding::Utf8));
    assert_eq!(text_of(&l), "a\u{FFFD}\u{FFFD}\u{FFFD}z");
    assert_eq!(l.info.encoding, Encoding::Utf8);
    assert!(l.info.lossy);
}

#[test]
fn forced_windows_1254_on_utf8_text_is_mojibake_and_not_lossy() {
    let text = "ğış";
    let o = forced(Encoding::Legacy(encoding_rs::WINDOWS_1254));
    let l = load_bytes(text.as_bytes(), &o);
    // C4 9F C4 B1 C5 9F read as windows-1254: Ä, Ÿ, Ä, ±, Å, Ÿ.
    assert_eq!(text_of(&l), "Ä\u{178}Ä±Å\u{178}");
    assert_eq!(l.info.encoding, Encoding::Legacy(encoding_rs::WINDOWS_1254));
    // Each of those bytes decodes to a character the page's encoder writes back as the same
    // byte, so the round trip holds: not lossy.
    assert!(!l.info.lossy);
}

#[test]
fn forced_utf16_le_without_a_bom() {
    let bytes: Vec<u8> = "a\nİ漢".encode_utf16().flat_map(u16::to_le_bytes).collect();
    let l = load_bytes(&bytes, &forced(Encoding::Utf16Le));
    assert_eq!(text_of(&l), "a\nİ漢");
    assert_eq!(l.info.encoding, Encoding::Utf16Le);
    assert!(!l.info.lossy);
    // Not forced, the same bytes are not read as UTF-16.
    assert_ne!(text_of(&load_bytes(&bytes, &opts())), "a\nİ漢");
    // A BOM of the same order is skipped; an odd byte count is lossy.
    let mut with_bom = vec![0xFF, 0xFE];
    with_bom.extend_from_slice(&bytes);
    let l = load_bytes(&with_bom, &forced(Encoding::Utf16Le));
    assert_eq!(text_of(&l), "a\nİ漢");
    with_bom.push(0x41);
    assert!(load_bytes(&with_bom, &forced(Encoding::Utf16Le)).info.lossy);
    // And big endian.
    let mut be = vec![0xFE, 0xFF];
    be.extend("ab".encode_utf16().flat_map(u16::to_be_bytes));
    let l = load_bytes(&be, &forced(Encoding::Utf16Be));
    assert_eq!(text_of(&l), "ab");
    assert_eq!(l.info.encoding, Encoding::Utf16Be);
}

#[test]
fn forced_utf8_skips_a_bom_and_keeps_the_forced_label() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice("héllo\r\nx".as_bytes());
    for enc in [Encoding::Utf8, Encoding::Utf8Bom] {
        let l = load_bytes(&bytes, &forced(enc));
        assert_eq!(text_of(&l), "héllo\r\nx");
        assert_eq!(l.info.encoding, enc);
        assert!(!l.info.lossy);
        assert_eq!(l.info.eol.crlf, 1);
    }
    // Without a BOM nothing is skipped.
    let l = load_bytes("héllo".as_bytes(), &forced(Encoding::Utf8Bom));
    assert_eq!(text_of(&l), "héllo");
}

#[test]
fn forcing_what_the_detector_chose_changes_nothing() {
    let mut bom8 = vec![0xEF, 0xBB, 0xBF];
    bom8.extend_from_slice("héllo\r\nş\n".as_bytes());
    let mut le = vec![0xFF, 0xFE];
    le.extend("héllo\r\nş".encode_utf16().flat_map(u16::to_le_bytes));
    let mut be = vec![0xFE, 0xFF];
    be.extend("héllo\rş".encode_utf16().flat_map(u16::to_be_bytes));
    let cases: Vec<Vec<u8>> = vec![
        "plain utf-8 İstanbul\r\nx\n".as_bytes().to_vec(),
        bom8,
        le,
        be,
        vec![b'a', 0xF0, 0xFD, 0xFE, b'\n', b'z'],
        b"ok line\nbad \xFF\xFE\xC3 bytes".to_vec(),
        Vec::new(),
    ];
    for bytes in cases {
        let free = load_bytes(&bytes, &opts());
        let same = load_bytes(&bytes, &forced(free.info.encoding));
        assert_eq!(text_of(&same), text_of(&free));
        assert_eq!(same.info.encoding, free.info.encoding);
        assert_eq!(same.info.lossy, free.info.lossy);
        assert_eq!(same.info.eol, free.info.eol);
        assert_eq!(same.info.has_nul, free.info.has_nul);
        assert_eq!(same.info.longest_line_chars, free.info.longest_line_chars);
    }
}

#[test]
fn forced_encoding_works_through_a_reader_and_a_path() {
    let bytes = [b'a', 0xF0, 0xFD, 0xFE, b'\n'];
    let o = forced(Encoding::Legacy(encoding_rs::WINDOWS_1254));
    let mut r = Trickle::new(&bytes, 3);
    let l = load_reader(&mut r, None, &o, &|_, _| {}, &no()).unwrap();
    assert_eq!(text_of(&l), "ağış\n");
    let p = tmp("forced.txt");
    std::fs::write(&p, bytes).unwrap();
    let l = load_path(&p, &forced(Encoding::Utf8), &|_, _| {}, &no()).unwrap();
    assert!(l.info.lossy);
    assert_eq!(l.info.encoding, Encoding::Utf8);
    let _ = std::fs::remove_file(&p);
}
