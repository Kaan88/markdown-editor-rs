# Settings redesign: categories as pages, nothing scrolls (a design to approve)

Written 2026-11-21 on `mock/settings`. Nothing here is built: the pictures in
`shots/mock-settings/` are drawn by the app's own painter, fonts and palette from the real `Row`
data (`app::modal::rows`), with the control drawing copied from `mdshared-ui/src/settings.rs`, so
every control is the one the app draws today. The layout engine is the mock's own
(`bins/mdedit/src/mock/settings.rs`); its numbers are the ones proposed below. Regenerate with
`cargo build -p mdedit` then `powershell -File shots/mock-settings/make.ps1` and
`python shots/mock-settings/sheet.py`; `--mock settings-report` prints the fit numbers.

## 0. Why it keeps costing rounds

The form has "categories" but they are a **scroll jump list**: one body holding every row, a list
that scrolls it to a heading. Types and constants, all in `mdshared-ui`:

- `settings::Form` (layout, drawing, keys, pointer, `scroll`, `pin`, `pending_jump`,
  `jump_to_category`, `current_category`, `wheel`, the scrollbar) and `settings::Row` (`Header`
  starts a group; the groups *are* the categories).
- Heights: `ROW_H` 34, `ROW_HELP_H` 40, `HEADER_H` 38 / `HEADER_FIRST_H` 30, `BODY_TOP` 60
  (`DIALOG_PAD` 24 + 36), `NAV_W` 176, `NAV_ITEM` 34, `NAV_MIN_CARD` 700, card `MAX_CARD_H` 640
  (`SINGLE_MAX_CARD_H` 720 to 760 in the pinned revs; mdview now uses the 640).
- `overlay::dialog_card(cw, ch, w, h)` shrinks the card to `h - 48`; `DIALOG_FOOTER` 72 is spent
  on one Close button; `Form::layout` asks for `BODY_TOP + content + 4 + DIALOG_FOOTER`.
- Hosts: mdedit `app/modal.rs` (`rows`, `settings_rows`, `ai_rows`, `open_settings`,
  `settings_key/pointer/press/release/wheel/outcome`), mdview `bins/mdv-app/src/settings.rs`
  (`rows`, `form`).

The arithmetic that makes it unfixable by trimming rows: mdedit's rows add up to about **2,030 px**
(my sum of the form's own heights over the real rows, AI at its tallest; not measured by running
it). The body is 504 px at 1280x800 (640 - 60 - 72 - 4) and **176 px at the 520x360 minimum**
(card 488x312, and no category list at all, because the card is under 700 wide). So the
minimum window shows five rows and a scrollbar, and the list is gone. The old promise
("one page at the default and the minimum") was only ever kept at 1280x800, and cost a row there
too (HANDOFF "fits with 1 px to spare").

The stronger promise, tested instead of hoped: **every page fits its page area at 520x360, at
scale 1, 1.5 and 2, in the tallest state each page can be in.**

## 1. The grouping

### mdedit: 9 categories, 45 settings, none removed

