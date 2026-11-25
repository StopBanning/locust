//! THE MODEL LIST (B4): what each agent offers on this computer, as Electron's catalog builds it
//! (apps/desktop/src/main/model-catalog.ts, over runtime-adapters' parsers in commands.ts).
//!
//! Each agent's models come from its own source and fail on their own: Codex's from a live `model/list` on its
//! app-server, Claude Code's from the aliases its `--help` names, Cursor's from `cursor-agent --list-models`,
//! OpenCode 1's from `opencode models --verbose`, Antigravity's from `agy models`; Copilot and Muse list none, so
//! the catalog offers what Electron offers for them. Every runtime that listed anything gets an "Account default"
//! row first, with the effort levels all its current models agree on. A model is a JSON object in Electron's
//! `PublicModel` shape (id, runtime, displayName, description, supportedEfforts, and older, variants or
//! defaultEffort where they apply), so the door can hand it on as Electron's does.
//!
//! Codex's session asks `account/rateLimits/read` too, once its models are in, so its usage is read with them.
//!
//! Not ported: the models Claude Code's handshake lists.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const MAX_MODELS: usize = 40;
pub const ACCOUNT_DEFAULT: &str = "account-default";

fn re(pattern: &'static str) -> regex::Regex {
    crate::invariants::regex(pattern)
}

// ---- the parsers (commands.ts) --------------------------------------------------------------------------------------

/// parseEffortChoices: the choices a `--effort` / `--reasoning-effort` option lists in a help text.
pub fn parse_effort_choices(help: &str) -> Vec<String> {
    let Some(clause) = re(r"(?is)--(?:reasoning-)?effort\b.{0,240}?(?:\(\s*(?:choices:)?\s*([^)]*)\)|\[\s*possible values:\s*([^\]]*)\])").captures(help) else {
        return Vec::new();
    };
    let listed = clause.get(1).or_else(|| clause.get(2)).map(|m| m.as_str()).unwrap_or("");
    let mut out: Vec<String> = Vec::new();
    for entry in listed.split(',') {
        let word = entry.trim().trim_start_matches(['"', '\'']).trim_end_matches(['"', '\'']).trim().to_owned();
        if re(r"^[a-z]{1,16}$").is_match(&word) && !out.contains(&word) {
            out.push(word);
        }
    }
    out
}

/// parseClaudeModelHints: the aliases Claude Code's `--help` names for the newest model of each family, and its efforts.
pub fn parse_claude_model_hints(help: &str) -> Option<Value> {
    let mut aliases: Vec<String> = Vec::new();
    if let Some(clause) = re(r"alias for the latest model \(e\.g\.\s*([^)]*)\)").captures(help) {
        for m in re(r"'([a-z0-9][a-z0-9.-]{0,30})'").captures_iter(&clause[1]) {
            if !aliases.contains(&m[1].to_owned()) {
                aliases.push(m[1].to_owned());
            }
        }
    }
    let efforts = parse_effort_choices(help);
    if aliases.is_empty() && efforts.is_empty() {
        return None;
    }
    Some(json!({"aliases": aliases, "efforts": efforts}))
}

/// parseCursorModelList: a heading, then one `<id> - <display name>` per line.
pub fn parse_cursor_model_list(text: &str) -> Option<Value> {
    let mut models: Vec<Value> = Vec::new();
    for line in text.lines() {
        let Some(m) = re(r"^([a-z0-9][a-z0-9.-]{0,60})\s+-\s+(.{1,80})$").captures(line.trim()) else { continue };
        if models.iter().any(|x| x["id"] == m[1]) {
            continue;
        }
        models.push(json!({"id": &m[1], "displayName": m[2].trim()}));
    }
    if models.is_empty() {
        return None;
    }
    let aliases: Vec<Value> = models.iter().map(|x| x["id"].clone()).collect();
    Some(json!({"aliases": aliases, "efforts": [], "models": models}))
}

/// parseAgyModelList: `id<TAB>Display name` per line, each effort folded into its model.
pub fn parse_agy_model_list(text: &str) -> Option<Value> {
    let mut models: Vec<(String, String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        let Some(m) = re(r"^([a-z0-9][a-z0-9.-]{0,80})\t(.{1,80})$").captures(line.trim()) else { continue };
        let variant = re(r"^(.*)-(low|medium|high|xhigh|max)$").captures(&m[1]);
        let (id, name, effort) = match &variant {
            None => (m[1].to_owned(), m[2].trim().to_owned(), None),
            Some(v) => (
                v[1].to_owned(),
                re(r"(?i)\s*\((low|medium|high|xhigh|max)\)\s*$").replace(&m[2], "").trim().to_owned(),
                Some(v[2].to_owned()),
            ),
        };
        match models.iter_mut().find(|x| x.0 == id) {
            None => models.push((id, name, effort.into_iter().collect())),
            Some(known) => {
                if let Some(e) = effort
                    && !known.2.contains(&e)
                {
                    known.2.push(e);
                }
            }
        }
    }
    if models.is_empty() {
        return None;
    }
    let order = ["low", "medium", "high", "xhigh", "max"];
    let named: Vec<Value> = models
        .into_iter()
        .map(|(id, name, mut efforts)| {
            let mut v = json!({"id": id, "displayName": name});
            if !efforts.is_empty() {
                efforts.sort_by_key(|e| order.iter().position(|o| o == e));
                v["efforts"] = json!(efforts);
            }
            v
        })
        .collect();
    let aliases: Vec<Value> = named.iter().map(|x| x["id"].clone()).collect();
    Some(json!({"aliases": aliases, "efforts": [], "models": named}))
}

const OPENCODE_EFFORT_VARIANTS: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

