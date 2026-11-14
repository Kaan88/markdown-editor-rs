//! OS calls this crate makes itself (the system code page, memory and CPU figures for --trace);
//! the rest is `mdshared_ui::platform`. The only `unsafe` in the crate is here.

pub use mdshared_ui::platform::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The open dialog's named filter; "All files" comes with it.
const OPEN_FILTER: &[&str] = &[
    "txt", "md", "markdown", "rs", "toml", "py", "json", "js", "ts", "c", "h", "cpp", "hpp", "cs",
    "java", "go", "ini", "cfg", "log", "csv", "xml", "html", "css", "yaml", "yml", "sh", "ps1",
    "bat", "sql",
];

#[cfg(windows)]
pub fn pick_files(start_dir: Option<&Path>) -> Vec<PathBuf> {
    mdshared_ui::platform::pick_files(start_dir, "Text and code", OPEN_FILTER)
}

/// The open dialog without blocking the UI thread; `done` runs on a helper thread. False when
/// another dialog is open (`done` has then been called with an empty list).
#[cfg(not(windows))]
pub fn pick_files_async(
    start_dir: Option<&Path>,
    done: impl FnOnce(Vec<PathBuf>) + Send + 'static,
) -> bool {
    mdshared_ui::platform::pick_files_async(start_dir, "Text and code", OPEN_FILTER, done)
}

/// The Save as dialog, opened on `current`'s folder with its name filled in. Windows only: the
/// other systems use `mdshared_ui::platform::pick_save_async`.
#[cfg(windows)]
pub fn pick_save(current: &Path) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(dir) = current.parent().filter(|d| d.is_dir()) {
        dialog = dialog.set_directory(dir);
    }
    if let Some(name) = current.file_name() {
        dialog = dialog.set_file_name(name.to_string_lossy());
    }
    dialog.save_file()
}

/// The most characters a paste into the find box takes.
pub const PASTE_MAX_CHARS: usize = 1000;

/// The first line of UTF-16 `units` (up to a NUL, CR or LF), at most `max_chars` characters.
/// Unpaired surrogates become U+FFFD. Only the Windows clipboard hands out UTF-16; elsewhere the
/// function exists for its test alone.
#[cfg(any(windows, test))]
pub fn first_line(units: &[u16], max_chars: usize) -> String {
    let line = units
        .iter()
        .copied()
        .take_while(|&u| u != 0 && u != u16::from(b'\r') && u != u16::from(b'\n'));
    char::decode_utf16(line)
        .take(max_chars)
        .map(|c| c.unwrap_or('\u{FFFD}'))
        .collect()
}

/// The first line of the clipboard's text, at most [`PASTE_MAX_CHARS`] characters, for the find
/// box. Only that much is read: a clipboard holding a whole large file is not copied.
#[cfg(windows)]
pub fn clipboard_line() -> Option<String> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    const CF_UNICODETEXT: u32 = 13;
    // Another program may hold the clipboard for a moment.
    let mut opened = false;
    for _ in 0..5 {
        // SAFETY: OpenClipboard with a null window associates the clipboard with this task.
        if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
            opened = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !opened {
        return None;
    }
    let read = || {
        // SAFETY: the clipboard is open; the handle stays owned by the system.
        let h = unsafe { GetClipboardData(CF_UNICODETEXT) };
        if h.is_null() {
            return None;
        }
        // SAFETY: h is a global memory handle from the clipboard.
        let ptr = unsafe { GlobalLock(h) } as *const u16;
        if ptr.is_null() {
            return None;
        }
        // SAFETY: h is locked, so its size can be read; at most that many u16 are read, and
        // only as far as the first line needs (two units per character for surrogate pairs).
        let len = (unsafe { GlobalSize(h) } / 2).min(PASTE_MAX_CHARS * 2 + 2);
        // SAFETY: ptr points to at least `len` readable u16 units while the lock is held.
        let units = unsafe { std::slice::from_raw_parts(ptr, len) };
        let line = first_line(units, PASTE_MAX_CHARS);
        // SAFETY: matches the GlobalLock above.
        unsafe { GlobalUnlock(h) };
        Some(line)
    };
    let line = read();
    // SAFETY: the clipboard was opened by this thread above.
    unsafe { CloseClipboard() };
    line
}

#[cfg(not(windows))]
pub fn clipboard_line() -> Option<String> {
    mdshared_ui::platform::clipboard_text().map(|t| {
        t.lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(PASTE_MAX_CHARS)
            .collect()
    })
}

