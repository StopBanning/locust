//! Routines that ask for inputs (`shared/routine-inputs.ts`, W7).
//!
//! A routine declares what it needs to be told — a topic, a long note, one of
//! a few choices, a folder — and its steps say where each answer goes with
//! `{{key}}`. Running by hand asks for the values; a run that starts on its
//! own uses the defaults and refuses, in words, when a required input has none.

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashSet};

pub const MAX_INPUTS: usize = 12;
pub const MAX_INPUT_VALUE: usize = 4_000;
pub const MAX_INPUT_LABEL: usize = 80;
pub const MAX_CHOICES: usize = 12;
pub const MAX_CHOICE_LENGTH: usize = 80;

const INPUT_FIELDS: &[&str] = &["key", "label", "kind", "required", "default", "choices"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutineInputKind {
    Text,
    LongText,
    Choice,
    Folder,
}

impl RoutineInputKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::LongText => "long-text",
            Self::Choice => "choice",
            Self::Folder => "folder",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "text" => Some(Self::Text),
            "long-text" => Some(Self::LongText),
            "choice" => Some(Self::Choice),
            "folder" => Some(Self::Folder),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutineInput {
    pub key: String,
    pub label: String,
    pub kind: RoutineInputKind,
    pub required: bool,
    pub default: Option<String>,
    pub choices: Option<Vec<String>>,
}

impl RoutineInput {
    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        o.insert("key".into(), json!(self.key));
        o.insert("label".into(), json!(self.label));
        o.insert("kind".into(), json!(self.kind.as_str()));
        o.insert("required".into(), json!(self.required));
        if let Some(d) = &self.default {
            o.insert("default".into(), json!(d));
        }
        if let Some(c) = &self.choices {
            o.insert("choices".into(), json!(c));
        }
        Value::Object(o)
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let o = v.as_object()?;
        let key = o.get("key")?.as_str()?.to_owned();
        let label = o.get("label")?.as_str()?.to_owned();
        let kind = RoutineInputKind::parse(o.get("kind")?.as_str()?)?;
        let required = o.get("required")?.as_bool()?;
        let default = o
            .get("default")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let choices = o.get("choices").and_then(Value::as_array).map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        });
        Some(Self {
            key,
            label,
            kind,
            required,
            default,
            choices,
        })
    }
}

pub fn parse_inputs_array(v: &Value) -> Option<Vec<RoutineInput>> {
    let arr = v.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        out.push(RoutineInput::from_json(item)?);
    }
    Some(out)
}

fn has_control(s: &str) -> bool {
    s.chars()
        .any(|c| (c <= '\u{1f}' || c == '\u{7f}') && c != '\n' && c != '\t' && c != '\r')
}

fn has_line_break_or_control(s: &str) -> bool {
    s.chars().any(|c| c <= '\u{1f}' || c == '\u{7f}')
}