/// openCodeVariantsAfter: a model's efforts, from the JSON object `--verbose` prints after its id.
fn opencode_variants_after(lines: &[&str], at: usize) -> Vec<String> {
    if lines.get(at + 1).map(|l| l.trim()) != Some("{") {
        return Vec::new();
    }
    let mut body = Vec::new();
    for line in &lines[at + 1..] {
        body.push(*line);
        if *line == "}" {
            break;
        }
    }
    let Ok(model) = serde_json::from_str::<Value>(&body.join("\n")) else { return Vec::new() };
    let Some(variants) = model["variants"].as_object() else { return Vec::new() };
    OPENCODE_EFFORT_VARIANTS.iter().filter(|e| variants.contains_key(**e)).map(|e| (*e).to_owned()).collect()
}

/// parseOpenCodeModelList: one `provider/model` id per line (with `--verbose`, each followed by its JSON).
pub fn parse_opencode_model_list(text: &str) -> Option<Value> {
    let lines: Vec<&str> = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    let mut models: Vec<Value> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(m) = re(r"^([a-z0-9][a-z0-9._-]{0,60})/([A-Za-z0-9][A-Za-z0-9._-]{0,80})$").captures(line.trim()) else { continue };
        let id = format!("{}/{}", &m[1], &m[2]);
        if models.iter().any(|x| x["id"] == id.as_str()) {
            continue;
        }
        let name = m[2].to_owned();
        let mut v = json!({"id": id, "displayName": name});
        if name.ends_with("-free") {
            v["description"] = json!("free");
        }
        let efforts = opencode_variants_after(&lines, index);
        if !efforts.is_empty() {
            v["efforts"] = json!(efforts);
        }
        models.push(v);
    }
    if models.is_empty() {
        return None;
    }
    let aliases: Vec<Value> = models.iter().map(|x| x["id"].clone()).collect();
    Some(json!({"aliases": aliases, "efforts": [], "models": models}))
}

// ---- the catalog (model-catalog.ts) ---------------------------------------------------------------------------------

fn strings(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect()
}

/// parseModels: Codex's `model/list`, its hidden models left out, at most forty.
pub fn parse_codex_models(result: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for entry in result["data"].as_array().into_iter().flatten().take(MAX_MODELS) {
        let Some(id) = entry["id"].as_str().filter(|s| !s.is_empty()) else { continue };
        if entry["hidden"] == true {
            continue;
        }
        let mut efforts: Vec<String> = Vec::new();
        for effort in entry["supportedReasoningEfforts"].as_array().into_iter().flatten() {
            let name = effort["reasoningEffort"].as_str().or_else(|| effort.as_str());
            if let Some(n) = name.filter(|n| !n.is_empty())
                && !efforts.contains(&n.to_owned())
            {
                efforts.push(n.to_owned());
            }
        }
        let name = entry["displayName"].as_str().filter(|s| !s.is_empty()).unwrap_or(id);
        let description: String = entry["description"].as_str().unwrap_or("").chars().take(200).collect();
        out.push(json!({"id": id, "runtime": "codex", "displayName": name, "description": description, "supportedEfforts": efforts}));
    }
    out
}

/// claude-models.ts: the version each alias means, and the fixed versions offered under them.
pub const CLAUDE_ALIAS_DEFAULTS: [(&str, &str); 4] =
    [("fable", "claude-fable-5-1"), ("opus", "claude-opus-5-5"), ("sonnet", "claude-sonnet-5-5"), ("haiku", "claude-haiku-5-5")];
const FIVE: &[&str] = &["low", "medium", "high", "xhigh", "max"];
const FOUR: &[&str] = &["low", "medium", "high", "max"];
const CLAUDE_OLDER_MODELS: [(&str, &[&str]); 10] = [
    ("claude-opus-5", FIVE),
    ("claude-opus-4-8", FIVE),
    ("claude-opus-4-7", FIVE),
    ("claude-opus-4-6", FOUR),
    ("claude-opus-4-5", &[]),
    ("claude-fable-5", FIVE),
    ("claude-sonnet-5", FIVE),
    ("claude-sonnet-4-6", FOUR),
    ("claude-sonnet-4-5", &[]),
    ("claude-haiku-4-5-20251001", &[]),
];
/// claude-models.ts CLAUDE_OLDER_MODELS: the fixed versions offered under the aliases, newest first, with their levels.
pub fn claude_older_models() -> Vec<(&'static str, Vec<String>)> {
    CLAUDE_OLDER_MODELS.iter().map(|(id, e)| (*id, e.iter().map(|s| (*s).to_owned()).collect())).collect()
}

/// The older versions the picker folds under Claude Code (claudeModelsFrom's rule): each by its name, skipping one an
/// alias already means now (`now` holds the current rows' names) and a name offered twice.
pub fn claude_older_offered(now: &[String]) -> Vec<(String, String, Vec<String>)> {
    let mut offered: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (id, efforts) in claude_older_models() {
        let name = claude_model_name(id).unwrap_or_else(|| id.to_owned());
        if now.contains(&name) || offered.contains(&name) {
            continue;
        }
        offered.push(name.clone());
        out.push((id.to_owned(), name, efforts));
    }
    out
}

/// What Claude Code's catalogue says about a model (claudeModelsFrom's descriptions): an alias is "Always the newest
/// Opus", a fixed older version "This version, always", the account's default what it is. None where this build's
/// window has no catalogue words for the model (the picker then shows its levels).
pub fn catalogue_description(runtime: &str, id: &str) -> Option<String> {
    // Engine Model carries the name and efforts; retain the catalogue's free-model detail in the picker too.
    if runtime == "opencode" && id.ends_with("-free") {
        return Some("Free · no sign-in".to_owned());
    }
    if id == ACCOUNT_DEFAULT {
        return Some("The model your account picks for you".to_owned());
    }
    if runtime != "claude" {
        return None;
    }
    if CLAUDE_ALIAS_DEFAULTS.iter().any(|(alias, _)| *alias == id) {
        return Some(format!("Always the newest {}", capitalised(id)));
    }
    CLAUDE_OLDER_MODELS.iter().any(|(older, _)| *older == id).then(|| "This version, always".to_owned())
}

