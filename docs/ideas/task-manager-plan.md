# A task manager for the family: one plan

Written 2026-11-22 for the owner to approve. "You" is the owner. Nothing here is built; `mdtask` is
a working name (question 1). The pictures are on the branch `mock/task-manager`, `shots/mock-tm/`
(about 40 states, light and dark, nine also at 860x600); this plan names them by file. The owner
said a task manager was not in `docs/ecosystem-ideas.md`; it is read here as a **system** task
manager (processes, performance, startup apps, services), Windows first.

**What I read.** `CLAUDE.md`, `docs/ecosystem-ideas.md` on `main`, the file manager's plan and mockup
(`mock/file-manager`: `docs/ideas/file-manager-plan.md`, its README and shots), the launcher's mock, and
`mdshared-ui` / `mdshared-app` in `markdown-shared-rs` at the `rev` this repository pins (`gfx`,
`chrome`, `menu`, `rows`, `tree`, `pane`, `scroll`, `segments`, `overlay`, `platform`; `job`,
`watch`, `places`, `single`, `relaunch`, `settings`). `Cargo.lock` on `main`. `docs/RELEASE.md`
(the Windows files are not code-signed). Windows API facts come from my knowledge of the APIs
and two web searches on 2026-11-22; what a check did not settle is marked **spike**.

**What I ran.** The mockup, to make the shots; all were looked at (section 14). Nothing else: no
API call, no measurement, no process was touched. **Every figure of cost and size in sections 7 and
8 is an estimate or a target. Every process, number and graph in the pictures is made up.**

---

## 1. The recommendation

**Build it, small and honest**, as its own app on the shared crates, Windows first, in **about 56
working days** (11 weeks) for Windows, **+12 days** for Linux and macOS later. The first useful
slice is **about 18 days** (section 11).

It is worth building only if it is different from the three free tools you already have. Windows
Task Manager is fine for looking; System Informer and Process Explorer are deeper; PowerToys has
"File Locksmith"; Autoruns does startup. This plan does not try to beat them at depth. It is
different in four ways, each drawn:

1. **It runs as you, not as administrator**, shows everything Windows gives a standard user, and
   says exactly which number needs rights and why (`tm-details-denied`, `tm-elevate`). Elevation is
   asked **per action**, never for the whole window.
2. **It is light and keeps itself honest**: it shows its own cost, stops sampling when hidden, and is
   held to the family's budgets (`tm-cost`, `tm-settings`). Windows Task Manager is not light.
3. **"What is holding this file or folder"** is a page of its own, with the quick and the deep
   method, and refuses the one dangerous thing (`tm-files`, `tm-files-deep`).
4. **Startup apps tell you what turning them off writes** (`tm-startup`), and never delete.

Three decisions carry the rest:

1. **One sampler, one snapshot.** One thread, one `NtQuerySystemInformation` call a second, gives
   every process and every core. The UI reads an immutable snapshot. Nothing else polls (section 7).
2. **No kernel driver, ever.** System Informer needs one to end protected processes; antivirus
   products flag tools of this kind for exactly such things. Without a driver, a protected process
   (lsass on many PCs, csrss, MsMpEng) cannot be ended by anyone, and we say so (section 6).
3. **Per-action elevation through a one-shot helper**, with "restart as administrator" as the
   second choice (section 6).

## 2. What it is, and what it refuses to be

It is: a list of processes (grouped like Task Manager, as a true tree, or flat) with the columns of
section 3; end task, end tree, suspend, priority, affinity, open file location; a details pane for the
selected process; graphs of CPU (also per core), memory, disks, network and GPU with up to an hour of
history; startup apps; services; "what is holding this file or folder"; a tray icon with a small
graph; a compact always-on-top window.

It refuses:

- **Replacing Task Manager.** No Ctrl+Shift+Esc hook, no registry redirect (Image File Execution
  Options is exactly what malware does). You open it on purpose, or from the tray.
- **Being Process Explorer or Process Hacker.** No kernel driver, no code injection, no memory
  editor or string scan, no token or ACL editor, no DLL injection, no thread stack walks (that
  needs symbols and, often, a driver).
- **Being Autoruns.** Startup covers four places (section 5); Winlogon, AppInit, services' own
  start, shell extensions and the like are listed in "Not covered", not edited.
- **A service or a background agent.** No Windows service, no scheduled task of its own. With the
  tray on it is a normal process in your Run key, and you can turn that off.
- **Temperatures, fans, power, SMART.** They need vendor drivers or WMI; the GPU temperature in
  the mockup's tile is the one place it is a question (section 7).
- **Network use.** None, except its own updates. "Search online" is off, and the signature check
  does not look anything up (section 10).
- **Remote machines.** Local only.

## 3. Processes

**Three views, one table** (`tm-procs`, `tm-tree`, `tm-flat`): **Grouped** (Apps, Background,
Windows processes, as Task Manager; an app owns its children, collapsed to one line with a count and
summed numbers: Firefox (5)), **Tree** (the true parent and child tree, roots as Windows has them),
**Flat** (sorted by any column). Default: **Grouped**, because that is what you already read, one
key (T) toggles Tree. Which processes are "Apps" is a rule (has a top-level visible window, or its
parent chain leads to one), not a list.

**Columns.** Shown by default: Name, PID, Status, CPU, Memory, Disk I/O, Network, GPU. Available:
Commit, Handles, Threads, User, Publisher, Command line, Path, Started, Session, Integrity, Priority,
Parent. Columns are chosen and ordered from the header's context menu, and kept.

