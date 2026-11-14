# Releasing mdedit

**The way to publish is the tag-driven workflow** (`.github/workflows/release.yml`): set the version,
push, wait for the `ci` run, push the tag `v<version>`. What it builds, the asset names and the dry
run are in `docs/CI.md`. The local scripts below (`tools\release.ps1` on Windows, `tools/release-all.sh`
and "Releasing from Linux" on Linux) are the dry path: for trying a release and for testing, not
what users download. Their Windows files are cross-built or built on one PC; the published ones are
built natively by the workflow.

## Cutting a release with the local scripts

1. Set the version in `[workspace.package]` of `Cargo.toml`, commit, push.
2. `tools\release.ps1 -Notes 'One short sentence.'` builds `cargo build --release -p mdedit`
   (with `TEMP`/`TMP` inside `target\tmp`) and fills `target\dist\<version>\`. It refuses to run
   if the tree is dirty (changes under `tools\` and `docs\` are ignored) or HEAD is on no remote
   branch. It re-reads the manifest it wrote and compares the keys and the hash with the file.
3. Add `-Publish` (and `-ReleaseNotes '...'` for the GitHub release text) to create the release
   `v<version>` on the current commit with `gh release create`, marked latest. The release carries three assets: the exe, the manifest and the installer.

Check afterwards: fetch the manifest address below, download the asset from its URL, compare the
SHA-256, and run `mdedit --check-update`.

## Assets

- `mdedit-<version>-windows-x86_64.exe`: the executable, the only file the updater installs.
  There is no engine executable.
- `mdedit-setup-<version>.exe` (from 0.3.1): the Inno Setup installer (`packaging\windows\mdedit.iss`,
  built by `tools\release.ps1`), per-user, with file associations. Not code-signed. An extra asset;
  the manifest does not name it.
- `mdedit-<version>-linux-x86_64.AppImage` (from 0.5.0): the Linux build, the only file the updater
  installs on Linux (no engine). Made by `packaging/linux/build-appimage.sh`.
- `mdedit-update.txt`: the manifest (`manifest: 1`, `version`, `notes`, and the three keys
  `windows-x86_64.asset`, `.url`, `.sha256`; from 0.5.0 also the same three under `linux-x86_64`;
  grammar in `mdshared-app`'s `update.rs`).

Manifest address (the latest release's, so it must not be a pre-release or draft):
`https://github.com/Kaan88/markdown-editor-rs/releases/latest/download/mdedit-update.txt`

## v0.1.0 (published from commit 2ba9912)

Verified, against the live release:

- The manifest is served through GitHub's redirect.
- The asset downloaded from the manifest's URL (5917184 bytes) has the manifest's SHA-256.
- `mdedit --check-update` with the built exe (version 0.1.0) fetched and parsed the manifest and
  answered "nothing newer", exit code 0.

Not verified:

- No update has ever been installed from a real release: nothing newer than 0.1.0 exists.
- That an older build is offered this release was not tested; version ordering and offers are
  covered by the shared crates' unit tests only.
- The release script was run only on the Windows PC where v0.1.0 was cut.

## v0.2.0 (published from commit 64ddaa0, 2026-11-13)

Cut so that the updater can be tried from a 0.1.0 install. Verified against the live release:
the manifest is served at the address above and names 0.2.0; the asset downloaded from its URL
(5984768 bytes) has the manifest's SHA-256; `mdedit --check-update` with the 0.2.0 exe answered
"nothing newer", exit code 0.

Not verified: that a 0.1.0 install is offered 0.2.0, and that it downloads and installs it.
That is the test this release is for.

## v0.3.0 (published from commit b94a36e, 2026-11-13)

Cut with the find in files, function list, code folding, macros, split view and the shared
icons. Verified against the live release: the manifest is served at the address above and names
0.3.0 with the SHA-256 `41609145188fab19a7b3d30050164bfc82d1b93ce08f8c4664085b4807d2b236`; the
asset downloaded from the manifest's URL is 5637632 bytes and has that SHA-256, the same as the
local build in `target\dist\0.3.0`; `mdedit --check-update` with the 0.3.0 exe (run from Git
Bash) answered "nothing newer", exit code 0. `cargo test --release --workspace` gave 378 passed,
0 failed, 10 ignored.

