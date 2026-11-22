# Commander plugins: Total Commander WCX/WFX/WLX/WDX, Double Commander, Midnight Commander

Research note for the file-manager plan. Written 2026-11-22. Facts marked **(unverified)** were
not read from a primary source in this pass; they are from memory or inference and must be
checked before a decision rests on them. Section 9 lists what was actually read.

## 1. Verdict

A commander-style file manager in Rust can load Total Commander plugins: Double Commander does it
with the same four exports-by-name ABI, and a Rust host needs no new dependency for it
(`windows-sys` already gives `LoadLibraryW`/`GetProcAddress`). But the four plugin types are not
equal. **WCX (packers) and WDX (content) are cheap and fit a hand-drawn UI. WFX (file systems) is
a large, threaded, network-facing feature. WLX (listers) wants a native child window inside our
softbuffer window, which is Windows-only and fragile.** Third-party native DLLs run in our
address space, and the release build is `panic = "abort"`: a plugin crash is a manager crash.

Recommendation, decisive:

1. **Ship built-in read-only archives first** (zip, tar family, 7z via crates; priced in 7.3), with
   no plugin system. This covers the common case and keeps the launch path untouched.
2. **Then add an MC-style external-command protocol** (extfs-like: `list` / `copyout`, plus a
   `mc.ext`-like open/view table). It is language-agnostic, out-of-process by nature, and cheap.
3. **If TC compatibility is wanted, do WCX read-only + WDX only**, loaded lazily on first use
   through `LoadLibraryW`, **behind a plugin host process** (one child exe, a pipe, a watchdog,
   a job object), 64-bit plugins only. Refuse 32-bit plugins, refuse WLX, refuse WFX until the
   virtual-folder model exists and someone asks for a specific file system.
4. Never load a plugin on the launch path or at idle; the first frame and idle CPU budgets stay
   intact because nothing is loaded until a file that needs it is opened.

Confidence: high on the ABI shape, loading mechanics and plugin counts (read from sources);
medium on the day estimates (judgement, no prototype); low on the exe-size figures (estimates).

| Piece | Days (working, incl. tests and shots) | Confidence | Note |
|---|---|---|---|
| Loader core: `LoadLibraryW`, W-then-ANSI lookup, ABI structs, `.ini` path | 2 | high | shared by all types |
| WCX read-only (open, list, extract, progress, volume change, password) | 5 | high | virtual-folder over an archive |
| WCX write (`PackFiles`, `DeleteFiles`, caps, options dialog) | 4 | medium | needs `GetPackerCaps`, plugin config window |
| WDX (columns, detect string, field types, search by content) | 4 | high | fields feed columns and sort |
| WLX in-process (child HWND, resize, key forwarding, DPI, dark mode) | 8-10 | medium | Windows only; fragile |
| WFX (virtual folders, threads, progress, credentials, timeouts, UI) | 12-15 | medium | largest piece |
| MC-style extfs scripts / external-command protocol | 4-5 | high | `list`, `copyout`, `copyin`, `rm`, `mkdir` |
| Plugin manager UI (list, enable, detect strings, config buttons) | 4 | high | hand-drawn like the rest |
| Sandbox / host process: pipe protocol, watchdog, job object | 8 | medium | 64-bit helper |
| 32-bit helper process on top of the host (separate i686 build, same IPC) | 5-6 | low | **(unverified)** |
| **Whole, option C (WCX-RO + WDX, in-process, allow-list)** | **~15** | medium | loader 2 + WCX 5 + WDX 4 + UI 4 |
| **Whole, option C with host process** | **~23** | medium | + host 8 |
| **Whole, option D (all four types, host, 32-bit helper)** | **~55-65** | low | + WCX write 4, WLX 10, WFX 14, extfs 5, 32-bit 6 |

## 2. The four Total Commander plugin types

### 2.1 Common facts

- A plugin is a plain DLL renamed to `.wcx` / `.wfx` / `.wlx` / `.wdx`. Exports are looked up by
  name, not by ordinal. Only a few are mandatory; the rest are optional and the host checks for
  their presence. DC's `uwcxmodule.pas` shows exactly that: three mandatory lookups, then a list
  of optional ones.
- **Calling convention**: `__stdcall` in the C headers on Windows. On 64-bit Windows there is
  one convention, so stdcall and cdecl collapse; in Rust `extern "system"` is correct for both.
  Double Commander declares `stdcall` on Windows and `cdecl` elsewhere (from its Pascal unit
  `wlxplugin.pas`/`wdxplugin.pas`), which is how `.so` builds of the same API work on Linux.
- **ANSI vs Unicode**: each path-taking function has an ANSI name and a `...W` name. The ghisler
  64-bit page says the 64-bit TC falls back to ANSI when no Unicode export exists, and that a
  Unicode-only 64-bit plugin is enough. DC prefers `W`: `OpenArchiveW` first, `ReadHeaderExW`,
  then `ReadHeaderEx`, then `ReadHeader`. **Prefer the `W` exports, fall back to ANSI** only
  for old plugins; ANSI means the system code page, which loses names.