| Category | Holds (today's group) | Why |
|---|---|---|
| **Appearance** | Theme, Window backdrop, Window corners (View); Font, Font size (Editing) | how the app and its text look; the three window rows are the same three in mdview |
| **Editor** | Line numbers, Highlight the current line, Word wrap for new documents, Minimap for new windows, Minimap width (child), Live preview while typing (was the one-row "Preview") | what the editing surface shows |
| **Indentation** | Tab width, Detect indentation, Indent with spaces (child), Indent size (child) | one idea: how a line is indented; the two children are the fallback when detection finds no style |
| **Typing** | Close brackets and quotes, Automatic indent, Suggest words while typing; Trim trailing spaces, End with a line break (was "Saving") | what the editor does for you as you type and save |
| **Notes library** | Library folder + Choose (two rows become one) | its own feature with its own pane and menu; one row now, about four when the library plan's folder-per-group and AI import land |
| **Tabs** | Restore tabs at start, Start on the last pinned tab, Offer to close tabs left unopened (+ days, child), Don't reopen tabs barely looked at (+ seconds, child; Leave out the last tab too, child) | everything about which tabs come back (was "Session") |
| **AI service** | Connection, Provider (+ remove), Name (child), Base URL, API key (+ Save) or Program (+ Check) | who the text is sent to and how you sign in |
| **AI rewrite** | Model (+ refresh), Model name (child), Effort, Show changes before applying, Saved prompts | what the rewrite does once connected |
| **Updates** | Check for updates at launch, Look for a newer version; on an AppImage also Make mdedit the default | housekeeping |

**Moved:** Theme, Backdrop, Corners out of "View" (a window property, not an editor view option);
Font and Font size out of "Editing" into Appearance; Live preview out of its own one-row group
into Editor; the two Saving rows into Typing; the library folder out of the middle of the list
into its own page. **Split:** AI into two pages (service, rewrite): one page cannot hold 13 rows,
and the two halves are used at different times. **Merged:** four action rows become buttons on the
row they belong to (Choose..., Store the key, Remove this provider, Fetch again), with the same
ids and the same `Outcome::Action`, so the host's outcome code does not change. **Removed:** none.
Candidates I did not remove, the owner's call: Minimap width, Start on the last pinned tab.

**Reworded**, only where a help line or label cannot be one line at 520 px (a rule, below), the
same facts in fewer words, all on Tabs:
Start on the last pinned tab "The pinned tab you used last shows at start, not the tab you left
on." to "Not the tab you left on."; Offer to close "At start. Pinned and changed tabs are never
offered." to "Pinned and changed tabs are never offered."; Offer them after (help "Counted in days
you use mdedit, not calendar days.") to the unit "days of use", no help; Don't reopen "At start.
Pinned tabs and unsaved text always come back." to "Pinned tabs and unsaved text always come
back."; Looked at for less than: unit "seconds" to "sec" (the label was being cut), help "Only
while the tab is in front and mdedit has focus." to "Counted only while in front."; Leave out the
tab you were last on too to "Leave out the last tab too", help dropped (the line above says it).
Every other help line and label is today's text.

**New behaviour on existing rows:** a setting that only matters while another is on is a **child**
(indented 20 px, a thin guide) and is **disabled with its reason in the help slot** while the
parent is off ("Needs the switch above."): days, seconds, the last-tab switch, minimap width stays
enabled. On AI rewrite the rows no longer vanish when the connection is Off or the model has no
effort levels; they stay, disabled, with the reason ("This model has no effort levels."), so the
page keeps one height and the fit proof is about one shape.

### mdview: 4 categories, same shell

| Category | Holds | |
|---|---|---|
| **Appearance** | Page width, Text size, Page font, Code font; Theme, Window backdrop, Window corners | 7 rows, mdview's tallest |
| **Reading** | Always load remote images, Tick task boxes without asking, Play animated images, Preview footnotes and links on hover (was "Content") | how documents behave |
| **Tabs** | Start on the last pinned tab, Offer to close tabs not opened in a week, Don't reopen tabs barely looked at (+ seconds, child) | |
| **Updates** | Check for updates at launch, Look for a newer version; AppImage: Make mdview the default | |

**Must be the same in both:** the shell (card, list, ×, row metrics, help rule, keyboard); the
names and the *order* of the shared categories (Appearance, Tabs, Updates); the label and help
text of the shared rows, which already come from `mdshared_app::brief`, `backdrop::Backdrop` and
`corners` (so they cannot drift); window rows in the order Theme, Backdrop, Corners.
**Must differ:** Editor, Indentation, Typing, Notes library, AI service and AI rewrite exist only
in mdedit; Reading only in mdview; Appearance's text rows (mdedit one monospace font and a size in
points, mdview a page font, a code font and a text size in percent); Tabs (mdview has a fixed week
and no stepper, no "Restore tabs", no "last tab too"). I would let mdview keep "Reading" and not
rename it "Content": "Content" says nothing about what is in it.

## 2. The shape

