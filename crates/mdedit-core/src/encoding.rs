//! What a file's bytes were, and the names shown for it.
//!
//! Detection order (decided in `docs/DESIGN.md`): a byte order mark (UTF-8, UTF-16 LE, UTF-16
//! BE) decides; without one, the bytes are UTF-8 if they are valid UTF-8 from start to end;
//! otherwise the whole file is read in the system's legacy code page (Windows-1254 on the user's
//! PC), which the shell passes in through `LoadOptions`.

/// The encoding a file was read in. The buffer itself always holds UTF-8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// UTF-8 without a byte order mark.
    Utf8,
    /// UTF-8 starting with EF BB BF. The mark is not part of the buffer.
    Utf8Bom,
    /// FF FE. The mark is not part of the buffer.
    Utf16Le,
    /// FE FF. The mark is not part of the buffer.
    Utf16Be,
    /// A single- or multi-byte legacy code page, such as windows-1254.
    Legacy(&'static encoding_rs::Encoding),
}

impl Encoding {
    /// The name shown in the status area: "UTF-8", "UTF-8 with BOM", "UTF-16 LE", "UTF-16 BE",
    /// or the code page's name with an upper-case first letter ("Windows-1254", "Shift_JIS").
    pub fn label(self) -> String {
        todo!()
    }

    /// Length of the byte order mark in the file: 3, 2, 2, or 0.
    pub fn bom_len(self) -> usize {
        todo!()
    }
}

/// The encoding a byte order mark at the start of `head` names, if any. `head` may be shorter
/// than a mark (an empty or one-byte file has none).
pub fn sniff_bom(head: &[u8]) -> Option<Encoding> {
    let _ = head;
    todo!()
}

/// The `encoding_rs` encoding for a Windows ANSI code page number (`GetACP`): 874, 932, 936,
/// 949, 950 and 1250 to 1258. Anything else gives windows-1252.
pub fn legacy_for_code_page(code_page: u32) -> &'static encoding_rs::Encoding {
    let _ = code_page;
    todo!()
}
