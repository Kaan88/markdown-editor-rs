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

pub fn pick_files(start_dir: Option<&Path>) -> Vec<PathBuf> {
    mdshared_ui::platform::pick_files(start_dir, "Text and code", OPEN_FILTER)
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

#[cfg(not(windows))]
pub fn memory() -> Option<Memory> {
    None
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

#[cfg(not(windows))]
pub fn cpu_time() -> Option<Duration> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_answer() {
        assert!(code_page() > 0);
        if cfg!(windows) {
            let m = memory().expect("memory");
            assert!(m.private > 0 && m.peak_working_set > 0);
            assert!(cpu_time().is_some());
        }
    }
}