- **Card:** the window less 6 px on each side, up to **780 x 420**, centred; so 780x420 at
  1280x800 and **508x348 at the 520x360 minimum**. The same card for every page: the page you
  leave and the page you reach are the same size, nothing jumps.
- **Header (42 px, over both columns):** "Settings" (17 px semibold) at the left, a 28 px close
  button at the right, a hairline under. **There is no footer and no Close button.** Settings
  applies live, so the 72 px footer held one button that Esc already is. Esc, the × and a click
  outside close it. This is the biggest single saving (72 px) and the one thing the owner should
  say yes or no to.
- **Category list (left):** 168 px wide (120 px when the card is under 640 wide, which is any
  window under about 650), items 32 px (28 px at the minimum), 13 px text, muted unless current.
  The current page is marked as today: a filled rounded row (`pal.on`) with a 3x16 accent bar.
  With the keyboard in the list the current item also has the focus ring (`edit-indentation-kb_*`).
  The list never collapses and never scrolls: at the minimum it holds **10 items** (4 + 10x28
  = 284 of 304 px); mdedit uses 9. A hairline divides it from the page.
- **Page (right):** nothing else: no page title (the list says where you are), no scrollbar, no
  group headings. Groups inside a page are told apart by an 8 px gap and the hairlines that
  separate rows; the engine can draw a small-caps sub-heading but no page needed one.
- **Buttons:** none in the dialog frame. Actions are buttons in their rows (Choose..., Save,
  Edit..., Check now); small ones are icons (remove, refresh).
- **Search:** none, for now. 9 pages of at most 7 rows is a click and a glance; a search needs a
  results page, and a results page is the one page whose height is not known (it would have to
  be cut at N matches, which is the scroll problem again). If it comes, it goes at the top of
  the list, 36 px, and the list still fits at the minimum (256 + 36 = 292 of 304).
