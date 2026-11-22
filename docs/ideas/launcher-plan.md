# Hub: a launcher and updater for the apps, one plan

Written 2026-11-22 for the owner to approve. "You" in this document is the owner. Nothing here is
built. "Hub" is a working name (question 1). The pictures are on the branch `mock/launcher`,
`shots/mock-hub/` (17 states, light and dark, seven also at 860x600), and this plan refers to
them by file name; the list view and the two icon views are all there.

**What I read.** `CLAUDE.md`; `docs/PLAN.md`, `CODE.md`, `RELEASE.md`, `CI.md`, `DECISIONS.md` (the
form and the last entries), `ai-library-plan.md` (as the model of a plan) and
`ecosystem-ideas.md` (the tray-app and "count the binaries" passages), all at `main` `148814e`. The
mock's README and code on `mock/launcher`. In `markdown-shared-rs` at `b4b2182`: `update.rs`,
`updater.rs`, `selfupdate.rs`, `relaunch.rs`, `appimage.rs`, `single.rs` and `sister.rs` (by their
public items and doc comments), and `mdshared-fetch`'s `fetch_file` and `Cargo.toml`.
`packaging/windows/mdedit.iss`, `bins/mdedit/build.rs`, `bins/mdedit/src/updater.rs`, `args.rs`.

**What I ran.** `gh release download` (read only) of `sprawl-update.json` and its `.sig`,
Postern's `latest.json`, Inbound's `manifest.json` and `public.pem`, and the asset list of Inbound's
latest release, into a scratch folder. A count of `Cargo.lock` entries on `main`. For section 3A
(added the same day): `gh repo list Kaan88`, the latest release's assets of every repository with
releases, and, through `gh api` (read only), the installer scripts (`.iss`, `tauri.conf.json`), the
update code's doc comments and the release workflows of Sprawl, NewsQueue, Inbound, Postern,
Caustica, Markdown Viewer C++, thunderbird-jmap, mdview and Barbican; and the Desktop folders that
hold something to install. Nothing in any of those projects was changed.

**What I did not do.** No code of Hub was written; nothing was measured; no installer of any app
was run, silently or otherwise. Whether each installer really stays silent with the arguments
section 3A gives is **read from its script, not tried**. The crate prices in section 10 are
estimates from what each crate is known to pull in, not from a build. That GitHub's
`releases/latest/download/` addresses are not counted against the REST API's limit is what
GitHub documents and what mdedit's updater has relied on; I did not test it at volume.

---

## 1. What it is, and what it refuses to be

**It is** a small program of its own, `hub.exe`, that knows the apps you publish on GitHub, shows
which are installed on this PC and at what version, installs and updates them from their own
releases, keeps one previous version of each to go back to, starts them, and updates itself. It
lives in the tray. It is built like mdedit: Rust, winit and softbuffer, every control drawn by the
shared crates, Windows first.

**It is not part of mdedit.** The drawing settled it (README, "verdict"): a launcher has to be
running when no editor is, it must update mdedit while mdedit is open, and it would put a tray,
a scheduler and a process list into the editor's launch path. Each of those breaks a rule of
`PLAN.md`.

**It refuses to be:**

- **A store.** It installs only the apps in its catalog, which are yours, from your repositories.
  No search, no third-party apps, no "add any GitHub repository" (question 4).
- **A package manager.** No dependencies between apps, no shared runtimes, no system packages.
  On Linux an app installed with apt stays apt's (`linux-*`).
- **A service.** No Windows service, no scheduled task, no elevation of its own. It runs as you,
  per user, and asks Windows for administrator rights only when an installer that needs them is
  run at your click (6.4).
- **A process killer.** It never ends another program. It asks a window to close, or waits for it
  to (6.5).
- **Chatty.** It contacts github.com and the address GitHub sends a download to, nothing else: no
  account, no telemetry, no crash upload, no network at all between checks (section 4).
- **Busy.** At idle it uses no CPU and starts no timer but the one for the next check.

## 2. Tray, window, start at login, idle

**Two lives.** Hub is a tray icon with a menu most of the time (`tray-*`), and a window when you
open it (`main-*`, `icons-*`). Closing the window does not quit Hub; "Quit Hub" in the tray menu
does.

