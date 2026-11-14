//! Autosave and crash recovery. While a document has unsaved changes, a recovery copy of it is
//! kept in the `recover` folder beside the session file, written on a thread after a pause in
//! typing (and at the latest every half minute while typing goes on). A normal exit removes the
//! folder's files (the session keeps the unsaved text itself, see `Docs::write_backups`), so
//! what is found at the next start is the text of a run that did not end normally.
//!
//! Nothing here runs on a keystroke: [`Recovery::poll`] looks at the tabs once per pass of the
//! event loop (a handful of integer comparisons), and the first write starts the thread.

use crate::docs::{Docs, Recovered};
use mdedit_core::Encoding;
use mdedit_core::limits::{RECOVERY_IDLE_MS, RECOVERY_MAX_BYTES, RECOVERY_MAX_MS};
use mdedit_core::recovery::{self, Header};
use mdedit_highlight::DocId;
use ropey::Rope;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The folder of recovery copies for a session file.
pub fn dir_of(session: &Path) -> Option<PathBuf> {
    session.parent().map(|d| d.join("recover"))
}

enum Job {
    Write {
        name: String,
        header: Header,
        text: Rope,
    },
    Remove(String),
}

impl Job {
    fn name(&self) -> &str {
        match self {
            Job::Write { name, .. } | Job::Remove(name) => name,
        }
    }
}

struct Writer {
    tx: Sender<Job>,
    handle: JoinHandle<()>,
}

fn spawn_writer(dir: PathBuf) -> Option<Writer> {
    let (tx, rx) = channel::<Job>();
    let handle = std::thread::Builder::new()
        .name("mdedit-recovery".into())
        .spawn(move || {
            while let Ok(first) = rx.recv() {
                // Only the newest job of a name matters when several are waiting.
                let mut batch = vec![first];
                batch.extend(rx.try_iter());
                for (i, job) in batch.iter().enumerate() {
                    if batch[i + 1..].iter().any(|j| j.name() == job.name()) {
                        continue;
                    }
                    match job {
                        Job::Write { name, header, text } => {
                            if let Err(e) = recovery::write(&dir, name, header, text) {
                                eprintln!("mdedit: could not write a recovery copy: {e}");
                            }
                        }
                        Job::Remove(name) => {
                            let _ = std::fs::remove_file(dir.join(name));
                        }
                    }
                }
            }
        })
        .ok()?;
    Some(Writer { tx, handle })
}

/// What was last seen of a document: its text's revision and its encoding.
type Key = (u64, Encoding);

#[derive(Default)]
struct Slot {
    /// A copy of this document is, or may be, in the folder.
    on_disk: bool,
    seen: Option<Key>,
    last_change: Option<Instant>,
    dirty_since: Option<Instant>,
    written: Option<Key>,
}

/// A recovery copy left by an earlier run.
pub struct Found {
    pub header: Header,
    pub file: PathBuf,
}

/// What [`Recovery::restore`] did.
#[derive(Default)]
pub struct Restored {
    /// Documents put back.
    pub count: usize,
    /// Tabs of files, which wait for their file to be read; the caller starts those loads.
    pub load: Vec<usize>,
}

pub struct Recovery {
    dir: Option<PathBuf>,
    /// Names this run's files: no earlier file can have the same name.
    tag: String,
    slots: HashMap<DocId, Slot>,
    writer: Option<Writer>,
}

impl Recovery {
    /// No recovery: a window without a session (`--no-session`, a shot).
    pub fn off() -> Recovery {
        Recovery::new(None)
    }

    pub fn new(dir: Option<PathBuf>) -> Recovery {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        Recovery {
            dir,
            tag: format!("{millis:x}"),
            slots: HashMap::new(),
            writer: None,
        }
    }

    fn name(&self, id: DocId) -> String {
        format!("{}-{id}.{}", self.tag, recovery::EXTENSION)
    }

