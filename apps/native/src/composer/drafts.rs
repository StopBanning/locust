//! Per-conversation drafts — window-local Map, as Electron (`Composer.tsx`).
//! Electron does **not** persist drafts across restarts; only the queue does.

use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct Drafts {
    map: HashMap<String, String>,
    current: Option<String>,
}

impl Drafts {
    /// Switch draft key. Returns the text that should fill the box.
    /// A `pending…` key that gains a real id keeps the in-box text (Electron).
    pub fn switch(&mut self, next: Option<String>, value_now: &str) -> String {
        let before = self.current.clone();
        if before.as_deref() == next.as_deref() {
            return value_now.to_owned();
        }
        if let Some(before) = before {
            if value_now.trim().is_empty() {
                self.map.remove(&before);
            } else {
                self.map.insert(before.clone(), value_now.to_owned());
            }
            self.current = next.clone();
            if before.starts_with("pending") {
                return value_now.to_owned();
            }
        } else {
            self.current = next.clone();
        }
        match next {
            None => String::new(),
            Some(key) => self.map.get(&key).cloned().unwrap_or_default(),
        }
    }

    /// Store `value` under `key` (empty clears). Does not change the current key.
    pub fn put(&mut self, key: &str, value: &str) {
        if value.trim().is_empty() {
            self.map.remove(key);
        } else {
            self.map.insert(key.to_owned(), value.to_owned());
        }
    }

    #[cfg(test)]
    #[cfg(test)]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(String::as_str)
    }
}

/// Draft key for the open conversation (Electron: root/mission id, else `"new"`).
pub fn draft_key(conversation_id: Option<&str>, teammate_id: &str) -> String {
    match conversation_id {
        Some(id) if !id.is_empty() => id.to_owned(),
        _ => format!("pending:{teammate_id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_keeps_per_key_text() {
        let mut d = Drafts::default();
        assert_eq!(d.switch(Some("a".into()), ""), "");
        assert_eq!(d.switch(Some("b".into()), "hello a"), "");
        assert_eq!(d.switch(Some("a".into()), "hello b"), "hello a");
        assert_eq!(d.switch(Some("b".into()), "changed a"), "hello b");
    }

    #[test]
    fn pending_keeps_box_when_id_arrives() {
        let mut d = Drafts::default();
        assert_eq!(d.switch(Some("pending:codec".into()), ""), "");
        let kept = d.switch(Some("real-id".into()), "still typing");
        assert_eq!(kept, "still typing");
    }

    #[test]
    fn empty_clears_stored() {
        let mut d = Drafts::default();
        d.switch(Some("a".into()), "");
        d.switch(Some("b".into()), "kept for a");
        assert_eq!(d.switch(Some("a".into()), "x"), "kept for a");
        // Clearing the box for `a`, then leaving, removes that draft.
        d.switch(Some("b".into()), "");
        assert_eq!(d.get("a"), None);
        assert_eq!(d.switch(Some("a".into()), "still on b"), "");
    }

    #[test]
    fn draft_key_pending_without_conversation() {
        assert_eq!(draft_key(None, "codec"), "pending:codec");
        assert_eq!(draft_key(Some("m1"), "codec"), "m1");
    }
}
