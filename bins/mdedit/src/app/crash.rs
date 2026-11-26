//! Crash reports in mdedit: where they go, the context a report carries, and the bar that offers
//! last time's report on the next start. The reports themselves are `mdshared_app::crash`.

use super::*;
use mdshared_app::crash;

/// The folder of this launch's crash reports: [`crash_dir`] for the environment and the
/// arguments mdedit was started with.
pub fn crash_dir_of_this_launch() -> Option<PathBuf> {
    let flag = |f: &str| std::env::args().skip(1).any(|a| a == f);
    crash_dir(
        std::env::var_os("MDEDIT_SETTINGS").map(PathBuf::from),
        flag("--no-session") || flag("--shot"),
        mdshared_app::settings::data_dir("mdedit"),
    )
}

/// Where crash reports go. Beside the settings file `MDEDIT_SETTINGS` names, when it names one:
/// a development build then writes into its own scratch folder. Otherwise in `crashes` under
/// the data folder, except for a `--no-session` window or a shot, which keep none: they must
/// read and write nothing of the owner's (CLAUDE.md rule 9), and a report from the owner's own
/// folder would be offered to them as theirs. A value with no folder in it (empty, or a bare
/// file name) names no folder to be beside, so it keeps none either: the snippets and the
/// library refuse the same value, and `crashes` in whatever folder the build was started in is
/// no place of its own (audit 4, F20).
pub fn crash_dir(
    settings_named: Option<PathBuf>,
    no_session: bool,
    data_dir: Option<PathBuf>,
) -> Option<PathBuf> {
    match settings_named {
        Some(file) => file
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(|dir| dir.join("crashes")),
        None if no_session => None,
        None => data_dir.map(|d| d.join("crashes")),
    }
}

/// What a report says mdedit was doing, as a key that changes only when the text would: the
/// active document's kind and size (by 16 KB), the tab count, the preview pane and the split.
/// Never a document's text or a file's name.
#[derive(Clone, PartialEq, Default)]
pub(super) struct CrashKey {
    kind: Option<&'static str>,
    size_16k: u64,
    tabs: usize,
    pane: bool,
    split: bool,
}

impl App {
    /// Updates the context of a report when it changed; a few comparisons a frame.
    pub(super) fn crash_context(&mut self) {
        let ready = self.docs.active_ready();
        let key = CrashKey {
            kind: ready.map(|r| r.language.map_or("plain text", |l| l.name())),
            size_16k: ready.map_or(0, |r| r.buffer.len_bytes() as u64 / 16_384),
            tabs: self.docs.tabs.len(),
            pane: self.preview.is_some(),
            split: self.docs.other.is_some() || self.docs.same_file,
        };
        if key != self.crash_key {
            crash::set_context(&context_text(&key));
            self.crash_key = key;
        }
    }

    /// Once settled: the report of a crash last time, if there is one, gets its bar.
    pub(super) fn look_for_crash(&mut self) {
        let found = crash_dir_of_this_launch().and_then(|d| crash::pending(&d));
        if found != self.docs.crash {
            self.docs.crash = found;
            self.relayout_areas();
            self.full = true;
            self.redraw();
        }
    }

    /// The bar's buttons: open the report in a tab, copy it, or close the bar. Each marks the
    /// report seen, so it is not offered again; it stays in the folder.
    pub(super) fn crash_bar(&mut self, hit: Hit) {
        let Some(report) = self.docs.crash.take() else {
            return;
        };
        let seen = crash::mark_seen(&report).unwrap_or(report);
        match hit {
            Hit::CrashOpen => self.open(&seen),
            Hit::CrashCopy => {
                let text = std::fs::read_to_string(&seen).unwrap_or_default();
                if !text.is_empty() && platform::set_clipboard_text(&text) {
                    self.docs.confirm_toast(strings::CRASH_REPORT_COPIED);
                }
            }
            _ => {}
        }
        self.relayout_areas();
        self.full = true;
        self.redraw();
    }
}

