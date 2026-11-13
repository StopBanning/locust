//! Pre-send warnings in the composer:
//! 1. `connectorsOff.ts`: when a Claude Code teammate has no ticked connectors and the draft names one.
//! 2. `status.ts` (`droppedFreeModel`, `freeStartModel`): when a route still points at a free OpenCode model
//!    that OpenCode no longer lists.

use crate::engine::Model;
use regex::Regex;
use std::collections::HashSet;

pub const FREE_START_RUNTIME: &str = "opencode";

/// OpenCode's free models that answered, steadiest first (`shared/free-models.ts`).
pub const FREE_MODELS_BEST_FIRST: &[&str] = &[
    "opencode/deepseek-v4.1-flash-free",
    "opencode/nemotron-3.5-lightning-free",
    "opencode/nemotron-3.5-super-vl-free",
    "opencode/glm-5.3-flash-free",
    "opencode/mimo-v2.6-flash-free",
    "opencode/beam-free",
];

/// Human-readable connector name (`NewTeammateDialog.tsx` `connectorLabel`).
pub fn connector_label(name: &str) -> String {
    let bare = name.strip_prefix("claude.ai ").unwrap_or(name);
    let plugin_re = crate::invariants::regex(r"^plugin:[^:]+:(.+)$");
    let Some(caps) = plugin_re.captures(bare) else {
        return bare.to_owned();
    };
    let tail = caps.get(1).map_or(bare, |m| m.as_str());
    tail.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// First connector named in `text` as a whole token (`connectorsOff.ts` `connectorMentioned`).
pub fn connector_mentioned(text: &str, names: &[String]) -> Option<String> {
    let lower = text.to_lowercase();
    for name in names {
        let label = connector_label(name);
        if label.encode_utf16().count() < 3 {
            continue;
        }
        let escaped = regex::escape(&label.to_lowercase());
        let pattern = format!(r"(?:^|[^a-z0-9]){escaped}(?:$|[^a-z0-9])");
        if let Ok(re) = Regex::new(&pattern)
            && re.is_match(&lower)
        {
            return Some(label);
        }
    }
    None
}

/// What the box says, in the person's words: whose, which, and where to turn it on (`connectorsOffHint`).
pub fn connectors_off_hint(teammate: &str, label: &str) -> String {
    format!(
        "{teammate}'s connectors are off, so {label} won't be in this turn. Tick it in {teammate}'s settings, under Connectors, to turn it on."
    )
}

/// The listed free models of OpenCode, `FREE_MODELS_BEST_FIRST` first, then the rest as listed.
pub fn free_models_best_first(models: &[Model]) -> Vec<&Model> {
    let rank = |id: &str| -> usize {
        FREE_MODELS_BEST_FIRST
            .iter()
            .position(|&known| known == id)
            .unwrap_or(FREE_MODELS_BEST_FIRST.len())
    };
    let mut ranked: Vec<(usize, usize, &Model)> = models
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.id.starts_with("own-") && m.id.ends_with("-free"))
        .map(|(idx, m)| (rank(&m.id), idx, m))
        .collect();
    ranked.sort_by_key(|&(r, idx, _)| (r, idx));
    ranked.into_iter().map(|(_, _, m)| m).collect()
}

/// True once the OpenCode catalogue is known (`models` non-empty for free models) and `model` is an
/// `opencode/...-free` model that OpenCode no longer lists (`droppedFreeModel`).
pub fn dropped_free_model(runtime: &str, model: Option<&str>, opencode_models: &[Model]) -> bool {
    if runtime != FREE_START_RUNTIME {
        return false;
    }
    let Some(model_id) = model else {
        return false;
    };
    if !model_id.starts_with("opencode/") || !model_id.ends_with("-free") {
        return false;
    }
    let listed = free_models_best_first(opencode_models);
    !listed.is_empty() && !listed.iter().any(|m| m.id == model_id)
}

