//! Find in a conversation — port of `renderer/src/threadFind.ts`.

/// The most hits looked for: a one-letter search in a long conversation still answers at once.
pub const MAX_FIND_HITS: usize = 2000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindHit {
    /// The piece the match starts in, and the offset of its first letter there.
    pub from: (usize, usize),
    /// The piece it ends in, and the offset just past its last letter there.
    pub to: (usize, usize),
}

pub fn find_in(pieces: &[&str], query: &str, max: usize) -> Vec<FindHit> {
    if query.trim().is_empty() || pieces.is_empty() {
        return Vec::new();
    }
    // Search the original text: lowercasing can change its length (İ), corrupting offsets.
    let Ok(pattern) = regex::RegexBuilder::new(&regex::escape(query))
        .case_insensitive(true)
        .build()
    else {
        return Vec::new();
    };
    let mut starts = Vec::with_capacity(pieces.len());
    let mut length = 0;
    for piece in pieces {
        starts.push(length);
        length += piece.len();
    }
    let text = pieces.concat();
    let mut hits = Vec::new();
    let mut piece = 0;
    for found in pattern.find_iter(&text).take(max) {
        let start = found.start();
        let end = found.end();
        while piece + 1 < pieces.len() && start >= starts[piece] + pieces[piece].len() {
            piece += 1;
        }
        let mut last = piece;
        while last + 1 < pieces.len() && end > starts[last] + pieces[last].len() {
            last += 1;
        }
        hits.push(FindHit {
            from: (piece, text[starts[piece]..start].encode_utf16().count()),
            to: (last, text[starts[last]..end].encode_utf16().count()),
        });
    }
    hits
}

/// "3 of 12", "No matches", or nothing before anything is asked.
pub fn find_count_words(hits: usize, current: usize, query: &str) -> String {
    if query.trim().is_empty() {
        return String::new();
    }
    if hits == 0 {
        return "No matches".into();
    }
    let n = current.min(hits.saturating_sub(1)) + 1;
    let total = if hits >= MAX_FIND_HITS {
        format!("{MAX_FIND_HITS}+")
    } else {
        hits.to_string()
    };
    format!("{n} of {total}")
}

/// The next hit, or the one before, round the end and back to the start.
pub fn step_hit(current: usize, hits: usize, back: bool) -> usize {
    if hits == 0 {
        return 0;
    }
    if back {
        (current + hits - 1) % hits
    } else {
        (current + 1) % hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // a-conversation-can-be-searched.test.ts
    #[test]
    fn a_conversation_can_be_searched() {
        assert_eq!(
            find_in(
                &["The deploy failed. Deploy again?"],
                "deploy",
                MAX_FIND_HITS
            ),
            vec![
                FindHit {
                    from: (0, 4),
                    to: (0, 10)
                },
                FindHit {
                    from: (0, 19),
                    to: (0, 25)
                },
            ]
        );
        assert_eq!(
            find_in(
                &["Run ", "npm test", " again"],
                "run npm test a",
                MAX_FIND_HITS
            ),
            vec![FindHit {
                from: (0, 0),
                to: (2, 2)
            }]
        );
        assert_eq!(
            find_in(&["Run ", "npm test", " again"], "npm test", MAX_FIND_HITS),
            vec![FindHit {
                from: (1, 0),
                to: (1, 8)
            }]
        );
        assert_eq!(
            find_in(&["ab", "", "cd"], "bc", MAX_FIND_HITS),
            vec![FindHit {
                from: (0, 1),
                to: (2, 1)
            }]
        );
        assert_eq!(
            find_in(&["cost (USD): $4.20 vs $4x20"], "$4.20", MAX_FIND_HITS),
            vec![FindHit {
                from: (0, 12),
                to: (0, 17)
            }]
        );
        assert_eq!(
            find_in(&["a+b and a*b"], "a*b", MAX_FIND_HITS),
            vec![FindHit {
                from: (0, 8),
                to: (0, 11)
            }]
        );
        assert!(find_in(&["anything"], "", MAX_FIND_HITS).is_empty());
        assert!(find_in(&["anything"], "   ", MAX_FIND_HITS).is_empty());
        assert!(find_in(&[], "a", MAX_FIND_HITS).is_empty());
        let long = "a".repeat(MAX_FIND_HITS + 50);
        assert_eq!(
            find_in(&[long.as_str()], "a", MAX_FIND_HITS).len(),
            MAX_FIND_HITS
        );

        assert_eq!(find_count_words(12, 2, "deploy"), "3 of 12");
        assert_eq!(find_count_words(0, 0, "deploy"), "No matches");
        assert_eq!(find_count_words(0, 0, ""), "");
        assert_eq!(
            find_count_words(MAX_FIND_HITS, 0, "a"),
            format!("1 of {MAX_FIND_HITS}+")
        );
        assert_eq!(step_hit(0, 3, false), 1);
        assert_eq!(step_hit(2, 3, false), 0);
        assert_eq!(step_hit(0, 3, true), 2);
        assert_eq!(step_hit(0, 0, false), 0);
    }
}

#[cfg(test)]
mod unicode_review_tests {
    use super::*;
    #[test]
    fn unicode_offsets_are_utf16_and_simple_casefold_does_not_expand_letters() {
        assert_eq!(
            find_in(&["😀 café", " Σ ς K ſ İ x"], "CAFÉ Σ", 20),
            vec![FindHit {
                from: (0, 3),
                to: (1, 2)
            }]
        );
        assert_eq!(
            find_in(&["İ x"], "x", 20),
            vec![FindHit {
                from: (0, 2),
                to: (0, 3)
            }]
        );
        assert!(find_in(&["İ"], "i", 20).is_empty());
        assert_eq!(find_in(&["Σ ς K ſ"], "σ", 20).len(), 2);
        assert_eq!(find_in(&["K"], "k", 20).len(), 1);
    }
}