**Memory: private working set is the default, and the reason.** "Private working set" is the RAM
that only this process holds. It adds up across processes (shared pages are not counted twice, so
200 Chrome-like processes do not look like 40 GB), it is what Task Manager's "Memory" shows since
Windows 10, and it is the figure `CLAUDE.md`'s own budget uses ("private memory under 30 MiB"). It
comes free: `SYSTEM_PROCESS_INFORMATION` has a `WorkingSetPrivateSize` field since Vista, so no
process has to be opened. **Commit** (private bytes) is the other honest figure: what the process
has asked for, whether or not it is in RAM; it is what runs a machine out of memory and what finds a
leak, so it is one click away as a column, and the details pane shows both. "Working set" (with
shared pages) is not offered as a column: it is the number that misleads.

**CPU** is normalised the way Task Manager does (100% is all logical processors); a setting changes
it to "per core" (100% is one core, up to 1600%). Computed from the kernel and user times with the
**real elapsed time between two samples**, not an assumed second, so a sample after a pause is right.

**Disk and Network, said plainly.** What a standard user can get per process is the process's I/O
bytes (`IoCounters` in the same snapshot). That counts *all* I/O: files, pipes, the console, and
sockets. It is a fair "who is busy", not "who is using the disk"; the column is called **Disk I/O**
for that reason (the tooltip says so). True per-process disk and **per-process network bytes need a
Windows event trace, which needs administrator rights** (or membership of "Performance Log Users").
Standard user: the Network column shows each process's **connections** (`GetExtendedTcpTable` with
owner pid, no rights needed) and a dash; elevated, it shows Mbps (`tm-procs` is drawn as a standard
user: "5 conn", "16 conn"). That ETW session is one of the owner's questions (5), +4 days.

**GPU** per process comes from the PDH counters `GPU Engine` and `GPU Process Memory` (documented,
no rights), summed per process id across engines. Their cost is the risk of section 7.

**Heat.** A cell's background deepens with its value, one hue (the accent), so the eye finds the
hog without reading. Nothing blinks. **New and ended processes** can be marked for two seconds
(green, red) as Process Explorer does; off by default, a setting.

**Search** filters by name, description, command line and path, as you type (`tm-search`); Esc
clears. **Keys:** Del end task, Shift+Del end tree, Ctrl+Space suspend or resume, F5 refresh now,
Ctrl+L open file location, Alt+Enter details, T/F/G the views, Ctrl+F search, 1 to 5 the pages.

**Actions** (`tm-context`):

| Action | Calls | Standard user |
|---|---|---|
| End task | `OpenProcess(PROCESS_TERMINATE)`, `TerminateProcess`; for a window app, first `WM_CLOSE` to its windows and a 3 s wait (a setting: "ask nicely first") | your own processes |
| End tree | collect descendants from the snapshot (by parent pid **and** creation time, so a reused pid is not a child), end leaves first; shows the list (`tm-endtree`) | your own |
| Suspend / Resume | `NtSuspendProcess` / `NtResumeProcess` (undocumented, used by every such tool) | your own; marks it "Suspended" |
| Priority | `SetPriorityClass`; Realtime needs a privilege, so it is marked | your own, except Realtime |
| Efficiency mode | `SetProcessInformation(ProcessPowerThrottling)` plus `IDLE_PRIORITY_CLASS`, as Task Manager | your own |
| Affinity | `SetProcessAffinityMask`; dialog with presets (`tm-affinity`); resets when the process ends, and says so | your own |
| Open file location | `SHOpenFolderAndSelectItems` | all with a known path |
| Copy command line / path | clipboard | all |
| Create dump file | `MiniDumpWriteDump` to a file the user picks; large, so it asks | your own |
| Search online | off by default (section 10) | |

Every action that fails with "access denied" opens the elevation card (`tm-elevate`, section 6), once
per action, never silently.

### The details pane (`tm-details*`)

A pane at the right (Alt+Enter or the button), tabs **Overview, Modules, Handles, Network,
Environment**. What each needs, as drawn:

| Tab | How | Standard user |
|---|---|---|
| Overview | path `QueryFullProcessImageNameW`; command line `NtQueryInformationProcess(ProcessCommandLineInformation)`; user from the token; started, priority, affinity, integrity, session | own and most others; **spike**: whether the limited-information right is enough for the command line on every Windows 10/11 build (sources differ) |
| Modules | `EnumProcessModulesEx` (both 32- and 64-bit lists) | your own processes; others: shield card |
| Handles | enumerate once system-wide, keep those of the pid; names by duplicating each file handle into this process, in a worker with a 100 ms limit, because `NtQueryObject` can block forever on some pipe handles | your own processes only |
| Network | `GetExtendedTcpTable`/`GetExtendedUdpTable`, owner pid, state; **no bytes** | all |
| Environment | `ReadProcessMemory` of the PEB block (WoW64 processes via `ProcessWow64Information`) | your own, same bitness rules |

For a process you cannot open, the pane says what it cannot show, why, and offers "Show this once…"
(`tm-details-denied`), which goes through the helper. A **protected** process (PPL) cannot be
opened by anyone without a driver; the card says that instead, and offers nothing.

## 4. Performance