    /// The copies an earlier run left, oldest name first. One listing of the folder; a file is
    /// read only when there is one. Copies that are not whole, and temporary files, are removed.
    pub fn scan(&self) -> Vec<Found> {
        let Some(dir) = &self.dir else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut names: Vec<(String, PathBuf)> = entries
            .flatten()
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
            .collect();
        names.sort();
        let mut out = Vec::new();
        for (name, file) in names {
            if recovery::is_recovery_name(&name) {
                match recovery::read_header(&file) {
                    Some(header) => out.push(Found { header, file }),
                    None => {
                        let _ = std::fs::remove_file(&file);
                    }
                }
            } else if name.ends_with(".tmp") {
                let _ = std::fs::remove_file(&file);
            }
        }
        out
    }

    /// Puts what an earlier run left back into `docs` as modified tabs. A file that is still
    /// there gets a tab that puts the text back once the file is read; a document without a
    /// file, or whose file is gone, opens with its text as an untitled one.
    pub fn restore(&mut self, docs: &mut Docs) -> Restored {
        let mut out = Restored::default();
        for found in self.scan() {
            let h = &found.header;
            match h.path.as_ref().filter(|p| p.is_file()) {
                Some(path) => {
                    let k = docs.add_pending(path);
                    // The session's own backup of this tab is as new: it stays the only one.
                    if docs.tabs[k].restore.is_some() {
                        let _ = std::fs::remove_file(&found.file);
                        continue;
                    }
                    let id = docs.tabs[k].id;
                    docs.tabs[k].restore = Some(self.adopt(&found, id));
                    docs.tabs[k].recovered = Some(Recovered {
                        base: h.stamp(),
                        encoding: h.encoding.clone(),
                    });
                    out.load.push(k);
                    out.count += 1;
                }
                None => {
                    let Some((_, text)) = recovery::read(&found.file) else {
                        let _ = std::fs::remove_file(&found.file);
                        continue;
                    };
                    let k = docs.restore_untitled(h.untitled, &text);
                    if let Some(r) = docs.tabs[k].ready_mut()
                        && r.recovered_encoding(&h.encoding)
                    {
                        docs.tabs[k].encoding_changed = true;
                    }
                    let id = docs.tabs[k].id;
                    self.adopt(&found, id);
                    out.count += 1;
                }
            }
        }
        out
    }

    /// Renames a found copy to the name this run gives the document's, so the next write
    /// replaces it atomically and a removal finds it. The copy's path afterwards.
    fn adopt(&mut self, found: &Found, id: DocId) -> PathBuf {
        let Some(dir) = &self.dir else {
            return found.file.clone();
        };
        let to = dir.join(self.name(id));
        if std::fs::rename(&found.file, &to).is_ok() {
            self.slots.entry(id).or_default().on_disk = true;
            to
        } else {
            found.file.clone()
        }
    }

    fn send(&mut self, job: Job) {
        let Some(dir) = self.dir.clone() else {
            return;
        };
        if self.writer.is_none() {
            self.writer = spawn_writer(dir);
        }
        if let Some(w) = &self.writer {
            let _ = w.tx.send(job);
        }
    }

