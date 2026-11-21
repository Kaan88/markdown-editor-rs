# Dependabot triage: the five open PRs on `markdown-editor-rs`

Read-only triage. Nothing was merged, pushed, or committed to `main`. Every build ran in a
throwaway worktree under `C:\Users\kaank\Desktop\md-wt\`, with a scratch `MDEDIT_SETTINGS` so no
build could touch the owner's session; the owner's checkout was not written to at all.

Tested against `main` at **`9467859`** ("docs: the branches and worktrees as they are"), the commit
the dependabot branches are based on. Local toolchain: **rustc 1.99.1**, the same as CI's
`RUST_TOOLCHAIN`.

---

## The short version

| # | Bump | Real break, or a stale branch? | Verdict |
|---|---|---|---|
| 2 | `rfd` 0.15.4 → 0.18.0 | **No break at all.** It compiles and its last CI run (20 Nov) is green. | **Close it.** Take the bump in `markdown-shared-rs` first; it is the wrong shape as a mdedit-only PR. |
| 3 | actions group (4 actions) | **No break.** Its only red run is `main`'s clippy breakage from 19 Nov, already fixed. Rebases cleanly. | **Take it.** Rebase, rerun, expect green. |
| 4 | `windows-sys` 0.59 → 0.61.2 | **No break in the bump.** Windows job is green; Linux/macOS red from a `main` test that was being fixed while the run was going. | **Take it** after main's test fix is in. 1.5 KB smaller. |
| 5 | `chardetng` 0.1.17 → 1.0.0 | **Real break, and a semantic one:** `guess_assess` (the "am I sure?" answer the loader relies on) is gone. | **Close it**, or take with a deliberate behaviour change and new tests. |
| 6 | `parley` 0.12.0 → 0.13.0 | **Real break on every platform**, plus a three-repo problem. | **Close the three PRs** (mdedit, shared, mdview) and do parley 0.13 as one planned change. |

The important finding: **three of the five "red for six days" stories are not about the bumps.** On
2026-11-21 22:41–22:43 UTC dependabot rebased every branch onto `main` and reran CI, and `main`
itself was red at that moment (Linux and macOS). So the cargo PRs were painted red by the base.
Two of them (2 and 4) would go green by themselves once `main`'s test fix landed; one (3) never
even got a fresh run after its base was fixed.

### Timeline (all dates +0300, the owner's clock)

- `2026-11-19 17:38` — PR 3's last CI run starts, fails on `main`'s clippy lint.
- `2026-11-19 17:45` — `4b9e90c` "the locked-copy test passes clippy on Unix" fixes that. PR 3 has not been rerun since.
- `2026-11-21 02:47` — PR 2 is rerun, green.
- `2026-11-22 01:41` — `main`'s own CI runs at `9467859`; Linux and macOS fail on a path test. Windows passes.
- `2026-11-22 01:43` — dependabot rebases and reruns all five PRs against that same red `main`.
- `2026-11-22 02:01` — `e8530f7` fixes the path test on `main` — about twenty minutes after those PR runs started (`0c41787`, at 01:25, fixed the neighbouring one).

So PRs 2 and 4 were red only because `main` was red on non-Windows at the second they ran, and
PR 3's red is a three-day-old run against a lint that was fixed seven minutes after it started.
`main` is green again on all four platforms at `ab00a62` (run `44959762896`), so a rebase is all
that 3 and 4 need.

---

## 1. `rfd` 0.15.4 → 0.18.0 (PR #2)

### Why CI fails

It does not, now. Its run created 2026-11-20 23:47 UTC (`44839306081`) is green on all four
platforms, and on `main@9467859` locally:

```
cargo clippy --release --workspace --all-targets -- -D warnings   → exit 0
cargo build --release -p mdedit                                   → exit 0
```

The one red run it had (19 Nov) is the same `main` clippy break as PR 3:

```
error: this let-binding has unit value
  --> bins/mdedit/src/recovery.rs:915:9