Left, one tile per resource (`tm-perf-*`); right, the chosen one: a graph with a **window of 60
seconds, 10 minutes or 1 hour**, the numbers beneath, the facts, and **"Busiest now"**, the five
processes behind the graph, which Task Manager does not have.

| Page | Source | Needs |
|---|---|---|
| CPU, overall and per logical processor (`tm-perf-cpu`, `tm-perf-cores`) | `NtQuerySystemInformation(SystemProcessorPerformanceInformation)`: idle, kernel and user time per core, microseconds; kernel share drawn as a second line. Speed `CallNtPowerInformation(ProcessorInformation)`. Cores, caches `GetLogicalProcessorInformationEx` | nothing |
| Memory (`tm-perf-mem`) | `GlobalMemoryStatusEx`, `GetPerformanceInfo` (commit, cache, pools, handles, threads); composition (in use, modified, standby, free) `SystemMemoryListInformation`; compressed from the Memory Compression process. Speed and slots from the SMBIOS table, `GetSystemFirmwareTable('RSMB')`: no WMI | nothing |
| Disk (`tm-perf-disk`) | PDH `PhysicalDisk(*)`: `% Disk Time`, `Disk Read/Write Bytes/sec`, `Avg. Disk sec/Transfer`; type (SSD/HDD) `IOCTL_STORAGE_QUERY_PROPERTY` | nothing |
| Network (`tm-perf-net`) | `GetIfTable2`: bytes per adapter, link speed (microseconds). **No per-program traffic** (section 3), and the page says so | nothing |
| GPU (`tm-perf-gpu`) | PDH `GPU Engine(*)\Utilization Percentage` (engines 3D, Copy, Video Decode, Video Encode), `GPU Adapter Memory`, `GPU Process Memory`. Driver and DirectX version from DXGI | nothing. **Temperature: not shown** (vendor API) |

**History.** One ring buffer per series of `u16` (per mille). One hour at one sample a second of 16
cores + memory + 2 disks + 2 adapters + 5 GPU series is about 190 KB; the 10-minute and 1-hour
windows are drawn from the same buffer, decimated (max per bucket, so a spike is never lost).

**Per-process history** is kept only for the **selected** process and the top ten by CPU (two
minutes of `u8`), for the two small graphs of the details pane. Not for 300 processes: that is
the line between 15 MiB and 60 MiB.

## 5. Startup apps, services, and what is holding a file

### Startup (`tm-startup`, `tm-startup-admin`)

Four sources, which together are what Windows Settings and Task Manager list:

| Source | Read | Turning off writes | Standard user |
|---|---|---|---|
| `HKCU\...\Run`, `RunOnce` | registry | `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run`, value of the same name: first byte 03 = disabled, 02 or 06 = enabled, with the time (**spike**: diff what Windows Settings writes on this build, and copy it exactly) | yes |
| `HKLM\...\Run` and `Wow6432Node\...\Run` | registry | the same under `HKLM` (`StartupApproved\Run` / `Run32`) | **no: helper** |
| Startup folders (user, all users) | files | `StartupApproved\StartupFolder` | user: yes; all users: helper |
| Scheduled tasks with a logon trigger | `ITaskService` (COM) | the task's `Enabled` flag | your own tasks; others: helper |

**It never deletes** the entry. Turning it back on restores the old state byte for byte, which is
what the pane's text says. It does not stop a running program; it says so. "Open file location" and
"Registry entry" (opens the right key in `regedit`) are on every row.

**Startup impact** is the one thing in the mockup that cannot be had for free. Windows computes it
from a boot trace we cannot read. We can **record it ourselves**: if mdtask is itself an entry (the
tray), it samples CPU and I/O of every process that appears in the first 60 seconds after sign-in,
keeps five sign-ins, and shows Low, Medium, High from that. If it was not running at sign-in the column
shows "Not measured". The mockup shows values for illustration (question 8).

**Not covered** (listed, with a count, not edited): Winlogon Userinit and Shell, AppInit DLLs, Image
File Execution Options, Active Setup, shell extensions and browser helpers, services that start
automatically (they are on the Services page).

### Services (`tm-services`)

`OpenSCManager` with enumerate rights, `EnumServicesStatusExW(SC_ENUM_PROCESS_INFO)` gives name,
display name, state and **the process id**, so a service links to its process and back. Startup type,
binary path and account by `QueryServiceConfigW`, description and delayed start by
`QueryServiceConfig2`, read once and refreshed with `NotifyServiceStatusChange` while the page is
shown. **Start, Stop, Restart and Startup type need rights on almost every service**, so each goes
through the helper, per action, marked with the shield in the buttons. Filters: Running, Stopped, Not
Microsoft. Dependants are shown and **a stop that would stop others asks first**. It does not edit
the recovery options or the logon account, and "Open Services" is the one link for that.

### Files in use (`tm-files`, `tm-files-deep`)

