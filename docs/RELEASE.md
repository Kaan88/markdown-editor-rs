# Releasing mdedit

Stable. It changes when the release process changes. The record of each release is
`docs/release-log.md`; the workflows are described in `docs/CI.md`.

**A release is published by pushing a tag.** The workflow `release` then builds every file on
GitHub's runners, Windows natively on a Windows runner, and creates the release. The local
scripts (`tools\release.ps1`, `tools/release-all.sh`) are for trying a release and for testing:
`docs/release-local.md`. What users download never comes from them.

## Before

1. Everything for the release is on `main`, and nothing under "Known to be wrong" in
   `docs/STATE.md` should hold it.
2. The budgets were measured for what changed (`docs/CODE.md`, "Measuring").
3. `docs/release-notes/<version>.md` is written. **Its first non-empty line is the one-line note
   the update bar shows to every user**, so write that line for them.
4. The version has not been used before (`docs/DECISIONS.md`, D-016). 1.1.0 failed in CI and was
   never published; the next attempt was 1.1.1.
5. Only the app that changed is released (D-014).

## Cutting it

1. Set the version in `[workspace.package]` of `Cargo.toml`; optionally write
   `docs/release-notes/<version>.md`. Commit, push, and wait for the `ci` run on `main`.
2. `git tag v<version> && git push origin v<version>`.
3. The `release` run builds everything and creates the release `v<version>`: the assets below.

Dry run: Actions, `release`, Run workflow, leave `publish` unchecked (or
`gh workflow run release.yml --ref <branch>`). Everything is built and checked, and uploaded as
one artifact `release-files-all`; no release is created.

macOS files for a release made before them: Actions, `release`, Run workflow, `attach_macos_to`
`v<version>` (or `gh workflow run release.yml -f attach_macos_to=v<version>`), from a commit whose
version is that release's.

The first job of the run refuses a tag that is not `v` plus the version in `Cargo.toml`, before
anything is built. The release is marked latest, which is what the update manifests' address
relies on: it must not be a draft or a pre-release.

## After

Check against the live release. Every record in `docs/release-log.md` has these:

- the release is not a draft or pre-release and names 17 assets (16 before `release-notes-<version>.md`,
  15 before `hub.txt`, 1.3.0);