/// CLAUDE_ALIASES_MEASURED: an alias Claude Code accepts that its help does not name.
const CLAUDE_ALIASES_MEASURED: [&str; 1] = ["haiku"];

/// claudeModelName: `claude-opus-5-5` reads "Opus 5.5".
pub fn claude_model_name(id: &str) -> Option<String> {
    let m = re(r"^claude-([a-z]+)-(\d+)(?:-(\d{1,2}))?(?:-\d{8})?(?:-v\d+)?$").captures(id)?;
    let family = &m[1];
    let mut name = format!("{}{} {}", family[..1].to_uppercase(), &family[1..], &m[2]);
    if let Some(minor) = m.get(3) {
        name.push('.');
        name.push_str(minor.as_str());
    }
    Some(name)
}

/// claudeRouteModelName: what a Claude route runs -- a finished run's own report (`earned`), else Claude Code's own name
/// for the alias (`listed`, from its handshake), else the version the copied alias table says it means.
pub fn claude_route_model_name(model: &str, earned: Option<&str>, listed: Option<&str>) -> Option<String> {
    if let Some(named) = earned.and_then(claude_model_name) {
        return Some(named);
    }
    if let Some(listed) = listed.filter(|l| !l.is_empty()) {
        return Some(listed.to_owned());
    }
    let meant = CLAUDE_ALIAS_DEFAULTS.iter().find(|(alias, _)| *alias == model).map_or(model, |(_, id)| *id);
    claude_model_name(meant)
}

