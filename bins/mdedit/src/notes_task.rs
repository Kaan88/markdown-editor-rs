//! Write notes (`docs/ai-pictures-plan.md`, phase V3): what the task window keeps for its
//! fourth task. Each picture is measured by the engine when it is added (`prepare`), so its row
//! says what is sent before anything is; the run then sends the pictures in look requests of
//! four, as many at once as the connection takes, and one arrange request of what the looks said
//! and the text files. The answers are read by `mdedit_core::notes_ai`; nothing here reads the
//! disk or the engine. `app/notes_task.rs` is the window's part of it.

use crate::ai::PictureInfo;
use crate::task_ui::{FileEntry, FileKind};
use mdedit_core::notes_ai::{self, Arranged, HowMany, Look};
use mdedit_core::prefs::NotesOptions;

/// What the engine said a picture would be sent as.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Measure {
    /// Not measured yet: its row says "measuring…" and the run waits for it.
    #[default]
    Waiting,
    Ready(PictureInfo),
    /// It cannot be sent: the engine's reason, naming nothing but the file.
    Refused(String),
}

/// The most text the arrange request takes from the text files together (plan section 11).
pub const TEXT_MAX: usize = 200 * 1000;

/// One of the seven ticks of the Options popover (plan section 12).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tick {
    Copy,
    Rename,
    Describe,
    TextOut,
    Strip,
    Larger,
    /// "Let it add to notes I already have": never on in this build (D-079, phase A).
    AddToNotes,
}

impl Tick {
    pub const ALL: [Tick; 7] = [
        Tick::Copy,
        Tick::Rename,
        Tick::Describe,
        Tick::TextOut,
        Tick::Strip,
        Tick::Larger,
        Tick::AddToNotes,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tick::Copy => "Copy the pictures beside the notes",
            Tick::Rename => "Name the copies after what they show",
            Tick::Describe => "Describe each picture",
            Tick::TextOut => "Write out the text in the pictures",
            Tick::Strip => "Remove location and camera details before sending",
            Tick::Larger => "Send larger pictures, for small print",
            Tick::AddToNotes => "Let it add to notes I already have",
        }
    }

    /// The line under it.
    pub fn line(self) -> &'static str {
        match self {
            Tick::Copy => "Off links each picture where it is.",
            Tick::Rename => "Off keeps the files\u{2019} own names.",
            Tick::Describe => "A sentence or two under each picture, and in its brackets.",
            Tick::TextOut => "The words in each picture, as a quote under it.",
            Tick::Strip => "Where it was taken and with what: never in what is sent.",
            Tick::Larger => "2576 px on the long side, about 2.7 times the cost.",
            Tick::AddToNotes => "Not in this version: each run writes new notes.",
        }
    }

    pub fn on(self, o: &NotesOptions) -> bool {
        match self {
            Tick::Copy => o.copy,
            Tick::Rename => o.rename,
            Tick::Describe => o.describe,
            Tick::TextOut => o.text_out,
            Tick::Strip => o.strip,
            Tick::Larger => o.larger,
            Tick::AddToNotes => false,
        }
    }

    pub fn enabled(self, o: &NotesOptions) -> bool {
        match self {
            // The names of the copies mean nothing when nothing is copied.
            Tick::Rename => o.copy,
            Tick::AddToNotes => false,
            _ => true,
        }
    }

    /// The options with this tick changed. Strip and Larger change what is sent: the pictures
    /// are measured again.
    pub fn toggled(self, mut o: NotesOptions) -> NotesOptions {
        match self {
            Tick::Copy => o.copy = !o.copy,
            Tick::Rename => o.rename = !o.rename,
            Tick::Describe => o.describe = !o.describe,
            Tick::TextOut => o.text_out = !o.text_out,
            Tick::Strip => o.strip = !o.strip,
            Tick::Larger => o.larger = !o.larger,
            Tick::AddToNotes => {}
        }
        o
    }
}

/// The three ways of "How many notes", in the order the switch shows them.
pub const HOW_MANY: [HowMany; 3] = [HowMany::AsNeeded, HowMany::One, HowMany::PerPicture];

