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

/// `routeName.ts` `shortRuntimeName`: `Cursor`, not `Cursor Agent`. Runtimes whose name is already one word keep it.
pub fn short_runtime_name(runtime: &str) -> &str {
    match runtime {
        "codex" => "Codex",
        "claude" => "Claude",
        "cursor" => "Cursor",
        "gemini" => "Gemini",
        "copilot" => "Copilot",
        _ => crate::failure_words::runtime_display_name(runtime),
    }
}

/// A model as the chip and the routine line say it (`routeModelName`): Claude's own names, else the id spelt out with
/// its runtime's own prefix taken off ("Grok 4.7 High", "Nemotron 3.5 Lightning Free"); `auto` is "Auto model".
pub fn route_model_name(runtime: &str, model: &str) -> String {
    route_model_name_earned(runtime, model, None)
}

/// `routeName.ts` `routeModelName(runtime, model, earned)`: for Claude, what a finished run on the route reported, else
/// Claude Code's own name for the alias, else the copied table (`claudeRouteModelName`); every other runtime as above.
pub fn route_model_name_earned(runtime: &str, model: &str, earned: Option<&str>) -> String {
    if runtime == "claude" {
        let listed = CATALOG.with(|c| c.borrow().claude_aliases.get(model).cloned());
        if let Some(named) = crate::models::claude_route_model_name(model, earned, listed.as_deref()) {
            return named;
        }
    }
    if runtime == "antigravity"
        && let Some(named) = crate::runtime::antigravity::antigravity_tier_name(model)
    {
        return named.to_owned();
    }
    model_display_name(runtime, model)
}

/// `routeName.ts` `routeChrome`: a route as the composer's chip says it. A model of the person's own by its name alone;
/// a name that already starts with the runtime's says it once ("Kilo Auto Free"); the account's own default is the
/// runtime's name alone ("Codex"), keeping what follows it ("Codex · Medium"); else runtime, `separator`, model.
pub fn route_chrome(runtime: &str, model_id: &str, model: &str, separator: &str) -> String {
    let own = model_id.strip_prefix("own-").and_then(|rest| rest.split_once('/')).is_some_and(|(hex, name)| {
        hex.len() == 8 && hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) && !name.is_empty()
    });
    if own {
        return model.to_owned();
    }
    let short = short_runtime_name(runtime);
    if model.to_lowercase().starts_with(&format!("{} ", short.to_lowercase())) {
        return model.to_owned();
    }
    if model_id == "account-default" {
        let lower = model.to_lowercase();
        let rest = ["account default", "default"]
            .iter()
            .find(|head| lower.starts_with(*head))
            .map_or(model, |head| {
                let after = &model[head.len()..];
                after.strip_prefix(" · ").unwrap_or(after)
            });
        return if rest.is_empty() { short.to_owned() } else { format!("{short}{separator}{rest}") };
    }
    format!("{short}{separator}{model}")
}

/// What the catalogues read last say (`routeName.ts` `rememberOwnModels`): each runtime's own spelling of a model, and
/// the name Claude Code gives each alias. Kept per thread, as Electron keeps it per renderer: the window's thread reads
/// the catalogues and draws the names.
#[derive(Default)]
struct Catalog {
    spellings: std::collections::HashMap<String, String>,
    claude_aliases: std::collections::HashMap<String, String>,
}

thread_local! {
    static CATALOG: std::cell::RefCell<Catalog> = std::cell::RefCell::new(Catalog::default());
}

/// `rememberOwnModels`: every model the catalogues list, as (runtime, id, display name). A name that is the id, or the
/// id without its provider (`nemotron-3-ultra-free`), is an identifier, not a spelling, and is never taken; nor is
/// `account-default`'s label. A Claude alias's name is what Claude Code says it means today.
pub fn remember_own_models<'a>(models: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>) {
    let mut fresh = Catalog::default();
    for (runtime, id, name) in models {
        if runtime == "claude" && !id.starts_with("claude-") && id != crate::models::ACCOUNT_DEFAULT {
            fresh.claude_aliases.insert(id.to_owned(), name.to_owned());
        }
        if id != crate::models::ACCOUNT_DEFAULT && name != id && !id.ends_with(&format!("/{name}")) {
            fresh.spellings.insert(format!("{runtime}:{id}"), name.to_owned());
        }
    }
    CATALOG.with(|c| *c.borrow_mut() = fresh);
}

/// `sameName`: two spellings of one name differ only in case, hyphens and spaces.
fn same_name(name: &str) -> String {
    name.to_lowercase().split(|c: char| c.is_whitespace() || c == '-').filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// `routeName.ts` `modelDisplayName`: an id spelt out with no alias read (`claude`/`sonnet` is "Sonnet"); the chip's
/// `route_model_name` and the hover both end here.
pub fn model_display_name(runtime: &str, model: &str) -> String {
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
    let theirs = CATALOG.with(|c| c.borrow().spellings.get(&format!("{runtime}:{model}")).cloned());
    // Kilo names every model it lists, and the names are names, not descriptions: the chip says what the picker says.
    if runtime == "kilo"
        && let Some(kilos) = theirs.clone()
    {
        return kilos;
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
    let spelled = spell(bare);
    // The runtime's own spelling of the SAME name is the maker's styling ("GPT-6-Luna"); a different name never is.
    match theirs {
        Some(theirs) if same_name(&theirs) == same_name(&spelled) => theirs,
        _ => spelled,
    }
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
            let name = if let Some(tier) = (runtime == "antigravity").then(|| crate::runtime::antigravity::antigravity_tier_name(model)).flatten() { tier.to_owned() }
                else { model_display_name(runtime, model) };
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