Not verified: that an older install is offered 0.3.0 and installs it. Nothing newer than 0.3.0
exists yet, so that test needs an install of an earlier build.

## v0.3.1 (published from commit 5210d8a, 2026-11-13)

Cut with TypeScript, Dart, Zig, SCSS, Less, INI and CMake colouring (shared branch
`feat/more-languages`, commit `fce8c8e`; the shared `main` was not moved) and the installer
`mdedit-setup-0.3.1.exe`, the first release with three assets (4,277,023 bytes for the installer).
Verified against the live release: the manifest at the address above names 0.3.1 (it still served
0.3.0 for about two minutes after publishing); the exe downloaded from its URL (5650432 bytes) has
the manifest's SHA-256 `ce872472fc1e945702c1fb8fc8596a4cef97b84d5e7607b0fbe657bd5f68d75a`;
`mdedit --check-update` with that exe answered "nothing newer", exit 0; the published 0.3.0 exe's
`--check-update` answered "mdedit 0.3.1 is available" with the 0.3.1 exe's URL, exit 0. The
installer downloaded from the release has the SHA-256 of the local build. Silent install and
uninstall of the local build were checked with each task (see `docs/HANDOFF.md`).

Not verified: that an older install downloads and installs 0.3.1 (the offer only was seen); the
installer on a machine other than this PC, clicked through by a person, or over an older install;
that Windows shows mdedit under "Open with" and Default apps. The installer is not code-signed.

## v0.3.2 (published from commit 787178b, 2026-11-13)

Cut with the launch look for updates at every start (shared branch `fix/update-look-every-launch`,
commit `1c3710a`; the shared `main` was not moved) and submenus that open on mouse-over. Assets:
the exe (5,651,968 bytes), the manifest and `mdedit-setup-0.3.2.exe` (4,278,379 bytes). Verified
against the live release: the manifest at the address above names 0.3.2 (it was already
current at the first fetch, a few minutes after publishing); the exe downloaded from its URL is
5651968 bytes and has the manifest's SHA-256
`5872957b62369d773f104bb9f4eb6af1e67c8f98951f06ebc74884f9cd26a026`, the same as the local build;
`mdedit --check-update` with that exe (run from Git Bash) answered "nothing newer", exit 0; the
published 0.3.1 exe's `--check-update` answered "mdedit 0.3.2 is available" with the 0.3.2 exe's
URL, exit 0. `cargo test --release --workspace` gave 382 passed, 0 failed.

Not verified: that an older install downloads and installs 0.3.2 (the offer only was seen); the
0.3.2 installer was built but not run or compared with the uploaded copy. Copies of 0.3.1 and
older still look once a day, so they find 0.3.2 at launch only when their day is up, or by the
manual check.

## v0.4.0 (published from commit c2e43f1, 2026-11-13)

Cut with the minimap (View menu or Ctrl+Shift+M) and the setup's last page without the Default
apps question. Assets: the exe (5,678,592 bytes), the manifest and `mdedit-setup-0.4.0.exe`
(4,285,554 bytes). Verified against the live release: the manifest at the address above names
0.4.0 a minute after publishing; the exe downloaded from its URL has the manifest's SHA-256
`e3d05e91bd3cde888acae3eaa569c7fde60caf64b2be2e2fec8229897c369f67`; the installer downloaded
from the release has the local build's SHA-256
`ccb65eb36b3b178ffc3832d6be4959c676cbc994dd703754c7e908ea872bafa9`; `mdedit --check-update`
with the 0.4.0 exe answered "nothing newer", exit 0; the published 0.3.2 exe's answered "mdedit
0.4.0 is available", exit 0.

