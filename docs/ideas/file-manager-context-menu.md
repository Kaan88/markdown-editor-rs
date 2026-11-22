# The right-click menu: ours first, or one merged menu?

Question 3 of `docs/ideas/file-manager-plan.md`, explained. Pictures are in `shots/mock-fm/` (light and dark exist for each).

## 1. What the question is about

When you right-click a file in Explorer you get two kinds of entries:

- **The shell's own**: Open, Cut, Copy, Delete, Properties.
- **Entries other programs add**: 7-Zip, Git Bash Here, "Scan with Microsoft Defender", Share, cloud clients.

The second kind are not text in a list. Each is a small **DLL that runs inside the program showing the menu** (a COM "context menu handler"). So our file manager can either draw its own menu, or ask Windows for those entries too. There are three ways to do it.

## 2. Option A: ours only

![Our menu](../shots/mock-fm/fm-context-light.png)

The family's menu, drawn by us: fast, themed, knows mdedit and mdview ("Edit in mdedit", "View in mdview"). Nothing from 7-Zip or Git. Cost: 0 days.

## 3. Option B: ours first, Windows' own menu behind "More options" (recommended)

Same menu as above, with a last row **More options… (Shift+F10)**. Choosing it, or Shift+right-click, opens **Windows' own menu** at the pointer, with every third-party entry. About 2 days.

- The common case is instant and always right: Windows loads the third-party DLLs only when you ask.
- Windows 11 Explorer does the same ("Show more options"), for the same reason.
- The Windows menu is native, so in a dark window it may appear light unless we opt in to dark menus (an undocumented call, not yet checked on this PC).

## 4. Option C: one merged menu

![Merged menu](../shots/mock-fm/fm-context-shell-light.png)

Our menu, then a captioned **Windows shell extensions** part (7-Zip, Git, Terminal, Defender, Share) drawn by us. About 7 days (B plus 5).

- **Length.** 21 rows: it no longer fits under the pointer and covers the command strip.
- **Speed.** Every right-click must load every handler first, so one slow handler delays every menu.
- **Work.** We must read each native entry (text, state, icon, submenus such as "Send to" or 7-Zip's) and route clicks back to the right handler. Entries that draw themselves, or fill their submenu on demand, need special cases; a missed one shows an item that does nothing.
- **Look.** One consistent menu, which is the only point in its favour.

## 5. The risk shared by B and C

Handlers run **in our process**. One that hangs freezes the window; one that crashes ends the program (the session file then brings the tabs back). Explorer has the same exposure. With B it can only happen when you ask for More options; with C it can happen on any right-click. A has no such risk.

## 6. Side by side

| | A: ours only | **B: More options** | C: merged |
|---|---|---|---|
| Days | 0 | **2** | 7 |
| 7-Zip, Git, Defender entries | no | **yes, on request** | yes, always |
| Speed of an ordinary right-click | instant | **instant** | waits for the slowest handler |
| Chance a third-party DLL hangs or crashes us | none | **only when asked** | every right-click |
| Menu look | ours | ours, then Windows' | ours throughout |
| Risk of an entry that does nothing | none | none (Windows runs its own) | real |

## 7. Recommendation and when to change it

Take **B**: you keep the fast, themed menu for the 95% case and lose nothing, for 2 days and little risk. If a handler proves too unstable, fall back to A, which only loses third-party entries. Reconsider C only if you use More options daily and want one menu.

**To answer:** B, A or C?