**The window is created on demand and destroyed on close.** The tray needs only a hidden window
to receive the icon's messages (winit can own one; `ecosystem-ideas.md` notes the same). While the
window is closed Hub holds no drawing surface, no fonts beyond what the tray tooltip needs (none:
the tooltip and menu are the system's) and no list. That is what makes a resident process cheap.

**The tray icon and menu are the system's**, `Shell_NotifyIconW` and `TrackPopupMenu`, through
`windows-sys`, which the shared crates already use; the drawn menu of the mock (`tray-*`) is the
model for the items, not for the look. The icon gets a badge with the count of updates (two
icons, drawn at build time: plain and with a dot; a number in the badge is drawn into an
`HICON` at run time from the shared painter, only when the count changes).

**Start at login**: a value `Hub` under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`,
`"<path>\hub.exe" --tray`, written and removed by the setting (`settings-general-*`). On by
default only if you tick it on the first-run screen (`first-run-*`). With `--tray` Hub starts with
no window, waits two minutes, then runs its first check, so it adds nothing to your sign-in.

**Idle cost: zero.** The event loop waits with `ControlFlow::WaitUntil(next check)`. Waiting for
an app to close is `WaitForSingleObject` on its process handle in a thread that sleeps in the
kernel. No polling anywhere.

**Budgets** (measured as mdedit's are, `CODE.md` "Measuring"):

| | Budget | Why |
|---|---|---|
| Window's first frame, opened from the tray | under 100 ms | The family's figure |
| Private memory, window open | under 30 MiB | The family's figure |
| Private memory, tray only | under 8 MiB | It is resident all day; the shell of a winit app with no surface is about that (to be measured in phase 0, and the figure set from it) |
| CPU at idle | zero | |
| Network at idle | none | |
| Sign-in | nothing for two minutes | |

## 3. The apps: catalog, detection, versions, manifests

### 3.1 The catalog

A list compiled into Hub, one entry per app: its name, blurb, icon, repository, how to find it
on this PC, and the address of its manifest. The eight on 2026-11-22 are the README's table
(mdedit, mdview, Sprawl, NewsQueue, Postern, Inbound, Caustica, and Hub itself; Markdown Viewer
C++ left out). A new app joins by a new entry and a Hub release, which Hub then installs on
itself. A catalog fetched from the network would let a release of Hub's repository add apps
without a build; it is not worth a second source of truth for eight entries.

The icons are each app's own `.ico`, copied into Hub's resources at build time (eight small
files). Not drawn tiles: the mock's coloured tiles stand in for them.

### 3.2 Is it installed, and which version

In this order, the first that answers wins:

1. **Hub's own record**, `%LOCALAPPDATA%\Hub\installed.ini`: for each app Hub installed or
   adopted, the folder, the executable, the version, the kind (setup, files, zip) and the date.
   Authoritative for what Hub did.
2. **The uninstall entry** under `HKCU` and `HKLM` `...\CurrentVersion\Uninstall` (and its
   `WOW6432Node` twin), found by the catalog's key (mdedit's Inno `AppId` is
   `{C58A9AB1-8337-4056-9953-04E92C05F414}_is1`) or by `DisplayName`: `InstallLocation`,
   `DisplayVersion`.
3. **The places the installers use**: `%LOCALAPPDATA%\Programs\<name>\<name>.exe`, as
   `sister::find` already looks.
4. **The executable's version resource** (`GetFileVersionInfoW`).

An app found by 2 to 4 and not by 1 is "Installed elsewhere" with **Adopt** (`main-*`, NewsQueue):
adopting records it in 1 without touching it.

Two faults found while reading, both to fix in the apps, not worked around in Hub:

- **mdedit's exe has no version resource** (`build.rs` writes only `1 ICON`), and neither,
  probably, has mdview's. Step 4 then finds nothing. Fix: one `VERSIONINFO` block in each
  `build.rs`, half an hour each.
- **mdedit's updater leaves the uninstall entry's version behind.** `APP.uninstall_key` is `None`
  in `bins/mdedit/src/updater.rs`, so `record_version` does nothing, and after a self-update
  Windows' Installed apps still shows the version the setup installed. Fix: set the key to the
  `_is1` entry above. Not seen on a real install: inferred from the code.

Hub never runs an app to ask its version (`--version` would start a program you did not ask to
start).

### 3.3 The manifests today

Four formats, two apps with none (the README's table; the three non-mdedit ones read today):

| App | What its release carries | Signed |
|---|---|---|
| mdedit, mdview | `key: value` text, formats 1 and 2 (`mdshared_app::update`), a `-macos` file beside | No; SHA-256 over TLS |
| Sprawl | `sprawl-update.json`: `version`, `files[]` (name, sha256, size: the exe and a Windows App Runtime DLL), `setup` (name, sha256, size); `sprawl-update.json.sig`, 64 bytes in hex | Yes, 64-byte signature: ed25519 by its size (not verified) |
| Postern | `latest.json` in the Tauri updater's shape: `version`, `pub_date`, `platforms` with only `linux-x86_64` and `linux-aarch64` (AppImages, minisign signatures); **no Windows entry**; `.sha256` per file | Linux only, minisign (ed25519) |
| Inbound | `manifest.json` per platform: `version`, `channel`, `installerUrl` (the Windows **zip**, 101 MB, not the 68.7 MB setup), `size`, `sha256`, `signature` (base64, 384 bytes), `notes[]` with tags; `public.pem`, an RSA-3072 key | Yes, RSA (padding not checked) |
| NewsQueue | `SHA256SUMS.txt` and a `.sha256` per file | No manifest |
| Caustica | `SHA256SUMS` | No manifest |

Three signature schemes, one of which (Inbound's) carries its public key in the same release it
signs, which proves nothing against someone who can publish a release.

### 3.4 One manifest for Hub

**Proposal: every app's release carries one more file, `hub.txt`, in one format, and Hub reads
only that.** No adapters for the four formats. The file is written by the release workflow from
what it already knows, so adopting it is one script and one step per repository (phase A).

It is the family's `key: value` grammar (`update.rs`: size limits, `#` comments, unknown keys
refused, https only, lower-case hex), with a new header so no old parser ever reads it:

```text
hub: 1
app: sprawl
version: 1.0.5
date: 2026-11-20
notes: Faster scan of network drives; the size column sorts again.
notes.file: release-notes.md
windows-x86_64.kind: files
windows-x86_64.exe: sprawl.exe
windows-x86_64.file: sprawl.exe
windows-x86_64.file.url: https://github.com/Kaan88/sprawl/releases/download/v1.0.5/sprawl.exe
windows-x86_64.file.sha256: f432cef021fca78b4553c81a924f2c3d4d74b942512027d090f4b92c4b166d65
windows-x86_64.file.size: 12269568
windows-x86_64.file2: Microsoft.WindowsAppRuntime.Bootstrap.dll
windows-x86_64.file2.url: https://github.com/Kaan88/sprawl/releases/download/v1.0.5/Microsoft.WindowsAppRuntime.Bootstrap.dll
windows-x86_64.file2.sha256: 280275d5c4d509e85e394ff99bfb271fb0d6426718b1fa0913dce5888f847d14
windows-x86_64.file2.size: 399712
windows-x86_64.setup: Sprawl-Setup-1.0.5.exe
windows-x86_64.setup.url: https://github.com/Kaan88/sprawl/releases/download/v1.0.5/Sprawl-Setup-1.0.5.exe
windows-x86_64.setup.sha256: a268e03bb2d0150bd8b8b5725b738b312218ebe99d7429099d62b9c488f60196
windows-x86_64.setup.size: 5544567
windows-x86_64.setup.silent: /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CURRENTUSER
# Illustrative: Sprawl itself stays installed for all users (3A).
windows-x86_64.setup.scope: user
windows-x86_64.uninstall: Sprawl_is1
linux-x86_64.kind: appimage
...
```

The keys:

