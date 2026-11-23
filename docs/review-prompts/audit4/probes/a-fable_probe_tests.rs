//! Review probes (not for commit). Each passes on 1d503d8 and asserts the fault at `FAULT`.
use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::keys::{Key, KeyName};
use crate::recycle::FolderBin;

/// P4: the tab holds the text it read; the file is changed on disk and the tab has not read it
/// again yet. The plan takes its notes from the tab and its digest from the disk, so the write's
/// "the source is still the file the plan was made from" passes, and step 8 puts a document in
/// the bin that no note holds.
#[test]
fn probe_a_plan_made_from_a_stale_tab_writes_the_old_text_and_recycles_the_new() {
    let dir = scratch("fable-stale");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    a.recycle = Box::new(FolderBin::new(&dir.join("bin")));
    let source = dir.join("Trip.md");
    std::fs::write(&source, "## One\n\nalpha\n\n## Two\n\nbeta\n").expect("the source");
    open_now(&mut a, &source);
    // Another program saves the file; the watcher has not been heard yet.
    let newer = "## One\n\nALPHA REWRITTEN\n\n## Two\n\nbeta\n\n## Three\n\nonly on disk\n";
    std::fs::write(&source, newer).expect("the newer text");
    assert!(!a.docs.active_doc().expect("a tab").modified());
    assert!(a.plan_open_active(), "FAULT: the plan opens on the stale text");
    a.compose();
    {
        let p = a.plan.as_mut().expect("open");
        p.keep_the_rest();
        p.toggle_recycle();
        assert!(p.recycle_on(), "{:?}", p.recycle_why());
    }
    a.key_pressed(Key::Name(KeyName::Enter), None);
    let library = dir.join("library");
    let one = std::fs::read_to_string(library.join("Trip/one.md")).unwrap_or_default();
    // FAULT: the note is cut from the old text.
    assert!(one.contains("alpha") && !one.contains("REWRITTEN"), "{one}");
    assert!(!library.join("Trip/three.md").exists());
    // FAULT: and the newer document, which no note holds, is in the bin.
    assert!(!source.exists(), "the source left its place");
    let binned = std::fs::read_to_string(FolderBin::held(&dir.join("bin"), &source))
        .unwrap_or_default();
    assert_eq!(binned, newer);
    let _ = std::fs::remove_dir_all(&dir);
}