fn capitalised(word: &str) -> String {
    let mut c = word.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// claudeModelsFrom, before Claude Code's handshake has listed anything: each alias, then the older fixed versions.
pub fn claude_models_from(hints: Option<&Value>) -> Vec<Value> {
    let Some(hints) = hints else { return Vec::new() };
    let named = strings(&hints["aliases"]);
    let efforts = strings(&hints["efforts"]);
    let mut aliases: Vec<String> = Vec::new();
    if !named.is_empty() {
        for a in named.iter().cloned().chain(CLAUDE_ALIASES_MEASURED.iter().map(|s| (*s).to_owned())) {
            if !aliases.contains(&a) {
                aliases.push(a);
            }
        }
    }
    let meant = |alias: &str| CLAUDE_ALIAS_DEFAULTS.iter().find(|(a, _)| *a == alias).map(|(_, id)| *id);
    let current: Vec<Value> = aliases
        .iter()
        .map(|alias| {
            let family = capitalised(alias);
            let name = meant(alias).and_then(claude_model_name).unwrap_or_else(|| family.clone());
            json!({"id": alias, "runtime": "claude", "displayName": name, "description": format!("Always the newest {family}"), "supportedEfforts": efforts})
        })
        .collect();
    if current.is_empty() {
        return current;
    }
    let means_now: Vec<String> = aliases.iter().filter_map(|a| claude_model_name(meant(a).unwrap_or(a))).collect();
    let mut offered: Vec<String> = Vec::new();
    let mut older = Vec::new();
    for (id, model_efforts) in CLAUDE_OLDER_MODELS {
        let name = claude_model_name(id).unwrap_or_else(|| id.to_owned());
        if means_now.contains(&name) || offered.contains(&name) {
            continue;
        }
        offered.push(name.clone());
        older.push(json!({"id": id, "runtime": "claude", "displayName": name, "description": "This version, always", "supportedEfforts": model_efforts, "older": true}));
    }
    [current, older].concat()
}

const CURSOR_EFFORT_WORDS: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

/// splitCursorModelId: `cursor-grok-4.6-high-fast` is the family `cursor-grok-4.6` at `high-fast`.
pub fn split_cursor_model_id(id: &str) -> (String, Option<String>) {
    let mut words: Vec<&str> = id.split('-').collect();
    let mut fast = false;
    let mut thinking = false;
    let mut level: Option<String> = None;
    if words.len() > 1 && words.last() == Some(&"fast") {
        fast = true;
        words.pop();
    }
    if words.len() > 1 && words.last() == Some(&"thinking") {
        thinking = true;
        words.pop();
    }
    if words.len() > 2 && words[words.len() - 2] == "extra" && words.last() == Some(&"high") {
        level = Some("xhigh".into());
        words.truncate(words.len() - 2);
    } else if words.len() > 1 && words.last().is_some_and(|w| CURSOR_EFFORT_WORDS.contains(w)) {
        level = words.pop().map(str::to_owned);
    }
    if !thinking && level.is_some() && words.len() > 1 && words.last() == Some(&"thinking") {
        thinking = true;
        words.pop();
    }
    let effort = match (&level, fast) {
        (None, true) => Some("fast".to_owned()),
        (None, false) => None,
        (Some(l), true) => Some(format!("{l}-fast")),
        (Some(l), false) => Some(l.clone()),
    };
    match effort {
        None => (id.to_owned(), None),
        Some(e) => (format!("{}{}", words.join("-"), if thinking { "-thinking" } else { "" }), Some(e)),
    }
}

fn cursor_name_words(name: &str) -> Vec<String> {
    name.chars()
        .filter(|c| !matches!(*c, '\u{200b}'..='\u{200d}' | '\u{feff}'))
        .collect::<String>()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// sharedCursorName: the words every variant's name shares at its start and its end.
pub fn shared_cursor_name(names: &[String]) -> Option<String> {
    let lists: Vec<Vec<String>> = names.iter().map(|n| cursor_name_words(n)).collect();
    let first = lists.first()?;
    let mut head = 0;
    while head < first.len() && lists.iter().all(|w| w.get(head) == first.get(head)) {
        head += 1;
    }
    let mut tail = 0;
    while tail < first.len() - head
        && lists.iter().all(|w| w.len() >= tail + 1 && w.len() - 1 - tail >= head && w[w.len() - 1 - tail] == first[first.len() - 1 - tail])
    {
        tail += 1;
    }
    let shared: Vec<String> = first[..head].iter().chain(first[first.len() - tail..].iter()).cloned().collect();
    (!shared.is_empty()).then(|| shared.join(" "))
}

/// cursorModelsFrom: Cursor's listed variants grouped into one model each, its efforts the variants it listed.
pub fn cursor_models_from(hints: Option<&Value>) -> Vec<Value> {
    let Some(listed) = hints.and_then(|h| h["models"].as_array()) else { return Vec::new() };
    struct Family {
        plain_name: Option<String>,
        default_id: String,
        variant_names: Vec<String>,
        fast_names: Vec<String>,
        variants: Vec<(String, String)>,
        efforts: Vec<String>,
    }
    let mut families: Vec<(String, Family)> = Vec::new();
    for model in listed {
        let id = model["id"].as_str().unwrap_or_default();
        let name = model["displayName"].as_str().unwrap_or_default().to_owned();
        let (family, effort) = split_cursor_model_id(id);
        let at = match families.iter().position(|(f, _)| *f == family) {
            Some(at) => at,
            None => {
                families.push((
                    family.clone(),
                    Family { plain_name: None, default_id: id.to_owned(), variant_names: vec![], fast_names: vec![], variants: vec![], efforts: vec![] },
                ));
                families.len() - 1
            }
        };
        let held = &mut families[at].1;
        match effort {
            None => {
                held.plain_name = Some(name);
                held.default_id = id.to_owned();
            }
            Some(e) => {
                match held.variants.iter_mut().find(|(k, _)| *k == e) {
                    Some(slot) => slot.1 = id.to_owned(),
                    None => held.variants.push((e.clone(), id.to_owned())),
                }
                if !held.efforts.contains(&e) {
                    held.efforts.push(e.clone());
                }
                if e.ends_with("fast") {
                    held.fast_names.push(name);
                } else {
                    held.variant_names.push(name);
                }
            }
        }
    }
    families
        .into_iter()
        .map(|(family, held)| {
            let shared = shared_cursor_name(&held.variant_names)
                .or_else(|| shared_cursor_name(&[held.variant_names.clone(), held.fast_names.clone()].concat()));
            let named = match (&held.plain_name, &shared) {
                (None, Some(shared)) => held.variants.iter().find(|(effort, id)| {
                    !effort.ends_with("fast")
                        && cursor_name_words(listed.iter().find(|m| m["id"] == id.as_str()).and_then(|m| m["displayName"].as_str()).unwrap_or(""))
                            .join(" ")
                            == *shared
                }),
                _ => None,
            };
            let mut row = json!({
                "id": named.map(|(_, id)| id.clone()).unwrap_or(held.default_id.clone()),
                "runtime": "cursor",
                "displayName": held.plain_name.clone().or(shared.clone()).unwrap_or(family),
                "description": "",
                "supportedEfforts": held.efforts
            });
            if !held.variants.is_empty() {
                row["variants"] = Value::Object(held.variants.iter().map(|(k, v)| (k.clone(), json!(v))).collect());
            }
            if let Some((effort, _)) = named {
                row["defaultEffort"] = json!(effort);
            }
            row
        })
        .collect()
}

/// opencodeModelsFrom: OpenCode 1's listed models; a `-free` one costs nothing and needs no sign-in.
pub fn opencode_models_from(hints: Option<&Value>) -> Vec<Value> {
    let Some(listed) = hints.and_then(|h| h["models"].as_array()) else { return Vec::new() };
    listed
        .iter()
        .map(|m| {
            let id = m["id"].as_str().unwrap_or_default();
            json!({"id": id, "runtime": "opencode", "displayName": m["displayName"],
                "description": if id.ends_with("-free") { "Free · no sign-in" } else { "Through OpenCode" },
                "supportedEfforts": m["efforts"].as_array().cloned().unwrap_or_default()})
        })
        .collect()
}

/// `opencodeModelsFrom` with OpenCode's version check: V1 keeps its CLI list, V2 never offers stale CLI hints.
pub fn opencode_models_for_version(major: Option<u32>, hints: Option<&Value>) -> Vec<Value> {
    if major == Some(2) {
        return Vec::new();
    }
    opencode_models_from(hints)
}

/// copilotModelsFrom: Copilot lists no models without a paid run, so Auto, which is real.
pub fn copilot_models() -> Vec<Value> {
    vec![json!({"id": "auto", "runtime": "copilot", "displayName": "Auto", "description": "Copilot picks the model your plan allows", "supportedEfforts": []})]
}

/// commands.ts MUSE_EFFORTS.
const MUSE_EFFORTS: [&str; 8] = ["none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra"];

/// museModelsFrom: the two models measured to reach a Muse model, at its own default effort.
pub fn muse_models() -> Vec<Value> {
    let model = |id: &str, name: &str, description: &str| {
        json!({"id": id, "runtime": "muse", "displayName": name, "description": description, "supportedEfforts": MUSE_EFFORTS, "defaultEffort": "high"})
    };
    vec![
        model("muse-wonder-1.0", "Muse Wonder 1.0", "Meta's newest, and Muse Code's default"),
        model("muse-spark-1.3", "Muse Spark 1.3", "The Muse before it"),
    ]
}

/// Apple's on-device model, the one model the apple runtime has. It has no account, so no account-default row.
pub fn apple_models() -> Vec<Value> {
    use crate::runtime::apple::{APPLE_MODEL, APPLE_MODEL_NAME, APPLE_RUNTIME};
    vec![json!({"id": APPLE_MODEL, "runtime": APPLE_RUNTIME, "displayName": APPLE_MODEL_NAME,
        "description": "Apple Intelligence, on this Mac · free · nothing leaves it · talks only", "supportedEfforts": []})]
}

/// antigravityModelsFrom: `agy models`, each with its efforts beside it.
pub fn antigravity_models_from(hints: Option<&Value>) -> Vec<Value> {
    let Some(listed) = hints.and_then(|h| h["models"].as_array()) else { return Vec::new() };
    listed
        .iter()
        .map(|m| {
            json!({"id": m["id"], "runtime": "antigravity", "displayName": m["displayName"],
                "description": m["description"].as_str().unwrap_or("Through Antigravity CLI"),
                "supportedEfforts": m["efforts"].as_array().cloned().unwrap_or_default()})
        })
        .collect()
}

/// accountDefaultModel: the row for sending no model, with the levels every current model of the runtime shares.
pub(crate) fn account_default_model(models: &[Value], runtime: &str) -> Option<Value> {
    let listed: Vec<&Value> = models.iter().filter(|m| m["runtime"] == runtime && m["id"] != ACCOUNT_DEFAULT && m["older"] != true).collect();
    let first = listed.first()?;
    let shared: Vec<String> = strings(&first["supportedEfforts"])
        .into_iter()
        .filter(|e| listed.iter().all(|m| strings(&m["supportedEfforts"]).contains(e)))
        .collect();
    Some(json!({"id": ACCOUNT_DEFAULT, "runtime": runtime, "displayName": "Account default", "description": "The model your account picks for you", "supportedEfforts": shared}))
}

/// withAccountDefaults: each runtime's account-default row, in front of the models.
pub fn with_account_defaults(models: Vec<Value>) -> Vec<Value> {
    let mut runtimes: Vec<String> = Vec::new();
    for m in &models {
        if let Some(r) = m["runtime"].as_str()
            && !runtimes.iter().any(|x| x == r)
        {
            runtimes.push(r.to_owned());
        }
    }
    let already: Vec<String> = models.iter().filter(|m| m["id"] == ACCOUNT_DEFAULT).filter_map(|m| m["runtime"].as_str().map(str::to_owned)).collect();
    let defaults: Vec<Value> = runtimes
        .iter()
        .filter(|r| *r != "kilo" && *r != crate::runtime::apple::APPLE_RUNTIME && !already.contains(r))
        .filter_map(|r| account_default_model(&models, r))
        .collect();
    [defaults, models].concat()
}

/// What each agent printed, as a sweep gathers it. None for one not installed or not answering.
#[derive(Debug, Default, Clone)]
pub struct Listed {
    pub codex: Option<Value>,
    /// What Codex's account has used, read in the same session as its models (Electron 0.390), in a turn's words.
    pub codex_usage: Option<String>,
    pub claude: Option<Value>,
    pub cursor: Option<Value>,
    pub opencode: Option<Value>,
    /// OpenCode 2's models read from its ACP `session/new` advertisement.
    pub opencode_acp: Vec<Value>,
    pub antigravity: Option<Value>,
    pub copilot: bool,
    pub muse: bool,
    /// Apple's on-device model answers on this Mac (`runtime::apple::probe`).
    pub apple: bool,
}

/// The catalog, in the order Electron's probe builds it: Codex's, then Claude Code, Cursor, OpenCode, Copilot,
/// Antigravity and Muse, each runtime's account default in front.
pub fn catalog(listed: &Listed) -> Vec<Value> {
    let mut models = listed.codex.as_ref().map(parse_codex_models).unwrap_or_default();
    models.extend(claude_models_from(listed.claude.as_ref()));
    models.extend(cursor_models_from(listed.cursor.as_ref()));
    models.extend(opencode_models_from(listed.opencode.as_ref()));
    models.extend(listed.opencode_acp.iter().cloned());
    if listed.copilot {
        models.extend(copilot_models());
    }
    models.extend(antigravity_models_from(listed.antigravity.as_ref()));
    if listed.muse {
        models.extend(muse_models());
    }
    if listed.apple {
        models.extend(apple_models());
    }
    with_account_defaults(models)
}

/// cursorChoiceFor: a Cursor route's level put into the id it names. A level the family does not list is left as it
/// came.
pub fn cursor_choice_for(models: &[Value], model: Option<&str>, effort: Option<&str>) -> (Option<String>, Option<String>) {
    let (Some(model), Some(effort)) = (model, effort) else {
        return (model.map(str::to_owned), effort.map(str::to_owned));
    };
    let family = models
        .iter()
        .find(|m| m["id"] == model || m["variants"].as_object().is_some_and(|v| v.values().any(|id| id == model)));
    match family.and_then(|f| f["variants"][effort].as_str()) {
        Some(id) => (Some(id.to_owned()), None),
        None => (Some(model.to_owned()), Some(effort.to_owned())),
    }
}

// ---- reading this computer ------------------------------------------------------------------------------------------

/// A list command's output (stdout and stderr), or None when it is not installed or did not finish in time.
fn output_of(program: &str, args: &[&str], limit: Duration) -> Option<String> {
    let path = crate::runtime::discovery::find_named(program).ok()?;
    let mut child = Command::new(path).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    let (tx, rx) = mpsc::channel();
    let mut stdout = child.stdout.take()?;
    let mut stderr = child.stderr.take()?;
    std::thread::spawn(move || {
        use std::io::Read;
        let mut out = String::new();
        let mut err = String::new();
        let _ = stdout.read_to_string(&mut out);
        let _ = stderr.read_to_string(&mut err);
        let _ = tx.send(format!("{out}\n{err}"));
    });
    match rx.recv_timeout(limit) {
        Ok(text) => {
            let _ = child.wait();
            Some(text)
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

/// Codex's `model/list`, from an app-server started for it and stopped after (no turn is started).
/// windowName (app-server-events.ts): a window by its length, else by the label Codex gave it.
fn window_name(minutes: Option<f64>, label: &str) -> String {
    match minutes {
        None => label.to_owned(),
        Some(n) if n >= 10_080. => "weekly".into(),
        Some(n) if n >= 1_440. => format!("{}-day", (n / 1_440.).round()),
        Some(n) if n >= 60. && n % 60. == 0. => format!("{}-hour", n / 60.),
        Some(n) => format!("{n}-minute"),
    }
}

/// usageWindowFromSnapshot (app-server-events.ts): a rate-limit snapshot in the words a turn's reading uses, the
/// fullest window first; None when it names no window.
pub fn codex_usage_from_snapshot(snapshot: &Value) -> Option<String> {
    let mut parts: Vec<(String, f64, Option<String>)> = Vec::new();
    for (label, value) in snapshot.as_object()? {
        if !value.is_object() {
            continue;
        }
        let Some(used) = value["usedPercent"].as_f64().or(value["used_percent"].as_f64()).or(value["utilization"].as_f64().map(|n| n * 100.)) else {
            continue;
        };
        let minutes = value["windowDurationMins"].as_f64().or(value["windowMinutes"].as_f64()).or(value["window_minutes"].as_f64());
        // Epoch seconds: a value this small would be 1970.
        let resets = value["resetsAt"]
            .as_f64()
            .or(value["resets_at"].as_f64())
            .filter(|seconds| *seconds >= 1_000_000_000.)
            .and_then(|seconds| chrono::DateTime::from_timestamp_millis((seconds * 1000.) as i64))
            .map(|at| at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
        // `+ 0.` so a rounded -0 reads as 0, as JavaScript's does.
        parts.push((format!("{} window", window_name(minutes, label)), used.round().clamp(0., 100.) + 0., resets));
    }
    if parts.is_empty() {
        return None;
    }
    parts.sort_by(|a, b| b.1.total_cmp(&a.1));
    let words: Vec<String> = parts.iter().map(|(label, used, resets)| format!("{label} {used}% used{}", resets.as_ref().map(|at| format!(" \u{b7} resets {at}")).unwrap_or_default())).collect();
    Some(words.join(" \u{b7} "))
}

/// The usage readings the catalog read with its models, by account (model-catalog.ts `usageWindows`): Codex's, when
/// its account said.
pub fn usage_windows(listed: &Listed) -> std::collections::HashMap<String, String> {
    listed.codex_usage.iter().map(|reading| ("codex".to_owned(), reading.clone())).collect()
}

fn codex_model_list(limit: Duration) -> Option<(Value, Option<String>)> {
    codex_list_when_ready(crate::agents::held_answer().as_deref(), limit, || {
        let path = crate::runtime::discovery::find_named("codex").ok()?;
        let mut command = Command::new(path);
        command.arg("app-server");
        Some(command)
    })
}

/// model-catalog.ts's probe fails soft, without spawning anything, when Codex is not ready: a Codex the last sweep
/// settled as not ready (signed out, a version that cannot run) is not started, and the catalog has no Codex models
/// (an empty catalog is not kept, so it is asked again). With no sweep held yet, or Codex still being checked, there
/// is no answer to go by, and the server is asked.
fn codex_list_when_ready(
    held: Option<&[crate::agents::Found]>,
    limit: Duration,
    command: impl FnOnce() -> Option<Command>,
) -> Option<(Value, Option<String>)> {
    let codex = held.and_then(|found| found.iter().find(|f| f.id == "codex"));
    if codex.is_some_and(|f| !f.ready && !f.checking) {
        return None;
    }
    codex_session(command()?, limit)
}

/// One Codex app-server session: its `model/list` result, and what the account has used. None when the models could
/// not be read.
fn codex_session(mut command: Command, limit: Duration) -> Option<(Value, Option<String>)> {
    // In its own process group, so the server is shut down whole once it has answered (model-catalog.ts's probe kills
    // the tree: "the process group elsewhere"). A `codex` that is a launcher for the real server would otherwise leave
    // the server running after the launcher was killed: a background process nobody asked for.
    crate::runtime::process::own_group(&mut command);
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut stdin = child.stdin.take()?;
    let stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&line)
                && tx.send(v).is_err()
            {
                break;
            }
        }
    });
    let deadline = Instant::now() + limit;
    let wait_for = |id: u64| -> Option<Value> {
        loop {
            let left = deadline.checked_duration_since(Instant::now())?;
            let v = rx.recv_timeout(left).ok()?;
            if v["id"] == id {
                return Some(v);
            }
        }
    };
    let mut send = |v: Value| writeln!(stdin, "{v}").ok();
    let result = (|| {
        send(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"clientInfo": {"name": "locust", "version": "0.1.0"}}}))?;
        wait_for(1)?;
        send(json!({"jsonrpc": "2.0", "method": "initialized"}))?;
        send(json!({"jsonrpc": "2.0", "id": 2, "method": "model/list", "params": {}}))?;
        let models = wait_for(2)?["result"].clone();
        // AND WHAT THE ACCOUNT HAS USED (Electron 0.390), in the same session: no turn spent. A reading otherwise only
        // ever came from a run. `account/rateLimits/read` returns the snapshot a turn would push; an older Codex without
        // the method, or an account that will not say, just leaves the reading out. It is read from the account itself,
        // so it counts use outside Locust too, and says when (0.406).
        let reading = send(json!({"jsonrpc": "2.0", "id": 3, "method": "account/rateLimits/read", "params": {}}))
            .and_then(|_| wait_for(3))
            .and_then(|v| codex_usage_from_snapshot(&v["result"]["rateLimits"]))
            .map(|reading| format!("{reading} \u{b7} as of {}", chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)));
        Some((models, reading))
    })();
    crate::runtime::process::kill(child.id());
    let _ = child.kill();
    let _ = child.wait();
    result
}