- `releases/latest/download/` `mdedit-update.txt`, `mdedit-update-2.txt` and
  `mdedit-update-macos.txt` all name the new version, and `hub.txt` (Hub's manifest, below) does, and `hub.txt`'s `notes.file` names an asset that exists;
- every file the manifests name (the Windows exe, `mdedit-ai.exe`, `mdview-engine.exe`, the
  AppImage, both Mac zips) downloads with the SHA-256 the manifest gives;
- the published exe's `--check-update` answers "nothing newer", and the previous version's
  answers "mdedit <version> is available".

Then:

1. Add a section at the top of `docs/release-log.md`: the run ids, what is in it, what was
   verified and what was not.
2. Rewrite `docs/STATE.md`: "Released" names the new version, and "On `main`, not released" is
   what came after it.

Not verified for any release so far: the installer run on Windows, an installed copy updating
itself, the Mac and Linux files on a desktop.

## The files

Seventeen, since the release after 1.3.0 (`release-notes-<version>.md`); sixteen in 1.3.0 (`hub.txt`);
fifteen before.

| File | For |
| --- | --- |
| `mdedit-<version>-windows-x86_64.exe` | The executable; the only file the updater installs. |
| `mdedit-setup-<version>.exe` | The Inno Setup installer (per user, file associations). |
| `mdedit-<version>-windows-x86_64.zip` | Portable: `mdedit.exe`, `LICENSE`, `README.md` in one folder. |
| `mdedit-<version>-linux-x86_64.AppImage` | Any distribution with glibc 2.31 or newer. Updates itself. |
| `mdedit-<version>-linux-x86_64.tar.gz` | `install.sh [PREFIX]` (default `~/.local`) and `uninstall.sh`. |
| `mdedit_<version>_amd64.deb` | Debian 11+, Ubuntu 20.04+ and derivatives. |
| `mdedit-<version>-1.x86_64.rpm` | Fedora, openSUSE, RHEL and derivatives. |
| `PKGBUILD` | Arch: `makepkg -si` in a folder holding it; installs the tar.gz. |
| `mdedit-update.txt` | The update manifest (Windows and Linux entries). |
| `mdedit-<version>-macos-aarch64.zip`, `mdedit-<version>-macos-x86_64.zip` | `mdedit.app` for Apple Silicon and Intel Macs, macOS 12 or later; ad-hoc signed only, so opened with right-click, Open the first time. |
| `mdedit-update-macos.txt` | The manifest the macOS builds read: they download the zip and replace their bundle with it. |
| `mdedit-ai.exe` | The AI engine for Windows, an asset of its own; a copy is also inside the zip, the installer, the AppImage (`usr/bin`), the Linux packages (`lib/mdedit/`) and the Mac bundle. |
| `mdview-engine.exe` | The preview helper for Windows, built from the same toolkit pin as the embedded renderer and installed beside `mdedit.exe`. |
| `mdedit-update-2.txt` | The format-2 manifest (below). |
| `hub.txt` | The manifest of Hub, the owner's launcher and updater (`tools/write-hub-txt.ps1`, a copy of the script in the Hub repository, whose first line names the Hub commit it came from). Windows only: the exe, both engines and the setup, with their hashes. Written by the `publish` job after the other manifests; a release whose files it refuses fails there. Older builds never read it. Its `notes.file` line names the next file. |
| `release-notes-<version>.md` | The full release text, the same file as the GitHub release body (`docs/release-notes/<version>.md` with the macOS paragraph), at most 64 KiB, for the release-notes page of Hub; named by `hub.txt`'s `notes.file`. Copied by the `publish` job from `notes/release-notes.md`. |

## The manifests

Three, because copies of different ages read different ones:

- `mdedit-update.txt` is format 1 (the exe and the engine). It is read by every copy up to
  1.0.6, which refuses unknown keys, and by 1.1 and later on Linux. Keys: `manifest: 1`,
  `version`, `notes`, and `asset`, `url`, `sha256` under `windows-x86_64` and `linux-x86_64`.
- `mdedit-update-2.txt` is format 2: the same, and `windows-x86_64.mdedit-ai.url` and `.sha256`.
  Read by 1.1 and later on Windows.
- `mdedit-update-macos.txt` is what the macOS builds read: they download the zip and replace
  their bundle with it.

`tools/write-manifest.sh` writes the first two; `packaging/macos/write-manifest.sh` the third.
`bins/mdedit/tests/release_manifest.rs` reads format 1 with the shared updater as it was at
1.0.6 and format 2 with the current one, against the assembled files, before anything is
published. The grammar is in `mdshared-app`'s `update.rs`.

The address every copy looks at is the latest release's:
`https://github.com/Kaan88/markdown-editor-rs/releases/latest/download/mdedit-update.txt`

## Who should use which

- Windows: the setup for a normal install; the exe or the zip for a portable copy.
- Linux with a package manager: the deb, the rpm or the PKGBUILD, so the system keeps it updated.
- Linux, any distribution, no root: the AppImage (self-updating) or the tar.gz into `~/.local`.

## Which copies update themselves

On Windows every copy that is not a `cargo` build folder: the installed one, the portable exe, the
zip's (the folder must be writable). On Linux only the AppImage (`$APPIMAGE` names a file in a
writable folder, `mdshared_app::updater::installed_files`). The deb, rpm, tar.gz and Arch
packages do not look for updates at launch, and the Update button answers with the line
"Only the AppImage can update itself." (the shared crate's text off Windows; also what
Settings > Updates and the About dialog's check lead to). The window writes its own desktop
entry only when `$APPIMAGE` is set, so the packages, which install a desktop file with
`Exec=mdedit %F` and `StartupWMClass=mdedit`, are not affected.

## Known limits

- Windows and Linux are x86_64 only. macOS is built for Apple Silicon and for Intel.
- The Windows files are not code-signed. The Mac bundles are ad-hoc signed only, so they are
  opened with right-click, Open the first time.
- The deb and the rpm are not signed and no repository serves them.
- The Arch package is `mdedit-bin`: it installs the release tar.gz. The PKGBUILD in the repository
  keeps `sha256sums=('SKIP')`, because the sum of a release that does not exist yet cannot be
  known; the release's own `PKGBUILD` asset has the real one.
