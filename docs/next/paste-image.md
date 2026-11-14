# paste-image

## README

Feature list: **Paste an image.** Ctrl+V in a saved Markdown file with an image on the clipboard (and no
text) saves it as a PNG beside the file (`notes-image-1.png`) and inserts `![](notes-image-1.png)`.
Windows: clipboard PNG or bitmap. Linux: needs `wl-paste` (Wayland) or `xclip` (X11).
The shortcut table gains no row: it is Paste (Ctrl+V, Shift+Insert).

## DESIGN

- Paste reads text first. Only when there is none (empty or not text) is the clipboard asked for an image.
- Only for a document with a file whose language is Markdown, or an unsaved document. Other files: paste does
  nothing, as before.
- Unsaved document (and an image on the clipboard): toast "Save the document as a Markdown file first, then
  paste the image"; nothing else happens.
- The file is `<document name without extension>-image-<n>.png` in the document's folder, the first n from 1
  that does not exist (created exclusively, so nothing is overwritten).
- The inserted text is `![](name)` with a forward slash path and these characters written %XX: space, `%`,
  `(`, `)`, `[`, `]`, `<`, `>`, `#`, `?`, `"`, control characters. Other characters (accented letters) stay.
- It goes in as an ordinary paste: one undo step (with several carets it is inserted at each). The file stays
  after undo; the toast says "Image saved as <name>".
- Read-only or lossy documents: the paste is refused as for text and the new file is removed again.
- Image data over 64 MB, or not a PNG on Linux, is ignored. Write errors: toast "Could not save the image: ...".
- Windows: the registered "PNG" clipboard format (keeps transparency) first, else CF_DIB (24 and 32 bit,
  alpha dropped) converted with the `png` crate. Linux: `wl-paste --type image/png` or
  `xclip -selection clipboard -t image/png -o` (Wayland session tries wl-paste first), run only on such a
  paste, stopped after 3 seconds.
- Drops are not touched: dropping a file opens it today.

## Release note

Paste an image into a saved Markdown file: Ctrl+V saves it beside the file as a PNG and inserts the link.

## Handoff

Built: `bins/mdedit/src/image_paste.rs` (names, link encoding, DIB to PNG, unit tests), `platform::clipboard_png`
(Windows API; Linux via wl-paste/xclip subprocess), `paste_image` in `app/commands.rs`. New direct dependency
`png = "0.18"` in bins/mdedit (already in the build through mdshared-text; Cargo.lock only gains the edge).
No shared change.

Decisions: no arboard image feature (it pulls the `image` crate); subprocess on Linux keeps the dependency tree
unchanged and needs no PNG encoder since the tools return PNG. No setting for a subfolder: Settings has no
Markdown group. Name uses a counter, not a timestamp. Drops left alone (they open files). Budgets: nothing runs
at startup or on keystrokes; the image path runs only on a paste with no text on the clipboard.

Not verified: nothing run locally (CI only). Windows clipboard code only compiles in Windows CI; never tried
with a real clipboard (browser image, Snipping Tool). Linux tools never tried on real Wayland/X11. No app-level
test (the clipboard cannot be faked) and no screenshot: the only new visible state is a toast, which --shot
clears.

Left out: pasting a file list of images as links; image paste without wl-paste/xclip on Linux; subfolder setting;
macOS (not a target).