/// Every agent's list, read in parallel from this computer, each within 20 seconds.
pub fn read_listed() -> Listed {
    read_listed_checked().0
}

/// model-catalog.ts `listsItsModelsButListedNone`, over the runtimes that are ready and what each listed: an OpenCode,
/// Cursor or Claude Code that is ready and listed no models (Claude Code: no aliases either) means the list is not the
/// list. Its probe can fail on a busy machine while the runtime itself is fine (0.552).
pub fn lists_its_models_but_listed_none(ready: &[(&str, Option<&Value>)]) -> bool {
    ready.iter().any(|(id, hints)| {
        let none = |key: &str| hints.and_then(|h| h[key].as_array()).is_none_or(Vec::is_empty);
        matches!(*id, "opencode" | "cursor" | "claude") && none("models") && !(*id == "claude" && !none("aliases"))
    })
}

/// Whether Antigravity is installed (`available`) and has not yet answered (`unknown` readiness):
/// the model catalog must not be cached without it, whereas `ready`, `authentication-required`,
/// and `unavailable` are settled answers (`a-slow-antigravity-holds-nothing-up`).
pub fn antigravity_model_list_is_pending(installed: bool, status: crate::agents::Probe) -> bool {
    installed && status == crate::agents::Probe::ProbeFailed
}

