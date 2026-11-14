use super::*;
use crate::find::FindOptions;
use std::fs;
use std::sync::atomic::AtomicU64;
use std::time::Instant;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "mdedit-replace-{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.path(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, bytes).unwrap();
        p
    }

    fn read(&self, name: &str) -> Vec<u8> {
        fs::read(self.path(name)).unwrap()
    }

    /// Names of the files directly in the folder, sorted.
    fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Ok(entries) = fs::read_dir(&self.0) {
            for e in entries.flatten() {
                if let Ok(m) = e.metadata() {
                    let mut p = m.permissions();
                    #[allow(clippy::permissions_set_readonly_false)]
                    p.set_readonly(false);
                    let _ = fs::set_permissions(e.path(), p);
                }
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn no() -> AtomicBool {
    AtomicBool::new(false)
}

fn lit(text: &str) -> Query {
    Query::new(
        text,
        FindOptions {
            match_case: true,
            ..Default::default()
        },
    )
    .unwrap()
}

fn re(text: &str) -> Query {
    Query::new(
        text,
        FindOptions {
            match_case: true,
            regex: true,
            ..Default::default()
        },
    )
    .unwrap()
}

fn opts() -> PlanOptions {
    PlanOptions {
        load: LoadOptions {
            legacy: encoding_rs::WINDOWS_1254,
            ..LoadOptions::default()
        },
        ..PlanOptions::default()
    }
}

fn plan_tree(dir: &TempDir, q: &Query, repl: &str) -> Plan {
    plan(
        Source::Tree(&dir.0),
        q,
        repl,
        &opts(),
        &[],
        &no(),
        &|_, _| {},
    )
}

fn run(plan: &Plan) -> Report {
    apply(
        plan,
        &plan.all(),
        &ApplyOptions::default(),
        &no(),
        &|_, _| {},
    )
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

fn names(list: &[(PathBuf, String)]) -> Vec<String> {
    list.iter()
        .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn literal_over_a_tree_in_find_in_files_order() {
    let d = TempDir::new("literal");
    d.write("b.txt", b"one needle two needle\nthree\n");
    d.write("a.txt", b"needle\n");
    d.write("sub/c.txt", b"no match here\nneedle at the end");
    d.write("sub/none.txt", b"nothing\n");
    d.write(".hidden/x.txt", b"needle\n");
    d.write(".git/config", b"needle\n");
    d.write("node_modules/m.js", b"needle\n");
    d.write("target/t.txt", b"needle\n");
    let p = plan_tree(&d, &lit("needle"), "pin");
    let found: Vec<PathBuf> = p
        .files
        .iter()
        .map(|f| f.path.strip_prefix(&d.0).unwrap().to_path_buf())
        .collect();
    let expected: Vec<PathBuf> = ["a.txt", "b.txt", "sub/c.txt"]
        .iter()
        .map(PathBuf::from)
        .collect();
    assert_eq!(found, expected);
    assert_eq!(p.totals.files, 3);
    assert_eq!(p.totals.matches, 4);
    assert_eq!(p.totals.searched, 4);
    assert!(p.skipped.is_empty() && !p.truncated && !p.cancelled);
    assert_eq!(p.files[1].matches, 2);
    assert_eq!(p.files[1].bytes, 28);
    // Planning wrote nothing.
    assert_eq!(d.read("a.txt"), b"needle\n");
    let r = run(&p);
    assert_eq!(r.changed.len(), 3);
    assert_eq!(r.replacements, 4);
    assert!(r.skipped.is_empty() && r.failures.is_empty() && !r.cancelled);
    assert_eq!(d.read("a.txt"), b"pin\n");
    assert_eq!(d.read("b.txt"), b"one pin two pin\nthree\n");
    assert_eq!(d.read("sub/c.txt"), b"no match here\npin at the end");
    assert_eq!(d.read(".hidden/x.txt"), b"needle\n");
    assert_eq!(d.read(".git/config"), b"needle\n");
    assert_eq!(d.read("node_modules/m.js"), b"needle\n");
    assert_eq!(d.read("target/t.txt"), b"needle\n");
    // No temporary file is left behind.
    assert_eq!(
        d.names().iter().filter(|n| n.contains("~mdedit")).count(),
        0
    );
}

#[test]
fn single_file_and_file_list_sources() {
    let d = TempDir::new("sources");
    let a = d.write("a.txt", b"x1 x2\n");
    let b = d.write("b.txt", b"x3\n");
    let p = plan(
        Source::Tree(&a),
        &lit("x"),
        "y",
        &opts(),
        &[],
        &no(),
        &|_, _| {},
    );
    assert_eq!(p.totals.matches, 2);
    let list = vec![b.clone(), a.clone(), b.clone()];
    let p = plan(
        Source::Files(&list),
        &lit("x"),
        "y",
        &opts(),
        &[],
        &no(),
        &|_, _| {},
    );
    let order: Vec<_> = p.files.iter().map(|f| f.path.clone()).collect();
    assert_eq!(order, vec![b, a]);
    // A missing file is skipped with a reason; a folder that is not there gives an empty plan.
    let ghost = vec![d.path("ghost.txt")];
    let p = plan(
        Source::Files(&ghost),
        &lit("x"),
        "y",
        &opts(),
        &[],
        &no(),
        &|_, _| {},
    );
    assert_eq!((p.files.len(), p.skipped.len()), (0, 1));
    let p = plan(
        Source::Tree(&d.path("nowhere")),
        &lit("x"),
        "y",
        &opts(),
        &[],
        &no(),
        &|_, _| {},
    );
    assert!(p.files.is_empty() && p.skipped.is_empty());
}

#[test]
fn regex_with_captures_and_names() {
    let d = TempDir::new("regex");
    d.write(
        "a.txt",
        b"mail ann@example and bob@site\nSmith, John\nprice 5 USD\n",
    );
    let p = plan_tree(&d, &re(r"(\w+)@(\w+)"), "$2 at $1");
    assert_eq!(p.files[0].matches, 2);
    run(&p);
    assert_eq!(
        d.read("a.txt"),
        b"mail example at ann and site at bob\nSmith, John\nprice 5 USD\n"
    );
    let p = plan_tree(
        &d,
        &re(r"(?P<last>\w+), (?P<first>\w+)"),
        "${first} ${last}$$",
    );
    run(&p);
    assert!(d.read("a.txt").windows(11).any(|w| w == b"John Smith$"));
    // A group that is not there is empty; $0 is the whole match.
    let p = plan_tree(&d, &re(r"price (\d)"), "[$0|$7|$1]");
    run(&p);
    assert!(d.read("a.txt").windows(14).any(|w| w == b"[price 5||5] U"));
}

#[test]
fn whole_word_and_case() {
    let d = TempDir::new("word");
    d.write("a.txt", b"cat concat Cat cat.\n");
    let ww = Query::new(
        "cat",
        FindOptions {
            whole_word: true,
            match_case: true,
            ..Default::default()
        },
    )
    .unwrap();
    let p = plan_tree(&d, &ww, "dog");
    assert_eq!(p.totals.matches, 2);
    run(&p);
    assert_eq!(d.read("a.txt"), b"dog concat Cat dog.\n");
    let any_case = Query::new("CAT", FindOptions::default()).unwrap();
    let p = plan_tree(&d, &any_case, "x");
    assert_eq!(p.totals.matches, 2);
    run(&p);
    assert_eq!(d.read("a.txt"), b"dog conx x dog.\n");
}

/// The bytes of `text` in `enc` (with the mark the encoding has).
fn encode(text: &str, enc: Encoding) -> Vec<u8> {
    let mut out = Vec::new();
    let rope = ropey::Rope::from_str(text);
    crate::save::encode_to(&rope, enc, &mut out, &no()).unwrap();
    out
}

#[test]
fn every_encoding_and_line_ending_comes_back_byte_exact() {
    let eols = ["\n", "\r\n", "\r"];
    for eol in eols {
        let src =
            format!("first line{eol}the needle is here, needle again{eol}last{eol}end needle");
        let want = src.replace("needle", "NAIL");
        let cases: Vec<(&str, Vec<u8>, Vec<u8>)> = vec![
            ("utf8", src.clone().into_bytes(), want.clone().into_bytes()),
            (
                "utf8bom",
                [&[0xEF, 0xBB, 0xBF][..], src.as_bytes()].concat(),
                [&[0xEF, 0xBB, 0xBF][..], want.as_bytes()].concat(),
            ),
            ("utf16le", utf16(&src, true, true), utf16(&want, true, true)),
            (
                "utf16be",
                utf16(&src, false, true),
                utf16(&want, false, true),
            ),
        ];
        for (name, before, after) in cases {
            let d = TempDir::new("enc");
            d.write("f.txt", &before);
            let p = plan_tree(&d, &lit("needle"), "NAIL");
            assert_eq!(p.files.len(), 1, "{name} {eol:?}");
            assert_eq!(p.files[0].matches, 3);
            let r = run(&p);
            assert!(r.failures.is_empty() && r.skipped.is_empty(), "{:?}", r);
            assert_eq!(d.read("f.txt"), after, "{name} {eol:?}");
        }
    }
}

#[test]
fn utf8_with_non_ascii_text_and_a_final_newline_state() {
    let d = TempDir::new("utf8");
    let src = "Größe: Straße — 日本語 needle 🙂\nnext";
    d.write("f.txt", src.as_bytes());
    d.write("g.txt", b"needle\n");
    d.write("h.txt", b"needle");
    run(&plan_tree(&d, &lit("needle"), "Nadel ✓"));
    assert_eq!(d.read("f.txt"), src.replace("needle", "Nadel ✓").as_bytes());
    assert_eq!(d.read("g.txt"), "Nadel ✓\n".as_bytes());
    assert_eq!(d.read("h.txt"), "Nadel ✓".as_bytes());
}

#[test]
fn legacy_code_page_is_kept() {
    let d = TempDir::new("legacy");
    let enc = Encoding::Legacy(encoding_rs::WINDOWS_1254);
    for eol in ["\r\n", "\n"] {
        let src = format!("İstanbul güzel şehir needle ığ{eol}çok needle iyi ÖĞRENCİ{eol}");
        let want = src.replace("needle", "iğne");
        d.write("t.txt", &encode(&src, enc));
        let p = plan_tree(&d, &lit("needle"), "iğne");
        assert_eq!(p.files.len(), 1);
        assert!(matches!(p.files[0].encoding, Encoding::Legacy(_)));
        let r = run(&p);
        assert!(r.failures.is_empty(), "{:?}", r.failures);
        assert_eq!(d.read("t.txt"), encode(&want, p.files[0].encoding));
        // No byte outside the replaced text moved.
        let before = encode(&src, enc);
        let after = d.read("t.txt");
        let n = before
            .iter()
            .zip(&after)
            .take_while(|(a, b)| a == b)
            .count();
        assert!(n > 10, "the text before the first match is untouched");
        assert_eq!(&before[..n], &after[..n]);
    }
}

#[test]
fn mixed_line_endings_stay_and_new_breaks_follow_the_majority() {
    let d = TempDir::new("mixed");
    d.write("m.txt", b"a needle\r\nb\nc needle\r\nd\r\n");
    let p = plan_tree(&d, &lit("needle"), "x\ny");
    assert_eq!(p.files[0].eol.style(), crate::LineEnding::Mixed);
    run(&p);
    assert_eq!(d.read("m.txt"), b"a x\r\ny\r\nb\nc x\r\ny\r\nd\r\n");
}

#[test]
fn empty_replacement_deletes() {
    let d = TempDir::new("delete");
    d.write("a.txt", b"foo-bar-foo\n");
    run(&plan_tree(&d, &lit("foo"), ""));
    assert_eq!(d.read("a.txt"), b"-bar-\n");
}

#[test]
fn changed_between_plan_and_apply_is_skipped() {
    let d = TempDir::new("changed");
    let a = d.write("a.txt", b"needle one\n");
    d.write("b.txt", b"needle two\n");
    d.write("c.txt", b"needle three\n");
    let p = plan_tree(&d, &lit("needle"), "N");
    // Same length, different bytes (and the same stamp size): the hash tells.
    fs::write(&a, b"noodle one\n").unwrap();
    // Longer.
    d.write("b.txt", b"needle two\nmore\n");
    let r = run(&p);
    assert_eq!(names(&r.skipped), ["a.txt", "b.txt"]);
    assert!(r.skipped.iter().all(|(_, why)| why.contains("changed")));
    assert_eq!(r.changed.len(), 1);
    assert_eq!(d.read("a.txt"), b"noodle one\n");
    assert_eq!(d.read("b.txt"), b"needle two\nmore\n");
    assert_eq!(d.read("c.txt"), b"N three\n");
    // A file that is gone is skipped too, not a crash.
    let p = plan_tree(&d, &lit("noodle"), "N");
    fs::remove_file(d.path("a.txt")).unwrap();
    let r = run(&p);
    assert_eq!(r.changed.len(), 0);
    assert_eq!(r.skipped.len() + r.failures.len(), 1);
}

#[test]
fn a_read_only_file_fails_alone() {
    let d = TempDir::new("readonly");
    d.write("a.txt", b"needle a\n");
    let ro = d.write("b.txt", b"needle b\n");
    d.write("c.txt", b"needle c\n");
    let p = plan_tree(&d, &lit("needle"), "N");
    let mut perm = fs::metadata(&ro).unwrap().permissions();
    perm.set_readonly(true);
    fs::set_permissions(&ro, perm).unwrap();
    let r = apply(
        &p,
        &p.all(),
        &ApplyOptions {
            backup: true,
            ..ApplyOptions::default()
        },
        &no(),
        &|_, _| {},
    );
    assert_eq!(names(&r.failures), ["b.txt"]);
    assert!(r.failures[0].1.contains("read-only"));
    assert_eq!(r.changed.len(), 2);
    assert_eq!(d.read("a.txt"), b"N a\n");
    assert_eq!(d.read("b.txt"), b"needle b\n");
    assert_eq!(d.read("c.txt"), b"N c\n");
    // The failed file left no backup and no temporary file.
    assert!(!d.path("b.txt.bak").exists());
    assert!(d.names().iter().all(|n| !n.contains("~mdedit")));
}

#[test]
fn binary_oversized_unreadable_and_lossy_files_are_skipped_in_the_plan() {
    let d = TempDir::new("skips");
    d.write("ok.txt", b"needle\n");
    d.write("bin.dat", b"needle\0\0more needle");
    d.write("big.txt", &b"needle ".repeat(1000));
    // Valid UTF-8 text with one stray byte: the loader reads it with U+FFFD, so it could not
    // be written back unchanged.
    let mut lossy = "needle ü ö ä ß é è ê ë ".repeat(10).into_bytes();
    lossy.push(0xFF);
    d.write("lossy.txt", &lossy);
    let o = PlanOptions {
        max_file_bytes: 4096,
        ..opts()
    };
    let p = plan(
        Source::Tree(&d.0),
        &lit("needle"),
        "N",
        &o,
        &[],
        &no(),
        &|_, _| {},
    );
    assert_eq!(p.files.len(), 1);
    assert_eq!(names(&p.skipped), ["big.txt", "bin.dat", "lossy.txt"]);
    assert_eq!(p.skipped_count, 3);
    assert!(p.skipped[0].1.contains("larger"));
    assert!(p.skipped[1].1.contains("binary"));
    assert!(p.skipped[2].1.contains("unchanged"));
    let r = run(&p);
    assert_eq!(r.changed.len(), 1);
    assert_eq!(d.read("bin.dat"), b"needle\0\0more needle");
    assert_eq!(d.read("lossy.txt"), lossy);
}

#[test]
fn files_open_with_unsaved_changes_are_left_out() {
    let d = TempDir::new("dirty");
    let a = d.write("a.txt", b"needle\n");
    let b = d.write("b.txt", b"needle\n");
    // In the plan.
    let p = plan(
        Source::Tree(&d.0),
        &lit("needle"),
        "N",
        &opts(),
        std::slice::from_ref(&a),
        &no(),
        &|_, _| {},
    );
    assert_eq!(names(&p.skipped), ["a.txt"]);
    assert!(p.skipped[0].1.contains("unsaved"));
    assert_eq!(p.files.len(), 1);
    // And again in apply, for a document that was edited after the plan was made.
    let p = plan_tree(&d, &lit("needle"), "N");
    let r = apply(
        &p,
        &p.all(),
        &ApplyOptions {
            skip: std::slice::from_ref(&b),
            ..ApplyOptions::default()
        },
        &no(),
        &|_, _| {},
    );
    assert_eq!(names(&r.skipped), ["b.txt"]);
    assert_eq!(d.read("a.txt"), b"N\n");
    assert_eq!(d.read("b.txt"), b"needle\n");
    // A different spelling of the same path is the same file.
    let alias = d.0.join("sub/../b.txt");
    fs::create_dir_all(d.path("sub")).unwrap();
    let p = plan_tree(&d, &lit("needle"), "N");
    let r = apply(
        &p,
        &p.all(),
        &ApplyOptions {
            skip: &[alias],
            ..ApplyOptions::default()
        },
        &no(),
        &|_, _| {},
    );
    assert_eq!(names(&r.skipped), ["b.txt"]);
}

#[test]
fn only_the_chosen_entries_are_applied() {
    let d = TempDir::new("chosen");
    for n in ["a", "b", "c", "d"] {
        d.write(&format!("{n}.txt"), b"needle\n");
    }
    let p = plan_tree(&d, &lit("needle"), "N");
    let r = apply(
        &p,
        &[2, 0, 0, 99],
        &ApplyOptions::default(),
        &no(),
        &|_, _| {},
    );
    assert_eq!(r.changed.len(), 2);
    assert_eq!(d.read("a.txt"), b"N\n");
    assert_eq!(d.read("b.txt"), b"needle\n");
    assert_eq!(d.read("c.txt"), b"N\n");
    assert_eq!(d.read("d.txt"), b"needle\n");
}

#[test]
fn cancel_between_files_leaves_written_files_whole() {
    let d = TempDir::new("cancel");
    for i in 0..6 {
        d.write(&format!("f{i}.txt"), format!("needle {i}\n").as_bytes());
    }
    let p = plan_tree(&d, &lit("needle"), "N");
    let stop = AtomicBool::new(false);
    let seen = std::cell::Cell::new(0);
    let r = apply(
        &p,
        &p.all(),
        &ApplyOptions::default(),
        &stop,
        &|done, total| {
            assert_eq!(total, 6);
            seen.set(done);
            if done == 2 {
                stop.store(true, Ordering::Relaxed);
            }
        },
    );
    assert!(r.cancelled);
    assert_eq!(r.changed.len(), 2);
    assert_eq!(seen.get(), 2);
    assert_eq!(d.read("f0.txt"), b"N 0\n");
    assert_eq!(d.read("f1.txt"), b"N 1\n");
    for i in 2..6 {
        assert_eq!(
            d.read(&format!("f{i}.txt")),
            format!("needle {i}\n").as_bytes()
        );
    }
    // What was done can be undone.
    let rr = revert(&r, &no(), &|_, _| {});
    assert_eq!(rr.restored.len(), 2);
    assert_eq!(d.read("f0.txt"), b"needle 0\n");
    // A plan cancelled part way says so and keeps the files it finished.
    let stop = AtomicBool::new(false);
    let p = plan(
        Source::Tree(&d.0),
        &lit("needle"),
        "N",
        &opts(),
        &[],
        &stop,
        &|done, _| {
            if done == 3 {
                stop.store(true, Ordering::Relaxed);
            }
        },
    );
    assert!(p.cancelled);
    assert_eq!(p.files.len(), 3);
    let yes = AtomicBool::new(true);
    let p = plan(
        Source::Tree(&d.0),
        &lit("needle"),
        "N",
        &opts(),
        &[],
        &yes,
        &|_, _| {},
    );
    assert!(p.cancelled && p.files.is_empty());
}

#[test]
fn revert_restores_bytes_exactly() {
    let d = TempDir::new("revert");
    let originals: Vec<(&str, Vec<u8>)> = vec![
        ("a.txt", "needle ü\r\nx\n".as_bytes().to_vec()),
        ("b.txt", utf16("needle\r\nlines\r\n", true, true)),
        ("c.txt", [&[0xEF, 0xBB, 0xBF][..], b"needle\n"].concat()),
        (
            "d.txt",
            encode(
                "İğne needle\r\n",
                Encoding::Legacy(encoding_rs::WINDOWS_1254),
            ),
        ),
    ];
    for (n, b) in &originals {
        d.write(n, b);
    }
    let p = plan_tree(&d, &lit("needle"), "a much longer replacement");
    assert_eq!(p.files.len(), 4);
    let r = run(&p);
    assert_eq!(r.changed.len(), 4);
    assert_eq!(r.revertible(), 4);
    for (n, b) in &originals {
        assert_ne!(&d.read(n), b);
    }
    // One file edited after the replacement is not touched by the revert.
    let mut edited = d.read("a.txt");
    edited.extend_from_slice(b"user edit");
    d.write("a.txt", &edited);
    let rr = revert(&r, &no(), &|_, _| {});
    assert_eq!(rr.restored.len(), 3);
    assert_eq!(names(&rr.skipped), ["a.txt"]);
    assert_eq!(d.read("a.txt"), edited);
    for (n, b) in originals.iter().skip(1) {
        assert_eq!(&d.read(n), b, "{n}");
    }
    // Reverting twice changes nothing more.
    let rr = revert(&r, &no(), &|_, _| {});
    assert!(rr.restored.is_empty());
}

#[test]
fn revert_from_the_backup_when_memory_was_not_kept() {
    let d = TempDir::new("revert-bak");
    d.write("a.txt", b"needle 1\n");
    d.write("b.txt", b"needle 2\n");
    let p = plan_tree(&d, &lit("needle"), "N");
    // No room in memory and no backup: nothing can be restored, and the report says so.
    let none = apply(
        &p,
        &[0],
        &ApplyOptions {
            keep_bytes: 0,
            ..ApplyOptions::default()
        },
        &no(),
        &|_, _| {},
    );
    assert_eq!(none.revertible(), 0);
    let rr = revert(&none, &no(), &|_, _| {});
    assert!(rr.restored.is_empty() && rr.skipped.len() == 1);
    assert_eq!(d.read("a.txt"), b"N 1\n");
    // With a backup it works.
    let r = apply(
        &p,
        &[1],
        &ApplyOptions {
            keep_bytes: 0,
            backup: true,
            ..ApplyOptions::default()
        },
        &no(),
        &|_, _| {},
    );
    assert_eq!(r.revertible(), 1);
    assert!(r.reverts[0].original.is_none());
    let rr = revert(&r, &no(), &|_, _| {});
    assert_eq!(rr.restored.len(), 1);
    assert_eq!(d.read("b.txt"), b"needle 2\n");
    // The backup stays, and a damaged backup is not trusted.
    assert_eq!(d.read("b.txt.bak"), b"needle 2\n");
    let r2 = apply(
        &plan_tree(&d, &lit("needle"), "N"),
        &[0],
        &ApplyOptions {
            keep_bytes: 0,
            backup: true,
            ..ApplyOptions::default()
        },
        &no(),
        &|_, _| {},
    );
    let bak = r2.changed[0].backup.clone().unwrap();
    fs::write(&bak, b"tampered").unwrap();
    let rr = revert(&r2, &no(), &|_, _| {});
    assert!(rr.restored.is_empty() && rr.skipped[0].1.contains("backup"));
}

#[test]
fn backup_names() {
    let d = TempDir::new("backup");
    d.write("a.rs", b"needle\n");
    let bak = ApplyOptions {
        backup: true,
        ..ApplyOptions::default()
    };
    let q = lit("needle");
    let again = |d: &TempDir| {
        // Put the needle back so the file can be replaced again.
        d.write("a.rs", b"needle\n");
        let p = plan_tree(d, &q, "N");
        apply(&p, &p.all(), &bak, &no(), &|_, _| {})
    };
    let r = apply(&plan_tree(&d, &q, "N"), &[0], &bak, &no(), &|_, _| {});
    assert_eq!(d.read("a.rs.bak"), b"needle\n");
    assert_eq!(r.changed[0].backup, Some(d.path("a.rs.bak")));
    assert_eq!(d.read("a.rs"), b"N\n");
    // A backup that is there is never overwritten: the next one is .bak1, then .bak2.
    d.write("a.rs.bak", b"precious");
    let r = again(&d);
    assert_eq!(r.changed[0].backup, Some(d.path("a.rs.bak1")));
    assert_eq!(d.read("a.rs.bak"), b"precious");
    let r = again(&d);
    assert_eq!(r.changed[0].backup, Some(d.path("a.rs.bak2")));
    // With every name taken the file is not changed.
    for n in 3..=REPLACE_BACKUPS {
        d.write(&format!("a.rs.bak{n}"), b"x");
    }
    d.write("a.rs", b"needle\n");
    let p = plan_tree(&d, &q, "N");
    let r = apply(&p, &p.all(), &bak, &no(), &|_, _| {});
    assert_eq!(r.failures.len(), 1);
    assert!(r.failures[0].1.contains("backup"));
    assert_eq!(d.read("a.rs"), b"needle\n");
}

#[test]
fn a_replacement_the_encoding_cannot_hold_is_refused() {
    let d = TempDir::new("unencodable");
    let enc = Encoding::Legacy(encoding_rs::WINDOWS_1254);
    let src = encode("Türkçe needle ışık güzel\r\n", enc);
    d.write("t.txt", &src);
    d.write("u.txt", "needle\n".as_bytes());
    let p = plan_tree(&d, &lit("needle"), "→ arrow");
    assert_eq!(p.files.len(), 2);
    let r = apply(
        &p,
        &p.all(),
        &ApplyOptions {
            backup: true,
            ..ApplyOptions::default()
        },
        &no(),
        &|_, _| {},
    );
    assert_eq!(names(&r.failures), ["t.txt"]);
    assert!(
        r.failures[0].1.contains("cannot be stored"),
        "{}",
        r.failures[0].1
    );
    assert_eq!(d.read("t.txt"), src);
    assert!(!d.path("t.txt.bak").exists());
    // The UTF-8 file took the arrow.
    assert_eq!(d.read("u.txt"), "→ arrow\n".as_bytes());
    assert!(d.names().iter().all(|n| !n.contains("~mdedit")));
}

#[test]
fn nothing_to_change_is_reported_not_written() {
    let d = TempDir::new("same");
    d.write("a.txt", b"needle\n");
    let p = plan_tree(&d, &lit("needle"), "needle");
    let stamp_before = stamp(&d.path("a.txt"));
    let r = run(&p);
    assert!(r.changed.is_empty());
    assert_eq!(r.skipped.len(), 1);
    assert_eq!(stamp(&d.path("a.txt")), stamp_before);
}

#[test]
fn preview_shows_before_and_after() {
    let d = TempDir::new("preview");
    let mut text = String::new();
    for i in 0..8 {
        text.push_str(&format!("line {i}: the needle\t is here\n"));
    }
    text.push_str(&format!("{}needle{}\n", "x".repeat(300), "y".repeat(300)));
    d.write("a.txt", text.as_bytes());
    let p = plan_tree(&d, &re(r"(\w+) (needle)"), "$2-$1");
    let f = &p.files[0];
    assert_eq!(f.matches, 8);
    assert_eq!(f.preview.len(), REPLACE_PREVIEW_LINES);
    assert_eq!(f.preview[0].line, 0);
    assert_eq!(f.preview[0].before, "line 0: the needle  is here");
    assert_eq!(f.preview[0].after, "line 0: needle-the  is here");
    assert_eq!(f.preview[4].line, 4);
    // A long line is cut around the match.
    let p = plan_tree(&d, &lit("xneedley"), "<>");
    let f = &p.files[0];
    assert_eq!(f.preview[0].line, 8);
    assert!(f.preview[0].before.len() < 200 && f.preview[0].before.contains("xneedley"));
    assert!(f.preview[0].after.contains("<>"));
    // A match across lines shows the break.
    let p = plan_tree(&d, &lit("here\nline 1"), "H");
    assert!(p.files[0].preview[0].before.contains('↵'));
}

#[test]
fn many_matches_and_capped_files() {
    let d = TempDir::new("many");
    let text = "a\n".repeat(REPLACE_MATCHES_PER_FILE + 500);
    d.write("a.txt", text.as_bytes());
    let p = plan_tree(&d, &lit("a"), "bb");
    assert!(p.files[0].capped);
    assert_eq!(p.files[0].matches, REPLACE_MATCHES_PER_FILE);
    let r = run(&p);
    assert_eq!(r.replacements, REPLACE_MATCHES_PER_FILE + 500);
    assert_eq!(
        d.read("a.txt"),
        "bb\n".repeat(REPLACE_MATCHES_PER_FILE + 500).as_bytes()
    );
}

#[test]
fn limits_truncate_the_plan() {
    let d = TempDir::new("limits");
    for i in 0..10 {
        d.write(&format!("f{i}.txt"), b"needle\n");
    }
    let o = PlanOptions {
        max_files: 4,
        ..opts()
    };
    let p = plan(
        Source::Tree(&d.0),
        &lit("needle"),
        "N",
        &o,
        &[],
        &no(),
        &|_, _| {},
    );
    assert_eq!((p.files.len(), p.truncated), (4, true));
    let o = PlanOptions {
        max_bytes: 20,
        ..opts()
    };
    let p = plan(
        Source::Tree(&d.0),
        &lit("needle"),
        "N",
        &o,
        &[],
        &no(),
        &|_, _| {},
    );
    assert_eq!((p.files.len(), p.truncated), (3, true));
}

#[test]
fn progress_counts_files() {
    let d = TempDir::new("progress");
    for i in 0..5 {
        d.write(
            &format!("f{i}.txt"),
            if i % 2 == 0 { b"needle" } else { b"none" },
        );
    }
    let seen = std::cell::RefCell::new(Vec::new());
    let p = plan(
        Source::Tree(&d.0),
        &lit("needle"),
        "N",
        &opts(),
        &[],
        &no(),
        &|a, b| {
            seen.borrow_mut().push((a, b));
        },
    );
    assert_eq!(*seen.borrow(), (1..=5).map(|i| (i, 5)).collect::<Vec<_>>());
    let seen = std::cell::RefCell::new(Vec::new());
    apply(&p, &p.all(), &ApplyOptions::default(), &no(), &|a, b| {
        seen.borrow_mut().push((a, b));
    });
    assert_eq!(*seen.borrow(), vec![(1, 3), (2, 3), (3, 3)]);
}

/// `cargo test --release -p mdedit-core replace_perf -- --ignored --nocapture`
#[test]
#[ignore]
fn replace_perf() {
    let base = std::env::var("MDEDIT_PERF_DIR").map_or_else(
        |_| std::env::temp_dir().join("mdedit-replace-perf"),
        PathBuf::from,
    );
    let _ = fs::remove_dir_all(&base);
    let files = 5000;
    for i in 0..files {
        let dir = base.join(format!("d{}", i % 50));
        fs::create_dir_all(&dir).unwrap();
        let mut text = String::new();
        for l in 0..60 {
            text.push_str(&format!("fn item_{i}_{l}() {{ let old_name = {l}; }}\r\n"));
        }
        fs::write(dir.join(format!("f{i}.rs")), text).unwrap();
    }
    let q = lit("old_name");
    let t = Instant::now();
    let p = plan(
        Source::Tree(&base),
        &q,
        "new_name",
        &opts(),
        &[],
        &no(),
        &|_, _| {},
    );
    let t_plan = t.elapsed();
    let t = Instant::now();
    let r = apply(&p, &p.all(), &ApplyOptions::default(), &no(), &|_, _| {});
    let t_apply = t.elapsed();
    let t = Instant::now();
    let rr = revert(&r, &no(), &|_, _| {});
    let t_revert = t.elapsed();
    eprintln!(
        "{files} files, {} matches, {} KiB: plan {t_plan:?}, apply {t_apply:?} ({} changed, {} failures), revert {t_revert:?} ({} restored)",
        p.totals.matches,
        p.totals.bytes / 1024,
        r.changed.len(),
        r.failures.len(),
        rr.restored.len()
    );
    let _ = fs::remove_dir_all(&base);
}
