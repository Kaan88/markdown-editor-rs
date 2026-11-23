# pdfstamp

pdfstamp puts a line of text on every page of a PDF file. It needs nothing else installed.

## Install

1. Install Rust from rustup.rs if you do not have it.
2. Run `cargo install pdfstamp`. The build takes a minute or two.
3. Open a new terminal and run `pdfstamp --version` to check.

## Usage

Stamp a file: `pdfstamp contract.pdf --text "Received"`.
Several files at once: `pdfstamp *.pdf --text "Received"`.

| Option | Meaning | Default |
| --- | --- | --- |
| `--position` | top, bottom, left or right | bottom |
| `--size` | text size in points | 10 |
| `--colour` | hex colour | 000000 |

### Output files

The stamped copy is saved beside the original as `name-stamped.pdf`. The original is never overwritten.

## Limits

Scanned pages can be stamped, but the text is drawn over the image and may cover something. Password-protected files cannot be opened: pdfstamp says so and stops. Files over two gigabytes are not supported, because the whole page tree is read into memory before anything is written.

PDF 1.2 and older is not tested.

## Fonts

The text is drawn in Helvetica. Another font cannot be chosen yet.

## Licence

MIT.
