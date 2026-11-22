# Hub: a launcher and updater for the apps (mockups, branch `mock/launcher`, throwaway)

Drawn with mdedit's own painter, fonts, palette and widgets (`bins/mdedit/src/mock.rs` and
`mock/*.rs`, reached only by `--mock hub-<name>`). Regenerate with `shots/mock-hub/make.ps1`
after `cargo build --release -p mdedit`; it runs every shot with `--behind --no-session` and
`MDEDIT_SETTINGS` in `%TEMP%\mock-hub-settings`. Every state is in light and dark, at 1280x800.
`*-compact-*` is 860x600, the size the window should open at: under 1000 px the Released and
Download columns go, and under 150 px of room the pane under the list is hidden.

Nothing is real but the app list. What is installed on the "PC", every state, every hash and the
sizes of old versions in the roll back dialog are made up.

## The apps (read on 2026-11-22 with `gh`, read only)

Of 40 repositories on `Kaan88`, these 7 (plus one old one) have releases that carry installers.

| App | Latest | Date | Platforms in the release | Update manifest |
|---|---|---|---|---|
| mdedit | v1.2.1 | 2026-11-21 | Windows setup, exe, zip; Linux AppImage, deb, rpm, tar.gz; macOS arm64 and x64 zips | `mdedit-update.txt`, `-2.txt`, `-macos.txt` (format 1 and 2, SHA-256, no signature) |
| mdview | v1.0.4 | 2026-11-21 | same as mdedit | `mdview-update.txt`, `-macos.txt` |
| Sprawl | v1.0.5 | 2026-11-20 | Windows only: `Sprawl-Setup-1.0.5.exe`, `sprawl.exe` | `sprawl-update.json` + `.sig` (signed) |
| NewsQueue | v1.0.10 | 2026-11-20 | Windows `Setup.exe`; Linux amd64 tar.gz; Docker files, web zip | none: `.sha256` and `SHA256SUMS.txt` |
| Postern | v0.3.5 | 2026-11-18 | Windows exe and msi (x64, arm64); AppImage, deb, rpm (x64, arm64) | `latest.json`, each AppImage has `.sig`; `.sha256` per file |
| Inbound | v0.9.0 | 2026-11-17 | Windows setup and zip; macOS dmg and zip (arm64, x64); Linux AppImage, deb, rpm, pkg, tar.gz | `manifest.json` per platform, `public.pem`, `SHA256SUMS*` |
| Caustica | v0.13.0 | 2026-11-12 | Windows setup (735 MB), AppImage (725 MB), zip | none: `SHA256SUMS` |
| Markdown Viewer C++ | v0.1.4 | 2026-11-02 | Windows setup and portable zip | none: `SHA256SUMS.txt` (left out of the mock: mdview replaces it) |

Not in the list: `inbound` also publishes Android builds (`android-v0.0.x`, APK, mostly
pre-releases), which a desktop launcher ignores. The other repositories have no releases.
The mock also draws Hub itself (version 0.4.0, made up) as a row, because it updates itself.

## How updating works today, and what the mock assumes

mdedit and mdview use `mdshared_app::updater`: a `key: value` manifest named by the latest release,
SHA-256 of each file, the exe swapped in by `selfupdate` (staged file, old one retired), the running
copy told to restart. That is one app updating itself. A hub is the same machinery run from outside,
for many apps, so the mock assumes: (1) Hub reads each app's own manifest where there is one (three
formats here: text, JSON with a signature, per-platform JSON) and falls back to `SHA256SUMS` where
there is none; (2) Hub never ends a process, it asks the window to close; (3) Hub fetches only
github.com, and the release notes need the release API, which is new network use for the notes
only (named under Settings > Updates).

## The shots

