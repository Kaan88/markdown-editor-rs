//! Write notes' write, its pure half (`docs/ai-pictures-plan.md` phase V5, sections 9, 10 and
//! 11): where each note goes, which pictures are copied beside it and under what name, and the
//! limits of a run, all worked out before a byte is written. The window (`app/notes_write.rs`)
//! asks the disk and does the writing; nothing here reads or writes a file.
//!
//! A picture is copied into `pictures/` in the folder of each note that shows it, once per
//! folder: two notes of one section share the copy, two notes of two sections have one each, so
//! the link is the same short `pictures/<name>` at every depth (section 9). A picture no kept note
//! shows is not copied.

use crate::notes_ai::{Draft, Look, NOTES_MAX, PICTURES_DIR, numbered, picture_name};
use crate::runs;
use mdshared_app::library::note_folder;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The most bytes one note may hold as it is written (section 11).
pub const NOTE_BYTES_MAX: usize = 200 * 1024;
/// The most copies one run makes, and their bytes together (section 11).
pub const COPIES_MAX: usize = 100;
pub const COPY_BYTES_MAX: u64 = 200 * 1000 * 1000;
/// The most bytes of one copy: what undo reads to know it is the run's own
/// (`runs::DIGEST_MAX_BYTES`), so a copy undo could not check is never made.
pub const COPY_FILE_MAX: u64 = runs::DIGEST_MAX_BYTES;
/// The extensions a copy may have: the pictures a run's undo removes. A copy with any other would
/// make the whole record one undo refuses to act on.
pub const COPY_EXTENSIONS: [&str; 10] = [
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "avif", "tif", "tiff",
];
/// The numbered names tried for a copy whose name is taken: `-2` to `-99` (section 9).
const NUMBERS_MAX: u32 = 99;

/// A picture of the window, by its place there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Original {
    pub path: PathBuf,
    /// Its size when the window measured it: what the limits count.
    pub size: u64,
    /// A picture pasted from the clipboard: its file is in a temp folder that goes when the
    /// window closes, so it is copied beside its note even with "Copy the pictures" off.
    pub pasted: bool,
}

/// What is at a name a copy would take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Taken {
    Free,
    /// A file with the original's very bytes: it is used, and not written again.
    Same,
    /// Another file: the next number is tried.
    Other,
}

/// One copy the run makes, or finds made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Copy {
    /// The picture's place in the window.
    pub id: usize,
    pub from: PathBuf,
    pub to: PathBuf,
    pub size: u64,
    /// The same bytes are at `to` already: nothing is written, and undo leaves the file.
    pub there: bool,
}

impl Copy {
    /// The copy's file name, as the note's link holds it under `pictures/`.
    pub fn name(&self) -> String {
        self.to
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// One note of the plan, where it goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteAt {
    /// Its place in the plan's notes.
    pub index: usize,
    pub folder: PathBuf,
    pub path: PathBuf,
}

/// What a write does, worked out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Planned {
    pub notes: Vec<NoteAt>,
    pub copies: Vec<Copy>,
}

impl Planned {
    /// The copy of picture `id` beside the notes of `folder`.
    pub fn copy_of(&self, folder: &Path, id: usize) -> Option<&Copy> {
        let dir = folder.join(PICTURES_DIR);
        self.copies
            .iter()
            .find(|c| c.id == id && c.to.parent() == Some(dir.as_path()))
    }

    /// The copies written by the run, not found there.
    pub fn written(&self) -> impl Iterator<Item = &Copy> {
        self.copies.iter().filter(|c| !c.there)
    }

    /// The folders the run needs, parents first: each note's, then each copy's `pictures`.
    pub fn folders(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = self.notes.iter().map(|n| n.folder.clone()).collect();
        out.extend(
            self.copies
                .iter()
                .filter_map(|c| c.to.parent().map(Path::to_path_buf)),
        );
        out.sort_by_key(|f| f.components().count());
        let mut seen: Vec<PathBuf> = Vec::new();
        out.retain(|f| {
            let new = !seen.contains(f);
            if new {
                seen.push(f.clone());
            }
            new
        });
        out
    }
}

/// How the copies are named: the options of section 12 and the names the owner gave.
pub struct Naming<'a> {
    /// "Copy the pictures beside the notes": off, nothing is copied.
    pub copy: bool,
    /// "Name the copies after what they show".
    pub rename: bool,
    pub looks: &'a [Look],
    /// The plan card's renames: a picture's place to its copy's file name (`Run::copy_names`).
    pub names: &'a HashMap<usize, String>,
}