= note: `-D clippy::let-unit-value` implied by `-D warnings`
```

### Is it a breaking change?

For our code, no. The only call site is `bins/mdedit/src/platform.rs:34`,
`rfd::FileDialog::new()…save_file()`, and that API is unchanged across 0.15 → 0.18. What changed
in between (from rfd's changelog):

- 0.16 — `windows-sys` 0.60, the `wayland` feature became optional, `set_parent` takes `?Sized`.
- **0.17 — the `tokio` and `async-std` features were removed.**
- 0.18 — GTK3 leak fixes, a GTK4 backend, MSRV 1.92, `set_show_hidden_files`, filter fixes.

The 0.17 line matters for the *shared* crate, not for this PR: `markdown-shared-rs`'s
`mdshared-ui` Linux target asks for `features = ["xdg-portal", "async-std"]`, which 0.17 deleted.
So bumping rfd there is not a one-line edit.

### Where it is used

- `bins/mdedit/src/platform.rs:34` — the **Save as** dialog, in a `#[cfg(windows)]` block: mdedit's
  rfd dependency is under `[target.'cfg(windows)'.dependencies]` only (`bins/mdedit/Cargo.toml:40-44`).
- `markdown-shared-rs` → `mdshared-ui/src/platform.rs:21,32,138` — the open/save pickers used by
  both apps; `rfd 0.15.4` on Windows and on Linux (`xdg-portal`, `async-std`).
- mdview has no rfd of its own; it inherits `mdshared-ui`'s.

### What it costs

- The PR leaves **two rfds in one Windows binary**: `rfd 0.18.0` (mdedit, Save as) and `rfd 0.15.4`
  (mdshared-ui, everything else). `cargo tree` confirms both, and both `windows-sys 0.59` and
  `0.61.2` in the same link.
- That directly contradicts the note the code carries: *"The Save as dialog; the same version and
  features as mdshared-ui, which opens the others."* (`bins/mdedit/Cargo.toml:43`). Two file-dialog
  implementations in one app is a support surface, not a build error.
- Size: `mdedit.exe` 8,893,440 bytes with the PR vs 8,890,880 on `main` — **+2,560 bytes (+0.03 %)**.
- Launch path: none. rfd only runs when the dialog is opened.
- Linux/macOS: **zero effect**, because mdedit does not depend on rfd there.

### Recommendation

**Close PR #2 as a mdedit-only bump.** It is not broken, it is just the wrong order: do the bump in
`markdown-shared-rs` (both the Windows and the Linux target, replacing `async-std` per rfd 0.17),
run `tests/shots.sh` in both apps there, push, move the `mdshared-*` pins, then bump mdedit's
Windows `rfd` in the same change. That is maybe an hour of work, but it is a cross-repo change with
a screenshots pass, not a dependabot merge.

---

## 2. GitHub actions group, 4 updates (PR #3)

`actions/checkout@v4 → v7`, `actions/upload-artifact@v4 → v7`, `actions/download-artifact@v4 → v8`,
`actions/cache@v4 → v6`, across `ci.yml`, `mac-look.yml`, `preview-test.yml`, `release.yml`.

### Why CI fails

Its only failing run is from **2026-11-19**, and the error is in Rust code, not YAML:

```
error: this let-binding has unit value
  --> bins/mdedit/src/recovery.rs:915:9
error: calls to `std::mem::drop` with a value that implements `Copy` does nothing
  --> bins/mdedit/src/recovery.rs:935:9
= note: `-D clippy::let-unit-value` implied by `-D warnings`
error: could not compile `mdedit` (bin "mdedit" test) due to 2 previous errors
```

That is `main`'s own Unix-only test code, fixed seven minutes later by `4b9e90c`
("the locked-copy test passes clippy on Unix"). PR 3 has not been rerun since, so it still shows
the old red.

### Is it a breaking change?

No. Evidence: the same four action versions **already ran green on this branch** on 2026-11-16
(run `43916112088`, success). The workflows in a `pull_request` run come from the PR head, so those
runs used checkout@v7 etc. (The other "failure" that day, `43930652422`, is a cancelled macOS job
from `concurrency: cancel-in-progress`, with Linux, Windows and the other Mac green.)

### Where it is used