/// The clipboard's image as PNG file bytes, or `None` when it holds no image (or one that is not
/// understood). Windows: the "PNG" format that browsers and screenshot tools offer, else the
/// bitmap, converted. Read only when a paste finds no text.
#[cfg(windows)]
pub fn clipboard_png() -> Option<Vec<u8>> {
    use crate::image_paste::{MAX_BYTES, dib_to_png, is_png};
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, OpenClipboard, RegisterClipboardFormatW,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    const CF_DIB: u32 = 8;
    let mut opened = false;
    for _ in 0..5 {
        // SAFETY: OpenClipboard with a null window associates the clipboard with this task.
        if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
            opened = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !opened {
        return None;
    }
    // The bytes of one clipboard format, copied out.
    let bytes_of = |format: u32| -> Option<Vec<u8>> {
        // SAFETY: the clipboard is open; the handle stays owned by the system.
        let h = unsafe { GetClipboardData(format) };
        if h.is_null() {
            return None;
        }
        // SAFETY: h is a global memory handle from the clipboard.
        let size = unsafe { GlobalSize(h) };
        if size == 0 || size > MAX_BYTES {
            return None;
        }
        // SAFETY: as above; the lock is released below.
        let ptr = unsafe { GlobalLock(h) } as *const u8;
        if ptr.is_null() {
            return None;
        }
        // SAFETY: ptr points to `size` readable bytes while the lock is held.
        let copy = unsafe { std::slice::from_raw_parts(ptr, size) }.to_vec();
        // SAFETY: matches the GlobalLock above.
        unsafe { GlobalUnlock(h) };
        Some(copy)
    };
    let read = || {
        let name: [u16; 4] = [u16::from(b'P'), u16::from(b'N'), u16::from(b'G'), 0];
        // SAFETY: name is NUL-terminated.
        let png_format = unsafe { RegisterClipboardFormatW(name.as_ptr()) };
        if png_format != 0
            && let Some(bytes) = bytes_of(png_format)
            && is_png(&bytes)
        {
            return Some(bytes);
        }
        dib_to_png(&bytes_of(CF_DIB)?)
    };
    let png = read();
    // SAFETY: the clipboard was opened by this thread above.
    unsafe { CloseClipboard() };
    png
}

/// The clipboard's image as PNG file bytes, or `None`. The clipboard crate in use reads text only
/// (its image support pulls in an image library), so this asks the desktop's own tool for the
/// PNG: `wl-paste` on Wayland, `xclip` on X11, whichever is installed. Neither installed means
/// no image paste, which is what a paste then does: nothing.
#[cfg(not(windows))]
pub fn clipboard_png() -> Option<Vec<u8>> {
    const WL: (&str, &[&str]) = ("wl-paste", &["--no-newline", "--type", "image/png"]);
    const X11: (&str, &[&str]) = (
        "xclip",
        &["-selection", "clipboard", "-t", "image/png", "-o"],
    );
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
    let order = if wayland { [WL, X11] } else { [X11, WL] };
    order
        .iter()
        .find_map(|(program, args)| capture_png(program, args))
}

/// Runs `program` and returns its output when it is a PNG of a sensible size within three
/// seconds; a tool that hangs on a clipboard owner that does not answer is stopped.
#[cfg(not(windows))]
fn capture_png(program: &str, args: &[&str]) -> Option<Vec<u8>> {
    use crate::image_paste::{MAX_BYTES, is_png};
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes);
        let _ = tx.send(bytes);
    });
    let got = rx.recv_timeout(Duration::from_secs(3)).ok();
    // Finished or not, nothing more is wanted from it.
    let _ = child.kill();
    let _ = child.wait();
    got.filter(|b| b.len() <= MAX_BYTES && is_png(b))
}

/// The system's ANSI code page (`GetACP`), for files that are neither UTF-8 nor marked.
#[cfg(windows)]
pub fn code_page() -> u32 {
    // SAFETY: GetACP takes no arguments and only reads process state.
    unsafe { windows_sys::Win32::Globalization::GetACP() }
}

#[cfg(not(windows))]
pub fn code_page() -> u32 {
    1252
}

/// Process memory: private bytes and the peak working set.
#[derive(Clone, Copy, Debug)]
pub struct Memory {
    pub private: u64,
    pub peak_working_set: u64,
}

#[cfg(windows)]
pub fn memory() -> Option<Memory> {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut c: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    // SAFETY: `c` is a correctly sized, writable PROCESS_MEMORY_COUNTERS_EX, whose layout starts
    // with PROCESS_MEMORY_COUNTERS; the pseudo handle needs no closing.
    let ok = unsafe {
        K32GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&mut c as *mut PROCESS_MEMORY_COUNTERS_EX).cast::<PROCESS_MEMORY_COUNTERS>(),
            c.cb,
        )
    };
    (ok != 0).then_some(Memory {
        private: c.PrivateUsage as u64,
        peak_working_set: c.PeakWorkingSetSize as u64,
    })
}

