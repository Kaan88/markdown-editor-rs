# The local release scripts

For trying a release and for testing. **These are not how a release is published**: that is the
tag-driven workflow in `docs/RELEASE.md`. The Windows files these scripts make are cross-built or
built on one PC; the published ones are built natively by the workflow.

The first two sections were moved here unchanged from `docs/RELEASE.md` on 2026-11-21, and
"Local builds" from `docs/CI.md`. They were not re-checked then: the older parts describe the
scripts as they were when 0.1.0 to 0.5.0 were made.

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

## Local builds

`tools/release-all.sh` (the local, cross-built Windows files from Wine and `cargo-xwin`) keeps
working and now also builds the Linux packages; it is for testing. Files for users come from
this workflow, with the Windows exe built natively. `packaging/linux/build-packages.sh` builds
the Linux packages locally with Docker (`target/dist-linux/`), the same script the workflow runs.
