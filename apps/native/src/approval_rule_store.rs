//! Saved approval rules — port of `main/approval-rule-store.ts`.
//!
//! `approval-rules.json` in the native data folder (seeded once from Electron). A file that
//! cannot be read is UNREADABLE — never empty. Electron's folder stays read-only via Store checks.

use crate::approval_rules::{ApprovalRule, ApprovalRuleDraft, RuleEffect, RuleKind};
use crate::secrets::{rule_secret_refusal, secret_in};
use crate::store::Store;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Mutex;

const SCHEMA_VERSION: u64 = 1;
const MAX_FILE_BYTES: usize = 1024 * 1024;
const FILE_NAME: &str = "approval-rules.json";

pub const MAX_RULES: usize = 200;
pub const UNREADABLE: &str =
    "The saved approval rules could not be read. Every card asks until they can.";

/// Electron `StoredRule` — same shape as [`ApprovalRule`] (uses / lastUsedAt included).
pub type StoredRule = ApprovalRule;

/// Saved approvals panel — exact UI strings from Settings > Teammates.
pub const SAVED_APPROVALS_HEADING: &str = "Saved approvals";
pub const SAVED_APPROVALS_LEDE: &str = "What a teammate may do without asking, or may never do, as you said on an approval card. Each holds for that teammate in that folder.";
pub const SAVED_APPROVALS_NOTE: &str = "A rule answers what a teammate asks you about. Some AI agents run what they judge safe without asking, and a rule cannot stop that: for a hard stop, use Ask mode.";
pub const SAVED_APPROVALS_EMPTY: &str = "None yet. On an approval card, \u{201C}Yes, and don\u{2019}t ask again\u{201D} saves one, and so does \u{201C}Never allow this\u{201D} beside Deny.";
pub const SAVED_APPROVALS_REMOVE: &str = "Remove";
pub const SAVED_APPROVALS_REMOVE_ALL_RULES: &str = "Remove all rules";
pub const SAVED_APPROVALS_REMOVE_ALL: &str = "Remove all";
pub const SAVED_APPROVALS_KEEP_THEM: &str = "Keep them";
pub const SAVED_APPROVALS_START_OVER: &str = "Start over: take every rule back at once.";

static WRITES: Mutex<()> = Mutex::new(());

fn empty_file() -> Value {
    json!({ "schemaVersion": SCHEMA_VERSION, "rules": [] })
}

fn text(value: &Value, max: usize) -> Option<String> {
    let s = value.as_str()?;
    if s.is_empty() || s.encode_utf16().count() > max {
        None
    } else {
        Some(s.to_owned())
    }
}

fn parse_rule(value: &Value) -> Option<StoredRule> {
    let rule_id = text(&value["ruleId"], 80)?;
    let pattern = text(&value["pattern"], 400)?;
    let created_at = text(&value["createdAt"], 40)?;
    let effect = RuleEffect::parse(value["effect"].as_str()?)?;
    let kind = RuleKind::parse(value["kind"].as_str()?)?;
    let teammate_id = if value.get("teammateId").is_some() {
        Some(text(&value["teammateId"], 200)?)
    } else {
        None
    };
    let folder = if value.get("folder").is_some() {
        let folder = text(&value["folder"], 1_024)?;
        if !Path::new(&folder).is_absolute() {
            return None;
        }
        Some(folder)
    } else {
        None
    };
    // Invalid uses are omitted (Electron), never make the whole rule unreadable.
    let uses = value.get("uses").and_then(|v| {
        let n = v.as_u64()?;
        (n <= 9_007_199_254_740_991).then_some(n)
    });
    // Mirror Electron: invalid optional lastUsedAt is omitted, not fatal — unless the key is
    // present and text() fails when we require a string; Electron uses text() the same way and
    // simply leaves it off. Keep that behaviour.
    let last_used_at = value.get("lastUsedAt").and_then(|v| text(v, 40));
    Some(StoredRule {
        rule_id,
        effect,
        kind,
        pattern,
        teammate_id,
        folder,
        created_at,
        uses,
        last_used_at,
    })
}

/// Parse a raw approval-rules.json body. Unreadable → exact Electron words.
pub fn parsed_rule_file(raw: &str) -> Result<Vec<StoredRule>, String> {
    let value: Value = serde_json::from_str(raw).map_err(|_| UNREADABLE.to_owned())?;
    if value.get("schemaVersion").and_then(Value::as_u64) != Some(SCHEMA_VERSION)
        || !value.get("rules").is_some_and(Value::is_array)
    {
        return Err(UNREADABLE.to_owned());
    }
    let mut rules = Vec::new();
    for row in value["rules"].as_array().ok_or_else(|| UNREADABLE.to_owned())? {
        // A rule that does not read is refused for the whole file, never half-applied.
        rules.push(parse_rule(row).ok_or_else(|| UNREADABLE.to_owned())?);
    }
    Ok(rules)
}