pub fn how_many_label(h: HowMany) -> &'static str {
    match h {
        HowMany::AsNeeded => "As many as it takes",
        HowMany::One => "One note",
        HowMany::PerPicture => "One per picture",
    }
}

/// The long side a run asks for.
pub fn long_side(o: &NotesOptions) -> u32 {
    if o.larger { 2576 } else { 1568 }
}

/// A run of Write notes: the look requests, then the arrange request.
#[derive(Clone, Debug, Default)]
pub struct Run {
    /// Each look request's pictures, by their place in the window.
    pub batches: Vec<Vec<usize>>,
    /// The next batch to send.
    pub next: usize,
    /// The requests out: (request id, batch).
    pub in_flight: Vec<(u64, usize)>,
    /// The batches answered and read.
    pub answered: Vec<usize>,
    /// The batches that failed, each with why.
    pub failed: Vec<(usize, String)>,
    /// What the model said of each picture; a look's `id` is the picture's place in the window.
    pub looks: Vec<Look>,
    /// Requests at once (D-080).
    pub parallel: usize,
    /// Stop, a failure, or the window's change: no more requests go.
    pub stopped: bool,
    /// The arrange request out.
    pub arrange: Option<u64>,
    /// The arrange answer, read; the plan card (V4) shows it.
    pub arranged: Option<Arranged>,
    /// Why the whole run stopped (the model reads no pictures, the engine is old), with the
    /// settings button.
    pub fault: Option<String>,
    /// Bytes and requests sent, for the record.
    pub bytes: u64,
    pub requests: u64,
    /// The text files the arrange request is given, by their place in the window.
    pub texts: Vec<usize>,
    /// What the looks were asked with: a look is kept for the next Send only when it was asked
    /// the same instruction, and with the text written out if that is asked now.
    pub instruction: String,
    pub text_out: bool,
    /// Why the arrange request failed, with "Try it again" and "One note per picture instead".
    pub arrange_failed: Option<String>,
    /// What "Arrange again…" asked, apart from the instruction the looks were asked with: the
    /// looks stay kept for the window's own Send.
    pub arrange_instruction: Option<String>,
    /// The plan "Arrange again…" replaces, kept until the new one is read: a failure or a stop
    /// gives it back as it was.
    pub previous: Option<Arranged>,
    /// The plan card's renames of the copies: a picture's place in the window to its copy's file
    /// name with its extension, set only when the owner renamed one (phase V5 honours it).
    pub copy_names: std::collections::HashMap<usize, String>,
    /// The requests the provider answered, and those that failed: a stopped one is neither.
    /// The plan card says them apart ("answered 3 requests, 1 failed").
    pub answers: u64,
    pub failures: u64,
}

impl Run {
    /// The pictures looked at so far.
    pub fn looked(&self) -> usize {
        self.answered
            .iter()
            .map(|&b| self.batches.get(b).map_or(0, Vec::len))
            .sum()
    }

    pub fn pictures(&self) -> usize {
        self.batches.iter().map(Vec::len).sum()
    }

    /// Whether a look request is out or still to go.
    pub fn looking(&self) -> bool {
        !self.in_flight.is_empty() || (!self.stopped && self.next < self.batches.len())
    }

    /// Every batch answered or failed, and nothing out.
    pub fn looks_ended(&self) -> bool {
        self.in_flight.is_empty() && (self.stopped || self.next >= self.batches.len())
    }

    /// The look of the picture at `i`.
    pub fn look(&self, i: usize) -> Option<&Look> {
        self.looks.iter().find(|l| l.id == i)
    }

    /// Whether the picture at `i` was in a batch that failed.
    pub fn failed_at(&self, i: usize) -> bool {
        self.failed
            .iter()
            .any(|(b, _)| self.batches.get(*b).is_some_and(|ids| ids.contains(&i)))
    }

    /// An arrange request failed or was stopped (`why` is `None`): a plan asked again gives back
    /// the one it was to replace, and says so by returning true; a first plan keeps why, with the
    /// ways on.
    pub fn arrange_lost(&mut self, why: Option<String>) -> bool {
        if let Some(p) = self.previous.take() {
            self.arranged = Some(p);
            return true;
        }
        if why.is_some() {
            self.arrange_failed = why;
        }
        false
    }

