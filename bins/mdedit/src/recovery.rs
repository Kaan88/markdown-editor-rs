//! Autosave and crash recovery. While a document has unsaved changes, a recovery copy of it is
//! kept in the `recover` folder beside the session file, written on a thread after a pause in
//! typing (and at the latest every half minute while typing goes on). A normal exit removes the
//! folder's files (the session keeps the unsaved text itself, see `Docs::write_backups`), so
//! what is found at the next start is the text of a run that did not end normally.
//!
//! Nothing here runs on a keystroke: [`Recovery::poll`] looks at the tabs once per pass of the
//! event loop (a handful of integer comparisons), and the first write starts the thread.

use crate::docs::{Docs, Recovered};
use crate::session::Unsaved;
use mdedit_core::Encoding;
use mdedit_core::limits::{RECOVERY_IDLE_MS, RECOVERY_MAX_BYTES, RECOVERY_MAX_MS};
use mdedit_core::recovery::{self, Header};
use mdedit_highlight::DocId;
use ropey::Rope;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
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
        /// The revision written, handed back when the write fails.
        key: Key,
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

/// Writes that failed, by name and revision: the next look tries them again.
type Failed = Arc<Mutex<Vec<(String, Key)>>>;

/// Writes sent and not done yet.
type InFlight = Arc<AtomicUsize>;

fn spawn_writer(dir: PathBuf, failed: Failed, in_flight: InFlight) -> Option<Writer> {
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
                        Job::Write {
                            name,
                            header,
                            text,
                            key,
                        } => {
                            if let Err(e) = recovery::write(&dir, name, header, text) {
                                eprintln!("mdedit: could not write a recovery copy: {e}");
                                if let Ok(mut f) = failed.lock() {
                                    f.push((name.clone(), *key));
                                }
                            }
                        }
                        Job::Remove(name) => {
                            let _ = std::fs::remove_file(dir.join(name));
                        }
                    }
                }
                // Done, written or failed (a failure is listed first, so a look that sees
                // nothing in flight sees it).
                let writes = batch
                    .iter()
                    .filter(|j| matches!(j, Job::Write { .. }))
                    .count();
                in_flight.fetch_sub(writes, Ordering::AcqRel);
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
    /// When the copy was last written, if the system says.
    pub modified: Option<SystemTime>,
}

/// Which copy in `found` covers each session backup in `unsaved` (holds newer text of the same
/// draft, so the backup is not opened as well), by index. A copy covers a backup only when it is
/// certain (audit 1 round 4, #1, #4, #5, #9):
///
/// * the same draft id in both; or, for a copy and a backup from before 1.1 (neither has an id),
///   the same file (as the tabs compare files: `same_path`) or without a file the same number;
/// * one copy for that backup and one backup for that copy;
/// * the copy at least as new as the backup (`backup_time`, its file's time). A backup written at
///   a later normal exit than the copy holds the newer text.
///
/// Whatever is not certain is not covered: both texts open, one as an extra draft, and nothing is
/// deleted on a guess.
///
/// A backup from before 1.1 has no id in the session; `legacy` gives it the one it is restored
/// under ([`legacy_draft`]), which its tab's copy then names (audit 1 round 5).
pub fn cover(
    found: &[Found],
    unsaved: &[Unsaved],
    backup_time: impl Fn(&Unsaved) -> Option<SystemTime>,
    legacy: impl Fn(&Unsaved) -> Option<u64>,
) -> Vec<Option<usize>> {
    let same = |f: &Found, u: &Unsaved| match (f.header.draft, u.draft) {
        (Some(a), Some(b)) => a == b,
        (Some(a), None) => legacy(u) == Some(a),
        (None, None) => match (&f.header.path, &u.path) {
            (Some(a), Some(b)) => mdshared_app::paths::same_path(a, b),
            (None, None) => f.header.untitled == u.untitled,
            _ => false,
        },
        _ => false,
    };
    unsaved
        .iter()
        .map(|u| {
            let mut copies = (0..found.len()).filter(|&i| same(&found[i], u));
            let (Some(i), None) = (copies.next(), copies.next()) else {
                return None;
            };
            let backups = unsaved.iter().filter(|v| same(&found[i], v)).count();
            let newer = match (found[i].modified, backup_time(u)) {
                (Some(c), Some(b)) => c >= b,
                _ => false,
            };
            (backups == 1 && newer).then_some(i)
        })
        .collect()
}

