Unreleased: menus fade in as they open, and a setting to turn motion off. (Renamed to the version's own file when it is cut.)

## New

- **Menus fade in.** Menus, context menus and submenus fade in and settle into place as they
  open, in about a ninth of a second; closing is instant, and the word completion list appears at
  once as before.
- **Animations, in Settings › Appearance.** On by default. Off, nothing moves. With the system's
  animation effects off (Windows: Accessibility › Visual effects), nothing moves either.

## The AI task window

- **A folder added to the window is walked whole.** Files in its subfolders, at any depth, are
  found and listed after the folder's own; a dot file or folder, `target` and `node_modules` are
  left out, and one add stops at 2,000 files, saying so in the window. The folder is read on its
  own thread — the window says "Reading <folder>…" meanwhile, and Esc stops it — and the cap
  counts only the files the window and a question hold not already. A folder of more than 200 new
  files is counted first and asked about before anything is read ("Add 1,240 files from
  Recipes?"); Add them reads and adds them, Leave them out reads none, and a folder that brings
  nothing new says so. A folder that cannot be read is named in the window, and so is one the walk
  stopped short of. A file added again is not one left out, and the plan waits while a question's
  files are still coming in.
- **Settings opened from the window comes to the front.** "Open AI settings" hides the AI task
  window until Settings closes, then brings it back as it was, with the connection, the limits
  and the saved prompts read again.
- **Esc no longer throws the setup away.** In a field with words in it, Esc leaves the field;
  then the window says once what closing would drop, and the next Esc closes it. Typing again
  goes back into the instruction. Cancel still closes at once.
- **Back on a rewrite's review and on a split's plan.** Back returns to the window with nothing
  written and the file still ticked.
- **The window reopens on the task it was last on**, and grows with a large window.
- **Its messages are in the window**, over the foot of its list, not in a toast far from it.
- **Write notes' Files… lists pictures.**
- **"Show what is sent"** says whether anything was sent yet, and closing its tab (Close, Close
  others or Close all) brings the window back.
- **Write notes after a failed request** offers to go on only when there is something to go on
  with, so it can no longer get stuck, and "One note per picture instead" no longer changes How
  many or unticks the text file. The plan card counts the requests answered apart from those that
  failed.
- **Clearer words.** The button says what it does and how many files ("Make the plan for 4
  files", "Rewrite 1 file"), and is off, with the reason under it, when there is no file or no
  group. Files never sent are no longer called "not placed". The Options popover is drawn whole and
  on top, "Show what is sent" no longer overlaps Cancel, and the window reads whole at 520 px.
