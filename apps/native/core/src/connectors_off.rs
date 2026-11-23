//! Pre-send warnings in the composer:
//! 1. `connectorsOff.ts`: when a Claude Code teammate has no ticked connectors and the draft names one.
//! 2. `status.ts` (`droppedFreeModel`, `freeStartModel`): when a route still points at a free OpenCode model
//!    that OpenCode no longer lists.

use crate::engine::Model;
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub use crate::agents::FREE_START_RUNTIME;

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

/// A run as `free_models_that_answered` reads it: its agent, its model, how it ended ("completed", "failed",
/// "cancelled", "running"), and when.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunSeen<'a> {
    pub runtime: &'a str,
    pub model: &'a str,
    pub phase: &'a str,
    pub at: &'a str,
}

/// `freeModelsThatAnswered`: the OpenCode free models that finished a run here, newest first, each once. A run that
/// failed or was stopped, or was on a paid model or another agent, does not count: answering is what earns the spot.
pub fn free_models_that_answered(runs: &[RunSeen<'_>]) -> Vec<String> {
    let mut answered: Vec<&RunSeen<'_>> = runs
        .iter()
        .filter(|r| r.runtime == FREE_START_RUNTIME && r.phase == "completed" && r.model.starts_with("opencode/") && r.model.ends_with("-free"))
        .collect();
    answered.sort_by(|a, b| b.at.cmp(a.at));
    let mut out: Vec<String> = Vec::new();
    for run in answered {
        if !out.iter().any(|m| m == run.model) {
            out.push(run.model.to_owned());
        }
    }
    out
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

/// Retired before anyone here ran it (`missionView.ts` `KNOWN_RETIRED_MODELS`): a new person's first press must not
/// find out the hard way.
pub const KNOWN_RETIRED_MODELS: &[&str] = &["opencode/exo-free", "opencode/fledge-alpha-free"];

/// `missionView.ts` `modelRetired`: OpenCode's "has been deprecated" or "Model not found: <id>".
pub fn model_retired(message: &str) -> bool {
    crate::invariants::regex(r"(?i)\bmodel \S+ has been deprecated\b|\bmodel not found: \S+").is_match(message)
}

/// `missionView.ts` `modelUnavailable`: provider endpoint/model unavailable or free model refused.
pub fn model_unavailable(message: &str) -> bool {
    crate::invariants::regex(r"(?i)\b(?:model|endpoint) is unavailable\b|\brefused the free model\b").is_match(message)
}

/// One mission as `retired_free_models` reads it (`missionView.ts` `retiredFreeModels`).
#[derive(Clone, Copy, Debug)]
pub struct RetiredMission<'a> {
    pub runtime: &'a str,
    pub model: &'a str,
    pub phase: &'a str,
    pub last_updated_at: &'a str,
    pub events: &'a [Value],
}

/// The OpenCode models to leave out of the window's list (`missionView.ts` `retiredFreeModels`): the known ones,
/// and any whose latest run here ended on OpenCode's retired/not-found error -- unless a run on it finished after
/// that, which says it is back.
pub fn retired_free_models(missions: &[RetiredMission<'_>]) -> HashSet<String> {
    let mut retired_at: HashMap<String, i64> = KNOWN_RETIRED_MODELS
        .iter()
        .map(|&m| (m.to_owned(), 0))
        .collect();
    let mut answered_at: HashMap<String, i64> = HashMap::new();
    for mission in missions {
        if mission.runtime != FREE_START_RUNTIME {
            continue;
        }
        let stamp = chrono::DateTime::parse_from_rfc3339(mission.last_updated_at)
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0);
        if mission.phase == "completed" {
            let prev = answered_at.get(mission.model).copied().unwrap_or(-1);
            answered_at.insert(mission.model.to_owned(), prev.max(stamp));
            continue;
        }
        if mission.phase != "failed" {
            continue;
        }
        let Some(failed) = mission.events.iter().rev().find(|e| e["type"] == "run.failed") else {
            continue;
        };
        let msg = failed["payload"]["message"].as_str().unwrap_or("");
        if !model_retired(msg) {
            continue;
        }
        let prev = retired_at.get(mission.model).copied().unwrap_or(-1);
        retired_at.insert(mission.model.to_owned(), prev.max(stamp));
    }
    retired_at
        .into_iter()
        .filter(|(model, at)| answered_at.get(model).copied().unwrap_or(-1) <= *at)
        .map(|(model, _)| model)
        .collect()
}

/// Why a failed free-model offer is shown under a thread (`Thread.tsx` `limitModel.why`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitModelWhy {
    Retired,
    Down,
    Limit,
}

