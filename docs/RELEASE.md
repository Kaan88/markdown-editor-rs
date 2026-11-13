# Releasing mdedit

## Cutting a release

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
  built by `toolselease.ps1`), per-user, with file associations. Not code-signed. An extra asset;
  the manifest does not name it.
- `mdedit-update.txt`: the manifest (`manifest: 1`, `version`, `notes`, and the three keys
  `windows-x86_64.asset`, `.url`, `.sha256`; grammar in `mdshared-app`'s `update.rs`).

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
