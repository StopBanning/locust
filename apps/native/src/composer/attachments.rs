//! Attachment preamble / caps — port of `shared/attachments.ts`.

pub const MAX_ATTACHMENTS: usize = 8;
pub const MAX_PASTED_BYTES: usize = 24 * 1024 * 1024;

pub fn attachment_preamble(paths: &[String]) -> Option<String> {
    if paths.is_empty() {
        return None;
    }
    let listed = paths
        .iter()
        .map(|path| {
            if path.chars().any(|c| c <= '\u{001f}' || c == '\u{007f}') {
                format!(
                    "- {}",
                    serde_json::to_string(path).unwrap_or_else(|_| path.clone())
                )
            } else {
                format!("- {path}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(if paths.len() == 1 {
        format!("Read this file in the workspace before you answer:\n{listed}")
    } else {
        format!(
            "Read these {} files in the workspace before you answer:\n{listed}",
            paths.len()
        )
    })
}

pub fn with_attachments(prompt: &str, paths: &[String]) -> String {
    match attachment_preamble(paths) {
        None => prompt.to_owned(),
        Some(preamble) => format!("{preamble}\n\n{prompt}"),
    }
}

/// Split an attachment preamble from the person's text (Electron `splitAttachments`).
/// Matches `/^Read th(?:is file|ese \d+ files) in the workspace before you answer:\n((?:- [^\n]+\n?)+)\n([\s\S]+)$/`.
pub fn split_attachments(prompt: &str) -> (String, Vec<String>) {
    let rest = if let Some(after) = prompt.strip_prefix("Read this file in the workspace before you answer:\n") {
        after
    } else if let Some(after) = prompt.strip_prefix("Read these ") {
        let Some((n, after)) = after.split_once(" files in the workspace before you answer:\n") else {
            return (prompt.to_owned(), Vec::new());
        };
        if n.is_empty() || !n.chars().all(|c| c.is_ascii_digit()) {
            return (prompt.to_owned(), Vec::new());
        }
        after
    } else {
        return (prompt.to_owned(), Vec::new());
    };
    // ((?:- [^\n]+\n?)+)\n([\s\S]+)$ — list lines, then a separating newline, then the body.
    let mut listed = Vec::new();
    let mut at = 0;
    while at < rest.len() {
        if !rest[at..].starts_with("- ") {
            break;
        }
        let after_dash = at + 2;
        let line_end = rest[after_dash..].find('\n').map(|i| after_dash + i).unwrap_or(rest.len());
        if line_end == after_dash {
            break;
        }
        listed.push(rest[after_dash..line_end].to_owned());
        at = line_end;
        if rest[at..].starts_with('\n') {
            at += 1;
        }
    }
    // Required separating newline after the list group (withAttachments inserts preamble\n\nbody).
    if listed.is_empty() || !rest[at..].starts_with('\n') {
        return (prompt.to_owned(), Vec::new());
    }
    let body = &rest[at + 1..];
    if body.is_empty() {
        (prompt.to_owned(), Vec::new())
    } else {
        (body.to_owned(), listed)
    }
}

pub fn attachment_label(count: usize) -> String {
    if count == 1 {
        "1 file".into()
    } else {
        format!("{count} files")
    }
}

/// Cap + dedupe paths the way Composer.attachPaths does. Returns (kept, note).
pub fn attach_paths(current: &[String], added: &[String]) -> (Vec<String>, Option<String>) {
    let mut all = Vec::new();
    for path in current.iter().chain(added.iter()) {
        if !all.iter().any(|p| p == path) {
            all.push(path.clone());
        }
    }
    let left = all.len().saturating_sub(MAX_ATTACHMENTS);
    let note = if left > 0 {
        let names = all[MAX_ATTACHMENTS..]
            .iter()
            .map(|p| p.rsplit('/').next().unwrap_or(p.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "A message carries {MAX_ATTACHMENTS} files at most, so {} not attached: {names}.",
            if left == 1 {
                "one was".to_string()
            } else {
                format!("{left} were")
            }
        ))
    } else {
        None
    };
    all.truncate(MAX_ATTACHMENTS);
    (all, note)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preamble_one_and_many() {
        let one = attachment_preamble(&["NOTES.md".into()]).unwrap();
        assert!(one.starts_with("Read this file in the workspace before you answer:"));
        let many = attachment_preamble(&["a.md".into(), "b.md".into()]).unwrap();
        assert!(many.starts_with("Read these 2 files"));
    }

    #[test]
    fn attach_paths_caps_and_notes() {
        let current: Vec<String> = (0..7).map(|i| format!("f{i}.txt")).collect();
        let added = vec!["a.txt".into(), "b.txt".into(), "c.txt".into()];
        let (kept, note) = attach_paths(&current, &added);
        assert_eq!(kept.len(), 8);
        assert!(note.unwrap().contains("not attached"));
    }

    #[test]
    fn split_attachments_strips_preamble_like_electron() {
        let one = with_attachments("Fix the cart", &["NOTES.md".into()]);
        assert_eq!(split_attachments(&one), ("Fix the cart".into(), vec!["NOTES.md".into()]));
        let many = with_attachments("and test", &["a.md".into(), "b.md".into()]);
        assert_eq!(
            split_attachments(&many),
            ("and test".into(), vec!["a.md".into(), "b.md".into()])
        );
        assert_eq!(split_attachments("plain ask"), ("plain ask".into(), Vec::new()));
        // A preamble with nothing under it is not one of ours.
        assert_eq!(
            split_attachments("Read this file in the workspace before you answer:\n- x.md\n"),
            ("Read this file in the workspace before you answer:\n- x.md\n".into(), Vec::new())
        );
    }
}