| Key | Meaning |
|---|---|
| `hub: 1` | The header. A parser refuses any other number |
| `app`, `version`, `date` | The catalog name (must match the entry), the version, the release day |
| `notes` | The one line the list and the notification show |
| `notes.file` | An asset of the same release with the full notes in Markdown, fetched only when you open "What's new" (`notes-*`). This keeps Hub off the REST API entirely (section 4) |
| `P.kind` | How this platform installs: `setup` (run the installer), `files` (place executables), `zip` (unpack a folder), `appimage`, `app` (a macOS bundle in a zip) |
| `P.exe` | The executable Hub starts, relative to the install folder |
| `P.file`, `P.file2` … `P.file9` | The files of a `files` install, each with `.url`, `.sha256`, `.size` |
| `P.zip` | The archive of a `zip` install, with `.url`, `.sha256`, `.size` |
| `P.setup` | The installer, with `.url`, `.sha256`, `.size`, `.silent` (its arguments for a silent run) and `.scope` (`user` or `machine`) |
| `P.uninstall` | The name of its uninstall key, for detection and for `DisplayVersion` |
| `P.min` | Optional: the oldest installed version this update can be applied over with `files`/`zip`; older ones get the setup |
| `P.wait` | Optional: further executables of the app that must all have ended before an install (Inbound's host, bridge and command line) |
| `P.updates` | Optional: `self` when the app's own updater keeps it current, and Hub only installs, shows and starts it (none of the seven; kept for a later app) |

Platforms are `windows-x86_64`, `windows-aarch64` (Postern ships one), `linux-x86_64`,
`linux-aarch64`, `macos-aarch64` and `macos-x86_64`; Hub reads the one it runs on.

Rules: a platform may list both a `setup` and a `files` or `zip`; Hub uses the setup for a first
install (it writes the uninstall entry, the Start menu item and the file associations) and the
smaller files or zip for an update. Every URL must start with
`https://github.com/Kaan88/<the app's repository>/releases/download/`; anything else refuses the
whole manifest. Sizes are checked before and after a download.

**Where it is read**: `https://github.com/Kaan88/<repo>/releases/latest/download/hub.txt`, the
address mdedit's manifests already use, which always names the latest non-draft, non-pre-release
release. A beta channel (`settings-updates-*`) would read the newest release including
pre-releases, which needs the API; it is left out of the first cut (question 6).

**The parser lives in the shared crate** (`mdshared_app::hubmanifest`, beside `update.rs`, using
its `Version` and SHA-256), so each app's own test can parse the `hub.txt` its release workflow
writes, as `release_manifest.rs` does for mdedit today.

**Signatures: not in the first cut.** Every app's integrity today is SHA-256 in a manifest fetched
over TLS from github.com, so whoever can publish a release can publish a manifest. A signature
helps only if its key is not where the release is made; in every repository it is a GitHub
Actions secret, so it would not. If you want it anyway, the format takes a detached
`hub.txt.sig` (ed25519) with the public key compiled into Hub's catalog, never fetched; that
costs a crate (section 10) and half a day (question 5).

## 3A. Each project's setup today, and what it needs

Read on 2026-11-22, read only: the latest release of every repository of `Kaan88` that has
releases (11 of 38; 15 of the 38 are forks with none), their installer scripts and update code,
and the Desktop folders that hold a program but no setup. "Own updater" is the app's built-in one.
No Windows file of any project is code-signed (Authenticode).

**What every catalog app needs, whatever its row says**: its release workflow writes `hub.txt`
(3.4) with one shared script, and its test parses it; its own updater stands down when Hub
manages it (question 3, the `managed-by: hub` file of section 15); its exe has a version
resource. The table says what comes on top.

| Project | Ships today | Installer, scope, folder | Manifest and own updater | What has to change | Days |
|---|---|---|---|---|---|
| **mdedit** (`markdown-editor-rs`) | Setup, exe, zip; AppImage, deb, rpm, tar.gz, PKGBUILD; macOS zips | Inno, per user (`lowest`), `%LOCALAPPDATA%\Programs\mdedit`, AppId `{C58A9AB1-…}` | Formats 1 and 2 and `-macos`; shared updater, SHA-256 over TLS, `releases/latest/download/` | `hub.txt` (`files`: the exe and two engines; `setup` for a first install with `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`); a `VERSIONINFO` in `build.rs` (it has only the icon); `uninstall_key` set so `DisplayVersion` follows updates (3.2); answer the sister `close` request | 1 |
| **mdview** (`markdown-viewer-rs`) | The same set | Inno, per user, `%LOCALAPPDATA%\Programs\mdview`, AppId `{B7E2F4A9-…}` | `mdview-update.txt`, `-macos`; shared updater | As mdedit, with the same script: `hub.txt`, a `VERSIONINFO` (none today), its uninstall key, the `close` request | 0.75 |
| **Sprawl** | `Sprawl-Setup.exe`, `sprawl.exe`, the Windows App SDK bootstrap DLL; Windows only | Inno, **`admin`, Program Files** (`{autopf}\Sprawl`), AppId `Sprawl` (key `Sprawl_is1`) | `sprawl-update.json` + `.sig`; own updater asks **api.github.com** daily, swaps files in a per-user copy, runs the setup elevated for a Program Files copy | **Stays for all users** (answered 2026-11-22, question 7). `hub.txt` lists the setup with `scope: machine` and `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`: Hub runs it elevated at your click, with Windows' prompt, every update. No `files` entry, since Hub, not elevated, cannot write to Program Files. The installer is unchanged | 0.75 |
| **NewsQueue** | `Setup.exe`, a portable zip, Linux amd64 tar.gz, Docker files, a web zip, a source zip | Inno, per user (`lowest` with `{autopf}`, so `%LOCALAPPDATA%\Programs\NewsQueue`), AppId a GUID; `UninstallDisplayName` carries the version | `SHA256SUMS.txt`, `.sha256`; own updater asks **api.github.com**, runs the setup silently with `NQ_WAIT_PID` and `NQ_RELAUNCH` | `hub.txt` (`setup`, silent); detected by its AppId, not by name, since the name changes with each version; on Linux a tar.gz is not an install Hub does: an AppImage, or "Open the release page" there | 1 |
| **Postern** | Tauri: NSIS setup and MSI, x64 and arm64; AppImage, deb, rpm, x64 and arm64 | NSIS `currentUser`; MSI per machine (WiX) | `latest.json`, Tauri's, **Linux AppImages only**; on Windows **no updater at all** | `hub.txt` with `windows-x86_64` and `windows-aarch64` entries: the NSIS setup, silent with `/S`; the MSI left out of Hub. The uninstall key NSIS writes is to be found (not read here) | 1 |
| **Inbound** | Setup 68.7 MB, zip 101 MB; macOS dmg and zip; AppImage, deb, rpm, Arch, tar.gz; a Firefox add-on | Inno, per user, `%LOCALAPPDATA%\Programs\Inbound`, AppId `{6F4B7C2E-…}`; C# and WinUI 3, several processes (host, window, browser bridge, command line) and a native-messaging registration | `manifest.json` per platform, **RSA-signed** with a repository secret, `public.pem` beside it; own updater unpacks the zip and swaps `app\` with `apply-update.ps1`, waiting for every process; releases are created as **drafts** and published by hand | `hub.txt` with `setup` only, silent, and `wait` naming every exe: the zip needs `apply-update.ps1` and the registration the setup does, so Hub must not unpack it. A draft is invisible to `releases/latest/` until published | 1.5 |
| **Caustica** | Setup 770 MB, zip 965 MB, AppImage 760 MB | Inno, per user with an **override dialog** (`PrivilegesRequiredOverridesAllowed=dialog`), `{autopf}\Caustica`, AppId `{6400E487-…}` | `SHA256SUMS`; own updater asks **api.github.com**, runs the setup on Windows, replaces the AppImage on Linux | `hub.txt` (`setup` with `/CURRENTUSER`, so the dialog never shows; the AppImage). Its size needs Hub's resume and free-space check (section 5) | 0.75 |
| Markdown Viewer C++ | Setup, portable zip | Inno, per user; a second, test `AppId` line left in the script | `SHA256SUMS.txt`; none | Nothing: replaced by mdview, out of the catalog (question 2) | 0 |
| thunderbird-jmap | A win64 zip and a signed complete MAR with `update.xml`; no setup | None: unzip and run | Mozilla's own updater, pointed at the GitHub release | Nothing: **out of the catalog** (answered 2026-11-22, question 2); its own updater carries on | 0 |
| kweb mail for Stalwart | An Android APK | | | Nothing: not a desktop app | 0 |
| Lyrics for Audirvana | One `Lyrics.exe`, 2.0.0; repository archived | | | Nothing: archived | 0 |

**Projects with no release yet that will ship something to install:**

| Project | What there is | What it needs before Hub can carry it | Days, when it ships |
|---|---|---|---|
| Barbican (C#, WinUI 3 and Uno, mail) | `packaging/` with WiX, MSI and Debian files; no release | A release workflow; a per-user installer (an MSI is per machine unless written otherwise); `hub.txt` | 2 |
| `utilities` (private: bootnext, display toggle, speaker control; Rust tray apps) | Built locally; older AutoHotkey and Python builds of Bootnext in `Desktop\Apps` | The repository made public when it ships (answered 2026-11-22, question 11), with releases; one per-user Inno script for the three, or portable exes as `files`; `hub.txt` | 1.5 |
| Claymore Control 1.5 (Desktop) | `Install.cmd` and `Uninstall.cmd`, per user; no repository found under `Kaan88` | A repository and releases; an Inno per-user setup in place of the two scripts, or a zip as `zip`; `hub.txt` | 1 |
| Gamma22Tray (Desktop) | A PyInstaller one-folder build; no repository found | A repository and releases; a zip as `zip`, or a setup; `hub.txt` | 1 |
| TwinPane (`Desktop\commander`) | A C# prototype zip in `artifacts`; no repository on GitHub | Later: it is a prototype | 0 now |
| `purge` (private, Rust uninstaller) | Nothing shipped | Later: an uninstaller of other programs needs administrator rights by nature, so Hub would run its setup elevated at your click | 0 now |
| Spatial Commander (Desktop) | A Godot export | **Never in Hub**: its `CREDITS.md` says the build is for private use and must not be shared or published | 0 |

Not counted: forks (winit, softbuffer, stalwart, lightcraft, pdfcraft, Klipper and the others),
web apps and servers (Class Compass, kweb-bridge, MCP-Tunnels), and scripts with no install.

**What this survey changes in the plan.**

- **Three apps' own updaters use the REST API** (Sprawl, NewsQueue and Caustica ask
  `api.github.com/repos/…/releases/latest`), so every copy of them lives under the 60-an-hour
  limit Hub avoids. Standing them down under Hub (question 3) removes that as well.
- **Most apps update through their setup, not by placed files.** Inbound must (several processes,
  a registration), NewsQueue, Postern and Caustica publish no per-file list, and Sprawl lives in
  Program Files, so the `setup` kind of 6.4 is the common case.
- **Sprawl stays installed for all users** (your answer): every Sprawl update from Hub shows
  Windows' prompt, "install by itself" never applies to it, and it updates by its setup, not by
  placed files. Only mdedit and mdview are `files` updates.
- **Hub writes `DisplayVersion` itself** after every update it does (6.1), so an app's entry in
  Installed apps is right whatever its own updater does (mdedit's, today, leaves it behind).

## 4. Checking

- **When**: two minutes after Hub starts, then every 6 hours by default (Every hour, 6 hours,
  day, week, never, `settings-updates-*`), at the next due time measured from the last check
  that succeeded, so a PC that sleeps does not check on waking unless one is due. "Check now"
  in the bar and the tray at any time.
- **What**: one GET of `hub.txt` per catalog app (seven requests every six hours), through
  `mdshared_fetch::fetch_file` with a 64 KiB cap and a 10-second deadline, at most two at a
  time. Nothing else: no release list, no API call. Full notes are one more GET of the
  `notes.file` asset, when you open them, kept in the cache by version.
- **Rate limits**: the `releases/latest/download/` and `releases/download/` addresses are served
  as web downloads, not REST API calls, so the 60-requests-an-hour limit of unauthenticated API
  use does not apply. 28 requests a day is nothing next to that in any case. If GitHub answers
  429 or 403, Hub waits out the `Retry-After` it gives, or an hour.
- **Offline**: a failed check is not an error shown in a bar: the status bar says "Last check
  failed: no connection, trying again at 14:30", and Hub tries again after 30 minutes, then
  doubles up to the interval. No retry storm, no watching the network (that would mean the
  Network List Manager's COM interface, for a nicety).
- **The promise**, in the status bar and Settings > Network (`settings-updates-*`): github.com
  for the manifests and notes, the address GitHub redirects a download to for the files, nothing
  else. The User-Agent is `Hub/<version>`, no more.

## 5. Downloading and verifying

- **Streaming to disk.** `fetch_file` keeps the body in memory and caps it (the updater's cap is
  128 MiB). Caustica's setup is 735 MB. So the shared fetcher gains `fetch_to_file(url, path,
  max_bytes, deadline, progress)`: the same transport (WinHTTP on Windows, the system's libcurl
  loaded at run time elsewhere), writing to `<name>.part` as it arrives, hashing as it goes.
- **Resume.** A `.part` that survives (Hub quit, the PC slept, the line dropped) is resumed with a
  `Range: bytes=<size>-` request, accepted only if the answer is `206` with a matching
  `Content-Range`; otherwise it starts again. The hash is then computed over the whole file
  (re-reading the part already on disk). Three tries per file, then "Not updated" with the
  reason.
- **Verifying.** SHA-256 against the manifest (the shared implementation in `update.rs`) and the
  size, before the file is renamed from `.part` or run. A mismatch deletes the file and says what
  was left (`failed-*`, `icons-failed-*`). Nothing downloaded is ever run or copied before it is
  verified.
- **Where.** `%LOCALAPPDATA%\Hub\cache\<app>-<version>\`, removed after a successful install
  except what roll back keeps (6.6).
- **Progress** is reported no more than ten times a second to the window, and not at all while
  the window is closed (the tray tooltip reads it when hovered).
- **Pause and cancel** (`updating-*`) close the connection and keep the `.part`.

## 6. Installing

### 6.1 The order of an update

Download, verify, wait for the app to be closed (6.5), place the new version (6.2 to 6.4), check
that the new executable exists and its version resource says the new version, record it in
`installed.ini` and in the uninstall entry's `DisplayVersion`, start it again if it was open
(`updating-*` pane, the four steps). Every step that fails puts the old version back and says so.

### 6.2 `files`: executables placed

What mdedit and mdview do to themselves, run from outside. `selfupdate::swap_in` already does
the careful part for a set of targets: stage each new file beside its target, retire the old
ones, rename the new ones in, put the old ones back if any rename fails. Hub calls it with the
files of the manifest. The retired files are moved into the roll back folder instead of deleted.

### 6.3 `zip`: a folder unpacked

Unpacked into `<install folder>.new`, then the old folder renamed to `<install folder>.old` and the
new one into place (two renames on one volume), then `.old` moved to the roll back folder. An app
that keeps data in its install folder would lose it: the manifest's `zip` kind is allowed only for
apps that do not. None of the seven updates by zip (3A). Paths in the archive are checked: no
absolute paths, no `..`, no names Windows refuses, a total size cap from the manifest.

### 6.4 `setup`: the app's installer run

Run with its `.silent` arguments, wait for it, read its exit code (Inno: 0 is success), then
re-detect. Per-user installers (`scope: user`) need no administrator rights; they are what Hub
prefers and what the first-run screen promises. A `machine` installer (Postern's MSI, any setup
that asks for elevation) is run only when you press Install or Update on it, through
`ShellExecuteExW` with the `runas` verb, so Windows shows its own consent prompt; Hub itself never
runs elevated, and "Install updates by itself" never applies to such an app. A silent setup that
shows a window anyway (an installer that ignores the arguments) is the installer's fault and is
named in its row as "needs you".

### 6.5 Apps that are running

- **Seeing it**: the process list (`EnumProcesses`, `QueryFullProcessImageNameW`), matched by the
  full path of the executable Hub knows, so a portable copy elsewhere is not counted. Read when the
  window opens and before an install, not continuously.
- **mdedit and mdview**: asked to close through the sister protocol (`mdshared_app::sister`) with
  a new request `close`: the window runs its own close, which asks about unsaved tabs, and its
  session brings the tabs back after the update (`running-*`). An older build that does not know
  the request answers `Older` and is treated like any other app.
- **Every other app**: `WM_CLOSE` posted to its visible top-level windows, which is what clicking
  the window's X does. If it is still running after 30 seconds, the row says "Waiting: it is
  open" (`updating-*`, `icons-updating-*`) and Hub waits on the process handle.
- **"Update when I close it"** waits on the process handle in a sleeping thread and installs when
  it ends.
- **Never** `TerminateProcess`, never a forced restart of Explorer or of anything else.

### 6.6 Rolling back

Hub keeps the previous version of each app it updated (`rollback-*`), one by default (0 to 3 in
Settings): the retired files of a `files` update, the old folder of a `zip`, the previous
installer of a `setup` app (re-run to go back; Inno installers install an older version over a
newer one without complaint). Older versions are offered from the releases and downloaded under
the same rules. "Skip this version" is recorded in `installed.ini`. Settings, sessions and data
of the apps are never touched by Hub, so a roll back puts back the program, not your files; an
app whose new version changed its settings format is the app's problem to have handled.

### 6.7 Uninstalling

From the row menu (`menu-*`): an app installed by a setup is uninstalled by its own
`UninstallString` (quietly when it is Inno's, `/VERYSILENT`); one Hub placed is deleted folder and
all, after asking. Its settings stay unless the app's uninstaller removes them.

## 7. Hub updates itself

The shared updater as mdedit uses it, unchanged: Hub's own release carries `hub-update.txt` in
format 1 (and a `hub.txt` like every app, so it shows in its own list, `self-update-*`). Hub checks
itself first and swaps itself last, when no install is running, then restarts with
`relaunch::spawn` and the restart flag. If the swap fails the old exe is put back by `swap_in`.
The tray icon is re-added by the new process.

## 8. Launching, notifications, settings, first run

- **Launching**: Open in a row or tile, a double click on a tile, or the tray menu's list of
  installed apps: `CreateProcessW` of the recorded executable, detached, no arguments, with its
  folder as the working directory. Nothing is waited on.
- **Notifications**: one per check that found something, never one per app: "3 updates: mdedit,
  Sprawl, Inbound". Shown with `Shell_NotifyIconW`'s balloon (`NIF_INFO`), which Windows 10 and
  11 turn into a toast attributed to Hub and route through Focus Assist; no WinRT, no app id
  registration. Clicking it opens the window. "Tell me when updates are found" turns it off;
  failed and finished updates are not notified, they are in the window and the badge.
- **Settings**: on the shared paged form (`mdshared_ui::settings::pages`, D-086), the pages
  drawn in `settings-general-*` and `settings-updates-*`: General (install folder, start with
  Windows, tray icon, notifications, theme, show apps as list or icons), Updates (how often,
  pre-releases, install by itself for closed apps, versions kept, update Hub), Apps (one row per
  app: hide it, never update it), Network (the list of what Hub contacts), About. The file is
  `%LOCALAPPDATA%\Hub\settings.ini` through `mdshared_app::settings`, with a `HUB_SETTINGS`
  variable for tests, as mdedit has `MDEDIT_SETTINGS`.
- **The views**: list, large icons, small icons (section 13), one switch in the bar, kept in
  settings.
- **First run** (`first-run-*`): the apps found on the PC offered for adoption, the others with a
  tick to install, the install folder, start with Windows. Nothing is installed until you press
  the button.
- **Hub's own installer**: an Inno Setup per user, like mdedit's, plus a portable exe.

## 9. Linux and macOS, after

- **Linux.** Hub as an AppImage, the tray through the StatusNotifierItem D-Bus protocol (which
  means a D-Bus client: `zbus` and its async stack, or `libdbus` loaded at run time the way the
  fetcher loads libcurl; question 8), start at login by a `~/.config/autostart/hub.desktop`
  file. Apps: AppImages placed in `~/Applications` (or `~/.local/bin`), the `appimage` kind,
  updated by replacing the file; a deb, rpm or Arch install is shown as the package manager's
  (`linux-*`, mdedit "Installed with apt") with no Update button. An app with no Linux build is
  shown greyed with the reason (Sprawl). Running apps: `/proc/<pid>/exe`; asking to close:
  the sister protocol for the two apps, nothing for the others (no portable `WM_CLOSE`), so they
  wait. Notifications: the `org.freedesktop.Notifications` D-Bus call, the same client as the
  tray. **4 days.**
- **macOS.** Hub as a menu-bar item (`NSStatusItem`, through the `objc2` bindings the Mac build
  already has via winit), a login item through `SMAppService` (macOS 13+) or a LaunchAgent plist.
  Apps: `.app` bundles in `~/Applications`, the `app` kind (a zip unpacked and the bundle
  swapped, as mdedit's Mac updater does), ad-hoc signed, so the first open is still right-click,
  Open. Notifications: `UNUserNotificationCenter` needs a signed bundle with an identity, which
  these are not; until then, the badge only. **3 days.**

## 10. Crates, priced (rule 7; none added by this document)

Hub is a new workspace, so its `Cargo.lock` is whatever the shared crates already pull in (the
same set mdedit's 455-entry lock has, less mdedit's own: syntect, the viewer's renderer, the
highlighter) plus what is listed here. "Launch path" means work done before the first frame or at
tray start.

| Need | Without a new crate | With one | Price of the crate | Recommendation |
|---|---|---|---|---|
| Tray icon, menu, balloon | `windows-sys` (`Win32_UI_Shell`, `Win32_UI_WindowsAndMessaging`, features of a crate already there) | `tray-icon` | About 10 lock entries on Windows (`muda`, `crossbeam-channel`, its own windowing), and assumptions about owning the event loop that fight winit's | **Without** |
| Registry (Run key, uninstall entries) | `windows-sys` `Win32_System_Registry` | `winreg` (already in mdedit's lock, through another crate) | 1 entry, ~30 KiB | **Without**; ten calls |
| Process list, `WM_CLOSE` | `windows-sys` `Win32_System_ProcessStatus`, `Threading` (already enabled in the shared crates) | `sysinfo` | ~10 entries, a large crate, and a full process scan where a path match is needed | **Without** |
| Unpacking a zip | A ~300-line reader of the central directory over `miniz_oxide`'s streaming inflate (stored and deflate only, which is what the release workflows make) | `zip` with no default features and `deflate` | 4 to 6 entries (`zip`, `crc32fast`, `flate2`, `miniz_oxide` and helpers); ~150 KiB; nothing at launch | **Ask**: `miniz_oxide` alone is 1 entry (it is in the shared set only through `png`; to be checked) and the reader is half a day; `zip` is the safer parser of a hostile archive |
| JSON | Not needed: `hub.txt` replaces the JSON manifests | `serde_json` | | **Not needed** |
| Signature (only if question 5 is yes) | No | `ed25519-dalek` without default features | ~8 entries (`curve25519-dalek`, `ed25519`, `sha2`, `signature`, `subtle`, `zeroize`, …), ~120 KiB, nothing at launch | **Only if asked** |
| Linux tray and notifications | `libdbus` loaded at run time, ~400 lines of our own | `zbus` | 30+ entries with its async runtime pieces, ~1 MiB | **Ask** when Linux starts |
| Hub's installer | Inno Setup, as mdedit's | | | Not a crate |

What Hub adds to the **shared** crates' dependencies: nothing for Windows. `fetch_to_file` uses
the fetcher's existing transports.

## 11. What goes where

| Piece | Where | Why |
|---|---|---|
| The `hub.txt` parser and its writer for tests (`hubmanifest`) | `mdshared_app`, from the start | Every app's release test parses its own `hub.txt`, as mdedit's does its update manifests today |
| `fetch_to_file`: streaming, `.part`, `Range` resume, hashing as it goes | `mdshared-fetch`, a new function beside `fetch_file` | mdedit's and mdview's own updaters can use it next (their downloads are 35 MB in memory today); `fetch_file` stays as it is |
| The sister request `close` | `mdshared_app::sister`, opt-in (an app that does not handle it answers as today) | Both apps must answer it; the protocol is shared. mdview's pin moves when it wants it |
| A `VERSIONINFO` resource in `build.rs` | Each app | Two lines each, mdedit and mdview |
| `uninstall_key` set in mdedit's `APP` | mdedit | A one-line fault fix (3.2) |
| Detection, `installed.ini`, the installers, the process waits, roll back | Hub | Policy no other app has |
| Tray, Run key, balloon | Hub first; `mdshared_ui::platform::tray` only when a second app wants a tray | The rule says shared first, but nothing else in the family lives in the tray; the "gamma tray" idea in `ecosystem-ideas.md` would be the second user, and the code is written to be lifted |
| The grid of tiles (the icon views) | Hub first, written against `mdshared_ui::rows`' arithmetic so it can join it | The list of equal rows is shared already; a grid is its sibling. No second user yet |
| The window, list, panes, bar, settings form, painter, fonts, menus | Already shared | Used as they are |
| `selfupdate::swap_in`, `relaunch`, `single` (one Hub per user), the updater for Hub itself | Already shared | Used as they are |

Every shared addition is a new function or an opt-in request: no existing caller draws or does
anything differently (rule 8), so neither app's pin has to move until it wants the new piece.

## 12. Testing

- **Unit tests** in the shared crate for `hubmanifest`: every key, every refusal (unknown key,
  http, a URL outside the app's repository, short hash, oversize, duplicate, the wrong header),
  in the style of `update.rs`'s.
- **A fake catalog and fake manifests.** `HUB_CATALOG` names a catalog file and
  `HUB_FETCH_ROOT` a folder that stands in for github.com (`https://github.com/Kaan88/x/...`
  becomes `<root>/Kaan88/x/...`), honoured only in builds with the `test-hooks` feature, never in
  a release. With it: install, update, a hash mismatch, a truncated `.part` resumed, a zip with
  `..` in a path, a setup that exits with 1, an app that is running, a roll back. The installers
  in these tests are small programs built by the test (a fake `setup.exe` that writes a folder and
  an uninstall key under a test-only registry path, `HKCU\Software\HubTest`).
- **A local server** for the transport only: `fetch_to_file` against a server on `127.0.0.1`
  that drops the connection half way and honours `Range`. The fetcher refuses http, so the test
  server serves TLS with a certificate made for the test and trusted only by the test's
  transport configuration (a test-only switch in `mdshared-fetch`, behind the same feature).
- **Shots**: the 17 mocked states become `tests/shots.sh` entries of the real window, both
  themes, 1280x800 and 860x600, plus the tray menu and the balloon drawn by `--shot` (the system
  draws the real ones; the shot shows the text). The owner looks at each (rule 6).
- **By hand on Windows, before phase 4 is called done**: each of the seven apps installed, updated
  and rolled back on a clean user account, with the app open and closed; mdedit with unsaved tabs.
- **Never on your settings**: every test and every shot with `--no-session`-like isolation:
  `HUB_SETTINGS` in a scratch folder, `--behind`.

## 13. The icon views

Asked for on 2026-11-22 and drawn on `mock/launcher` (`icons-*`, `icons-small-*`,
`icons-updating-*`, `icons-failed-*`, each in both themes, the first three also at 860x600):

- A switch of three buttons left of the platform picker: list, large icons, small icons. Also in
  Settings > General.
- **Large icons**: a tile of at least 164 by 196 px with the icon, badges, name, blurb,
  `installed → latest`, the status line and the row's one action; seven to a row at 1280, four at
  860.
- **Small icons**: 108 by 104 px, the icon, name and one word; seven to a row even at 860, so the
  pane under them stays on the small window. Badges: a blue arrow for an update, a red `!` for a
  failure, a green check for done, a green dot for running.
- Both keep the list's order, selection, pane and row menu; no state exists in one view and not
  the others. A grid adds arrow-key movement in two directions and nothing else.

Cost: 1.5 days inside phase 3 (the tiles, the switch, keyboard, shots).

## 14. The phases

A day is a day of this project's kind: code, a test per behaviour that fails with its fix
reverted, fmt and clippy clean, shots looked at in both themes, budgets measured before and
after.

| # | What ships | Days |
|---|---|---|
| 0 | The repository (`Kaan88/hub-rs`, question 1), workspace on the shared crates pinned by rev, `ci` copied from mdedit's, a window with the list from the compiled catalog (no network), single instance, the tray icon and menu, the window made and destroyed on demand, Run key. Tray memory measured and the budget of section 2 set | 3 |
| 1 | Detection (3.2), `installed.ini`, Adopt, the `hubmanifest` parser in the shared crate with its tests, the fake catalog and fetch root | 2.5 |
| 2 | Checking (section 4): the schedule, back off, the status bar's promise, notes on demand, the badge and the balloon | 2 |
| 3 | The list's states, the panes, the row menu, and the icon views (13) | 3 |
| 4 | `fetch_to_file` in the shared fetcher with resume and its local-server test; the cache; `files` installs through `swap_in`; running apps (6.5) including the sister `close` request in the shared crate and in mdedit; roll back of `files` | 4 |
| 5 | `setup` and `zip` installs, elevation by click, uninstall, roll back of both | 3.5 |
| 6 | Hub updates itself; its installer and portable exe; its release workflow; first run; Settings on the paged form | 2.5 |
| A0 | The shared `hub.txt` writer script (PowerShell and shell, in `markdown-shared-rs/tools`), which every release workflow calls with its files | 0.5 |
| A1 | mdedit and mdview: `hub.txt`, version resources, uninstall keys, the `close` request, their own updaters standing down (3A) | 1.75 |
| A2 | Sprawl (its setup, for all users), NewsQueue, Postern (Windows entries, x64 and arm64), Inbound (setup only, `wait`), Caustica (`/CURRENTUSER`), each in its own repository and tried silently there (3A) | 5 |
| 7 | The by-hand pass on Windows (12) and what it finds | 1.5 |
| | **Windows, total** | **29.25** |
| L | Linux (9) | 4 |
| M | macOS (9) | 3 |
| | **Total** | **36.25** |

Not in the total, done as each one ships: Barbican (2), the three utilities (1.5), Claymore
Control (1) and Gamma22Tray (1), the second table of 3A.

A1 joins the first slice: it needs no `hub.txt` yet, but detection reads the version resources and
the uninstall keys. A2 runs in each app's repository beside phases 4 and 5, and must be done for an
app before Hub offers that app's updates; until then the app's row shows its version and Open.

**The first useful slice: phases 0, 1, 2, 4 and A1 for mdedit and mdview only, about 13 days.** Those
two already publish manifests the shared updater reads, and their updates are `files` installs,
so the slice needs no `hub.txt` yet (it reads their format-2 manifests through `update.rs` as a
stopgap, and drops that when phase A lands), no setup runs, no zip and no elevation. What you get:
Hub in the tray, a badge when mdedit or mdview has an update, Update with the app open (asked to
close, tabs back after), roll back, Open. Everything after it is adding kinds of install and
apps.

**Order, and why.** Detection and the manifest before checking, because a check needs to know what
is installed; `files` before `setup`, because it is the shared code already proven on mdedit; Hub
updating itself after there is something worth updating; Linux and macOS last, as the family
does.

## 15. Risks

- **Other apps' installers.** Five of the seven are not built on the shared crates. Their
  installers are known from their scripts (3A), but none was run silently here. One that ignores
  its silent arguments or asks for elevation changes what Hub can do for it. Phase A2 tries each;
  the worst case is that app being "Open the release page" only.
- **Postern has no Windows update entry at all**: its `latest.json` lists Linux AppImages only.
  Its Windows update in Hub is a silent setup run each time.
- **Caustica's 735 MB.** Resume, disk space (check free space before the download: twice the size
  for a zip) and the time a hash of 735 MB takes (about two seconds, on the worker).