/// The name a copy of the picture at `id` starts from (section 9): the owner's name from the
/// plan card when there is one, else the look's title when "Name the copies after what they
/// show" is on, else the original's own name; always with the original's extension, since the
/// bytes are the original's, and always through the name rule, so nothing typed or answered
/// can name a path.
pub fn copy_name(id: usize, original: &Path, naming: &Naming) -> String {
    let ext = original
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let own = original
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(given) = naming.names.get(&id).map(|n| n.trim()) {
        // The owner's name with its extension, or without one: the stem is what they typed.
        let stem = match Path::new(given).extension() {
            Some(e) if e.to_string_lossy().eq_ignore_ascii_case(&ext) => Path::new(given)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            _ => given.to_string(),
        };
        if !stem.trim().is_empty() {
            return picture_name(None, &format!("{stem}.x"), &ext);
        }
    }
    let title = naming
        .rename
        .then(|| naming.looks.iter().find(|l| l.id == id))
        .flatten()
        .map(|l| l.title.as_str());
    picture_name(title, &own, &ext)
}

/// The plan of a write: each note in its section's folder under `home`, and each picture a kept
/// note shows copied into that folder's `pictures/` once, at a name free there or holding the
/// same bytes (`taken`, asked of the disk by the caller: the name, then the original). `Err` says
/// why the run cannot be written, with both numbers when a limit is passed (section 11).
pub fn plan(
    notes: &[Draft],
    home: &Path,
    originals: &dyn Fn(usize) -> Option<Original>,
    naming: &Naming,
    taken: &dyn Fn(&Path, &Path) -> Taken,
) -> Result<Planned, String> {
    if notes.is_empty() {
        return Err("no note is left to write.".to_string());
    }
    if notes.len() > NOTES_MAX {
        return Err(format!(
            "{} notes is more than the {NOTES_MAX} one run writes.",
            notes.len()
        ));
    }
    let mut planned = Planned::default();
    for (index, note) in notes.iter().enumerate() {
        let folder = note_folder(home, &note.sections);
        let path = folder.join(note.file_name());
        if planned.notes.iter().any(|n| same_name(&n.path, &path)) {
            return Err(format!(
                "two notes would be {}: rename one on the plan.",
                shown(&path)
            ));
        }
        planned.notes.push(NoteAt {
            index,
            folder,
            path,
        });
    }
    let mut bytes: u64 = 0;
    for at in planned.notes.clone() {
        let Some(note) = notes.get(at.index) else {
            continue;
        };
        let dir = at.folder.join(PICTURES_DIR);
        for id in note.pictures() {
            if planned.copy_of(&at.folder, id).is_some() {
                continue;
            }
            let Some(original) = originals(id).filter(|o| naming.copy || o.pasted) else {
                continue;
            };
            let ext = original
                .path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if !COPY_EXTENSIONS.contains(&ext.as_str()) {
                return Err(format!(
                    "{} is not a kind of picture a run copies. Leave it out, or turn off \u{201C}Copy the pictures beside the notes\u{201D}.",
                    shown(&original.path)
                ));
            }
            if original.size > COPY_FILE_MAX {
                return Err(format!(
                    "{} is {} MB, more than the {} MB a copy may be.",
                    shown(&original.path),
                    original.size.div_ceil(1_000_000),
                    COPY_FILE_MAX / 1_000_000
                ));
            }
            let base = copy_name(id, &original.path, naming);
            let mut chosen = None;
            for n in 1..=NUMBERS_MAX {
                let name = if n == 1 {
                    base.clone()
                } else {
                    numbered(&base, n)
                };
                let to = dir.join(&name);
                if planned.copies.iter().any(|c| same_name(&c.to, &to)) {
                    continue;
                }
                match taken(&to, &original.path) {
                    Taken::Free => chosen = Some((to, false)),
                    Taken::Same => chosen = Some((to, true)),
                    Taken::Other => continue,
                }
                break;
            }
            let Some((to, there)) = chosen else {
                return Err(format!(
                    "every name for a copy of {} in {} is taken.",
                    shown(&original.path),
                    dir.display()
                ));
            };
            if !there {
                bytes = bytes.saturating_add(original.size);
            }
            planned.copies.push(Copy {
                id,
                from: original.path,
                to,
                size: original.size,
                there,
            });
        }
    }
    let written = planned.written().count();
    if written > COPIES_MAX {
        return Err(format!(
            "{written} copies is more than the {COPIES_MAX} one run makes."
        ));
    }
    if bytes > COPY_BYTES_MAX {
        return Err(format!(
            "the copies come to {} MB, more than the {} MB one run copies.",
            bytes.div_ceil(1_000_000),
            COPY_BYTES_MAX / 1_000_000
        ));
    }
    Ok(planned)
}

/// Whether a record is one of Write notes': the shared record's `notes` kind (plan section 10).
/// A `markdown` record is 5.8's Markdown note, however many notes or pictures it holds.
pub fn is_notes_run(record: &runs::Record) -> bool {
    record.kind == runs::Kind::Notes
}

/// Two paths that name one file on a disk that does not tell case apart.
fn same_name(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .to_lowercase()
        .eq(&b.to_string_lossy().to_lowercase())
}

fn shown(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
#[path = "notes_write_tests.rs"]
mod tests;