The first update seen from start to end: the published 0.3.2 exe, in a folder of its own with
a scratch settings file, started normally. The bar "mdedit 0.4.0 is available" with its notes
and an Update button showed by itself at launch (seen in a screenshot). After a click on
Update the exe in that folder was the 0.4.0 file (the manifest's SHA-256), the old one stood
beside it as `mdedit.exe.old`, and `--check-update` with the new file answered "nothing newer".

Not verified: what the bar shows after the download and that the restart it offers starts
0.4.0 (the window was closed by script instead); an update of a copy the setup installed; the
setup's wizard pages on screen (the 0.4.0 setup was run silently only); Linux.

## Releasing from Linux

Added 2026-11-14; checked against the scripts on the same day for 0.5.0. Both platforms' files can
be built on a Linux PC with Docker, `appimagetool` (with `mksquashfs`), `curl`, `wine`, the Inno
Setup folder from a Windows install, and `python3` with `pefile` for the installer's icon step
below. Publishing needs `gh`. Nothing here replaces `tools\release.ps1` on Windows; the two make
the same Windows file names.

### Commands

```
packaging/linux/build-appimage.sh                  # target/dist-linux/mdedit-<ver>-linux-x86_64.AppImage
tools/release-cross.sh --notes 'One sentence.'     # target/dist-cross/: the Windows exe, the setup, the manifest
tools/write-manifest.sh --version V --notes N --out DIR --windows EXE --linux APPIMAGE
tools/release-all.sh --notes 'One sentence.'       # everything into target/release-files-all/ (default manifest line: "A new version of mdedit.")
tools/release-all.sh --notes '...' --notes-file FILE --publish    # also: gh release create v<ver> ... --latest
tools/wine-compare.sh NATIVE_EXE CROSS_EXE         # tests/shots.sh under Wine with both exes, cmp the PNGs
```

`release-all.sh` reads the version from `[workspace.package]` of `Cargo.toml`, refuses a dirty tree
(changes under `tools/` and `docs/` excepted) and a HEAD no remote branch contains (it runs
`git fetch` first). It runs the AppImage script, then `tools/release-cross.sh`, copies the three
files into `target/release-files-all/` (the intermediate outputs stay in `target/dist-linux/` and
`target/dist-cross/`), and writes the one manifest there with `tools/write-manifest.sh`, which
re-reads it and checks every hash against the file it names. `--notes` is the manifest's line (at
most 1024 bytes, one line), `--notes-file` the GitHub release text.
`--publish --dry-run` prints the `gh` command instead of running it. The release has four assets:
the AppImage, the exe, the setup and the manifest (`windows-x86_64.*` and `linux-x86_64.*` keys in
the one `mdedit-update.txt`; Linux has no engine keys, mdedit has no engine anyway).

### What is built how

- **AppImage.** `packaging/linux/Dockerfile` (Ubuntu 20.04, glibc 2.31, rustup stable) builds
  `cargo build --release --locked -p mdedit` from the repository mounted read-only. The script
  then checks that `objdump -T` needs no glibc symbol newer than 2.31 (0.4.1: 2.30), assembles
  `AppDir` (`usr/bin/mdedit`, `mdedit.desktop`, `mdedit.png`, `.DirIcon`, `AppRun` a symlink to
  `usr/bin/mdedit`) and runs the host's `appimagetool` with a pinned runtime
  (`--runtime-file`; the script fetches that runtime once with `curl` and checks its SHA-256, so
  `appimagetool` never downloads one). Squashfs compression is the default (see "Release 0.5.0"
  in `docs/HANDOFF.md`). `SOURCE_DATE_EPOCH` is the commit time: two runs on
  one commit give the same AppImage (checked).
  Nothing is bundled; the AppImage needs from the host libc, fontconfig, freetype, libX11,
  libX11-xcb and libxcb (a bare `ubuntu:20.04` or `debian:11` container lacks the X11 ones, and
  works once `libx11-6 libx11-xcb1 libxcb1 libfontconfig1 libfreetype6` are installed), plus
  libcurl, wayland and the like loaded at run time if they are there.
- **Windows exe.** `packaging/windows-cross/Dockerfile` (Ubuntu 24.04, rustup stable with the
  `x86_64-pc-windows-msvc` target, `cargo-xwin` 0.23.1, clang, lld, llvm) runs
  `cargo xwin build --release --locked --target x86_64-pc-windows-msvc -p mdedit`. `build.rs` uses
  `rc.exe` when it finds one and otherwise `llvm-rc` (`MD_LLVM_RC`, else PATH), so the icon is
  embedded (the script sets `MD_LLVM_RC=/usr/bin/llvm-rc`).
