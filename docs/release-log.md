# Release log

One section per published release, newest first: the run, what was in it, what was checked
against the live release and what was not. Add a section at the top after each release; never
edit an old one. How a release is cut is in `docs/RELEASE.md`.

The sections for 0.1.0 to 1.2.1 were moved here unchanged from `docs/RELEASE.md` on 2026-11-21,
and only put in order. There is no section for 0.5.1: the old file had none.

## v1.3.0 (published 2026-11-23 by the release workflow, from the tag at commit ebe939d)

Release run 45084291493, every job green; `ci` run 45081888446 on the commit passed (Linux, Windows,
both Macs). The `ci` runs before it had failed on Linux since the paged Settings: two labels were
too wide in the runners' fonts (fixed in ee18685, D-099). Release notes:
`docs/release-notes/1.3.0.md`.

What's in it: the notes library and its pane; Split into notes, by headings or by AI, with a
record of each run and undo; the original into the Recycle Bin; Move the notes with their
pictures; Settings as pages; the Preview dropdown and the other parts of issues #7 and #8; a
closed mdview noticed; and for Hub, a version resource in the exe and `hub.txt`, the sixteenth
asset and the first release to have it.

Before the tag: the owner's hand check of `main` in a live window (D-101, no blocker; "Move the
notes" not tried), and the budgets against 1.2.1 (no change,
`docs/measurements/2026-11-23-1.3.0-against-1.2.1.md`).

Verified against the live release:
- 16 assets, not a draft or pre-release;
- `mdedit-update.txt`, `mdedit-update-2.txt`, `mdedit-update-macos.txt` and `hub.txt` all name
  1.3.0, with the right note line;
- every file they name (Windows exe, `mdedit-ai.exe`, `mdview-engine.exe`, the setup, AppImage,
  both Mac zips) downloaded with the SHA-256 the manifests name;
- the published 1.3.0 exe's `--check-update` answered "nothing newer"; the installed 1.2.1's
  answered "mdedit 1.3.0 is available".

Not verified: the installer run, a 1.2.1 install updating itself (and recording its version under
the uninstall key, new in 1.3.0), Hub reading `hub.txt` from this release, the Mac and Linux files
on a desktop. Known faults shipped with it are in `docs/STATE.md`.

## v1.2.1 (published 2026-11-21 by the release workflow, from the tag at commit 528446a)

Release run 44850785598, every job green; `ci` run 44848935863 on the commit passed (Linux,
Windows, both Macs). The first `ci` run on 1.2.1 (44846981681, commit e82d360) failed in two
tests: the Modify room was a count of spaces, narrower in the runners' fallback fonts (fixed: the
room is measured in the font in use), and the focus test compared a path the tab had made
canonical. Release notes: `docs/release-notes/1.2.1.md`.

What's in it: the setting not to reopen tabs barely looked at (focused time), and the AI menu's
narrower Modify button, its room measured in the font in use.

Verified against the live release:
- 15 assets;
- `mdedit-update.txt`, `mdedit-update-2.txt` and `mdedit-update-macos.txt` all name 1.2.1;
- every file they name (Windows exe, `mdedit-ai.exe`, `mdview-engine.exe`, AppImage, both Mac
  zips) downloaded with the SHA-256 the manifests name;
- the published 1.2.1 exe's `--check-update` answered "nothing newer"; the published 1.2.0 exe's
  answered "mdedit 1.2.1 is available".

Not verified: the installer run, a 1.2.0 install updating itself, the Mac and Linux files on a
desktop, the Modify room in a Mac or Linux font (measured, not seen).

## v1.2.0 (published 2026-11-21 by the release workflow, from the tag at commit bd9cac3)

Release run 44840555985, every job green; `ci` run 44839022973 on the commit passed (Linux,
Windows, both Macs). Release notes: `docs/release-notes/1.2.0.md`.

What's in it: visual editing (Ctrl+E), the language switch and the .md offer, several custom AI
providers with or without a key, Modify for saved prompts, the caret kept per file, the stale
days setting, and every audit 2 fix (see `docs/HANDOFF.md`).

Verified against the live release:
- the release is not a draft or pre-release and names 15 assets;
- `releases/latest/download/` `mdedit-update.txt`, `mdedit-update-2.txt` and
  `mdedit-update-macos.txt` all name 1.2.0;
- the Windows exe (8,399,872 bytes), `mdedit-ai.exe`, `mdview-engine.exe` and the AppImage
  (17,504,760 bytes) downloaded with the SHA-256 the manifests name;
- the published exe's `--check-update` answered "nothing newer", exit 0.

Not verified: the installer run on Windows, a 1.1.2 install updating itself, the Mac and Linux
files on a desktop.

## v1.1.2 (published 2026-11-20 by the release workflow, from the tag at commit f8dce9b)

