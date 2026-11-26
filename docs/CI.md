# CI (GitHub Actions)

Stable. It changes when a workflow changes. How a release is cut is in `docs/RELEASE.md`; this
file says what runs.

Four workflows live in `.github/workflows/`. Nothing here needs a secret: the repository, the
forks of winit and softbuffer and the shared crates are all public, and the release job uses the
default `GITHUB_TOKEN`.

| Workflow | Runs on | What it is for |
| --- | --- | --- |
| `ci` | A push to `main`, every pull request, by hand | fmt, clippy, the tests and the screenshot suite on Linux, Windows and two Macs |
| `mac-look` | A push to `main`, by hand | Builds on a Mac, renders the shots and uploads them, to see how the app looks there |
| `preview-test` | By hand only | On Windows: builds mdedit with and without the built-in preview beside a build of the viewer, runs the preview tests, uploads shots; a second job builds `main` beside the branch to compare them |
| `release` | A pushed tag `v*`, by hand | Builds every package and publishes the release |

The Rust toolchain is pinned in the workflows (`RUST_TOOLCHAIN`, 1.99.1) with
`dtolnay/rust-toolchain`; there is no `rust-toolchain.toml`, so a local build keeps using
whatever Rust is installed. `Swatinem/rust-cache` caches the registry and `target`.

## ci.yml: the checks

Runs on a push to `main` (and to `f/macos`), on every pull request, and by hand
(`workflow_dispatch`). A newer run of the same branch cancels the older one. Permissions:
`contents: read`.

Three jobs: `linux` (ubuntu-latest), `windows` (windows-latest, the MSVC toolchain, natively)
and `macos` (two runners: Apple Silicon and Intel). Each does the same:

1. `cargo fmt --all --check`
2. `cargo clippy --release --workspace --all-targets -- -D warnings`
3. `cargo test --release --workspace`
4. The headless screenshot suite as a smoke test: build `mdedit` and stage the toolkit's pinned
   `mdview-engine` beside it (see `docs/CODE.md`), run `bash tests/shots.sh target/shots`,
   then `tests/shots-check.sh target/shots`, which fails when
   any shot named in `shots.sh` is missing, empty or not a PNG. There is no pixel comparison:
   the runners' fonts differ from the development machines'. The PNGs are uploaded as the
   artifacts `shots-linux`, `shots-windows` and `shots-macos-<arch>`.

The Mac jobs also zip `mdedit.app` and upload it. The Linux job installs the system packages the
build links against (the list of `packaging/linux/Dockerfile`).

No test is skipped and no step may fail: `ci.yml` has no `--skip`, no `continue-on-error` and no
`TEMP` override. (A few tests are `#[ignore]`d in the source: timing benchmarks and sheet
generators that are run by hand with `-- --ignored`.)

What the first runs on real Windows and on a clean Linux runner found, all of it tests that
assumed the machine they were written on, is in `docs/archive/ci-first-runs-2026-11-14.md`. The
lesson that stays: a test compares paths as paths, builds its expected text from the document's
own line ending, and measures text in the font in use.

## Waiting for a run

Run everything locally first. Push, start the run once, and then **wait without polling**: the
account's GitHub API quota was once used up by agents polling runs. Do not use `gh run watch`.
Wait about twelve minutes, then look once:

```bash
gh run list --workflow ci.yml --branch <branch> --limit 1
gh run view <id> --json status,conclusion
gh run view <id> --log-failed          # when it failed
gh run download <id> -n shots-windows -D <folder>
```

If it has not finished, wait five more minutes between single looks. If `gh` answers "API rate
limit exceeded", wait fifteen minutes and carry on with local work.

## release.yml: the release

Triggered by pushing a tag `v*`, and by hand with `workflow_dispatch` (input `publish`, default
`false`). The first job refuses a tag that is not `v` plus the version in `[workspace.package]`
of `Cargo.toml`, before anything is built.

| Job | Runs on | Does |
| --- | --- | --- |
| `windows` | windows-latest | Installs Inno Setup (`choco install innosetup`), runs `tools\release.ps1` without `-Publish` (the native build of `mdedit.exe` and of `mdview-engine.exe` from the pinned toolkit revision, the Inno Setup installer), then `packaging\windows\build-zip.ps1` (the portable zip). |
| `linux` | ubuntu-latest | `packaging/linux/build-packages.sh`: builds the binary in the Ubuntu 20.04 image of `packaging/linux/Dockerfile` (glibc 2.31, the script refuses a binary that needs a newer one), then the AppImage, tar.gz, deb, rpm and the PKGBUILD from that one binary. Smoke test: the tar.gz is unpacked, installed into a temporary prefix, run headless, uninstalled. |
| `macos` | macos-15, macos-15-intel | Builds `mdview-engine` from the pinned toolkit revision and `mdedit.app` with `packaging/macos/make-bundle.sh` (the engine in `Contents/MacOS`) (ad-hoc signature, checked with `codesign --verify`) and zips it as `mdedit-<version>-macos-aarch64.zip` and `-x86_64.zip`. |
| `attach` | ubuntu-latest | Only for a manual run with `attach_macos_to: v<version>` (then `windows`, `linux` and `publish` are skipped): adds the two macOS zips and `mdedit-update-macos.txt` to that existing release, and the macOS paragraph to its text. |
| `publish` | ubuntu-latest | Needs `windows`, `linux` and `macos`. Also writes `mdedit-update-macos.txt` (`packaging/macos/write-manifest.sh`) and adds the macOS paragraph (`packaging/macos/release-notes.md`) to the release text. Writes `mdedit-update.txt` with both platforms (`tools/write-manifest.sh`: the Windows entry names the native exe the `windows` job built and its engine, the Linux entry the AppImage), checks every hash against its file, copies the release text to `release-notes-<version>.md` (at most 64 KiB), writes `hub.txt` for Hub with `-NotesFile` naming it (`tools/write-hub-txt.ps1` under `pwsh`, which refuses what Hub's parser would refuse), runs the repo's manifest test (`cargo test --release -p mdedit --test release_manifest`) against the assembled `target/release-files-all/`, and, only for a tag or `publish: true`, runs `gh release create v<version>` at the tagged commit with all assets, marked latest. This is the only job with `contents: write`. |

The release text is `docs/release-notes/<version>.md` when that file exists (its first non-empty
line is also the manifest's one-line note, which the update bar shows), else a default line.

Since 1.1.1 the AI engine `mdedit-ai` is built too and goes into every package
(`docs/RELEASE.md`, "The files").