/// Linux: `RssAnon` (memory private to the process, the counterpart of Windows' private bytes)
/// and `VmHWM` (the peak resident set) from `/proc/self/status`. Other systems: none.
#[cfg(not(windows))]
pub fn memory() -> Option<Memory> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        parse_status(&status)
    }
    #[cfg(not(target_os = "linux"))]
    None
}

/// The two figures out of the text of `/proc/self/status` (lines like `RssAnon:   1234 kB`).
#[cfg(any(target_os = "linux", test))]
fn parse_status(status: &str) -> Option<Memory> {
    let kib = |key: &str| -> Option<u64> {
        let line = status.lines().find_map(|l| l.strip_prefix(key))?;
        Some(line.trim().strip_suffix("kB")?.trim().parse::<u64>().ok()? * 1024)
    };
    Some(Memory {
        private: kib("RssAnon:")?,
        peak_working_set: kib("VmHWM:")?,
    })
}

/// CPU time the process has used, kernel and user together.
#[cfg(windows)]
pub fn cpu_time() -> Option<Duration> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: four writable FILETIMEs and the current-process pseudo handle.
    let ok = unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    };
    let t = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
    // FILETIME counts 100 ns units.
    (ok != 0).then(|| Duration::from_nanos((t(kernel) + t(user)) * 100))
}

/// Linux: user plus system time of the process from `/proc/self/stat`, in clock ticks of
/// `USER_HZ`, which is 100 on every Linux. Other systems: none.
#[cfg(not(windows))]
pub fn cpu_time() -> Option<Duration> {
    #[cfg(target_os = "linux")]
    {
        let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
        parse_stat(&stat)
    }
    #[cfg(not(target_os = "linux"))]
    None
}

/// The CPU time out of the text of `/proc/self/stat`. The command name (field 2) may hold spaces
/// and parentheses, so the fields are counted from the last `)`: `utime` and `stime` are the 12th
/// and 13th after it (fields 14 and 15 of the line).
#[cfg(any(target_os = "linux", test))]
fn parse_stat(stat: &str) -> Option<Duration> {
    let rest = &stat[stat.rfind(')')? + 1..];
    let mut f = rest.split_whitespace().skip(11);
    let utime: u64 = f.next()?.parse().ok()?;
    let stime: u64 = f.next()?.parse().ok()?;
    Some(Duration::from_millis((utime + stime) * 10))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_answer() {
        assert!(code_page() > 0);
        if cfg!(any(windows, target_os = "linux")) {
            let m = memory().expect("memory");
            assert!(m.private > 0 && m.peak_working_set > 0);
            assert!(cpu_time().is_some());
        }
    }

    #[test]
    fn proc_text_is_read() {
        let status = "Name:\tmdedit\nVmHWM:\t   2048 kB\nRssAnon:\t   1024 kB\nRssFile:\t 5 kB\n";
        let m = parse_status(status).unwrap();
        assert_eq!((m.private, m.peak_working_set), (1024 * 1024, 2048 * 1024));
        assert!(parse_status("Name:\tx\n").is_none());
        // Field 2 holds a space and a parenthesis; utime is 250 ticks, stime 50.
        let stat = "42 (my (ed) it) S 1 2 3 4 5 6 7 8 9 10 250 50 0 0 20";
        assert_eq!(parse_stat(stat), Some(Duration::from_millis(3000)));
        assert!(parse_stat("garbage").is_none());
    }

    #[test]
    fn pasted_line_is_the_first_and_capped() {
        let w = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
        assert_eq!(first_line(&w("needle\r\nrest"), 1000), "needle");
        assert_eq!(first_line(&w("a\nb"), 1000), "a");
        assert_eq!(first_line(&w("ab\0cd"), 1000), "ab");
        assert_eq!(first_line(&w("ğış\u{1F600}x"), 4), "ğış\u{1F600}");
        assert_eq!(first_line(&w(""), 1000), "");
        // A 1 MB single line gives the cap, not the line.
        let long = vec![u16::from(b'a'); 1 << 20];
        assert_eq!(first_line(&long, PASTE_MAX_CHARS).len(), PASTE_MAX_CHARS);
        // An unpaired surrogate.
        assert_eq!(first_line(&[0xD800, u16::from(b'x')], 10), "\u{FFFD}x");
    }
}
