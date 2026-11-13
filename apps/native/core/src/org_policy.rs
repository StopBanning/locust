//! An organization's machine policy (Electron `shared/organization-policy.ts`, `main/organization-policy.ts`,
//! `main/organization-spend.ts`).
//!
//! An administrator's `policy.json` in the machine-wide folder can limit which AI agents and models run, the highest
//! mode, a monthly spending cap, and a few features. It is read, never created or written. A broken file is a
//! closed gate, never an empty policy: no turn starts until it is fixed. Every turn crosses it before an agent is
//! discovered, started or recorded.

use serde_json::{Map, Value, json};
use std::path::PathBuf;
use std::sync::Mutex;

pub const POLICY_RUNTIME_IDS: [&str; 11] = ["codex", "claude", "cursor", "gemini", "opencode", "copilot", "antigravity", "muse", "kilo", "omniroute", "acp"];
/// Increasing authority: read, plan, ask before actions, edit without asking, unrestricted.
pub const POLICY_MODE_ORDER: [&str; 5] = ["ask", "plan", "approve-each", "accept-edits", "auto"];
const KEYS: [&str; 12] = ["version", "allowedAgents", "allowedModels", "highestMode", "monthlyLimitUsd", "otherApps", "phone", "chatApps", "cloud", "terminal", "browser", "accounts"];
const FEATURES: [&str; 7] = ["otherApps", "phone", "chatApps", "cloud", "terminal", "browser", "accounts"];

/// A launch-time snapshot. An error is a closed gate, never an empty policy.
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyState {
    pub path: String,
    pub present: bool,
    /// The settings, validated: only known keys, each well formed.
    pub policy: Map<String, Value>,
    pub error: Option<String>,
}

fn mode_name(mode: &str) -> &'static str {
    match mode {
        "ask" => "Ask",
        "plan" => "Plan",
        "approve-each" => "Approve each",
        "accept-edits" => "Accept edits",
        "auto" => "Auto",
        _ => "that mode",
    }
}

/// The machine-wide path on each platform.
pub fn policy_path(platform: &str, windows_common_data: Option<&str>) -> String {
    match platform {
        "win32" => format!("{}\\Locust\\policy.json", windows_common_data.unwrap_or("C:\\ProgramData")),
        "darwin" => "/Library/Application Support/Locust/policy.json".into(),
        _ => "/etc/locust/policy.json".into(),
    }
}

/// A monthly limit in dollars: positive and finite, as Electron's `isMonthlyLimit` takes it.
fn monthly_limit_ok(value: &Value) -> bool {
    value.as_f64().is_some_and(|n| n.is_finite() && n > 0.0 && n <= 1_000_000.0)
}

/// Every known key is validated. Empty allowlists intentionally deny everything. Unknown keys are skipped and named.
pub fn parse_policy(value: &Value, unknown_key: &mut dyn FnMut(&str)) -> Result<Map<String, Value>, String> {
    let Some(object) = value.as_object() else { return Err("The policy must be a JSON object.".into()) };
    for key in object.keys() {
        if !KEYS.contains(&key.as_str()) {
            unknown_key(key);
        }
    }
    let bad = |key: &str| Err(format!("The {key} setting is invalid."));
    let mut out = Map::new();
    if let Some(v) = object.get("version") {
        if v != &json!(1) {
            return bad("version");
        }
        out.insert("version".into(), json!(1));
    }
    if let Some(v) = object.get("allowedAgents") {
        if !v.as_array().is_some_and(|a| a.iter().all(|r| r.as_str().is_some_and(|r| POLICY_RUNTIME_IDS.contains(&r)))) {
            return bad("allowedAgents");
        }
        out.insert("allowedAgents".into(), v.clone());
    }
    if let Some(v) = object.get("allowedModels") {
        let Some(models) = v.as_object() else { return bad("allowedModels") };
        for (id, patterns) in models {
            let ok = POLICY_RUNTIME_IDS.contains(&id.as_str())
                && patterns.as_array().is_some_and(|a| a.iter().all(|p| p.as_str().is_some_and(|p| !p.is_empty() && p.len() <= 512 && !p.contains(['\r', '\n', '\0']))));
            if !ok {
                return bad("allowedModels");
            }
        }
        out.insert("allowedModels".into(), v.clone());
    }
    if let Some(v) = object.get("highestMode") {
        if !v.as_str().is_some_and(|m| POLICY_MODE_ORDER.contains(&m)) {
            return bad("highestMode");
        }
        out.insert("highestMode".into(), v.clone());
    }
    if let Some(v) = object.get("monthlyLimitUsd") {
        if v != &json!(0) && !monthly_limit_ok(v) {
            return bad("monthlyLimitUsd");
        }
        out.insert("monthlyLimitUsd".into(), v.clone());
    }
    for key in FEATURES {
        if let Some(v) = object.get(key) {
            if !v.is_boolean() {
                return bad(key);
            }
            out.insert(key.into(), v.clone());
        }
    }
    Ok(out)
}

