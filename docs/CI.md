# CI and releases (GitHub Actions)

Two workflows live in `.github/workflows/`. Nothing here needs a secret: the repository, the
forks of winit and softbuffer and the shared crates are all public, and the release job uses the
default `GITHUB_TOKEN`.

## ci.yml: checks

Runs on a push to `main`, on every pull request, and by hand (`workflow_dispatch`). A newer run of
the same branch cancels the older one. Permissions: `contents: read`.

Two jobs, `linux` (ubuntu-latest) and `windows` (windows-latest, the MSVC toolchain, natively):

1. `cargo fmt --all --check`
2. `cargo clippy --release --workspace --all-targets -- -D warnings`
3. `cargo test --release --workspace`
4. The headless screenshot suite as a smoke test: `cargo build --release -p mdedit`,
   `bash tests/shots.sh target/shots`, then `tests/shots-check.sh target/shots`, which fails when
   any shot named in `shots.sh` is missing, empty or not a PNG. There is no pixel comparison:
   the runners' fonts differ from the development machines'. The PNGs are uploaded as the
   artifacts `shots-linux` and `shots-windows` (14 days).

The Rust toolchain is pinned in the workflows (`RUST_TOOLCHAIN`, 1.99.1) with
`dtolnay/rust-toolchain`; there is no `rust-toolchain.toml`, so a local build keeps using
whatever Rust is installed. `Swatinem/rust-cache` caches the registry and `target`. The Linux job
installs the system packages the build links against (the list of `packaging/linux/Dockerfile`).

## release.yml: the release

Triggered by pushing a tag `v*`, and by hand with `workflow_dispatch` (input `publish`, default
`false`). The first job refuses a tag that is not `v` plus the version in `[workspace.package]`
of `Cargo.toml`, before anything is built.

| Job | Runs on | Does |
| --- | --- | --- |
| `windows` | windows-latest | Installs Inno Setup (`choco install innosetup`), runs `tools\release.ps1` without `-Publish` (the native build of `mdedit.exe`, the Inno Setup installer), then `packaging\windows\build-zip.ps1` (the portable zip). |
| `linux` | ubuntu-latest | `packaging/linux/build-packages.sh`: builds the binary in the Ubuntu 20.04 image of `packaging/linux/Dockerfile` (glibc 2.31, the script refuses a binary that needs a newer one), then the AppImage, tar.gz, deb, rpm and the PKGBUILD from that one binary. Smoke test: the tar.gz is unpacked, installed into a temporary prefix, run headless, uninstalled. |
| `publish` | ubuntu-latest | Needs both. Writes `mdedit-update.txt` with both platforms (`tools/write-manifest.sh`: the Windows entry names the native exe the `windows` job built, the Linux entry the AppImage), checks every hash against its file, runs the repo's manifest test (`cargo test --release -p mdedit --test release_manifest`) against the assembled `target/release-files-all/`, and, only for a tag or `publish: true`, runs `gh release create v<version>` at the tagged commit with all assets, marked latest. This is the only job with `contents: write`. |

The release text is `docs/release-notes/<version>.md` when that file exists (its first non-empty
line is also the manifest's one-line note, which the update bar shows), else a default line.

### Cutting a release

1. Set the version in `[workspace.package]` of `Cargo.toml`; optionally write
   `docs/release-notes/<version>.md`. Commit, push, and wait for the `ci` run on `main`.
2. `git tag v<version> && git push origin v<version>`.
3. The `release` run builds everything and creates the release `v<version>`: the assets below.

Dry run: Actions, `release`, Run workflow, leave `publish` unchecked (or
`gh workflow run release.yml --ref <branch>`). Everything is built and checked, and uploaded as
one artifact `release-files-all`; no release is created.

### Assets

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

## Local builds

`tools/release-all.sh` (the local, cross-built Windows files from Wine and `cargo-xwin`) keeps
working and now also builds the Linux packages; it is for testing. Files for users come from
this workflow, with the Windows exe built natively. `packaging/linux/build-packages.sh` builds
the Linux packages locally with Docker (`target/dist-linux/`), the same script the workflow runs.

## Known limits

- x86_64 only: no ARM builds, no macOS.
- The Windows files are not code-signed.
- The deb and the rpm are not signed and no repository serves them.
- The Arch package is `mdedit-bin`: it installs the release tar.gz. The PKGBUILD in the repository
  keeps `sha256sums=('SKIP')`, because the sum of a release that does not exist yet cannot be
  known; the release's own `PKGBUILD` asset has the real one.

## Findings from the first runs

Found by the first CI runs on real Windows (the first time this code ran there since several
features were added) and on a clean Linux runner. None is a defect of the program; all are tests
that assume the machine they were written on.

Windows (windows-latest, the MSVC toolchain):

- `app::tests::a_macro_replays_typing_help_to_the_same_result` and
  `docs::tests::typing_help_pairs_undoes_in_one_step_and_works_at_every_caret` typed Enter into a
  new document and compared with `"\n"`. A new document on Windows ends its lines with CRLF
  (`mdedit-core`'s `eol` takes the platform's, by design). Fixed: both now build the expected text
  from the document's own line ending.
- `replace_files::tests::literal_over_a_tree_in_find_in_files_order` compared the found paths
  with the string `"sub/c.txt"`; Windows spells it `sub\c.txt`. Fixed: it compares `Path`s.
- `%TEMP%` on the runner is the 8.3 short path (`C:\Users\RUNNER~1\...`). A file that exists was
  canonicalized to the long form and a missing one kept the short spelling, so one file had two
  identities; `mdshared_app::paths::canonical_or_absolute` now canonicalizes the nearest existing
  folder of a missing file (fixed in the shared crates), and the job needs no `TEMP` override.

No test is skipped and no step may fail: `ci.yml` has no `--skip`, no `continue-on-error` and no
`TEMP` override. (A few tests are `#[ignore]`d in the source: timing benchmarks and sheet
generators that are run by hand with `-- --ignored`.)

Linux (ubuntu-latest):

- `app::modal::tests::every_help_line_fits_in_every_layout` measures help lines in the UI font.
  With DejaVu Sans, the only sans the runner has by default, a line was 431 px against 422 px of
  room; with Noto Sans (the second entry of the font stack, installed by `ci.yml`) it fits. The
  development machines have Segoe UI first in the stack.
- The screenshots of the runners differ from the development machines' because of fonts, which
  is why the smoke test compares nothing.

What real Windows renders (the `shots-windows` artifact of the first full run; main menu, settings,
palette, compare and the emoji file looked at): the app's own drawing in Segoe UI, all icons
present, the compare view with its difference colouring, and the emoji file with the heart and the
warning sign (and the check mark, sun, telephone, keycap, flag and rocket) in colour, from Segoe UI
Emoji.
