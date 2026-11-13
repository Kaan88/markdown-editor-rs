//! Fuzzy matching for a command palette and file pickers: a query matches a candidate when its
//! characters occur in the candidate in order, ignoring case. Standard library only, so the
//! module can live in a shared crate unchanged.
//!
//! The score prefers, in this order of weight: a candidate that starts with the query (it beats
//! every other match), matches at the start of a word (after a space, `_`, `-`, `/`, `\` or `.`,
//! or at a lower-to-upper case change) and at the very start, matches that follow each other,
//! and short candidates. Gaps between matched characters cost a little each. The best alignment
//! is found by dynamic programming over (query char, candidate char), so the cost is
//! O(query length x candidate length) with both capped: a candidate is cut after
//! [`MAX_CANDIDATE_CHARS`] chars and a query after [`MAX_QUERY_CHARS`] chars (white space in the
//! query is ignored, so "tog bk" is "togbk"). Results are deterministic.

/// The longest candidate looked at, in chars; the rest of a longer one is ignored.
pub const MAX_CANDIDATE_CHARS: usize = 256;
/// The longest query used, in chars (after dropping white space); the rest is ignored.
pub const MAX_QUERY_CHARS: usize = 64;

const MATCH: i32 = 16;
const WORD_START: i32 = 24;
const AT_START: i32 = 16;
const CONSECUTIVE: i32 = 18;
const GAP: i32 = 1;
const LENGTH: i32 = 1;
const PREFIX: i32 = 1 << 20;
const NONE: i32 = i32::MIN / 2;

/// A successful match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    /// Higher is better. Only meaningful between matches of the same query.
    pub score: i32,
    /// The char indexes (not byte offsets) in the candidate, of the cut candidate when it was
    /// cut, of the query's chars, ascending.
    pub positions: Vec<u32>,
}

/// The simple lowercase of `c`: its first char where Unicode gives several.
fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn is_separator(c: char) -> bool {
    matches!(c, ' ' | '_' | '-' | '/' | '\\' | '.')
}

/// Matches `query` against `candidate`. `None` when the query's chars (white space dropped) do
/// not occur in order. An empty query matches everything with score 0 and no positions.
pub fn score(query: &str, candidate: &str) -> Option<Match> {
    let q: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(lower)
        .take(MAX_QUERY_CHARS)
        .collect();
    if q.is_empty() {
        return Some(Match {
            score: 0,
            positions: Vec::new(),
        });
    }
    let orig: Vec<char> = candidate.chars().take(MAX_CANDIDATE_CHARS).collect();
    let n = orig.len();
    if q.len() > n {
        return None;
    }
    let low: Vec<char> = orig.iter().map(|&c| lower(c)).collect();
    // The bonus for a match at each candidate position.
    let bonus: Vec<i32> = (0..n)
        .map(|j| {
            let prev = if j == 0 { None } else { Some(orig[j - 1]) };
            let mut b = MATCH;
            if j == 0 {
                b += AT_START + WORD_START;
            } else if let Some(p) = prev
                && (is_separator(p) || (p.is_lowercase() && orig[j].is_uppercase()))
            {
                b += WORD_START;
            }
            b
        })
        .collect();
    // dp[i * n + j]: the best score with query char i matched at candidate char j.
    let mut dp = vec![NONE; q.len() * n];
    for j in 0..n {
        if low[j] == q[0] {
            dp[j] = bonus[j];
        }
    }
    for i in 1..q.len() {
        // The best earlier row value, less a gap per skipped char, ending just before j.
        let mut run = NONE;
        for j in 1..n {
            let prev = dp[(i - 1) * n + j - 1];
            run = if run > NONE { run - GAP } else { NONE }.max(prev);
            if low[j] != q[i] {
                continue;
            }
            let best = run.max(if prev > NONE {
                prev + CONSECUTIVE
            } else {
                NONE
            });
            if best > NONE {
                dp[i * n + j] = best + bonus[j];
            }
        }
    }
    let last = (q.len() - 1) * n;
    // The end of the best alignment; the first of equal ones.
    let (mut j, mut best) = (n, NONE);
    for k in 0..n {
        if dp[last + k] > best {
            best = dp[last + k];
            j = k;
        }
    }
    if best == NONE {
        return None;
    }
    // Walk back to the positions.
    let mut positions = vec![0u32; q.len()];
    for i in (0..q.len()).rev() {
        positions[i] = j as u32;
        if i == 0 {
            break;
        }
        let here = dp[i * n + j] - bonus[j];
        let row = (i - 1) * n;
        j = if dp[row + j - 1] > NONE && dp[row + j - 1] + CONSECUTIVE == here {
            j - 1
        } else {
            (0..j)
                .rev()
                .find(|&k| dp[row + k] > NONE && dp[row + k] - GAP * (j - 1 - k) as i32 == here)
                .unwrap_or(j - 1)
        };
    }
    let starts_with = positions.iter().enumerate().all(|(i, &p)| p as usize == i);
    let mut total = best - LENGTH * n as i32;
    if starts_with {
        total += PREFIX;
    }
    Some(Match {
        score: total,
        positions,
    })
}