| File | Shows | Why |
|---|---|---|
| `main-*` | The list: 2 up to date or elsewhere, 3 with an update (one open), 2 not installed, one installed outside Hub (NewsQueue, with Adopt). mdedit selected, its notes and the files an update would write below. Compact has no pane. | The one screen the owner lives in. Every state has its own action; the status bar says what Hub contacts. |
| `menu-*` | The row menu: update, open, release notes, roll back, skip this version, show in folder, GitHub, uninstall. | Where the rare actions live, so rows keep one or two buttons. |
| `updating-*` | Update all in progress: mdedit downloading, Sprawl checking its SHA-256, Inbound waiting because it is open. The pane lists the four steps of one update. | Shows the steps (download, check, replace, restart if it was open) and that waiting is a state, not an error. |
| `running-*` | mdedit is open with 2 unsaved tabs: close it and update, update when I close it, not now. | The hard question. Hub asks the window to close, mdedit's own Save question comes first, mdedit's session brings the tabs back. |
| `notes-*` | Release notes of mdedit, 1.2.1 above the installed 1.2.0, from the release's own text. | Reading before updating. Real text from the releases. |
| `failed-*` | mdedit updated; Sprawl refused: hash mismatch. A bar says what was left (1.0.3 untouched, file deleted); the pane has the expected and received hashes and Try again. | A failed update must say what is left. |
| `rollback-*` | Go back to an earlier mdedit: the kept copy of 1.2.0 (no download) or older releases (download and check). Skip 1.2.1. | Needs a policy: Hub keeps one previous version per app. |
| `first-run-*` | Nothing installed yet: pick apps, one install folder (per user), start with Windows, a found install (NewsQueue) offered for adoption. | The only screen that is not the list. |
| `tray-*` | The tray icon with a "3" badge, its tooltip, and the menu: update all, update one, open an app. | Hub lives in the tray; most days that is all the owner sees. |
| `settings-updates-*`, `settings-general-*` | Settings in the category design: look every 6 hours, pre-releases, install by itself for closed apps, versions kept, update Hub; install folder, start with Windows, tray, notifications, show apps as list or icons; and the list of what Hub contacts. | The four settings asked for, and the network promise in plain words. |
| `self-update-*` | Hub 0.4.0 downloaded; a bar and the first row say "restart to finish"; the pane explains that Hub finishes last and puts its old exe back if the swap fails. | The launcher must keep itself current without breaking an install in flight. |
| `linux-*` | The same window on Linux: mdedit from apt (update it with apt), AppImage updates, Sprawl has no Linux build (dimmed, disabled, why). | Platform rows differ in what Install means. |
| `icons-*` | The main state as large icons: seven tiles in one row at 1280 (four to a row at 860, where the pane gives way as in the list). Each tile has the icon with its badges, name, blurb, `installed → latest`, the status line and the row's one action; the rest is in the `⋯` menu. | The view the owner asked for beside the list. Every state of the list still has its words and its button. |
| `icons-small-*` | The same as small icons: icon, name and one word (Update, the version, Not installed, Not by Hub). Badges carry the rest: blue arrow for an update, green dot when the app is open. | All seven fit one row even at 860, so the pane under them stays on the compact size too. It is a launcher first, an updater second. |
| `icons-updating-*` | Update all in progress, as large icons: mdedit's progress bar on its tile, Sprawl checking its SHA-256, Inbound waiting because it is open. | Waiting and progress have to read on a tile as well as in a row. |
| `icons-failed-*` | The failed update as small icons: a red `!` badge on Sprawl and "Not updated", a green check on mdedit, the bar and the pane as in `failed-*`. | A failure must be seen even when the tile has one word. |


`numbers.txt` is the layout report (`--mock hub-report`).

## The view switch

Three buttons left of the platform picker, in every window state: list, large icons, small icons.
Settings > General keeps the same choice ("Show apps as"). The icon views keep the list's order
(newest release first, the strip above the tiles says so), its selection and its pane; only the
columns go. Large tiles are at least 164 px wide and 196 px tall; small ones at least 108 by 104.
Neither view adds a state the list does not have.

## Open questions the drawing raised

- Which view Hub opens in the first time: the list (it says the most) or small icons (a launcher)?

- First install: run each app's own Setup silently, or place the exe? Setups write the uninstall
  entry; the exe alone is what the updater swaps. mdedit's update downloads 35 MB (exe, engine, AI
  engine) against a 12 MB setup.
- One app per update manifest format means Hub needs a small adapter per format; Caustica and
  NewsQueue have only checksums, so Hub can verify a download but cannot learn the version without
  the tag name.
- "No administrator rights" holds only for per-user installs; Postern's msi and Inbound's pkg are not.