    /// Whether this run's looks may be kept for a Send asked `instruction`, with the text in
    /// the pictures written out or not: a look asked otherwise is asked again.
    pub fn keeps(&self, instruction: &str, text_out: bool) -> bool {
        self.instruction == instruction && (self.text_out || !text_out)
    }

    /// Every picture the model has said something of, in the window's order: what the arrange
    /// request may place. Looks kept from an earlier Send count, as do the ones of this one.
    pub fn sent_ids(&self) -> Vec<usize> {
        let mut ids: Vec<usize> = self.looks.iter().map(|l| l.id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// The batches neither answered, failed nor out: the ones a stop or a failure kept from
    /// leaving.
    fn unsent(&self) -> Vec<usize> {
        (0..self.batches.len())
            .filter(|b| {
                !self.answered.contains(b)
                    && !self.failed.iter().any(|(f, _)| f == b)
                    && !self.in_flight.iter().any(|(_, f)| f == b)
            })
            .collect()
    }

    /// The batches made again: the answered ones first, still counted as answered, then the
    /// batches `then`, still to send.
    fn rebatch(&mut self, then: &[usize]) {
        let mut kept: Vec<Vec<usize>> = self
            .answered
            .iter()
            .filter_map(|&b| self.batches.get(b).cloned())
            .collect();
        let n = kept.len();
        // A batch with no picture left in it is never sent.
        kept.extend(
            then.iter()
                .filter_map(|&b| self.batches.get(b).cloned())
                .filter(|ids| !ids.is_empty()),
        );
        self.batches = kept;
        self.answered = (0..n).collect();
        self.next = n;
        self.stopped = false;
        self.fault = None;
    }

    /// "Try it again": the batches that failed go again, then those never sent; the answered
    /// ones are kept (plan section 15).
    pub fn again(&mut self) {
        let unsent = self.unsent();
        let mut then: Vec<usize> = self.failed.drain(..).map(|(b, _)| b).collect();
        then.extend(unsent);
        self.rebatch(&then);
    }

    /// "Go on with N": the failed batches are left out; those never sent still go.
    pub fn go_on(&mut self) {
        let unsent = self.unsent();
        self.failed.clear();
        self.rebatch(&unsent);
    }

    /// How many pictures "Go on with N" goes on with: those looked at (in this Send or kept from
    /// an earlier one) and those still to send; never those of the failed batches.
    pub fn going_on_with(&self) -> usize {
        let unsent: usize = self
            .unsent()
            .iter()
            .map(|&b| self.batches.get(b).map_or(0, Vec::len))
            .sum();
        let mut looked = self.sent_ids();
        for &b in &self.answered {
            looked.extend(self.batches.get(b).into_iter().flatten().copied());
        }
        looked.sort_unstable();
        looked.dedup();
        looked.len() + unsent
    }

    /// Whether going on without the failed looks leaves anything to make notes of: a picture
    /// looked at or still to send, or a text file. Going on with nothing would send an arrange
    /// request that cannot work, so it is not offered (the hand-driven run, 2026-11-25).
    pub fn can_go_on(&self) -> bool {
        self.going_on_with() > 0 || !self.texts.is_empty()
    }

    /// The words of "Go on with N" (the button that leaves the failed looks out).
    pub fn go_on_label(&self) -> String {
        match self.going_on_with() {
            0 => "Go on with the text only".to_string(),
            n => format!("Go on with {n}"),
        }
    }
}

/// The batches of a run: the pictures that take part, four to a request.
pub fn batches(pictures: &[usize]) -> Vec<Vec<usize>> {
    pictures
        .chunks(notes_ai::LOOK_BATCH)
        .map(<[usize]>::to_vec)
        .collect()
}

/// Why a file takes no part in Write notes, on top of what `task_add` found.
pub fn reason(f: &FileEntry, measure: &Measure) -> Option<String> {
    match f.kind {
        FileKind::Picture => match measure {
            Measure::Refused(why) => Some(why.clone()),
            _ => None,
        },
        FileKind::Text => {
            if f.text.is_none() {
                if f.size as usize > f.limits.rewrite_bytes {
                    Some(format!(
                        "too long to be sent whole: {} KB of {} KB",
                        f.size.div_ceil(1000),
                        f.limits.rewrite_bytes / 1000
                    ))
                } else {
                    Some("not UTF-8: save it as UTF-8 first".to_string())
                }
            } else {
                None
            }
        }
        FileKind::Other => Some(
            f.why
                .clone()
                .unwrap_or_else(|| "not a picture or a text file".to_string()),
        ),
    }
}

/// What a picture's row says at its right: what goes, from the engine's figures.
pub fn sent_as(m: &Measure) -> String {
    match m {
        Measure::Waiting => "measuring\u{2026}".to_string(),
        Measure::Refused(_) => String::new(),
        Measure::Ready(info) if info.smaller() => format!(
            "sent smaller: {} \u{00D7} {} \u{00B7} {}",
            info.width,
            info.height,
            crate::task_ui::size_text(info.bytes)
        ),
        Measure::Ready(_) => "sent as it is".to_string(),
    }
}

/// What a picture's row says under its name: its own pixels and size.
pub fn picture_line(f: &FileEntry, m: &Measure) -> String {
    match m {
        Measure::Ready(info) if info.original_width > 0 => format!(
            "{} \u{00D7} {} \u{00B7} {}",
            info.original_width,
            info.original_height,
            crate::task_ui::size_text(f.size)
        ),
        _ => crate::task_ui::size_text(f.size),
    }
}

/// "5 pictures, 1.5 MB as sent, and 1 text file · 3 requests".
pub fn summary(pictures: &[&Measure], texts: usize, how_many: HowMany, ai_on: bool) -> String {
    let n = pictures.len();
    if n == 0 && texts == 0 {
        return "No file takes part.".to_string();
    }
    let bytes: u64 = pictures
        .iter()
        .map(|m| match m {
            Measure::Ready(i) => i.bytes,
            _ => 0,
        })
        .sum();
    let waiting = pictures.iter().any(|m| matches!(m, Measure::Waiting));
    let mut s = match n {
        0 => String::new(),
        1 => "1 picture".to_string(),
        n => format!("{n} pictures"),
    };
    // With the AI off nothing is measured, for nothing is sent.
    if n > 0 && ai_on {
        if waiting {
            s.push_str(", measuring");
        } else {
            s.push_str(&format!(", {} as sent", crate::task_ui::size_text(bytes)));
        }
    }
    if texts > 0 {
        if s.contains(',') {
            s.push_str(", and ");
        } else if !s.is_empty() {
            s.push_str(" and ");
        }
        s.push_str(&match texts {
            1 => "1 text file".to_string(),
            t => format!("{t} text files"),
        });
    }
    if !ai_on {
        s.push_str(" \u{00B7} no request");
        return s;
    }
    let requests = requests(n, texts, how_many);
    s.push_str(&format!(
        " \u{00B7} {requests} request{}",
        if requests == 1 { "" } else { "s" }
    ));
    s
}

/// The requests a run makes: a look per four pictures, and the arrange unless it is one note
/// per picture with no text file.
pub fn requests(pictures: usize, texts: usize, how_many: HowMany) -> usize {
    let looks = pictures.div_ceil(notes_ai::LOOK_BATCH);
    looks + usize::from(arranges(pictures, texts, how_many))
}

/// Whether a run ends with the request that arranges the notes.
pub fn arranges(pictures: usize, texts: usize, how_many: HowMany) -> bool {
    how_many != HowMany::PerPicture || (pictures == 0 && texts > 0)
}

/// The button: what leaves. "Send 5 pictures and 1 file".
pub fn primary_label(pictures: usize, texts: usize, ai_on: bool) -> String {
    if !ai_on {
        return "Make the notes".to_string();
    }
    let p = match pictures {
        0 => String::new(),
        1 => "1 picture".to_string(),
        n => format!("{n} pictures"),
    };
    let t = match texts {
        0 => String::new(),
        1 => "1 file".to_string(),
        n => format!("{n} files"),
    };
    match (p.is_empty(), t.is_empty()) {
        (false, false) => format!("Send {p} and {t}"),
        (false, true) => format!("Send {p}"),
        (true, false) => format!("Send {t}"),
        (true, true) => "Send".to_string(),
    }
}

/// The heading over the pictures: what is sent of them.
pub fn pictures_heading(strip: bool, ai_on: bool) -> &'static str {
    match (ai_on, strip) {
        (false, _) => "PICTURES \u{00B7} NOTHING IS SENT WITH THE AI OFF",
        (true, true) => {
            "PICTURES \u{00B7} SENT TO THE AI, AS EACH ROW SAYS \u{00B7} LOCATION AND CAMERA DETAILS REMOVED"
        }
        (true, false) => {
            "PICTURES \u{00B7} SENT TO THE AI, AS EACH ROW SAYS \u{00B7} LOCATION AND CAMERA DETAILS KEPT"
        }
    }
}

/// Why the run cannot go now, said on the footer with both numbers where there are two.
pub fn blocked(pictures: usize, text_bytes: usize, waiting: bool) -> Option<String> {
    if pictures > notes_ai::PICTURES_MAX {
        return Some(format!(
            "{pictures} pictures are ticked: a run takes {}.",
            notes_ai::PICTURES_MAX
        ));
    }
    if text_bytes > TEXT_MAX {
        return Some(format!(
            "The text files are {} KB together: a run takes {} KB.",
            text_bytes.div_ceil(1000),
            TEXT_MAX / 1000
        ));
    }
    if waiting {
        return Some("Measuring the pictures\u{2026}".to_string());
    }
    None
}

/// What the field says while the run is out.
pub fn progress(run: &Run, via: &str) -> String {
    if run.arrange.is_some() {
        return format!("Asking {via} to arrange the notes\u{2026}   Esc stops");
    }
    format!(
        "Looking at the pictures with {via} \u{00B7} {} of {} \u{00B7} request {} of {}\u{2026}   Esc stops",
        run.looked(),
        run.pictures(),
        (run.answered.len() + run.in_flight.len()).max(1),
        run.batches.len()
    )
}

/// "Show what is sent" for Write notes: each look request's pictures as they go and its text,
/// word for word, then the arrange request, whose lines of what the pictures show are the
/// model's own answers and so cannot be shown before the looks.
pub fn request_text(
    s: &crate::task_ui::TaskScreen,
    instruction: &str,
    headings: &[String],
) -> String {
    let (pictures, texts) = s.notes_parts();
    let mut out = String::new();
    let batches = batches(&pictures);
    for (k, batch) in batches.iter().enumerate() {
        out.push_str(&format!(
            "=== LOOK REQUEST {} of {} ===
SYSTEM:
{}

PICTURES, IN THIS ORDER:
",
            k + 1,
            batches.len(),
            notes_ai::LOOK_SYSTEM
        ));
        for &i in batch {
            let f = &s.files[i];
            out.push_str(&format!(
                "- {}: {}
",
                f.name,
                sent_as(&f.measure)
            ));
        }
        let sent: Vec<notes_ai::Sent> = batch
            .iter()
            .map(|&i| notes_ai::Sent {
                id: i,
                name: s.files[i].name.clone(),
            })
            .collect();
        out.push_str(
            "
PROMPT:
",
        );
        out.push_str(&notes_ai::look_prompt(instruction, &sent, s.notes.text_out));
        out.push_str(
            "

",
        );
    }
    if requests(pictures.len(), texts.len(), s.notes.how_many) > batches.len() {
        let files: Vec<notes_ai::TextFile> = texts
            .iter()
            .filter_map(|&i| {
                let f = &s.files[i];
                Some(notes_ai::TextFile {
                    name: f.name.clone(),
                    text: f.text.clone()?,
                })
            })
            .collect();
        out.push_str(&format!(
            "=== ARRANGE REQUEST: NO PICTURE IN IT ===
SYSTEM:
{}

PROMPT:
",
            notes_ai::ARRANGE_SYSTEM
        ));
        if !pictures.is_empty() {
            out.push_str(
                "(The lines of what each picture shows are the looks' answers: they are put in                  here when the looks are back.)

",
            );
        }
        out.push_str(&notes_ai::arrange_prompt(
            instruction,
            s.notes.how_many,
            &[],
            &files,
            headings,
        ));
        out.push('\n');
    }
    out
}

/// Where a picture pasted from the clipboard is kept until the window closes: the engine
/// reads pictures from a path. A note never links one here: it is always copied.
pub fn pasted_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("mdedit-pasted")
}