- **Antivirus and SmartScreen.** Nothing is code-signed. A downloaded, unsigned setup run silently
  by another unsigned program is exactly what heuristics flag. Mark-of-the-Web: files Hub writes
  carry none (it is not a browser), so SmartScreen's prompt does not appear for them; a scanner
  may still quarantine. Code signing (question 9) is the real fix, for every app.
- **A resident process is a new kind of program for this family.** Its memory over a week, a
  sleep and resume, a change of user session, an Explorer restart (the tray icon must be
  re-added on `TaskbarCreated`) are new tests.
- **Two updaters for one app.** mdedit and mdview still update themselves. If both Hub and the app
  see 1.2.2, the app's own bar and Hub's badge both offer it, and the two must not run at once;
  nothing I read stops that today. Proposal: when Hub manages an app it writes
  `managed-by: hub` into the app's install folder, and the app's updater, seeing it, leaves the
  update to Hub (question 3).
- **Scope.** This is a sixth binary to release and maintain. `ecosystem-ideas.md`'s rule was "do
  not start a fifth before the window frame is shared". The frame (chrome, list, panes, settings)
  is shared now; this plan uses it as it is, which is the reason the estimate is weeks, not
  months.

## 16. What I would not build

- Adapters for the four manifest formats (3.4).
- A catalog fetched from the network, or adding any repository by URL (3.1).
- A Windows service or a scheduled task.
- Delta updates or patches: whole files, hashed.
- Ending a process, ever.
- Installing apps for all users, or anything that needs Hub itself to run elevated.
- A "beta channel" in the first cut (question 6).
- A drawn tray menu: the system's is the one people expect, and it costs nothing at idle.

