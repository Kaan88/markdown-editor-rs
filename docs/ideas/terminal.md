# A terminal for this ecosystem

> The owner asked on 2026-11-22 for a standard terminal first, blocks later. The plan built on
> that is `docs/ideas/terminal-plan.md`; this document's verdict on the plain terminal is history.

Written 2026-11-22, at the owner's request, to reopen `ecosystem-ideas.md` §12. That section's
verdict was **Don't**. This document moves it to **possible**, says what changed, and says what
would still have to be decided before any of it is built. Nothing here is a decision to build;
the charter still says "no terminal" for mdedit, and that stands until the owner says otherwise.

## What §12 got right, and what it missed

§12 priced a plain terminal (one tab, ConPTY, a VT grid, scrollback, 7-11 pw, ~20% leverage) and
called it a commodity: Windows Terminal and WezTerm are good and free, and the VT/PTY tail is
long. That is still true of a *plain* terminal, and this document does not argue otherwise.

What it missed is the one version that is not a commodity: **a terminal that knows where each
command starts and ends**, and can therefore treat a command and its output as one thing that this
ecosystem already knows how to handle — something to fold away, search, open in mdedit, or keep
as a note in the library. None of the free terminals do that with a Markdown library beside them.

## What the drawing established

Branch `mock/terminal`, worktree `..\mdedit-term`, drawn by mdedit's own painter behind
`--mock term-<name>`. Twenty shots in `shots\mock-term\` (ten states, both themes), the layout
figures in `shots\mock-term\numbers.txt`, the list in `shots\mock-term\README.md`. A drawing, not
code to merge.

| Shot | Shows |
|---|---|
| `window-*` | Blocks: a test run folded to one line ("312 lines"), `git status` hovered with its actions, a failed build open with its exit code, the prompt |
| `menu-*` | A block's menu, on the failed block |
| `save-*` | Save as a note: the library's places beside the note's preview |
| `find-*` | Find over the grid, the count, and matches hidden inside a folded block |
| `settings-shell-*`, `settings-appearance-*` | Settings in the category design of `docs/archive/settings-redesign.md` |
| `plain-*` | No shell marks: one stream, the offer to install them, every block action greyed |
| `alt-*` | A full-screen program (htop) on the alternate screen: blocks paused, the status says so |
| `ai-*`, `ai-placed-*` | Explain this block, and the AI's suggestion placed at the prompt, not run |

What drawing it taught, which reading about it would not have:

- **Blocks need the shell to say where a command begins and ends** (OSC 133 marks). PowerShell,
  bash, zsh and fish can all emit them, but only once a few lines are added to the profile. So
  `plain-*` — no marks, one stream, nothing to fold — is not an edge case: it is the first run on
  every machine, and the terminal has to be a good plain terminal there or it fails at once.
- **A block costs about 46 px** of chrome (header and spacing) at the default font, on top of its
  output. Short commands are mostly chrome; folding has to be cheap and the default for success.
- **Only failure deserves a mark.** The failed block gets a red tint, a rule down its edge and its
  exit code; a success gets only a small tick and its duration, which reads at a glance.
- **Full-screen programs suspend blocks.** On the alternate screen there is nothing to fold; the
  status strip saying so is enough.

## What it would take

§12's 7-11 pw for the terminal itself still holds, and is the larger part. The blocks are on top:

| Piece | pw |
|---|---|
| The plain terminal (§12: PTY, VT state, grid and scrollback, drawing, selection and keys, tests) | 7-11 |
| Shell marks: reading OSC 133, the block model, the profile lines offered and installed per shell | 1-2 |
| Blocks drawn: fold, actions, the failure mark, find across folded blocks | 1-2 |
| Open in mdedit, save as a note (the library's own writer), copy | 0.5-1 |
| Explain this block through `mdedit-ai`, its suggestion placed at the prompt | 0.5-1 |
| **Total** | **10-17**, an estimate from the drawing, not a plan |

Leverage rises from §12's ~20% only a little: the blocks reuse the painter, the menus, the
settings form, the library's writer and the AI engine, but the grid, the parser and the PTY are as
new as they were.

New dependencies would be a VT parser or terminal state (`vte`, `alacritty_terminal` or
`wezterm-term`) and a PTY crate. Under rule 7 each one is asked for and priced before it is added;
none is priced here.

## What has to be decided first

1. **Where it lives.** The charter says mdedit has no terminal, and "not an IDE" is the reason
   mdedit stayed small. So this is either **a third app** on the shared crates — its own exe, its
   own launch path and budgets, nothing added to mdedit's — or **a written exception** to the
   charter. The drawing used mdedit's binary only because that was the quickest painter to hand;
   it says nothing about where the real one goes. A third app is the reading that keeps every
   existing rule.
2. **Whether the plain terminal is good enough to be used without blocks.** If it is not, nobody
   gets as far as installing the marks. That is most of the cost, and it is the commodity part.
3. **Which shells first.** PowerShell on Windows is the obvious one; WSL and ssh change the PTY
   story and the profile lines.

## Verdict

**Possible**, as a third app, for the block version only. A plain terminal is still a **Don't**,
for §12's reasons. Nothing is planned; this waits for the owner, and when the uncommitted work in
`ecosystem-ideas.md` is settled, §12 there gets a line pointing here.