- **Keyboard:** the dialog opens with the keyboard in the list, on the page last shown this run
  (Appearance the first time; a caller may open a page directly, e.g. the AI failure bar opens
  AI service). **Up/Down, Home/End, PageUp/PageDown in the list move and show the page at once**
  (no Enter: a page has no side effects). **Tab, Right or Enter** from the list goes to the
  first enabled control of the page; Tab goes on through the page's controls, then the ×, then
  back to the list; Shift+Tab reverses (so Shift+Tab from the first control is "back to the
  list"). **Ctrl+PageUp / Ctrl+PageDown switch page from anywhere**, including inside a text
  box. **Esc** closes a dropdown first, then the dialog, as now. A letter in the list jumps to
  the next category starting with it (as the notes pane does). Typing does not filter. Disabled
  rows are skipped by Tab and arrows; their reason is on the row, so a pointer or a screen
  reader finds it without a tooltip.

## 3. The mockups (`shots/mock-settings/`)

`<app>-<page>[-state]_<default|min>_<light|dark>.png`, `default` = 1280x800, `min` = 520x360;
`edit` = mdedit, `view` = mdview (drawn over mdedit's window, which is only the picture behind).

- Dense with toggles: `edit-typing_*`, `edit-tabs_*`, `edit-editor_*`
- Text fields and a folder picker: `edit-notes-library_*` (stacked field + Choose...),
  `edit-ai-service_*` (inline and stacked fields, a secret key, three icon/text buttons)
- Short: `edit-updates_*`, `edit-indentation_*`
- The hard state: `edit-ai-rewrite-hard_*` (AI rewrite with a two-line error help line under
  Model, Model name focused with its caret and the accent underline, Effort disabled with its
  reason); also `edit-tabs-off_*` (two disabled children with their reasons)
- Keyboard in the list: `edit-indentation-kb_*`
- Every page of both apps at the minimum on one sheet: `sheet-edit-all-pages_min_light.png`,
  `..._dark.png`, `sheet-view-all-pages_min_light.png`, `..._dark.png`
- mdview: `view-appearance_*`, `view-reading_*`, `view-tabs_*`, `view-tabs-off_*`, `view-updates_*`
- The fit numbers: `fit-report.txt`

## 4. Layout numbers for the real thing

| | |
|---|---|
| Card | min(window - 12, 780) x min(window - 12, 420), centred, radius 10, no footer |
| Header | 42 px; title x +24; close 28x28 at 12 px from the right; hairline under |
| List | x +12, width 168 / 120 (card under 640), top = header + 4; item 32 / 28 (card under 400 tall); text x +16; marker 3x16 accent at x +3 |
| Divider | 1 px at list right + 5.5 |
| Page area | x = list right + 12 (card +192 default, +144 minimum), to card right - 8; y from the header to card bottom - 2 |
| Page left margin | text at page x + 16 = card +208 default, +160 minimum |
| Control right edge | card right - 26 |
| Page padding | 4 above the first row, 6 below the last |
| Rows | 34 plain; 40 with a one-line help beside the label; 52 (38 + 14 per line) with the help under the row; stacked text field 64, 80 with one help line (66 + 14 per line) |
| Group gap | 8 px, no heading |
| Child | indent 20, 2 px guide at +14 |
| Controls | toggle 40x20; segmented, dropdown, stepper, text and buttons 28 high; inline text field 150 to 240 wide (the room left after its label); stacked field full width; icon button 28x28; text button at least 80 |
| Help line | 11.5 px muted. Beside the label if it fits on one line in the room left of the control; otherwise under the whole row at full width, at most two lines (three under a stacked field). **Never cut with an ellipsis** (a test). A disabled row's reason takes the same slot |
| Disabled | label and value in `pal.faint`, the reason in `pal.muted`, toggle and segmented drawn dimmed; skipped by Tab |
| Fonts | unchanged: 13.5 px labels, 13 px controls, 11.5 px help, 17 px title |

## 5. The fit proof (mockup engine; page area 356 x 304 at 520x360, 580 x 376 at 1280x800)

"Content" is the page's height including its 4 px above and 6 px below; "left" is page area minus
content. The tallest state of each page is used (rule on for Tabs, AI service with a custom
OpenAI-compatible provider, which has the most rows, AI rewrite in its hard state).

| mdedit page | rows | content at 520x360 | left at 520x360 | left at 1280x800 |
|---|---|---|---|---|
| Appearance | 5 | 260 | 44 | 164 |
| Editor | 6 | 266 | 38 | 136 |
| Indentation | 4 | 196 | 108 | 218 |
| Typing | 5 | 244 | 60 | 170 |
| Notes library | 1 | 90 | 214 | 286 |
| Tabs | 7 | 272 (278 with the rule off: two reasons) | 32 (26) | 104 (98) |
| AI service | 5 | 276 | 28 | 126 |
| AI rewrite | 4 to 5 | 188 (248 hard) | 116 (56) | 212 (166) |
| Updates | 2 | 78 | 226 | 298 |

mdview at the minimum: Appearance 284 (**20 left**, the closest of all), Reading 146 (158),
Tabs 170 (134), Updates 78 (226). The list: 9 items need 256 of 304 (mdedit), 4 need 116 (mdview).

**Closest to not fitting:** mdview Appearance (20 px), then mdedit Tabs with the rule off (26),
then AI service (28). **One more row next year:** a plain 34 px row fits only where 34 or more is
left: Appearance, Editor, Indentation, Typing, AI rewrite (all of mdedit but Tabs and AI service);
a 40 px helped row fits Appearance too, a 52 px one only Indentation, Typing and AI rewrite. Neither fits
Tabs, AI service, Editor (38) or mdview Appearance. What happens then is not a scrollbar: the
fit test fails in CI at that commit, naming the page and the pixels over, and the fix is a
split into a second category (the list has one to three spare slots: 10 at the minimum, mdedit
uses 9), or a rewording to one line. That is the contract: **a page is at most about 7 rows, a
list at most 10 items at the minimum**, and the minimum window height is what pays for it:
every 40 px of minimum height is one more row on every page. If rows keep arriving, raising the
minimum from 360 to 400 is the cheapest lever the owner has.

No page fails to fit, and no font was made smaller. What the proof depends on: the cuts under
"Reworded" (measured: with today's Tabs text the page is 358 px at the minimum, 54 over, and the label of the seconds row is cut), and the 6 px margin, 42 px header, no footer
and 8 px gap.

## 6. What it costs, in days

| | days | |
|---|---|---|
| Shared (`mdshared-ui::settings`) | **5** | paged `Form` replacing scroll/pin/jump: page model, header and ×, list states and focus ring 1.75; row layout (help beside/under, stacked field, inline buttons, children, disabled with reason, flexible text width) 1.5; keyboard and pointer model (list stop, Tab order, Ctrl+PageUp/Down, type-to-jump) 0.75; fit-test helper (`Form::page_heights`) and the shot harness 1 |
| Shared, optional | 0.75 | `settings::common` builders for the rows both apps have (window, tabs, updates), so they cannot drift |
| mdedit | **2** | regroup `rows()` into pages (ids unchanged), AI into two pages with disabled rows, the four merged buttons, the rewordings, `open_settings_at(page)`, replace `every_help_line_fits_in_every_layout`, shots |
| mdview | **1** | four pages, tests, shots |
| Windows/Linux/macOS look, both apps | 0.5 | fonts differ per platform; nothing here was checked off Windows |
| **Total** | **8.5** (9.25 with the builders) | |

Budgets: the form is built on open and drawn on demand; it adds no work at start, typing or
scrolling. Not measured, since nothing is built.

### What breaks

mdview (`bins/mdv-app/src/app/tests.rs`):
- `the_settings_dialog_has_categories_and_fits_the_window`: `(520, 360)` is listed as `nav =
  false`, and the list is now always there, so that case flips; its `card.h <= 640` still holds
  (420) and says nothing. It should become the stronger test: for each size, no page's content
  exceeds its page area, no label or help is cut, and every control is inside the card.
- `the_settings_dialog_is_one_page_at_the_default_and_the_minimum_window_size` **no longer exists
  on mdview `main`** (commit `d67a35c` replaced it with the test above when the form left
  `single_page`; HANDOFF and a stale doc comment above the first test still use its name). Its
  old form accepted scrolling at the minimum (`scrolls: true`); the new promise is the opposite.
- `tab_in_the_settings_dialog_visits_every_control_and_then_close`: Tab now walks the list, the
  current page's controls and the ×, not every control of every group; it must loop over pages.
  `focus_on_close()` becomes the × stop.

mdedit: `every_help_line_fits_in_every_layout` (`app/modal.rs`) checks 1280x800, 560x640 and
scale 1.5, never 520x360; it is replaced by the page fit test. `settings_has_an_ai_group_that_asks_for_the_key`
and the library tests that type into Settings (`library_write_tests.rs`) drive by row id: they keep
working only if `focus_row(id)` and `set_value` switch to the page that has the row.
The `Outcome`, `Value` and row-id contract with the host is unchanged.

## 7. Not checked

- Nothing was run as the real feature: no pointer, wheel, hover, keyboard or dropdown; the
  pictures are single frames. The keyboard model is a proposal.
- Shots are at scale 1 only. Scale 1.5 and 2 (the same logical minimum, text measured at the
  larger scale) are in the proposed test, not run.
- Windows only. No Linux (AppImage "Make default" row, no corners row, no backdrop row: all
  fewer or shorter rows than drawn) or macOS fonts.
- The AI service page is drawn in its tallest state (custom provider); the Claude Code / Codex
  state (Program row, Check button, a "Found ... at <path>" help line that can be long) is
  cheaper by count but its long path help was not measured. A long `key_error` is not either.
- mdview's rows are written out from its `settings.rs`, not taken from mdview itself; its
  pictures sit over mdedit's window.
- Light and dark were looked at on a sample of the 72 pictures, the rest by the numbers.
- No `[patch]` and no shared-crate copy were needed; the pinned shared revision is untouched.