- **Integers**: ghisler's page says "All integer parameters in plugin functions remain 32-bit";
  only pointers and handles grow on 64-bit. So `tHeaderDataExW` etc. use `int`/`u32` and
  pointers.
- **Packing**: DC's Pascal declares `THeaderData`, `THeaderDataEx`, `THeaderDataExW`,
  `tOpenArchiveData(W)` as `packed`. The C headers I fetched (ghisler SDK on GitHub) show no
  `#pragma pack` in `wcxhead.h`, `listplug.h`, `fsplugin.h`, `contplug.h` (as read through a
  summarising fetch, so **partly unverified**; the owner's brief said pack(1)). It does not matter
  in practice: I worked the layouts. `tHeaderDataExW` has two `WCHAR[1024]` (4096 bytes), then ten
  32-bit fields (40 bytes), so `CmtBuf` sits at offset 4136, already 8-aligned; packed and
  natural layouts agree on x64 and x86. `tRemoteInfo` is 5 x 32-bit (SizeLow, SizeHigh, a FILETIME
  as two DWORDs, Attr), no padding either way. **Use `#[repr(C, packed(1))]` anyway and
  `read_unaligned`**, which is correct under both readings.
- **Configuration**: each plugin that wants settings receives, after load,
  `PackSetDefaultParams` / `FsSetDefaultParams` / `ListSetDefaultParams` /
  `ContentSetDefaultParams` with a struct `{ size, PluginInterfaceVersionLow, PluginInterfaceVersionHi,
  DefaultIniName[MAX_PATH] }` (field names seen in the four headers). `DefaultIniName` is where the plugin
  should keep its `.ini`. DC passes `wcx.ini` / `wlx.ini` in its config dir (read in `uwcxmodule.pas`,
  `uwlxmodule.pas`). WLX docs: call `ListSetDefaultParams` immediately after load, before `ListLoad`.
- **File names and bitness**: TC 32-bit loads `.wcx/.wfx/.wlx/.wdx`; TC 64-bit loads the same base
  name with `64` appended: `.wcx64`, `.wfx64`, `.wlx64`, `.wdx64` (ghisler WDX SDK, "64-bit
  support"; the forum confirms for wcx64/wlx64). `wincmd.ini` lists plugins with the 32-bit
  extension and the 64-bit TC finds the `64` file itself (forum, ghisler.ch). **A 64-bit process
  cannot load a 32-bit DLL, and the reverse.** TC's answer is to ship two builds and two plugin
  sets; a third-party 64-to-32 wrapper (`Wcx6432`) exists for WCX only, without packing
  (forum thread; not read in full). Double Commander on Windows loads whatever the build's
  bitness allows; its own wiki says nothing about `.wcx64` **(unverified)**.
- **Matching plugin to file**: WCX by extension list in the registry and, optionally, by content
  (`CanYouHandleThisFile`, caps `PK_CAPS_BY_CONTENT`); WLX by a detect string
  (`ListGetDetectString`, a small expression language over `EXT=`, `MULTIMEDIA`, `FORCE`, file
  signature tests such as `[0]="PK"`: **expression grammar unverified, from memory**); WDX by
  `ContentGetDetectString` per file type; WFX by name in the "Network neighbourhood" list, not
  by file. In TC the registry is `wincmd.ini` sections per plugin type (e.g. `[PackerPlugins]`,
  `[ListerPlugins]`, `[FileSystemPlugins]`, `[ContentPlugins]`; names **unverified**).

### 2.2 WCX (packers)

Callbacks, from `wcxplugin.pas` (identical in meaning to `wcxhead.h`):
`TChangeVolProc(ArcName, Mode)`, `TProcessDataProc(FileName, Size)`,
`TPkCryptProc(CryptoNr, Mode, ArchiveName, Password, MaxLen)`, each with a `W` twin.

| Export | Role | Mandatory |
|---|---|---|
| `OpenArchive(W)` | takes `tOpenArchiveData{ArcName, OpenMode, OpenResult, CmtBuf, CmtBufSize, CmtSize, CmtState}`; returns handle | yes (A or W) |
| `ReadHeader` / `ReadHeaderEx` / `ReadHeaderExW` | next entry; `tHeaderDataEx` fields: ArcName[1024], FileName[1024], Flags, PackSize(+High), UnpSize(+High), HostOS, FileCRC, FileTime, UnpVer, Method, FileAttr, CmtBuf..., Reserved[1024]; the W form adds `MfileTime` in DC's declaration | yes |
| `ProcessFile(W)` | `(hArcData, Operation, DestPath, DestName)`, Operation `PK_SKIP=0`, `PK_TEST=1`, `PK_EXTRACT=2` | yes |
| `CloseArchive` | free handle | yes |
| `SetChangeVolProc(W)`, `SetProcessDataProc(W)` | host registers callbacks; progress callback returning 0 aborts (**return semantics from memory**) | optional but needed for UI |
| `PackFiles(W)`, `DeleteFiles(W)` | write support; flags `PK_PACK_MOVE_FILES=1`, `PK_PACK_SAVE_PATHS=2`, `PK_PACK_ENCRYPT=4` | optional |
| `GetPackerCaps` | bit flags `PK_CAPS_NEW=1, MODIFY=2, MULTIPLE=4, DELETE=8, OPTIONS=16, MEMPACK=32, BY_CONTENT=64, SEARCHTEXT=128, HIDE=256, ENCRYPT=512` | optional |
| `ConfigurePacker(Parent, DllInstance)` | plugin shows its own dialog, **needs a parent HWND** | optional |
| `CanYouHandleThisFile(W)` | content sniffing | optional |
| `PackToMem`, `StartMemPack`, `DoneMemPack` | in-memory packing (rarely used) | optional |
| `PkSetCryptCallback(W)` | host passes a password store callback; `PK_CRYPT_*` modes 1-6 | optional |

Open modes `PK_OM_LIST=0`, `PK_OM_EXTRACT=1`. Errors `E_END_ARCHIVE=10` ... `E_NOT_SUPPORTED=24`,
`E_UNKNOWN=32768`. Iterating an archive is a pull loop: `OpenArchive`, then `ReadHeaderEx` until
`E_END_ARCHIVE`, calling `ProcessFile(PK_SKIP)` for each entry you do not extract. **Many packers
re-scan from the top on every extract**, so a virtual folder needs one pass to list and a
fresh open per extraction batch. DC's `PackSetDefaultParams` reports interface 2.22.

### 2.3 WDX (content)

From `wdxplugin.pas` (stdcall on Windows): `ContentGetDetectString(DetectString, maxlen)`,
`ContentGetSupportedField(FieldIndex, FieldName, Units, maxlen)`,
`ContentGetValue(W)(FileName, FieldIndex, UnitIndex, FieldValue, maxlen, flags)`, optional
`ContentSetDefaultParams`, `ContentPluginUnloading`, `ContentStopGetValue(W)`,
`ContentGetDefaultSortOrder`, `ContentGetSupportedFieldFlags`, `ContentSetValue(W)`,
`ContentSendStateInformation(W)`, `ContentEditValue`.

Field types: `ft_numeric_32=1, numeric_64=2, numeric_floating=3, date=4, time=5, boolean=6,
multiplechoice=7, string=8, fulltext=9, datetime=10, stringw=11, fulltextw=12`. Return codes
`ft_nosuchfield=-1, fileerror=-2, fieldempty=-3, ondemand=-4, notsupported=-5, setcancel=-6,
delayed=0`. Flag `CONTENT_DELAYIFSLOW=1`: the host asks for cheap values first and fetches slow
ones on demand. Pure data in, pure data out: **no window, no callbacks**. This is why WDX is the
safest and cheapest type to host. (In DC's `wfxplugin.pas` the same `FsContent*` functions appear
for WFX content, a different spelling for file-system fields.)

### 2.4 WLX (listers)

From `wlxplugin.pas`: `ListLoad(W)(ParentWin, FileToLoad, ShowFlags) -> HWND`,
`ListLoadNext(W)(ParentWin, PluginWin, FileToLoad, ShowFlags)`, `ListCloseWindow(ListWin)`,
`ListGetDetectString`, `ListSearchText(W)`, `ListSearchDialog`, `ListSendCommand(ListWin, Command,
Parameter)` (copy, newparams, selectall, setpercent), `ListPrint(W)`, `ListNotificationReceived`,
`ListSetDefaultParams`, `ListGetPreviewBitmap(W)`. Show flags `lcp_wraptext=1, fittowindow=2,
ansi=4, ascii=8, variable=12, forceshow=16, fitlargeronly=32, center=64, darkmode=128,
darkmodenative=256`.

**The hard part**: `ListLoad` receives the host's parent `HWND` and **creates its own child window**
inside it, returning that handle. The plugin then owns painting, scrolling, keyboard focus and
its own message loop inside that rectangle. The host only resizes the child (`MoveWindow`) and
forwards some keys. Nothing here fits a framebuffer UI where we blit pixels. See 5.6.

### 2.5 WFX (file systems)

From `wfxplugin.pas` (prototypes there are in comments; names are the standard ones):
`FsInit(W)(PluginNr, ProgressProc, LogProc, RequestProc)`, `FsFindFirst(W)(Path, WIN32_FIND_DATA)`,
`FsFindNext(W)`, `FsFindClose`, `FsMkDir`, `FsExecuteFile(MainWin, RemoteName, Verb)`,
`FsRenMovFile(Old, New, Move, OverWrite, RemoteInfo)`, `FsGetFile(Remote, Local, CopyFlags,
RemoteInfo)`, `FsPutFile(Local, Remote, CopyFlags)`, `FsDeleteFile`, `FsRemoveDir`, `FsDisconnect`,
`FsSetAttr`, `FsSetTime`, `FsStatusInfo(RemoteDir, InfoStartEnd, InfoOperation)`,
`FsGetDefRootName`, `FsExtractCustomIcon`, `FsSetDefaultParams`, `FsGetPreviewBitmap`,
`FsLinksToLocalFiles`, `FsGetLocalName`, `FsSetCryptCallback`, `FsGetBackgroundFlags`, and the
content set `FsContentGetSupportedField`, `FsContentGetValue`, ... .

Callbacks: `TProgressProc`, `TLogProc` (types `msgtype_connect=1` ... `operationcomplete=7`),
`TRequestProc` (`RT_UserName=1, RT_Password=2, RT_MsgYesNo=9`, ...), `TCryptProc` (`FS_CRYPT_*`).
Results `FS_FILE_OK=0, EXISTS=1, NOTFOUND=2, READERROR=3, WRITEERROR=4, USERABORT=5,
NOTSUPPORTED=6, EXISTSRESUMEALLOWED=7`; copy flags `OVERWRITE=1, RESUME=2, MOVE=4`;
`FsStatusInfo` operations 1..20 (`LIST` .. `PUT_MULTI_THREAD`). `FsExecuteFile` may return
`FS_EXEC_YOURSELF=-1` (the host should run it) or `FS_EXEC_SYMLINK=-2` (follow). Struct
`tRemoteInfo` = SizeLow, SizeHigh, LastWriteTime, Attr. Paths are rooted: the plugin shows up
as a node under a pseudo-root ("Network"), and everything inside is `\Plugin\dir\file`.

## 3. How Double Commander does it

- **Language and loading**: Free Pascal; `TWcxModule.LoadModule` calls `mbLoadLibrary` (a
  UTF-8-aware wrapper), then resolves names with the equivalent of `GetProcAddress`. That
  wrapper is `LoadLibraryW` on Windows; on Unix it must be `dlopen`, but the page I read did not
  show the call **(unverified)**. DC's docs say it implements the TC plugin API, so on Windows
  TC plugins work, and the Linux builds of the same four extensions are `.so` files with the
  same file name suffixes.
- **WCX**: `ReadHeaderExW` > `ReadHeaderEx` > `ReadHeader`; `OpenArchiveW` first; every optional
  call has a W-first, ANSI-fallback wrapper. `CloseArchive` is required or load fails.
  `PackSetDefaultParams` is called with interface 2.22 and `wcx.ini` in DC's config dir.
- **DC-specific additions**: a DC plugin may export `ExtensionInitialize(StartupInfo)` and
  `ExtensionFinalize`. `TExtensionStartupInfo` carries `StructSize, PluginDir, PluginConfDir`
  (UTF-8), `LanguageID`, `Translation`, `TranslateString`, `VersionAPI`, and function pointers:
  `InputBox`, `MessageBox`, `MsgChoiceBox`, `DialogBoxLFM/LRS/LFMFile/Param`, `SendDlgMsg`,
  `SetProperty/GetProperty`, `CreateComponent`. So DC plugins can show dialogs through the
  host's own toolkit instead of Win32/GTK directly. Also `GetBackgroundFlags` on WCX/WFX.
  `EXT_MAX_PATH` is 16384.
- **WLX parent window (`uwlxmodule.pas`)**: `WlxPrepareContainer` converts DC's container control
  to a native handle. Windows + Qt5: `winId` of the container (`GA_PARENT` for quick view, else
  `GA_ROOT`), plugin window reparented with `GWL_HWNDPARENT`, resize via `MoveWindow`. Windows +
  native LCL: subclass the parent for `WM_COMMAND`, subclass the plugin window to forward unhandled
  `WM_KEYDOWN`. GTK2: parent is a fixed widget; move/size/focus by `gtk_widget_*`. Qt4/5/6: parent
  is `GetContainerWidget`, resize with `QWidget_move/resize`. So on Linux a "WLX" means a
  GTK/Qt widget plugin built for DC (e.g. `wlxwebkit`, `gtksourceview`, `zathura` in the DC
  plugin repo), not a TC plugin. `UnloadModule` is compiled out for Qt/GTK, so the library is
  never freed there.
- **What is cross-platform and what is not**: WCX that only touch files (7-Zip, Zip, UnRAR, cpio,
  rpm, deb) have `.so`/`.dylib` builds; the DC plugin index also lists many Linux/macOS-only
  plugins (MacCloud, MacPreview, mtp, adb, wlxmpv). TC's Win32-only plugins (GUI dialogs, Registry,
  Services, shell integration) do not load on Linux at all.
- DC's bundled set (help page): WCX Base64, CPIO, DEB, RPM, SevenZip, UnRAR, Zip; WDX Exif,
  AudioInfo, DEB, RPM, Lua scripts; WFX FTP, MacCloud; WLX MacPreview, Explorer Preview, RTF,
  WlxMplayer, Windows Media Player; plus a DSX (search) type of its own. The built-in FTP needs
  OpenSSL/GnuTLS and libssh2 (SFTP).

## 4. Midnight Commander: a different design

- **VFS layer**: `src/vfs/` holds `cpio`, `extfs`, `ftpfs`, `local`, `sfs`, `sftpfs`, `shell`
  (fish-style: files over a remote shell), `tar`. A virtual path is an ordinary path with a
  VFS prefix inside it (`/path/file.tar/utar://`, `/#tar:`, `ftp://host/dir`, `sftp://`, the
  `/#sh:` shell link); the VFS layer is in-process C, but selecting one is just part of the
  string. The panel never knows what is behind a path.
- **extfs**: scripts (shell, Perl, Python) in `extfs.d` (system libexec or `~/.local/share/mc/extfs.d/`),
  executable, called as `script command args...`. Verbs from `extfs.d` README: `list archive`,
  `copyout archive stored extractto`, `copyin archive stored source`, `rm archive stored`,
  `mkdir archive dir`, `rmdir archive dir`, plus an undocumented `run`. Exit status 0 = ok,
  nonzero = failed or unsupported. A trailing `+` in the file name marks a VFS that needs no archive.
  `list` prints one `ls -l`-like line per entry: permissions, links, owner, group, size, date,
  path, optional `-> target`; no `.` or `..`. The helper directory ships `a+`, `apt+`, `audio`,
  `deb`, `gitfs+`, `iso9660`, `lslR`, `mailfs`, `patchfs`, `rpm`, `s3+`, `torrent`, `uzip`,
  `urar`, `u7z`, `uwim`, `ucab`, `uar` and about twenty more (listing on GitHub, 38 shown, may be
  longer). So the feature set is "any command-line tool becomes a file system".
- **`mc.ext`**: an ini file (`[mc.ext.ini]` version 4.0), sections matched top to bottom by
  `Directory`, `Regex`, `Shell` (extension or exact name), `Type` (output of `file`), each with
  `Open` (Enter), `View` (F3), `Edit` (F4) commands and macros `%f %p %d %s %t %u %cd %view{...}
  %var{VAR:default}`; `Include=` shares commands. `mcedit` has its own syntax-highlight files and
  `filehighlight.ini` colours files by rule (unfetched, **unverified** beyond naming).
- **The lesson**: the contract is text on stdout plus an exit code. A script cannot corrupt the
  manager's memory, can be written in a day by anyone, is killed with the process, can be
  time-limited, and is language-agnostic. Costs: process-spawn latency per listing (fine for
  archives, poor for a 50 000-entry directory with per-file calls), no in-process progress
  callbacks (poll the temp file size), and no rich UI. A native C ABI (TC) is faster and gives
  richer features (progress, change-volume, password store, icons, thumbnails) but trusts the
  plugin entirely.

## 5. Loading them from Rust

### 5.1 Libraries

| Option | Adds to `Cargo.lock` | Exe cost | Launch path |
|---|---|---|---|
| `windows-sys` `LoadLibraryW`, `GetProcAddress`, `FreeLibrary` (feature `Win32_System_LibraryLoader`) | nothing (already a dependency) | a few hundred bytes of imports **(estimate)** | none until first call |
| `libloading` 0.9.1 (ISC, updated 2026-11-13; rust-version 1.88; on Windows depends on `windows-link` 0.2, on Unix on `cfg-if` 1; `windows-sys` and `libc` only as dev-dependencies, per its `Cargo.toml`) | `libloading` + `windows-link` (plus `cfg-if` on Unix); `windows-link` is tiny (raw `link` declarations, no code) | a few KiB **(estimate)** | none |

Verdict: **use `windows-sys` directly**. We need `LoadLibraryW` and `GetProcAddress` and nothing
more; `libloading`'s value is a safe-ish `Symbol` lifetime and Unix `dlopen`, which a
Windows-first manager does not need. If Linux/macOS plugin loading is ever wanted, then
`libloading` is the one to ask about (price above; two small lock entries on Windows, one on Unix).
Neither is added without the owner saying so.

### 5.2 Structs and strings

`#[repr(C, packed(1))]` mirrors of `tHeaderDataExW`, `tOpenArchiveDataW`, `tRemoteInfo`,
`WIN32_FIND_DATAW` (592 bytes: attributes, three FILETIMEs, size high/low, two reserved DWORDs,
`WCHAR[260]`, `WCHAR[14]`), and the four default-param structs. Read fields with
`read_unaligned`. Paths are UTF-16 NUL-terminated, fixed buffers of 1024 (WCX header) or
`MAX_PATH` / 32000 (WLX/WDX docs). Never trust a returned string: bound the scan, convert with
lossy decoding, and cap lengths. Comments buffers (`CmtBuf`) are plugin-owned; we pass `0`.

### 5.3 Threads and panics

Plugins assume a single caller thread per module (TC calls WFX from worker threads only with
`FS_STATUS_OP_*_MULTI_THREAD` ops; WCX is called serially). Serialise every call per plugin
behind a mutex and never call a plugin on the UI thread. Our callbacks (`extern "system" fn`)
are called from inside the plugin and **must not panic**: wrap each in `catch_unwind` is
meaningless under `panic = "abort"`, so they simply must be written without `unwrap`/index/arith
overflow. A panic in our callback, or a crash (access violation) in the plugin, aborts the
process. In-process there is no recovery except Structured Exception Handling, which Rust does
not offer safely. **This is the argument for a host process.**

### 5.4 Host process

A second exe (`mdfm-host.exe`) loads plugins; the manager talks to it over an anonymous pipe
with a small length-prefixed binary protocol (open, list-next, extract, progress, cancel, field
get). Watchdog: no reply in N seconds, kill and restart. Wrap the host in a **Windows job object**
(`CreateJobObjectW`, `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, memory and process-count limits), set
`SetDllDirectory` / use `LoadLibraryExW` with `LOAD_LIBRARY_SEARCH_*` flags so the DLL's own
folder is the search path and the current directory is not (closes search-path hijack).
Cost ~8 days; the same host serves WCX, WDX, later WFX. A 32-bit helper is the same code built
for `i686-pc-windows-msvc` and shipped as a second exe (estimate 5-6 days, **unverified**:
nobody here has measured the i686 toolchain on this project).

### 5.5 32-bit plugins in a 64-bit process

Impossible directly. Options: (a) ignore them (64-bit plugins exist for the popular ones; section
7), (b) the 32-bit helper above, speaking the same pipe protocol, (c) copy TC's approach and
ship two builds. Recommendation: (a), then (b) only on demand.

### 5.6 WLX windows

Create the plugin's child window with the manager window as parent: `ListLoadW(parent_hwnd,
path, flags)`, then `SetWindowPos`/`MoveWindow` the returned HWND over the viewer rectangle on
each layout change. softbuffer blits into the window's DC: a child window paints over our pixels,
which works because child windows clip their parent area, **Windows only**. Caveats: (1)
`WS_CLIPCHILDREN` on our top-level so we never paint under it; (2) DPI: the plugin may be per-monitor
DPI-unaware; DPI mismatch is its problem, but our rectangle maths must use physical pixels; (3)
focus and keyboard: our winit event loop does not see keys that go to the child, so F3/Esc/Tab
must be handled by a subclass or by polling `WM_KEYDOWN` on the plugin window (DC does exactly this
with `PluginProc`); (4) dark mode flags `lcp_darkmode`/`darkmodenative` exist, but older listers
ignore them; (5) the plugin's message loop runs on our UI thread, so a hung plugin freezes
the whole window: only a separate-process host with `SetParent` across processes avoids that, and
cross-process parenting has its own focus and input-queue-attachment quirks **(unverified)**.
Estimate 8-10 days for in-process, +5 for cross-process. **Skip WLX** unless a specific
one is wanted; built-in viewers (text, hex, image) replace most of what listers do.

### 5.7 WFX as a virtual-folder model

`FsFindFirst/Next/Close` map onto an iterator that fills a panel listing. Paths with a plugin
prefix (`\\Plugin\path`) need a virtual-folder abstraction in the panel (panel root = a
`Provider`, not a directory). Copy, move and delete between panes become `FsGetFile`/`FsPutFile`
transfers with a progress callback, resume flags, overwrite prompts, a `RequestProc` that opens
UI (credentials, yes/no), connection loss, and **timeouts the plugin does not provide**, so a
network WFX hangs the calling thread. That requires a worker per operation, cancellation by
killing the host, credentials storage via `FsSetCryptCallback` (our own password store,
encrypted: more work), and `FsStatusInfo` brackets. 12-15 days, largely because the
panel must already support virtual providers; if the manager is built with them anyway (for
zip-as-folder), the marginal cost is lower.

## 6. Risks

| Risk | Detail | Mitigation |
|---|---|---|
| Crash takes manager down | AV in plugin = process abort (also our `panic = "abort"`) | host process; or allow-list |
| Memory corruption | plugin writes through our pointers | host process; fixed buffers, bounded copies |
| Hangs | network WFX, packers on damaged archives with no timeout | worker + watchdog + kill |
| DLL search-path hijack | `LoadLibraryW` with relative name or current directory | absolute path, `LoadLibraryExW` with `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR`; `SetDllDirectoryW("")` |
| Plugins write to registry / `.ini` | many keep settings in the Windows registry or next to the DLL | give them `DefaultIniName` in our config dir; accept the registry (cannot stop it) or run under restricted token |
| Unsigned, unmaintained, 32-bit abandonware | most of the catalogue | allow-list by SHA-256, show publisher, refuse 32-bit |
| Network access | a WFX can send anything anywhere; conflicts with "no network use except updates and AI" in `CLAUDE.md` | a plugin that is not on a vetted list does not load; the manager's own rule must state that a plugin is third-party code outside our promise |

Mitigation ladder, cheapest first: (1) in-process, allow-list by hash, WCX/WDX only; (2)
out-of-process host per plugin type with pipe, watchdog, restart; (3) job object (kill-on-close,
memory/process limits) and a restricted token / low integrity level for the host, no network
access via AppContainer **(unverified: not prototyped)**; (4) refuse unsigned? Authenticode signing is
rare among TC plugins, so "refuse unsigned" would refuse almost everything; use the hash
allow-list instead.

**Licensing**:
- TC plugin API: the official SDK repos on GitHub (`ghisler/WCX-SDK`, `WFX-SDK`, `WLX-SDK`,
  `WDX-SDK`) hold the headers and docs. The repo pages show **no licence file** and the four
  headers carry **no licence or copyright notice** (only a file/version comment). The plugin page
  on ghisler.com states no terms for plugin writers. I found no text forbidding other programs to
  load plugins; absence of a licence is not permission, so **ask or rely on the SDK's evident
  purpose** (unverified legal reading; this is not legal advice). Double Commander (GPL)
  re-expressed the interface in its own Pascal units and ships its own extensions, and supports
  TC plugins on Windows; Ghisler's page says plugins are mostly third-party software.
- We would write our own declarations of function names and field layouts (facts about an
  interface), not copy header text.
- Each plugin is licensed by its author: freeware closed-source (most), GPL (DC's own, 7-Zip
  family LGPL with unRAR restriction), shareware. **We may not redistribute them**; the
  manager would point at where to download and let the user drop the DLL into a folder. UnRAR
  is a licence special case (the rar licence forbids using the code to recreate the RAR
  compression algorithm) **(from memory, unverified)**.

## 7. How many plugins exist and which matter

### 7.1 Counts

Counted by reading the category totals on `totalcmd.net` on 2026-11-22: page header counts
(`N found`) per type, "x64" mark per entry visible on the first page.

| Type | Plugins listed | First-page entries marked x64 |
|---|---|---|
| WLX Lister | 265 | 20 of 20 |
| WCX Packer | 217 | 15 of 15 shown |
| WFX File system | 185 | not read (category page not fetched; count from the home page) |
| WDX Content | 180 | 20 of 20 |
| **Total of four types** | **847** | |
| Whole site (all categories, add-ons) | 1,226 plugins and add-ons, 354 authors, 117 updated "this year" | |

Method caveats: the first page of each list is sorted by recent date, so "all x64" is a bias:
the newest entries are 64-bit by nature. I did not count the entire catalogue's x64 share
(not computed; **unverified**). wincmd.ru and the DC plugin list were not counted. The DC
wiki/GitHub page lists far fewer (about 20 named plugins; it is a Linux-centred list).

### 7.2 Worth having (what I could verify, and what I could not)

| Type | Plugin | Why | 64-bit / maintained |
|---|---|---|---|
| WCX | tc4shell 7-Zip bridge family (Modern7z 1.9.2, 26 Oct 2026; Iso7z, Forensic7z, eDecoder...) | one engine, 7-Zip formats | tc4shell page shows versions and dates, **not licence or 64-bit** (unverified) |
| WCX | DC's own Zip, SevenZip, UnRAR, CPIO, DEB, RPM | DC ships them as WCX | DC help page |
| WCX | ISO, MSI, CHM-type WCX | image and installer browsing | **names from memory, unverified** |
| WCX | DiskDir, pgdump, Image2PDF, WdxInfoPacker (seen on first page, 2025-26) | niche but maintained | 64-bit marked on totalcmd.net |
| WFX | FTP built into DC; Secure FTP / SFTP plugins by tc4shell / Ghisler's FTP | remote | **unverified here** (WFX category page not fetched) |
| WFX | Registry, Services, Processes, Network, Amazon S3, Google Drive, Dropbox, ADB, Git | classic TC WFX | DC list names adb-wfx, cloud_storage, hdfs_wfx, mtp_plugin; others **unverified** |
| WLX | Imagine, SQLite viewer, Markdown viewers, TCOfficeView 2.4.1 (11 Nov 2026), Synta 1.19.0, csvtab, IFC/draw.io/3mf viewers | the active ones on 2026-11-22 | totalcmd.net first page |
| WDX | MediaInfo 26.10 (23 Oct 2026), EverySearch 1.5, xPDFSearch 1.4.7.1, WDX_GitCommander 2.0.3.0, ImageMetaData 2.4.2.0, TCMediaInfo, MSWord WDX | metadata columns, search | all marked x64, dates as shown |

Maintained recently (verified from dates on the first pages): 117 plugins on the site were
updated this year, and several in each type were updated in November 2026. That is a
living ecosystem, but concentrated in WLX/WDX/WCX by small authors.

### 7.3 What a plain Rust app can do without plugins

| Ground | Crate (latest read 2026-11-22) | Licence | Cost |
|---|---|---|---|
| zip read/write | `zip` 9.8.1 (updated 2026-11-20) | MIT | adds deflate/other codec crates to `Cargo.lock`; features trim it |
| tar | `tar` 0.4.46 | MIT OR Apache-2.0 | small, few deps; add `flate2`/`xz2`/`zstd` for tgz/txz/tzst |
| 7z | `sevenz-rust2` 0.25.0 (updated 2026-11-06) | Apache-2.0 | pulls LZMA, optionally many codecs |
| SFTP | `russh` 0.65.4 (Apache-2.0) with an SFTP layer, or `ssh2` 0.9.7 (MIT OR Apache-2.0, wraps libssh2, C) | | **large** dependency graphs, crypto; `ssh2` needs a C library |

Not added here; every one needs the owner's yes and a lock/exe/launch-path price measured on
a real build (feature-trimmed, lazy). The exe cost of each cannot be stated without building;
my guess for zip+tar+gzip is under about 300 KiB, for russh well over 1 MiB, but **these are
estimates, not measurements**.

## 8. Recommendation and decision table

Phases (each ends with tests per behaviour, shots in both themes, budgets measured):

1. **Archives as folders, built in** (zip, tar, tar.gz, 7z read-only; extract and view in
   place). 6-8 days incl. the virtual-folder abstraction in the panel. Prices the crates first.
2. **External-command protocol** (MC style): a table of `open`/`view`/`edit` commands by
   extension (`mc.ext`-like ini) and an `extfs`-like list/copyout contract for user scripts.
   4-5 days.
3. **Only if asked: WCX read-only + WDX via host process**, 64-bit only, hash allow-list,
   manager UI. ~23 days.
4. **Refuse by default**: WLX (window embedding), WFX (network, threads, auth), 32-bit
   plugins, writing to packers.

| Option | What | Days | Risk | Verdict |
|---|---|---|---|---|
| A | No plugins: built-in zip/tar/7z read-only + extfs-style external commands | ~12 (phases 1+2) | low: crates only, no foreign code | **Do this first.** |
| B | MC-style out-of-process script/exe protocol, user-written providers | ~5 on top of A | low-medium: arbitrary commands run, but out of process | **Good second step**; cheap and safe |
| C | In-process WCX/WDX only via `LoadLibraryW` with an allow-list | ~15 | medium-high: plugin crash = manager abort | Only with the host process (~23), never bare |
| D | Full TC compatibility: WCX, WDX, WFX, WLX, 32-bit helper | ~55-65 | high: large surface, window embedding, networking | **Refuse** unless a named plugin demands it |

## 9. Sources

Read (2026-11-22):

- https://ghisler.com/plugins.htm : the four plugin types and links to the guides (WCX 2.21se,
  FS 2.1se, LS 2.13, Content 2.12) and samples; no licence terms for plugin writers.
- https://ghisler.github.io/WDX-SDK/64_bit_support.htm : `64` suffix naming, Unicode fallback,
  integer sizes stay 32-bit; no word on packing or calling convention.
- https://github.com/ghisler/WCX-SDK : repo of headers/docs, no licence statement. Raw
  `src/wcxhead.h`, `src/listplug.h`, `src/fsplugin.h`, `src/contplug.h` (WCX/WLX/WFX/WDX SDK
  repos) read for field names and the absence of licence/pragma lines (via summarising fetch).
- https://raw.githubusercontent.com/doublecmd/doublecmd/master/sdk/wcxplugin.pas, `wfxplugin.pas`,
  `wlxplugin.pas`, `wdxplugin.pas`, `extension.pas` : function names, constants, `packed`,
  stdcall/cdecl, DC extension API.
- https://raw.githubusercontent.com/doublecmd/doublecmd/master/src/uwcxmodule.pas and
  `src/uwlxmodule.pas` : loading, W-first lookups, `PackSetDefaultParams`, parent-window code per
  widgetset.
- https://doublecmd.github.io/doc/en/plugins.html : DC's bundled plugins, extensions, library needs.
- https://github.com/doublecmd/plugins (via search/fetch summary) : DC plugin repo list.
- https://www.ghisler.ch/board/ (search results for 64-bit plugin threads, e.g. p=300419,
  p=237186, p=309714): 32/64-bit rule, `.wcx64`, `Wcx6432` wrapper. Only search snippets read.
- https://www.totalcmd.net/ (home), /directory/packer.html, /directory/lister.html,
  /directory/content.html : counts and first-page entries. `/directory/fs.html` and
  `/directory/filesystem.html` returned 404; WFX count comes from the home page.
- https://raw.githubusercontent.com/MidnightCommander/mc/master/src/vfs/extfs/helpers/README :
  extfs verbs and `list` line format.
- https://github.com/MidnightCommander/mc/tree/master/src/vfs and `.../extfs/helpers` : VFS
  directories and helper scripts.
- https://raw.githubusercontent.com/MidnightCommander/mc/master/misc/mc.ext.ini.in : `mc.ext`
  format and macros.
- https://tc4shell.com/en/7zip/ and https://tc4shell.com/en/ : tc4shell plugin list/dates.
- crates.io API for `libloading` (0.9.1, ISC), `zip`, `tar`, `sevenz-rust2`, `russh`, `ssh2`,
  `windows-link`; `libloading` `Cargo.toml` on GitHub (dependencies).

Not verified or not fetched:

- Any statement of Ghisler's legal terms for non-TC hosts (none found; no legal reading made).
- `dlopen` use inside `mbLoadLibrary`; DC's handling of `.wcx64` names.
- WLX detect-string grammar; `wincmd.ini` section names; `PackFiles`/`DeleteFiles`/progress
  return-value semantics (from memory of the guide, not fetched).
- `#pragma pack` in the TC C headers (summarising fetch reported none; DC declares `packed`).
- WFX category count on totalcmd.net beyond the home-page number; x64 share of the whole
  catalogue; wincmd.ru counts; licences of named plugins; 32-bit helper effort; exe-size
  estimates; all day estimates (judgement).
