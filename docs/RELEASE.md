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