## 17. Questions only you can answer

1. **The name**, and the repository (`Kaan88/hub-rs` is the working one).
2. **Which apps** are in the catalog. *Answered 2026-11-22: the seven; thunderbird-jmap out;
   Markdown Viewer C++ out.*
3. **Who updates mdedit and mdview** when Hub is installed: Hub only (the apps' own bars stand
   down, 15), or both, each offering it?
4. **Only your apps**, or may you add another repository by URL?
5. **Signatures**: SHA-256 over TLS as today (recommended), or `hub.txt.sig` with an ed25519 key
   compiled into Hub, a crate and half a day?
6. **Pre-releases**: needed in the first cut? It means the releases API (60 requests an hour,
   unauthenticated) and a list instead of one file.
7. **Sprawl's scope**. *Answered 2026-11-22: it stays for all users, with Windows' prompt on
   every update.* Inbound and Caustica were settled by reading them: both update through their
   setup (3A).
8. **Linux tray**: `zbus` (a large dependency) or `libdbus` loaded at run time (our code)?
9. **Code signing**: is a certificate for the Windows apps on the table? It changes how Hub and
   every app are received by SmartScreen and antivirus.
10. **The view Hub opens in** the first time: list or small icons?
11. **Private and unreleased projects** (the utilities, Claymore Control, Gamma22Tray). *Answered
    2026-11-22: each is made public when it ships, so Hub reads its releases with no token.*