/// The sentence shown under a failed free model (`Thread.tsx` `limitModel` offer).
pub fn limit_model_offer_sentence(why: Option<LimitModelWhy>) -> &'static str {
    match why {
        Some(LimitModelWhy::Retired) => {
            "OpenCode no longer offers this model. Switching puts your message back in the chat box, to send there."
        }
        Some(LimitModelWhy::Down) => {
            "This model\u{2019}s provider is down for now. Switching puts your message back in the chat box, to send there."
        }
        Some(LimitModelWhy::Limit) | None => {
            "This model is at its limit. Switching puts your message back in the chat box, to send there."
        }
    }
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

    // --- a-model-opencode-cannot-find-is-retired.test.tsx

    const SAID: &str = "OpenCode stopped: Model not found: opencode/fledge-alpha-free. Did you mean: exo-free, ling-3.0-flash-fin-free, ling-3.1-flash-free?";

    fn catalog() -> Vec<Model> {
        [
            "opencode/exo-free",
            "opencode/fledge-alpha-free",
            "opencode/ling-3.0-flash-fin-free",
            "opencode/beam-free",
        ]
        .into_iter()
        .map(m)
        .collect()
    }

    fn shown(retired: &HashSet<String>) -> Vec<Model> {
        catalog().into_iter().filter(|entry| !retired.contains(&entry.id)).collect()
    }

    // case: reads "Model not found" as retired, and only that
    #[test]
    fn reads_model_not_found_as_retired_and_only_that() {
        assert!(model_retired(SAID));
        assert!(!model_unavailable(SAID));
        assert!(!model_retired("Error: ENOENT: file not found: notes.md"));
        assert!(!model_retired("OpenCode stopped: Upstream request failed: Model is unavailable."));
        assert!(model_unavailable("OpenCode stopped: Upstream request failed: Model is unavailable."));
    }

    // case: a new person never starts on Fledge Alpha: left out before anyone here has run it
    #[test]
    fn a_new_person_never_starts_on_fledge_alpha_left_out_before_anyone_here_has_run_it() {
        let retired = retired_free_models(&[]);
        assert!(retired.contains("opencode/fledge-alpha-free"));
        let visible = shown(&retired);
        assert!(!visible.iter().any(|entry| entry.id == "opencode/fledge-alpha-free"));
        assert_eq!(
            free_start_model("opencode", &visible, &[], &HashSet::new()).as_deref(),
            Some("opencode/beam-free")
        );
        let without_beam: Vec<Model> = visible
            .into_iter()
            .filter(|entry| !entry.id.starts_with("opencode/beam"))
            .collect();
        assert_eq!(
            free_start_model("opencode", &without_beam, &[], &HashSet::new()).as_deref(),
            Some("opencode/ling-3.0-flash-fin-free")
        );
    }

    // case: the next model OpenCode drops is learned from its first failure, and is back once it answers
    #[test]
    fn the_next_model_opencode_drops_is_learned_from_its_first_failure_and_is_back_once_it_answers() {
        let lost = "OpenCode stopped: Model not found: opencode/beam-free. Did you mean: ling-3.1-flash-free?";
        let failed_events = vec![serde_json::json!({"type": "run.failed", "payload": {"message": lost}})];
        let failed = RetiredMission {
            runtime: "opencode",
            model: "opencode/beam-free",
            phase: "failed",
            last_updated_at: "2026-10-10T01:00:00Z",
            events: &failed_events,
        };
        assert!(retired_free_models(&[failed]).contains("opencode/beam-free"));
        let completed = RetiredMission {
            runtime: "opencode",
            model: "opencode/beam-free",
            phase: "completed",
            last_updated_at: "2026-10-11T01:00:00Z",
            events: &[],
        };
        assert!(!retired_free_models(&[failed, completed]).contains("opencode/beam-free"));
    }

    // case: a retired model is not "at its limit"
    #[test]
    fn a_retired_model_is_not_at_its_limit() {
        let sentence = limit_model_offer_sentence(Some(LimitModelWhy::Retired));
        assert!(sentence.contains("OpenCode no longer offers this model."));
        assert!(!sentence.contains("at its limit"));
        let label = format!("Switch to {}", opencode_model_display_name("opencode/ling-3.0-flash-fin-free"));
        assert_eq!(label, "Switch to Ling 3.0 Flash Fin Free");
    }

    // case: a provider that is down says so; a limit, or no reason given, stays a limit
    #[test]
    fn a_provider_that_is_down_says_so_a_limit_or_no_reason_given_stays_a_limit() {
        assert!(limit_model_offer_sentence(Some(LimitModelWhy::Down)).contains("provider is down for now."));
        assert!(limit_model_offer_sentence(Some(LimitModelWhy::Limit)).contains("This model is at its limit."));
        assert!(limit_model_offer_sentence(None).contains("This model is at its limit."));
    }
}
