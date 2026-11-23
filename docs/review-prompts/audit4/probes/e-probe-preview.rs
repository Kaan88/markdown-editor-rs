//! Review probe (not to be committed): hostile requests must not panic the preview server.
#![cfg(feature = "server")]

use mdview_preview::Server;
use mdview_preview::wire::Request;

const DOCS: &[&str] = &[
    "",
    "\n",
    "# T\n\npara one\n\n- [ ] a\n- [x] b\n  - [ ] c\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```rust\nfn x(){}\n```\n\n> quote\n> more\n\nlast é ü 😀 text\n",
    "- a\n  - b\n    - c\n\n1. x\n2. y\n\n<details><summary>s</summary>\n\nin\n\n</details>\n\n[^1]: note\n\ntext[^1]\n",
    "é",
    "😀😀😀\n\n😀",
    "```mermaid\ngraph TD\nA-->B\n```\n\n$$x^2$$\n",
    "<div>\n\n# in html\n\n</div>\n\n---\n\n***\n",
];

fn floats() -> Vec<f32> {
    vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        50.0,
        199.5,
        1e9,
        -1e9,
        f32::MAX,
        f32::MIN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::MIN_POSITIVE,
    ]
}

fn ints() -> Vec<u32> {
    vec![0, 1, 2, 3, 5, 7, 100, u32::MAX - 1, u32::MAX]
}

fn drain(s: &mut Server) {
    let drew = s.frame().is_some();
    let _ = s.selection_answer(drew);
    let _ = s.take_task();
    let _ = s.take_pointer();
    let _ = s.take_block();
    let _ = s.gap_y();
}

fn server(doc: &str, before_view: bool) -> Server {
    let mut s = Server::new();
    let text = Request::Text {
        text: doc.to_string(),
        base_dir: None,
    };
    let view = Request::View {
        width: 400,
        height: 300,
        scale: 1.0,
        dark: false,
    };
    if before_view {
        s.apply(text);
        s.apply(view);
    } else {
        s.apply(view);
        s.apply(text);
    }
    s
}

#[test]
fn hostile_pointer_requests() {
    for doc in DOCS {
        let mut s = server(doc, true);
        drain(&mut s);
        for &x in &floats() {
            for &y in &floats() {
                for clicks in [0, 1, 2, 3, u32::MAX] {
                    s.apply(Request::Press { x, y, clicks });
                    s.apply(Request::Drag { x: y, y: x });
                    s.apply(Request::Copy);
                    drain(&mut s);
                    s.apply(Request::Release);
                }
                s.apply(Request::Hover { x, y });
                s.apply(Request::BlockAt { x, y });
                drain(&mut s);
            }
        }
    }
}

#[test]
fn hostile_gaps_lines_and_scrolls() {
    for doc in DOCS {
        for before in [true, false] {
            let mut s = server(doc, before);
            for &first in &ints() {
                for &last in &ints() {
                    for &h in &floats() {
                        s.apply(Request::Gap {
                            gap: Some((first, last, h)),
                        });
                        drain(&mut s);
                        s.apply(Request::BlockOfLine { line: first });
                        s.apply(Request::Follow {
                            line: first,
                            column: last,
                        });
                        s.apply(Request::Scroll { by: h });
                        s.apply(Request::Align {
                            line: h,
                            tail: first as f32,
                            end: last as f32,
                        });
                        s.apply(Request::SelectAll);
                        s.apply(Request::Copy);
                        drain(&mut s);
                    }
                }
            }
            // A gap set, then the text changes under it (shorter, empty, longer).
            s.apply(Request::Gap {
                gap: Some((3, 9, 120.0)),
            });
            for t in ["", "x", "a\n\nb\n\nc\n\nd\n\ne\n\nf\n\ng\n"] {
                s.apply(Request::Text {
                    text: t.to_string(),
                    base_dir: None,
                });
                drain(&mut s);
                s.apply(Request::BlockAt { x: 100.0, y: 50.0 });
                s.apply(Request::BlockOfLine { line: 4 });
                s.apply(Request::Press {
                    x: 100.0,
                    y: 60.0,
                    clicks: 2,
                });
                drain(&mut s);
            }
            s.apply(Request::Gap { gap: None });
            drain(&mut s);
        }
    }
}

/// A selection made in one text, then a shorter text: the offsets must not be used.
#[test]
fn selection_then_shorter_text_then_drag() {
    let mut s = server(DOCS[2], true);
    drain(&mut s);
    s.apply(Request::Press {
        x: 100.0,
        y: 100.0,
        clicks: 1,
    });
    s.apply(Request::Text {
        text: "é".into(),
        base_dir: None,
    });
    s.apply(Request::Drag { x: 300.0, y: 250.0 });
    s.apply(Request::Copy);
    drain(&mut s);
    s.apply(Request::Release);
    drain(&mut s);
}

/// Sizes at the small edge and odd scales.
#[test]
fn odd_views() {
    for doc in DOCS {
        let mut s = server(doc, true);
        for (w, h) in [(0, 0), (1, 1), (0, 300), (400, 0), (119, 7), (3000, 2)] {
            for scale in [0.0, 0.05, 0.11, 1.0, 3.7, f32::NAN, f32::INFINITY, -2.0, 64.0] {
                s.apply(Request::View {
                    width: w,
                    height: h,
                    scale,
                    dark: true,
                });
                s.apply(Request::Gap {
                    gap: Some((1, 5, 40.0)),
                });
                drain(&mut s);
                s.apply(Request::Press {
                    x: 10.0,
                    y: 1.0,
                    clicks: 3,
                });
                s.apply(Request::BlockAt { x: 10.0, y: 1.0 });
                drain(&mut s);
                s.apply(Request::Gap { gap: None });
            }
        }
    }
}
