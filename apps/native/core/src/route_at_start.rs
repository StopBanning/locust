//! Route resolution at mission start and per-composer route selection (Electron `shared/route-at-start.ts`).
//! Keeps a teammate's saved account for CLI/editor starts and honors explicit default/account overrides
//! when the person chose them in the model picker.

use std::collections::HashMap;

const KNOWN_RUNTIMES: &[&str] = &[
    "codex",
    "claude",
    "cursor",
    "gemini",
    "opencode",
    "copilot",
    "antigravity",
    "muse",
    "kilo",
    "acp",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeammateRoute {
    pub runtime: String,
    pub model: String,
    pub mode: String,
    pub account: Option<String>,
    pub effort: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartRouteInput {
    pub runtime: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub account: Option<String>,
    pub model_choice: Option<String>,
    pub route_override_for: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartRoute {
    pub runtime: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub account: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerRoute {
    pub runtime: String,
    pub model: String,
    pub account: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartAs {
    pub teammate_id: String,
    pub route: PickerRoute,
    pub mode: String,
    pub effort: Option<String>,
    pub one_off: bool,
}

fn is_mission_runtime(runtime: &str) -> bool {
    KNOWN_RUNTIMES.contains(&runtime)
}

fn kilo_on_auto_free_opt(runtime: &str, model: Option<String>) -> Option<String> {
    if runtime != "kilo" {
        return model;
    }
    match model {
        Some(m) if m != "account-default" && !m.trim().is_empty() => Some(m),
        _ => Some(crate::runtime::kilo::AUTO_FREE_MODEL.to_owned()),
    }
}

fn kilo_on_auto_free_picker(mut route: PickerRoute) -> PickerRoute {
    if route.runtime == "kilo" && (route.model == "account-default" || route.model.trim().is_empty()) {
        route.model = crate::runtime::kilo::AUTO_FREE_MODEL.to_owned();
    }
    route
}

/// Resolves the route when a turn starts (`routeAtStart`):
/// - Keeps a saved teammate's runtime, model, account and effort when no picker override for THIS teammate was passed
///   (e.g. CLI/MCP/editor starts).
/// - Uses the requested route (including an explicit `account: None` for "This computer's sign-in") when `route_override_for`
///   names this teammate.
pub fn route_at_start(input: &StartRouteInput, teammate: Option<(&str, Option<&TeammateRoute>)>) -> StartRoute {
    let runtime = input
        .runtime
        .as_deref()
        .filter(|r| is_mission_runtime(r))
        .unwrap_or("codex")
        .to_owned();
    let requested = StartRoute {
        model: kilo_on_auto_free_opt(&runtime, input.model.clone()),
        runtime,
        effort: input.effort.clone(),
        account: input.account.clone(),
    };
    let Some((teammate_id, Some(saved_route))) = teammate else {
        return requested;
    };
    if input.route_override_for.as_deref() == Some(teammate_id) {
        return requested;
    }
    let saved_model = if saved_route.runtime == "kilo"
        && (saved_route.model == "account-default" || saved_route.model.trim().is_empty())
    {
        crate::runtime::kilo::AUTO_FREE_MODEL.to_owned()
    } else {
        saved_route.model.clone()
    };
    let choice_or_model = input.model_choice.as_deref().or(input.model.as_deref());
    if requested.runtime == saved_route.runtime && choice_or_model == Some(saved_model.as_str()) {
        return StartRoute {
            runtime: requested.runtime,
            model: requested.model.or(Some(saved_model)),
            effort: requested.effort,
            account: saved_route.account.clone(),
        };
    }
    StartRoute {
        runtime: saved_route.runtime.clone(),
        model: Some(saved_model),
        effort: saved_route.effort.clone(),
        account: saved_route.account.clone(),
    }
}

pub fn pick_key_for(conversation: Option<&str>, teammate_id: Option<&str>) -> String {
    if let Some(conv) = conversation {
        return conv.to_owned();
    }
    match teammate_id {
        Some(id) => format!("new:{id}"),
        None => "new".to_owned(),
    }
}

pub fn composer_route_for(
    fallback: &PickerRoute,
    teammate: Option<(&str, Option<&TeammateRoute>)>,
    explicit: &HashMap<String, PickerRoute>,
    key: Option<&str>,
    conversation: Option<&PickerRoute>,
) -> PickerRoute {
    if let Some(picked) = key.and_then(|k| explicit.get(k)) {
        return kilo_on_auto_free_picker(picked.clone());
    }
    if let Some(conv) = conversation {
        return kilo_on_auto_free_picker(conv.clone());
    }
    let Some((_, saved)) = teammate else {
        return kilo_on_auto_free_picker(fallback.clone());
    };
    let chosen = saved
        .map(|r| PickerRoute {
            runtime: r.runtime.clone(),
            model: r.model.clone(),
            account: r.account.clone(),
        })
        .unwrap_or_else(|| fallback.clone());
    kilo_on_auto_free_picker(chosen)
}

pub fn start_as(
    teammate_id: &str,
    saved: Option<&TeammateRoute>,
    composer_route: &PickerRoute,
    composer_mode: &str,
    composer_effort: Option<&str>,
    explicit: &HashMap<String, PickerRoute>,
) -> StartAs {
    let key = pick_key_for(None, Some(teammate_id));
    let route = composer_route_for(composer_route, Some((teammate_id, saved)), explicit, Some(&key), None);
    StartAs {
        teammate_id: teammate_id.to_owned(),
        route: PickerRoute {
            runtime: route.runtime,
            model: route.model,
            account: route.account,
        },
        mode: saved.map(|r| r.mode.clone()).unwrap_or_else(|| composer_mode.to_owned()),
        effort: match saved {
            None => composer_effort.map(str::to_owned),
            Some(r) => r.effort.clone(),
        },
        one_off: false,
    }
}

pub fn as_reviewer(mut start: StartAs, antigravity_cli: bool) -> StartAs {
    if (start.route.runtime == "antigravity" && !antigravity_cli) || start.mode == "ask" || start.mode == "plan" {
        return start;
    }
    start.mode = "ask".to_owned();
    start.one_off = true;
    start
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_saved_account_for_cli_or_editor_starts_and_an_explicit_default_for_a_picker_override() {
        let acct = "account_0123456789abcdef0123456789abcdef".to_owned();
        let teammate_route = TeammateRoute {
            runtime: "codex".into(),
            model: "gpt-5.4".into(),
            mode: "ask".into(),
            account: Some(acct.clone()),
            effort: None,
        };
        let mut explicit = HashMap::new();
        explicit.insert(
            "new:tm_1".into(),
            PickerRoute { runtime: "codex".into(), model: "gpt-5.4".into(), account: None },
        );
        let fallback = PickerRoute { runtime: "claude".into(), model: "account-default".into(), account: None };
        let composer = start_as("tm_1", Some(&teammate_route), &fallback, "ask", None, &explicit);
        assert_eq!(composer.route.account, None);
        assert_eq!(
            route_at_start(
                &StartRouteInput {
                    runtime: Some("codex".into()),
                    model: Some("gpt-5.4".into()),
                    route_override_for: Some("tm_1".into()),
                    account: composer.route.account.clone(),
                    ..Default::default()
                },
                Some(("tm_1", Some(&teammate_route))),
            )
            .account,
            None
        );
        assert_eq!(
            route_at_start(
                &StartRouteInput { runtime: Some("codex".into()), ..Default::default() },
                Some(("tm_1", Some(&teammate_route))),
            )
            .account,
            Some(acct)
        );
    }
}
