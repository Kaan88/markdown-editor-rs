use super::*;
use crate::load::{LoadOptions, load_bytes};
use std::time::Instant;

fn opts() -> LoadOptions {
    LoadOptions {
        legacy: encoding_rs::WINDOWS_1254,
        ..LoadOptions::default()
    }
}

fn no_cancel() -> AtomicBool {
    AtomicBool::new(false)
}

fn encode_all(rope: &Rope, enc: Encoding) -> Vec<u8> {
    let mut out = Vec::new();
    encode_to(rope, enc, &mut out, &no_cancel()).unwrap();
    out
}

/// Loads `bytes`; `None` when the load is lossy (the file would stay read-only), else whether
/// the saved bytes equal the input.
fn round_trip(bytes: &[u8]) -> Option<bool> {
    let loaded = load_bytes(bytes, &opts());
    if loaded.info.lossy {
        return None;
    }
    Some(encode_all(loaded.buffer.rope(), loaded.info.encoding) == bytes)
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

fn legacy(text: &str, enc: &'static encoding_rs::Encoding) -> Vec<u8> {
    let (bytes, _, bad) = enc.encode(text);
    assert!(!bad, "test text must fit {}", enc.name());
    bytes.into_owned()
}

#[test]
fn fixtures_round_trip() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "py") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        match round_trip(&bytes) {
            Some(ok) => {
                assert!(ok, "{} did not round trip", path.display());
                checked += 1;
            }
            None => eprintln!("{}: lossy, skipped", path.display()),
        }
    }
    assert!(checked >= 10, "only {checked} fixtures checked");
}

const BODIES: [&str; 12] = [
    "",
    "one",
    "one\n",
    "one\r\n",
    "one\r",
    "one\ntwo\nthree",
    "one\r\ntwo\r\nthree\r\n",
    "one\rtwo\rthree\r",
    "one\r\ntwo\nthree\rfour\n\n\r\n",
    "\n\n\n",
    "\r\n\r\n",
    "a\u{0}b\r\nc\u{0}\n",
];

