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

/// Where and how the policy is read (Electron `organizationPolicyPath`'s and `readOrganizationPolicy`'s options).
pub struct PolicyOptions<'a> {
    /// A packaged app never consults the development test seam.
    pub packaged: bool,
    pub platform: &'a str,
    pub env: &'a dyn Fn(&str) -> Option<String>,
    /// Windows' machine-wide application data folder, as Windows itself names it.
    pub common_data: &'a dyn Fn() -> Result<String, String>,
}

/// The file to read: the development override `LOCUST_POLICY_FILE` in an unpackaged app (an absolute path only), or
/// the machine-wide path, on Windows under the OS known folder rather than a per-process `ProgramData`.
pub fn organization_policy_path(options: &PolicyOptions) -> Result<String, String> {
    if !options.packaged && let Some(file) = (options.env)("LOCUST_POLICY_FILE") {
        let absolute = std::path::Path::new(&file).is_absolute() || windows_absolute(&file);
        if !absolute {
            return Err("LOCUST_POLICY_FILE must name an absolute path".into());
        }
        return Ok(file);
    }
    match options.platform {
        "win32" => Ok(format!("{}\\Locust\\policy.json", (options.common_data)()?.trim_end_matches('\\'))),
        other => Ok(policy_path(other, None)),
    }
}

/// `win32.isAbsolute`: a drive's root or a rooted path.
fn windows_absolute(path: &str) -> bool {
    let b = path.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')) || path.starts_with('\\') || path.starts_with('/')
}

/// What `reg query ... /v "Common AppData"` says, its folder or none.
pub fn common_app_data_from_reg(said: &str) -> Option<String> {
    said.lines().find_map(|line| {
        let rest = line.trim_start().strip_prefix("Common AppData")?;
        let rest = rest.trim_start().strip_prefix("REG_SZ")?;
        let found = rest.trim();
        (!found.is_empty()).then(|| found.to_owned())
    })
}

/// A folder Windows named: absolute, and never a network share.
pub fn settle_common_data(found: Option<String>) -> Result<String, String> {
    match found {
        Some(found) if windows_absolute(&found) && !found.starts_with("\\\\") => Ok(found),
        _ => Err("Windows did not identify its machine-wide application data folder.".into()),
    }
}

/// Electron `windowsCommonData`: the OS known folder, read from the machine's registry, which only an administrator can
/// change; PowerShell's answer is the fallback. A tool comes from Windows' own folder, so a `SystemRoot` handed to this
/// process cannot swap in a look-alike.
pub fn windows_common_data() -> Result<String, String> {
    let tool = |parts: &[&str]| {
        let fixed = format!("C:\\Windows\\System32\\{}", parts.join("\\"));
        if std::path::Path::new(&fixed).exists() {
            fixed
        } else {
            format!("{}\\System32\\{}", std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()), parts.join("\\"))
        }
    };
    let run = |program: String, args: &[&str]| -> Option<String> {
        let mut command = std::process::Command::new(program);
        command.args(args).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let out = command.output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let key = "HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Shell Folders";
    if let Some(found) = run(tool(&["reg.exe"]), &["query", key, "/v", "Common AppData"]).and_then(|said| common_app_data_from_reg(&said)) {
        return settle_common_data(Some(found));
    }
    let said = run(
        tool(&["WindowsPowerShell", "v1.0", "powershell.exe"]),
        &["-NoProfile", "-NonInteractive", "-Command", "[Environment]::GetFolderPath('CommonApplicationData')"],
    );
    settle_common_data(said.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()))
}

/// Electron `readOrganizationPolicy`: the path first, then the file, read once and never written. A path that cannot be
/// named (an invalid override, Windows not naming its folder) is a closed gate too.
pub fn read_organization_policy(options: &PolicyOptions, read: &dyn Fn(&str) -> std::io::Result<Vec<u8>>, note: &mut dyn FnMut(String)) -> PolicyState {
    match organization_policy_path(options) {
        Ok(path) => read_policy(&path, read, note),
        Err(why) => {
            let label = if options.packaged { None } else { (options.env)("LOCUST_POLICY_FILE") };
            let path = label.unwrap_or_else(|| "the machine-wide Locust/policy.json file".into());
            let error = format!("Locust cannot use the organization policy at {path}. {why} No turns or connections will start until your administrator fixes it and Locust is restarted.");
            note(error.clone());
            PolicyState { path, present: true, policy: Map::new(), error: Some(error) }
        }
    }
}

/// This machine's policy, as the app reads it once at launch: a release build is the packaged app.
pub fn read_machine_policy(env: &dyn Fn(&str) -> Option<String>, read: &dyn Fn(&str) -> std::io::Result<Vec<u8>>, note: &mut dyn FnMut(String)) -> PolicyState {
    let platform = if cfg!(target_os = "macos") { "darwin" } else if cfg!(windows) { "win32" } else { "linux" };
    // A test build (debug or release) is never the packaged app: its fixtures point the override at their own file.
    let options = PolicyOptions { packaged: !cfg!(debug_assertions) && !cfg!(test), platform, env, common_data: &windows_common_data };
    read_organization_policy(&options, read, note)
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
        "cloud" => "Cloud computers and cloud tasks are off.",
        "terminal" => "The terminal panel is off.",
        "browser" => "The browser panel is off.",
        "accounts" => "Added agent accounts are off.",
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
    // An own ACP agent's route names its agent and, after `::`, the model picked there: only that model is matched.
    let id = if runtime == "acp" { model.and_then(crate::acp_agents::parsed_acp_route_model).and_then(|route| route.model) } else { model.map(str::to_owned) };
    id.is_some_and(|id| id != "account-default" && patterns.iter().filter_map(Value::as_str).any(|p| model_matches(p, &id)))
}