/// Read once, without ever creating, replacing or writing the administrator's file.
pub fn read_policy(path: &str, read: &dyn Fn(&str) -> std::io::Result<Vec<u8>>, note: &mut dyn FnMut(String)) -> PolicyState {
    let fail = |why: &str| PolicyState {
        path: path.into(),
        present: true,
        policy: Map::new(),
        error: Some(format!(
            "Locust cannot use the organization policy at {path}. {why} No turns or connections will start until your administrator fixes it and Locust is restarted."
        )),
    };
    let bytes = match read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return PolicyState { path: path.into(), present: false, policy: Map::new(), error: None },
        Err(_) => return fail("The file could not be read."),
    };
    if bytes.len() > 1024 * 1024 {
        return fail("The file exceeds 1 MB.");
    }
    let Ok(text) = String::from_utf8(bytes) else { return fail("The file is not valid JSON.") };
    let Ok(value) = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) else { return fail("The file is not valid JSON.") };
    let mut unknown: Vec<String> = Vec::new();
    match parse_policy(&value, &mut |key| unknown.push(key.into())) {
        Ok(policy) => {
            for key in unknown {
                note(format!("Unknown setting \"{key}\" in {path} was skipped."));
            }
            PolicyState { path: path.into(), present: true, policy, error: None }
        }
        Err(why) => {
            let state = fail(&why);
            note(state.error.clone().unwrap_or_default());
            state
        }
    }
}

pub fn policy_refusal(state: &PolicyState, setting: &str, why: &str) -> String {
    format!("{why} Set by your organization ({setting} in {}). Nothing was started.", state.path)
}

/// A feature the organization turned off, said; or the policy's own error.
pub fn feature_refusal(state: Option<&PolicyState>, setting: &str) -> Option<String> {
    let state = state?;
    if let Some(error) = &state.error {
        return Some(error.clone());
    }
    if state.policy.get(setting) != Some(&json!(false)) {
        return None;
    }
    let name = match setting {
        "otherApps" => "Access from other AI apps, the locust command and editors is off.",
        "phone" => "Phone pairing is off.",
        "chatApps" => "Teammates in chat apps are off.",
        "cloud" => "Cloud computers and cloud sessions are off.",
        "terminal" => "The terminal panel is off.",
        "browser" => "The browser panel is off.",
        "accounts" => "Added accounts are off.",
        _ => "This is off.",
    };
    Some(policy_refusal(state, setting, name))
}

pub fn mode_allowed(state: Option<&PolicyState>, mode: &str) -> bool {
    let Some(state) = state else { return true };
    if state.error.is_some() {
        return false;
    }
    match state.policy.get("highestMode").and_then(Value::as_str) {
        None => true,
        Some(highest) => POLICY_MODE_ORDER.iter().position(|m| *m == mode) <= POLICY_MODE_ORDER.iter().position(|m| *m == highest),
    }
}

