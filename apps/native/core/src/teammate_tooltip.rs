//! The action, person, role and recorded route on a teammate's hover.
use crate::engine::{Teammate, runtime_name};

fn spell(id: &str) -> String {
    let tokens: Vec<_> = id.split('-').collect();
    let short_number = |s: &str| !s.is_empty() && s.len() <= 2 && s.bytes().all(|c| c.is_ascii_digit());
    let mut out = String::new();
    for (i, token) in tokens.iter().enumerate() {
        let minor = i >= 2 && short_number(token) && short_number(tokens[i - 1])
            && !tokens[i - 2].bytes().all(|c| c.is_ascii_digit())
            && !tokens.get(i + 1).is_some_and(|s| short_number(s));
        if i > 0 {
            out.push(if minor { '.' } else if matches!(tokens[i-1].to_ascii_lowercase().as_str(), "gpt" | "cli" | "ai")
                && token.starts_with(|c: char| c.is_ascii_digit()) { '-' } else { ' ' });
        }
        let lower = token.to_ascii_lowercase();
        out.push_str(&match lower.as_str() {
            "gpt" | "cli" | "ai" | "glm" | "vl" => token.to_ascii_uppercase(),
            "deepseek" => "DeepSeek".into(), "mimo" => "MiMo".into(), "minimax" => "MiniMax".into(),
            _ => { let mut c = token.chars(); c.next().map(|f| format!("{}{}", f.to_uppercase(), c.as_str())).unwrap_or_default() }
        });
    }
    out
}

/// A model as the chip and the routine line say it (`routeModelName`): Claude's own names, else the id spelt out with
/// its runtime's own prefix taken off ("Grok 4.7 High", "Nemotron 3.5 Lightning Free"); `auto` is "Auto model".
pub fn route_model_name(runtime: &str, model: &str) -> String {
    if runtime == "claude" {
        // claudeRouteModelName: a bare alias by the version it means today (`opus` is "Opus 5.5").
        let meant = crate::models::CLAUDE_ALIAS_DEFAULTS.iter().find(|(alias, _)| *alias == model).map_or(model, |(_, id)| *id);
        if let Some(named) = crate::models::claude_model_name(meant) {
            return named;
        }
    }
    if runtime == "antigravity"
        && let Some(named) = crate::runtime::antigravity::antigravity_tier_name(model)
    {
        return named.to_owned();
    }
    // A model of the person's own (`own-1a2b3c4d/acme-70b`) by its own name.
    if let Some(rest) = model.strip_prefix("own-")
        && let Some((hex, name)) = rest.split_once('/')
        && hex.len() == 8
        && hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && !name.is_empty()
    {
        return spell(name);
    }
    let bare = model.strip_prefix(&format!("{runtime}/")).unwrap_or(model);
    if bare.trim().eq_ignore_ascii_case("auto") {
        return "Auto model".into();
    }
    // modelDisplayName: an id with a provider is split at its FIRST slash only, each side spelt out.
    if let Some(at) = bare.find('/').filter(|at| *at > 0) {
        return format!("{}/{}", spell(&bare[..at]), spell(&bare[at + 1..]));
    }
    let prefix = format!("{runtime}-");
    let bare = match bare.get(..prefix.len()) {
        Some(head) if bare.len() > prefix.len() && head.eq_ignore_ascii_case(&prefix) => &bare[prefix.len()..],
        _ => bare,
    };
    spell(bare)
}

pub fn teammate_tooltip(teammate: &Teammate) -> String {
    let mut said = format!("Message {} · {}", teammate.name, teammate.role);
    if let (Some(runtime), Some(model)) = (teammate.runtime.as_deref(), teammate.model.as_deref()) {
        if !matches!(runtime, "claude" | "codex" | "cursor" | "copilot" | "opencode" | "antigravity" | "muse" | "kilo" | "acp" | "gemini" | "apple") {
            return format!("{said} · {runtime} / {model}");
        }
        let runtime_label = match runtime { "claude" => "Claude", "copilot" => "Copilot", _ => runtime_name(runtime) };
        let route = if model == "account-default" { runtime_label.to_owned() } else {
            // Strip a provider/prefix only when it repeats this runtime. Antigravity's tiers read by their names
            // (antigravity-tiers-are-named-once); everything else keeps its whole id, as Electron's modelDisplayName.
            let bare = model.strip_prefix(&format!("{runtime}/")).unwrap_or(model);
            let bare = bare.strip_prefix(&format!("{runtime}-")).unwrap_or(bare);
            let name = if let Some(tier) = (runtime == "antigravity").then(|| crate::runtime::antigravity::antigravity_tier_name(model)).flatten() { tier.to_owned() }
                else if bare.eq_ignore_ascii_case("auto") { "Auto model".into() }
                else { bare.split('/').map(spell).collect::<Vec<_>>().join("/") };
            if name.to_lowercase().starts_with(&format!("{} ", runtime_label.to_lowercase())) { name }
            else { format!("{runtime_label} / {name}") }
        };
        said.push_str(" · "); said.push_str(&route);
    }
    said
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mate(name: &str, role: &str, route: Option<(&str, &str)>) -> Teammate {
        Teammate { id: "tm_test".into(), name: name.into(), role: role.into(),
            runtime: route.map(|r| r.0.into()), model: route.map(|r| r.1.into()),
            account: None, busy: false, runs_in: String::new(), folder: None }
    }
    #[test]
    fn gives_the_name_the_role_and_the_model() {
        assert_eq!(teammate_tooltip(&mate("Wren", "Code & Migrations", Some(("cursor", "cursor-grok-4.6-medium")))), "Message Wren · Code & Migrations · Cursor / Grok 4.6 Medium");
    }
    #[test]
    fn names_the_model_as_the_chip_does_never_by_its_id() {
        assert_eq!(teammate_tooltip(&mate("Ash", "Custom", Some(("opencode", "opencode/nemotron-3.5-lightning-free")))), "Message Ash · Custom · OpenCode / Nemotron 3.5 Lightning Free");
        assert_eq!(teammate_tooltip(&mate("Wren", "Custom", Some(("codex", "account-default")))), "Message Wren · Custom · Codex");
    }
    #[test]
    fn stops_after_the_role_when_the_teammate_has_never_run() {
        let said = teammate_tooltip(&mate("Gem", "Research & Briefs", None));
        assert_eq!(said, "Message Gem · Research & Briefs"); assert!(!said.contains('/'));
    }
    #[test]
    fn still_leads_with_the_action_so_a_click_is_predictable() {
        assert!(teammate_tooltip(&mate("Wren", "Custom", None)).starts_with("Message Wren"));
    }
    #[test]
    fn names_the_runtime_the_way_the_rest_of_the_app_does() {
        assert!(teammate_tooltip(&mate("A", "Custom", Some(("opencode", "x")))).contains("OpenCode"));
    }
}