/// The draft id a session backup from before 1.1 (no id of its own) is restored under: the same
/// at every start while the backup file is unchanged, so a recovery copy written for its tab
/// names it even when the session could not be written with the id since (audit 1 round 5: a
/// session that cannot be written no longer makes one more draft at each start). Made of the
/// backup's name, the document it stands for, and the file's size and time. `None` when the file
/// cannot be looked at.
pub fn legacy_draft(file: &Path, u: &Unsaved) -> Option<u64> {
    let meta = std::fs::metadata(file).ok()?;
    let time = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    // FNV-1a, 64 bits.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat(u.file.as_bytes());
    eat(&[0]);
    if let Some(p) = &u.path {
        eat(p.to_string_lossy().as_bytes());
    }
    eat(&[0]);
    eat(&u.untitled.to_le_bytes());
    eat(&meta.len().to_le_bytes());
    eat(&time.to_le_bytes());
    Some(h.max(1))
}

/// What [`Recovery::restore`] did.
#[derive(Default)]
pub struct Restored {
    /// Documents put back.
    pub count: usize,
    /// Tabs of files, which wait for their file to be read; the caller starts those loads.
    pub load: Vec<usize>,
    /// For each copy given, in order, the tab it came back as; `None` for a copy that could not be
    /// read. A session backup the copy covers is that tab's too.
    pub tabs: Vec<Option<DocId>>,
}

