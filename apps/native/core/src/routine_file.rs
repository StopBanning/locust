//! A routine as a file (`main/routine-file.ts`): `<name>.routine.md` or `<name>.locust-routine.json`.
//!
//! What travels is what a routine IS — its steps, what it asks for, the kind of
//! route it was made for, the connectors its steps name, and `when: hook` if
//! it is a called-link routine. Machine-local ids, run counts, clock schedules,
//! and execution state never travel.

use crate::routine_inputs::{RoutineInput, inputs_refusal, kept_inputs, parse_inputs_array};
use serde_json::{Map, Value, json};
use std::collections::HashSet;

pub const ROUTINE_FILE_FORMAT: &str = "locust-routine";
pub const ROUTINE_FILE_VERSION: u64 = 1;
pub const ROUTINE_FILE_KEYS: &[&str] = &[
    "format",
    "version",
    "name",
    "steps",
    "inputs",
    "handOffs",
    "route",
    "connectors",
    "schedule",
];
pub const MAX_ROUTINE_FILE_BYTES: usize = 512 * 1024;
const MAX_CONNECTORS: usize = 24;

const MARKDOWN_FIELDS: &[&str] = &["locust-routine", "runs-on", "connectors", "inputs", "when"];
const CHECKS_THE_WORK: &str = "checks the work";
const ASKS_FIRST: &str = "waits for your OK";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutineFileSchedule {
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RoutineFileHandOff {
    pub role: Option<String>,
    pub check: Option<bool>,
    pub ask_first: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RoutineFileRoute {
    pub runtime: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutineFile {
    pub format: String,
    pub version: u64,
    pub name: String,
    pub schedule: Option<RoutineFileSchedule>,
    pub steps: Vec<String>,
    pub inputs: Vec<RoutineInput>,
    pub hand_offs: Vec<RoutineFileHandOff>,
    pub route: RoutineFileRoute,
    pub connectors: Vec<String>,
}

impl RoutineFile {
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("format".into(), json!(self.format));
        obj.insert("version".into(), json!(self.version));
        obj.insert("name".into(), json!(self.name));
        if let Some(sched) = &self.schedule {
            obj.insert("schedule".into(), json!({"kind": sched.kind}));
        }
        obj.insert("steps".into(), json!(self.steps));
        obj.insert(
            "inputs".into(),
            Value::Array(self.inputs.iter().map(RoutineInput::to_json).collect()),
        );
        let hos: Vec<Value> = self
            .hand_offs
            .iter()
            .map(|h| {
                let mut m = Map::new();
                if let Some(r) = &h.role {
                    m.insert("role".into(), json!(r));
                }
                if h.check == Some(true) {
                    m.insert("check".into(), json!(true));
                }
                if h.ask_first == Some(true) {
                    m.insert("askFirst".into(), json!(true));
                }
                Value::Object(m)
            })
            .collect();
        obj.insert("handOffs".into(), Value::Array(hos));
        let mut route_obj = Map::new();
        if let Some(rt) = &self.route.runtime {
            route_obj.insert("runtime".into(), json!(rt));
        }
        obj.insert("route".into(), Value::Object(route_obj));
        obj.insert("connectors".into(), json!(self.connectors));
        Value::Object(obj)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlaggedPath {
    pub step: usize,
    pub path: String,
}

fn starts_path_at(s: &str, idx: usize) -> bool {
    let rest = &s[idx..];
    if rest.starts_with("/Users/") || rest.starts_with("/home/") {
        return true;
    }
    let bytes = rest.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

struct PathMatch {
    start: usize,
    end: usize,
    quote: Option<char>,
    path: String,
    tail: String,
}

fn find_path_matches(step: &str) -> Vec<PathMatch> {
    let mut out = Vec::new();
    let mut i = 0;
    let bytes = step.as_bytes();
    while i < bytes.len() {
        let ch = step[i..].chars().next().unwrap();
        if ch == '"' || ch == '\'' || ch == '`' {
            let after_quote = i + ch.len_utf8();
            if after_quote < bytes.len() && starts_path_at(step, after_quote) {
                let rest = &step[after_quote..];
                if let Some(rel_end) = rest.find(|c: char| c == ch || c == '\r' || c == '\n') {
                    let candidate = &rest[..rel_end];
                    let close_idx = after_quote + rel_end;
                    if !candidate.is_empty() && step[close_idx..].starts_with(ch) {
                        out.push(PathMatch {
                            start: i,
                            end: close_idx + ch.len_utf8(),
                            quote: Some(ch),
                            path: candidate.to_owned(),
                            tail: String::new(),
                        });
                        i = close_idx + ch.len_utf8();
                        continue;
                    }
                }
            }
        }
        let prev_ok = if i == 0 {
            true
        } else {
            let prev = step[..i].chars().next_back().unwrap();
            prev.is_whitespace() || matches!(prev, '(' | '=' | '[' | '<')
        };
        if prev_ok && starts_path_at(step, i) {
            let rest = &step[i..];
            let end_rel = rest
                .find(|c: char| {
                    c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '<' | '>' | '|' | '*' | '?' | '(' | ')')
                })
                .unwrap_or(rest.len());
            let bare = &rest[..end_rel];
            let trimmed = bare.trim_end_matches(['.', ',', ';', ':', '!']);
            let tail = bare[trimmed.len()..].to_owned();
            out.push(PathMatch {
                start: i,
                end: i + end_rel,
                quote: None,
                path: trimmed.to_owned(),
                tail,
            });
            i += end_rel;
            continue;
        }
        i += ch.len_utf8();
    }
    out
}

/// The absolute paths the steps name, by step (counting from 1), trailing punctuation left off.
pub fn absolute_paths_in(steps: &[String]) -> Vec<FlaggedPath> {
    let mut out = Vec::new();
    for (at, step) in steps.iter().enumerate() {
        for m in find_path_matches(step) {
            out.push(FlaggedPath {
                step: at + 1,
                path: m.path,
            });
        }
    }
    out
}

/// Turn each absolute path in `steps` into a text input (`path`, `path_2`, ...).
pub fn paths_as_inputs(
    steps: &[String],
    inputs: &[RoutineInput],
) -> Result<(Vec<String>, Vec<RoutineInput>), String> {
    let mut made: Vec<RoutineInput> = inputs.to_vec();
    let mut key_of: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut next_steps = Vec::with_capacity(steps.len());
    for step in steps {
        let matches = find_path_matches(step);
        if matches.is_empty() {
            next_steps.push(step.clone());
            continue;
        }
        let mut built = String::with_capacity(step.len());
        let mut cursor = 0;
        for m in matches {
            built.push_str(&step[cursor..m.start]);
            let key = if let Some(existing) = key_of.get(&m.path) {
                existing.clone()
            } else {
                let mut num = 1usize;
                let k = loop {
                    let candidate = if num == 1 {
                        "path".to_owned()
                    } else {
                        format!("path_{num}")
                    };
                    if !made.iter().any(|i| i.key == candidate) {
                        break candidate;
                    }
                    num += 1;
                };
                let label = if k == "path" {
                    "Path".to_owned()
                } else {
                    format!("Path {}", k.trim_start_matches("path_"))
                };
                key_of.insert(m.path.clone(), k.clone());
                made.push(RoutineInput {
                    key: k.clone(),
                    label,
                    kind: crate::routine_inputs::RoutineInputKind::Text,
                    required: true,
                    default: None,
                    choices: None,
                });
                k
            };
            if let Some(q) = m.quote {
                built.push(q);
                built.push_str("{{");
                built.push_str(&key);
                built.push_str("}}");
                built.push(q);
            } else {
                built.push_str("{{");
                built.push_str(&key);
                built.push_str("}}");
            }
            built.push_str(&m.tail);
            cursor = m.end;
        }
        built.push_str(&step[cursor..]);
        next_steps.push(built);
    }
    if made.len() > crate::routine_inputs::MAX_INPUTS {
        return Err(format!(
            "That would be {} inputs; a routine asks for at most {}.",
            made.len(),
            crate::routine_inputs::MAX_INPUTS
        ));
    }
    Ok((next_steps, made))
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// Each connector (of those this machine has) that a step names, by its name, ignoring case.
pub fn connectors_named_in(steps: &[String], names: &[String]) -> Vec<String> {
    let text = steps.join("\n").to_lowercase();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for name in names {
        if name.is_empty() || !seen.insert(name.as_str()) {
            continue;
        }
        let needle = name.to_lowercase();
        let mut search_from = 0;
        let mut matched = false;
        while let Some(pos) = text[search_from..].find(&needle) {
            let abs_start = search_from + pos;
            let abs_end = abs_start + needle.len();
            let left_ok = abs_start == 0 || !is_ident_char(text.as_bytes()[abs_start - 1]);
            let right_ok = abs_end == text.len() || !is_ident_char(text.as_bytes()[abs_end]);
            if left_ok && right_ok {
                matched = true;
                break;
            }
            search_from = abs_start + 1;
        }
        if matched {
            out.push(name.clone());
        }
    }
    out
}

/// Build the portable `RoutineFile` for a stored routine.
pub fn routine_to_file(
    routine: &Value,
    steps: &[String],
    inputs: &[RoutineInput],
    role_of: &dyn Fn(&str) -> Option<String>,
    connector_names: &[String],
) -> RoutineFile {
    let owner = routine["teammateId"].as_str().unwrap_or("");
    let held_handoffs = routine["handOffs"].as_array();
    let is_chain = held_handoffs
        .is_some_and(|arr| arr.iter().any(|h| h["check"] == true));
    let hand_offs = (0..steps.len())
        .map(|at| {
            let entry = held_handoffs.and_then(|arr| arr.get(at));
            let who = entry.and_then(|e| e["teammateId"].as_str());
            let named = match who {
                None => {
                    if is_chain && !owner.is_empty() {
                        Some(owner)
                    } else {
                        None
                    }
                }
                Some(w) if w == owner => {
                    if is_chain {
                        Some(owner)
                    } else {
                        None
                    }
                }
                Some(w) => Some(w),
            };
            let role = named.and_then(role_of);
            let check = entry.is_some_and(|e| e["check"] == true).then_some(true);
            let ask_first = (at > 0 && entry.is_some_and(|e| e["askFirst"] == true)).then_some(true);
            RoutineFileHandOff {
                role,
                check,
                ask_first,
            }
        })
        .collect();
    let schedule = (routine["schedule"]["kind"] == "hook").then(|| RoutineFileSchedule {
        kind: "hook".into(),
    });
    let runtime = routine["route"]["runtime"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);
    RoutineFile {
        format: ROUTINE_FILE_FORMAT.into(),
        version: ROUTINE_FILE_VERSION,
        name: routine["name"].as_str().unwrap_or("").trim().to_owned(),
        schedule,
        steps: steps.to_vec(),
        inputs: kept_inputs(inputs),
        hand_offs,
        route: RoutineFileRoute { runtime },
        connectors: connectors_named_in(steps, connector_names),
    }
}

/// The JSON text of the file.
pub fn routine_file_text(file: &RoutineFile) -> String {
    let mut s = serde_json::to_string_pretty(&file.to_json()).unwrap_or_else(|_| "{}".into());
    s.push('\n');
    s
}

fn yaml_scalar(s: &str) -> String {
    if s.contains('\n') || s.contains('\r') {
        return serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    }
    let needs_quotes = s.is_empty()
        || s != s.trim()
        || matches!(
            s.to_ascii_lowercase().as_str(),
            "true" | "false" | "null" | "~" | "yes" | "no" | "on" | "off"
        )
        || s.parse::<f64>().is_ok()
        || s.starts_with(['-', '?', ':', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`'])
        || s.contains(": ")
        || s.contains(" #");
    if needs_quotes {
        serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
    } else {
        s.to_owned()
    }
}

/// The routine as Markdown (`<name>.routine.md`).
pub fn routine_markdown(file: &RoutineFile) -> String {
    let mut front = String::from("---\n");
    front.push_str(&format!("locust-routine: {}\n", file.version));
    if file.schedule.as_ref().is_some_and(|s| s.kind == "hook") {
        front.push_str("when: hook\n");
    }
    if let Some(rt) = &file.route.runtime {
        front.push_str(&format!("runs-on: {}\n", yaml_scalar(rt)));
    }
    if !file.connectors.is_empty() {
        front.push_str("connectors:\n");
        for c in &file.connectors {
            front.push_str(&format!("  - {}\n", yaml_scalar(c)));
        }
    }
    if !file.inputs.is_empty() {
        front.push_str("inputs:\n");
        for input in &file.inputs {
            front.push_str(&format!("  - key: {}\n", yaml_scalar(&input.key)));
            front.push_str(&format!("    label: {}\n", yaml_scalar(&input.label)));
            front.push_str(&format!("    kind: {}\n", input.kind.as_str()));
            front.push_str(&format!("    required: {}\n", input.required));
            if let Some(def) = &input.default {
                front.push_str(&format!("    default: {}\n", yaml_scalar(def)));
            }
            if let Some(choices) = &input.choices {
                front.push_str("    choices:\n");
                for ch in choices {
                    front.push_str(&format!("      - {}\n", yaml_scalar(ch)));
                }
            }
        }
    }
    front.push_str("---\n\n");
    let mut out = front;
    out.push_str(&format!("# {}\n\n", file.name));
    for (at, step) in file.steps.iter().enumerate() {
        if at > 0 {
            out.push('\n');
        }
        let h = file.hand_offs.get(at);
        let mut heading = format!("## Step {}", at + 1);
        if let Some(role) = h.and_then(|x| x.role.as_deref()) {
            heading.push_str(&format!(" · {role}"));
        }
        if h.is_some_and(|x| x.check == Some(true)) {
            heading.push_str(&format!(" · {CHECKS_THE_WORK}"));
        }
        if h.is_some_and(|x| x.ask_first == Some(true)) {
            heading.push_str(&format!(" · {ASKS_FIRST}"));
        }
        out.push_str(&heading);
        out.push_str("\n\n");
        out.push_str(step);
        out.push('\n');
    }
    out
}

/// What an export writes: the Markdown when it carries the routine exactly, else JSON.
pub fn routine_export_text(file: &RoutineFile) -> (String, bool) {
    let md = routine_markdown(file);
    if let Ok(back) = parse_routine_file(&md)
        && back == *file
    {
        return (md, true);
    }
    (routine_file_text(file), false)
}

/// `<name>.routine.md` or `<name>.locust-routine.json`, with a filesystem-safe name.
pub fn routine_file_name(name: &str, markdown: bool) -> String {
    let replaced: String = name
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (c as u32) < 0x20 {
                ' '
            } else {
                c
            }
        })
        .collect();
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_end_matches(['.', ' ']);
    let plain: String = trimmed.chars().take(60).collect();
    let stem = if plain.is_empty() { "routine" } else { &plain };
    if markdown {
        format!("{stem}.routine.md")
    } else {
        format!("{stem}.locust-routine.json")
    }
}

fn one_line(v: &Value, max_utf16: usize) -> bool {
    v.as_str().is_some_and(|s| {
        !s.trim().is_empty()
            && s.encode_utf16().count() <= max_utf16
            && !s.chars().any(|c| c <= '\u{1f}' || c == '\u{7f}')
    })
}

fn parse_yaml_scalar(raw: &str) -> Result<String, ()> {
    let t = raw.trim();
    if t.starts_with('&') || t.starts_with('*') {
        return Err(());
    }
    if t.starts_with('[') && !t.ends_with(']') {
        return Err(());
    }
    if t.starts_with('{') && !t.ends_with('}') {
        return Err(());
    }
    if t.starts_with('"') {
        if !t.ends_with('"') || t.len() < 2 {
            return Err(());
        }
        return serde_json::from_str::<String>(t).map_err(|_| ());
    }
    if t.starts_with('\'') {
        if !t.ends_with('\'') || t.len() < 2 {
            return Err(());
        }
        return Ok(t[1..t.len() - 1].replace("''", "'"));
    }
    Ok(t.to_owned())
}

fn parse_frontmatter_yaml(lines: &[&str]) -> Result<Map<String, Value>, ()> {
    let mut root = Map::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent != 0 {
            return Err(());
        }
        let Some(colon) = trimmed.find(':') else {
            return Err(());
        };
        let key = trimmed[..colon].trim();
        let rest = trimmed[colon + 1..].trim();
        if key.is_empty() {
            return Err(());
        }
        if rest.starts_with('&') || rest.starts_with('*') {
            return Err(());
        }
        if rest.is_empty() {
            // Expect a block list under this key.
            i += 1;
            let mut items: Vec<Value> = Vec::new();
            while i < lines.len() {
                let sub = lines[i];
                let sub_trim = sub.trim();
                if sub_trim.is_empty() || sub_trim.starts_with('#') {
                    i += 1;
                    continue;
                }
                let sub_indent = sub.len() - sub.trim_start().len();
                if sub_indent == 0 {
                    break;
                }
                if !sub_trim.starts_with("- ") && sub_trim != "-" {
                    return Err(());
                }
                let after_dash = sub_trim.strip_prefix('-').unwrap().trim();
                if after_dash.contains(':') && !after_dash.starts_with('"') && !after_dash.starts_with('\'') {
                    // Map item in list (e.g. inputs entry).
                    let item_indent = sub_indent + 2;
                    let mut obj = Map::new();
                    let Some(c) = after_dash.find(':') else {
                        return Err(());
                    };
                    let k0 = after_dash[..c].trim();
                    let v0 = after_dash[c + 1..].trim();
                    if v0.is_empty() {
                        obj.insert(k0.into(), Value::Array(Vec::new()));
                    } else {
                        obj.insert(k0.into(), Value::String(parse_yaml_scalar(v0)?));
                    }
                    i += 1;
                    while i < lines.len() {
                        let prop = lines[i];
                        let prop_trim = prop.trim();
                        if prop_trim.is_empty() || prop_trim.starts_with('#') {
                            i += 1;
                            continue;
                        }
                        let prop_indent = prop.len() - prop.trim_start().len();
                        if prop_indent < item_indent || prop_trim.starts_with("- ") {
                            break;
                        }
                        let Some(pc) = prop_trim.find(':') else {
                            return Err(());
                        };
                        let pk = prop_trim[..pc].trim();
                        let pv = prop_trim[pc + 1..].trim();
                        if pv.is_empty() {
                            // Nested list (e.g. choices).
                            i += 1;
                            let mut sub_list = Vec::new();
                            while i < lines.len() {
                                let cl = lines[i];
                                let ct = cl.trim();
                                if ct.is_empty() || ct.starts_with('#') {
                                    i += 1;
                                    continue;
                                }
                                let ci = cl.len() - cl.trim_start().len();
                                if ci <= prop_indent || !ct.starts_with("- ") {
                                    break;
                                }
                                sub_list.push(Value::String(parse_yaml_scalar(ct.strip_prefix("- ").unwrap())?));
                                i += 1;
                            }
                            obj.insert(pk.into(), Value::Array(sub_list));
                        } else {
                            obj.insert(pk.into(), Value::String(parse_yaml_scalar(pv)?));
                            i += 1;
                        }
                    }
                    items.push(Value::Object(obj));
                } else {
                    items.push(Value::String(parse_yaml_scalar(after_dash)?));
                    i += 1;
                }
            }
            root.insert(key.into(), Value::Array(items));
        } else if rest.starts_with('[') && rest.ends_with(']') {
            let inner = rest[1..rest.len() - 1].trim();
            let mut items = Vec::new();
            if !inner.is_empty() {
                for part in inner.split(',') {
                    items.push(Value::String(parse_yaml_scalar(part)?));
                }
            }
            root.insert(key.into(), Value::Array(items));
            i += 1;
        } else {
            root.insert(key.into(), Value::String(parse_yaml_scalar(rest)?));
            i += 1;
        }
    }
    if root.is_empty() {
        return Err(());
    }
    Ok(root)
}

fn parse_step_heading(line: &str) -> Option<(usize, &str)> {
    let rest = line.strip_prefix("## Step ")?;
    let digits_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    if digits_end == 0 {
        return None;
    }
    let num: usize = rest[..digits_end].parse().ok()?;
    let suffix = &rest[digits_end..];
    if suffix.is_empty() || suffix.starts_with(" · ") {
        Some((num, suffix))
    } else {
        None
    }
}

fn markdown_fields(text: &str) -> Result<Value, String> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let Some(close) = lines.iter().enumerate().skip(1).find_map(|(i, l)| (*l == "---").then_some(i)) else {
        return Err("That routine file opens its settings with --- and never closes them.".into());
    };
    let top = parse_frontmatter_yaml(&lines[1..close]).map_err(|()| {
        "That routine file's settings, between its --- lines, are not readable.".to_string()
    })?;
    if let Some(when) = top.get("when")
        && when.as_str() != Some("hook")
    {
        return Err("That routine file has a trigger Locust does not read.".into());
    }
    if let Some(stray) = top.keys().find(|k| !MARKDOWN_FIELDS.contains(&k.as_str())) {
        let short: String = stray.chars().take(40).collect();
        return Err(format!(
            "That routine file has a setting Locust does not read ({short}), so it was not opened."
        ));
    }
    let Some(version) = top.get("locust-routine") else {
        return Err(
            "That file is not a Locust routine: its settings do not start with locust-routine."
                .into(),
        );
    };
    if version.as_str() != Some("1") {
        let v_str: String = version.as_str().unwrap_or("").chars().take(12).collect();
        return Err(format!(
            "That routine file is version {v_str}; this Locust reads version {ROUTINE_FILE_VERSION}."
        ));
    }

    let body = &lines[close + 1..];
    let Some(title_at) = body.iter().position(|l| !l.trim().is_empty()) else {
        return Err(
            "That routine file needs its name as a title, a line starting with # , after its settings."
                .into(),
        );
    };
    let Some(name) = body[title_at].strip_prefix("# ").filter(|n| !n.is_empty()) else {
        return Err(
            "That routine file needs its name as a title, a line starting with # , after its settings."
                .into(),
        );
    };

    let mut steps: Vec<String> = Vec::new();
    let mut hand_offs: Vec<Value> = Vec::new();
    let mut current: Option<Vec<&str>> = None;

    let finish = |cur: &mut Option<Vec<&str>>, steps: &mut Vec<String>| {
        let Some(mut lines) = cur.take() else {
            return;
        };
        while lines.first().is_some_and(|l| l.trim().is_empty()) {
            lines.remove(0);
        }
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        steps.push(lines.join("\n"));
    };

    for &line in &body[title_at + 1..] {
        let expected_num = steps.len() + if current.is_none() { 1 } else { 2 };
        if let Some((num, suffix)) = parse_step_heading(line)
            && num == expected_num
        {
            let was_none = current.is_none();
            finish(&mut current, &mut steps);
            current = Some(Vec::new());
            let mut parts: Vec<&str> = if suffix.is_empty() {
                Vec::new()
            } else {
                suffix
                    .split(" · ")
                    .skip(1)
                    .map(str::trim)
                    .collect()
            };
            let ask_first = parts.last().is_some_and(|p| *p == ASKS_FIRST);
            if ask_first {
                parts.pop();
            }
            let check = parts.last().is_some_and(|p| *p == CHECKS_THE_WORK);
            if check {
                parts.pop();
            }
            if parts.len() > 1 {
                return Err(format!("Step {num}'s heading names more than one teammate."));
            }
            if ask_first && steps.is_empty() && was_none {
                return Err("Step 1 cannot wait for your OK: it starts when you run the routine.".into());
            }
            let mut h = Map::new();
            if let Some(role) = parts.first() {
                h.insert("role".into(), json!(role));
            }
            if check {
                h.insert("check".into(), json!(true));
            }
            if ask_first {
                h.insert("askFirst".into(), json!(true));
            }
            hand_offs.push(Value::Object(h));
        } else if let Some(cur) = &mut current {
            cur.push(line);
        } else if !line.trim().is_empty() {
            return Err("That routine file has text between its title and ## Step 1. Put it in a step, or take it out.".into());
        }
    }
    finish(&mut current, &mut steps);
    if steps.is_empty() {
        return Err("That routine file has no steps: each starts with a line ## Step 1, ## Step 2 and so on.".into());
    }

    let inputs = match top.get("inputs") {
        None => Value::Array(Vec::new()),
        Some(Value::Array(arr)) => Value::Array(
            arr.iter()
                .map(|entry| {
                    if let Some(obj) = entry.as_object()
                        && let Some(req) = obj.get("required").and_then(Value::as_str)
                        && (req == "true" || req == "false")
                    {
                        let mut cloned = obj.clone();
                        cloned.insert("required".into(), Value::Bool(req == "true"));
                        Value::Object(cloned)
                    } else {
                        entry.clone()
                    }
                })
                .collect(),
        ),
        Some(other) => other.clone(),
    };

    let mut out = Map::new();
    out.insert("format".into(), json!(ROUTINE_FILE_FORMAT));
    out.insert("version".into(), json!(ROUTINE_FILE_VERSION));
    out.insert("name".into(), json!(name));
    if top.get("when").and_then(Value::as_str) == Some("hook") {
        out.insert("schedule".into(), json!({"kind": "hook"}));
    }
    out.insert("steps".into(), json!(steps));
    out.insert("inputs".into(), inputs);
    out.insert("handOffs".into(), Value::Array(hand_offs));
    out.insert(
        "route".into(),
        match top.get("runs-on") {
            Some(r) => json!({"runtime": r}),
            None => json!({}),
        },
    );
    out.insert(
        "connectors".into(),
        top.get("connectors").cloned().unwrap_or_else(|| json!([])),
    );
    Ok(Value::Object(out))
}

fn is_markdown(text: &str) -> bool {
    text.starts_with("---\n") || text.starts_with("---\r\n") || text.starts_with("---\r")
}

/// Read a routine file (Markdown or JSON), refusing anything invalid.
pub fn parse_routine_file(text: &str) -> Result<RoutineFile, String> {
    if text.len() > MAX_ROUTINE_FILE_BYTES {
        return Err("That file is too large to be a routine.".into());
    }
    let plain = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let value: Value = if is_markdown(plain) {
        markdown_fields(plain)?
    } else {
        serde_json::from_str(plain).map_err(|_| {
            "That file is not a routine: it is neither a routine in Markdown nor readable JSON."
                .to_string()
        })?
    };
    let Some(obj) = value.as_object() else {
        return Err("That file is not a routine.".into());
    };
    if obj.get("format").and_then(Value::as_str) != Some(ROUTINE_FILE_FORMAT) {
        return Err(match obj.get("format").and_then(Value::as_str) {
            Some(f) if f.len() <= 40 => format!("That file is a \"{f}\" file, not a Locust routine."),
            _ => "That file is not a Locust routine.".into(),
        });
    }
    if obj.get("version").and_then(Value::as_u64) != Some(ROUTINE_FILE_VERSION) {
        return Err(match obj.get("version").and_then(Value::as_u64) {
            Some(v) => format!(
                "That routine file is version {v}; this Locust reads version {ROUTINE_FILE_VERSION}."
            ),
            None => "That routine file has no version this Locust reads.".into(),
        });
    }
    if let Some(stray) = obj.keys().find(|k| !ROUTINE_FILE_KEYS.contains(&k.as_str())) {
        let short: String = stray.chars().take(40).collect();
        return Err(format!(
            "That routine file has a field Locust does not read ({short}), so it was not opened."
        ));
    }
    if let Some(missing) = ROUTINE_FILE_KEYS
        .iter()
        .find(|&&k| k != "schedule" && !obj.contains_key(k))
    {
        return Err(format!("That routine file is missing its {missing} field."));
    }
    if let Some(sched) = obj.get("schedule") {
        let ok = sched
            .as_object()
            .is_some_and(|s| s.len() == 1 && s.get("kind").and_then(Value::as_str) == Some("hook"));
        if !ok {
            return Err("That routine file has a trigger Locust does not read.".into());
        }
    }
    let name = obj["name"].as_str().unwrap_or("").trim();
    if name.is_empty()
        || name.encode_utf16().count() > 80
        || name.chars().any(|c| c <= '\u{1f}' || c == '\u{7f}')
    {
        return Err("That routine has no usable name.".into());
    }
    let Some(steps_arr) = obj["steps"].as_array() else {
        return Err("That routine's steps are not ones Locust can keep: 1 to 12 steps, none empty.".into());
    };
    if steps_arr.is_empty()
        || steps_arr.len() > 12
        || steps_arr.iter().any(|s| {
            s.as_str().is_none_or(|s| {
                s.trim().is_empty()
                    || s.chars()
                        .any(|c| (c <= '\u{1f}' || c == '\u{7f}') && c != '\n' && c != '\t' && c != '\r')
            })
        })
    {
        return Err("That routine's steps are not ones Locust can keep: 1 to 12 steps, none empty.".into());
    }
    let steps: Vec<String> = steps_arr
        .iter()
        .map(|s| s.as_str().unwrap().to_owned())
        .collect();
    for (i, step) in steps.iter().enumerate() {
        let n = step.encode_utf16().count();
        if n > 8000 {
            return Err(format!(
                "Step {} is {} characters; a step can be at most 8,000. Shorten it and save again.",
                i + 1,
                if n < 1000 { n.to_string() } else { format!("{},{:03}", n / 1000, n % 1000) }
            ));
        }
    }
    if let Some(saying) = inputs_refusal(&obj["inputs"], Some(&steps)) {
        return Err(format!(
            "That routine's inputs are not ones Locust can keep. {saying}"
        ));
    }
    let inputs: Vec<RoutineInput> = kept_inputs(
        &parse_inputs_array(&obj["inputs"])
            .ok_or_else(|| "That routine's inputs are not ones Locust can keep.".to_string())?,
    );

    let Some(ho_arr) = obj["handOffs"].as_array() else {
        return Err("That routine's hand-offs do not line up with its steps.".into());
    };
    if ho_arr.len() != steps.len() {
        return Err("That routine's hand-offs do not line up with its steps.".into());
    }
    let mut hand_offs = Vec::with_capacity(ho_arr.len());
    for entry in ho_arr {
        let Some(ho_obj) = entry.as_object() else {
            return Err("That routine's hand-offs do not line up with its steps.".into());
        };
        if !ho_obj
            .keys()
            .all(|k| k == "role" || k == "check" || k == "askFirst")
        {
            return Err("That routine's hand-offs do not line up with its steps.".into());
        }
        if let Some(r) = ho_obj.get("role")
            && !one_line(r, 60)
        {
            return Err("That routine's hand-offs do not line up with its steps.".into());
        }
        if let Some(c) = ho_obj.get("check")
            && *c != Value::Bool(true)
        {
            return Err("That routine's hand-offs do not line up with its steps.".into());
        }
        if let Some(a) = ho_obj.get("askFirst")
            && *a != Value::Bool(true)
        {
            return Err("That routine's hand-offs do not line up with its steps.".into());
        }
        hand_offs.push(RoutineFileHandOff {
            role: ho_obj.get("role").and_then(Value::as_str).map(ToOwned::to_owned),
            check: (ho_obj.get("check") == Some(&Value::Bool(true))).then_some(true),
            ask_first: (ho_obj.get("askFirst") == Some(&Value::Bool(true))).then_some(true),
        });
    }
    if hand_offs.first().is_some_and(|h| h.ask_first == Some(true)) {
        return Err("Step 1 cannot wait for your OK: it starts when you run the routine.".into());
    }

    let Some(route_obj) = obj["route"].as_object() else {
        return Err("That routine's route is not one Locust can read.".into());
    };
    if route_obj.keys().any(|k| k != "runtime") {
        return Err("That routine's route is not one Locust can read.".into());
    }
    let runtime = match route_obj.get("runtime") {
        None => None,
        Some(v) => {
            let Some(rt) = v.as_str() else {
                return Err("That routine's route is not one Locust can read.".into());
            };
            if !(1..=40).contains(&rt.len())
                || !rt
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err("That routine's route is not one Locust can read.".into());
            }
            Some(rt.to_owned())
        }
    };

    let Some(conn_arr) = obj["connectors"].as_array() else {
        return Err("That routine's list of connectors is not one Locust can read.".into());
    };
    if conn_arr.len() > MAX_CONNECTORS || !conn_arr.iter().all(|c| one_line(c, 80)) {
        return Err("That routine's list of connectors is not one Locust can read.".into());
    }
    let connectors = conn_arr
        .iter()
        .map(|c| c.as_str().unwrap().to_owned())
        .collect();

    Ok(RoutineFile {
        format: ROUTINE_FILE_FORMAT.into(),
        version: ROUTINE_FILE_VERSION,
        name: name.to_owned(),
        schedule: obj.get("schedule").map(|_| RoutineFileSchedule {
            kind: "hook".into(),
        }),
        steps,
        inputs,
        hand_offs,
        route: RoutineFileRoute { runtime },
        connectors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routine_inputs::RoutineInputKind;

    fn sample_file() -> RoutineFile {
        RoutineFile {
            format: ROUTINE_FILE_FORMAT.into(),
            version: ROUTINE_FILE_VERSION,
            name: "Weekly notes".into(),
            schedule: None,
            steps: vec![
                "Read what changed in {{folder}} since {{since}}.\n\n## What to look for\n\n- new files\n- removed tests".into(),
                "Write the notes in a {{tone}} tone.".into(),
                "Check the notes say only what changed.".into(),
            ],
            inputs: vec![
                RoutineInput {
                    key: "folder".into(),
                    label: "Folder".into(),
                    kind: RoutineInputKind::Folder,
                    required: true,
                    default: None,
                    choices: None,
                },
                RoutineInput {
                    key: "since".into(),
                    label: "Since when".into(),
                    kind: RoutineInputKind::Text,
                    required: false,
                    default: Some("5".into()),
                    choices: None,
                },
                RoutineInput {
                    key: "tone".into(),
                    label: "Tone".into(),
                    kind: RoutineInputKind::Choice,
                    required: true,
                    default: Some("true".into()),
                    choices: Some(vec!["Brief".into(), "true".into()]),
                },
                RoutineInput {
                    key: "note".into(),
                    label: "Anything else".into(),
                    kind: RoutineInputKind::LongText,
                    required: false,
                    default: Some("two\nlines".into()),
                    choices: None,
                },
            ],
            hand_offs: vec![
                RoutineFileHandOff::default(),
                RoutineFileHandOff {
                    role: Some("Writer".into()),
                    check: None,
                    ask_first: None,
                },
                RoutineFileHandOff {
                    role: Some("Reviewer".into()),
                    check: Some(true),
                    ask_first: None,
                },
            ],
            route: RoutineFileRoute {
                runtime: Some("claude".into()),
            },
            connectors: vec!["GitHub".into()],
        }
    }

    #[test]
    fn markdown_round_trips_and_falls_back_to_json_when_awkward() {
        let file = sample_file();
        let md = routine_markdown(&file);
        assert!(md.starts_with("---\nlocust-routine: 1\nruns-on: claude\n"));
        assert!(md.contains("\n# Weekly notes\n\n## Step 1\n\nRead what changed in {{folder}}"));
        assert!(md.contains("\n## Step 2 · Writer\n\nWrite the notes in a {{tone}} tone.\n"));
        assert!(md.contains("\n## Step 3 · Reviewer · checks the work\n"));
        assert_eq!(parse_routine_file(&md).unwrap(), file);
        assert_eq!(routine_export_text(&file), (md, true));
        assert_eq!(routine_file_name("Weekly notes", true), "Weekly notes.routine.md");

        let mut awkward = file.clone();
        awkward.steps[1] = "Then:\n## Step 3\nwritten in the step itself".into();
        assert_eq!(routine_export_text(&awkward), (routine_file_text(&awkward), false));

        let mut spaced = file.clone();
        spaced.steps[0] = "Ends on a blank line.\n\n".into();
        assert!(!routine_export_text(&spaced).1);

        let mut dotted = file.clone();
        dotted.hand_offs[1].role = Some("A · B".into());
        assert!(!routine_export_text(&dotted).1);
        assert_eq!(routine_file_name("Weekly notes", false), "Weekly notes.locust-routine.json");
    }

    #[test]
    fn markdown_hand_written_and_refusals() {
        let hand = [
            "---",
            "locust-routine: 1",
            "inputs:",
            "  - key: count",
            "    label: How many",
            "    kind: text",
            "    required: false",
            "    default: 3",
            "---",
            "",
            "# Tidy up",
            "",
            "## Step 1",
            "",
            "List the {{count}} oldest branches.",
            "",
            "## Step 2 · checks the work",
            "",
            "Check none of them is main.",
            "",
        ]
        .join("\r\n");
        let parsed = parse_routine_file(&hand).unwrap();
        assert_eq!(parsed.name, "Tidy up");
        assert_eq!(parsed.inputs[0].default.as_deref(), Some("3"));
        assert_eq!(parsed.hand_offs[1].check, Some(true));

        let said = |t: &str| parse_routine_file(t).unwrap_err();
        assert!(said("---\nlocust-routine: 2\n---\n# A\n\n## Step 1\n\nGo.\n").contains("version 2"));
        assert!(said("---\nlocust-routine: 1\nschedule: daily\n---\n# A\n\n## Step 1\n\nGo.\n").contains("(schedule)"));
        assert!(said("---\nlocust-routine: 1\n---\n# A\n\nA note.\n\n## Step 1\n\nGo.\n").contains("between its title and ## Step 1"));
        assert!(said("---\nlocust-routine: 1\n---\n# A\n\nGo.\n").contains("between its title"));
        assert!(said("---\nlocust-routine: 1\n---\n# A\n").contains("no steps"));
        assert!(said("---\nlocust-routine: 1\n---\n# A\n\n## Step 1 · Writer · Reviewer\n\nGo.\n").contains("more than one teammate"));
        assert!(said("---\nlocust-routine: 1\n---\n\n## Step 1\n\nGo.\n").contains("name as a title"));
        assert!(said("---\nlocust-routine: 1\n\n# A\n").contains("never closes"));
        assert!(said("---\nlocust-routine: 1\n---\n# A\n\n## Step 1\n\nRead {{missing}}.\n").contains("not declared"));
        assert!(said("---\nnot: [closed\n---\n# A\n").contains("not readable"));
        assert!(said("---\na: &x 1\nb: *x\n---\n# A\n").contains("not readable"));
    }

    #[test]
    fn absolute_paths_and_conversion() {
        let steps = vec![
            "Read C:\\work\\notes, then /home/test/docs.".into(),
            "Read C:\\work\\notes again.".into(),
        ];
        assert_eq!(
            absolute_paths_in(&steps),
            vec![
                FlaggedPath { step: 1, path: "C:\\work\\notes".into() },
                FlaggedPath { step: 1, path: "/home/test/docs".into() },
                FlaggedPath { step: 2, path: "C:\\work\\notes".into() },
            ]
        );
        let existing = vec![RoutineInput {
            key: "path".into(),
            label: "Existing".into(),
            kind: RoutineInputKind::Text,
            required: false,
            default: None,
            choices: None,
        }];
        let (converted, _) = paths_as_inputs(&steps, &existing).unwrap();
        assert_eq!(
            converted,
            vec![
                "Read {{path_2}}, then {{path_3}}.",
                "Read {{path_2}} again."
            ]
        );
        assert_eq!(
            absolute_paths_in(&["Read \"C:\\My Folder\\file.md\".".into()]),
            vec![FlaggedPath { step: 1, path: "C:\\My Folder\\file.md".into() }]
        );
        let (q_steps, q_inputs) = paths_as_inputs(&["Read \"C:\\My Folder\\file.md\".".into()], &[]).unwrap();
        assert_eq!(q_steps, vec!["Read \"{{path}}\"."]);
        assert_eq!(q_inputs[0].key, "path");
    }

    #[test]
    fn hook_schedule_carries_and_other_schedules_are_refused() {
        let json = r#"{"format":"locust-routine","version":1,"name":"Hook","schedule":{"kind":"hook"},"steps":["Run."],"inputs":[],"handOffs":[{}],"route":{},"connectors":[]}"#;
        let parsed = parse_routine_file(json).unwrap();
        assert_eq!(parsed.schedule.as_ref().map(|s| s.kind.as_str()), Some("hook"));
        let md = routine_markdown(&parsed);
        assert!(md.contains("when: hook\n"));
        assert_eq!(parse_routine_file(&md).unwrap(), parsed);

        let extra_sched = r#"{"format":"locust-routine","version":1,"name":"Hook","schedule":{"kind":"hook","extra":1},"steps":["Run."],"inputs":[],"handOffs":[{}],"route":{},"connectors":[]}"#;
        assert!(parse_routine_file(extra_sched).is_err());
        let daily_md = "---\nlocust-routine: 1\nwhen: daily\n---\n# A\n\n## Step 1\n\nGo.\n";
        assert!(parse_routine_file(daily_md).is_err());
    }
}