/// `read_listed`, and whether a ready runtime listed nothing, so the answer is not kept. OpenCode 2 lists its models in
/// an ACP session started in parallel with the other probes; a failed V2 advertisement marks the catalog incomplete.
pub fn read_listed_checked() -> (Listed, bool) {
    let limit = Duration::from_secs(20);
    let codex = std::thread::spawn(move || codex_model_list(limit));
    let claude = std::thread::spawn(move || output_of("claude", &["--help"], limit).as_deref().and_then(parse_claude_model_hints));
    let cursor = std::thread::spawn(move || output_of("cursor-agent", &["--list-models"], limit).as_deref().and_then(parse_cursor_model_list));
    // (whether OpenCode answered as 1.x, whether OpenCode answered as 2.x, V1 hints, V2 ACP models)
    let opencode = std::thread::spawn(move || {
        let Ok(path) = crate::runtime::discovery::find_named("opencode") else {
            return (false, false, None, Vec::new());
        };
        let Some(version) = output_of("opencode", &["--version"], limit) else {
            return (false, false, None, Vec::new());
        };
        if let Some(parsed) = crate::runtime::version::parse(&version) {
            if parsed.major == 2 {
                let rt = crate::opencode_acp_models::ProbeRuntime::for_path(
                    "opencode",
                    path.to_string_lossy().into_owned(),
                    Some(2),
                );
                let models = crate::opencode_acp_models::read_opencode_acp_models_default(&rt, limit);
                return (false, true, None, models);
            }
            if parsed.major >= 3 {
                return (false, false, None, Vec::new());
            }
        }
        (
            true,
            false,
            output_of("opencode", &["models", "--verbose"], limit)
                .as_deref()
                .and_then(parse_opencode_model_list),
            Vec::new(),
        )
    });
    let agy = std::thread::spawn(move || output_of("agy", &["models"], limit).as_deref().and_then(parse_agy_model_list));
    let (opencode_one, opencode_two, opencode, opencode_acp) = opencode.join().unwrap_or((false, false, None, Vec::new()));
    let (codex, codex_usage) = codex.join().ok().flatten().map_or((None, None), |(models, usage)| (Some(models), usage));
    let listed = Listed {
        codex,
        codex_usage,
        claude: claude.join().ok().flatten(),
        cursor: cursor.join().ok().flatten(),
        opencode,
        opencode_acp,
        antigravity: agy.join().ok().flatten(),
        copilot: crate::runtime::discovery::find_named("copilot").is_ok(),
        muse: crate::runtime::discovery::find_named("muse").is_ok(),
        apple: crate::runtime::apple::available(),
    };
    let installed = |name: &str| crate::runtime::discovery::find_named(name).is_ok();
    let mut ready: Vec<(&str, Option<&Value>)> = Vec::new();
    if opencode_one {
        ready.push(("opencode", listed.opencode.as_ref()));
    }
    if installed("cursor-agent") {
        ready.push(("cursor", listed.cursor.as_ref()));
    }
    if installed("claude") {
        ready.push(("claude", listed.claude.as_ref()));
    }
    let incomplete = lists_its_models_but_listed_none(&ready)
        || (opencode_two && listed.opencode_acp.is_empty());
    (listed, incomplete)
}

