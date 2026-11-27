//! An ACP agent's question, as a form: acp-elicitation.ts (27 Nov 2026).
//!
//! An agent asks with `elicitation/create` once the client offers `clientCapabilities.elicitation.form`. MEASURED on
//! agy 1.5.18 (`agy acp`, Gemini 3.8 Flash; ~/work/scratch/agy-acp-probe/): its `ask_question` arrives as a form whose
//! fields are `q1` (a string with `oneOf` const/title choices), `q1_other` (words beside them), and, for a multi-select,
//! `q2` of type array with `items.anyOf`. `{action:"accept",content:{q1:"2"}}` carries the same turn on;
//! `{action:"decline"}` completes the tool unanswered; `{action:"cancel"}` is a stopped run's answer. A form that cannot
//! be read whole is never guessed at.
use serde_json::{Map, Value, json};

const MAX_QUESTIONS: usize = 12;
const MAX_OPTIONS: usize = 24;

/// One choice: what the person reads, and what goes back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormOption {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormQuestion {
    /// The field the answer is filed under (`q1`).
    pub id: String,
    pub question: String,
    /// Empty: a question answered in words.
    pub options: Vec<FormOption>,
    pub multiple: bool,
    /// The field that takes a written answer beside the choices (`q1_other`).
    pub other_field: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Form {
    pub tool_call_id: Option<String>,
    pub message: Option<String>,
    pub questions: Vec<FormQuestion>,
}

fn text(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.trim().is_empty()).map(str::to_owned)
}

fn options_of(entries: &Value) -> Option<Vec<FormOption>> {
    let list = entries.as_array()?;
    let mut options = vec![];
    for entry in list.iter().take(MAX_OPTIONS) {
        let value = entry["const"].as_str()?.to_owned();
        let label = text(&entry["title"]).unwrap_or_else(|| value.clone());
        options.push(FormOption { label, value });
    }
    (!options.is_empty()).then_some(options)
}

fn enum_options(values: &Value) -> Option<Vec<FormOption>> {
    let list = values.as_array().filter(|l| !l.is_empty())?;
    list.iter()
        .take(MAX_OPTIONS)
        .map(|v| v.as_str().map(|s| FormOption { label: s.into(), value: s.into() }))
        .collect()
}

/// The form, read whole, or None when any part of it cannot be: an answer to half a form would say the person chose
/// nothing for the rest.
pub fn form_of(params: &Value) -> Option<Form> {
    if !params["mode"].is_null() && params["mode"] != "form" {
        return None;
    }
    let properties = params["requestedSchema"]["properties"].as_object()?;
    if properties.is_empty() {
        return None;
    }
    let other_of = |name: &str| {
        let field = format!("{name}_other");
        properties.contains_key(&field).then_some(field)
    };
    let is_other = |name: &str| name.strip_suffix("_other").is_some_and(|base| properties.contains_key(base));
    let mut questions = vec![];
    for (name, field) in properties {
        if is_other(name) {
            continue;
        }
        let question = text(&field["title"]).or_else(|| text(&field["description"]))?;
        let (options, multiple) = match field["type"].as_str() {
            Some("array") => {
                let items = &field["items"];
                (Some(options_of(&items["anyOf"]).or_else(|| options_of(&items["oneOf"])).or_else(|| enum_options(&items["enum"]))?), true)
            }
            Some("string") | None => {
                let found = options_of(&field["oneOf"]).or_else(|| options_of(&field["anyOf"])).or_else(|| enum_options(&field["enum"]));
                // A string with no choices is a question answered in words; one whose choices cannot be read is not.
                if found.is_none() && (!field["oneOf"].is_null() || !field["anyOf"].is_null() || !field["enum"].is_null()) {
                    return None;
                }
                (found, false)
            }
            // A number, a yes/no, an object: never guessed at.
            _ => return None,
        };
        questions.push(FormQuestion { id: name.clone(), question, options: options.unwrap_or_default(), multiple, other_field: other_of(name) });
        if questions.len() > MAX_QUESTIONS {
            return None;
        }
    }
    (!questions.is_empty()).then(|| Form { tool_call_id: text(&params["toolCallId"]), message: text(&params["message"]), questions })
}

/// The form as the request Codex's questions already make (`item/tool/requestUserInput`), so the same question card,
/// the same pairing check and the same answer come back (approval-channel.ts `acpQuestionRequest`).
pub fn question_request(form: &Form, cwd: &str) -> Value {
    let mut params = json!({
        "cwd": cwd,
        "questions": form.questions.iter().map(|q| json!({
            "id": q.id,
            "header": null,
            "question": q.question,
            "options": q.options.iter().map(|o| json!({"label": o.label, "description": null})).collect::<Vec<_>>(),
            // Words are taken where the agent has a field for them, or the question is only words.
            "isOther": q.other_field.is_some() || q.options.is_empty(),
            "isSecret": false,
        })).collect::<Vec<_>>(),
    });
    if let Some(id) = &form.tool_call_id {
        params["itemId"] = json!(id);
    }
    params
}

/// The card's answer (`{answers:{q1:{answers:[...]}}}`) as the agent's form takes it. Anything but answers -- the run
/// ended and the channel refused what was waiting, or a decision reached a question -- is `cancel`: no answer is made
/// up. A chosen label goes back as its value; other words go to the written-answer field; a form left wholly alone is
/// declined.
pub fn form_result(form: &Form, card: &Value) -> Value {
    let Some(given) = card["answers"].as_object() else {
        return json!({"action": "cancel"});
    };
    let mut content = Map::new();
    for q in &form.questions {
        let said: Vec<String> = given
            .get(&q.id)
            .and_then(|a| a["answers"].as_array())
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned))
            .collect();
        if said.is_empty() {
            continue;
        }
        let chosen: Vec<String> =
            said.iter().filter_map(|s| q.options.iter().find(|o| &o.label == s).map(|o| o.value.clone())).collect();
        let written = said.iter().filter(|s| !q.options.iter().any(|o| &o.label == *s)).cloned().collect::<Vec<_>>().join("\n");
        if q.options.is_empty() {
            content.insert(q.id.clone(), json!(written));
            continue;
        }
        if !chosen.is_empty() {
            content.insert(q.id.clone(), if q.multiple { json!(chosen) } else { json!(chosen[0]) });
        }
        if !written.is_empty()
            && let Some(other) = &q.other_field
        {
            content.insert(other.clone(), json!(written));
        }
    }
    if content.is_empty() { json!({"action": "decline"}) } else { json!({"action": "accept", "content": content}) }
}