pub struct Recovery {
    dir: Option<PathBuf>,
    /// Names this run's files: no earlier file can have the same name.
    tag: String,
    slots: HashMap<DocId, Slot>,
    writer: Option<Writer>,
    failed: Failed,
    in_flight: InFlight,
    /// Copies that could not be read at the start: kept, whatever the exit, for the next start
    /// (audit 1 round 4, #3).
    unreadable: Vec<PathBuf>,
    /// Tests: the writer thread cannot be started.
    #[cfg(test)]
    no_writer: bool,
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
            failed: Failed::default(),
            in_flight: InFlight::default(),
            unreadable: Vec::new(),
            #[cfg(test)]
            no_writer: false,
        }
    }

    fn name(&self, id: DocId) -> String {
        format!("{}-{id}.{}", self.tag, recovery::EXTENSION)
    }

    /// The copies an earlier run left, oldest name first. One listing of the folder; a file is
    /// read only when there is one. Copies that are not whole, and temporary files, are removed;
    /// one that cannot be read now (another program has it open) is kept for the next start.
    pub fn scan(&mut self) -> Vec<Found> {
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
                match recovery::read_header_checked(&file) {
                    Ok(Some(header)) => {
                        let modified = std::fs::metadata(&file).and_then(|m| m.modified()).ok();
                        out.push(Found {
                            header,
                            file,
                            modified,
                        })
                    }
                    Ok(None) => {
                        let _ = std::fs::remove_file(&file);
                    }
                    Err(e) => {
                        eprintln!("mdedit: a recovery copy cannot be read now ({e}); kept");
                        self.unreadable.push(file);
                    }
                }
            } else if name.ends_with(".tmp") {
                let _ = std::fs::remove_file(&file);
            }
        }
        out
    }

    /// Puts the copies `found` (from `scan`, left by an earlier run) back into `docs` as modified
    /// tabs. A file that is still there gets a tab that puts the text back once the file is
    /// read; a document without a file, or whose file is gone, opens with its text as an
    /// untitled one.
    pub fn restore(&mut self, docs: &mut Docs, found: Vec<Found>) -> Restored {
        let mut out = Restored::default();
        for found in found {
            let h = &found.header;
            out.tabs.push(None);
            // A file tab that already takes another copy's text: this one opens as a draft of
            // its own, so neither is lost.
            let file_tab = h
                .path
                .as_ref()
                .filter(|p| p.is_file())
                .map(|p| docs.add_pending(p))
                .filter(|&k| docs.tabs[k].restore.is_none());
            let k = match file_tab {
                Some(k) => {
                    // A session backup this tab has is older than the copy, which a later run
                    // wrote: the copy replaces it and is never deleted for it.
                    let id = docs.tabs[k].id;
                    docs.tabs[k].restore = Some(self.adopt(&found, id));
                    docs.tabs[k].recovered = Some(Recovered {
                        base: h.stamp(),
                        encoding: h.encoding.clone(),
                    });
                    out.load.push(k);
                    k
                }
                None => {
                    let text = match recovery::read_checked(&found.file) {
                        Ok(Some((_, text))) => text,
                        Ok(None) => {
                            let _ = std::fs::remove_file(&found.file);
                            continue;
                        }
                        Err(e) => {
                            eprintln!("mdedit: a recovery copy cannot be read now ({e}); kept");
                            self.unreadable.push(found.file.clone());
                            continue;
                        }
                    };
                    let k = docs.restore_untitled(h.untitled, &text);
                    if let Some(r) = docs.tabs[k].ready_mut()
                        && r.recovered_encoding(&h.encoding)
                    {
                        docs.tabs[k].encoding_changed = true;
                    }
                    let id = docs.tabs[k].id;
                    self.adopt(&found, id);
                    k
                }
            };
            // The copy names its draft, under whatever name the tab now has: no header is
            // rewritten for it.
            if let Some(d) = h.draft {
                docs.tabs[k].draft = d;
            }
            out.count += 1;
            *out.tabs.last_mut().unwrap() = Some(docs.tabs[k].id);
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
        #[cfg(test)]
        let start = !self.no_writer;
        #[cfg(not(test))]
        let start = true;
        if self.writer.is_none() && start {
            self.writer = spawn_writer(dir, self.failed.clone(), self.in_flight.clone());
        }
        let write = match &job {
            Job::Write { name, key, .. } => Some((name.clone(), *key)),
            Job::Remove(_) => None,
        };
        if write.is_some() {
            self.in_flight.fetch_add(1, Ordering::AcqRel);
        }
        let sent = self.writer.as_ref().is_some_and(|w| w.tx.send(job).is_ok());
        // A writer that stopped is replaced at the next write, not sent to again.
        if !sent {
            self.writer = None;
        }
        // Not handed to a writer: a failed write, tried again like one.
        if !sent && let Some(w) = write {
            if let Ok(mut f) = self.failed.lock() {
                f.push(w);
            }
            self.in_flight.fetch_sub(1, Ordering::AcqRel);
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
        // A copy that could not be written is not written: it is due again after the pause, so
        // a folder that cannot be written is tried at that pace, not in a loop.
        let failed = self
            .failed
            .lock()
            .map(|mut f| std::mem::take(&mut *f))
            .unwrap_or_default();
        for (name, key) in failed {
            let id = self.slots.keys().copied().find(|&id| self.name(id) == name);
            if let Some(slot) = id.and_then(|id| self.slots.get_mut(&id))
                && slot.written == Some(key)
            {
                slot.written = None;
                slot.last_change = Some(now);
                slot.dirty_since = Some(now);
            }
        }
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
                draft: Some(d.draft),
                bytes: 0,
            };
            header.set_stamp(&r.info.stamp);
            jobs.push(Job::Write {
                name,
                header,
                text: r.buffer.snapshot(),
                key,
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
        // A write in flight may fail, and one may have failed already (one that could not be
        // sent, or one the thread finished since the list was taken): look again soon, so a
        // failure is seen without any other event to wake the window. The count is read before
        // the list: the thread lists a failure before it counts the write done, so a count of
        // nothing means every failure of those writes is in the list (audit 1 round 3, #2).
        let busy = self.in_flight.load(Ordering::Acquire) > 0;
        let failed = self.failed.lock().is_ok_and(|f| !f.is_empty());
        if busy || failed {
            let soon = now + Duration::from_secs(1);
            next = Some(next.map_or(soon, |n| n.min(soon)));
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
    /// back (it is in no other place; the next start offers it). `unsaved_kept` is false when
    /// the session listing the backups of the unsaved text could not be written: then the copy
    /// of every tab with unsaved changes stays too, for the next start (audit 1 round 5, #4).
    pub fn finish(&mut self, docs: &Docs, unsaved_kept: bool) {
        self.flush();
        let Some(dir) = &self.dir else { return };
        let modified: Vec<PathBuf> = if unsaved_kept {
            Vec::new()
        } else {
            docs.tabs
                .iter()
                .filter(|d| d.modified())
                .map(|d| dir.join(self.name(d.id)))
                .collect()
        };
        let keep: Vec<&Path> = docs
            .tabs
            .iter()
            .filter_map(|d| d.restore.as_deref())
            .chain(self.unreadable.iter().map(PathBuf::as_path))
            .chain(modified.iter().map(PathBuf::as_path))
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

/// Tests: a file another program holds so it cannot be read (on Windows open without read
/// sharing; elsewhere without permissions), until dropped.
#[cfg(test)]
pub(crate) struct Locked {
    path: PathBuf,
    #[cfg(windows)]
    _file: std::fs::File,
}

#[cfg(test)]
impl Locked {
    /// `None` when the file stays readable (a user every file is readable to): nothing to test.
    pub(crate) fn new(path: &Path) -> Option<Locked> {
        #[cfg(windows)]
        let locked = {
            use std::os::windows::fs::OpenOptionsExt;
            // Others may delete or rename it, not read or write it.
            const FILE_SHARE_DELETE: u32 = 4;
            Locked {
                path: path.to_path_buf(),
                _file: std::fs::OpenOptions::new()
                    .read(true)
                    .share_mode(FILE_SHARE_DELETE)
                    .open(path)
                    .unwrap(),
            }
        };
        #[cfg(not(windows))]
        let locked = {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).unwrap();
            Locked {
                path: path.to_path_buf(),
            }
        };
        std::fs::read(path).is_err().then_some(locked)
    }
}

#[cfg(test)]
impl Drop for Locked {
    fn drop(&mut self) {
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
        #[cfg(windows)]
        let _ = &self.path;
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

    /// Audit 1, finding 5: a copy that could not be written is not taken as written; it is
    /// tried again after the pause, and written once the folder can be.
    #[test]
    fn a_failed_write_is_tried_again() {
        let (dir, path, mut rec) = setup("failed", b"one\n");
        // A file where the folder goes: the write fails.
        std::fs::write(dir.join("recover"), b"in the way").unwrap();
        let mut d = docs();
        let i = d.open(&path);
        d.load_now(i, None, false);
        let t0 = Instant::now();
        assert!(d.edit(EditOp::Insert("a"), None));
        assert_eq!(rec.poll(&d, t0), Some(t0 + secs(3)));
        // Sent to be written: looked at again within a second, to see how it went.
        let again = rec
            .poll(&d, t0 + secs(3))
            .expect("a look while the write is in flight");
        assert!(again <= t0 + secs(4));
        rec.flush();
        // The failure is seen at the next look: due again after the pause, not at once.
        assert_eq!(rec.poll(&d, t0 + secs(4)), Some(t0 + secs(7)));
        std::fs::remove_file(dir.join("recover")).unwrap();
        rec.poll(&d, t0 + secs(7));
        rec.flush();
        // Written now: nothing in flight, nothing due.
        assert_eq!(rec.poll(&d, t0 + secs(8)), None);
        assert_eq!(files(&dir).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 1, round 2: a write that cannot even be handed to the writer is tried again too.
    #[test]
    fn a_write_that_cannot_be_sent_is_tried_again() {
        let (dir, path, mut rec) = setup("unsent", b"one\n");
        rec.no_writer = true;
        let mut d = docs();
        let i = d.open(&path);
        d.load_now(i, None, false);
        let t0 = Instant::now();
        assert!(d.edit(EditOp::Insert("a"), None));
        rec.poll(&d, t0);
        // The poll that could not send the write looks again soon: nothing else may wake an
        // idle window to see the failure (audit 1 round 3, #2).
        assert_eq!(rec.poll(&d, t0 + secs(3)), Some(t0 + secs(4)));
        // Due again after the pause, not taken as written.
        assert_eq!(rec.poll(&d, t0 + secs(4)), Some(t0 + secs(7)));
        rec.no_writer = false;
        rec.poll(&d, t0 + secs(7));
        rec.flush();
        assert_eq!(files(&dir).len(), 1);
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
        let found = rec.scan();
        let got = rec.restore(&mut e, found);
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
        rec.finish(&e, true);
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
        let found = rec.scan();
        let got = rec.restore(&mut e, found);
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
        let found = rec.scan();
        let got = rec.restore(&mut e, found);
        assert_eq!((got.count, got.load.len()), (1, 0));
        assert_eq!(text_of(&e, 0), "mine one\n");
        assert!(e.tabs[0].path.is_none() && e.tabs[0].modified());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn damaged_and_foreign_files_are_dropped() {
        let (dir, _path, mut rec) = setup("damaged", b"one\n");
        let r = dir.join("recover");
        std::fs::create_dir_all(&r).unwrap();
        std::fs::write(r.join("1-1.rec"), b"junk").unwrap();
        std::fs::write(r.join("1-2.rec.tmp"), b"half").unwrap();
        std::fs::write(r.join("notes.txt"), b"mine").unwrap();
        assert!(rec.scan().is_empty());
        assert_eq!(files(&dir), ["notes.txt"], "only what is not ours stays");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two copies of one file (two runs, or a draft whose tab was renamed): the file's tab takes
    /// the first, the second comes back as a draft of its own, and each keeps its draft id.
    /// Neither replaces the other (audit 1 round 4, #4).
    #[test]
    fn two_copies_of_one_file_both_come_back() {
        let (dir, path, mut rec) = setup(
            "two-copies",
            b"one
",
        );
        let r = dir.join("recover");
        let h = |draft| Header {
            path: Some(path.clone()),
            untitled: 0,
            encoding: "UTF-8".into(),
            eol: "Unix (LF)".into(),
            disk: None,
            draft: Some(draft),
            bytes: 0,
        };
        recovery::write(&r, "9-1.rec", &h(1), &Rope::from_str("first")).unwrap();
        recovery::write(&r, "9-2.rec", &h(2), &Rope::from_str("second")).unwrap();
        let mut d = docs();
        let found = rec.scan();
        let got = rec.restore(&mut d, found);
        assert_eq!(got.count, 2);
        let k = got.load[0];
        assert_eq!(d.tabs[k].path.as_deref(), Some(path.as_path()));
        assert_eq!(d.tabs[k].draft, 1);
        let other = d.index_of_id(got.tabs[1].unwrap()).unwrap();
        assert_ne!(other, k);
        assert_eq!(
            (text_of(&d, other).as_str(), d.tabs[other].draft),
            ("second", 2)
        );
        assert!(d.tabs[k].restore.as_ref().unwrap().is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Audit 1 round 4, #3: a copy another program holds open, so it cannot be read now, is
    /// neither deleted at the start nor at the exit, and comes back once it can be read.
    #[test]
    fn a_copy_that_cannot_be_read_now_is_kept() {
        let (dir, path, mut rec) = setup("locked", b"one\n");
        let r = dir.join("recover");
        let h = Header {
            path: Some(path.clone()),
            untitled: 0,
            encoding: "UTF-8".into(),
            eol: "Unix (LF)".into(),
            disk: None,
            draft: Some(5),
            bytes: 0,
        };
        recovery::write(&r, "9-1.rec", &h, &Rope::from_str("copy")).unwrap();
        let held = r.join("9-1.rec");
        #[cfg(windows)]
        let lock = {
            use std::os::windows::fs::OpenOptionsExt;
            // Others may delete it, not read it.
            const FILE_SHARE_DELETE: u32 = 4;
            std::fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_DELETE)
                .open(&held)
                .unwrap()
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&held, std::fs::Permissions::from_mode(0o000)).unwrap();
        }
        if std::fs::read(&held).is_ok() {
            // Running as a user every file is readable to: nothing to test.
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let mut d = docs();
        let found = rec.scan();
        assert!(found.is_empty());
        rec.restore(&mut d, found);
        rec.finish(&d, true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&held, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        #[cfg(windows)]
        drop(lock);
        assert!(held.is_file(), "the copy is kept");
        let mut again = Recovery::new(Some(r.clone()));
        let found = again.scan();
        assert_eq!(found.len(), 1);
        let mut d = docs();
        assert_eq!(again.restore(&mut d, found).count, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn copy(path: Option<&str>, untitled: u32, draft: Option<u64>, at: u64) -> Found {
        Found {
            header: Header {
                path: path.map(PathBuf::from),
                untitled,
                encoding: "UTF-8".into(),
                eol: "Unix (LF)".into(),
                disk: None,
                draft,
                bytes: 0,
            },
            file: PathBuf::from(format!("{at}.rec")),
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(at)),
        }
    }

    fn entry(file: &str, untitled: u32, path: Option<&str>, draft: Option<u64>) -> Unsaved {
        Unsaved {
            file: file.into(),
            untitled,
            path: path.map(PathBuf::from),
            draft,
        }
    }

    /// Backups written at second 100; a copy at 200 is newer, one at 50 older.
    fn at_100(_: &Unsaved) -> Option<SystemTime> {
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(100))
    }

    #[test]
    fn a_copy_covers_the_backup_of_its_draft() {
        let found = vec![
            copy(Some("/a/f.txt"), 0, Some(1), 200),
            copy(None, 2, Some(2), 201),
        ];
        let list = [
            entry("1.txt", 0, Some("/a/f.txt"), Some(1)),
            // The id decides, whatever the names say.
            entry("2.txt", 7, Some("/elsewhere.txt"), Some(2)),
            entry("3.txt", 2, None, Some(3)),
            entry("4.txt", 0, Some("/a/f.txt"), None),
        ];
        assert_eq!(
            cover(&found, &list, at_100, |_| None),
            [Some(0), Some(1), None, None]
        );
        // A copy older than the backup does not cover it: the backup is the newer text.
        let old = vec![copy(Some("/a/f.txt"), 0, Some(1), 50)];
        assert_eq!(cover(&old, &list[..1], at_100, |_| None), [None]);
        // A backup whose time is unknown is not covered.
        assert_eq!(cover(&found, &list[..1], |_| None, |_| None), [None]);
        assert_eq!(cover(&[], &list[..1], at_100, |_| None), [None]);
    }

    /// Before 1.1 neither had an id: the file, compared as the tabs compare files, or the number
    /// decides, and only one to one (audit 1 round 4, #4, #5).
    #[test]
    fn copies_and_backups_without_ids_cover_only_when_certain() {
        let found = vec![
            copy(Some("/a/f.txt"), 0, None, 200),
            copy(None, 2, None, 201),
        ];
        let list = [
            entry("1.txt", 0, Some("/a/f.txt"), None),
            entry("2.txt", 2, None, None),
            entry("3.txt", 3, None, None),
            entry("4.txt", 0, Some("/a/g.txt"), None),
            // A file's copy does not cover a document without one, nor the other way round.
            entry("5.txt", 0, Some("/a/other.txt"), None),
        ];
        assert_eq!(
            cover(&found, &list, at_100, |_| None),
            [Some(0), Some(1), None, None, None]
        );
        // An id on one side only is not a match.
        let mixed = [entry("1.txt", 0, Some("/a/f.txt"), Some(9))];
        assert_eq!(cover(&found, &mixed, at_100, |_| None), [None]);
        // Two backups numbered 1 and one copy numbered 1: neither is certainly the copy's.
        let two = [entry("1.txt", 1, None, None), entry("2.txt", 1, None, None)];
        let one = vec![copy(None, 1, None, 200)];
        assert_eq!(cover(&one, &two, at_100, |_| None), [None, None]);
        // Two copies for one backup: the same.
        let copies = vec![copy(None, 1, None, 200), copy(None, 1, None, 201)];
        assert_eq!(cover(&copies, &two[..1], at_100, |_| None), [None]);
        // Windows compares file names without case, as its tabs do.
        if cfg!(windows) {
            let upper = [entry("1.txt", 0, Some("/A/F.TXT"), None)];
            assert_eq!(cover(&found, &upper, at_100, |_| None), [Some(0)]);
        }
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
        let found = rec.scan();
        rec.restore(&mut e, found);
        // Closed before the file was read: the copy is the only place the text is.
        rec.finish(&e, true);
        assert_eq!(files(&dir).len(), 1);
        let mut again = Recovery::new(Some(dir.join("recover")));
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
        let found = rec.scan();
        rec.restore(&mut e, found);
        let r = e.tabs[0].ready().unwrap();
        assert_eq!(r.info.encoding, e.legacy());
        assert_eq!(text_of(&e, 0), "caf\u{e9}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
