//! Attachment preamble / caps — port of `shared/attachments.ts`. The words live in `locust_core::attachments`; what
//! stays here is the composer's own `attachPaths`.

pub use locust_core::attachments::{MAX_ATTACHMENTS, MAX_PASTED_BYTES, attachment_label, split_attachments, with_attachments};

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
    fn attach_paths_caps_and_notes() {
        let current: Vec<String> = (0..7).map(|i| format!("f{i}.txt")).collect();
        let added = vec!["a.txt".into(), "b.txt".into(), "c.txt".into()];
        let (kept, note) = attach_paths(&current, &added);
        assert_eq!(kept.len(), 8);
        assert!(note.unwrap().contains("not attached"));
    }
}