## 18. Decisions taken in this document

Taken for you, each with what would overturn it, ready to be copied into a decisions log.

| # | Decision | Reason | What would overturn it |
|---|---|---|---|
| 1 | Hub is its own program and repository, not part of mdedit | Launch path, running when the editor is not, updating the editor | Nothing |
| 2 | One manifest, `hub.txt` in the family's `key: value` grammar, in every app's release; no adapters for today's four formats | One parser to trust; each app adds one workflow step | An app that cannot change its release workflow |
| 3 | No REST API: manifests and full notes are release assets at `releases/latest/download/` | No rate limit, one address per app | Pre-releases (question 6) |
| 4 | SHA-256 over TLS, no signatures in the first cut | A key kept where releases are made adds nothing | You (question 5) |
| 5 | Downloads stream to `.part` and resume with `Range`; nothing is run or copied before its hash matches | 735 MB files; a broken file must never be installed | Nothing |
| 6 | A first install runs the app's setup; updates place files or a folder where the manifest allows, else run the setup | The setup writes the uninstall entry and associations; updates should not need the installer's UI | An app whose setup does work the files alone would miss |
| 7 | Hub never ends a process; it asks (sister `close`, `WM_CLOSE`) or waits on the process handle | Unsaved work | Nothing |
| 8 | Hub runs per user, never elevated; a machine-wide installer runs elevated only at your click | One trust boundary, Windows' own prompt | Nothing |
| 9 | One previous version kept per app for roll back | The drawn policy; cheap | Disk space with Caustica (set 0 for it) |
| 10 | Tray, Run key, notifications and process list through `windows-sys`, no tray or process crates | No new dependencies on Windows | A Windows API that turns out unworkable from winit's thread |
| 11 | The tray and the grid start in Hub, written to be lifted into the shared crates when a second app wants them | Shared first, but there is no second user | A second app with a tray or a grid |
| 12 | The window exists only while it is open | Resident memory | Nothing |
| 13 | Spatial Commander is never in the catalog | Its credits forbid sharing it | Nothing |
| 14 | The catalog is the seven: mdedit, mdview, Sprawl, NewsQueue, Postern, Inbound, Caustica (and Hub). thunderbird-jmap and Markdown Viewer C++ are out. `updates: self` stays in the format for a later app | The owner, 2026-11-22 | The owner |
| 15 | Sprawl stays installed for all users; Hub updates it by its setup, elevated at your click, never by itself | The owner, 2026-11-22 | The owner |
| 16 | Inbound, NewsQueue, Postern, Caustica and Sprawl update through their setups; only mdedit and mdview by placed files | Read from their scripts and updaters (3A), and decision 15 | An app that publishes a per-file list later |
| 17 | A project reaches Hub only through public releases; the private ones are made public when they ship | The owner, 2026-11-22; Hub sends no token | The owner |