fn valid_input_key(key: &str) -> bool {
    let bytes = key.as_bytes();
    (1..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes[1..]
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
}

fn is_absolute_path(value: &str) -> bool {
    if value.starts_with('/') {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

fn comma(n: usize) -> String {
    if n < 1000 {
        n.to_string()
    } else {
        format!("{},{:03}", n / 1000, n % 1000)
    }
}

fn input_refusal(value: &Value, at: usize) -> Option<String> {
    let where_str = format!("Input {}", at + 1);
    let obj = value.as_object()?;
    if let Some(stray) = obj.keys().find(|f| !INPUT_FIELDS.contains(&f.as_str())) {
        return Some(format!(
            "{where_str} has a field Locust does not keep: {stray}."
        ));
    }
    let key = obj.get("key").and_then(Value::as_str).filter(|k| valid_input_key(k));
    let Some(key) = key else {
        return Some(format!(
            "{where_str} needs a key of lower-case letters, digits and underscores, starting with a letter, at most 32 long."
        ));
    };
    let label = obj.get("label").and_then(Value::as_str);
    if label.is_none_or(|l| {
        l.trim().is_empty() || l.trim().encode_utf16().count() > MAX_INPUT_LABEL || has_line_break_or_control(l)
    }) {
        return Some(format!(
            "Input \"{key}\" needs a label of one line, at most {MAX_INPUT_LABEL} characters."
        ));
    }
    let Some(kind) = obj
        .get("kind")
        .and_then(Value::as_str)
        .and_then(RoutineInputKind::parse)
    else {
        return Some(format!(
            "Input \"{key}\" is not a text, long-text, choice or folder."
        ));
    };
    if !obj.get("required").is_some_and(Value::is_boolean) {
        return Some(format!(
            "Input \"{key}\" must say whether it is required."
        ));
    }
    let choices = obj.get("choices");
    let mut parsed_choices: Vec<String> = Vec::new();
    if kind == RoutineInputKind::Choice {
        let Some(arr) = choices.and_then(Value::as_array) else {
            return Some(format!(
                "Input \"{key}\" is a choice, so it needs 2 to {MAX_CHOICES} answers to choose from."
            ));
        };
        if arr.len() < 2 || arr.len() > MAX_CHOICES {
            return Some(format!(
                "Input \"{key}\" is a choice, so it needs 2 to {MAX_CHOICES} answers to choose from."
            ));
        }
        let mut seen = HashSet::new();
        for c in arr {
            let Some(s) = c.as_str() else {
                return Some(format!(
                    "Each answer of \"{key}\" must be one line, at most {MAX_CHOICE_LENGTH} characters."
                ));
            };
            if s.trim().is_empty()
                || s.encode_utf16().count() > MAX_CHOICE_LENGTH
                || has_line_break_or_control(s)
            {
                return Some(format!(
                    "Each answer of \"{key}\" must be one line, at most {MAX_CHOICE_LENGTH} characters."
                ));
            }
            if !seen.insert(s) {
                return Some(format!("\"{key}\" lists the same answer twice."));
            }
            parsed_choices.push(s.to_owned());
        }
    } else if choices.is_some_and(|c| !c.is_null()) {
        return Some(format!(
            "Input \"{key}\" is not a choice, so it has no answers to choose from."
        ));
    }

    if let Some(given) = obj.get("default").filter(|v| !v.is_null()) {
        if kind == RoutineInputKind::Folder {
            return Some(format!(
                "Input \"{key}\" is a folder, and a folder only comes from the folder picker, so it has no default."
            ));
        }
        let Some(s) = given.as_str() else {
            return Some(format!(
                "The default of \"{key}\" must be text of at most {} characters.",
                comma(MAX_INPUT_VALUE)
            ));
        };
        if s.encode_utf16().count() > MAX_INPUT_VALUE || has_control(s) {
            return Some(format!(
                "The default of \"{key}\" must be text of at most {} characters.",
                comma(MAX_INPUT_VALUE)
            ));
        }
        if kind == RoutineInputKind::Text && (s.contains('\n') || s.contains('\r')) {
            return Some(format!(
                "The default of \"{key}\" is one line; use a long-text input for several."
            ));
        }
        if kind == RoutineInputKind::Choice && !parsed_choices.iter().any(|c| c == s) {
            return Some(format!(
                "The default of \"{key}\" is not one of its answers."
            ));
        }
    }
    None
}

/// The `{{key}}` markers one step holds, in order, repeats included.
pub fn placeholders_in(step: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = step.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            let start = i + 2;
            let mut j = start;
            while j + 1 < bytes.len() {
                if bytes[j] == b'{' || bytes[j] == b'}' {
                    break;
                }
                j += 1;
            }
            // Check if we stopped at `}}`
            if j + 1 < bytes.len() && bytes[j] == b'}' && bytes[j + 1] == b'}' {
                out.push(step[start..j].trim().to_owned());
                i = j + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Keys the steps write that no input declares, each once.
pub fn undeclared_placeholders(steps: &[String], inputs: Option<&[RoutineInput]>) -> Vec<String> {
    let declared: HashSet<&str> = inputs
        .unwrap_or(&[])
        .iter()
        .map(|i| i.key.as_str())
        .collect();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for step in steps {
        for key in placeholders_in(step) {
            if !declared.contains(key.as_str()) && seen.insert(key.clone()) {
                out.push(key);
            }
        }
    }
    out
}

/// Whether these are inputs the store will keep.
pub fn valid_inputs(value: &Value) -> bool {
    inputs_refusal(value, None).is_none()
}

/// Why these inputs cannot be kept — and, when steps are given, why they do not match them — or None.
pub fn inputs_refusal(value: &Value, steps: Option<&[String]>) -> Option<String> {
    let Some(arr) = value.as_array() else {
        return Some("Inputs must be a list.".into());
    };
    if arr.len() > MAX_INPUTS {
        return Some(format!("A routine asks for at most {MAX_INPUTS} inputs."));
    }
    let mut seen = HashSet::new();
    for (at, entry) in arr.iter().enumerate() {
        if !entry.is_object() {
            return Some(format!("Input {} is not an input.", at + 1));
        }
        if let Some(refusal) = input_refusal(entry, at) {
            return Some(refusal);
        }
        let key = entry["key"].as_str().unwrap_or("");
        if !seen.insert(key.to_owned()) {
            return Some(format!(
                "Two inputs are called \"{key}\"; each key can be declared once."
            ));
        }
    }
    let Some(steps) = steps else {
        return None;
    };
    let parsed: Vec<RoutineInput> = arr
        .iter()
        .filter_map(RoutineInput::from_json)
        .collect();
    let missing = undeclared_placeholders(steps, Some(&parsed));
    let Some(first_key) = missing.first() else {
        return None;
    };
    let first_step = steps
        .iter()
        .position(|s| placeholders_in(s).iter().any(|k| k == first_key))
        .unwrap_or(0);
    Some(format!(
        "Step {} uses {{{{{first_key}}}}}, which is not declared as an input. Add an input with the key {first_key}, or take the braces out.",
        first_step + 1
    ))
}

/// A copy a store can keep: fields named one by one, trimmed label, unset fields omitted.
pub fn kept_inputs(inputs: &[RoutineInput]) -> Vec<RoutineInput> {
    inputs
        .iter()
        .map(|i| RoutineInput {
            key: i.key.clone(),
            label: i.label.trim().to_owned(),
            kind: i.kind,
            required: i.required,
            default: i.default.clone(),
            choices: i.choices.clone(),
        })
        .collect()
}

pub fn kept_inputs_value(inputs: &[RoutineInput]) -> Value {
    Value::Array(kept_inputs(inputs).iter().map(RoutineInput::to_json).collect())
}

fn value_refusal(input: &RoutineInput, value: &str) -> Option<String> {
    let len = value.encode_utf16().count();
    if len > MAX_INPUT_VALUE {
        return Some(format!(
            "\"{}\" is {} characters; an input can be at most {}.",
            input.label,
            comma(len),
            comma(MAX_INPUT_VALUE)
        ));
    }
    if has_control(value) {
        return Some(format!(
            "\"{}\" holds a character that cannot be sent.",
            input.label
        ));
    }
    if input.kind == RoutineInputKind::Text && (value.contains('\n') || value.contains('\r')) {
        return Some(format!(
            "\"{}\" is one line; use more than one line only where the routine asks for long text.",
            input.label
        ));
    }
    if input.kind == RoutineInputKind::Choice
        && !input
            .choices
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .any(|c| c == value)
    {
        let preview: String = value.chars().take(60).collect();
        return Some(format!(
            "\"{preview}\" is not one of the answers to \"{}\".",
            input.label
        ));
    }
    if input.kind == RoutineInputKind::Folder && !is_absolute_path(value) {
        return Some(format!(
            "\"{}\" must be a folder chosen with the folder picker.",
            input.label
        ));
    }
    None
}

/// Settle every input's value for one run.
pub fn resolve_values(
    inputs: &[RoutineInput],
    given: Option<&Value>,
) -> Result<BTreeMap<String, String>, String> {
    let by_hand = given.is_some();
    let entered: &serde_json::Map<String, Value> = match given {
        None => {
            static EMPTY: std::sync::OnceLock<serde_json::Map<String, Value>> =
                std::sync::OnceLock::new();
            EMPTY.get_or_init(serde_json::Map::new)
        }
        Some(v) => v
            .as_object()
            .ok_or_else(|| "The values for this routine could not be read.".to_string())?,
    };
    if let Some(stray) = entered
        .keys()
        .find(|k| !inputs.iter().any(|i| &i.key == *k))
    {
        return Err(format!("This routine does not ask for \"{stray}\"."));
    }
    let mut values = BTreeMap::new();
    for input in inputs {
        let said = match entered.get(&input.key) {
            None => None,
            Some(v) => {
                let Some(s) = v.as_str() else {
                    return Err(format!("The value of \"{}\" must be text.", input.label));
                };
                Some(s)
            }
        };
        let value = match said {
            Some(s) if !s.trim().is_empty() => Some(s),
            _ => input.default.as_deref(),
        };
        let Some(value) = value.filter(|v| !v.trim().is_empty()) else {
            if !input.required {
                values.insert(input.key.clone(), String::new());
                continue;
            }
            return Err(if by_hand {
                format!(
                    "\"{}\" needs a value before this routine can run.",
                    input.label
                )
            } else {
                format!(
                    "This routine asks for \"{}\", which has no default, so it cannot start on its own. Open it and set a default, or run it by hand.",
                    input.label
                )
            });
        };
        if let Some(refusal) = value_refusal(input, value) {
            return Err(refusal);
        }
        values.insert(input.key.clone(), value.to_owned());
    }
    Ok(values)
}

/// Fill the steps in. One pass, as text: a value that itself contains `{{another}}` is not substituted again.
pub fn substitute_steps(steps: &[String], values: &BTreeMap<String, String>) -> Vec<String> {
    let re = regex::Regex::new(r"\{\{\s*([a-z][a-z0-9_]{0,31})\s*\}\}").expect("valid regex");
    steps
        .iter()
        .map(|step| {
            re.replace_all(step, |caps: &regex::Captures<'_>| {
                let key = &caps[1];
                match values.get(key) {
                    Some(v) => v.clone(),
                    None => caps[0].to_owned(),
                }
            })
            .into_owned()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_inputs() -> Vec<RoutineInput> {
        vec![
            RoutineInput {
                key: "topic".into(),
                label: "Topic".into(),
                kind: RoutineInputKind::Text,
                required: true,
                default: Some("default topic".into()),
                choices: None,
            },
            RoutineInput {
                key: "tone".into(),
                label: "Tone".into(),
                kind: RoutineInputKind::Choice,
                required: true,
                default: Some("Brief".into()),
                choices: Some(vec!["Brief".into(), "Detailed".into()]),
            },
        ]
    }

    #[test]
    fn keeps_all_four_kinds_and_refuses_bad_keys_duplicates_extra_fields_defaults_and_choices() {
        let inputs = sample_inputs();
        let all = json!([
            inputs[0].to_json(),
            inputs[1].to_json(),
            {"key": "note", "label": "Note", "kind": "long-text", "required": false, "default": "two\nlines"},
            {"key": "folder", "label": "Folder", "kind": "folder", "required": true}
        ]);
        assert!(valid_inputs(&all));

        let i0 = inputs[0].to_json();
        let i1 = inputs[1].to_json();
        let mut overlong_default = i0.clone();
        overlong_default["default"] = json!("a".repeat(4001));
        let mut bad_key = i0.clone();
        bad_key["key"] = json!("Topic");
        let mut too_long_key = i0.clone();
        too_long_key["key"] = json!("a".repeat(33));
        let mut extra = i0.clone();
        extra["extra"] = json!("secret");
        let mut nl_label = i0.clone();
        nl_label["label"] = json!("two\nlines");
        let mut bad_default = i1.clone();
        bad_default["default"] = json!("Other");
        let mut dup_choices = i1.clone();
        dup_choices["choices"] = json!(["same", "same"]);
        let mut one_choice = i1.clone();
        one_choice["choices"] = json!(["Only"]);
        let mut folder_default = i0.clone();
        folder_default["kind"] = json!("folder");
        folder_default["default"] = json!("C:\\local");
        let thirteen: Vec<Value> = (0..13)
            .map(|at| {
                let mut e = i0.clone();
                e["key"] = json!(format!("key_{at}"));
                e
            })
            .collect();

        for bad in [
            json!([bad_key]),
            json!([too_long_key]),
            json!([i0, i0]),
            json!([extra]),
            json!([overlong_default]),
            json!([nl_label]),
            json!([bad_default]),
            json!([dup_choices]),
            json!([one_choice]),
            json!([folder_default]),
            Value::Array(thirteen),
        ] {
            assert!(inputs_refusal(&bad, None).is_some(), "expected refusal for {bad}");
        }

        let arr = kept_inputs_value(&inputs);
        assert!(inputs_refusal(&arr, Some(&["Read {{missing}}".into()])).unwrap().contains("not declared"));
        assert!(inputs_refusal(&arr, Some(&["Read {{Topic}}".into()])).unwrap().contains("not declared"));
        assert!(inputs_refusal(&arr, Some(&["Read {{}}".into()])).unwrap().contains("not declared"));
    }

    #[test]
    fn uses_defaults_refuses_missing_required_and_substitutes_once() {
        let inputs = sample_inputs();
        let resolved = resolve_values(&inputs, None).unwrap();
        assert_eq!(resolved["topic"], "default topic");
        assert_eq!(resolved["tone"], "Brief");

        let mut no_default = inputs[0].clone();
        no_default.default = None;
        assert!(resolve_values(&[no_default.clone()], None).unwrap_err().contains("cannot start on its own"));
        assert!(resolve_values(&[no_default], Some(&json!({}))).unwrap_err().contains("needs a value"));

        for bad in [
            json!(null),
            json!([]),
            json!({"stranger": "x"}),
            json!({"topic": 42}),
            json!({"topic": "a".repeat(4001)}),
            json!({"topic": "two\nlines"}),
            json!({"tone": "Other"}),
        ] {
            assert!(resolve_values(&inputs, Some(&bad)).is_err(), "expected err for {bad}");
        }

        let mut map = BTreeMap::new();
        map.insert("topic".into(), "$& $(no shell) {{tone}}".into());
        map.insert("tone".into(), "Brief".into());
        assert_eq!(
            substitute_steps(&["{{topic}} / {{tone}} / {{topic}}".into()], &map),
            vec!["$& $(no shell) {{tone}} / Brief / $& $(no shell) {{tone}}"]
        );
    }
}