/// Electron `policyModels`: only selectable ids, each effort variant included; no synthetic default escapes the
/// allowlist. A family whose own id is blocked is offered under its first allowed variant, with only those efforts.
pub fn policy_models(state: Option<&PolicyState>, models: Vec<Value>) -> Vec<Value> {
    models
        .into_iter()
        .filter_map(|model| {
            let runtime = model["runtime"].as_str().unwrap_or_default().to_owned();
            let allowed = |id: Option<&str>| model_allowed(state, &runtime, id);
            let own_ok = allowed(model["id"].as_str());
            let Some(variants) = model["variants"].as_object() else { return own_ok.then_some(model) };
            let kept: Map<String, Value> = variants.iter().filter(|(_, id)| allowed(id.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
            if own_ok && kept.len() == variants.len() {
                return Some(model);
            }
            let id = if own_ok { model["id"].clone() } else { kept.values().next()?.clone() };
            let efforts: Vec<Value> = model["supportedEfforts"].as_array().into_iter().flatten().filter(|e| e.as_str().is_some_and(|e| kept.contains_key(e))).cloned().collect();
            let default = model["defaultEffort"].as_str().filter(|e| kept.contains_key(*e)).map(|e| json!(e)).or_else(|| efforts.first().cloned());
            let mut out = model.clone();
            out["id"] = id;
            out["variants"] = Value::Object(kept);
            out["supportedEfforts"] = Value::Array(efforts);
            match default {
                Some(effort) => out["defaultEffort"] = effort,
                None => {
                    if let Some(o) = out.as_object_mut() {
                        o.remove("defaultEffort");
                    }
                }
            }
            Some(out)
        })
        .collect()
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

/// Interactive terminals and legacy cloud sessions do not return spending receipts to Locust
/// (`shared/organization-policy.ts` `policyUnmeteredRefusal`).
pub fn unmetered_refusal(
    state: Option<&PolicyState>,
    runtime: &str,
    model: Option<&str>,
    mode: &str,
    cloud: bool,
    account: bool,
) -> Option<String> {
    if let Some(refused) = turn_refusal(state, runtime, model, mode, cloud, account) {
        return Some(refused);
    }
    let state = state?;
    if state.policy.get("monthlyLimitUsd").is_some() {
        return Some(policy_refusal(
            state,
            "monthlyLimitUsd",
            "This launch cannot report spending to Locust, so its monthly limit cannot be enforced.",
        ));
    }
    None
}

/// An interactive shell can enforce none of the organization's limits on agents, models, spend or modes, so any of them
/// keeps the terminal closed, as does the terminal turned off or a policy Locust cannot use
/// (`shared/organization-policy.ts` `policyTerminalRefusal`).
pub fn terminal_refusal(state: Option<&PolicyState>) -> Option<String> {
    if let Some(refused) = feature_refusal(state, "terminal") {
        return Some(refused);
    }
    let state = state?;
    for key in ["allowedAgents", "allowedModels", "monthlyLimitUsd"] {
        if state.policy.get(key).is_some() {
            return Some(policy_refusal(
                state,
                key,
                "An interactive shell cannot enforce this organization restriction. Use a conversation in Locust.",
            ));
        }
    }
    if state.policy.get("highestMode").is_some_and(|mode| mode != "auto") {
        return Some(policy_refusal(
            state,
            "highestMode",
            "An interactive shell cannot enforce the highest permission mode. Use a conversation in Locust.",
        ));
    }
    None
}

/// A plain cloud shell cannot constrain which installed agent a person starts there
/// (`shared/organization-policy.ts` `policyCloudTerminalRefusal`).
pub fn cloud_terminal_refusal(state: Option<&PolicyState>) -> Option<String> {
    if let Some(refused) = feature_refusal(state, "cloud") {
        return Some(refused);
    }
    let state = state?;
    if state.policy.get("allowedAgents").is_some() {
        return Some(policy_refusal(
            state,
            "allowedAgents",
            "A cloud terminal cannot enforce the allowed AI agents. Use a conversation in Locust.",
        ));
    }
    if state.policy.get("allowedModels").is_some() {
        return Some(policy_refusal(
            state,
            "allowedModels",
            "A cloud terminal cannot enforce the allowed models. Use a conversation in Locust.",
        ));
    }
    unmetered_refusal(Some(state), "opencode", None, "auto", false, false)
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

thread_local! {
    /// A test's policy that only its own thread sees, so tests running side by side in one process never see it.
    static THIS_THREAD: std::cell::RefCell<Option<PolicyState>> = const { std::cell::RefCell::new(None) };
}

/// A policy of a test's own on this thread only, until `None` puts the process's policy back for it.
pub fn use_policy_on_this_thread(state: Option<PolicyState>) {
    THIS_THREAD.with(|t| *t.borrow_mut() = state);
}

/// This process's policy, read once at first use from the machine-wide file.
pub fn current() -> PolicyState {
    if let Some(here) = THIS_THREAD.with(|t| t.borrow().clone()) {
        return here;
    }
    STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(|| {
            read_machine_policy(&|key| std::env::var(key).ok(), &|p| std::fs::read(PathBuf::from(p)), &mut |line| eprintln!("organization-policy: {line}"))
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
