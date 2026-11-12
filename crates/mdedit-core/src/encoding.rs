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
        match self {
            Encoding::Utf8 => "UTF-8".to_string(),
            Encoding::Utf8Bom => "UTF-8 with BOM".to_string(),
            Encoding::Utf16Le => "UTF-16 LE".to_string(),
            Encoding::Utf16Be => "UTF-16 BE".to_string(),
            Encoding::Legacy(e) => {
                let mut chars = e.name().chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                    None => String::new(),
                }
            }
        }
    }

    /// Length of the byte order mark in the file: 3, 2, 2, or 0.
    pub fn bom_len(self) -> usize {
        match self {
            Encoding::Utf8Bom => 3,
            Encoding::Utf16Le | Encoding::Utf16Be => 2,
            Encoding::Utf8 | Encoding::Legacy(_) => 0,
        }
    }
}

/// The encoding a byte order mark at the start of `head` names, if any. `head` may be shorter
/// than a mark (an empty or one-byte file has none).
pub fn sniff_bom(head: &[u8]) -> Option<Encoding> {
    match head {
        [0xEF, 0xBB, 0xBF, ..] => Some(Encoding::Utf8Bom),
        // FF FE 00 00 is read as UTF-16 LE; UTF-32 is not detected.
        [0xFF, 0xFE, ..] => Some(Encoding::Utf16Le),
        [0xFE, 0xFF, ..] => Some(Encoding::Utf16Be),
        _ => None,
    }
}

/// The `encoding_rs` encoding for a Windows ANSI code page number (`GetACP`): 874, 932, 936,
/// 949, 950 and 1250 to 1258. Anything else gives windows-1252.
pub fn legacy_for_code_page(code_page: u32) -> &'static encoding_rs::Encoding {
    match code_page {
        874 => encoding_rs::WINDOWS_874,
        932 => encoding_rs::SHIFT_JIS,
        936 => encoding_rs::GBK,
        949 => encoding_rs::EUC_KR,
        950 => encoding_rs::BIG5,
        1250 => encoding_rs::WINDOWS_1250,
        1251 => encoding_rs::WINDOWS_1251,
        1252 => encoding_rs::WINDOWS_1252,
        1253 => encoding_rs::WINDOWS_1253,
        1254 => encoding_rs::WINDOWS_1254,
        1255 => encoding_rs::WINDOWS_1255,
        1256 => encoding_rs::WINDOWS_1256,
        1257 => encoding_rs::WINDOWS_1257,
        1258 => encoding_rs::WINDOWS_1258,
        _ => encoding_rs::WINDOWS_1252,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_sniffing() {
        assert_eq!(
            sniff_bom(&[0xEF, 0xBB, 0xBF, b'a']),
            Some(Encoding::Utf8Bom)
        );
        assert_eq!(sniff_bom(&[0xEF, 0xBB]), None);
        assert_eq!(sniff_bom(&[0xFF, 0xFE]), Some(Encoding::Utf16Le));
        assert_eq!(sniff_bom(&[0xFF, 0xFE, 0, 0]), Some(Encoding::Utf16Le));
        assert_eq!(sniff_bom(&[0xFE, 0xFF, 0, b'a']), Some(Encoding::Utf16Be));
        assert_eq!(sniff_bom(&[0xFF]), None);
        assert_eq!(sniff_bom(b""), None);
        assert_eq!(sniff_bom(b"abc"), None);
    }

    #[test]
    fn labels_and_bom_lengths() {
        assert_eq!(Encoding::Utf8.label(), "UTF-8");
        assert_eq!(Encoding::Utf8Bom.label(), "UTF-8 with BOM");
        assert_eq!(Encoding::Utf16Le.label(), "UTF-16 LE");
        assert_eq!(Encoding::Utf16Be.label(), "UTF-16 BE");
        assert_eq!(
            Encoding::Legacy(encoding_rs::WINDOWS_1254).label(),
            "Windows-1254"
        );
        assert_eq!(
            Encoding::Legacy(encoding_rs::SHIFT_JIS).label(),
            "Shift_JIS"
        );
        assert_eq!(Encoding::Utf8Bom.bom_len(), 3);
        assert_eq!(Encoding::Utf16Le.bom_len(), 2);
        assert_eq!(Encoding::Utf16Be.bom_len(), 2);
        assert_eq!(Encoding::Utf8.bom_len(), 0);
        assert_eq!(Encoding::Legacy(encoding_rs::WINDOWS_1252).bom_len(), 0);
    }

    #[test]
    fn code_pages() {
        assert_eq!(legacy_for_code_page(1254), encoding_rs::WINDOWS_1254);
        assert_eq!(legacy_for_code_page(932), encoding_rs::SHIFT_JIS);
        assert_eq!(legacy_for_code_page(936), encoding_rs::GBK);
        assert_eq!(legacy_for_code_page(949), encoding_rs::EUC_KR);
        assert_eq!(legacy_for_code_page(950), encoding_rs::BIG5);
        assert_eq!(legacy_for_code_page(874), encoding_rs::WINDOWS_874);
        assert_eq!(legacy_for_code_page(437), encoding_rs::WINDOWS_1252);
        assert_eq!(legacy_for_code_page(0), encoding_rs::WINDOWS_1252);
        assert_eq!(legacy_for_code_page(1258), encoding_rs::WINDOWS_1258);
    }
}