`.github/workflows/{ci,mac-look,preview-test,release}.yml`. Rebased onto `main@9467859` in a
worktree: applies cleanly (29 lines across 4 files). After the rebase **every** `actions/*`
reference in the repo is on the new version — `git grep` finds nothing left at `@v4`.

### What it costs

Nothing measurable. No Rust code, no dependency in the binary. The one thing to check by eye is
`docs/CI.md`, which describes the workflow and may still name v4.

### Recommendation

**Take it.** Rebase (or `gh pr comment /dependabot rebase`), let it run once, merge. Zero risk.

---

## 3. `windows-sys` 0.59.0 → 0.61.2 (PR #4)

### Why CI fails

Not the bump. The Windows job — the only one that compiles `windows-sys` — **passes** (in the
latest run and the one before). Linux and macOS fail in `mdedit-core`, on a path test:

```
macos: test runs::tests::a_record_reads_back_with_its_paths_and_survives_a_hostile_file ... FAILED
thread '…' panicked at crates/mdedit-core/src/runs.rs:2999:9:
assertion `left == right` failed
linux: (the same test, the same line)
```

And `main` itself failed the same way at `9467859` in run `44958041996`:

```
test runs::tests::a_path_written_into_a_record_reads_back_as_the_same_path ... FAILED
assertion `left == right` failed: from C:/library/Group/One.md
```

i.e. a Windows path rule asserted on Unix. It was fixed on `main` at `e8530f7`
("the record's path test pins the rule, not this machine's Path"), committed at **02:01**, about
twenty minutes after the PR runs started. Nothing in `windows-sys` is involved.

Locally, on Windows, against `main@9467859`:

```
cargo clippy --release --workspace --all-targets -- -D warnings   → exit 0
cargo test   --release --workspace                                → exit 0 (whole suite)
```

### Is it a breaking change?

Not for the API surface we use. It compiles unchanged: `GetACP`, `ReplaceFileW`,
`MoveFileExW`, the clipboard `DataExchange`/`Memory` calls, `K32GetProcessMemoryInfo`,
`GetProcessTimes`. The feature names in both `Cargo.toml`s still exist.

### Where it is used

- `bins/mdedit/src/platform.rs:66-435` — clipboard, `GetACP`, `--trace` memory/CPU figures.
- `crates/mdedit-core/src/save.rs:377-381` — `ReplaceFileW`, the crate's one `unsafe` block.
- **`markdown-shared-rs`**: `mdshared-app`, `mdshared-fetch`, `mdshared-ui` all pin `"0.59"`;
  `mdview`'s `bins/mdv-app` does too. Those are not bumped by this PR, and do not need to be:
  `windows-sys` is generated bindings, and already appears at three versions on `main`
  (0.52 through `rustix` 0.38, 0.59, and 0.61.2 through `notify 8.2.1`). This PR moves mdedit's
  own edge from 0.59 to a version that is already linked.

### What it costs

- Size: `mdedit.exe` **8,889,344** bytes with the PR vs **8,890,880** on `main` — **1,536 bytes
  smaller (−0.02 %)**. mdedit's own 0.59 copy goes away and the 0.61.2 the tree already links
  through `notify` is reused, so nothing new reaches the link at all.
- Launch path: no behavioural change; the same Win32 calls, same signatures. Nothing to measure
  beyond the exe size already taken.
