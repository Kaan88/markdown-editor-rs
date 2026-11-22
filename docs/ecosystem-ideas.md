# New applications for the mdview / mdedit ecosystem

> The plans written since for the ideas the owner asked about, and where each stands, are in
> `docs/ideas/` (its `README.md` is the index).

Reviewed 2026-11-21. This is an architecture and feasibility review of the twelve ideas in the
owner's list, against the code that exists on this PC.

**What I read.** `markdown-shared-rs` in full at the module level: 34,485 lines of Rust across
`mdshared-app` (9.7k), `mdshared-ui` (15.9k), `mdshared-text` (1.5k), `mdshared-highlight` (4.8k)
and `mdshared-fetch` (2.6k), plus `docs/{PLAN,FEATURES,SISTER,LINUX}.md`. `mdview` (57,133 lines,
399 commits, 2026-11-06 to 2026-11-21): `docs/HANDOFF.md`, `docs/PLAN.md`, `crates/core/src/html.rs`,
`crates/preview/src`, `crates/render/Cargo.toml`. `mdedit` (83,471 lines, 340 commits, 2026-11-12 to
2026-11-21): `docs/{HANDOFF,PLAN,DESIGN}.md`, `bins/mdedit-ai/*`, `crates/mdedit-core/Cargo.toml`
and its module list, `bins/mdedit/Cargo.toml`.

**What I did not do.** No cargo build, no run, no JMAP/NNTP/feed/KDBX/PDF request. External facts
(Stalwart capabilities, crate licences, OCR models) were checked on crate and vendor pages on
2026-11-21 and are marked where they matter; everything else comes from the code. The X G6 CLI the
owner mentions is not in these three repositories, so its reuse is assumed, not checked.

**How the estimates work.** One person-week (pw) is one maintainer-week with AI agents at these
repos' bar: a test that fails without the fix, a hand run on Windows, an audit pass. The pace here
is unusual — mdedit shipped its embedded preview pane (renderer + protocol 3-7 + UI + tests) in
about two days and the whole AI feature (engine, four connections, review UI) in about three — so
these figures price the *unshareable protocol and failure-mode work*, not typing speed. A
subsystem I price at 1-2 pw is one where the happy path is a day and the edge cases are the week.

## Verdict table

| # | Idea | Smallest useful version | Size (pw) | Existing-code leverage | Verdict |
|---|---|---|---|---|---|
| 2 | Mail over JMAP | One Stalwart account: read, thread, search, attach, compose, send | 12-16 | ~30% | **Build next** |
| 2b | JMAP calendar + contacts | Two more views in the same app, no reminders at first | +6-9 | ~20% | Build later |
| 3 | Photo culling | Folder cull: grid + loupe, flags/stars, XMP sidecars | 6-9 | ~20-25% | Build later; better as a file-manager mode |
| 6 | Usenet reader + downloader | Reader only; downloader is a second product | 9-13 reader, 10-14 downloader | ~25% / ~10% | Reader later; **downloader: don't** (built anyway: NewsQueue, §6b) |
| 6b | NewsQueue window on the shared stack | Beside the Win32 window, same in-process engine | 9.5-17.5 (~12) | engine done; frame, settings, menus shared | Costing only; see 6d |
| 6c | Inbound window on the shared stack | Rust client of the C# Host: list, add, progress, tray | 14-22.5 after 6b | Host and protocol done; DTOs by hand | Costing only; see 6d |
| 6d | **One downloader window over both engines** | Rust window, NewsQueue core linked, Inbound Host bundled; list, add, progress, tray | 12-20.5 (~15) | both engines done; frame, settings, menus shared | **Plan: replaces 6b and 6c** |
| 10 | Feed reader | OPML, refresh, list, read state, renderer pane | 3-5 | ~40-45% | **Build next (second)** |
| 8 | Local-LLM workbench | N models on one prompt, side by side, runs saved as .md | 2-3 as a feature | ~85% as a feature | **Only as a feature of mdedit** |
| 14 | Document archive | Text-PDF index + tags + search; OCR later | 10-16 (6-8 text-only) | ~25-30% | Build later |
| 19 | Tasks over Markdown | Today/Upcoming/Board over `- [ ]` lines | 2-3 | ~45% as an app, ~80% as a feature | **Only as a feature** |
| 15 | Hardware panel | Tray + X G6, volume, gamma, DDC/CI | 3-4 | ~30% | Build later |
| 12 | Terminal | One tab, ConPTY, VT grid, scrollback | 7-11 | ~20% | **Don't** |
| 18 | Secrets vault | KDBX 4 read/write, tree, list, copy-clear | 4-6 | ~40% | Build later |
| 21 | Two-pane file manager | Keyboard-first, copy/move/delete, preview | 7-10 | ~55-60% | Build later (or next, if no-network is the priority) |

"Leverage" is the share of total effort (including tests) that exists today and can be reused
without modification. It is deliberately not a line count: `mdshared-ui` is 15.9k lines, but none of
it speaks JMAP.

---

## 2. Mail client over JMAP

### What it really is

A three-pane client for **one Stalwart account**: mailboxes, a paged message list with server-side
search, a conversation view (JMAP gives threads for free), read/unread/flag/archive/delete/move,
attachments in and out, and compose/reply/forward/send through JMAP Submission. Bodies are plain
text first; simple HTML renders through mdview's allow-list subset; remote images stay off until
asked, as in mdview.

It is **not** a Gmail client: no OAuth, no multi-provider quirks, no offline-first cache, no PGP, no
Sieve rules editor, no sharing. The server is the source of truth, so the local cache may be
discarded and rebuilt; drafts are server-side (`$draft` emails), never local-only. That single
decision removes the most dangerous class of mail-client bug.

### What it would take

The work, subsystem by subsystem (the numbers sum to the range above):

| Subsystem | New? | pw |
|---|---|---|
| HTTP: POST, headers, auth, gzip, streaming (EventSource) or polling `Email/changes`, deadlines, caps | Yes, general | 1.5-2 |
| JMAP core: session, per-account capabilities, batched method calls, back-references (`#id`), opaque `state` strings, request limits, errors | Yes | 2-3 |
| Mail model: Mailbox, Email, Thread, Identity, EmailSubmission; `query` + `get` with property selection; keywords; move/copy; drafts | Yes | 2.5-3 |
| MIME: multipart, encoded words, base64/QP, body-part choice, attachment names; blob download/upload | Mostly crate | 1.5-2 |
| Rendering: text; HTML → mdview subset (strip `<style>`/`<script>`, rewrite `cid:` images to cache files); remote-image opt-in | Partly | 1.5-2 |
| Compose: multi-line editor, recipient fields, quoting, drafts, send | **Yes, the weak spot** | 3-5 |
| UI: three panes, virtual message list, thread view, toolbar, keyboard model, unread counts, search bar | Partly | 2.5-3.5 |
| Store/cache (optional; online-only is a valid v1) | Yes | 2-3 |
| Tests: a fake JMAP server, fixtures, state/error cases, MIME corpus | Yes | 2-3 |

Crates: `jmap-client` 0.4 (Apache-2.0 OR MIT; its own documentation lists Core, Mail and WebSocket —
**check Submission, Calendars and Contacts coverage before relying on it**), `mail-parser`
(Stalwart Labs, Apache/MIT, MIME), `serde`/`serde_json` (mdedit already carries `serde_json`),
`flate2` (pure Rust on `miniz_oxide`) for gzip, `keyring` 4.2 (already in `mdedit-ai`),
`url`. On Windows the transport can stay WinHTTP, as `mdshared-fetch` does, so the app gains **no
TLS stack**; off Windows, the system libcurl the fetch crate already loads. The alternative is
`ureq` 3.5 + native-tls, which `mdedit-ai` already uses — but it is inside a binary today, not a
library crate.

Protocol shape that makes this tractable: `Email/query` with `collapseThreads`, `Email/get` with a
property list and `#ids` back-reference to the query result, `Thread/get`, `Email/set` with
`ifInState`, `Blob/upload` for attachments, `EmailSubmission/set` with `onSuccessUpdateEmail` to
move the message to Sent and clear `$draft`. Nothing needs SMTP, IMAP, or a database of its own.

### Leverage from the shared crates