/// The best `limit` of `items` for `query`: (index into `items`, match), best first. Ties in
/// score go to the shorter candidate (in chars, as cut), then to the earlier item. An empty query
/// returns the first `limit` items in their order, each with score 0.
pub fn rank(query: &str, items: &[&str], limit: usize) -> Vec<(usize, Match)> {
    if query.chars().all(char::is_whitespace) {
        return items
            .iter()
            .enumerate()
            .take(limit)
            .map(|(i, _)| {
                (
                    i,
                    score("", "").unwrap_or(Match {
                        score: 0,
                        positions: Vec::new(),
                    }),
                )
            })
            .collect();
    }
    let mut found: Vec<(usize, usize, Match)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let m = score(query, s)?;
            Some((i, s.chars().take(MAX_CANDIDATE_CHARS).count(), m))
        })
        .collect();
    found.sort_by(|a, b| {
        b.2.score
            .cmp(&a.2.score)
            .then(a.1.cmp(&b.1))
            .then(a.0.cmp(&b.0))
    });
    found.truncate(limit);
    found.into_iter().map(|(i, _, m)| (i, m)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(q: &str, items: &[&str]) -> Vec<usize> {
        rank(q, items, items.len())
            .into_iter()
            .map(|(i, _)| i)
            .collect()
    }

    #[test]
    fn subsequence_and_case() {
        assert!(score("tb", "Toggle bookmark").is_some());
        assert!(score("bkm", "Toggle bookmark").is_some());
        assert!(score("TB", "toggle bookmark").is_some());
        assert!(score("bt", "Toggle bookmark").is_none());
        assert!(score("xyz", "abc").is_none());
        assert!(score("abcd", "abc").is_none());
        let m = score("tb", "Toggle bookmark").unwrap();
        assert_eq!(m.positions, vec![0, 7]);
        let m = score("bkm", "Toggle bookmark").unwrap();
        assert_eq!(m.positions, vec![7, 10, 11]);
        // Query white space is ignored.
        assert_eq!(
            score("t b", "Toggle bookmark"),
            score("tb", "Toggle bookmark")
        );
    }

    #[test]
    fn empty_query_keeps_the_order() {
        let items = ["b", "a", "c"];
        let r = rank("", &items, 10);
        assert_eq!(r.iter().map(|x| x.0).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert!(r.iter().all(|x| x.1.score == 0 && x.1.positions.is_empty()));
        assert_eq!(rank("  ", &items, 2).len(), 2);
        assert_eq!(rank("", &[], 5).len(), 0);
        assert_eq!(rank("a", &items, 0).len(), 0);
    }

    #[test]
    fn commands() {
        let items = [
            "Show whitespace",
            "Save as",
            "Toggle bookmark",
            "Select all",
            "Save",
            "Sort lines",
        ];
        let o = order("sa", &items);
        let pos = |i: usize| o.iter().position(|&x| x == i).unwrap();
        assert!(pos(1) < pos(0), "Save as before Show whitespace: {o:?}");
        assert!(pos(4) < pos(1), "the shorter prefix match first: {o:?}");
        assert_eq!(order("tb", &items)[0], 2);
        assert_eq!(order("bkm", &items), vec![2]);
        // Exact prefix beats a better-aligned word match elsewhere.
        assert_eq!(order("sort", &["Auto sort", "Sort lines"])[0], 1);
        // Word starts beat scattered letters.
        assert_eq!(order("tl", &["Settle", "Toggle line"])[0], 1);
    }

    #[test]
    fn paths() {
        let items = [
            "src/app/commands.rs",
            "src/app.rs",
            "src/apps/other/cmd_helpers.rs",
            "docs/PLAN.md",
        ];
        let o = order("appcmd", &items);
        // `src/app.rs` has no "cmd"; the docs file has no match at all.
        assert_eq!(o.len(), 2);
        assert!(o.contains(&0) && o.contains(&2));
        let m = score("appcmd", items[0]).unwrap();
        assert_eq!(&m.positions[..4], &[4, 5, 6, 8]);
        // Unbroken runs beat the same letters split by a gap.
        assert_eq!(order("cmd", &["src/app/commands.rs", "src/cmd.rs"])[0], 1);
        // Camel case humps and separators of all kinds.
        let m = score("gtl", "goToLine").unwrap();
        assert_eq!(m.positions, vec![0, 2, 4]);
        assert!(score("sc", "src\\main.c").is_some());
        assert!(
            score("mr", "main.rs").unwrap().score
                > score("mr", "mainrs_extra_thing").unwrap().score
        );
    }

    #[test]
    fn ties_go_to_the_shorter_then_the_earlier() {
        let items = ["abc def", "abc", "abc", "abcd"];
        assert_eq!(order("abc", &items), vec![1, 2, 3, 0]);
        let r = rank("abc", &items, 2);
        assert_eq!(r.iter().map(|x| x.0).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn deterministic_and_positions_are_valid() {
        let items = [
            "éclair au café",
            "ÉCLAIR",
            "cafe",
            "日本語のファイル",
            "a_b_c-d.e/f\\g",
        ];
        for q in ["é", "ca", "ac", "日語", "abcdefg", "ÉC", "xx"] {
            let a = rank(q, &items, 9);
            assert_eq!(a, rank(q, &items, 9));
            for (i, m) in a {
                let cs: Vec<char> = items[i].chars().collect();
                assert_eq!(
                    m.positions.len(),
                    q.chars().filter(|c| !c.is_whitespace()).count()
                );
                assert!(m.positions.windows(2).all(|w| w[0] < w[1]));
                for (k, &p) in m.positions.iter().enumerate() {
                    let qc = q.chars().nth(k).unwrap();
                    assert_eq!(lower(cs[p as usize]), lower(qc));
                }
            }
        }
    }

    #[test]
    fn hostile_input_is_bounded() {
        let long = "a".repeat(100_000);
        let m = score("aaaa", &long).unwrap();
        assert_eq!(m.positions, vec![0, 1, 2, 3]);
        // Only the first 256 chars are looked at.
        let tail = format!("{}b", "a".repeat(300));
        assert!(score("b", &tail).is_none());
        // A long query is cut to its first 64 chars.
        let q = "a".repeat(500);
        assert_eq!(score(&q, &long).unwrap().positions.len(), 64);
        assert!(score(&"b".repeat(5), &long).is_none());
        // Odd characters, empty candidate, NUL, combining marks.
        assert!(score("a", "").is_none());
        assert!(score("\0", "a\0b").is_some());
        assert!(score("e\u{301}", "e\u{301}").is_some());
        let many: Vec<String> = (0..2000).map(|i| format!("item number {i}")).collect();
        let refs: Vec<&str> = many.iter().map(String::as_str).collect();
        assert_eq!(rank("in9", &refs, 5).len(), 5);
    }
}