fn to_json(rules: &[StoredRule]) -> Value {
    json!({
        "schemaVersion": SCHEMA_VERSION,
        "rules": rules.iter().map(|r| {
            let mut row = json!({
                "ruleId": r.rule_id,
                "effect": r.effect.as_str(),
                "kind": r.kind.as_str(),
                "pattern": r.pattern,
                "createdAt": r.created_at,
            });
            if let Some(id) = &r.teammate_id {
                row["teammateId"] = json!(id);
            }
            if let Some(folder) = &r.folder {
                row["folder"] = json!(folder);
            }
            if let Some(uses) = r.uses {
                row["uses"] = json!(uses);
            }
            if let Some(at) = &r.last_used_at {
                row["lastUsedAt"] = json!(at);
            }
            row
        }).collect::<Vec<_>>()
    })
}

fn load(store: &Store) -> Result<Vec<StoredRule>, String> {
    let file = store
        .seeded_json(FILE_NAME, empty_file())
        .map_err(|_| UNREADABLE.to_owned())?;
    let raw = serde_json::to_string(&file).map_err(|_| UNREADABLE.to_owned())?;
    if raw.len() > MAX_FILE_BYTES {
        return Err(UNREADABLE.to_owned());
    }
    parsed_rule_file(&raw)
}

fn save(store: &Store, rules: &[StoredRule]) -> Result<(), String> {
    store.save_json(FILE_NAME, &to_json(rules))
}

pub fn list(store: &Store) -> Result<Vec<StoredRule>, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    load(store)
}

pub fn add(store: &Store, draft: ApprovalRuleDraft) -> Result<StoredRule, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    // Validate the draft the way Electron re-parses a candidate before saving.
    let provisional = json!({
        "ruleId": "rule_validation",
        "effect": draft.effect.as_str(),
        "kind": draft.kind.as_str(),
        "pattern": draft.pattern,
        "createdAt": "2026-01-01T00:00:00.000Z",
        "teammateId": draft.teammate_id,
        "folder": draft.folder,
    });
    // Drop null optional keys so parse_rule treats them as absent.
    let mut provisional = provisional;
    if provisional["teammateId"].is_null()
        && let Some(object) = provisional.as_object_mut() { object.remove("teammateId"); }
    if provisional["folder"].is_null()
        && let Some(object) = provisional.as_object_mut() { object.remove("folder"); }
    if parse_rule(&provisional).is_none() {
        return Err("That rule could not be saved.".into());
    }

    // A rule is the command as written, kept for good (0.870).
    if let Some(kind) = secret_in(&draft.pattern) {
        return Err(rule_secret_refusal(kind));
    }

    let mut rules = load(store)?;
    if let Some(same) = rules.iter().find(|r| {
        r.effect == draft.effect
            && r.kind == draft.kind
            && r.pattern == draft.pattern
            && r.teammate_id == draft.teammate_id
            && r.folder == draft.folder
    }) {
        return Ok(same.clone());
    }
    if rules.len() >= MAX_RULES {
        return Err(format!(
            "{MAX_RULES} rules is the most Locust keeps; remove one first."
        ));
    }
    let id = format!(
        "rule_{}",
        crate::native::uuid_v4()
            .map_err(|e| e.to_string())?
            .replace('-', "")
            .chars()
            .take(16)
            .collect::<String>()
    );
    let rule = StoredRule {
        rule_id: id,
        effect: draft.effect,
        kind: draft.kind,
        pattern: draft.pattern,
        teammate_id: draft.teammate_id,
        folder: draft.folder,
        created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        uses: None,
        last_used_at: None,
    };
    rules.push(rule.clone());
    save(store, &rules)?;
    Ok(rule)
}

pub fn remove(store: &Store, rule_id: &str) -> Result<(), String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    let rules = load(store)?;
    let next: Vec<_> = rules
        .into_iter()
        .filter(|r| r.rule_id != rule_id)
        .collect();
    save(store, &next)
}

/// Every rule at once: the file is left readable and empty. Returns how many went.
pub fn remove_all(store: &Store) -> Result<usize, String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    let rules = load(store)?;
    let count = rules.len();
    save(store, &[])?;
    Ok(count)
}

