# The AI on the web: a plan

Written 2026-11-24 for the owner. Nothing in it is decided or built. It changes D-015 (only the
text a command runs on leaves the machine, only to the provider chosen), so it needs the owner's
yes and a decision that names D-015 before any code.

## What it is for

The owner (2026-11-24): "get further data, like get playtimes from howlongtobeat for these games
kinda stuff." A list in a document, and the AI filling in what the list does not say, from the web.

## The shape: a setting, then a question each run

Not a switch on each command. Two layers:

1. **Settings › AI › Web access**: `Off` (the default) or `Ask me each time`. With `Off` nothing
   changes from today: the model is not told the web exists, and no request carries a web tool.
   Under the row, in the same words as the Connection row: who receives what (the provider, and
   the search provider behind it).
2. **During a run, the AI asks; you answer.** When the setting is on, the model is told it may ask
   for the web. If it asks, the run stops before any search and a card says what it wants:

   > **The AI wants to use the web**
   > Why: to look up playtimes for the 14 games in the list.
   > Searches: about 14, on howlongtobeat.com
   > Reads pages: no
   > The searches go to Anthropic and its search provider; they carry the game names.
   >
   > [Allow for this run]  [Run without the web]  [Cancel]

   "Allow" is for this run only. There is no "always allow".

The model cannot be paused in the middle of a request (a provider's web tools run on the
provider's side), so "the AI asks" is **two requests**:

- **Request 1, no web tools.** The usual request, plus one rule: if the task needs facts from the
  web, answer only with a small JSON request (`why`, `searches` as a count, `sites`, `read_pages`)
  instead of the result. If it does not need the web, it answers as today and nothing else
  happens. Nothing from the web is in this request, so a web page cannot be what asks.
- **Request 2, only after Allow**, the same text again with the web tools on and the **limits the
  card showed, enforced by the provider, not by the model**: a search count (`max_uses`) and the
  sites (`allowed_domains`). It answers with the result as today.

The JSON contract (the ask and its reader) lives in `mdedit-core`, read with the same depth-bounded
reader as the other AI answers, every field optional, no path that can panic (rule 11). The engine
only learns a `web` field on `run` (search on, pages on, count, sites) and reports what was searched
(D-055: the engine's protocol stays free of task words).

## What the review shows

A run that used the web always opens the review, whatever "Show changes before applying" says:
a page can steer an answer, and nothing from it goes into the document without a look. Over the
review, a band: **Searched** (each query the model really sent, taken from the answer, not from what
it promised) and **Sources** (the pages it used, as links). The result itself carries no citation
marks; the system prompt asks for none and the engine drops the citation blocks from the text.

## Per connection

| Connection | Request 2 | Count enforced | Sites enforced | Phase |
|---|---|---|---|---|
| Claude API | the server tools `web_search_20260209` (and `web_fetch_20260209` when pages are allowed) | yes, `max_uses` | yes, `allowed_domains` | 1 |
| Claude Code | `--tools "WebSearch"` (plus `WebFetch`), `--allowedTools "WebFetch(domain:…)"`, `--strict-mcp-config`; today it runs with `--tools ""` (`bins/mdedit-ai/src/cli.rs:316`) | no (a turn cap only) | for pages yes, for search no | 1 |
| Codex | `--search`, sandbox stays read-only | no | no | 2 |
| OpenAI-compatible | none: no common way across providers | — | — | not planned |

Where a limit is not enforced, the card says so ("any site", "no limit on searches").
A connection with no web support is never told the web exists; its runs are as today.

Engine work for the Claude API: the new answer blocks (`server_tool_use`, `web_search_tool_result`,
`web_fetch_tool_result`) are read and skipped or collected, never put in the text; a search error
arrives as a 200 with an error object inside, not as an HTTP error; a long search turn can end with
`pause_turn` and must be continued; the 3-minute "nothing received" limit must not fire while
searches run. All on the stream thread that exists (`sse::read`).

## Search only first; reading pages later

Phase 1 is search only. Reading pages is how a document's text gets out to a site of the model's
choosing, and how a hostile page gets the most room. On the Claude API a page can be read only if
its address is already in the conversation, which narrows that, but phase 1 leaves it off anyway.
For the HowLongToBeat case the search results' text may already hold the hours; step 0 finds out.

## Step 0: try it before building it

Before any window code: a throwaway in `spikes/web-search/`, 20 games, the Claude API, search only,
`allowed_domains: ["howlongtobeat.com"]`. Record how many came back right (checked by hand against
the site), how many searches, the cost and the time. HowLongToBeat has no public API and resists
scraping; if search results do not carry the hours, the feature needs pages read (phase 2) or a
different source, and the owner decides with those numbers. Its terms of use should be read too.

## Where it applies

- Phase 1: the rewrite commands on the open document (Rewrite with instruction…, saved prompts).
  "Add a column with the main-story hours from HowLongToBeat" is a rewrite of a table.
- Phase 2: the task window's "Rewrite them" (one card for the whole run, not per file).
- Never: "File them", "Split them", the folder sort (the model chooses structure there, not facts).

## Risks, and what answers each

| Risk | Answer |
|---|---|
| A web page steers the result | Review forced on; Searched and Sources shown; search only in phase 1 |
| The document's text leaves for a site | Search only; sites limited where the provider can limit them; the card says what the queries carry |
| The document itself asks for the web (an injected line) | The card shows the sites and the count; the owner decides; Allow is per run |
| Cost runs away on a long list | A hard cap of 50 searches a run; the card shows the count before anything is spent |
| Claude Code picks up the owner's own tools or MCP servers | `--tools` names exactly what is allowed, `--strict-mcp-config`; a test pins the arguments |
| Wrong numbers that look right | Sources beside the review; the plan says so plainly in the card's help line |

## Budgets

Nothing on the launch path: the setting is one row read with the others, the engine starts on the
first AI command as today. Measured anyway before and after (rule 1).

## Tests and shots

- Tests: the ask's reader (a good ask, garbage, an ask with no count, an answer that is both);
  request 1 has no web tools with the setting on or off; request 2 carries exactly the card's
  count and sites; the Claude Code arguments for off, search, and pages; the stream reader skips
  the server-tool blocks and collects the queries; `pause_turn` continues; Off sends what it sends
  today, byte for byte.
- Shots (`tests/shots.sh`, both themes): the Settings row off and on; the ask card (search only,
  with pages, a connection that cannot limit sites); the review with the Searched and Sources band.

## Decisions it needs

1. Web access exists, off by default, asked per run, and amends D-015.
2. Search only in phase 1; reading pages waits for step 0's numbers.
3. Which connections: the Claude API and Claude Code first.
4. The cap: 50 searches a run.
5. Whether a saved prompt may carry its own ask ("this prompt uses the web, on these sites"), so
   the card comes before request 1 and the extra request is saved. Not in phase 1.

## Size

Step 0: half a day. Phase 1 (setting, two-step run, card, review band, Claude API and Claude Code,
tests and shots): about three days at this project's bar.