Release run 44651603606, every job green; `ci` run 44649826322 on the commit passed. Release
notes: `docs/release-notes/1.1.2.md`.

What's in it: the sixth audit round's two fixes (a reloaded tab stays editable; a due session
retry is made even when nothing changed). 1.1.1 is published and stays as it was; the version
was not reused.

Verified against the live release:
- the release is not a draft or pre-release and names 15 assets;
- `releases/latest/download/` `mdedit-update.txt`, `mdedit-update-2.txt` and
  `mdedit-update-macos.txt` all name 1.1.2 (format 1, format 2 and macOS);
- the Windows exe (8,335,360 bytes) and the AppImage (17,467,896 bytes) downloaded with the
  SHA-256 the manifest names.

Not verified: the installer run on Windows, a 1.1.1 install updating itself, the Mac and Linux
files on a desktop.

## v1.1.1 (published 2026-11-19 by the release workflow, run by hand with `publish`, at commit 905dac6)

Run 44608990427; the `ci` run on the commit (44604761547) passed on Windows, Linux, macOS arm64
and Intel.

What's in it:
- AI rewriting, with the AI engine `mdedit-ai` in every package and the format-2 Windows manifest
  (`mdedit-update-2.txt`);
- the recovery fixes of audit rounds 3 to 5;
- the AI activity strip, Save prompt, and image path paste.

1.1.0 was never published: its CI failed on the Intel Mac timing test, and the version was moved
to 1.1.1.

Released on the owner's word after Astra's round-5 findings were fixed (`3e6359c`). Astra's round
6 was deferred to a later version. The owner runs a local build of the same code (`3e6359c` plus
docs).

15 assets (exe 8,335,360 bytes, mdedit-ai.exe 2,265,088, AppImage 17,467,896).

Verified against the live release:
- all three manifests name 1.1.1;
- format 1 has no `mdedit-ai` key, and format 2 does;
- every file they name downloaded with the manifest's SHA-256;
- the tagged URL the repair reads (`releases/download/v1.1.1/mdedit-update-2.txt`) answers 200.

Not verified:
- a 1.0.6 install updating itself, then fetching `mdedit-ai` at its next start;
- the installer run on Windows;
- the Mac and Linux files run on a desktop.

## v1.0.6 (published 2026-11-18 by the release workflow, run by hand with `publish`, at commit 8969e4f)