The well-loved tool (PowerToys' File Locksmith, Unlocker, LockHunter). Two methods, because neither
covers the other:

| | **Quick**: Restart Manager | **Deep**: handle scan |
|---|---|---|
| Calls | `RmStartSession`, `RmRegisterResources`, `RmGetList` (twice: first for the size) | `NtQuerySystemInformation(SystemExtendedHandleInformation)` once, then match |
| Finds | programs with the file open or **loaded as a DLL**, with their kind (application, service, Explorer, console), the service short name, and whether they can be restarted | any handle to any file or **folder**, **working directories** (a console sitting in the folder), by file id |
| Misses | folders and the current directory of programs; kernel-held locks | handles of processes it cannot open (protected: counted, and shown as a number) |
| Time | milliseconds, **spike**: it is known to be slow on some machines with network volumes | **estimate** one to two seconds for about 100,000 handles, cancellable, with a count |
| Rights | none for your own processes; SYSTEM's listed with less detail | your own processes without; elevated for the rest |

A third candidate, `NtQueryInformationFile(FileProcessIdsUsingFileInformation)` (class 47), asks the
kernel directly "which processes have this file open" given one handle to it. A web check found it
only in a community example, not in the PowerToys source; **spike (half a day)**: if it works
unelevated on files and folders it replaces the quick method and half the deep one. The plan keeps
Restart Manager as the supported route and treats this as an optimisation.

**The deep scan matches by file identity** (`FileIdInfo`: volume serial and 128-bit id), not by the
path string, so hard links, `subst` drives, 8.3 names and `\\?\` forms all agree; for a folder it
matches any handle whose final path starts with it.

**What the page offers per program:** End task, **Close app** (`RmShutdown` for a restartable
application: it asks politely and saves nothing for you), **Show in Processes**, Stop service
(helper). **What it refuses:** closing a handle inside another program (the `DuplicateHandle` with
`DUPLICATE_CLOSE_SOURCE` trick). It corrupts that program's state, and the red note on the page says
so. Question 10 adds an Explorer verb "What's using this?" (a static per-user registry verb that
opens `mdtask --files <path>`: no DLL in Explorer, one registry write, off by default, +1 day).

## 6. Elevation: run as you, ask per action

`mdtask.exe` has the manifest `asInvoker`. It never asks for rights at start.

1. **The action fails with `ERROR_ACCESS_DENIED`** (or is known in advance to need rights, which the
   UI marks with the shield). The window shows the card of `tm-elevate`: what is being done, to whom,
   and two choices.
2. **"Do this once" (the default, the first choice):** the window starts **`mdtask.exe --op`** with
   `ShellExecuteExW(verb="runas")`. Windows shows its own UAC prompt. The helper has **no window**,
   does **one operation from a fixed list** (set priority, end task, end tree, suspend, resume,
   affinity, service start/stop/restart/startup type, HKLM startup toggle, read details of one
   process, start an ETW session), reports on a pipe the parent created (named with a random id,
   access limited to the user's SID), and exits. It has no persistence and no listening
   socket; each operation validates its arguments (a pid and its creation time must match, a service
   name must exist); the list is a `match`, not a plug-in point.
3. **"Restart as administrator":** relaunches the same exe elevated with `--restore` (the page, the
   selection, the columns), through `mdshared_app::relaunch` and `single` as the family already
   does, and closes the first window when the second answers. Everything in the new window runs
   as administrator, and the status bar says **Administrator** in green (`tm-procs` shows
   **Standard user**). Needed for: a trace session that stays on (per-process network), or many
   actions in a row.

**What an unsigned exe does to this.** UAC shows an unsigned program's prompt in yellow, with
"Unknown publisher". The helper is the same unsigned exe, so the owner will see that prompt each time.
That is the strongest argument for signing this one app (question 6), and the plan does not hide it.

**What nobody can do:** protected processes (PPL: parts of Defender, and lsass on PCs that enable it)
and the System process. The pane and the menu say "protected by Windows", not "access denied".

## 7. The Windows plumbing, and what each choice costs

**Process list: `NtQuerySystemInformation(SystemProcessInformation)`.** One call returns every
process with its threads: pid, parent, creation time, kernel and user time, handle and thread
counts, working set, **private working set**, commit, peak figures, and the I/O counters. It is what
Task Manager and every such tool use. Microsoft documents the function as "may be altered or
unavailable in future versions" and points to other APIs, none of which gives all of this in one
call. We take that risk deliberately and isolate it in one file (`sys/nt.rs`, about 200 lines of
hand-written FFI: the struct layouts, the buffer-size loop on `STATUS_INFO_LENGTH_MISMATCH`, and a
version check that falls back to Toolhelp, which gives names, pids and parents but no CPU).

| Option | Gives | Cost | Verdict |
|---|---|---|---|
| **`NtQuerySystemInformation`** | everything above, one call | **estimate** 0.5 to 2 ms for 300 processes and 4,000 threads (the buffer is a few hundred KB) | **use** |
| Toolhelp (`CreateToolhelp32Snapshot`) | names, pids, parents, thread counts | a snapshot of heaps and modules is slower and no CPU times | fallback only |
| PDH `\Process(*)` | per-process counters by name | wildcard expansion is slow, names clash (`firefox#3`), reused | **no** for processes; yes for disks and GPU |
| **WMI** (`Win32_Process`, `Win32_PerfFormattedData`) | everything, slowly | **hundreds of ms a query**, starts `wmiprvse.exe`, spikes CPU, and the COM security setup is a pile of code | **never** |
| ETW (kernel trace) | per-process disk and network bytes, exact | a session needs administrator; the events are many; parsing is real code | **only elevated**, optional (question 5) |
| `sysinfo` crate | processes, cores, memory, cross-platform | new in `Cargo.lock` (section 8); hides private working set, I/O, handles, threads details we want | **no** on Windows |

**Everything per process that is not in the snapshot** is looked up **once per process** (keyed by pid
and creation time, never by pid alone) and cached: the full path, the user (token), the command line,
the icon (`SHGetFileInfo`, lazily, only for rows on the screen), and the **signature**.

**Signatures.** `WinVerifyTrust` with revocation checks off and `WTD_CACHE_ONLY_URL_RETRIEVAL`, so it
**never goes to the network**; it reads the certificate chain from the local store and the catalog.
It costs milliseconds to tens of milliseconds per file and touches the disk, so it runs on a worker
thread, only for rows that are visible and only while the Publisher column or the pane needs it.
The consequence is stated in Settings > Privacy (`tm-privacy`): a revoked certificate is not noticed.

**GPU through PDH.** The `GPU Engine` counter has one instance per (process, engine), named
`pid_1234_luid_..._engtype_3D`. On a busy machine that is hundreds of instances. Adding the wildcard
path expands it once, but instances appear and vanish, so it has to be re-expanded: **estimate** a
few ms per collect plus tens of ms per expansion. **Spike (one day, section 12): measure it on this
PC.** If it breaks the budget: expand every five seconds, not every second, or query
`D3DKMTQueryStatistics` directly (undocumented-ish, used by System Informer; cheaper, more code).

**PDH itself is loaded lazily**, the first time the Performance or the GPU column needs it, so it is
**nothing on the launch path** (rule 1).

**The sampling loop and what "idle" means.** A tool whose job is to sample cannot be at zero CPU
while it samples, so the family's budget is restated for it, and measured, not assumed:

| State | Sampling | Target |
|---|---|---|
| Window hidden, minimised, occluded, or session locked; no tray icon | **stopped**, no timer, no thread awake | **0% CPU, 0 wake-ups**: the family's idle budget, unchanged |
| Tray icon only | every 5 s: CPU, memory, and nothing else | under 0.02% of a core (**estimate**) |
| Window visible | every 1 s (setting 0.5, 1, 2, 5 s); a redraw only when something changed | under 1% of one core on average; a sample under 5 ms (**targets**) |
| Window visible, nothing selected, Performance page | the process list is still read (cheap) but no per-process lookups | |

A hidden window is found by the winit `Occluded` event, the minimised state, and the session-lock
notification. After a pause the first sample's elapsed time is the real one (section 3), and the
graphs show a gap rather than a line across it. The status bar always shows **the app's own cost,
measured by itself** (`GetProcessTimes` on itself, the sample's own duration, the private
bytes: `tm-procs`, `tm-cost`), and the row of this app is marked "this app" in the list. If it is
heavier than its budget, that is a bug a user can see.

**Own memory** (**estimate**): process table 300 rows at 300 B = 90 KB; icons 300 at 1 KB; history
190 KB; per-process history 12 KB; strings and caches under 1 MB; the font and the painter as the
other apps. **Target: 12 to 18 MiB private, budget 30 MiB.** PDH adds some when loaded. To be
measured with the family's method (`docs/CODE.md`) at each phase.

**32 and 64 bit.** Ship **x86-64**, which runs on Windows on ARM under emulation; an ARM64 build is
a later question, and the family ships none today (`docs/RELEASE.md`). A 64-bit process reading a
32-bit one: modules by `EnumProcessModulesEx(LIST_MODULES_ALL)`; environment and command line by the
WoW64 PEB (`ProcessWow64Information`, 32-bit structure layouts); the command-line information class
avoids the PEB entirely where it works.

**What antivirus may flag.** Honest list, since this class of tool is flagged: opening other
processes (`OpenProcess` with `PROCESS_VM_READ`), `SeDebugPrivilege` (we enable it **only in an
elevated helper, only when an operation needs it**), the system-wide handle enumeration, a suspend
call, a kernel ETW session, `TerminateProcess`. System Informer and Process Hacker are on some
products' lists for such behaviour plus a driver. Mitigations, in order of value: **no driver, no
injection, no packer, no obfuscation; sign the exe** (question 6); the helper is the same exe with a
fixed op list; a README line "what it calls and why"; submit each release to Microsoft Defender's
false-positive form; and **test each release candidate against Defender** (and, if the owner
likes, VirusTotal) before tagging. This is a risk, not a certainty, and it is the hardest one (section 13).

## 8. The shared and the app-specific split, and prices (rule 7, rule 8)

**Already in `markdown-shared-rs`, used as is:** `gfx::Painter` (text, `rbox`, `fill_a`, icons),
`chrome` (title row, tabs for the five pages, window buttons), `menu` (the context and menu-bar
menus), `overlay` (dialogs, backdrop), `segments` (the History and view switches), `scroll`, `pane`,
`rows` and `tree` (virtual rows; the tree's open/closed state), `settings` and its pages
(`tm-settings`), `job` (the sampler, the signature worker, the deep scan), `watch` (not needed),
`places` (remembered column widths and window size), `single`, `relaunch`, `platform` (the
Windows window details), `update`/`selfupdate`.

**New, and shareable first (rule 8):**

| What | Where | Days | Notes |
|---|---|---|---|
| `graph`: area and line graph over a `&[u16]` ring, grid, labels, a sparkline variant, a gap for missing samples; `History` ring buffer with decimation | `mdshared_ui::graph` (new module, additive: no existing caller changes) | 2 | The mock's `tmpages::graph` draws it by one-pixel columns, which is enough; the launcher's and a future monitor's use it too |
| `table`: a column header with sort, resize and a total line; heat cells | `mdshared_ui` beside `rows`, only if the file manager's table is not enough | 1 to 2 | The file manager plan has a details view with columns; take it, do not write a second |
| tray icon and its flyout; always-on-top window flag | `mdshared_ui::platform` (Windows `Shell_NotifyIconW`; Linux and macOS later) | 3 (counted in phase 10) | The family has no tray today. The launcher plan may need one too: write once |

**App-specific, in `bins/mdtask` and a `crates/mdtask-sys` (the platform layer: sampling, actions):**
the table of processes and its three views, the pages, the details pane, the elevation helper, the
startup and service models, the file-holder scan, the sampling policy. `mdtask-sys` is a crate with
one `trait Sampler` and three implementations, so Linux and macOS are additions, not rewrites.

**Dependencies, priced.** None is added by this plan.

| Candidate | What it adds to `Cargo.lock` / the exe / the launch path | Verdict |
|---|---|---|
| `windows-sys` 0.61 (already in the lock, `bins/mdedit` and `mdedit-core`) | **no new crate**. New *features* (`Win32_System_Threading`, `_ProcessStatus`, `_Diagnostics_ToolHelp`, `_Services`, `_Registry`, `_NetworkManagement_IpHelper`, `_Security_WinTrust`, `_System_RestartManager`, `_System_Performance` for PDH, `_UI_Shell`, `_System_Power`): more bindings compiled, **no measured exe growth expected**, **estimate** under 100 KB; nothing on the launch path | **use**; the NT calls are declared by hand (about 200 lines), because `windows-sys` has only part of them |
| `windows` 0.62 (in the lock) | COM (`ITaskService`) is easier with it; already compiled for another purpose | use **only** for `ITaskService` if needed; otherwise not |
| `sysinfo` | a **new** crate, and with default features rayon and crossbeam too; exe growth to be measured; hides the fields we need | **no** (Windows); for Linux and macOS it is an option to price then |
| `libc` (in the lock) | `/proc`, `sysctl`, `proc_pidinfo` on Linux and macOS | **use** later |
| `ntapi`, `winapi`, `wmi`, `windows-service` | new crates | **no** |

## 9. Linux and macOS, after

Not before Windows has run for a while. The `Sampler` trait makes them an addition.
**Linux:** `/proc/[pid]/stat` and `statm`, private memory from `/proc/[pid]/smaps_rollup`
(Private_Clean + Private_Dirty, a little costly: only for visible rows), `/proc/stat` for cores,
`/proc/meminfo`, `/proc/net/dev`, `/proc/diskstats`, `/proc/[pid]/io` for your own processes
(others need root: the same shield story), GPU per vendor sysfs where it exists (not promised).
End task is `kill`; priority is `setpriority`; "Startup" is `~/.config/autostart` and systemd user
units; "Services" is systemd, `systemctl` via D-Bus (a new dependency; question 13). **macOS:**
`proc_listpids`, `proc_pidinfo(PROC_PIDTASKINFO)` and `proc_pid_rusage` (`ri_phys_footprint`, the
number Activity Monitor shows), `sysctl(KERN_PROC)`, `host_processor_info` per core, `vm_statistics64`;
only your own processes' details without root; GPU through IOKit statistics (maybe); startup items
and launchd agents for "Startup". **About 6 days each** for the sampler, the actions and the shots.
The pages for startup, services and files in use are Windows-shaped, and on the other systems
become the platform's honest equivalent or are absent (the page tab is then not shown).

## 10. Menus, keyboard, tray, compact, privacy

**Menus.** A menu bar (File, View, Process, Tools, Help) that Alt shows, as mdedit and the file manager.
**Process** has the actions of the context menu; **View** the views, columns, "Always on top", "Compact
mode", the history window; **Tools** "Run new task…" (`CreateProcess` as you: a dialog with a path
field, "Run as administrator" through the helper), Options. The context menu of a process is
`tm-context`; of a service and of a startup entry the same pattern with their own items.

**Tray** (`tm-tray`): an icon of **two bars, CPU on the left and memory on the right**, 16 px, red
when CPU is over 90% or memory over 95%; the tooltip repeats the numbers. Click opens a **flyout** with
the five small graphs and "Busiest now"; double click opens the window. It is **off by default**;
turning it on writes the user's Run key entry (`--tray`), and the Startup page shows it
(`mdtask (tray)`), so there is no hidden autostart. **Compact mode** (`tm-compact`): a 380 px wide window,
always on top if you want (the pin), no tabs: the five graphs and the top four processes; Esc or the
button returns to the full window; the mode is remembered. Sampling is the visible one (1 s).

**Settings** (`tm-settings`, `tm-privacy`): Sampling, Processes, Tray and compact, Startup and
services, Privacy, Advanced; and it explains the costs (section 7) in the page itself.

**Privacy and the network.** None, but its own update check. **"Search online" is off and opens the
default browser on a search page for the file name**: the app itself connects to nothing, and the
setting says what is put in the address. Signatures never look anything up (section 7).

## 11. Phases, days, and the first useful slice

One day is one maintainer-day with agents at the repository's bar: a test that fails without the fix,
a hand run on Windows, an audit pass (the convention of `ecosystem-ideas.md`). **Estimates.**

| # | Phase | Days | Leaves you with |
|---|---|---|---|
| 0 | **Spike**: sampler cost on this PC, the GPU path, `FileProcessIdsUsingFileInformation`, Restart Manager timing, `StartupApproved` bytes, command-line right, unsigned UAC prompt | 3 | Numbers, a go or no-go per feature; the spike is thrown away |
| 1 | `mdshared_ui::graph` and `History` (shared first, both apps' shots before and after) | 2 | Graphs for every family app |
| 2 | `mdtask-sys` Windows sampler: process snapshot, cores, memory, per-process cache (path, user, icon), the `Sampler` trait, the budget test (own CPU, own memory) | 5 | A thread that fills a snapshot |
| 3 | **Processes page**: table, three views, columns, sort, search, heat, end task and tree, suspend, priority, menus, keys | 6 | **The first useful slice, with 4** |
| 4 | Performance: CPU (cores), memory, history windows; the tile list | 2 | `tm-perf-cpu`, `tm-perf-mem` |
| 5 | Disks, network adapters, GPU (PDH, lazily loaded) | 5 | The other four pages |
| 6 | Details pane: overview, modules, handles, network, environment; signatures worker | 6 | `tm-details*` |
| 7 | Elevation: the helper, the card, relaunch, the allow-list, tests for each op's refusal | 4 | `tm-elevate`, per-action rights |
| 8 | Startup apps (four sources) | 5 | `tm-startup` |
| 9 | Services | 4 | `tm-services` |
| 10 | Files in use: quick and deep | 4 | `tm-files*` |
| 11 | Tray (shared), compact mode, always on top, the sampling policy (hidden, locked, tray-only) and its measurement | 4 | `tm-tray`, `tm-compact` |
| 12 | Settings, menus, session, hand run on Windows 10 and 11, shots in `tests/shots.sh`, CI | 5 | A release candidate |
| 13 | Packaging, Defender test, the README on what it calls, release notes, `docs/DESIGN.md` | 3 | v0.1.0 |
| | **Windows total** | **56** | about 11 weeks |
| 14 | Linux sampler and actions | 6 | later |
| 15 | macOS sampler and actions | 6 | later |

**The first useful slice: phases 0 to 4, about 18 days.** A window that opens fast, lists processes
grouped, as a tree or flat, with CPU, memory, disk I/O, connections and GPU if the spike allows it,
searches, ends a task or a tree, sets priority, suspends, shows CPU and memory graphs for an hour,
runs as you, tells you which actions would need rights, and shows its own cost. It lacks details,
startup, services, files, the tray; it is already better than the free Task Manager for the one thing
you do most, "what is eating the machine and kill it". The next most valuable slice is **Files in use**
(4 days, phase 10, which does not depend on the rest): do it second if you want a visible win early.

## 12. Testing, and the spikes

**Tests, one per behaviour, each failing with its fix reverted (rule 2).** The sampler's process
table is tested against **a fake snapshot** (a byte buffer in the real layout, built in the test) for
tree building, pid reuse (same pid, different creation time), CPU percentages over unequal intervals,
new and ended processes. The tree and grouped views are pure functions of the table and are tested
without a window. The helper's `match` on operations has a test for **each refusal** (a pid whose
creation time does not match, an unknown service name, an op not in the list). The startup toggle is
tested on a **scratch registry key** that the code takes as a parameter (never `HKCU\...\Run` of the
owner), and on a temp folder for the Startup folder. The deep scan is tested on handles the test itself
opens (a file, a folder, a current directory of a child it starts). The budget script
measures first frame, private memory, and **CPU of a hidden window over 60 seconds (must be 0 wake-ups)**
and of a visible one. Screenshots: every state of this plan goes into `tests/shots.sh` in both themes
(rule 6). Services and Run-key writes are **never** run by tests against the real machine.

**The spikes of phase 0 (3 days), each with the question and the result that changes the plan:**

1. Snapshot call: time and buffer size at this PC's 140 processes; if over 5 ms, sample at 2 s by default.
2. PDH GPU: collect and expansion times on this PC; if over 10 ms a sample, the five-second path or D3DKMT.
3. `FileProcessIdsUsingFileInformation` on a file and a folder, unelevated: yes means Quick gets simpler.
4. Restart Manager on a file on a network share and on a busy C: file: if it can take seconds, it runs on a worker with a cancel button.
5. What Windows Settings writes to `StartupApproved` (diff before and after); copy exactly.
6. The command-line information class with the limited right, on a SYSTEM process and a 32-bit one.
7. The UAC prompt for the unsigned helper, so you see it.

## 13. Risks

| Risk | Why it matters | Mitigation |
|---|---|---|
| **Trust: an unsigned process tool that opens other processes and asks for UAC** | **The hardest one.** Antivirus may flag it; SmartScreen shows "unknown publisher"; the UAC prompt is yellow. It decides whether anyone dares to run it | No driver, no injection; a fixed-op helper; sign this app (question 6); test each release against Defender; the README says what it calls |
| Per-process network and true disk need administrator | The two columns people look for are the two it cannot fill as a standard user | Say it in the UI (connections, "Disk I/O"); the optional ETW session behind the shield; question 5 |
| The sampler breaks the family's idle budget | The app's whole job is a timer | Stops when hidden (section 7), measured by script, own cost always shown |
| `NtQuerySystemInformation` layout changes | Undocumented fields | One file, a version check, a Toolhelp fallback, the fake-snapshot tests per known layout |
| PDH's GPU counters are slow or leak instances | Cost on the 1 s path | Spike 2; lazy load; five-second path |
| Killing the wrong thing | Data loss is what ends a process tool | End tree lists what dies; system processes ask twice and are marked; protected ones are not offered; undo does not exist, so the dialog says that |
| A hung handle query freezes the pane | `NtQueryObject` on some pipes never returns | Worker with a time limit, abandoned (leaked) rather than waited for |
| It duplicates three free tools | The reason to build is the family, not the feature | Section 1: four differences, the first slice is small, and question 2 asks whether to build at all |
| Scope creep to Process Explorer | Each missing feature is easy to ask for | Section 2's list of refusals; "A new feature goes in only when the owner asks" (`CLAUDE.md`) |

## 14. What drawing taught, and which shot shows what

Things the pictures made me change in the plan, which a text plan would have missed:

- **The Network column cannot honestly show Mbps for a standard user.** The first draw did. It now shows
  connections (`tm-procs`, `tm-flat`) and the plan says why. "Disk" became "Disk I/O" for the same reason.
- **The sampling cost belongs in the window**, not only in a document: the status bar and `tm-cost`.
- **The elevation card needs two choices with their consequences written out** (`tm-elevate`), and the
  unsigned prompt is part of the picture.
- **The deep file scan wants a counted "could not read" line**, or its answer looks complete when it is not (`tm-files-deep`).
- **Startup needs a "what this writes" box** to be trusted (`tm-startup`).
- **The tab strip of the shared chrome draws a document icon and a close button on every tab**
  (`tm-*`, all of them). Page tabs should have neither; this needs a tab variant in `mdshared_ui::chrome`
  (an additive option, half a day, in phase 1). At 860 px the labels clip (`tm-procs-compact-*`).

| What to look at | Shot |
|---|---|
| Grouped (default), the tree, flat with the wide columns, a search | `tm-procs`, `tm-tree`, `tm-flat`, `tm-search` |
| Details: overview, modules, handles, network, environment, and a process that needs rights | `tm-details`, `-modules`, `-handles`, `-net`, `-env`, `-denied` |
| The context menu with its Priority submenu | `tm-context` |
| The questions: end a tree, affinity, administrator rights | `tm-endtree`, `tm-affinity`, `tm-elevate` |
| Performance: CPU, per core, memory, disk, network, GPU | `tm-perf-cpu`, `-cores`, `-mem`, `-disk`, `-net`, `-gpu` |
| Startup (user entry; all-users entry with the shield), services | `tm-startup`, `tm-startup-admin`, `tm-services` |
| What is holding a file (quick) and a folder (deep) | `tm-files`, `tm-files-deep` |
| Tray flyout and the icon enlarged; the compact window | `tm-tray`, `tm-compact` |
| Options and the privacy page; the cost of the app itself | `tm-settings`, `tm-privacy`, `tm-cost` |
| At 860x600 | `tm-procs-compact`, `-tree-`, `-details-`, `-context-`, `-perf-cpu-`, `-perf-cores-`, `-startup-`, `-services-`, `-files-` |

Each exists as `-light.png` and `-dark.png`. All were looked at in at least one theme and the dark and
light of the main ones; nothing here is the owner's approval, which is given by looking (rule 6).

## 15. The owner's questions

1. **Name.** `mdtask`? (It follows `mdview`, `mdedit`.) Or `mdtop`, `mdmon`.
2. **Build it at all, given Task Manager, System Informer, Process Explorer, PowerToys File Locksmith
   and Autoruns exist?** The plan says yes, small, for the four differences in section 1. If those four
   do not matter to you, the answer is no.
3. **Elevation:** one-shot helper first, "restart as administrator" second (recommended), or the reverse?
4. **Memory column default:** private working set (recommended), or commit?
5. **Per-process network and true disk:** accept a dash and connections as a standard user
   (recommended for the first release), or build the optional elevated trace session (+4 days)?
6. **Sign this app?** The Windows files are not signed today (`docs/RELEASE.md`); for a tool that
   asks for UAC and opens other processes it matters more than for the editor. A certificate costs
   money, and the process of getting one takes time; I did not price it.
7. **Run on sign-in with a tray icon:** off by default (recommended), or on?
8. **Startup impact:** record it ourselves if the tray is on (section 5), or show no impact column?
9. **"Search online":** ship it disabled (recommended) or leave it out of the first release?
10. **Explorer verb "What's using this?"** (opt-in, +1 day), or only the page?
11. **Linux and macOS:** after Windows is released (recommended), or never?
12. **Services:** read-only plus start/stop/restart (recommended), or also startup type? (The mockup shows the button.)
13. **Dependencies later:** is a D-Bus crate acceptable for systemd on Linux, or `systemctl` by command?
14. **Order after the first slice:** Files in use next (4 days, visible), or details, or startup?

## 16. Decisions to copy into `docs/DECISIONS.md` if you approve

A separate app `mdtask`, Windows first, on the shared crates; one `NtQuerySystemInformation` sampler,
no WMI, PDH only for disks and GPU and loaded lazily, ETW only elevated and optional; runs as the
user, elevation per action through a one-shot helper with a fixed operation list; no kernel driver, no
injection; private working set is the default memory figure, "Disk I/O" and connections are what a
standard user gets, bytes per program only when elevated; sampling stops when the window is hidden
and the app shows its own cost; startup entries are switched through `StartupApproved` and never
deleted; no network use, and signatures are checked offline; no new crate. This plan changes no
earlier decision.
