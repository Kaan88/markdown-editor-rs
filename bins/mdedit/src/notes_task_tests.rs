use super::*;
use crate::ai::PictureInfo;

fn ready(bytes: u64, w: u32, ow: u32) -> Measure {
    Measure::Ready(PictureInfo {
        format: "jpeg".into(),
        width: w,
        height: w / 2,
        bytes,
        sha256: "s".into(),
        original_width: ow,
        original_height: ow / 2,
        done: Vec::new(),
        thumb: None,
    })
}

/// Pictures plan 13: the footer says what goes, in bytes and requests, the button names what
/// leaves, and nothing is said to be sent with the AI off.
#[test]
fn the_footer_and_the_button_say_what_leaves() {
    let a = ready(1_000_000, 1568, 4000);
    let b = ready(500_000, 800, 800);
    assert_eq!(
        summary(&[&a, &b], 1, HowMany::AsNeeded, true),
        "2 pictures, 1.5 MB as sent, and 1 text file \u{00B7} 2 requests"
    );
    assert_eq!(
        summary(&[&a, &Measure::Waiting], 0, HowMany::AsNeeded, true),
        "2 pictures, measuring \u{00B7} 2 requests"
    );
    assert_eq!(
        summary(&[&Measure::Waiting], 0, HowMany::One, false),
        "1 picture \u{00B7} no request"
    );
    assert_eq!(primary_label(5, 1, true), "Send 5 pictures and 1 file");
    assert_eq!(primary_label(0, 2, true), "Send 2 files");
    assert_eq!(primary_label(5, 1, false), "Make the notes");
}

/// Pictures plan 3 and 11: a look per four pictures, and the arrange unless it is one note per
/// picture.
#[test]
fn a_run_makes_a_look_per_four_pictures_and_one_arrange() {
    assert_eq!(requests(5, 1, HowMany::AsNeeded), 3);
    assert_eq!(requests(8, 0, HowMany::One), 3);
    assert_eq!(requests(5, 0, HowMany::PerPicture), 2);
    assert_eq!(requests(0, 2, HowMany::PerPicture), 1);
    let b = batches(&[0, 1, 2, 3, 4, 7]);
    assert_eq!(b, vec![vec![0, 1, 2, 3], vec![4, 7]]);
}

/// Pictures plan 11: a limit hit is said with both numbers.
#[test]
fn a_limit_is_said_with_both_numbers() {
    assert_eq!(
        blocked(104, 0, false).as_deref(),
        Some("104 pictures are ticked: a run takes 100.")
    );
    assert_eq!(
        blocked(3, 250_001, false).as_deref(),
        Some("The text files are 251 KB together: a run takes 200 KB.")
    );
    assert!(blocked(100, 200_000, false).is_none());
    assert!(blocked(1, 0, true).is_some(), "it waits for the measures");
}

/// Pictures plan 12: the names of the copies mean nothing when nothing is copied, and adding to
/// notes already there is not in this build.
#[test]
fn the_options_ticks_follow_each_other() {
    let o = NotesOptions::default();
    assert!(Tick::Rename.enabled(&o));
    let off = Tick::Copy.toggled(o);
    assert!(!off.copy && !Tick::Rename.enabled(&off));
    assert!(!Tick::AddToNotes.enabled(&o));
    assert_eq!(Tick::AddToNotes.toggled(o), o);
    assert_eq!(long_side(&o), 1568);
    assert_eq!(long_side(&Tick::Larger.toggled(o)), 2576);
}

/// Pictures plan 15: "Try it again" sends the failed looks and keeps the answered ones; "Go on
/// with N" leaves the failed ones out.
#[test]
fn a_failed_look_is_sent_again_or_left_out_and_the_answers_stay() {
    let mut r = Run {
        batches: vec![vec![0, 1, 2, 3], vec![4, 5]],
        next: 2,
        answered: vec![0],
        failed: vec![(1, "Too many requests.".into())],
        stopped: true,
        ..Run::default()
    };
    assert!(r.failed_at(4) && !r.failed_at(0));
    assert_eq!(r.looked(), 4);
    let mut again = r.clone();
    again.again();
    assert_eq!(again.batches, vec![vec![0, 1, 2, 3], vec![4, 5]]);
    assert_eq!((again.next, again.answered.clone()), (1, vec![0]));
    assert!(!again.stopped && again.failed.is_empty());
    r.go_on();
    assert_eq!(r.batches, vec![vec![0, 1, 2, 3]]);
    assert!(r.looks_ended() && r.failed.is_empty());
    assert_eq!(r.looked_ids(), vec![0, 1, 2, 3]);
}

/// A thumbnail is kept small (pictures plan 6: the editor's memory does not grow with a photo),
/// and a transparent pixel adds no colour to its neighbour.
#[test]
fn a_thumbnail_is_kept_small_and_its_cutout_has_no_fringe() {
    let (w, h) = (160u32, 80u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for (k, px) in rgba.chunks_mut(4).enumerate() {
        if k % 2 == 0 {
            px.copy_from_slice(&[255, 0, 0, 255]);
        }
    }
    let (out, nw, nh) = small(&rgba, w, h, 80);
    assert_eq!((nw, nh), (80, 40));
    assert_eq!(out.len(), (80 * 40 * 4) as usize);
    assert_eq!(
        &out[..4],
        &[255, 0, 0, 127],
        "red, half covered: no black in it"
    );
    let (same, sw, sh) = small(&rgba[..(40 * 20 * 4)], 40, 20, 80);
    assert_eq!((sw, sh, same.len()), (40, 20, 40 * 20 * 4));
    assert_eq!(
        small(&[1, 2], 4, 4, 80).1,
        0,
        "a short buffer is no picture"
    );
}

/// A picture's row says what goes from the engine's figures: smaller, or as it is.
#[test]
fn a_row_says_what_the_picture_is_sent_as() {
    assert_eq!(sent_as(&Measure::Waiting), "measuring\u{2026}");
    assert_eq!(
        sent_as(&ready(204_000, 1568, 4000)),
        "sent smaller: 1568 \u{00D7} 784 \u{00B7} 204 KB"
    );
    assert_eq!(sent_as(&ready(204_000, 800, 800)), "sent as it is");
}