/// Probes a discovered runtime set in parallel (Codex `app-server` and OpenCode 2 ACP concurrently),
/// keeping V1 discovery-only and marking a failed OpenCode 2 ACP read incomplete so `kept_catalog` retries it.
pub fn read_discovered_checked(
    runtimes: &[crate::opencode_acp_models::ProbeRuntime],
    spawn: crate::opencode_acp_models::ProbeSpawn<'_>,
    scratch: Option<crate::opencode_acp_models::ScratchFactory>,
    deadline: Duration,
) -> (Listed, bool) {
    let opencode = runtimes.iter().find(|r| r.id == "opencode");
    let codex = runtimes.iter().find(|r| r.id == "codex");
    let opencode_v2 = opencode.is_some_and(|r| r.ready && r.major == Some(2));

    let (opencode_acp, codex_result) = std::thread::scope(|scope| {
        let oc_handle = if opencode_v2 {
            let rt = opencode.unwrap();
            Some(scope.spawn(move || {
                crate::opencode_acp_models::read_opencode_acp_models(
                    rt,
                    crate::opencode_acp_models::OpenCodeAcpModelsOptions {
                        spawn,
                        scratch,
                        runtime: "opencode",
                        deadline,
                    },
                )
            }))
        } else {
            None
        };
        let cx_handle = if let Some(rt) = codex.filter(|r| r.ready && r.executable.is_some()) {
            Some(scope.spawn(move || probe_codex_models_with(rt, spawn, deadline)))
        } else {
            None
        };
        let oc = oc_handle.and_then(|h| h.join().ok()).unwrap_or_default();
        let cx = cx_handle.and_then(|h| h.join().ok()).flatten();
        (oc, cx)
    });

    let opencode_v1_hints = opencode
        .filter(|r| r.ready && r.major != Some(2))
        .and_then(|r| r.hints.clone());

    let listed = Listed {
        codex: codex_result,
        opencode: opencode_v1_hints,
        opencode_acp,
        ..Default::default()
    };

    let mut ready: Vec<(&str, Option<&Value>)> = Vec::new();
    if let Some(oc) = opencode.filter(|r| r.ready && r.major != Some(2)) {
        ready.push(("opencode", oc.hints.as_ref()));
    }
    let incomplete = lists_its_models_but_listed_none(&ready)
        || (opencode_v2 && listed.opencode_acp.is_empty());
    (listed, incomplete)
}