/// Whether Ctrl+V in the task window adds the clipboard's picture: only on Write notes, and
/// only when the clipboard has no text, which goes to the field as in any other paste (a
/// spreadsheet's cells offer both).
pub fn paste_is_a_picture(task: crate::task_ui::TaskKind, text: Option<&str>) -> bool {
    task == crate::task_ui::TaskKind::Notes && text.is_none_or(|t| t.trim().is_empty())
}

/// Whether `path` is a picture pasted from the clipboard.
pub fn is_pasted(path: &std::path::Path) -> bool {
    path.parent()
        .is_some_and(|d| mdshared_app::paths::same_path(d, &pasted_dir()))
}

/// The pasted pictures older than a day, left by a window that never closed (a crash): the
/// ones of a window still open elsewhere are younger.
pub fn stale_pasted(dir: &std::path::Path, now: std::time::SystemTime) -> Vec<std::path::PathBuf> {
    let day = std::time::Duration::from_secs(24 * 3600);
    std::fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    name.starts_with("Screenshot ")
                        && name.ends_with(".png")
                        && e.metadata()
                            .and_then(|m| m.modified())
                            .ok()
                            .and_then(|t| now.duration_since(t).ok())
                            .is_some_and(|age| age > day)
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default()
}

/// The most a kept thumbnail is on its long side, in pixels: twice the row's square, for a
/// screen at 200 %. The engine's is 160 px; it is made smaller once, when it arrives, so a
/// window of a hundred pictures holds about 2.5 MB of them, not 10.
pub const THUMB_PX: u32 = 80;