- mdview already has the same bump green (its PR #3, all four platforms passing on 2026-11-16/21).

### Recommendation

**Take it**, after `main` contains `e8530f7` (it does as of `ab00a62`): rebase and it should be
green on all four. Nothing to absorb.

---

## 4. `chardetng` 0.1.17 → 1.0.0 (PR #5)

### Why CI fails

A genuine compile error on **all four** platforms, from one function:

```
error[E0061]: this function takes 1 argument but 0 arguments were supplied
   --> crates\mdedit-core\src\load.rs:755:24
755 |     let mut detector = chardetng::EncodingDetector::new();
    |                        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^-- argument #1 of type
    |                                                          `chardetng::Iso2022JpDetection` is missing

error[E0599]: no method named `guess_assess` found for struct
              `chardetng::EncodingDetector` in the current scope
   --> crates\mdedit-core\src\load.rs:757:34
757 |     let (guess, sure) = detector.guess_assess(tld_hint(opts.legacy), false);
    |                                  ^^^^^^^^^^^^ method not found
```

Reproduced locally on `main@9467859` (branch already based on current `main`, so no rebase is
pending): the same two errors.

### Is it a breaking change?

Yes, and not only syntactically. chardetng 1.0.0's own release notes:

- "Remove the `guess_assess` API."
- "Add control whether ISO-2022-JP detection is considered or not."
- "Use two-variant enum instead of a boolean for whether UTF-8 detection is allowed."

We use exactly the thing that was removed. `choose_encoding` is:

```rust
let mut detector = chardetng::EncodingDetector::new();
detector.feed(sample, whole);
let (guess, sure) = detector.guess_assess(tld_hint(opts.legacy), false);
Detect::Legacy(if sure { guess } else { opts.legacy })
```

`sure` is a **confidence signal**, and it is what makes the loader fall back to the system code page
(`opts.legacy`, from `GetACP`) when the detector is only guessing. 1.0's `guess()` returns just
`&'static Encoding` — the confidence is gone from the public API, with no replacement. Porting the
call mechanically (`guess(..., Utf8Detection::Deny)`) means every non-UTF-8 file is taken as the
detector's guess instead of the system page: on a Turkish Windows (cp1254), Turkish files that used
to come back as Windows-1254 would come back as the detector's default instead.

### Where it is used

Only mdedit: `crates/mdedit-core/src/load.rs:755` (`choose_encoding`), reached from `detect()` on
the file-open path. `markdown-shared-rs` and mdview do not depend on it. So it is a **local** job,
unlike parley.

Tests that pin the current behaviour and would need a decision, not a rename:

- `crates/mdedit-core/tests/load.rs:146 invalid_utf8_without_bom_falls_back_to_windows_1254`
  — three bytes `F0 FD FE` must come back as the *system* page, not the detector's guess.
- `crates/mdedit-core/tests/load.rs:165 legacy_page_is_guessed_from_the_text` — a Turkish fixture
  must be found as windows-1254 whatever the system page is.
- `crates/mdedit-core/tests/load.rs:811 forcing_what_the_detector_chose_changes_nothing`.

### What it costs

Not the launch path — the detector runs when a file is opened, never at startup, and only on the
first non-UTF-8 bytes. The real cost is a **behaviour decision plus its tests**: either accept that
the "not sure → system code page" rule disappears (and rewrite the two tests to the new rule, and
say so in the release notes), or keep 0.1.17 and wait for chardetng to report confidence again.

### Recommendation

**Close it.** This is a 1.0 in name only, and the API it removes is the one this app needs. The
bump buys nothing we can see (no detection-accuracy fix in the 1.0.0 notes) and loses a rule the
loader was deliberately built around. If the owner wants the 1.0 line anyway, it is a real change:
`Iso2022JpDetection::Deny` (mdedit never asked for ISO-2022-JP), `Utf8Detection::Deny`,
`guess()` instead of `guess_assess()`, and the two tests above rewritten to whatever new rule is
chosen, in the same commit.

---

## 5. `parley` 0.12.0 → 0.13.0 (PR #6)

### Why CI fails

Two independent real breaks, on every platform. Reproduced locally on a branch already on
`main@9467859`:

```
error[E0599]: no method named `align` found for struct `parley::Layout<B>` in the current scope
error[E0308]: mismatched types
   --> bins\mdedit\src\view.rs:1341:29
     expected `parley::layout::layout::Layout<[u8; 4]>`, found `parley::Layout<[u8; 4]>`
note: there are multiple different versions of crate `parley` in the dependency graph
error: could not compile `mdedit` (bin "mdedit") due to 8 previous errors
```

- The `E0308`s are **two parley versions in one binary**: `mdshared-text` (pinned `=0.12.0` by
  commit) hands `parley 0.12` `Layout`/`FontContext` types to mdedit code that now uses `0.13`.
- `align` was **removed in 0.13** in favour of `Layout::set_alignment` called *before*
  `break_all_lines` (`bins/mdedit/src/view.rs:723`).

### Is it a breaking change?

Yes, squarely. From parley's changelog for 0.13.0 (2026-11-06):

- "Breaking change: `Layout::align` has been removed in favour of `Layout::set_alignment`."
  Also: "`Start` alignment is now the default (it was previously implicitly `Left`)".
- "Breaking change: in-flow inline boxes are now represented by U+FFFC…", `inline_boxes_mut`
  replaced by `edit_inline_box`, `ShapedRun::advance` removed, `ShapedRun` is `#[non_exhaustive]`.
- Fixed: descent metrics encoded positive, `Cluster::visual_offset` with `set_line_x`,
  "Empty text is no longer laid out as a space", letter spacing on zero-width characters.

Those "fixed" lines are why a screenshots pass is mandatory, not optional: 0.13 changes where
carets and empty lines land.

### Where it is used

`parley = "=0.12.0"` is pinned in **three repositories**, and the type crosses the crate boundary
in two of them:

| repo | file | site |
|---|---|---|
| mdedit | `Cargo.toml:28` (workspace pin) | `bins/mdedit/src/view.rs:723` `layout.align(Alignment::Start, …)` |
| shared | `crates/mdshared-text/Cargo.toml:8` | `src/text.rs:497` `layout.align`, and `text.rs:332 draw_layout_clipped`, the boundary mdedit calls |
| shared | `crates/mdshared-ui/Cargo.toml:10` | `src/gfx.rs:179` `layout.align` |
| shared | — | `crates/mdshared-text/examples/fontprobe.rs:73` |
| mdview | `bins/mdv-app/Cargo.toml:19`, `crates/render/Cargo.toml:12` | `render/src/layout/pager.rs:601,705,706`, `prose.rs:120` |

The three dependabot PRs for parley — mdedit #6, shared #7, mdview #6 — are all open and all red.
Shared #7 fails with the same `no method named align` in `mdshared-text` and `mdshared-ui`.

`spikes/` also pins `=0.12.0` (`spikes/longlines`), but it is excluded from the workspace, so it
does not affect CI; it would simply stop building until it was moved too.

### What it costs

- **It cannot be taken in mdedit alone** even if it compiled: the lock shows two complete text
  stacks (`fontique` 0.12/0.13, `harfrust` 0.12/0.15, `skrifa` 0.44/0.49, `read-fonts` 0.41/0.46,
  and two ICU data sets — 2.3 *and* 2.4). That is duplicated font and Unicode data in the binary
  and in memory.
- **It is on the launch path.** `view.rs` shapes the visible lines with parley before the first
  frame (`docs/DESIGN.md:359`). The budgets are first frame < 100 ms and private memory < 30 MiB
  (`docs/PLAN.md:52-53`). 0.13 changes line metrics and alignment defaults, so both the numbers and
  the shots must be measured; saying "it's a patch" is not available here.
- The work: ~8 `align` call sites in three repos, `set_alignment` before breaking, then both apps'
  `tests/shots.sh` in both themes, and the budgets.

### Recommendation

**Close the three parley PRs (mdedit #6, shared #7, mdview #6)** and do "parley 0.13" as one
planned change, in the order the project's rule 8 asks for: shared first (its own shots), push, move
the `mdshared-*` pins in mdview and mdedit, then each app's shots and budget run. It is a day of
careful work, not five minutes of `cargo update`, and it is exactly the kind of change where the
owner has to look at the shots.

---

## 6. Is dependabot configured sensibly? (`.github/dependabot.yml`)

The file is sane in shape: weekly on Monday, `mdshared-*`/`mdview-*`/winit/softbuffer left alone,
minor+patch grouped, actions grouped, majors left on their own. The same file is copied verbatim in
all three repos. Two things are wrong, and one is structural.

1. **`open-pull-requests-limit: 5` is too low for a week with four majors.** When four majors
   arrive at once (this week: parley, rfd, windows-sys, chardetng) they fill the queue. In
   `markdown-shared-rs` the cargo queue is **already at 5/5** (parley, x11rb, windows-sys,
   libloading, notify-group), so tomorrow's run cannot open anything at all. mdedit is at 4/5 and
   the Monday minor+patch group will take the fifth. **Raise it to 10.**
2. **There is no way for a PR whose base is red to stop looking like a bad PR.** Three of these five
   were red because `main` was red when dependabot happened to rerun inside it (one of them, PR 3,
   from a lint fixed seven minutes after its run started). That is not fixable in YAML, but it is
   worth knowing: **look at the base run before believing a dependabot red**, which is what this
   triage had to do for every PR.
3. **Cross-repo crates will never pass alone.** `parley` and `rfd` are pinned by hand in
   `markdown-shared-rs`, so dependabot's single-repo PRs for them are red by construction — three
   PRs for parley, in three repos, all failing, is not five minutes of attention, it is a project.
   The config should either ignore `parley` and `rfd` (and `windows-sys`, if the owner wants one
   version across the repos) until the shared bump is done, or the repos should bump them in a
   planned change and let dependabot keep them out of the queue. Right now they spend three of the
   five slots each week.

Smaller, optional: `labels: ["dependencies"]` would make them greppable; `commit-message` already
has a prefix; a `cooldown` (a day or two) would stop a brand-new release arriving before anyone has
looked at it — not a problem this week.

Note the process the config assumes is in `docs/DECISIONS.md` D-012: PRs are merged by hand once CI
passes *and the budgets are measured*. Six days and five open PRs is what that process looks like
when nobody has a slot for a cross-repo change; the config itself did not stop anything except new
PRs in the shared repo.

---

## What was run, and what was not

**Run here (Windows, rustc 1.99.1):**

| Worktree | Branch | What | Result |
|---|---|---|---|
| `md-wt\deps-6` | `triage/pr6` (parley) | `cargo clippy --release --workspace --all-targets -- -D warnings` | 8 errors, same as CI |
| `md-wt\deps-5` | `triage/pr5` (chardetng) | same | 2 errors, same as CI |
| `md-wt\deps-4` | `triage/pr4` (windows-sys) | same, then `cargo test --release --workspace`, then a release build for size | clippy 0, tests 0 |
| `md-wt\deps-2` | `triage/pr2` (rfd), rebased onto `main@9467859` | same, then full test suite, plus a release build for size | clippy 0, tests 0 |
| `md-wt\deps-3` | `triage/pr3` (actions), rebased onto `main@9467859` | rebase and `git grep` over the workflows | clean, no `@v4` left |
| `md-wt\main` | `main@9467859` | `cargo build --release -p mdedit` for the size baseline | 8,890,880 bytes |

Branches 5, 6 and 4 needed no rebase: dependabot had already rebased them onto `9467859` on
2026-11-21. The three exe sizes are plain `cargo build --release -p mdedit` releases (not the
`cargo test` build, whose dev-dependencies unify one feature set differently), each next to the
other in one target directory, so they compare like with like.

**Not run:** Linux and macOS builds (no host here); the screenshot suite (`tests/shots.sh`) — no
display-independent shot pass was needed for a report and the owner approves visuals by looking;
`markdown-shared-rs`'s and mdview's own test suites. The size figures are Windows release exes from
one shared `target` directory, so they compare like with like, but they are not a full budget run.
No dialog was opened by hand: PR 2's Save as dialog compiles, links and its suite passes, but
nobody clicked it.

**Not verified:** whether parley 0.13 moves first frame or memory — that needs the measurement the
project asks for (`docs/PLAN.md`), and it is only worth doing if the bump is taken.

**Cleanup:** all six worktrees I made under `C:\Users\kaank\Desktop\md-wt\` (`deps-2`…`deps-6`,
`main`) and their `triage/*` branches were removed, along with the scratch `target` directory. I
wrote nothing in the owner's checkout. It now shows `docs/ecosystem-ideas.md` and `docs/STATE.md`
modified — both are other agents' work, not mine; other worktrees (`move-notes`, `p1-*`, `bench-*`)
were left exactly as they were.