fn context_text(k: &CrashKey) -> String {
    let doc = match k.kind {
        Some(kind) => format!("{kind}, {} KB", k.size_16k * 16),
        None => "no document".to_string(),
    };
    let mut s = format!(
        "{doc}; {} tab{}",
        k.tabs,
        if k.tabs == 1 { "" } else { "s" }
    );
    if k.pane {
        s.push_str("; preview pane open");
    }
    if k.split {
        s.push_str("; split view");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::super::files_tests::{app, open_now, scratch};
    use super::*;

    #[test]
    fn crash_reports_go_beside_a_named_settings_file_and_never_to_the_owner_from_a_test_run() {
        let data = Some(PathBuf::from("data").join("mdedit"));
        let named = PathBuf::from("scratch").join("settings.ini");
        // A settings file named: its folder, with or without a session, never the data folder.
        for no_session in [true, false] {
            assert_eq!(
                crash_dir(Some(named.clone()), no_session, data.clone()),
                Some(PathBuf::from("scratch").join("crashes"))
            );
        }
        // A bare file name, or nothing at all: no folder to be beside, so no reports, as the
        // snippets and the library refuse it (audit 4, F20). Before, `crashes` in the folder the
        // build was started in.
        for bare in ["settings.ini", ""] {
            for no_session in [true, false] {
                assert_eq!(
                    crash_dir(Some(PathBuf::from(bare)), no_session, data.clone()),
                    None,
                    "{bare:?}"
                );
            }
        }
        // `--no-session` or a shot without one: no reports at all.
        assert_eq!(crash_dir(None, true, data.clone()), None);
        // The owner's own launch: the data folder, as before.
        assert_eq!(
            crash_dir(None, false, data),
            Some(PathBuf::from("data").join("mdedit").join("crashes"))
        );
        assert_eq!(crash_dir(None, false, None), None);
    }

    #[test]
    fn the_context_names_kinds_and_sizes_only() {
        let k = CrashKey {
            kind: Some("Markdown"),
            size_16k: 3,
            tabs: 2,
            pane: true,
            split: false,
        };
        assert_eq!(
            context_text(&k),
            "Markdown, 48 KB; 2 tabs; preview pane open"
        );
        assert_eq!(
            context_text(&CrashKey {
                tabs: 1,
                ..Default::default()
            }),
            "no document; 1 tab"
        );
    }

    #[test]
    fn last_times_report_gets_a_bar_and_its_buttons_put_it_away() {
        let dir = scratch("crash-bar");
        let md = dir.join("a.md");
        std::fs::write(&md, "# Secret title\n").unwrap();
        let mut a = app();
        // The report opens as text, where the format bar would go away too.
        a.docs.format_bar = false;
        open_now(&mut a, &md);
        a.crash_context();
        assert!(!context_text(&a.crash_key).contains("Secret"));
        a.compose();
        let text_top = a.areas.text.y;
        let reports = dir.join("crashes");
        std::fs::create_dir_all(&reports).unwrap();
        let report = reports.join("crash-00000000001.txt");
        std::fs::write(&report, "mdedit closed because of an error.\n").unwrap();
        a.docs.crash = mdshared_app::crash::pending(&reports);
        a.relayout_areas();
        a.compose();
        assert!(a.hit_rect(Hit::CrashOpen).is_some() && a.hit_rect(Hit::CrashCopy).is_some());
        assert!(a.areas.text.y > text_top, "the bar takes its room");
        // Open: a tab with the report, the bar gone, the report seen.
        a.crash_bar(Hit::CrashOpen);
        a.compose();
        assert!(a.docs.crash.is_none() && a.hit_rect(Hit::CrashOpen).is_none());
        assert_eq!(a.areas.text.y, text_top);
        assert_eq!(mdshared_app::crash::pending(&reports), None);
        let seen = reports.join("crash-00000000001.seen.txt");
        assert!(seen.is_file());
        assert!(
            a.docs
                .tabs
                .iter()
                .any(|d| d.path.as_deref() == Some(seen.as_path()))
        );
    }
}