    /// Looks at the tabs: writes the copy of one that changed and has been left alone for
    /// [`RECOVERY_IDLE_MS`] (or has gone [`RECOVERY_MAX_MS`] with changes unwritten), removes the
    /// copy of one that is saved, closed or no longer changed. Returns when to look again.
    pub fn poll(&mut self, docs: &Docs, now: Instant) -> Option<Instant> {
        self.dir.as_ref()?;
        let (idle, longest) = (
            Duration::from_millis(RECOVERY_IDLE_MS),
            Duration::from_millis(RECOVERY_MAX_MS),
        );
        let mut next: Option<Instant> = None;
        let mut jobs = Vec::new();
        for d in &docs.tabs {
            // Text still to be put back from an earlier run is kept as it is.
            if d.restore.is_some() {
                continue;
            }
            let ready = d.ready().filter(|r| {
                !r.read_only && !r.info.lossy && r.buffer.len_bytes() <= RECOVERY_MAX_BYTES
            });
            let (Some(r), true) = (ready, d.modified()) else {
                if self.slots.remove(&d.id).is_some_and(|s| s.on_disk) {
                    jobs.push(Job::Remove(self.name(d.id)));
                }
                continue;
            };
            let name = self.name(d.id);
            let slot = self.slots.entry(d.id).or_default();
            let key = (r.buffer.revision(), r.info.encoding);
            if slot.seen != Some(key) {
                slot.seen = Some(key);
                slot.last_change = Some(now);
                slot.dirty_since.get_or_insert(now);
            }
            if slot.written == Some(key) {
                continue;
            }
            let last = slot.last_change.unwrap_or(now);
            let first = slot.dirty_since.unwrap_or(now);
            let due = (last + idle).min(first + longest);
            if now < due {
                next = Some(next.map_or(due, |n| n.min(due)));
                continue;
            }
            slot.written = Some(key);
            slot.dirty_since = None;
            slot.on_disk = true;
            let mut header = Header {
                path: d.path.clone(),
                untitled: d.untitled,
                encoding: r.info.encoding.label(),
                eol: r.info.eol.style().label().to_string(),
                disk: None,
                bytes: 0,
            };
            header.set_stamp(&r.info.stamp);
            jobs.push(Job::Write {
                name,
                header,
                text: r.buffer.snapshot(),
            });
        }
        // A tab that was closed takes its copy with it.
        let gone: Vec<DocId> = self
            .slots
            .keys()
            .copied()
            .filter(|id| !docs.tabs.iter().any(|d| d.id == *id))
            .collect();
        for id in gone {
            if self.slots.remove(&id).is_some_and(|s| s.on_disk) {
                jobs.push(Job::Remove(self.name(id)));
            }
        }
        for job in jobs {
            self.send(job);
        }
        next
    }

    /// Waits for the thread to finish what it was given and stops it (it starts again when
    /// something is written next).
    pub fn flush(&mut self) {
        if let Some(w) = self.writer.take() {
            drop(w.tx);
            let _ = w.handle.join();
        }
    }

