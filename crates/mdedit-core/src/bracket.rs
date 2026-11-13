//! The bracket that pairs with the one at the caret: `()`, `[]` and `{}`, counted by nesting of
//! the same kind only. Text is not parsed, so a bracket in a string or a comment counts like any
//! other.

use ropey::Rope;

/// The partner of a bracket and whether it lies further on.
fn pair(c: char) -> Option<(char, bool)> {
    Some(match c {
        '(' => (')', true),
        '[' => (']', true),
        '{' => ('}', true),
        ')' => ('(', false),
        ']' => ('[', false),
        '}' => ('{', false),
        _ => return None,
    })
}

/// The partner of the bracket at char index `at`, looking through at most `limit` bytes.
fn partner(rope: &Rope, at: usize, c: char, limit: usize) -> Option<usize> {
    let (other, forward) = pair(c)?;
    let (mut depth, mut bytes) = (0usize, 0usize);
    let mut step = |ch: char| -> Option<bool> {
        bytes += ch.len_utf8();
        if bytes > limit {
            return None;
        }
        if ch == c {
            depth += 1;
        } else if ch == other {
            if depth == 0 {
                return Some(true);
            }
            depth -= 1;
        }
        Some(false)
    };
    if forward {
        for (k, ch) in rope.chars_at(at + 1).enumerate() {
            if step(ch)? {
                return Some(at + 1 + k);
            }
        }
    } else {
        let mut it = rope.chars_at(at);
        let mut k = at;
        while let Some(ch) = it.prev() {
            k -= 1;
            if step(ch)? {
                return Some(k);
            }
        }
    }
    None
}

/// The byte offsets of the bracket at the caret and of its partner: the bracket right after the
/// caret if there is one with a partner, else the one right before it. `None` when neither is a
/// bracket or the partner is not within `limit` bytes.
pub fn matching(rope: &Rope, caret: usize, limit: usize) -> Option<(usize, usize)> {
    if caret > rope.len_bytes() {
        return None;
    }
    let at = rope.byte_to_char(caret);
    let after = (at < rope.len_chars()).then_some(at);
    let before = at.checked_sub(1);
    [after, before].into_iter().flatten().find_map(|i| {
        let c = rope.char(i);
        let j = partner(rope, i, c, limit)?;
        Some((rope.char_to_byte(i), rope.char_to_byte(j)))
    })
}

/// The opener that a `closer` typed at byte offset `caret` would pair with: the nearest `(`,
/// `[` or `{` of the matching kind before `caret` that no bracket of that kind closes, counted by
/// nesting of the same kind only (as [`matching`]). Looks through at most `limit` bytes. `None`
/// when `closer` is not `)`, `]` or `}`, when there is no such opener or when it is further
/// than `limit`.
pub fn opener_before(rope: &Rope, caret: usize, closer: char, limit: usize) -> Option<usize> {
    if caret > rope.len_bytes() || !matches!(closer, ')' | ']' | '}') {
        return None;
    }
    let at = rope.byte_to_char(caret);
    let j = partner(rope, at, closer, limit)?;
    Some(rope.char_to_byte(j))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(text: &str, caret: usize) -> Option<(usize, usize)> {
        matching(&Rope::from_str(text), caret, 1 << 16)
    }

    #[test]
    fn pairs_by_nesting() {
        let t = "f(a[1], {b: (c)})";
        assert_eq!(m(t, 1), Some((1, 16)));
        assert_eq!(m(t, 17), Some((16, 1)));
        assert_eq!(m(t, 3), Some((3, 5)));
        assert_eq!(m(t, 6), Some((5, 3)));
        assert_eq!(m(t, 8), Some((8, 15)));
        assert_eq!(m(t, 12), Some((12, 14)));
        // Between "c" and ")": the one after the caret.
        assert_eq!(m(t, 14), Some((14, 12)));
        // Between ")" and "}": after first.
        assert_eq!(m(t, 15), Some((15, 8)));
        assert_eq!(m(t, 0), None);
        assert_eq!(m(t, 2), Some((1, 16)));
        assert_eq!(m(t, 99), None);
    }

    #[test]
    fn unpaired_and_other_kinds() {
        assert_eq!(m("(]", 0), None);
        assert_eq!(m("((a)", 0), None);
        assert_eq!(m("((a)", 1), Some((1, 3)));
        // Other kinds in between do not count.
        assert_eq!(m("(])", 0), Some((0, 2)));
        // The one before the caret is used when the one after has no partner.
        assert_eq!(m("(a)(", 3), Some((2, 0)));
        assert_eq!(m("", 0), None);
    }

    #[test]
    fn offsets_are_bytes_and_the_search_is_limited() {
        let t = "ç(ğü)ş";
        assert_eq!(m(t, 2), Some((2, 7)));
        assert_eq!(m(t, 8), Some((7, 2)));
        let far = format!("({})", "x".repeat(100));
        let r = Rope::from_str(&far);
        assert_eq!(matching(&r, 0, 101), Some((0, 101)));
        assert_eq!(matching(&r, 0, 100), None);
        assert_eq!(matching(&r, 102, 100), None);
    }

    #[test]
    fn opener_before_finds_the_unclosed_opener() {
        let o =
            |t: &str, caret: usize, c: char| opener_before(&Rope::from_str(t), caret, c, 1 << 16);
        assert_eq!(o("fn f() {\n  a(b)\n  ", 18, '}'), Some(7));
        assert_eq!(o("{ { } ", 6, '}'), Some(0));
        assert_eq!(o("( [ ", 4, ')'), Some(0));
        assert_eq!(o("a) ", 3, ')'), None);
        assert_eq!(o("{ ", 2, '>'), None);
        assert_eq!(o("{ ", 99, '}'), None);
        assert_eq!(o("{ aaaa ", 7, '}'), Some(0));
        assert_eq!(opener_before(&Rope::from_str("{ aaaa "), 7, '}', 3), None);
        assert_eq!(o("é{é ", 6, '}'), Some(2));
    }
}