/// Each body in each encoding it can be generated in. UTF encodings must always be exact;
/// a legacy page whose guess makes the load lossy is reported, not failed.
#[test]
fn generated_round_trips() {
    let mut skipped: Vec<String> = Vec::new();
    let mut count = 0;
    let mut check = |name: &str, bytes: Vec<u8>, must_be_exact: bool| match round_trip(&bytes) {
        Some(ok) => {
            assert!(ok, "{name}: not byte-exact for {bytes:?}");
            count += 1;
        }
        None => {
            assert!(!must_be_exact, "{name}: unexpectedly lossy for {bytes:?}");
            skipped.push(format!("{name}: {bytes:?}"));
        }
    };
    let samples: [(&str, &'static encoding_rs::Encoding, &str); 4] = [
        (
            "windows-1254",
            encoding_rs::WINDOWS_1254,
            "Şişli çğüöı İĞÜÖ",
        ),
        (
            "windows-1252",
            encoding_rs::WINDOWS_1252,
            "café Müller ñandú",
        ),
        ("shift_jis", encoding_rs::SHIFT_JIS, "日本語のテキスト"),
        ("gbk", encoding_rs::GBK, "中文文本测试"),
    ];
    for body in BODIES {
        let mixed = |word: &str| body.replacen("one", word, 1).replace("two", word);
        check("ascii", body.as_bytes().to_vec(), true);
        let uni = mixed("héllo ünï 日本 😀");
        check("utf8", uni.as_bytes().to_vec(), true);
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice(uni.as_bytes());
        check("utf8-bom", bom, true);
        for le in [true, false] {
            let name = if le { "utf16le" } else { "utf16be" };
            check(name, utf16(&uni, le, true), true);
            check(name, utf16(body, le, true), true);
        }
        for (name, enc, word) in samples {
            check(name, legacy(&mixed(word), enc), false);
        }
    }
    check("bom only utf8", vec![0xEF, 0xBB, 0xBF], true);
    check("bom only le", vec![0xFF, 0xFE], true);
    check("bom only be", vec![0xFE, 0xFF], true);
    eprintln!(
        "{count} exact, {} lossy skipped: {skipped:#?}",
        skipped.len()
    );
}

#[test]
fn bom_only_files_write_just_the_bom() {
    for (enc, bom) in [
        (Encoding::Utf8Bom, &[0xEFu8, 0xBB, 0xBF][..]),
        (Encoding::Utf16Le, &[0xFF, 0xFE]),
        (Encoding::Utf16Be, &[0xFE, 0xFF]),
        (Encoding::Utf8, &[]),
    ] {
        assert_eq!(encode_all(&Rope::new(), enc), bom);
    }
}

#[test]
fn an_edit_changes_only_its_bytes() {
    // UTF-8: insert "XY" before "three"; the CRLF, LF and CR breaks stay as they are.
    let text = "ünö\r\ndös\nthree\rfour";
    let input = text.as_bytes().to_vec();
    let at = text.find("three").unwrap();
    let mut expected = input.clone();
    expected.splice(at..at, *b"XY");
    let loaded = load_bytes(&input, &opts());
    let mut rope = loaded.buffer.snapshot();
    rope.insert(rope.byte_to_char(at), "XY");
    assert_eq!(encode_all(&rope, loaded.info.encoding), expected);

    // UTF-16 LE with a surrogate pair elsewhere in the file.
    let text = "a😀b\r\nc\nd\re";
    let input = utf16(text, true, true);
    let loaded = load_bytes(&input, &opts());
    assert_eq!(loaded.info.encoding, Encoding::Utf16Le);
    let mut rope = loaded.buffer.snapshot();
    rope.insert(rope.byte_to_char(text.find('c').unwrap()), "Ω😀");
    let expected = utf16("a😀b\r\nΩ😀c\nd\re", true, true);
    assert_eq!(encode_all(&rope, Encoding::Utf16Le), expected);

    // Windows-1254.
    let text = "Şişli\r\nçğü\nöı\rİ";
    let input = legacy(text, encoding_rs::WINDOWS_1254);
    let loaded = load_bytes(&input, &opts());
    assert_eq!(
        loaded.info.encoding,
        Encoding::Legacy(encoding_rs::WINDOWS_1254)
    );
    let mut rope = loaded.buffer.snapshot();
    rope.insert(rope.byte_to_char(text.find('ö').unwrap()), "ğ");
    let expected = legacy("Şişli\r\nçğü\nğöı\rİ", encoding_rs::WINDOWS_1254);
    assert_eq!(encode_all(&rope, loaded.info.encoding), expected);
}

#[test]
fn unencodable_reports_character_line_and_column() {
    let enc = Encoding::Legacy(encoding_rs::WINDOWS_1254);
    let text = "Şişli\r\nçğü\nöı世x\n";
    let rope = Rope::from_str(text);
    assert_eq!(
        first_unencodable(&rope, enc),
        Some(('世', text.find('世').unwrap()))
    );
    match encode_to(&rope, enc, &mut Vec::new(), &no_cancel()) {
        Err(SaveError::Unencodable { ch, line, col }) => assert_eq!((ch, line, col), ('世', 2, 2)),
        other => panic!("{other:?}"),
    }
    assert_eq!(first_unencodable(&rope, Encoding::Utf8), None);
    assert_eq!(first_unencodable(&Rope::from_str("Şişli"), enc), None);

    let dir = TempDir::new("unencodable");
    let path = dir.path("a.txt");
    std::fs::write(&path, b"old").unwrap();
    let err = save_path(&path, &rope, enc, None, &no_cancel()).unwrap_err();
    assert!(matches!(err, SaveError::Unencodable { .. }));
    assert!(err.to_string().contains("line 3, column 3"));
    assert_eq!(std::fs::read(&path).unwrap(), b"old");
    assert_eq!(dir.files(), vec!["a.txt".to_string()]);
}

#[test]
fn stateful_encoder_is_flushed() {
    let rope = Rope::from_str("日本語");
    let out = encode_all(&rope, Encoding::Legacy(encoding_rs::ISO_2022_JP));
    // The text ends back in ASCII: ESC ( B.
    assert!(out.ends_with(&[0x1B, b'(', b'B']), "{out:?}");
}

#[test]
fn long_text_in_legacy_streams_across_chunks() {
    let text = "Şişli çğüöı\r\n".repeat(50_000);
    let rope = Rope::from_str(&text);
    let enc = Encoding::Legacy(encoding_rs::WINDOWS_1254);
    assert_eq!(
        encode_all(&rope, enc),
        legacy(&text, encoding_rs::WINDOWS_1254)
    );
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "mdedit-save-{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // A read-only file blocks removal on Windows.
        if let Ok(entries) = std::fs::read_dir(&self.0) {
            for e in entries.flatten() {
                if let Ok(meta) = e.metadata() {
                    let mut p = meta.permissions();
                    #[allow(clippy::permissions_set_readonly_false)]
                    p.set_readonly(false);
                    let _ = std::fs::set_permissions(e.path(), p);
                }
            }
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn new_file_and_replace() {
    let dir = TempDir::new("replace");
    let path = dir.path("a.txt");
    let rope = Rope::from_str("one\r\ntwo");
    let saved = save_path(
        &path,
        &rope,
        Encoding::Utf8Bom,
        Some(&FileStamp::Missing),
        &no_cancel(),
    )
    .unwrap();
    let expected = b"\xEF\xBB\xBFone\r\ntwo";
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert_eq!(saved.bytes, expected.len() as u64);
    assert_eq!(saved.stamp, stamp(&path));

    let rope = Rope::from_str("three");
    let saved = save_path(
        &path,
        &rope,
        Encoding::Utf8,
        Some(&saved.stamp),
        &no_cancel(),
    )
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"three");
    assert_eq!(saved.bytes, 5);
    assert_eq!(dir.files(), vec!["a.txt".to_string()]);
}

#[test]
fn changed_on_disk_is_refused() {
    let dir = TempDir::new("changed");
    let path = dir.path("a.txt");
    std::fs::write(&path, b"first").unwrap();
    let seen = stamp(&path);
    let rope = Rope::from_str("mine");

    // Modified after the stamp (a different size, so the stamp differs on any clock).
    std::fs::write(&path, b"someone else's text").unwrap();
    let err = save_path(&path, &rope, Encoding::Utf8, Some(&seen), &no_cancel()).unwrap_err();
    assert!(matches!(err, SaveError::ChangedOnDisk));
    assert_eq!(std::fs::read(&path).unwrap(), b"someone else's text");

    // Deleted after the stamp.
    let seen = stamp(&path);
    std::fs::remove_file(&path).unwrap();
    let err = save_path(&path, &rope, Encoding::Utf8, Some(&seen), &no_cancel()).unwrap_err();
    assert!(matches!(err, SaveError::ChangedOnDisk));
    assert!(!path.exists());

    // Missing at load and still missing is fine; so is no check at all.
    save_path(
        &path,
        &rope,
        Encoding::Utf8,
        Some(&FileStamp::Missing),
        &no_cancel(),
    )
    .unwrap();
    std::fs::write(&path, b"other").unwrap();
    save_path(
        &path,
        &Rope::from_str("forced"),
        Encoding::Utf8,
        None,
        &no_cancel(),
    )
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"forced");
    assert_eq!(dir.files(), vec!["a.txt".to_string()]);
}

#[test]
fn read_only_target_is_refused_and_unchanged() {
    let dir = TempDir::new("readonly");
    let path = dir.path("a.txt");
    std::fs::write(&path, b"keep").unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&path, perms).unwrap();
    let err = save_path(
        &path,
        &Rope::from_str("new"),
        Encoding::Utf8,
        None,
        &no_cancel(),
    )
    .unwrap_err();
    assert!(matches!(err, SaveError::ReadOnly));
    assert_eq!(std::fs::read(&path).unwrap(), b"keep");
    assert_eq!(dir.files(), vec!["a.txt".to_string()]);
}