    /// A normal exit: the copies are removed, except those of tabs whose text was never put
    /// back (it is in no other place; the next start offers it).
    pub fn finish(&mut self, docs: &Docs) {
        self.flush();
        let Some(dir) = &self.dir else { return };
        let keep: Vec<&Path> = docs
            .tabs
            .iter()
            .filter_map(|d| d.restore.as_deref())
            .collect();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            if !keep.contains(&e.path().as_path()) {
                let _ = std::fs::remove_file(e.path());
            }
        }
        // The folder goes too once it is empty.
        let _ = std::fs::remove_dir(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::EditOp;
    use mdedit_core::selection::Selection;
    use std::sync::Arc;

    fn docs() -> Docs {
        Docs::new(1254, Arc::new(|_| {}))
    }

    /// A folder with `file.txt` in it, and a `Recovery` over the folder `recover` beside.
    fn setup(name: &str, text: &[u8]) -> (PathBuf, PathBuf, Recovery) {
        let dir = mdshared_app::paths::canonical_or_absolute(&std::env::temp_dir())
            .join(format!("mdedit-recover-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("file.txt");
        std::fs::write(&path, text).unwrap();
        let rec = Recovery::new(Some(dir.join("recover")));
        (dir, path, rec)
    }

    fn text_of(d: &Docs, i: usize) -> String {
        let r = d.tabs[i].ready().unwrap();
        let mut s = String::new();
        r.buffer.text(0..r.buffer.len_bytes(), &mut s);
        s
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir.join("recover"))
            .map(|r| {
                r.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    /// A copy is written after a pause, never sooner, and at the latest after the longest wait
    /// however long typing goes on; saving, undoing and closing remove it.
    #[test]
    fn a_copy_follows_the_text_and_goes_with_the_changes() {
        let (dir, path, mut rec) = setup("follow", b"one\n");
        let mut d = docs();
        let i = d.open(&path);
        d.load_now(i, None, false);
        let t0 = Instant::now();
        // Nothing modified: nothing written, nothing to wait for.
        assert_eq!(rec.poll(&d, t0), None);
        rec.flush();
        assert!(files(&dir).is_empty());

        assert!(d.edit(EditOp::Insert("a"), None));
        // Noticed now, due after the pause.
        assert_eq!(rec.poll(&d, t0), Some(t0 + secs(3)));
        assert_eq!(rec.poll(&d, t0 + secs(2)), Some(t0 + secs(3)));
        rec.flush();
        assert!(files(&dir).is_empty(), "not before the pause");
        // Typing goes on: each change pushes the pause back ...
        assert!(d.edit(EditOp::Insert("b"), None));
        assert_eq!(rec.poll(&d, t0 + secs(2)), Some(t0 + secs(5)));
        assert!(d.edit(EditOp::Insert("c"), None));
        assert_eq!(rec.poll(&d, t0 + secs(4)), Some(t0 + secs(7)));
        // ... but not past the longest wait since the first change.
        let mut now = t0 + secs(4);
        while now < t0 + secs(27) {
            now += secs(2);
            assert!(d.edit(EditOp::Insert("x"), None));
            rec.poll(&d, now);
        }
        rec.flush();
        assert!(files(&dir).is_empty());
        assert!(d.edit(EditOp::Insert("y"), None));
        rec.poll(&d, t0 + secs(30));
        rec.flush();
        let list = files(&dir);
        assert_eq!(list.len(), 1, "{list:?}");
        let (h, body) = recovery::read(&dir.join("recover").join(&list[0])).unwrap();
        assert_eq!(h.path.as_ref(), Some(&path));
        assert_eq!(body, text_of(&d, i));
        assert_eq!(h.encoding, "UTF-8");
        assert!(h.stamp().is_some());

        // Saved: the copy goes at the next look.
        d.save(i, false).unwrap();
        assert_eq!(rec.poll(&d, t0 + secs(31)), None);
        rec.flush();
        assert!(files(&dir).is_empty());

        // Edited again, then undone to the file's text: gone as well.
        assert!(d.edit(EditOp::Insert("q"), None));
        rec.poll(&d, t0 + secs(40));
        rec.poll(&d, t0 + secs(50));
        rec.flush();
        assert_eq!(files(&dir).len(), 1);
        d.edit(EditOp::Undo, None);
        rec.poll(&d, t0 + secs(51));
        rec.flush();
        assert!(files(&dir).is_empty());

        // A closed tab takes its copy along.
        assert!(d.edit(EditOp::Insert("z"), None));
        rec.poll(&d, t0 + secs(60));
        rec.poll(&d, t0 + secs(70));
        rec.flush();
        assert_eq!(files(&dir).len(), 1);
        d.close(i, None);
        rec.poll(&d, t0 + secs(71));
        rec.flush();
        assert!(files(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_is_not_worth_a_copy_gets_none() {
        let (dir, _path, mut rec) = setup("none", b"x");
        let mut d = docs();
        let n = d.new_untitled();
        d.activate(n);
        // An untitled document that was never typed in.
        let t0 = Instant::now();
        assert_eq!(rec.poll(&d, t0), None);
        // A read-only copy (a comparison's saved side) is never kept.
        assert!(d.edit(EditOp::Insert("note"), None));
        d.tabs[n].ready_mut().unwrap().read_only = true;
        rec.poll(&d, t0);
        rec.poll(&d, t0 + secs(10));
        rec.flush();
        assert!(files(&dir).is_empty());
        // Off: no folder, no work.
        let mut off = Recovery::off();
        assert_eq!(off.poll(&d, t0), None);
        assert!(off.scan().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_crash_leaves_text_that_comes_back_modified() {
        let (dir, path, mut rec) = setup("crash", b"one\r\ntwo\r\n");
        let mut d = docs();
        let i = d.open(&path);
        d.load_now(i, None, false);
        d.tabs[i].ready_mut().unwrap().view.sel = Selection::at(0);
        assert!(d.edit(EditOp::Insert("zero\n"), None));
        let n = d.new_untitled();
        d.activate(n);
        assert!(d.edit(EditOp::Insert("note"), None));
        let t0 = Instant::now();
        rec.poll(&d, t0);
        rec.poll(&d, t0 + secs(5));
        // The process dies here: the thread finishes what it was given, nothing else runs.
        rec.flush();
        assert_eq!(files(&dir).len(), 2);
        drop(rec);

        // The next start.
        let mut rec = Recovery::new(Some(dir.join("recover")));
        let mut e = docs();
        let got = rec.restore(&mut e);
        assert_eq!((got.count, got.load.len()), (2, 1));
        assert_eq!(e.tabs.len(), 2);
        let k = got.load[0];
        assert_eq!(e.tabs[k].path.as_ref(), Some(&path));
        assert!(
            e.tabs[k].modified(),
            "the tab shows its mark before it is read"
        );
        // The untitled document has its text at once.
        let u = (0..2).find(|&x| x != k).unwrap();
        assert_eq!((text_of(&e, u).as_str(), e.tabs[u].untitled), ("note", 1));
        assert!(e.tabs[u].modified());
        // The file is read: the text goes back, the file on disk is untouched, Undo gives it.
        e.load_now(k, None, false);
        assert_eq!(text_of(&e, k), "zero\none\r\ntwo\r\n");
        assert!(e.tabs[k].modified() && e.tabs[k].restore.is_none());
        assert!(!e.tabs[k].changed_on_disk);
        assert_eq!(std::fs::read(&path).unwrap(), b"one\r\ntwo\r\n");
        e.activate(k);
        assert!(e.edit(EditOp::Undo, None));
        assert_eq!(text_of(&e, k), "one\r\ntwo\r\n");
        assert!(!e.tabs[k].modified());
        // The copies were taken over under this run's names; nothing else was added.
        assert_eq!(files(&dir).len(), 2);
        // A normal exit removes them, and the folder.
        rec.finish(&e);
        assert!(files(&dir).is_empty() && !dir.join("recover").exists());
        // Nothing left to find at the start after that.
        assert!(Recovery::new(Some(dir.join("recover"))).scan().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_changed_since_keeps_both() {
        let (dir, path, mut rec) = setup("changed", b"one\n");
        let mut d = docs();
        let i = d.open(&path);
        d.load_now(i, None, false);
        assert!(d.edit(EditOp::Insert("mine "), None));
        let t0 = Instant::now();
        rec.poll(&d, t0);
        rec.poll(&d, t0 + secs(5));
        rec.flush();
        drop(rec);
        // Another program rewrites the file before the next start.
        std::fs::write(&path, b"theirs, and longer\n").unwrap();

        let mut rec = Recovery::new(Some(dir.join("recover")));
        let mut e = docs();
        let got = rec.restore(&mut e);
        let k = got.load[0];
        e.load_now(k, None, false);
        assert_eq!(text_of(&e, k), "mine one\n");
        assert!(e.tabs[k].modified() && e.tabs[k].changed_on_disk);
        assert_eq!(
            e.tabs[k].banner(),
            Some(crate::commands::strings::FILE_CHANGED)
        );
        // Saving asks before it overwrites, as it does for any file changed on disk.
        assert!(matches!(
            e.save(k, false),
            Err(crate::docs::SaveFailed::Error(
                mdedit_core::SaveError::ChangedOnDisk
            ))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"theirs, and longer\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_is_gone_comes_back_as_an_untitled_document() {
        let (dir, path, mut rec) = setup("gone", b"one\n");
        let mut d = docs();
        let i = d.open(&path);
        d.load_now(i, None, false);
        assert!(d.edit(EditOp::Insert("mine "), None));
        let t0 = Instant::now();
        rec.poll(&d, t0);
        rec.poll(&d, t0 + secs(5));
        rec.flush();
        drop(rec);
        std::fs::remove_file(&path).unwrap();
        let mut rec = Recovery::new(Some(dir.join("recover")));
        let mut e = docs();
        let got = rec.restore(&mut e);
        assert_eq!((got.count, got.load.len()), (1, 0));
        assert_eq!(text_of(&e, 0), "mine one\n");
        assert!(e.tabs[0].path.is_none() && e.tabs[0].modified());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn damaged_and_foreign_files_are_dropped_and_the_session_backup_wins() {
        let (dir, path, rec) = setup("damaged", b"one\n");
        let r = dir.join("recover");
        std::fs::create_dir_all(&r).unwrap();
        std::fs::write(r.join("1-1.rec"), b"junk").unwrap();
        std::fs::write(r.join("1-2.rec.tmp"), b"half").unwrap();
        std::fs::write(r.join("notes.txt"), b"mine").unwrap();
        assert!(rec.scan().is_empty());
        assert_eq!(files(&dir), ["notes.txt"], "only what is not ours stays");

        // A whole copy of a tab whose text the session's backup already holds is not taken twice.
        let h = Header {
            path: Some(path.clone()),
            untitled: 0,
            encoding: "UTF-8".into(),
            eol: "Unix (LF)".into(),
            disk: None,
            bytes: 0,
        };
        recovery::write(&r, "9-9.rec", &h, &Rope::from_str("copy")).unwrap();
        let mut rec = rec;
        let mut d = docs();
        let k = d.add_pending(&path);
        d.tabs[k].restore = Some(dir.join("backup").join("1.txt"));
        let got = rec.restore(&mut d);
        assert_eq!(got.count, 0);
        assert_eq!(d.tabs[k].restore, Some(dir.join("backup").join("1.txt")));
        assert_eq!(files(&dir), ["notes.txt"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_exit_before_the_text_was_put_back_keeps_its_copy() {
        let (dir, path, mut rec) = setup("pending", b"one\n");
        let mut d = docs();
        let i = d.open(&path);
        d.load_now(i, None, false);
        assert!(d.edit(EditOp::Insert("mine "), None));
        let t0 = Instant::now();
        rec.poll(&d, t0);
        rec.poll(&d, t0 + secs(5));
        rec.flush();
        drop(rec);
        let mut rec = Recovery::new(Some(dir.join("recover")));
        let mut e = docs();
        rec.restore(&mut e);
        // Closed before the file was read: the copy is the only place the text is.
        rec.finish(&e);
        assert_eq!(files(&dir).len(), 1);
        let again = Recovery::new(Some(dir.join("recover")));
        assert_eq!(again.scan().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_document_over_the_limit_gets_no_copy() {
        let (dir, _path, mut rec) = setup("large", b"x");
        let mut d = docs();
        let n = d.new_untitled();
        d.activate(n);
        let big = "0123456789abcdef".repeat(RECOVERY_MAX_BYTES / 16 + 1);
        assert!(d.edit(EditOp::Insert(&big), None));
        let t0 = Instant::now();
        assert_eq!(rec.poll(&d, t0), None);
        rec.poll(&d, t0 + secs(60));
        rec.flush();
        assert!(files(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_encoding_of_an_unsaved_document_comes_back() {
        let (dir, _path, mut rec) = setup("encoding", b"x");
        let mut d = docs();
        let n = d.new_untitled();
        d.activate(n);
        assert!(d.edit(EditOp::Insert("caf\u{e9}"), None));
        assert!(d.save_in(d.legacy()));
        let t0 = Instant::now();
        rec.poll(&d, t0);
        rec.poll(&d, t0 + secs(5));
        rec.flush();
        drop(rec);
        let mut rec = Recovery::new(Some(dir.join("recover")));
        let mut e = docs();
        rec.restore(&mut e);
        let r = e.tabs[0].ready().unwrap();
        assert_eq!(r.info.encoding, e.legacy());
        assert_eq!(text_of(&e, 0), "caf\u{e9}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
