> Archived 2026-11-21 from `docs/CI.md, section 'Findings from the first runs'` at commit `2b407d2`. History, not the current state: read `docs/STATE.md`. Nothing below this line was changed.

# CI: findings from the first runs


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