Run 44314504746; the `ci` run on the commit (44314502216) passed. The offer to close tabs not
opened in the last 7 days of use (`mdshared_ui::stale`, shared `main` at `49de4ec`; setting
`offer_stale`, on), and the installer: no Open with entries for scripts and web pages (and those
of earlier installers taken off), .txt opened with mdedit (task `txtdefault`, ticked). Viewer
pinned at `31338da` (mdview 1.0.3). Released on the owner's word after they had the Windows test
builds of `e371df5` and `9e551d5` (the same code, before the merge and the version bump), with
the preview-link report (ticks left on after mdview's tab closes) left open until a fuller
report. 13 assets (exe 8,007,168 bytes, AppImage 15,444,472). Verified against the live release:
both manifests name 1.0.6, and every file they name downloaded with the manifest's SHA-256. Not
verified: the installer run on Windows (scripts' entries removed, .txt taken); a 1.0.5 install
updating itself; the Mac and Linux files run on a desktop.

## v1.0.5 (published 2026-11-18 by the release workflow, run by hand with `publish`, at commit a03b570)

Run 44261636486; the `ci` run on the commit (44261621346) passed. The tab left showing comes back
by identity (a pinned note no longer comes back as its neighbour) and the setting "Start on the
last pinned tab"; pin rules moved to `mdshared_ui::pins` (shared `main` at `6dcaf7f`); viewer
pinned at `9d3398a` (mdview's pinned tabs, on `main`, released as mdview 1.0.2 at `1ba74a4`).
The owner tried the Windows test build of `83fa762` (the same code, before the version bump):
"all work". 13 assets (exe 8,001,536 bytes, AppImage 15,444,472). Verified against the live
release: both manifests name 1.0.5, and every file they name downloaded with the manifest's
SHA-256. Not verified: a 1.0.4 install updating itself; the Mac and Linux files run on a desktop.

## v1.0.4 (published 2026-11-17 by the release workflow, run by hand with `publish`, at commit d3d2f58)

Run 44182210312; the `ci` run on the commit (44182209751) passed. The preview pane's cursor (a
hand over a task box) and the page kept still after a tick; viewer pinned at `56dfe05` (on
mdview's `main`, not an mdview release). The owner tried the Windows test build of `9c543c1`
(the same code, before the version bump) and said "good". 13 assets (exe 8,007,168 bytes,
AppImage 15,448,568). Verified against the live release: both manifests name 1.0.4, and every
file they name downloaded with the manifest's SHA-256. Not verified: a 1.0.3 install updating
itself; the Mac and Linux files run on a desktop.

## v1.0.3 (published 2026-11-17 by the release workflow, run by hand with `publish`, at commit ff30198)

Run 44149883394; the `ci` run on the commit (44149885114) passed. Ticking task boxes on the
preview pane; mdedit only, viewer pinned at `2dc2099` (on mdview's `main`, not an mdview
release). Released on the owner's word without a hands-on test. 13 assets, as for 1.0.0 (exe
8,005,632 bytes, AppImage 15,444,472). Verified against the live release: both manifests name
1.0.3, and every file they name downloaded with the manifest's SHA-256. Not verified: any
published file run on a desktop; a 1.0.2 install updating itself.

## v1.0.2 (published 2026-11-16 by the release workflow, run by hand with `publish`, at commit 61087cc)

Run 43979228491; the `ci` run on the commit (43979215380) passed. A right-click menu on the
preview pane; mdedit only, mdview pinned at `8faf059` as in 1.0.1. Released on the owner's word
without a hands-on test. 13 assets, as for 1.0.0 (exe 8,001,536 bytes, AppImage 15,444,472).
Verified against the live release: both manifests name 1.0.2, and every file they name
downloaded with the manifest's SHA-256. Not verified: any published file run on a desktop; a
1.0.1 install updating itself.

## v1.0.1 (published 2026-11-16 by the release workflow, run by hand with `publish`, at commit 1c589e9)

Run 43967599050 (the tag made by `gh release create`: the session could not push tags). Selecting
text on the preview pane; mdview pinned at `8faf059` (1.0.1). Released on the owner's word
without a hands-on test (the Windows test build of the branch was cancelled at their request);
the `ci` run on the commit was not waited for (it passed afterwards). 13 assets, as for 1.0.0.
Verified against the live release: both manifests name 1.0.1, and every file they name
downloaded with the manifest's SHA-256. Not verified: any published file run on a desktop; a
1.0.0 install updating itself.

## v1.0.0 (published 2026-11-16 by the release workflow, from the tag at commit 3e3f8d8)

Run 43937748930. The tag was pushed by the owner (the cloud session that cut the release could not
push tags through its proxy). The `ci` run on the same commit (43930227198) had passed on Windows,
Linux and the Intel Mac when the tag went out; its Apple-silicon Mac job, still waiting for a
runner then, passed afterwards. Released with mdview 1.0.0
(the pane's `--preview-server`, protocol `MDVPREVIEW3`); mdview-engine is built from the pinned
viewer revision `94280e0`.
Assets: `mdedit-1.0.0-windows-x86_64.exe` (7,969,280 bytes), `mdview-engine.exe`,
`mdedit-setup-1.0.0.exe`, `mdedit-1.0.0-windows-x86_64.zip`,
`mdedit-1.0.0-linux-x86_64.AppImage` (15,403,512 bytes), `mdedit-1.0.0-linux-x86_64.tar.gz`,
`mdedit_1.0.0_amd64.deb`, `mdedit-1.0.0-1.x86_64.rpm`, `PKGBUILD`,
`mdedit-1.0.0-macos-aarch64.zip`, `mdedit-1.0.0-macos-x86_64.zip`, `mdedit-update.txt` and
`mdedit-update-macos.txt`.

Verified against the live release: both manifests at the latest-download address name 1.0.0;
every file they name (exe, engine, AppImage, both Mac zips) downloaded and has the manifest's
SHA-256; the published AppImage's `--check-update` answered "nothing newer", exit 0.

Before publishing, the owner tried the preview pane's Windows test build of the same code (the
`preview-test` zip with the scroll-to-the-end fix) and approved it.

Not verified: the published files run on any desktop; an installed 0.7.1 updating itself to
1.0.0; the installer and the Linux packages installed.

## v0.7.1 (published 2026-11-15 by the release workflow, from the tag at commit fbb3ac7)

Run 43761807215; the `ci` run on the same commit (43760998757) had passed on Linux and Windows.
Assets: `mdedit-0.7.1-windows-x86_64.exe` (6,498,304 bytes), `mdedit-setup-0.7.1.exe`,
`mdedit-0.7.1-windows-x86_64.zip`, `mdedit-0.7.1-linux-x86_64.AppImage` (5,741,048 bytes),
`mdedit-0.7.1-linux-x86_64.tar.gz`, `mdedit_0.7.1_amd64.deb`, `mdedit-0.7.1-1.x86_64.rpm`,
`PKGBUILD` and `mdedit-update.txt`.

Verified against the live release: the manifest at the latest-download address names 0.7.1 with
a Windows and a Linux entry; the downloaded exe and AppImage have the manifest's two SHA-256
values.

Not verified: any published file run on any desktop (the hands-on tests ran local builds of the
same source); an installed 0.7.0 updating itself to 0.7.1; the deb, rpm, tar.gz and PKGBUILD.

## v0.7.0 (published 2026-11-15 by the release workflow, from the tag at commit 11c8d36)

Run 43739188755; the `ci` run on the same commit (43737712582) had passed on Linux and Windows.
Assets: `mdedit-0.7.0-windows-x86_64.exe` (6,483,456 bytes), `mdedit-setup-0.7.0.exe`,
`mdedit-0.7.0-windows-x86_64.zip`, `mdedit-0.7.0-linux-x86_64.AppImage` (5,732,856 bytes),
`mdedit-0.7.0-linux-x86_64.tar.gz`, `mdedit_0.7.0_amd64.deb`, `mdedit-0.7.0-1.x86_64.rpm`,
`PKGBUILD` and `mdedit-update.txt`.

Verified against the live release: the manifest at the latest-download address names 0.7.0 with
a Windows and a Linux entry; the downloaded exe and AppImage have the manifest's two SHA-256
values.

Not verified: any published file run on any desktop; an installed 0.6.0 updating itself to 0.7.0;
the deb, rpm, tar.gz and PKGBUILD installed.

## v0.6.0 (published 2026-11-15 by the release workflow, from the tag at commit c9d05b1)

Run 43701585904; the `ci` run on the same commit (43700523362) had passed on Linux and Windows.
Assets: `mdedit-0.6.0-windows-x86_64.exe` (6,381,568 bytes), `mdedit-setup-0.6.0.exe`,
`mdedit-0.6.0-windows-x86_64.zip`, `mdedit-0.6.0-linux-x86_64.AppImage` (5,671,416 bytes),
`mdedit-0.6.0-linux-x86_64.tar.gz`, `mdedit_0.6.0_amd64.deb`, `mdedit-0.6.0-1.x86_64.rpm`,
`PKGBUILD` and `mdedit-update.txt`.

Verified against the live release: the manifest at the latest-download address names 0.6.0 with
a Windows and a Linux entry; the downloaded exe and AppImage have the manifest's SHA-256 values.

Not verified: any published file run on any desktop; an installed 0.5.1 updating itself to
0.6.0; the Restart button after a real update (its first real use is the update after 0.6.0);
the installer and the Linux packages installed. The owner had not looked at the interface of
this release when it was published.

## v0.5.0 (published 2026-11-14 by the release workflow, from the tag at commit d49a0de)

Assets: `mdedit-0.5.0-windows-x86_64.exe` (6,124,544 bytes), `mdedit-setup-0.5.0.exe`,
`mdedit-0.5.0-windows-x86_64.zip`, `mdedit-0.5.0-linux-x86_64.AppImage` (5,585,400 bytes),
`mdedit-0.5.0-linux-x86_64.tar.gz`, `mdedit_0.5.0_amd64.deb`, `mdedit-0.5.0-1.x86_64.rpm`,
`PKGBUILD` and `mdedit-update.txt`. The Windows files were built natively on a Windows runner.

Verified against the live release: the manifest at the latest-download address names 0.5.0 with a
`windows-x86_64` and a `linux-x86_64` entry; the downloaded exe has the manifest's SHA-256
`cd59c3427f40c5b70c7cecf157cbf74eaf02c5234d4a4091a5a5c965b273e1df` and the AppImage
`e2ef0185383df0fc43e9148f3afced5efbe8d2b60ee26bfaea30d3b50e191841`; the published AppImage's
`--check-update` answered "nothing newer", exit 0.

Before publishing: an AppImage built from the same code offered, installed and became a newer
test build end to end on KDE Plasma 6 (see the driven-test section of `docs/HANDOFF.md`).

Not verified: that the published 0.4.0 Windows exe is offered 0.5.0 (the look was tried under
Wine and gave no output; the manifest grammar it reads is unchanged); the Windows files run by
hand on a Windows desktop; the deb, rpm, tar.gz and PKGBUILD of this exact build installed (the
dry-run builds of the same commit's parent were installed in Debian 12 and Fedora containers).

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

## v0.2.0 (published from commit 64ddaa0, 2026-11-13)

Cut so that the updater can be tried from a 0.1.0 install. Verified against the live release:
the manifest is served at the address above and names 0.2.0; the asset downloaded from its URL
(5984768 bytes) has the manifest's SHA-256; `mdedit --check-update` with the 0.2.0 exe answered
"nothing newer", exit code 0.

Not verified: that a 0.1.0 install is offered 0.2.0, and that it downloads and installs it.
That is the test this release is for.

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