/// Only `*` has special meaning; model ids are otherwise literal.
pub fn model_matches(pattern: &str, id: &str) -> bool {
    let (p, i): (Vec<char>, Vec<char>) = (pattern.chars().collect(), id.chars().collect());
    let (mut pi, mut ii, mut star, mut from) = (0usize, 0usize, None::<usize>, 0usize);
    while ii < i.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            pi += 1;
            from = ii;
        } else if pi < p.len() && p[pi] == i[ii] {
            pi += 1;
            ii += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            from += 1;
            ii = from;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

pub fn agent_allowed(state: Option<&PolicyState>, runtime: &str) -> bool {
    let Some(state) = state else { return true };
    state.error.is_none() && state.policy.get("allowedAgents").and_then(Value::as_array).is_none_or(|a| a.iter().any(|r| r == runtime))
}

pub fn model_allowed(state: Option<&PolicyState>, runtime: &str, model: Option<&str>) -> bool {
    if !agent_allowed(state, runtime) {
        return false;
    }
    let Some(patterns) = state.and_then(|s| s.policy.get("allowedModels")).and_then(|m| m.get(runtime)).and_then(Value::as_array) else { return true };
    if patterns.iter().any(|p| p == "*") {
        return true;
    }
    // A runtime's default is unknown until it runs. A finite allowlist needs an explicit model.
    let id = if runtime == "acp" { model.and_then(|m| m.split_once(':').map(|(_, rest)| rest).or(Some(m))) } else { model };
    id.is_some_and(|id| id != "account-default" && patterns.iter().filter_map(Value::as_str).any(|p| model_matches(p, id)))
}

/// The refusal for one turn, or none: the policy's own error first, then accounts, cloud, agent, model and mode.
pub fn turn_refusal(state: Option<&PolicyState>, runtime: &str, model: Option<&str>, mode: &str, cloud: bool, account: bool) -> Option<String> {
    let state = state?;
    if let Some(error) = &state.error {
        return Some(error.clone());
    }
    if account && let Some(refused) = feature_refusal(Some(state), "accounts") {
        return Some(refused);
    }
    if cloud && let Some(refused) = feature_refusal(Some(state), "cloud") {
        return Some(refused);
    }
    let agent = if runtime == "omniroute" { "OmniRoute".to_owned() } else { crate::carry_on::display_name(runtime).to_owned() };
    if !agent_allowed(Some(state), runtime) {
        return Some(policy_refusal(state, "allowedAgents", &format!("{agent} is not an allowed AI agent.")));
    }
    if !model_allowed(Some(state), runtime, model) {
        return Some(policy_refusal(state, "allowedModels", &format!("The {runtime} model {} is not allowed. Pick an allowed, named model.", model.unwrap_or("account default"))));
    }
    if !mode_allowed(Some(state), mode) {
        let highest = state.policy.get("highestMode").and_then(Value::as_str).unwrap_or("ask");
        return Some(policy_refusal(state, "highestMode", &format!("{} exceeds the highest allowed mode, {}.", mode_name(mode), mode_name(highest))));
    }
    None
}

/// The effective monthly limit: the organization's cap, a person's own lower amount, or none.
pub fn monthly_limit(state: Option<&PolicyState>, personal: Option<f64>) -> Option<f64> {
    match (state.and_then(|s| s.policy.get("monthlyLimitUsd")).and_then(Value::as_f64), personal) {
        (None, p) => p,
        (Some(cap), None) => Some(cap),
        (Some(cap), Some(p)) => Some(cap.min(p)),
    }
}

/// A policy setting as Settings says it: agents and modes by name, a limit in dollars a month.
pub fn setting_words(key: &str, value: &Value) -> String {
    let agent_words = |r: &str| match r {
        "omniroute" => "OmniRoute".to_owned(),
        "acp" => "Agents you added".to_owned(),
        other => crate::carry_on::display_name(other).to_owned(),
    };
    match key {
        "allowedAgents" => match value.as_array().filter(|a| !a.is_empty()) {
            Some(a) => a.iter().filter_map(Value::as_str).map(agent_words).collect::<Vec<_>>().join(", "),
            None => "None: no AI agent can run".into(),
        },
        "allowedModels" => match value.as_object().filter(|m| !m.is_empty()) {
            None => "Any".into(),
            Some(m) => m
                .iter()
                .map(|(r, p)| {
                    let list: Vec<&str> = p.as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                    format!("{}: {}", agent_words(r), if list.is_empty() { "none".into() } else { list.join(", ") })
                })
                .collect::<Vec<_>>()
                .join("; "),
        },
        "highestMode" => value.as_str().map(|m| mode_name(m).to_owned()).unwrap_or_default(),
        "monthlyLimitUsd" => match value.as_f64() {
            Some(n) if n == 0.0 => "$0, so nothing that spends can start".into(),
            Some(n) if n.fract() == 0.0 => format!("${} a month", n as i64),
            Some(n) => format!("${n:.2} a month"),
            None => value.to_string(),
        },
        k if FEATURES.contains(&k) => if value == &json!(false) { "Off".into() } else { "Allowed".into() },
        _ => value.to_string(),
    }
}

static STATE: Mutex<Option<PolicyState>> = Mutex::new(None);

/// This process's policy, read once at first use from the machine-wide file.
pub fn current() -> PolicyState {
    STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(|| {
            let platform = if cfg!(target_os = "macos") { "darwin" } else if cfg!(windows) { "win32" } else { "linux" };
            let path = policy_path(platform, std::env::var("ProgramData").ok().as_deref());
            read_policy(&path, &|p| std::fs::read(PathBuf::from(p)), &mut |line| eprintln!("organization-policy: {line}"))
        })
        .clone()
}

/// A policy of a test's own, in place of the machine's file.
pub fn use_policy(state: Option<PolicyState>) {
    *STATE.lock().unwrap_or_else(|e| e.into_inner()) = Some(state.unwrap_or(PolicyState { path: "/test/policy.json".into(), present: false, policy: Map::new(), error: None }));
}

#[cfg(test)]
#[path = "org_policy_tests.rs"]
mod tests;