/// Best listed OpenCode free model to switch to (`freeStartModel`).
pub fn free_start_model(
    runtime: &str,
    opencode_models: &[Model],
    answered: &[String],
    down: &HashSet<String>,
) -> Option<String> {
    if runtime != FREE_START_RUNTIME {
        return None;
    }
    let listed = free_models_best_first(opencode_models);
    let up: Vec<&&Model> = listed.iter().filter(|m| !down.contains(&m.id)).collect();
    for id in answered {
        if up.iter().any(|m| m.id == *id) {
            return Some(id.clone());
        }
    }
    up.first()
        .map(|m| m.id.clone())
        .or_else(|| listed.first().map(|m| m.id.clone()))
}

/// Title-cased display name for an OpenCode model id, matching `routeName.ts` `modelDisplayName`.
pub fn opencode_model_display_name(model_id: &str) -> String {
    let bare = model_id.strip_prefix("opencode/").unwrap_or(model_id);
    bare.split('-')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str) -> Model {
        Model {
            id: id.into(),
            name: id.into(),
            efforts: Vec::new(),
        }
    }

    #[test]
    fn finds_connector_named_in_draft_and_ignores_substrings() {
        let names = vec![
            "claude.ai Gmail".to_owned(),
            "claude.ai Google Calendar".to_owned(),
            "plugin:small-business:zoho-projects".to_owned(),
        ];
        assert_eq!(
            connector_mentioned("can you check my gmail for the invoice", &names).as_deref(),
            Some("Gmail")
        );
        assert_eq!(
            connector_mentioned("Add it to Google Calendar", &names).as_deref(),
            Some("Google Calendar")
        );
        assert_eq!(
            connector_mentioned("update zoho projects", &names).as_deref(),
            Some("Zoho Projects")
        );
        assert_eq!(connector_mentioned("gmailx is a typo", &names), None);
        assert_eq!(connector_mentioned("fix the login bug", &names), None);
    }

    #[test]
    fn connectors_off_hint_matches_electron_words() {
        assert_eq!(
            connectors_off_hint("Wren", "Gmail"),
            "Wren's connectors are off, so Gmail won't be in this turn. Tick it in Wren's settings, under Connectors, to turn it on."
        );
    }

    #[test]
    fn dropped_free_model_matches_electron_rules() {
        let listed = vec![
            m("opencode/deepseek-v4.1-flash-free"),
            m("opencode/glm-5.3-flash-free"),
            m("opencode/big-pickle"),
        ];
        assert!(dropped_free_model(
            "opencode",
            Some("opencode/longcat-2.5-preview-free"),
            &listed
        ));
        assert!(!dropped_free_model(
            "opencode",
            Some("opencode/deepseek-v4.1-flash-free"),
            &listed
        ));
        // Before the catalogue is read, nothing is said.
        assert!(!dropped_free_model(
            "opencode",
            Some("opencode/longcat-2.5-preview-free"),
            &[]
        ));
        // A paid model, one of the person's own, another runtime's, or no model at all: theirs to keep.
        assert!(!dropped_free_model(
            "opencode",
            Some("opencode/gone-paid"),
            &listed
        ));
        assert!(!dropped_free_model(
            "opencode",
            Some("mine/cheap-free"),
            &listed
        ));
        assert!(!dropped_free_model(
            "claude",
            Some("opencode/longcat-2.5-preview-free"),
            &listed
        ));
        assert!(!dropped_free_model("opencode", None, &listed));
        assert_eq!(
            opencode_model_display_name("opencode/longcat-2.5-preview-free"),
            "Longcat 2.5 Preview Free"
        );
        assert_eq!(
            free_start_model("opencode", &listed, &[], &HashSet::new()).as_deref(),
            Some("opencode/deepseek-v4.1-flash-free")
        );
        assert_eq!(
            opencode_model_display_name("opencode/deepseek-v4.1-flash-free"),
            "Deepseek V4.1 Flash Free"
        );
    }
}
