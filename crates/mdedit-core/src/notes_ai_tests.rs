use super::*;

fn note(body: &str) -> String {
    serde_like(&[("Trip", body)])
}

/// An arrange answer with these titles and bodies, as JSON.
fn serde_like(notes: &[(&str, &str)]) -> String {
    let esc = |s: &str| {
        let mut o = String::new();
        for c in s.chars() {
            match c {
                '"' => o.push_str("\\\""),
                '\\' => o.push_str("\\\\"),
                '\n' => o.push_str("\\n"),
                '\r' => o.push_str("\\r"),
                '\t' => o.push_str("\\t"),
                c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
                c => o.push(c),
            }
        }
        o
    };
    let list: Vec<String> = notes
        .iter()
        .map(|(t, b)| format!(r#"{{"title":"{}","body":"{}"}}"#, esc(t), esc(b)))
        .collect();
    format!(r#"{{"version":1,"notes":[{}]}}"#, list.join(","))
}

fn body_of(answer: &str, sent: &[usize]) -> Vec<Part> {
    read_arrange(answer, sent).expect("read").notes[0]
        .body
        .clone()
}

fn text_of(parts: &[Part]) -> String {
    parts
        .iter()
        .map(|p| match p {
            Part::Text(t) => t.clone(),
            Part::Picture { id, alt } => format!("<<{id}:{alt}>>"),
        })
        .collect()
}

fn names(id: usize) -> Option<String> {
    Some(format!("p{id}.png"))
}

/// Check 1: the envelope. No object, an object that does not parse, a version other than 1 and
/// an answer past the cap are each refused; words around the object are allowed.
#[test]
fn check_1_the_envelope() {
    assert_eq!(read_look("no json here", &[0]), Err(Fault::NoObject));
    assert_eq!(read_look("{\"pictures\": [}", &[0]), Err(Fault::NotJson));
    assert_eq!(
        read_look(r#"{"version":2,"pictures":[]}"#, &[0]),
        Err(Fault::Version)
    );
    assert_eq!(read_look(r#"{"version":1}"#, &[0]), Err(Fault::NoPictures));
    assert_eq!(read_arrange(r#"{"version":1}"#, &[0]), Err(Fault::NoNotes));
    let long = format!("{{\"version\":1,\"notes\":[]}}{}", " ".repeat(ANSWER_MAX));
    assert_eq!(read_arrange(&long, &[]), Err(Fault::TooLong));
    let r = read_look(
        "Here it is:\n```json\n{\"pictures\":[{\"id\":0,\"title\":\"A\"}]}\n```",
        &[0],
    )
    .expect("read");
    assert_eq!(r[0].title, "A");
}

/// Check 2: a number that was not sent is ignored, and the first answer for a number wins; a
/// `picture:` number that was not sent places nothing.
#[test]
fn check_2_numbers_that_were_not_sent() {
    let r = read_look(
        r#"{"pictures":[{"id":5,"title":"Not sent"},{"id":1,"title":"First"},{"id":1,"title":"Second"},{"id":-1},{"id":1.5}]}"#,
        &[0, 1],
    )
    .expect("read");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].title, "First");
    let b = body_of(&note("A ![x](picture:7) B ![y](picture:1)"), &[0, 1]);
    assert_eq!(text_of(&b), "A  B <<1:y>>");
}

/// Check 3: a title is one line of at most 120 characters with no brackets; sections are three
/// levels of at most 64 characters each, made folder names; the file name is the library's rule.
#[test]
fn check_3_titles_sections_and_names() {
    assert_eq!(clean_title("Sign-in [fails]\nsecond line"), "Sign-in fails");
    assert_eq!(clean_title(&"é".repeat(300)).chars().count(), TITLE_MAX);
    assert_eq!(clean_title("Notes ##"), "Notes");
    let s = clean_sections(&[
        "Work".into(),
        "".into(),
        "A/B:C".into(),
        "x".repeat(100),
        "Fourth".into(),
    ]);
    assert_eq!(s.len(), 3);
    assert_eq!(s[0], "Work");
    assert_eq!(s[1], "A_B_C");
    assert_eq!(s[2].chars().count(), SECTION_MAX);
    let a = read_arrange(
        r#"{"notes":[{"title":"Pasta Notes","sections":["Food","..",""],"body":"x"}]}"#,
        &[],
    )
    .expect("read");
    assert_eq!(a.notes[0].file_name(), "pasta-notes.md");
    assert_eq!(a.notes[0].sections, vec!["Food".to_string()]);
}

/// Check 4: what a picture shows, its subjects and its text are cut to their limits and marked;
/// a kind is one of five words or "other".
#[test]
fn check_4_a_picture_s_fields_are_cut_and_marked() {
    let says = "s".repeat(SAYS_MAX + 10);
    let text = "t".repeat(TEXT_MAX + 1);
    let answer = format!(
        r#"{{"pictures":[{{"id":0,"says":"{says}","text":"{text}","kind":"Screenshot","subjects":["a","{}","c","d",""]}},{{"id":1,"kind":"selfie"}}]}}"#,
        "b".repeat(50)
    );
    let r = read_look(&answer, &[0, 1]).expect("read");
    assert!(r[0].says.ends_with(CUT));
    assert_eq!(r[0].says.chars().count(), SAYS_MAX + CUT.chars().count());
    assert!(r[0].text.ends_with(CUT));
    assert_eq!(r[0].subjects.len(), SUBJECTS_MAX);
    assert!(r[0].subjects[1].ends_with(CUT));
    assert_eq!(r[0].kind, "screenshot");
    assert_eq!(r[1].kind, "other");
}

/// Check 5: a body has LF line ends, no controls but tab and line feed, no front matter at its
/// head and no first heading that repeats the title; past 200 KB it is cut at a line and marked.
#[test]
fn check_5_a_body_is_cleaned() {
    let b = body_of(
        &note("---\ntitle: x\n---\n\n# Trip\r\nDay one\u{7}\tok\r\n"),
        &[],
    );
    assert_eq!(text_of(&b), "\nDay one\tok");
    // A heading that is not the title stays.
    let b = body_of(&note("# Other\nx"), &[]);
    assert_eq!(text_of(&b), "# Other\nx");
    let long = "line\n".repeat(BODY_MAX / 5 + 100);
    let b = body_of(&note(&long), &[]);
    let t = text_of(&b);
    assert!(t.len() <= BODY_MAX + 10, "{}", t.len());
    assert!(t.ends_with("(cut)"));
}

/// Check 6: `![alt](picture:N)` becomes the app's own line, its alt one line with its brackets
/// escaped, an empty one taking what the look said; a picture placed twice keeps its first place;
/// any other picture loses its `!`.
#[test]
fn check_6_pictures_in_a_body() {
    let looks = [Look {
        id: 0,
        says: "The sign-in window".into(),
        ..Look::default()
    }];
    let a = read_arrange(
        &serde_like(&[
            ("One", "Before ![](picture:0) after ![x](picture:0)"),
            (
                "Two",
                "![again](picture:0) ![web](https://x.test/a.png) ![f](C:/secret.png)",
            ),
        ]),
        &[0],
    )
    .expect("read");
    let c = Compose {
        looks: &looks,
        name: &names,
        text_out: false,
    };
    assert_eq!(
        compose(&a.notes[0], &c),
        "# One\n\nBefore\n\n![The sign-in window](pictures/p0.png)\n\nafter\n"
    );
    let two = compose(&a.notes[1], &c);
    assert!(!two.contains("!["), "{two}");
    assert!(two.contains("[web](https://x.test/a.png)"), "{two}");
    assert!(!two.contains("secret.png"), "{two}");
    let a = read_arrange(&note("![a [b] c](picture:0)"), &[0]).expect("read");
    let t = compose(&a.notes[0], &c);
    assert!(t.contains("![a \\[b\\] c](pictures/p0.png)"), "{t}");
    // An escaped bracket does not close the words: that is no picture, and it loses its `!`.
    let esc = read_arrange(&note("x ![a\\](picture:0)"), &[0]).expect("read");
    assert!(!compose(&esc.notes[0], &c).contains("!["));
    // A name with a space or a bracket is escaped in the address.
    let spaced = |_: usize| Some("my (1).png".to_string());
    let c2 = Compose {
        looks: &looks,
        name: &spaced,
        text_out: false,
    };
    assert!(compose(&a.notes[0], &c2).contains("(pictures/my%20%281%29.png)"));
}

/// Check 7: links are kept only to the web and mail; any other target is unlinked and its words
/// kept; raw HTML has its `<` escaped outside code; a link's definition is never one: its
/// brackets are escaped, whatever it leads to (Fable's review of V2, 4).
#[test]
fn check_7_links_and_html() {
    let b = body_of(
        &note(
            "[ok](https://a.test) [mail](mailto:a@b.test) [js](javascript:alert(1)) [file](file:///etc/passwd) [rel](../x.md)\n<script>x</script> `<kbd>` \n```\n<b>code</b>\n```\n[r]: javascript:x\n[w]: https://w.test",
        ),
        &[],
    );
    let t = text_of(&b);
    assert!(t.contains("[ok](https://a.test)"), "{t}");
    assert!(t.contains("[mail](mailto:a@b.test)"), "{t}");
    assert!(t.contains(" js "), "{t}");
    assert!(!t.contains("(javascript"), "{t}");
    assert!(!t.contains("file:"), "{t}");
    assert!(!t.contains("../x.md"), "{t}");
    assert!(t.contains("&lt;script>x&lt;/script>"), "{t}");
    assert!(t.contains("`<kbd>`"), "{t}");
    assert!(t.contains("\n<b>code</b>\n"), "{t}");
    assert!(t.contains(r"\[w\]: https://w.test"), "{t}");
    assert!(t.contains(r"\[r\]: javascript:x"), "{t}");
}

/// Check 8: "Write out the text in the pictures" puts the look's text under the picture as a
/// quote, whole.
#[test]
fn check_8_the_text_of_a_picture_as_a_quote() {
    let looks = [Look {
        id: 0,
        text: "Your session\n\nhas expired.".into(),
        ..Look::default()
    }];
    let a = read_arrange(&note("![e](picture:0)"), &[0]).expect("read");
    let on = Compose {
        looks: &looks,
        name: &names,
        text_out: true,
    };
    assert_eq!(
        compose(&a.notes[0], &on),
        "# Trip\n\n![e](pictures/p0.png)\n\n> Your session\n>\n> has expired.\n"
    );
    let off = Compose {
        text_out: false,
        ..on
    };
    assert!(!compose(&a.notes[0], &off).contains("session"));
}

/// Check 9: past 100 notes, an empty body with no picture and a second note of one path are
/// dropped and counted; "One note" joins several under their titles; "One per picture" needs no
/// answer.
#[test]
fn check_9_limits_one_note_and_one_per_picture() {
    let mut many: Vec<(String, String)> = (0..NOTES_MAX + 3)
        .map(|i| (format!("N{i}"), "x".to_string()))
        .collect();
    many.push(("Empty".into(), "  \n ".into()));
    many.insert(1, ("n0".into(), "same path".into()));
    let refs: Vec<(&str, &str)> = many.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let a = read_arrange(&serde_like(&refs), &[]).expect("read");
    assert_eq!(a.notes.len(), NOTES_MAX);
    assert_eq!(a.dropped, 5);
    let one = joined(read_arrange(&serde_like(&[("A", "a"), ("B", "b")]), &[]).expect("read"));
    assert_eq!(one.notes.len(), 1);
    let c = Compose {
        looks: &[],
        name: &names,
        text_out: false,
    };
    assert_eq!(compose(&one.notes[0], &c), "# A\n\na\n\n## B\n\nb\n");
    let looks = [Look {
        id: 1,
        title: "Login error".into(),
        says: "A red error.".into(),
        ..Look::default()
    }];
    let file = |id: usize| format!("IMG_{id}.jpg");
    let n = per_picture(&[0, 1, 2], &looks, &file, true);
    assert_eq!(n.len(), 3);
    assert_eq!(n[0].title, "IMG_0");
    assert_eq!(n[1].title, "Login error");
    let c = Compose {
        looks: &looks,
        name: &names,
        text_out: false,
    };
    assert_eq!(
        compose(&n[1], &c),
        "# Login error\n\n![A red error.](pictures/p1.png)\n\nA red error.\n"
    );
    // Two pictures of one title are two notes of two paths.
    let same = [
        Look {
            id: 0,
            title: "Same".into(),
            ..Look::default()
        },
        Look {
            id: 1,
            title: "Same".into(),
            ..Look::default()
        },
    ];
    let n = per_picture(&[0, 1], &same, &file, false);
    assert_ne!(n[0].file_name(), n[1].file_name());
}

/// Check 10: a picture in no kept note is listed as not used, whatever the model said.
#[test]
fn check_10_pictures_in_no_note() {
    let a = read_arrange(
        r#"{"notes":[{"title":"A","body":"![x](picture:2)"},{"title":"A","body":"![y](picture:0)"}],"unused":[2]}"#,
        &[0, 1, 2],
    )
    .expect("read");
    assert_eq!(a.unused, vec![0, 1]);
}

/// Check 11: no text of the model is part of a path but through the name rule: a title or a
/// section that tries to climb out comes out as a plain name.
#[test]
fn check_11_no_model_text_in_a_path_but_through_the_rule() {
    let a = read_arrange(
        r#"{"notes":[{"title":"../../Windows/System32/evil","sections":["..","C:\\x","a/../b"],"body":"x"}]}"#,
        &[],
    )
    .expect("read");
    let d = &a.notes[0];
    let name = d.file_name();
    assert!(!name.contains(['/', '\\', ':']), "{name}");
    for f in d.folders() {
        assert!(!f.contains(['/', '\\', ':']), "{f}");
        assert_ne!(f, "..");
    }
    let p = picture_name(Some("../a"), "x.png", "p/ng");
    assert!(!p.contains(['/', '\\']), "{p}");
    assert_eq!(
        picture_name(Some("Login Error"), "IMG_1.PNG", "PNG"),
        "login-error.png"
    );
    assert_eq!(picture_name(None, "C:/a/IMG 1.jpg", "jpg"), "IMG 1.jpg");
    assert_eq!(numbered("login-error.png", 2), "login-error-2.png");
    assert_eq!(numbered("x", 3), "x-3");
}

/// The requests: the look names each picture by its number and file name, never its folder;
/// the arrange has no picture, only what the look said, and each text file whole.
#[test]
fn the_requests_say_what_is_sent() {
    let p = look_prompt(
        "Notes for the bug",
        &[Sent {
            id: 3,
            name: "IMG_4032.jpg".into(),
        }],
        true,
    );
    assert!(p.starts_with("Notes for the bug\n\n"));
    assert!(p.contains("3: IMG_4032.jpg\n"));
    assert!(p.contains("Write out the text"));
    let looks = [Look {
        id: 3,
        title: "Login error".into(),
        says: "A red error.".into(),
        text: "x".repeat(400),
        ..Look::default()
    }];
    let a = arrange_prompt(
        "",
        HowMany::One,
        &looks,
        &[TextFile {
            name: "notes.txt".into(),
            text: "whole\ntext\n".into(),
        }],
        &["Sign-in".into()],
    );
    assert!(a.contains("How many notes: one note."));
    assert!(a.contains("- Sign-in\n"));
    assert!(a.contains("3: Login error. A red error."));
    assert!(a.contains(&format!("\"{}\"", "x".repeat(TEXT_FOR_ARRANGE))));
    assert!(!a.contains(&"x".repeat(TEXT_FOR_ARRANGE + 1)));
    assert!(a.contains("--- the file notes.txt ---\nwhole\ntext\n--- the end of notes.txt ---"));
}

/// Rule 11: no input panics, broken JSON of every kind included.
#[test]
fn no_answer_panics() {
    let deep = "[".repeat(10_000);
    let cases: Vec<String> = vec![
        String::new(),
        "{".into(),
        "}{".into(),
        "{\"notes\":".into(),
        "{\"notes\":[{\"title\":1,\"body\":[]}]}".into(),
        "{\"notes\":[{\"sections\":{\"a\":1},\"body\":\"![](picture:99999999999999999999)\"}]}"
            .into(),
        "{\"notes\":[{\"body\":\"[[[[[[[[(((((((![![![\"}]}".into(),
        "{\"notes\":[{\"body\":\"```\\nunclosed\"}]}".into(),
        "{\"notes\":[{\"body\":\"---\\nonly front\"}]}".into(),
        "{\"notes\":[{\"body\":\"\\\\\"}]}".into(),
        "{\"pictures\":[{\"id\":1e999},{\"id\":\"0\"},null,3]}".into(),
        format!("{{\"notes\":{deep}}}"),
        "{\"notes\":[{\"body\":\"\u{feff}é\u{0}\u{1b}[31m\"}]}".into(),
        "{\"notes\":[{\"body\":\"[a](<javascript:x>) ![b](<picture:0>)\"}]}".into(),
    ];
    for c in &cases {
        let _ = read_look(c, &[0, 1]);
        if let Ok(a) = read_arrange(c, &[0, 1]) {
            for n in &a.notes {
                let _ = compose(
                    n,
                    &Compose {
                        looks: &[],
                        name: &names,
                        text_out: true,
                    },
                );
            }
        }
    }
    // An angle-bracketed picture address is the run's picture; an angle-bracketed script is not
    // a link.
    let b = body_of(&note("[a](<javascript:x>) ![b](<picture:0>)"), &[0]);
    assert_eq!(text_of(&b), "a <<0:b>>");
}

/// Fable's review of V2, 1 to 6: a parser reads blocks and paragraphs, not lines, so each of these
/// once put a remote picture or a tag live in a note. Each comes out with nothing live but a link
/// to the web, a code span closed on its own line, or a fence at the first column; every other
/// bracket, `!`, backtick and `<` is escaped. (The outputs were checked against the CommonMark
/// rules by hand: no parser is a dependency of this crate.)
#[test]
fn no_body_puts_a_live_picture_or_tag_in_a_note() {
    let cases: [(&str, &str); 23] = [
        (
            r#"1. Step

    ![shot](https://evil.test/p.png)"#,
            r#"1. Step

    [shot](https://evil.test/p.png)"#,
        ),
        (
            r#"1. Step

    <img src=https://evil.test/p.png>"#,
            r#"1. Step

    &lt;img src=https://evil.test/p.png>"#,
        ),
        (
            r#"!![x](https://evil.test/p.png)"#,
            r#"\![x](https://evil.test/p.png)"#,
        ),
        (
            r#"!![a](picture:99)[x](https://evil.test/p.png)"#,
            r#"\![x](https://evil.test/p.png)"#,
        ),
        (
            r#"[!](x)[y](https://evil.test/p.png)"#,
            r#"\![y](https://evil.test/p.png)"#,
        ),
        (
            r#"!![x][r]
[r]: https://evil.test/p.png"#,
            r#"!\!\[x\]\[r\]
\[r\]: https://evil.test/p.png"#,
        ),
        (
            r#"[[[[![x](https://evil.test/p.png)](a)](b)](c)](d)"#,
            r#"\[\[\[\!\[x\](https://evil.test/p.png)\](a)\](b)\](c)"#,
        ),
        (
            r#"[x]: https://a.test <img src=https://evil.test/p.png>"#,
            r#"\[x\]: https://a.test &lt;img src=https://evil.test/p.png>"#,
        ),
        (
            r#"[![x](https://evil.test/p.png)]: https://a.test"#,
            r#"\[[x](https://evil.test/p.png)\]: https://a.test"#,
        ),
        (
            r#"[x](https://a.test <img src=https://evil.test/p.png>)"#,
            r#"\[x\](https://a.test &lt;img src=https://evil.test/p.png>)"#,
        ),
        (
            r#"[x](https://a.test ![y](https://evil.test/p.png))"#,
            r#"\[x\](https://a.test [y](https://evil.test/p.png))"#,
        ),
        (
            r#"foo ` bar
baz ` <img src=https://evil.test/p.png> ` qux"#,
            r#"foo \` bar
baz ` <img src=https://evil.test/p.png> ` qux"#,
        ),
        (
            r#"[a ` b](https://a.test) ` <img src=https://evil.test/p.png> `"#,
            r#"[a \` b](https://a.test) ` <img src=https://evil.test/p.png> `"#,
        ),
        (
            r#"``` a `b
<img src=https://evil.test/p.png>
```"#,
            r#"\`\`\` a \`b
&lt;img src=https://evil.test/p.png>
```
```"#,
        ),
        (
            r#"- a
  ```
<img src=https://evil.test/p.png>"#,
            r#"- a
  \`\`\`
&lt;img src=https://evil.test/p.png>"#,
        ),
        (
            r#"\``x`` <img src=https://evil.test/p.png> ``"#,
            r#"\`\`x`` <img src=https://evil.test/p.png> ``"#,
        ),
        (
            r#"![x
y](https://evil.test/p.png)"#,
            r#"\!\[x
y\](https://evil.test/p.png)"#,
        ),
        (
            r#"[ok](https://a.test/(b)) [j](javascript:alert(1)) [a](<https://a.test/x>)"#,
            r#"[ok](https://a.test/(b)) j [a](https://a.test/x)"#,
        ),
        // Fable's review of V2 round 2, 1: a closer this reads wider than a parser is written as
        // one, so the next first-column fence opens for both.
        (
            "```\n    ```\n```\n<img src=https://evil.test/p.png>",
            "```\n```\n```\n<img src=https://evil.test/p.png>\n```",
        ),
        (
            "```\n```\u{a0}\n<img src=https://evil.test/p.png>",
            "```\n```\n&lt;img src=https://evil.test/p.png>",
        ),
        // 2: a run of tildes a parser could open a fence with is escaped.
        (
            " ~~~\n~~~\n<img src=https://evil.test/p.png>",
            " \\~~~\n~~~\n<img src=https://evil.test/p.png>\n~~~",
        ),
        // Round 3: the words of a link whose address is refused are written alone, tildes too.
        (
            "[~~~](x)\n~~~\n<img src=https://evil.test/p.png>",
            "\\~\\~\\~\n~~~\n<img src=https://evil.test/p.png>\n~~~",
        ),
        (
            "![a](picture:0)~~~ and ~~gone~~",
            "<<0:a>>\\~~~ and ~~gone~~",
        ),
    ];
    for (input, want) in cases {
        let b = body_of(&note(input), &[0]);
        assert_eq!(text_of(&b), want, "{input:?}");
    }
}

/// Fable's review of V2, 8: a line of 200 KB of brackets or of links half written is read in
/// linear time, not in the square of its length.
#[test]
fn a_long_line_of_brackets_is_read_at_once() {
    let t = std::time::Instant::now();
    for unit in ["[", "[a](", "`", "![", "]("] {
        let line = unit.repeat(BODY_MAX / unit.len() - 8);
        let b = body_of(&note(&line), &[0]);
        assert!(!text_of(&b).is_empty());
    }
    assert!(t.elapsed().as_secs() < 5, "{:?}", t.elapsed());
}

/// Fable's review of V2, 7: two pictures whose titles share their first 64 characters once looped
/// for ever, each number cut off by the name's cap. They are two notes of two paths.
#[test]
fn long_titles_alike_are_numbered_where_the_number_is_kept() {
    let long = "a".repeat(70);
    let file = |id: usize| format!("{long}{id}.png");
    let n = per_picture(&[0, 1, 2], &[], &file, false);
    assert_eq!(n.len(), 3);
    assert_ne!(n[0].file_name(), n[1].file_name());
    assert_ne!(n[1].file_name(), n[2].file_name());
    assert_ne!(n[0].file_name(), n[2].file_name());
}

/// Fable's review of V2, 9 and 10: `#`, `?` and `%` in a picture's name are escaped in its
/// address; the arrange request is given the whole of a picture's text on one line.
#[test]
fn an_address_keeps_its_hash_and_a_text_its_lines() {
    let a = read_arrange(&note("![x](picture:0)"), &[0]).expect("read");
    let hashed = |_: usize| Some("Shot #3 ?100%.png".to_string());
    let c = Compose {
        looks: &[],
        name: &hashed,
        text_out: false,
    };
    assert!(
        compose(&a.notes[0], &c).contains("(pictures/Shot%20%233%20%3F100%25.png)"),
        "{}",
        compose(&a.notes[0], &c)
    );
    let looks = [Look {
        id: 0,
        title: "Error".into(),
        text: "OK
Your session has expired"
            .into(),
        ..Look::default()
    }];
    let p = arrange_prompt("", HowMany::AsNeeded, &looks, &[], &[]);
    assert!(p.contains("Text: \"OK Your session has expired\""), "{p}");
}

/// Fable's review of V2 round 2, 3: a body that ends in an open fence is closed, so "One note"
/// answered with two cannot have the second's fence close the first's and its tag go live.
#[test]
fn a_fence_left_open_is_closed_before_the_next_note() {
    let names = |_: usize| None;
    let answer = serde_like(&[
        ("A", "```\nx"),
        ("B", "```\n<img src=https://evil.test/p.png>\n```"),
    ]);
    let one = joined(read_arrange(&answer, &[]).expect("read"));
    let c = Compose {
        looks: &[],
        name: &names,
        text_out: false,
    };
    assert_eq!(
        compose(&one.notes[0], &c),
        "# A\n\n```\nx\n```\n\n## B\n\n```\n<img src=https://evil.test/p.png>\n```\n"
    );
}
