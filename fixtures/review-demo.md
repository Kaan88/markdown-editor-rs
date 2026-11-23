# pdfstamp

pdfstamp puts a line of text on every page of a PDF file. It is a small command line tool and it needs nothing else installed.

## Install

You need to have Rust installed. If you do not have it, go to rustup.rs and follow the steps there, then come back.

Then run cargo install pdfstamp and wait for it to build, which takes a minute or two.

After that you can check that it worked by running pdfstamp --version in a new terminal window.

## Usage

To stamp a file you run pdfstamp with the file name and then the text you want, for example pdfstamp contract.pdf --text "Received".
There are other options as well. The position can be top, bottom, left or right. The size is in points. The colour is a hex number.
If you want more than one file stamped you just add more file names.

### Output files

The stamped file is saved next to the original with -stamped added to the name so nothing is overwritten.

## Limits

Pages that are scanned images can be stamped but the text is drawn on top of the image, so it can cover something. Files that are protected with a password cannot be opened at all and pdfstamp says so and stops. Files larger than two gigabytes are not supported because the whole page tree is read into memory before anything is written, which would not fit on most machines.

Very old PDF files (version 1.2 and older) are not tested.

## Fonts

The text is drawn in Helvetica. There is no way to choose another font yet.

## Licence

MIT. Do what you like with it.

## Thanks

Thanks to everyone who sent a bug report.