fn probe_codex_models_with(
    runtime: &crate::opencode_acp_models::ProbeRuntime,
    spawn: crate::opencode_acp_models::ProbeSpawn<'_>,
    limit: Duration,
) -> Option<Value> {
    let launch = runtime.executable.as_ref()?;
    let mut args = launch.prefix_args.clone();
    args.push("app-server".to_owned());
    let (mut child, rx) = spawn(&launch.executable_path, &args, &launch.env, std::path::Path::new("."))
        .ok()?;
    let deadline = Instant::now() + limit;
    let mut buffer: Vec<u8> = Vec::new();
    let mut wait_for = |id: u64| -> Option<Value> {
        loop {
            while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                let raw: Vec<u8> = buffer.drain(..=pos).collect();
                if let Ok(v) = serde_json::from_slice::<Value>(&raw)
                    && v["id"] == id
                {
                    return Some(v);
                }
            }
            let remaining = deadline.checked_duration_since(Instant::now())?;
            match rx.recv_timeout(remaining).ok()? {
                crate::opencode_acp_models::ProbeEvent::Data(chunk) => buffer.extend_from_slice(&chunk),
                crate::opencode_acp_models::ProbeEvent::StdoutClosed
                | crate::opencode_acp_models::ProbeEvent::Exited => return None,
            }
        }
    };
    let result = (|| {
        child.write_line(&format!("{}\n", json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"clientInfo": {"name": "locust", "version": "0.1.0"}}}))).ok()?;
        wait_for(1)?;
        child.write_line(&format!("{}\n", json!({"jsonrpc": "2.0", "method": "initialized"}))).ok()?;
        child.write_line(&format!("{}\n", json!({"jsonrpc": "2.0", "id": 2, "method": "model/list", "params": {}}))).ok()?;
        Some(wait_for(2)?["result"].clone())
    })();
    let _ = child.kill();
    result
}

/// A catalog kept for CACHE_MS (ten minutes) once it is whole; one missing a ready runtime's models is read again on
/// the next call instead of being kept (0.552), and so is one that listed nothing at all (Electron's
/// `MODELS_UNAVAILABLE`, never cached). One read at a time: the cache stays locked while it reads, so a second caller
/// waits for the first read and is answered from it rather than starting a second sweep.
pub fn kept_catalog(cache: &std::sync::Mutex<Option<(Instant, Vec<Value>)>>, read: impl FnOnce() -> (Listed, bool)) -> Vec<Value> {
    let mut held = crate::invariants::lock(cache);
    if let Some((_, models)) = held.as_ref().filter(|(at, _)| at.elapsed() < Duration::from_secs(600)) {
        return models.clone();
    }
    let (listed, incomplete) = read();
    let models = catalog(&listed);
    if !incomplete && !models.is_empty() {
        *held = Some((Instant::now(), models.clone()));
    }
    models
}

/// The catalog for this computer, read once and kept ten minutes (CACHE_MS): models change on release, not hourly.
pub fn local() -> Vec<Value> {
    use std::sync::Mutex;
    static CACHE: Mutex<Option<(Instant, Vec<Value>)>> = Mutex::new(None);
    let mut models = kept_catalog(&CACHE, read_listed_checked);
    // Your own models under OpenCode and your own ACP agents, read fresh: one added a moment ago is offered at once.
    if let Ok(store) = crate::store::Store::local() {
        models.extend(crate::own_models::for_store(&store).catalog().unwrap_or_default());
        models.extend(crate::acp_agents::acp_catalog(&crate::acp_agents::agents_for_store(&store)));
    }
    models
}

#[cfg(test)]
#[path = "models_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "muse_models_tests.rs"]
mod muse_models_tests;

#[cfg(test)]
#[path = "codex_usage_is_read_with_the_models_tests.rs"]
mod codex_usage_is_read_with_the_models_tests;

#[cfg(all(test, unix))]
#[path = "model_catalog_probe_tests.rs"]
mod model_catalog_probe_tests;
