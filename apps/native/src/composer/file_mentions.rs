//! `@` file mention matching — port of `renderer/src/fileMentions.ts`.

/// The file being asked for, if the message ends in `@something` (or a bare `@`).
pub fn at_query(text: &str) -> Option<String> {
    let re = crate::invariants::regex(r"(?:^|\s)@([^\s@]*)$");
    re.captures(text)
        .map(|c| c.get(1).map(|m| m.as_str().to_owned()).unwrap_or_default())
}

/// The message with the trailing `@something` taken out, once its file is picked.
pub fn without_at_query(text: &str) -> String {
    let re = crate::invariants::regex(r"(^|\s)@[^\s@]*$");
    re.replace(text, "$1").into_owned()
}

fn base_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Accents aside, as case is (Electron): only strings that hold a non-ASCII character are touched.
fn folded(text: &str) -> String {
    if !!text.is_ascii() {
        return text.to_owned();
    }
    use unicode_normalization::UnicodeNormalization;
    text.nfd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .collect()
}

/// Whether every character of `query` appears in `text`, in order.
fn in_order(text: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut q = query.chars();
    let mut want = q.next();
    for ch in text.chars() {
        if Some(ch) == want {
            want = q.next();
            if want.is_none() {
                return true;
            }
        }
    }
    false
}

/// The files that match, best first (Electron order). Default max is 8.
pub fn file_matches(paths: &[String], query: &str, max: usize) -> Vec<String> {
    let q = folded(&query.to_lowercase().replace('\\', "/"));
    let mut ranked: Vec<(String, i32, usize, usize)> = Vec::new();
    for path in paths {
        let lower = folded(&path.to_lowercase());
        let base = base_of(&lower);
        let rank = if q.is_empty() || base == q {
            0
        } else if base.starts_with(&q) {
            1
        } else if base.contains(&q) {
            2
        } else if lower.contains(&q) {
            3
        } else if in_order(&lower, &q) {
            4
        } else {
            -1
        };
        if rank >= 0 {
            ranked.push((
                path.clone(),
                rank,
                path.split('/').count(),
                path.encode_utf16().count(),
            ));
        }
    }
    ranked.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then(a.2.cmp(&b.2))
            .then(a.3.cmp(&b.3))
            .then_with(|| a.0.encode_utf16().cmp(b.0.encode_utf16()))
    });
    ranked.into_iter().take(max).map(|(p, _, _, _)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfd_matches_electron_without_ligature_transliteration() {
        let paths = vec!["ǽ.md".into(), "æ.md".into(), "Český.md".into()];
        assert_eq!(file_matches(&paths, "cesky", 8), vec!["Český.md"]);
        assert!(file_matches(&paths, "ae", 8).is_empty());
        assert_eq!(file_matches(&paths, "æ", 8), vec!["æ.md", "ǽ.md"]);
    }

    #[test]
    fn at_query_only_at_end() {
        assert_eq!(at_query("see @noon later"), None);
        assert_eq!(at_query("look at @readme"), Some("readme".into()));
        assert_eq!(at_query("@"), Some("".into()));
        assert_eq!(at_query("hi @"), Some("".into()));
    }

    #[test]
    fn without_at_keeps_space() {
        assert_eq!(without_at_query("hi @file"), "hi ");
        assert_eq!(without_at_query("@file"), "");
    }

    #[test]
    fn ranks_name_before_path() {
        let paths = vec![
            "docs/resume.md".into(),
            "resume.md".into(),
            "src/resume_old.rs".into(),
            "other/path.txt".into(),
        ];
        assert_eq!(file_matches(&paths, "resume", 8)[0], "resume.md");
        assert!(file_matches(&paths, "zzz", 8).is_empty());
    }

    #[test]
    fn accent_fold_finds_resume() {
        let paths = vec!["résumé.md".into()];
        assert_eq!(
            file_matches(&paths, "resume", 8),
            vec!["résumé.md".to_string()]
        );
    }
}