#[test]
fn cancel_leaves_the_target_untouched() {
    let dir = TempDir::new("cancel");
    let path = dir.path("a.txt");
    std::fs::write(&path, b"keep").unwrap();
    let cancel = AtomicBool::new(true);
    let err = save_path(&path, &Rope::from_str("new"), Encoding::Utf8, None, &cancel).unwrap_err();
    assert!(matches!(err, SaveError::Cancelled));
    assert_eq!(std::fs::read(&path).unwrap(), b"keep");
    assert_eq!(dir.files(), vec!["a.txt".to_string()]);
}

#[test]
fn cancel_during_a_large_write_is_noticed() {
    // Raises the flag from inside the writer after the first 100 KB.
    struct Trip<'a>(&'a AtomicBool, usize);
    impl Write for Trip<'_> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.1 += buf.len();
            if self.1 > 100_000 {
                self.0.store(true, Ordering::Relaxed);
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let rope = Rope::from_str(&"x".repeat(10_000_000));
    for enc in [
        Encoding::Utf8,
        Encoding::Utf16Le,
        Encoding::Legacy(encoding_rs::WINDOWS_1252),
    ] {
        let cancel = AtomicBool::new(false);
        let mut trip = Trip(&cancel, 0);
        let err = encode_to(&rope, enc, &mut trip, &cancel).unwrap_err();
        assert!(matches!(err, SaveError::Cancelled), "{enc:?}");
        assert!(trip.1 < 1_000_000, "{enc:?} wrote {}", trip.1);
    }
}

#[cfg(windows)]
#[test]
fn attributes_and_creation_time_survive_a_save() {
    use std::os::windows::fs::MetadataExt;
    let dir = TempDir::new("attrs");
    let path = dir.path("a.txt");
    std::fs::write(&path, b"old").unwrap();
    let status = std::process::Command::new("attrib")
        .arg("+h")
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    let before = std::fs::metadata(&path).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    save_path(
        &path,
        &Rope::from_str("new text"),
        Encoding::Utf8,
        None,
        &no_cancel(),
    )
    .unwrap();
    let after = std::fs::metadata(&path).unwrap();
    const HIDDEN: u32 = 0x2;
    assert_ne!(after.file_attributes() & HIDDEN, 0, "hidden attribute lost");
    assert_eq!(before.created().unwrap(), after.created().unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), b"new text");
    assert_eq!(dir.files(), vec!["a.txt".to_string()]);
}

#[test]
#[ignore = "writes 100 MB and 200 MB; run with --ignored --nocapture"]
fn timing_100_mb() {
    let line = "The quick brown fox jumps over the lazy dog, again and again.\r\n";
    let text = line.repeat(100 * 1024 * 1024 / line.len());
    let rope = Rope::from_str(&text);
    drop(text);
    let dir = TempDir::new("timing");
    for enc in [Encoding::Utf8, Encoding::Utf16Le] {
        let path = dir.path("big.txt");
        let start = Instant::now();
        let saved = save_path(&path, &rope, enc, None, &no_cancel()).unwrap();
        eprintln!(
            "{}: {} bytes in {:.0} ms",
            enc.label(),
            saved.bytes,
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}