Real, but scaffolding: `mdshared-ui` gives the window chrome and tabs (`chrome.rs`), menus
(`menu.rs`), the command palette (`palette.rs`, 12 rows, fuzzy), the settings form
(`settings.rs`: toggle, choice, number, text, secret, action), the single-line field
(`textfield.rs`), the folder tree model (`tree.rs`, 4,000 rows), the overlay scrollbar (`scroll.rs`),
panes (`pane.rs`), bars (`notice.rs`), the painter/theme (`gfx.rs`, `theme.rs`), key parsing
(`keys.rs`), file dialogs, clipboard and links (`platform.rs`), window placement (`place.rs`).
`mdshared-app` gives settings (`settings.rs`), paths (`paths.rs`), recents/places
(`recents.rs`, `places.rs`), single instance and the folder lock (`single.rs`, `folder_lock.rs`),
file watching (`watch.rs`), fuzzy matching (`fuzzy.rs`), crash reports (`crash.rs`) and the whole
updater (`update.rs`, `updater.rs`, `selfupdate.rs`) — `updater::App` already takes `name`,
`manifest_url`, `engine` and `extras`, so a new app is one const and one manifest, not a project.

What is **not** there: a virtualized list with columns and flags, a multi-line text editor, any
HTTP beyond image GETs, any storage, and any mail semantics. `mdshared-fetch` is explicitly a
hardened image fetcher: GET only, `Accept: image/*`, no cookies, no credentials, no decompression,
only a 200 counts, 12 MiB cap, 64 MiB cache, four workers that exit after 10 s idle
(`crates/mdshared-fetch/src/lib.rs`). The owner's line "JMAP is plain JSON over HTTPS, so
mdshared-fetch covers the transport" does not hold: it is the right *policy* template and the
wrong API. A JMAP client needs POST, headers, auth, gzip and a stream.