/// A card was answered by this rule.
pub fn used(store: &Store, rule_id: &str) -> Result<(), String> {
    let _lock = WRITES.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let rules = load(store)?;
    let next: Vec<_> = rules
        .into_iter()
        .map(|rule| {
            if rule.rule_id == rule_id {
                StoredRule {
                    uses: Some(rule.uses.unwrap_or(0) + 1),
                    last_used_at: Some(now.clone()),
                    ..rule
                }
            } else {
                rule
            }
        })
        .collect();
    save(store, &next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> (Temp, Store) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "locust-rules-{}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed),
            crate::native::uuid_v4().unwrap_or_else(|_| "x".into())
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("electron")).unwrap();
        let store = Store::new(path.join("native"), path.join("electron")).unwrap();
        (Temp(path), store)
    }

    fn draft(
        effect: RuleEffect,
        kind: RuleKind,
        pattern: &str,
        teammate_id: Option<&str>,
    ) -> ApprovalRuleDraft {
        ApprovalRuleDraft {
            effect,
            kind,
            pattern: pattern.into(),
            teammate_id: teammate_id.map(str::to_owned),
            folder: None,
        }
    }

    /// Built from pieces so this file holds no key-shaped literal (0.870).
    fn anthropic_key() -> String {
        ["sk", "ant", "api03", "Zq9xR2mT7vLp4WcN8bYd3KfH6sJ1gQe5"].join("-")
    }

    #[test]
    fn a_command_with_a_key_is_never_saved_as_a_rule() {
        let (dir, store) = fixture();
        let key = anthropic_key();
        let err = add(
            &store,
            draft(
                RuleEffect::Allow,
                RuleKind::Command,
                &format!("curl -H \"Authorization: Bearer {key}\" https://x.test"),
                None,
            ),
        )
        .unwrap_err();
        assert!(
            err.contains("holds")
                && err.contains("so it isn't saved as a rule")
                && err.contains("Allow it once instead."),
            "{err}"
        );
        assert!(list(&store).unwrap().is_empty());

        add(
            &store,
            draft(RuleEffect::Allow, RuleKind::Command, "npm test", None),
        )
        .unwrap();
        let saved = fs::read_to_string(dir.0.join("native").join(FILE_NAME)).unwrap();
        assert!(saved.contains("npm test"));
        assert!(!saved.contains(&key));
        assert!(!dir.0.join("electron").join(FILE_NAME).exists());
    }

    #[test]
    fn every_saved_rule_can_go_at_once() {
        let (dir, store) = fixture();
        add(
            &store,
            draft(RuleEffect::Allow, RuleKind::Command, "git status", None),
        )
        .unwrap();
        add(
            &store,
            draft(
                RuleEffect::Deny,
                RuleKind::Edit,
                "package.json",
                Some("tm_wren"),
            ),
        )
        .unwrap();
        assert_eq!(list(&store).unwrap().len(), 2);

        assert_eq!(remove_all(&store).unwrap(), 2);
        assert!(list(&store).unwrap().is_empty());
        let raw = fs::read_to_string(dir.0.join("native").join(FILE_NAME)).unwrap();
        assert!(parsed_rule_file(&raw).unwrap().is_empty());

        let next = add(
            &store,
            draft(RuleEffect::Allow, RuleKind::Read, "README.md", None),
        )
        .unwrap();
        assert_eq!(
            list(&store)
                .unwrap()
                .iter()
                .map(|r| r.rule_id.as_str())
                .collect::<Vec<_>>(),
            vec![next.rule_id.as_str()]
        );
    }

    #[test]
    fn remove_all_on_empty_store_is_zero() {
        let (_dir, store) = fixture();
        assert_eq!(remove_all(&store).unwrap(), 0);
        assert!(list(&store).unwrap().is_empty());
    }

    fn seed_rules(native: &Path, count: usize) -> Vec<StoredRule> {
        let rules: Vec<StoredRule> = (0..count)
            .map(|i| StoredRule {
                rule_id: format!("rule_{i}"),
                effect: RuleEffect::Deny,
                kind: RuleKind::Command,
                pattern: format!("command_{i}"),
                teammate_id: None,
                folder: None,
                created_at: "2026-10-04T00:00:00.000Z".into(),
                uses: None,
                last_used_at: None,
            })
            .collect();
        fs::create_dir_all(native).unwrap();
        fs::write(
            native.join(FILE_NAME),
            serde_json::to_string_pretty(&to_json(&rules)).unwrap(),
        )
        .unwrap();
        rules
    }

    #[test]
    fn full_store_refuses_the_201st_and_keeps_file_intact() {
        let (dir, store) = fixture();
        let native = dir.0.join("native");
        let rules = seed_rules(&native, MAX_RULES);
        let path = native.join(FILE_NAME);
        let before = fs::read_to_string(&path).unwrap();

        let err = add(
            &store,
            draft(RuleEffect::Allow, RuleKind::Read, "new-file", None),
        )
        .unwrap_err();
        assert_eq!(
            err,
            "200 rules is the most Locust keeps; remove one first."
        );
        assert_eq!(list(&store).unwrap(), rules);
        assert_eq!(list(&store).unwrap()[0], rules[0]);
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn duplicate_returns_existing_even_when_full() {
        let (dir, store) = fixture();
        let native = dir.0.join("native");
        let rules = seed_rules(&native, MAX_RULES);
        let path = native.join(FILE_NAME);
        let before = fs::read_to_string(&path).unwrap();

        let same = add(
            &store,
            draft(RuleEffect::Deny, RuleKind::Command, "command_0", None),
        )
        .unwrap();
        assert_eq!(same, rules[0]);
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn reads_all_rules_already_on_disk_past_the_cap() {
        let (dir, store) = fixture();
        let native = dir.0.join("native");
        let rules = seed_rules(&native, MAX_RULES + 1);
        assert_eq!(list(&store).unwrap(), rules);
    }

    #[test]
    fn unreadable_file_is_never_treated_as_empty() {
        let (dir, store) = fixture();
        let path = dir.0.join("native").join(FILE_NAME);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{broken").unwrap();
        assert_eq!(list(&store).unwrap_err(), UNREADABLE);
        assert_eq!(fs::read_to_string(&path).unwrap(), "{broken");
    }
}
