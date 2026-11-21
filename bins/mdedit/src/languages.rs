//! The languages a document can be switched to (the status bar's language field and the
//! palette's "Language" command), by the names the status bar shows. "Plain text" is no
//! language. The highlighter has no list of its own, so the list is the languages these names and
//! extensions find, each once, in name order: what the shared highlighter knows, without its
//! variants for one framework (Rails, ASP).

use mdedit_highlight::Language;
use std::path::Path;

/// What a document with no language is called.
pub const PLAIN: &str = "Plain text";

/// The extension a document with no language of its own is saved under.
pub const PLAIN_EXT: &str = "txt";

/// One name or extension per language offered.
const TOKENS: &[&str] = &[
    "md",
    "rs",
    "c",
    "cpp",
    "cs",
    "java",
    "js",
    "typescript",
    "go",
    "py",
    "rb",
    "sh",
    "powershell",
    "bat",
    "php",
    "html",
    "css",
    "scss",
    "less",
    "json",
    "yaml",
    "toml",
    "ini",
    "xml",
    "sql",
    "lua",
    "pl",
    "r",
    "scala",
    "kotlin",
    "swift",
    "dart",
    "zig",
    "hs",
    "clj",
    "erl",
    "d",
    "m",
    "mm",
    "ml",
    "pas",
    "groovy",
    "tcl",
    "tex",
    "lisp",
    "matlab",
    "applescript",
    "as",
    "jsp",
    "properties",
    "rst",
    "textile",
    "diff",
    "dot",
    "mk",
    "cmake",
    "dockerfile",
    "vue",
    "svelte",
];

/// Every language offered, in name order (case ignored).
pub fn all() -> Vec<Language> {
    let mut out: Vec<Language> = Vec::new();
    for token in TOKENS {
        if let Some(l) = Language::by_token(token)
            && !out.iter().any(|o| o.name() == l.name())
        {
            out.push(l);
        }
    }
    out.sort_by_key(|l| l.name().to_lowercase());
    out
}

/// The name a document's language goes by: the language's own, or "Plain text".
pub fn name_of(language: Option<Language>) -> &'static str {
    language.map_or(PLAIN, Language::name)
}

/// The language a name stands for: `Some(None)` for plain text, `None` for a name not offered.
pub fn by_name(name: &str) -> Option<Option<Language>> {
    if name == PLAIN {
        return Some(None);
    }
    all().into_iter().find(|l| l.name() == name).map(Some)
}

/// The same name, as the list keeps it (for a [`crate::places::Place`], which is `Copy`).
pub fn static_name(name: &str) -> Option<&'static str> {
    by_name(name).map(name_of)
}

/// Markdown, which a text the AI formatted as Markdown becomes.
pub fn markdown() -> Option<Language> {
    Language::by_token("md")
}

/// The extension Save offers a document that has no file yet: the first token that leads back to
/// the document's language as a file name does, so the file saved keeps the language it is shown
/// in. [`PLAIN_EXT`] for plain text, and for a language no token here names as an extension.
pub fn extension_of(language: Option<Language>) -> &'static str {
    let Some(language) = language else {
        return PLAIN_EXT;
    };
    TOKENS
        .iter()
        .copied()
        .find(|token| {
            Language::for_path(Path::new(&format!("x.{token}")), "")
                .is_some_and(|l| l.name() == language.name())
        })
        .unwrap_or(PLAIN_EXT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_has_each_language_once_and_names_lead_back() {
        let list = all();
        assert!(list.len() > 40, "{}", list.len());
        let names: Vec<&str> = list.iter().map(|l| l.name()).collect();
        for want in ["Markdown", "Rust", "TOML", "Kotlin", "Python", "JSON", "C#"] {
            assert!(names.contains(&want), "{want} in {names:?}");
        }
        let mut seen = names.clone();
        seen.dedup();
        assert_eq!(seen.len(), names.len());
        for l in &list {
            assert_eq!(by_name(l.name()), Some(Some(*l)));
        }
        assert_eq!(by_name(PLAIN), Some(None));
        assert_eq!(by_name("Klingon"), None);
        assert_eq!(markdown().map(Language::name), Some("Markdown"));
        assert_eq!(static_name("Rust"), Some("Rust"));
    }

    #[test]
    fn a_languages_extension_leads_back_to_it_and_plain_text_is_txt() {
        assert_eq!(extension_of(None), PLAIN_EXT);
        assert_eq!(extension_of(markdown()), "md");
        assert_eq!(extension_of(Language::by_token("rs")), "rs");
        assert_eq!(extension_of(Language::by_token("toml")), "toml");
        for language in all() {
            let ext = extension_of(Some(language));
            if ext == PLAIN_EXT {
                continue;
            }
            let back = Language::for_path(Path::new(&format!("note.{ext}")), "");
            assert_eq!(
                back.map(Language::name),
                Some(language.name()),
                "note.{ext} for {}",
                language.name()
            );
        }
    }
}