**New shared pieces this implies:** a transport crate, a small store, a list widget, and a
multi-line field (or a way to reuse mdedit's). See the overall section for boundaries. Build them
inside the mail app first; move each to `markdown-shared-rs` when the second app needs it, which is
the precedent the owner's own rule records.

### Effect on the other apps

If the transport/store/list live in **new crates**, `mdview` and `mdedit` link nothing new: Rust
drops unused code, and the modules start no threads at construction (the fetcher starts work on the
first request, the notes indexer on the first folder — the right precedent to copy). The costs that
remain are real but indirect: two more git pins per app if they ever adopt them, more build time,
and a faster-moving shared repository whose `rev` the two shipped apps must chase for security
fixes.

The dangerous coupling is the renderer. Mail wants HTML fidelity; mdview is feature-complete and
its HTML is an allow-list with no CSS (`crates/core/src/html.rs`: `<style>` becomes literal text,
`<head>`/`<template>` are dropped, `<div>`/tables/`<details>` are mapped, nothing is styled). If
mail ships by pinning `mdview-render` (as mdedit pins `mdview-preview`), no mdview release is
needed; if mail instead asks mdview for a CSS engine, the maintenance promise is broken. Pin, do
not modify. Newsletters will look like 2003; that is the honest ceiling.

A mail client also changes the ecosystem's network posture: "no network except self-update and
explicitly enabled AI" becomes "no network except configured servers". Keeping it a separate
binary is what preserves that sentence for mdview and mdedit.

### Risks and the thing that kills it

The **composer**. The ecosystem has exactly one multi-line editor, and it is mdedit: `docs.rs`
(5,733 lines), `view/` (1,702), `mdedit-core` (~26k lines: buffer, edit, find, diff, table,
markdown). The sister protocol is one-way for text — mdedit sends the viewer its document; the
viewer never sends text back. Handing compose off to mdedit would need a new bidirectional request
and a "return on Ctrl+Enter" flow, which is fragile across process lifetimes. Embedding
`mdedit-core` (buffer only) is possible, but its view and layout are in the mdedit binary, not a
library. The realistic path is a **new shared multi-line field**: plain text, one style, selection,
undo, IME, wrap — 2-3 pw, and it will still be a worse writing surface than mdedit. A mail client
whose compose is worse than the editor next to it is the failure mode the owner will notice daily.

Second: HTML mail. The mdview subset is good for GitHub READMEs and bad for marketing mail;
`cid:` images, inline styles and table layouts are the bulk of real mail. Plain-text-first is the
mitigation, and the UI must say when a message has an HTML part that is not shown.

Third: protocol edges. `maxObjectsInGet`, `maxBodyValueBytes`, state strings that must not be
compared, `ifInState` conflicts on drafts, and Stalwart's limits are all real; `Email/get` on text
bodies is where naive clients blow the memory budget.

Licence/protocol traps: `jmap-client` may not cover Submission; calendars/contacts are draft or
newer (next section). No TLS crate is needed if WinHTTP/libcurl is used. Stalwart supports OAuth
(verified), but the MVP should use a server API key or basic auth and skip OAuth entirely.

### Verdict

**Build next.** It is the only idea here that is a product rather than a utility, the protocol is
the simplest serious mail protocol that exists, and one Stalwart account removes most of the cost.
Scope it to read/send/search with plain-text-first and no calendar in v1.

---

## 2b. JMAP calendar and contacts

### What it really is

Two more views over the same session, store and account: a month/week/day calendar with an event
editor and RSVP, and an address-book list with a contact detail/edit form. Not a scheduling
product: no free/busy across accounts, no resource booking, no CalDAV, no CardDAV.

### What it would take

- **Calendar, 4-6 pw.** Month/week/day grids (new drawing, ~1.5-2 pw with hit-testing and drag to
  create/move/resize), event editor (times, all-day, time zone, recurrence, attendees, reminders),
  JSCalendar JSON handling, query with `expand` over a bounded window (Stalwart limits it, 365 days
  by default). Time zones are the trap: IANA ids, DST arithmetic, and a tz database. `jiff` or
  `chrono-tz` brings data (roughly 1-2 MB in the binary) that must be updated; mapping IANA to
  Windows zones by hand is worse.
- **Contacts, 2-3 pw.** RFC 9610 over JSContact (RFC 9553): nested JSON, not vCard. The list is
  easy; the editor needs repeatable groups (several emails, phones, addresses), which the shared
  settings form does not have (`settings.rs` rows are fixed controls, no "list of rows"). Add a
  small repeatable-field widget or ship a fixed subset (name, emails, phones, org, notes).
- Crates: `jmap-client` may not cover either (check); serde types are easy to hand-write;
  `icalendar` only if CalDAV/ICS import is wanted, which it is not for v1.

### Leverage

Same shell as mail (~20%): chrome, menus, palette, settings form, text field, tree, list (new,
shared with mail), `place.rs`, `keys.rs`. The domain (JSCalendar/JSContact, grids, recurrence) is
new.

### Effect on the other apps

One app, three views is the right shape: it reuses mail's session, auth, transport and store and
adds no new process. If instead it becomes a second app, it duplicates the account model and the
sync loop for no gain. It does add a `jiff`/tz dependency to the mail binary; that is ~1-2 MB and
worth it.

### Risks and the thing that kills it

**Reminders when the app is closed.** A calendar that cannot notify is a calendar the owner will
not trust. That means a login item/tray presence and Windows toast notifications, which for an
unpackaged Rust app need an AUMID and a Start-menu shortcut (or a packaged identity). This is a
1-2 pw platform feature the ecosystem has never built, and it also drags in "what happens when the
mail app is closed" for mail notifications. Recurrence and DST are the second trap. The calendars
spec is still a draft; Stalwart's own announcement names Mailtemi, Parula and OpenCloud as the first
client implementations, so expect churn.

### Verdict

**Build later.** After mail has been the daily client for a while, and either ship without reminders
(say so in the UI) or budget the tray/toast work as part of the decision.

---

## 3. Photo culling and metadata tool

### What it really is

The brief is a stub ("You have Lightroom, Capture"), so the only defensible reading is a **fast
pre-cull front-end**: point it at a shoot folder, see a thumbnail grid and a loupe, flag/reject and
star with the keyboard, filter, read EXIF, write ratings to XMP sidecars that Lightroom/Capture One
pick up, and hand the keepers to the real editor. Not a raw developer: no demosaic, no lens
corrections, no colour management, no catalog.

### What it would take

6-9 pw: folder scan and thumbnail pipeline on a worker (2-3), grid + loupe + selection + keyboard
(2), EXIF and orientation (0.5-1), XMP sidecar read/write (0.5-1), filters/sort (0.5), settings and
persistence (0.5), tests/fixtures (1). Formats: JPEG (`zune-jpeg` is already in `mdview-render`),
PNG (`png` 0.18 is already a dependency of `mdshared-text`), WebP, GIF; TIFF needs `tiff`; HEIC
needs `libheif` (C++, LGPL) or a system codec; RAW previews
need `rawler`/`rawloader` (pure Rust, patchy coverage) or the embedded JPEG that almost every raw
file carries (the cheap, correct route).

Colour is the hard part. There is no ICC handling anywhere in the ecosystem; `kamadak-exif` reads
EXIF but not profiles, and `lcms2` is C. For sRGB JPEGs, ignoring colour management is fine. For
AdobeRGB/ProPhoto raws, the cull will show wrong colours — which is exactly the thing a culler must
not do. The honest scope is "sRGB JPEGs and embedded raw previews, colours approximate".

### Leverage

~20-25%: chrome, menus, settings, fuzzy, watch, `files.rs` for stable reads, `places`/`recents`,
`gfx::Painter` (but only 1:1 `blit` — there is no scaled image draw in `gfx.rs`), and the decoders
in `mdview-render/src/image.rs` **which are not shared**. A new `mdshared-image` crate (decode hooks
+ scaled draw + orientation + LRU thumbnail cache) would be created here, and the file manager would
use it too.

### Effect on the other apps

A photo app is the cheapest way to force a real image crate into shared code, and mdview would
benefit later (better scaled images, orientation, thumbnail cache) without linking new decoders —
but only if decoders live in a separate crate from the drawing code. `mdview.exe` was 8.4 MB at
the last figure in its HANDOFF;
TIFF+RAW+HEIC decoders could add 1-3 MB to whichever binary links them, so keep them opt-in.

### Risks and the thing that kills it

**Colour accuracy and raw previews.** A culler that shows the wrong colours is worse than Explorer,
and the ecosystem has no colour-management answer. Missing from the brief: which cameras and
formats, whether flags go into XMP sidecars or the raw file, whether there is a database or just
folders, compare mode (Photo Mechanic's two-up pick), video files, and whether it must hand off to
Lightroom or Capture One. Without those decisions this is a 6-9 pw bet on a guess.

### Verdict

**Build later**, and preferably as a **mode of the file manager** (image folder → grid + loupe +
flags), not a separate app. As an app it duplicates the list, selection, file operations and
thumbnail cache the file manager needs anyway.

---

## 6. Usenet reader and downloader

### What it really is

Two products that share a folder icon. **Reader:** subscribe to groups, fetch headers by date range,
thread by `References`/`Message-ID`, read articles (text and a modest HTML subset), score and
killfile, keep read/unread locally (NNTP has no flags). **Downloader:** take an NZB, fetch segments
over several connections, yEnc-decode with CRC32, assemble, verify/repair with par2, unpack, queue
and retry. The reader is text-heavy and fits the stack; the downloader is a background transfer
engine with a completely different failure model.

### What it would take

Reader 9-13 pw: NNTP client (AUTHINFO, GROUP, XOVER/XHDR, ARTICLE, COMPRESS DEFLATE, TLS via the
system), 1.5-2; header store with bounded per-group cache, 2-3; threading (JWZ-style, missing
parents, crossposts), 1-1.5; scoring/killfile rules, 1-1.5; reader UI (group tree, article list,
article pane through the renderer), 2-3; composer (again), 1-2; tests against a recorded server,
1-2.

Downloader 10-14 pw: NZB parsing, 0.5; multi-connection scheduler with failover and backoff, 1.5-2;
yEnc + CRC32, 0.5-1; assembly and file naming, 1; par2 verify/repair (Reed-Solomon over GF(2^16),
recovery blocks), 2-4; unpack (zip/7z pure Rust; RAR is C++ `unrar` or nothing), 1-2; queue UI and
progress, 2-3; post-processing and retries, 1-2.

Crates: `nntp-rs`/`brokaw` exist but are young and the async ones bring Tokio, which this ecosystem
does not use anywhere; a blocking NNTP client of ~1,000 lines is likely the better fit. `quick-xml`
for NZB, `flate2`, a yEnc decoder (small, own), `rust-par2` (pure Rust, **maturity unverified**),
`sevenz-rust`, `zip`, `unrar` (C++, and the unRAR licence is not OSI). TLS: system, as above.

### Leverage

Reader ~25%: chrome/menu/palette/settings/tree, the store/list/page if mail built them, `fuzzy.rs`,
`watch.rs` (cache files). Downloader ~10%: settings and progress UI basically; the transfer engine,
par2 and unpack are new and unshareable.

**The owner's claim, tested.** "Build mail first and this gets cheap" is false for the protocol:
JMAP hands the client thread ids, flags and server-side search; NNTP hands it a line protocol, no
state, and headers that must be spooled by date. It is half true for the *UI*: rows, read state,
offline store, article rendering and a composer are shared, maybe 25% of the reader. The downloader
shares nothing with mail.

### Effect on the other apps

The reader would justify the store and list crates, and reuse the renderer for articles. The
downloader would add a second long-running worker model (connections, queue, retries) that is
useful to nothing else in the ecosystem and a par2/RAR dependency tree that ages badly. It also
grows the app from 9-13 pw to 18-24, and the maintenance surface from "a reader" to "a download
client that must not corrupt files".

### Risks and the thing that kills it

**Header volume.** A naive "download the group's headers" design dies on `alt.binaries.*` (millions
of articles). The reader must fetch by date range or from NZBs, which changes the UI (no complete
local overview) and the store. Second: par2 repair correctness — a wrong recovery implementation
silently corrupts downloads. Third: RAR. The downloader half is a solved commodity (SABnzbd,
NZBGet) with decade-old edge cases; rebuilding it is a poor use of one maintainer. Content
legality is the user's business; the app needs no index of its own.

### Verdict

**Reader: build later** (after the store/list exist; it is the best test of whether those
abstractions generalise beyond mail). **Downloader: don't.**

---

## 6b. A window for NewsQueue on the shared stack

Reviewed 2026-11-22, from a shallow clone of `Kaan88/newsqueue` at `0fc1db6` (release 1.0.10), and
`markdown-shared-rs` at `a2a2031`. No build, no run, nothing of the installed copy touched. §6's
"downloader: don't" priced *building* a downloader; NewsQueue is built, so the question is only
its window.

### What it really is

A fourth front end on an engine that already takes several. `newsqueue-core` (~10.7k lines with
its tests: download, par2, unrar, rss, model) has no UI and no Win32. A front end sends a
`Command` (39 of them: queue edits, pause, speed, settings, servers, RSS) and reads a secret-free
`Snapshot` through an `EngineHandle`; the engine ticks on its own thread and reports five
`Event`s (toast, message, notify, password needed, feeds needed). The Win32 desktop, the headless
server and the egui web UI already sit on that boundary. A winit + softbuffer window would be a
new Rust binary that **links the engine directly**, in process, with no IPC: the same shape as
mdedit owning its document and asking `mdedit-ai` for the network.

It is the same threading model as ours (std threads, blocking sockets, no async), and par2 and RAR
are child processes (`par2.exe`, `UnRAR.exe`), so the window inherits none of §6's correctness
risk.

### What the window has to show

From `app/src` (desktop, ~8.8k lines) and its 21 golden shots:

| Surface | In NewsQueue | In the shared crates | Gap |
|---|---|---|---|
| Frame, menu bar, toolbar, status bar | Win32 | `chrome`, `menu_bar`, `menu`, `keys`, `platform`, `theme`, `frost` | copy a loop (1-3 pw, §"The shell gap") |
| Queue: collections that expand into files, 15+ columns, progress bars, reorder, header drag | owner-data ListView | `rows` (one height, no columns), `tree` (folders only) | **a table**: columns, resize, expand, progress cells |
| Connections, PAR2, Log, RSS lists | four ListViews | same | the same table |
| Tabs (Queue, PAR2, Log, RSS) | Tab control | document tabs in `chrome` | small |
| Setup: 15 pages of fields | `FIELDS` table | `settings` (header, toggle, choice, number, text, action) | servers editor, scheduler grid |
| ~17 dialogs (speed, pause for, rename, password, shutdown, notes, stats, columns, RSS feeds...) | `dialogs.rs` | `overlay`, `textfield`, `notice`, `settings` | each one drawn |
| Statistics charts, speed graph | GDI | nothing | a chart, small |
| Tray icon, balloons, close-to-tray | `tray.rs` | nothing | **a tray**, per platform |
| Single instance forwarding NZBs, self-update, crash log | own | `single`, `updater`/`selfupdate`, `crash` | wire up |
| File and folder pickers | `IFileOpenDialog` | `rfd` already in `mdshared-ui` | none |

### What it would take

| Piece | pw |
|---|---|
| Copy the event loop from mdview (frame, damage, timers, dropped files) | 1-3 |
| Table widget: columns, resize, sort, expandable rows, progress cells, range selection, virtualised | 2.5-4 |
| Main window: tabs, toolbars, status bar, splitter, keyboard | 1-2 |
| Setup on `settings` rows, plus servers editor and scheduler grid | 1.5-2.5 |
| The dialogs | 1.5-2.5 |
| Charts | 0.5-1 |
| Tray (Windows first; Linux and macOS each later) | 0.5-1 |
| Shots in both themes, tests, budgets measured | 1-1.5 |
| **Total** | **9.5-17.5**, about 12 likely |

The table and the tray are the expensive part, and they are exactly what §"New shared crates" says
mail would need (`mdshared-ui::list` + `table`). NewsQueue would be their **first user**, ahead of
mail, with simpler rows (no variable heights, no HTML).

### What it gains

- **One look with mdview and mdedit**, and **dark mode**: the Win32 desktop is light only.
- **A Linux and macOS desktop.** The core builds and tests on Debian already; today Linux gets only
  the server and the browser. (macOS: not checked.)
- **The budgets.** First frame under 100 ms and private memory under 30 MiB become the window's
  rules. Not measured: the Win32 window is likely already light, so this is not a speed win on
  Windows, and should not be sold as one.
- **The table and the tray, built once** for the apps after it.

### What it costs, besides the weeks

- **Two desktop windows to keep, or a replacement.** The Win32 Compact window is approved and
  released. The new window can run beside it on the same engine until it reaches parity; then one
  goes. Keeping both forever doubles every UI change.
- **The house rules come with it.** `panic = "abort"` means auditing the engine's outside-input
  paths first (NZB XML, NNTP replies, yEnc, RSS, UnRAR output; ~40 `unwrap`/`expect` in
  `download.rs` alone), or one bad article kills the app mid-download. A `ci` workflow (it has only
  `docker-release.yml`). Shots in both themes.
- **The binary count.** It is the third winit app. §"Recommendation" says extract the frame before
  a fifth; a third copy of the loop is allowed but should be made knowingly.
- **Dependencies.** The engine brings `ureq`, `native-tls`, `quick-xml`, `chrono`, `fs2`,
  `serde_json` into a winit binary. They are NewsQueue's own and already shipped; none enter mdview
  or mdedit.

### Verdict

**Possible, and the better of the two download-manager windows to build first** (§6c): Rust to
Rust, in process, a bounded surface, and it pays for the table and the tray. About 12 pw. Build it
as a new binary beside the Win32 window, not over it, and retire one only at parity.

---

## 6c. A window for Inbound on the shared stack

Reviewed 2026-11-22, from a shallow clone of `Kaan88/inbound` at `8c8fa86` (release 0.9.0). No
build, no run.

### What it really is

Inbound is a C#/.NET 10 download manager: HTTP with segments and resume, queues and scheduler,
torrents (a libtorrent child process), TorBox, archives, Remote files over rclone (SFTP, FTP,
WebDAV, S3, Drive, OneDrive, Dropbox), browser extensions for Chrome, Edge and Firefox. About 145k
lines of C#; the WinUI window is 59.7k of them, `Runtime` 34.5k.

**The engine is a separate process.** One per-user Host owns the engine, the store, the scheduler
and the tray; every window is a client. The WinUI window, an Uno build for Linux and macOS, the
CLI, the native-messaging host and a Svelte web UI already are. The repository even has
`docs/new-frontend-guide.md`, written for exactly this: a new GUI in any toolkit.

So a winit + softbuffer window is a **Rust client of a C# Host**:

- Transport: a per-user named pipe `\\.\pipe\DownloadManager-<hash>`, owner-checked; each frame
  a 4-byte little-endian length and UTF-8 JSON, 8 MB at most. The Linux Host and the server have
  their own transports. A blocking pipe client in Rust is small.
- Protocol: version 1, 105 message types, `server.hello` with capabilities, then `jobs.snapshot`,
  then events. The DTOs (`DownloadManager.Contracts`, 8.2k lines) are mirrored by hand in Rust;
  nothing is generated.
- Rules: which action applies to which state lives in C# rule classes in the WinUI project
  (`FileMenuRules`, `SelectionCommands`, ...). The guide says copy them, do not invent. In Rust
  that is a port, with their tests.

### What the window has to show

The guide's own checklist, which a window must meet or list as `partial`:

- the list (8 columns, sortable, resizable, packages that expand, state views and queues in a
  sidebar, search, multiselect, bulk actions);
- adding (URL check, batch URLs, clipboard watch, duplicates policy, site logins);
- progress details with a segment map and a speed graph;
- queues, scheduler, quotas, completion actions;
- Options, many pages, one OK/Cancel/Apply;
- tray, notifications, updater, shell open/reveal;
- browser extension status;
- TorBox, containers and archives, torrents (file choice);
- **Remote files**: remotes, sign-in questions, paged browsing, two panes, transfers;
- managing another Inbound server.

### What it would take

| Piece | pw |
|---|---|
| Pipe client, framing, hello/capabilities, reconnect | 0.5-1 |
| Mirror the Contracts DTOs (105 messages) | 1.5-2.5 |
| Port the rule classes and their tests | 1-2 |
| Loop, table, tray | from §6b if built first; else 4-8 |
| Main list, sidebar, packages, search, bulk actions | 2-3 |
| Add flows, details with segment map, graph | 1.5-2.5 |
| Queues, scheduler, Options | 2-3 |
| Torrents, TorBox, archives | 1.5-2.5 |
| Remote files (two panes, transfers, sign-ins) | 2.5-4 (the file manager, §21, would share it) |
| Shots, tests against an isolated Host and `tools/TestServer` | 1.5-2 |
| **Total** | **14-22.5 after §6b**, **18-30.5 without it** |

### What it gains

- **The lightest Inbound window.** The install is about 200 MB (its own figure; how much is WinUI and
  how much .NET was not checked); a
  winit client is a few MB. Its launch and memory would meet our budgets. Not measured: what
  WinUI costs today is a guess until someone times it.
- **One look across the three**, and a native Linux and macOS window without Uno.

What it does not gain: the Host stays .NET and keeps running; the total memory of "Inbound
running" falls only by the window's share.

### What it costs

- **Two languages, one protocol, by hand.** Every new message or field in C# needs a Rust mirror,
  or the Rust window falls behind (the guide's rule: hide what you cannot do). This is the
  permanent cost, and it grows with every Inbound release.
- **The rule classes drift.** Ported rules are a second copy; a fix in C# must be made in Rust too.
- **A third window to keep**, beside WinUI and Uno, on a product that changes weekly (Remote files
  landed in the last week).
- **A JSON dependency in a winit app.** `serde` + `serde_json` (or a hand parser). Rule 7: ask, and
  price it.

### Verdict

**Possible, because the Host was built for it, but expensive and never finished**: about 18 pw
after §6b, and a protocol to follow forever. Build it only after the NewsQueue window, reusing its
table and tray, and start with a **smaller window** the guide allows as `partial`: the list,
adding, progress, pause/resume, tray. Leave Remote files, torrents and TorBox in the WinUI window
until the list is daily.

---

## 6d. One downloader window over both engines (the owner's direction)

Added 2026-11-22. The owner: keep NewsQueue and Inbound as they are, and build one Rust window on
the shared stack for both; the combined app will probably **ship the two engines itself**. This
section replaces 6b and 6c as the plan; they stay as the costing of each half. Same evidence as
6b and 6c: read, not built, not run, not measured.

### What it is

One app, one window, one tray, for every download: URLs and torrents through Inbound's engine,
NZBs through NewsQueue's. NewsQueue and Inbound stay released apps with their own windows; the
combined app is a third product that carries both engines inside its install.

| | NewsQueue's engine | Inbound's engine |
|---|---|---|
| Language | Rust (`newsqueue-core`) | C#/.NET 10 (the Host, with `Runtime`, `Engine`, `Persistence`...) |
| How the app carries it | linked into the exe, in process | the Host's self-contained publish shipped beside the exe, started by the app |
| How the window talks to it | `EngineHandle`: `Command` in, `Snapshot` out, no IPC | the named pipe: 4-byte length + JSON, protocol 1 |
| Its external tools | `par2`, `UnRAR` (vendored) | `inbound-torrent.exe` (libtorrent), 7-Zip, rclone |

### The one hard question: whose data

Both engines are single writers of their state folder, and the standalone apps may be running.

- **NewsQueue** keeps its state beside the exe (`app_base()`, or `NQ_CONFIG`) and has **no lock** on
  that folder: two engines on one folder corrupt the queue. The combined app needs its own folder,
  or a lock added to the core first (a lock file with `fs2`, which the core already depends on).
- **Inbound** has one Host per user, held by a mutex and a per-user pipe. A second Host with the
  same instance name refuses to start; `DOWNLOADMANAGER_INSTANCE` and `DOWNLOADMANAGER_DATA` give a
  separate one.

Two ways. **Open: the owner left this undecided (2026-11-22).** It is decided before any code:

1. **Own engines, own data.** The combined app runs its own Inbound instance and its own NewsQueue
   folder. No clash, ever. Servers, logins and queues are not shared with the standalone apps; the
   user moves to the combined app rather than using both.
2. **One engine per user, whoever starts it.** For Inbound, connect to the Host if one answers
   on the pipe, else start the bundled one: the Host already supports this (the WinUI window, the
   CLI and the extension all do it). For NewsQueue, the same needs the core to become a host too
   (a lock, and the combined app attaching through `newsqueue-server`'s API when the desktop owns
   the folder). More work, and it ties the combined app's release to both apps'.

1 needs no change in either repository except the NewsQueue lock as a guard; 2 lets the owner keep
using the standalone apps on the same queues. Either way, the NewsQueue lock goes in first.

### The smallest useful version

- One list over both engines: name, engine, size, progress, speed, state, time left; filters
  (Active, Queued, Failed, Done) across both; multiselect; keyboard.
- One Add: a URL or magnet goes to Inbound, an `.nzb` (file, drop or the shell's Open with) to
  NewsQueue.
- Pause, resume, remove, retry; pause all; speed limit per engine.
- A details pane: Inbound's segment map, NewsQueue's files, PAR2 and unpack stage.
- Tray, notifications, single instance, self-update.
- Settings: the servers, logins and folders each engine needs to run, on `mdshared-ui::settings`
  rows. Nothing else at first.

Left out, and said so in the window: Inbound's Remote files, TorBox, the scheduler, the browser
extension pairing; NewsQueue's RSS and statistics. They stay in the standalone apps until the list
is daily.

### What it would take

| Piece | pw |
|---|---|
| Copy the event loop from mdview | 1-3 |
| Table widget (the one §6b prices) | 2.5-4 |
| Tray | 0.5-1 |
| NewsQueue side: link the core, map `Snapshot` rows, commands, its settings pages | 1-1.5 |
| Inbound side: pipe client, reconnect, the DTOs the small version uses (not all 105), the state rules it needs, ported with their tests | 2-3 |
| Start, watch and stop the bundled Host; its instance and data folder | 0.5-1 |
| One list over two engines: a common row, states mapped, add routing | 1-1.5 |
| Details pane, both kinds | 1-1.5 |
| Install: the Rust exe, the self-contained Host, the vendored tools; update of all three | 1-2 |
| Shots in both themes, tests against an isolated Host, `tools/TestServer` and NewsQueue's mock NNTP server | 1.5-2 |
| **Total** | **12-20.5**, about 15 likely |

The table and the tray are built once here and are the ones mail would use next.

### What it costs, besides the weeks

- **The install is no longer light.** A self-contained .NET Host is tens of MB on disk (Inbound's
  own install is about 200 MB with WinUI; the Host alone was not measured). The window can meet
  the budgets; the app as a whole cannot claim "under 30 MiB" while the Host runs. Say so in
  PLAN.md's terms: the budgets are the window's.
- **Two engines to keep in step.** Inbound's DTOs are mirrored by hand; NewsQueue's `Snapshot` is
  a Rust type and can be used directly. Every release of either engine is a release decision for
  the combined app: pin a version of each, as the shared crates are pinned.
- **Two languages in one install**, with .NET's own updates and CVEs to follow.
- **The house rules.** `panic = "abort"` in the combined exe means NewsQueue's engine runs under it:
  audit its outside-input paths first (~40 `unwrap`/`expect` in `download.rs`). The Inbound Host is
  a separate process, so its crash ends a download, not the window: the window shows the Host
  gone and restarts it.
- **The binary count.** The third winit app. Allowed by §"Recommendation"; extract the frame before
  a fifth.

### Verdict

**Plan it as the ecosystem's downloader**, the smallest version above, about 15 pw, with the data
model still open. It replaces building separate windows for NewsQueue (§6b) and Inbound (§6c). Before code:
the owner picks the data model (1 or 2), the NewsQueue lock goes into its core, and the bundled
Host's size is measured.

---

## 10. Feed reader (RSS/Atom/JSON Feed)

### What it really is

An OPML-driven reader for a bounded set of feeds: fetch with conditional GET (ETag/Last-Modified),
parse RSS 0.x/1/2, Atom and JSON Feed, store items keyed by guid/link, list by feed and folder, mark
read/starred, and render the item's content through mdview's page (HTML subset). Refresh on a timer
while the window runs; no background service.

### What it would take

3-5 pw: transport GET with gzip and conditional headers (1; already needed by mail), feed parsing
and normalisation (0.5-1, `feed-rs` handles the formats), store and dedupe (1), list + reading pane
(1-1.5), OPML import/export and add-by-URL with `<link rel="alternate">` discovery (0.5), refresh
scheduler with backoff and tests against a local server (0.5-1). Crates: `feed-rs` (MIT,
maintained, RSS/Atom/JSON Feed with iTunes/Media extensions), `quick-xml`, `flate2`, `encoding_rs`
(already in `mdedit-core`), `url`. No TLS crate if the transport is WinHTTP/libcurl.

### Leverage

~40-45%, the best ratio of the network ideas: chrome, menu, palette, settings, list (new but shared
with mail), text field, store (new), transport (new), and — the big one — **mdview's renderer**.
`mdview-preview`'s `server` feature is already embedded in mdedit: measured +1.3 MB exe, +1 MiB
memory, first frame unchanged, keystroke-to-pane 8.4-8.8 ms (mdedit's HANDOFF). A feed reader gets
the same page, selection, links and scrolling without writing a renderer.

### Effect on the other apps

Feeds is the natural **second user** of the transport, store and list, which is exactly the
condition the owner's own rule sets for moving code into `markdown-shared-rs`. It would not need
mdview changes if it pins the renderer. It does add a network client to the ecosystem, which is why
it should be its own binary.

### Risks and the thing that kills it

Feed reality: malformed XML, wrong encodings, relative URLs, duplicate guids, updated items,
paywalls, JS-only sites, and feeds that return 500 for a week. `feed-rs` absorbs much of it. The
subtle one is **refresh policy vs. the zero-idle-CPU budget**: a 30-60 minute timer with exponential
backoff is acceptable; polling every feed every minute is not, and neither is a hidden background
process. Also decide the full-text question up front: summary-only feeds will send the reader to the
browser, and a readability extractor is another 1-2 pw with its own fidelity problems.

### Verdict

**Build next, second.** It is the cheapest real app here and the best way to prove the transport,
store and list on something with no credentials, no data loss and no product expectations — but
only after mail's version of those pieces exists, so they are shaped by the harder client.

---

## 8. Local-LLM workbench

### What it really is

In mdedit: take the current document (or a prompt), run it against N models at once — local
OpenAI-compatible servers and the Claude API — stream the answers side by side, and save the run as
a Markdown file with the model, parameters, timing and the outputs. Saved prompts already exist
(`ai-prompts.md`), the review/apply flow exists, and the rendered preview exists.

### What it would take

2-3 pw as an mdedit feature: a compare panel (two to four columns, each a streaming answer), a run
record written to `.md`, and small protocol additions to `mdedit-ai` — token counts and timing
(`proto.rs` sends `delta(id, chars)` today, no usage), and per-run parameters. Parallel runs already
work: `main.rs` keeps a `Jobs` map keyed by request id and spawns a thread per run. No new crates.
As a separate app it is 6-8 pw and duplicates the editor, the preview and the prompt file for no
gain.

### Leverage

~85% as a feature: `bins/mdedit/src/ai.rs` (client/state, 545), `ai_ui.rs` (drawing, 706),
`app/ai_events.rs` (1,220), the preview pane (642 + 1,223), `mdedit-ai`'s four connections
(`api.rs`, `cli.rs`, `sse.rs`, `keys.rs`), `ai-prompts.md` parsing, and `mdedit-core::ai` for the
provider list. An app would reuse the same engine but re-pay the shell, file handling, session and
updater.

### Effect on the other apps

None beyond mdedit. No shared crate changes; the protocol addition is private to mdedit and its
engine.

### Risks and the thing that kills it

**Local model speed.** Side-by-side comparison is only useful with models fast enough to stream
together; on one GPU, two 30B models do not. That is a UX constraint, not a code one. A workbench
that hides token counts and cost is not a workbench; add them. Nothing here is risky to the
ecosystem.

### Verdict

**Only as a feature of mdedit.** It is the highest-leverage idea in the list and does not deserve a
fourth binary.

---

## 14. Personal document archive

### What it really is

A folder watcher plus an ingestion pipeline: for every PDF (and later image), extract text, OCR it
if it has no text layer, pull the facts the owner cares about (vendor, device, dates, amounts),
tag it, and make it searchable. The "ask my documents" feature is retrieval (top chunks by keyword)
plus one AI call, not a chat product. Not a DMS: no versioning, no workflows, no sharing, no
scanning UI.

### What it would take

10-16 pw, and the split matters:

- **Text PDFs only, 6-8 pw:** PDF text extraction (`pdf-extract`/`lopdf`, pure Rust, MIT; layout
  quality is mediocre for columns and tables), a metadata store and index (2-3), folder watcher and
  incremental reindex (1), tag/date extraction rules plus an AI pass (1-2), search and list UI
  (1-2), open-in-system-viewer for the PDF itself (mdview cannot render PDFs).
- **Scanned PDFs, +4-8 pw:** rasterise pages (pdfium, C++/BSD, ~10 MB DLL; or `hayro`, pure Rust
  but explicitly experimental/WIP) and OCR (`ocrs` + `rten`, pure Rust, MIT/Apache code — **the
  models are CC-BY-SA-4.0**, 10-40 MB, Latin alphabet only, early-preview quality; or Tesseract,
  C++/Apache). MuPDF and Ghostscript are AGPL and out. A 2,000-document, 20-page archive at 0.5-2 s per page is 6-22 hours of first-pass
  CPU; the pipeline must be resumable and off the launch path.
- **Search:** SQLite FTS5 (C, bundled, excellent, but a new class of dependency for this ecosystem)
  or `tantivy` (pure Rust, heavy build, large index) or a hand-rolled inverted index (2-3 pw, no
  stemming, no ranking beyond BM25). This is the first idea that forces a real database decision.
- **AI answers:** chunk the text, retrieve top-k, send to `mdedit-ai` (the editor caps a run at
  200 KB; the engine itself takes a prompt, so the archive must do its own chunking and context
  budgeting).

### Leverage

~25-30%: chrome, menus, settings, list (new), `watch.rs`, the `notes`/`foldersearch` patterns
(background walker, bounded reads, cancellation), `mdedit-ai` for extraction and Q&A, `fuzzy.rs`,
`files.rs`. The PDF, OCR, index and extraction rules are all new.

### Effect on the other apps

It would force the image/PDF crates and a database into the ecosystem, and those are exactly the
dependencies that should not leak into mdview/mdedit. Keep them in the archive binary. It would also
be the first app whose useful state is large (an index over thousands of documents), so the
startup/memory budgets need a different treatment: the index on disk, a bounded working set, and a
first frame that does not wait for it.

### Risks and the thing that kills it

**The promise.** "When does the Philips warranty end?" requires (a) the text, which for scans means
a WIP rasteriser plus Latin-only OCR with unknown Turkish accuracy, (b) the date, which invoices
express in a dozen formats, and (c) retrieval that finds the right two pages among thousands. Each
is a project; together they are the hardest idea in the list after mail, with the least certain
payoff. The PDF rasterisation choice is the crux: pdfium is C++ (against the house rules, and a DLL
to ship), hayro is the pure-Rust bet but not production-grade yet.

### Verdict

**Build later**, text-only first, and do not promise answers over scans until OCR has been measured
on the owner's actual documents.

---

## 19. Tasks and projects over Markdown

> Proposed 2026-11-23 as the quickest first release, as a Tasks view in mdview:
> `docs/ideas/first-release-plan.md`.

### What it really is

A lens over task lines that already exist in `.md` files: `- [ ]` with a documented metadata
convention (due date, priority, project, recurrence), views for Today, Upcoming, By file and a
board grouped by heading or tag, and toggling that writes one byte back to the file. No database,
no sync, no server.

### What it would take

2-3 pw: a scanner over a folder (reuse `notes.rs`'s walk and `wikilink`-style line scanning, or
`foldersearch` with a task matcher), date/tag parsing (0.5), the views (1-1.5; Today/Upcoming are
lists, the board is columns of cards), write-back (0.5; mdview's `crates/core/src/tasks.rs` already
does byte-exact toggling with a scan-match guard), file watching (reuse). No new crates.

### Leverage

~45% as an app, ~80% as a feature: `mdshared-app/src/notes.rs` (index over a folder), `library.rs`
(lists of notes), `foldersearch.rs` (background scan with events), `fuzzy.rs`, `watch.rs`,
`places.rs`, plus mdview's task-box drawing and `tasks::toggle`.

### Effect on the other apps

As a feature of mdview (a "Tasks" pane/view) it adds no binary risk beyond the scanner, and mdview
already parses and ticks boxes. As a feature of mdedit it is a side panel over the open folder. As
an app it is a whole binary, installer, updater and release cadence for a view over text files.

### Risks and the thing that kills it

**Concurrent edits.** The write-back must refuse when the file changed under the scan, exactly as
mdview's `tasks.rs` does, or a tick can clobber a paragraph. **Recurrence and reminders**: a task
with a due date that never notifies is a list, not a task manager; notifications when the app is
closed are the same tray/toast problem as the calendar. **The convention**: if due dates live in
prose, parsing is guesswork; if they live in a strict syntax, the owner must write it — decide and
document it.

### Verdict

**Only as a feature**, in mdview and/or mdedit. It does not justify an app; the owner's own
description ("a lens over files you already write, not a new database") is the right scope.

---

## 15. Sound card / hardware control panel

### What it really is

A small tray app with a panel: the X G6 settings the owner's CLI already exposes, the default
output device's volume/mute, gamma, monitor brightness/contrast over DDC/CI, autostart and a few
global hotkeys. It replaces vendor software, which is the actual value.

### What it would take

3-4 pw: HID control of the device (reuse the CLI's protocol; **its source is not in these three
repos**, so this assumes a Rust CLI with a documented command/response format), 1-1.5; WASAPI
default-device volume/mute and device switching (`windows-sys`, `Win32_Media_Audio`, verbose COM
but no C), 1; gamma (`SetDeviceGammaRamp`, GDI already in `mdshared-ui`), 0.3; DDC/CI via
`dxva2.dll` (a system DLL) or the `ddc` crate, 0.5-1; tray icon and menu (`Shell_NotifyIconW`,
`Win32_UI_Shell`; winit can own a hidden window), 0.5-1; hotkeys (`RegisterHotKey`), autostart
(`HKCU\...\Run`), settings UI, 0.5-1. Crates: `windows-sys` only, if HID is done through SetupAPI +
`hid.dll`; `tray-icon` is an option but pulls its own windowing assumptions.

### Leverage

~30%: painter, theme, settings form, `place.rs` for the panel window, `single.rs` if one instance,
`crash.rs`, updater. The panel's controls (sliders, meters) are new; the tray lifecycle is new and
does not fit the "one window per settings folder" model.

### Effect on the other apps

Almost none, and that is the problem: it advances no shared code and adds another installer,
manifest and release. It would benefit from a shared `gfx` slider/knob widget, which nothing else
needs.

### Risks and the thing that kills it

**Device-specific reality.** If the CLI speaks a vendor protocol that changes with firmware, the app
is a support burden; if the CLI is not Rust or is not where the owner thinks, the estimate doubles.
WASAPI COM inside windows-sys is doable but slow to get right, and a volume slider that lags is
worse than the tray flyout Windows already has. DDC/CI is flaky across monitors and cables.

### Verdict

**Build later.** It is small, self-contained and genuinely replaces bloat, but it is a utility, not
an ecosystem app, and it should not delay mail.

---

## 12. Terminal emulator

> Reopened 2026-11-22: the verdict below is history. `docs/ideas/terminal.md` says why a terminal
> is possible after all, and `docs/ideas/terminal-plan.md` is the plan, Windows first.

### What it really is

The brief is a stub. A minimal version: one window, one tab, ConPTY on Windows (PowerShell/cmd,
optionally WSL/ssh), a VT parser, a cell grid with 256/truecolour, scrollback, selection and copy,
resize, cursor and a monospace font. Not a multiplexer, not a GPU terminal.

### What it would take

7-11 pw: PTY/ConPTY plumbing and process lifecycle (1-2), VT parser and terminal state machine
(2-4 from scratch; `vte`/`alacritty_terminal`/`wezterm-term` provide it, Apache/MIT, but their APIs
are shaped for their own apps), grid and scrollback storage (1), rendering (1-2: box drawing, wide
chars, emoji width, underline/italic, cursor, damage), selection/copy/paste and key encoding (1),
tabs/settings (0.5-1), tests with recorded escape sequences (1-2). Latency is fine on a cell blit,
but ConPTY adds ~5-15 ms before the app sees bytes.

### Leverage

~20%: `TextEngine`/`Bitmap` for glyphs, `gfx.rs`, `chrome.rs`, `keys.rs`, `settings.rs`. The grid,
the parser, the PTY and the terminal semantics are new, and the shared list/table abstractions do
not help at all (a terminal is a fixed-cell grid, not rows).

### Effect on the other apps

None. It would add `alacritty_terminal` or `wezterm-term` plus a PTY crate to the ecosystem, and a
second kind of text rendering that does not share mdview's layout.

### Risks and the thing that kills it

**It is a commodity.** Windows Terminal and WezTerm are good, fast and free; the ecosystem adds no
advantage here, and the VT/PTY correctness tail is long (mouse reporting, bracketed paste, OSC 8,
sixel/kitty graphics, alt-screen, reflow on resize, IME). The only interesting version is a terminal
**pane inside mdedit**, and mdedit's plan explicitly says "not an IDE… no terminal". Missing from
the brief: which shells (cmd/PowerShell/WSL/ssh), tabs/splits, scrollback size, keybindings or
leader key, mouse reporting, graphics protocols, font fallback, and whether it must be a pane
rather than an app.

### Verdict

**Don't.**

---

## 18. Secrets vault

### What it really is

A KDBX 4 vault: unlock with a password (and optionally a key file), group tree, entry list and
search, entry detail, copy password with an auto-clear timer, a password generator, atomic save,
lock on idle. Not a browser extension, not an auto-type agent, not a team vault.

### What it would take

4-6 pw: KDBX read/write through `keepass` 0.8 (MIT; its KDBX4 **writing** is described as
experimental) or `keepass-rs` 0.2 (MIT OR MulanPSL-2.0, a newer crate that claims full KDBX 3.1/4.0
write; maturity unverified) — 1-1.5 including interop tests; entry/group model and editing 1;
tree/list/detail UI 1-1.5; search and generator 0.5; clipboard auto-clear 0.3; lock/idle/minimize
and session hygiene 0.5-1; a compatibility test matrix against real KeePassXC and KeePass 2.x
databases 1. No C: Argon2, AES-KDF, ChaCha20 and AES come from the crate's RustCrypto dependencies.

### Leverage

~40%: `textfield.rs` has a real secret mode (dots, never copied as text out of the field),
`tree.rs` for groups, `palette.rs` for search, `settings.rs` for the form, `platform.rs` clipboard,
`crash.rs` (text-only reports — **verified: no minidump, no memory**, so a crash cannot dump the
master key), `updater`, `single.rs`, `folder_lock`. The KDBX model, crypto and interop are new.

### Effect on the other apps

A shared `secrets` crate would be the useful outcome: keyring slots, an override variable, redaction
and zeroizing strings — exactly the logic now buried in `bins/mdedit-ai/src/keys.rs` (service
"mdedit", SHA-256 slot names for custom URLs, an empty-slot rule). But **do not** make the vault the
key source for mail or the AI engine. That turns a password manager into a service with an unlock
protocol, breaks the property that AI keys never leave `mdedit-ai`, and adds a startup dependency to
every app. The OS credential store stays the place; the vault optionally edits the same store.

### Risks and the thing that kills it

**Write compatibility.** A vault that writes a KDBX variant another client rejects loses trust
immediately; KDBX 4.1, attachments, history entries, custom icons, recycle bin and HMAC block
streams all matter. Argon2 parameters can demand 1 GiB and seconds of CPU — respect them, cap them,
show progress. Security review is beyond what this ecosystem has done so far (the audits are
functional, not adversarial), and there is no memory locking (`VirtualLock`) or protection against
the pagefile; say so in the README rather than imply more than is true.

### Verdict

**Build later.** Bounded, daily-useful, and the best fit for the widget set after the file manager;
but it is security software, and it should start read-mostly with a strict interop test suite.

---

## 21. Two-pane file manager

> The owner asked (2026-11-22) for a standard file manager first, with split view, and a commander
> layout only later: `docs/ideas/file-manager-plan.md`, its mock-up and questions, and
> `docs/ideas/commander-plugins.md`.

### What it really is

A keyboard-first dual-pane manager: two independent folder panes with tabs eventually, a virtual
list with name/size/date, type-ahead and fuzzy filter, multi-select, copy/move/delete-to-recycle-bin
with progress, cancel and conflict dialogs, batch rename (regex), new folder, path bar, drives and
quick places, a preview pane that uses mdview's renderer for text/Markdown/images, and session
restore of both panes. Not an Explorer replacement: no shell extensions, no thumbnails for every
format, no cloud integration, no ACL editor.

### What it would take

7-10 pw: virtual list with columns, selection, keyboard and inline rename (2-3; shared with mail),
pane model, history, sort/filter and path bar (1-1.5), file operations with a worker, progress,
cancel and conflict handling (2-3), recycle bin and "open with" through the shell (0.5-1), batch
rename (0.5), preview embedding (0.5-1), session/places/drives (1), tests on a scratch tree with
junctions, long paths and read-only files (1-2). Crates: none new if file operations are
`std::fs` + `windows-sys` (`SHFileOperationW`/`IFileOperation` for the recycle bin, `MoveFileExW`
for no-replace semantics as `bins/mdedit/src/docs.rs`'s `move_no_replace` already does); `rfd` for dialogs
(already in `mdshared-ui`).

### Leverage

~55-60%, the highest of any idea: `tree.rs` (lazy, 4,000 rows, hidden/target/node_modules rules),
`fuzzy.rs`, `watch.rs` (live folders), `places.rs`/`recents.rs`, `settings.rs`, `single.rs`,
`folder_lock.rs`, `palette.rs` (quick jump), `chrome.rs`, `menu.rs`, `notice.rs` (progress/error
bars), `platform.rs` (open, reveal, clipboard), `gfx.rs`, and the renderer for preview. Add the new
list and image crates and most of the app is assembly.

### Effect on the other apps

This is the idea that most justifies the list/image shared crates, and the one most likely to make
mdview's renderer a library dependency of a third binary (already proven by mdedit: +1.3 MB, +1
MiB). It forces a shared **internal drag-and-drop** layer (pane to pane, row to row) that the tab
strip and future boards/photos also want. It does not need any network.

### Risks and the thing that kills it

**Data loss.** Copy/move/delete semantics are where file managers die: same-volume rename vs
cross-volume copy, junctions and symlinks, read-only and ACLs, long paths (`\\?\`), network shares
that block, OneDrive placeholders (opening triggers a download; deleting can remove cloud data),
free-space checks, partial copies, and an undo story. A single lost folder ends the project's
credibility with its only user. Second: the missing list/drag layer means this app is 2-3 pw more
expensive today than its reuse percentage suggests.

### Verdict

**Build later** — or **next if the owner wants a no-network daily tool**: it has the best reuse,
the least legal/protocol risk, and it makes the ecosystem's widgets earn their keep. But it should
come after the list and drag layers exist, and it needs a slow, paranoid file-operations core with
tests on real Windows volumes.

---

# Overall

## Dependency order: what unlocks what

The author's claim — mail unlocks Usenet and feeds — holds for the **shared infrastructure** (list,
store, page rendering) and not for the protocols. As a cost argument it is weak: it is a 20-25%
discount on the reader, not "cheap", and no discount at all on the downloader.

| Claim | Verdict |
|---|---|
| Mail → Usenet "gets cheap" | **False for the protocol, ~25% for the reader UI.** JMAP supplies thread ids, flags and server-side search; NNTP supplies a line protocol and no state. The downloader shares nothing with mail. |
| Mail → feeds | **False in protocol, true in infrastructure.** Feeds need GET + conditional headers + a store + a list + a page; mail needs all of those plus POST/auth/streaming. Building mail first means feeds is 2-4 pw of new code; building feeds first means the HTTP layer is shaped around GET and then extended. |
| Mail → calendar/contacts | **True**, and the only real dependency in the list: same account, session, auth, transport and store. Calendars additionally need a notification mechanism nothing has. |
| Photos → archive | Weak: shared image decode, orientation and thumbnails, but the archive's own work is PDF/OCR/index. |
| File manager → photo culling | **True**: culling is a mode (grid + loupe + flags) over the file manager's selection, file operations and thumbnail cache. |
| Vault → mail/AI keys | **False, and should stay false.** A shared `secrets` crate (keyring wrapper) is the dependency; the vault as a key server is a design regression. |
| Anything → terminal | Nothing. |
| Anything → tasks/workbench | mdview/mdedit already have the pieces; these are features, not dependencies. |

The real order is: **transport + store + list** (built inside mail) → **feeds** (cheap second user)
→ **Usenet reader** (reuses list/store/page, new NNTP) → **Usenet downloader** (nothing shared;
don't). Calendar/contacts ride on mail. Vault, file manager, archive and hardware are independent
branches off the same shell.

## The three strongest and the three weakest

**Strongest for this specific ecosystem**

1. **Mail over JMAP (12-16 pw).** The only product here, the protocol is JSON, one Stalwart account
   removes OAuth and provider quirks, and the failure mode is known (compose and HTML). It also
   produces the transport/store/list that everything else wants.
2. **Feed reader (3-5 pw).** The cheapest real app, no credentials, no data loss, and it exercises
   the renderer, the list, the store and the transport with a forgiving protocol. The best second
   user for the code mail creates.
3. **Two-pane file manager (7-10 pw, 55-60% reuse).** No network, no protocol, the highest reuse in
   the list, and daily value. The risk is data loss, not architecture.

**Weakest**

1. **Terminal.** A commodity with a long correctness tail and no ecosystem leverage; mdedit's own
   plan excludes it.
2. **Photo culling as briefed.** No colour management, no raw development, no stated formats or
   sidecar policy; the honest version is a file-manager mode.
3. **Document archive.** The promise depends on PDF rasterisation (C++ or a WIP pure-Rust
   renderer), Latin-only OCR with unknown Turkish quality, and retrieval quality — three projects
   behind one feature.

Runner-up weakness: the Usenet **downloader**, which is a second product with par2/RAR correctness
and a decade of commodity competition.

## Which ideas are really features

- **Local-LLM workbench** → an mdedit feature (2-3 pw; ~85% reuse).
- **Tasks over Markdown** → an mdview and/or mdedit feature (2-3 pw; ~80% reuse), not an app.
- **Calendar and contacts** → **views of the mail app**, not separate apps. One binary, three views,
  one account model.
- **Photo culling** → a mode of the file manager, if both are built.
- **Archive search** is arguably a file-manager feature; its **ingestion pipeline** (watch, OCR,
  extract, index) is app-scale. Do not merge them until the file manager exists.

## New shared crates the strongest path implies

Build each inside the first app that needs it; move it to `markdown-shared-rs` when the second app
does. Boundaries are the whole point — a shared crate that grows a scheduler, a schema and a policy
engine becomes a framework, and this ecosystem cannot afford to maintain one.

| Crate | Holds | Must not hold |
|---|---|---|
| `mdshared-net` | One blocking request: method, URL, headers, body bytes or a stream callback, per-call deadline, size cap, redirect policy, gzip, optional cookie jar; WinHTTP on Windows, system libcurl elsewhere (the `mdshared-fetch` routes, generalised). | Auth storage, retries/backoff policy, scheduling, caching, any protocol (JMAP/feed/NNTP), global singletons. Caller owns the thread. |
| `mdshared-store` | A small embedded record store: collections keyed by id, get/put/delete/range scan, atomic batch commit (temp + fsync + rename, as `settings::write_atomic`), an opaque per-collection `state` string, CRC per record, bounded compaction, one folder per account, one writer. | SQL, joins, cross-collection transactions, migrations beyond a version byte, full-text search (that is a separate index). |
| `mdshared-ui::list` (+`table`) | Virtualized rows: count, height (fixed or measured), draw; columns, elide, align; single/range/check selection; keyboard, hover, wheel; scrollbar hookup; optional drag-reorder hook. | A data model, async loading, a filesystem, mail/feed semantics. The app supplies rows and a "loading" placeholder. |
| `mdshared-ui::drag` | Drag source/target, ghost, autoscroll, drop payload type. | Shell drag-and-drop of files (that is winit's dropped-file event plus the app). |
| `mdshared-secrets` | Keyring slots, an override variable, error policy, redaction, zeroizing strings — `mdedit-ai/src/keys.rs` generalised. | A vault, an unlock protocol, an IPC service. |
| `mdshared-image` | Decode hooks, orientation, scaled draw (bilinear/area-average), LRU thumbnail cache with a byte cap. Decoders in a **separate** crate so mdview does not link TIFF/RAW/HEIC. | A colour-management engine (unless `lcms2`/C is accepted), a RAW developer. |
| `mdshared-shell` (the old PLAN step 4) | Regions, damage, hit testing, overlays, timers, IME preedit plumbing, dropped files. | Menus, commands, app state. |

**The shell gap is the elephant.** `markdown-shared-rs`'s plan put the window frame at step 4 and
deferred it; each app still owns its loop (`mdview`'s `app.rs` ~1.2k lines, `mdedit`'s `app.rs` 4.8k
plus ~15k in `app/`). The estimates in this report assume a new app **copies** an existing loop, as
mdedit did: 1-3 pw, plus drift between the copies. Designing one from scratch is 3-6 pw, and every
copy re-earns the bugs those files already fixed (hidden first frame, damage regions, timers, IME
preedit, dropped files). Before a **fifth** app, extract the frame; the fourth should copy knowingly.

## Where the hand-drawn rule gets expensive

| Area | Why it costs | Estimate | What exists today |
|---|---|---|---|
| Long scrollable lists (10k+ rows) | Virtualization, variable row heights, selection ranges, keyboard, type-ahead, sticky headers, 1-2 ms per frame | 1.5-3 pw for a good one | `palette.rs` (12 rows, fuzzy), `tree.rs` (4,000 rows, folders), no general list |
| Tables with columns | Widths, resizing, sorting, elide, alignment, horizontal scroll | +1 pw | mdview's table layout (`layout/table.rs`) is page rendering, not a widget |
| Drag and drop | Internal drag (pane↔pane, card↔column, tab reorder), ghost, autoscroll, drop targets | 1-1.5 pw | Tab drag is per app (`chrome.rs` has geometry only); file drops are winit events |
| Image decoding and display | Orientation, scaled decode, thumbnail cache, (colour management) | 1.5-3 pw (+2-4 for ICC) | `png`/`zune-jpeg`/`gif`/`image-webp` in `mdview-render`, unscaled `gfx::blit` |
| Multi-line text editing | Buffer, undo, selection, wrap, IME, caret, find | 4-8 pw | mdedit only (~26k lines core + 7.4k view/docs), not shared |
| Many fields / repeatable groups | Recipients chips, several emails/phones, date pickers, validation, focus traversal | 1-2 pw | `settings.rs` rows (toggle/choice/number/text/secret/action), single-line `textfield.rs` |
| Calendar grid | Hit-testing, drag create/move, month/week/day, DST | 1.5-2.5 pw | nothing |
| Accessibility | Screen readers, focus, IME candidate placement | unplanned | none anywhere (set aside by the owner) |

These numbers are why mail is 12-16 pw and not 6: the domain is half, the surface is the other half.

## Recommendation

1. **Build mail next, one Stalwart account, read/send/search, plain text first.** Put the transport,
   store and list inside the mail app as its own crates; do not pre-share them. Pin the renderer
   rather than change mdview. Budget 12-16 pw, and treat compose as the gating risk: if a 2-3 pw
   multi-line field cannot reach "pleasant", stop and reconsider before the rest of the app is
   built.
2. **The downloader (§6d, ~15 pw)**: one window on the shared stack over NewsQueue's and Inbound's
   engines, both shipped inside it. It builds the table and the tray mail needs, so it may go
   before mail if the owner wants a daily app sooner; decide the data model first.
3. **Then feeds (3-5 pw)**, which turns those crates into shared ones with two real users. The Usenet
   reader (§6) is dropped: the owner does not want one (2026-11-22).
4. **Count the binaries.** mdview and mdedit are two; the downloader and mail are the third and
   fourth, in whichever order; feeds would be the fifth. Do not start a fifth before the window frame is shared, or the maintenance multiplier (installers,
   manifests, AppImages, audits, three pin updates per shared fix) will eat the gains.
5. **File manager and vault are the best "later" bets** (55-60% and ~40% reuse, no network, bounded
   protocols). Calendar/contacts come after mail is daily, and only with a decision about
   notifications when the app is closed.
6. **Tasks and the LLM workbench are features**, not apps. Terminal and the archive are the ones to
   leave alone until something changes (a pure-Rust PDF renderer good enough for OCR, or a real
   need for a terminal pane in mdedit).

**Stop rule.** Ship mail when a week of real mail has been read and sent through it without opening
Thunderbird. Until then, do not add calendar, contacts, or a fourth binary.
