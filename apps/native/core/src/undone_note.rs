//! A teammate is told its last turn was undone (Electron `main/undone-note.ts`, 0.784). Undo puts the folder back
//! without the runtime knowing, and the session the next turn resumes still remembers writing the files; asked what it
//! had undone, a teammate answered that the file "is still on disk". So a follow-up to a turn that was undone starts
//! with what Undo did. Only the turn it follows: the next turn follows a new one, so it is said once.

use serde_json::Value;

/// At most this many paths are named; a turn that touched more is said by count.
pub const MAX_PATHS: usize = 12;

fn listed(paths: &[String]) -> String {
    let named = paths.iter().take(MAX_PATHS).cloned().collect::<Vec<_>>().join(", ");
    if paths.len() > MAX_PATHS { format!("{named} and {} more", paths.len() - MAX_PATHS) } else { named }
}

/// The note a follow-up starts with, from Undo's receipt (`{"restored": [...], "leftAlone": [{"path", "why"}]}`), or
/// None when Undo put nothing back and left nothing.
pub fn undone_note(receipt: &Value) -> Option<String> {
    let restored: Vec<String> = receipt["restored"].as_array().into_iter().flatten().filter_map(|p| p.as_str().map(str::to_owned)).collect();
    let left: Vec<String> = receipt["leftAlone"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| Some(format!("{} ({})", f["path"].as_str()?, f["why"].as_str().unwrap_or("left as it is"))))
        .collect();
    if restored.is_empty() && left.is_empty() {
        return None;
    }
    let mut lines = vec!["Before this message, the person pressed Undo on your previous turn in Locust.".to_owned()];
    if !restored.is_empty() {
        lines.push(format!("These files were put back as they were before that turn (a file it created is gone): {}.", listed(&restored)));
    }
    if !left.is_empty() {
        lines.push(format!("Left as they are: {}.", listed(&left)));
    }
    lines.push("Work from the folder as it is now, not from what that turn wrote.".into());
    Some(lines.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SAID: &str = "Before this message, the person pressed Undo on your previous turn in Locust.";

    #[test]
    fn names_what_was_put_back_and_what_was_left_and_why() {
        let note = undone_note(&json!({"restored": ["laptop-setup.md"], "leftAlone": [{"path": "notes.txt", "why": "changed since the turn"}]})).unwrap();
        assert!(note.contains(SAID) && note.contains("laptop-setup.md") && note.contains("notes.txt (changed since the turn)") && note.contains("Work from the folder as it is now"));
    }

    #[test]
    fn says_nothing_when_undo_put_nothing_back() {
        assert_eq!(undone_note(&json!({"restored": [], "leftAlone": []})), None);
        assert_eq!(undone_note(&json!({})), None);
    }

    #[test]
    fn names_a_long_list_by_count_past_its_limit() {
        let many: Vec<String> = (0..MAX_PATHS + 3).map(|i| format!("f{i}.md")).collect();
        let note = undone_note(&json!({"restored": many, "leftAlone": []})).unwrap();
        assert!(note.contains(&format!("f{}.md and 3 more", MAX_PATHS - 1)));
        assert!(!note.contains(&format!("f{MAX_PATHS}.md")));
    }
}