- **Installer.** `ISCC.exe` from the copied Inno Setup folder runs under Wine on the same
  `packaging\windows\mdedit.iss`, in a scratch tree (`target/cross-installer`) laid out the way
  the script expects its inputs. Then `tools/fix-setup-icons.py` (needs `python3` with `pefile`)
  rewrites the setup's resource tables to drop the stock Inno Setup icons that Wine's
  `UpdateResource` leaves beside ours, so the setup carries only our icon, as one built on Windows
  does. Without `pefile` the script prints a warning and leaves the setup as built (about 4.41 MB
  instead of 4.29 MB).
- **Manifest.** `tools/write-manifest.sh` writes `windows-x86_64.*` and `linux-x86_64.*` keys
  (`asset`, `url`, `sha256`) for the files it is given; the URLs point at tag `v<version>`.
  `release-cross.sh` alone writes a Windows-only manifest into `target/dist-cross/`.

### Licence

`cargo-xwin` downloads the MSVC C runtime and the Windows SDK from Microsoft at its first run.
Using them means accepting Microsoft's licence (https://go.microsoft.com/fwlink/?LinkId=2086102).
cargo-xwin 0.23.1 has no accept flag or variable: it accepts for you when it downloads, so a run
of `tools/release-cross.sh` is that acceptance. Whoever runs it accepts the licence for themselves.

### Caches

`$MD_BUILD_CACHE`, default `~/.cache/md-build`: `mdedit/cargo` (registry and git checkouts),
`mdedit/target-linux` and `mdedit/target-win` (cargo target directories), `xwin` (the SDK and CRT,
about 1.2 GB), `innosetup` (a copy of `/mnt/C/Program Files/Inno Setup 7`; override the source with
`INNO_SETUP_DIR`), `wine-prefix` (the `WINEPREFIX` for ISCC and `wine-compare.sh`, with the Windows
fonts copied into `drive_c/windows/Fonts`) and `runtime-x86_64-20251108` (the AppImage runtime,
SHA-256 pinned in `build-appimage.sh`). The Docker images are `mdedit-linux-build` and
`mdedit-windows-cross`. All of it can be deleted; the next run rebuilds it.

### What the cross-built Windows files have and have not been checked for

Checked (0.4.1, commit `7eb4b9b`'s code, on Linux): the exe imports the same 25 DLLs as the
natively built `target\dist\0.4.1\` exe, has the same icon resources, and is 5,676,032 bytes
against 5,680,128. Under Wine 11 with the same prefix and fonts, `tests/shots.sh` run with the
native exe and with the cross exe gave 74 PNGs each, all byte-identical; `--check-update` of both
answered "nothing newer", exit 0. The cross setup installs and uninstalls silently under Wine and
installs the same exe it was given.

Not checked: any run on Windows. The cross exe is not byte-identical to the native one (a
different linker and resource compiler) and has not been through the Windows-side checks
(`docs/HANDOFF.md`) that the native files had. The cross-built setup was about 4.41 MB against
4.29 MB for the native one because Wine's `UpdateResource` leaves Inno Setup's stock icons in
`Setup.exe` when ISCC swaps in ours; `tools/fix-setup-icons.py` now removes them (commit
`0bdcba8`; its effect on a setup run under Windows was not checked). That setup has never been
clicked through, and every run makes a different file (Inno stamps the build time), unlike the exe
and the AppImage.
Release builds meant for users are still best made, or at least tried, on Windows.

On Linux: with the shared pin of 0.4.1 `--check-update` answered "the look failed: Unsupported",
because the non-Windows transport was a stub. With the libcurl transport of the shared branch
`feat/linux` (pinned since 2026-11-14) it reaches GitHub from a Linux build ("nothing newer" while
the live manifest named 0.4.0; an AppImage updating itself has not run). The release files of 0.5.0
are made this way; Windows is covered only as far as the paragraph above says.