/// A straight-alpha RGBA picture made at most `max` px on its long side, each pixel the mean of
/// those under it (weighted by alpha, so a cut-out keeps no dark fringe). One that is already
/// small enough is kept as it is.
pub fn small(rgba: &[u8], w: u32, h: u32, max: u32) -> (Vec<u8>, u32, u32) {
    if w == 0 || h == 0 || rgba.len() < w as usize * h as usize * 4 {
        return (Vec::new(), 0, 0);
    }
    if w <= max && h <= max {
        return (rgba[..w as usize * h as usize * 4].to_vec(), w, h);
    }
    let (nw, nh) = if w >= h {
        (max, ((h as u64 * max as u64) / w as u64).max(1) as u32)
    } else {
        (((w as u64 * max as u64) / h as u64).max(1) as u32, max)
    };
    let mut out = vec![0u8; nw as usize * nh as usize * 4];
    for y in 0..nh {
        let (y0, y1) = (y * h / nh, ((y + 1) * h / nh).max(y * h / nh + 1));
        for x in 0..nw {
            let (x0, x1) = (x * w / nw, ((x + 1) * w / nw).max(x * w / nw + 1));
            let mut sum = [0u64; 4];
            for sy in y0..y1.min(h) {
                for sx in x0..x1.min(w) {
                    let k = (sy as usize * w as usize + sx as usize) * 4;
                    let a = u64::from(rgba[k + 3]);
                    for c in 0..3 {
                        sum[c] += u64::from(rgba[k + c]) * a;
                    }
                    sum[3] += a;
                }
            }
            let n = u64::from((y1.min(h) - y0) * (x1.min(w) - x0)).max(1);
            let k = (y as usize * nw as usize + x as usize) * 4;
            for c in 0..3 {
                out[k + c] = sum[c].checked_div(sum[3]).unwrap_or(0) as u8;
            }
            out[k + 3] = (sum[3] / n) as u8;
        }
    }
    (out, nw, nh)
}

#[cfg(test)]
#[path = "notes_task_tests.rs"]
mod tests;
