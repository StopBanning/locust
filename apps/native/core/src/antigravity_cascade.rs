//! Antigravity's own Connect server, spoken to directly over loopback plain HTTP
//! (`apps/desktop/src/main/antigravity-cascade.ts`).
//!
//! Used to read a waiting `ask_question` step (`GetCascadeTrajectorySteps`),
//! send the person's answer (`HandleCascadeUserInteraction`), and read whole
//! `PLANNER_RESPONSE` text when the JSONL transcript cut it short.

use serde_json::{Map, Value, json};
use std::{
    collections::HashMap,
    io::Read,
    sync::Mutex,
    time::Duration,
};

const SERVICE: &str = "/exa.language_server_pb.LanguageServerService/";
const RESPONSE_LIMIT: usize = 8 * 1024 * 1024;
const CALL_TIMEOUT_MS: u64 = 5_000;

/// Step statuses that mean the step is over and no longer waiting.
const FINISHED: &[&str] = &[
    "CORTEX_STEP_STATUS_DONE",
    "CORTEX_STEP_STATUS_ERROR",
    "CORTEX_STEP_STATUS_CANCELED",
    "CORTEX_STEP_STATUS_CLEARED",
    "CORTEX_STEP_STATUS_HALTED",
    "CORTEX_STEP_STATUS_INTERRUPTED",
    "CORTEX_STEP_STATUS_INVALID",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AntigravityQuestionOption {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AntigravityAskedQuestion {
    pub question: String,
    pub options: Vec<AntigravityQuestionOption>,
    pub multi_select: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AntigravityPendingQuestion {
    pub trajectory_id: String,
    pub step_index: usize,
    pub action: Option<String>,
    pub questions: Vec<AntigravityAskedQuestion>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AntigravityQuestionResponse {
    pub selected_option_ids: Vec<String>,
    pub write_in: Option<String>,
    pub skipped: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeHttpResult {
    pub status: u16,
    pub text: String,
}

pub trait CascadePost: Send + Sync {
    fn post(
        &self,
        port: u16,
        method: &str,
        body: &Value,
        token: &str,
    ) -> Result<CascadeHttpResult, String>;
}

/// Plain-HTTP Connect transport over loopback (`127.0.0.1`).
/// `ureq` has no TLS; the TLS port refuses with 400 and is skipped.
#[derive(Default)]
pub struct UreqCascadePost;

impl CascadePost for UreqCascadePost {
    fn post(
        &self,
        port: u16,
        method: &str,
        body: &Value,
        token: &str,
    ) -> Result<CascadeHttpResult, String> {
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout(Duration::from_millis(CALL_TIMEOUT_MS))
            .build();
        let url = format!("http://127.0.0.1:{port}{SERVICE}{method}");
        let response = match agent
            .post(&url)
            .set("content-type", "application/json")
            .set("connect-protocol-version", "1")
            .set("x-codeium-csrf-token", token)
            .send_json(body)
        {
            Ok(resp) => resp,
            Err(ureq::Error::Status(_, resp)) => resp,
            Err(ureq::Error::Transport(err)) => {
                // Never include the token in transport errors.
                let message = err.to_string();
                let scrubbed = if token.is_empty() {
                    message
                } else {
                    message.replace(token, "[REDACTED]")
                };
                return Err(scrubbed);
            }
        };
        let status = response.status();
        let mut reader = response.into_reader().take((RESPONSE_LIMIT + 1) as u64);
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Failed to read Antigravity response: {e}"))?;
        if bytes.len() > RESPONSE_LIMIT {
            return Err("Antigravity answered with more than Locust reads.".into());
        }
        let text = String::from_utf8_lossy(&bytes).into_owned();
        Ok(CascadeHttpResult { status, text })
    }
}

fn non_empty_str(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// The waiting question in a `GetCascadeTrajectorySteps` response, newest first.
/// A step that has finished or already carries a completed interaction has been answered.
pub fn pending_question_in(response: &Value, from_step: usize) -> Option<AntigravityPendingQuestion> {
    let steps = response.get("steps")?.as_array()?;
    for (index, step) in steps.iter().enumerate().rev() {
        let Some(obj) = step.as_object() else {
            continue;
        };
        if obj.get("type").and_then(Value::as_str) != Some("CORTEX_STEP_TYPE_ASK_QUESTION") {
            continue;
        }
        if let Some(status) = obj.get("status").and_then(Value::as_str)
            && FINISHED.contains(&status)
        {
            continue;
        }
        if obj
            .get("completedInteractions")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
        {
            continue;
        }
        let metadata = obj.get("metadata").and_then(Value::as_object);
        let source = metadata
            .and_then(|m| m.get("sourceTrajectoryStepInfo"))
            .and_then(Value::as_object);
        let Some(trajectory_id) = non_empty_str(source.and_then(|s| s.get("trajectoryId"))) else {
            continue;
        };
        // proto3 JSON leaves a zero out, so a missing index is the offset's own.
        let step_index = source
            .and_then(|s| s.get("stepIndex"))
            .and_then(Value::as_u64)
            .map(|n| n as usize)
            .unwrap_or(from_step + index);
        let asked = obj
            .get("askQuestion")
            .and_then(|a| a.get("questions"))
            .and_then(Value::as_array);
        let mut questions = Vec::new();
        if let Some(entries) = asked {
            for entry in entries {
                let Some(entry_obj) = entry.as_object() else {
                    continue;
                };
                let Some(question) = non_empty_str(entry_obj.get("question")) else {
                    continue;
                };
                let mut options = Vec::new();
                if let Some(raw_options) = entry_obj.get("options").and_then(Value::as_array) {
                    for opt in raw_options {
                        let Some(opt_obj) = opt.as_object() else {
                            continue;
                        };
                        if let (Some(id), Some(text)) = (
                            non_empty_str(opt_obj.get("id")),
                            non_empty_str(opt_obj.get("text")),
                        ) {
                            options.push(AntigravityQuestionOption { id, text });
                        }
                    }
                }
                let multi_select =
                    entry_obj.get("isMultiSelect").and_then(Value::as_bool) == Some(true);
                questions.push(AntigravityAskedQuestion {
                    question,
                    options,
                    multi_select,
                });
            }
        }
        if questions.is_empty() {
            continue;
        }
        let action = non_empty_str(metadata.and_then(|m| m.get("toolAction")));
        return Some(AntigravityPendingQuestion {
            trajectory_id,
            step_index,
            action,
            questions,
        });
    }
    None
}

/// Every `PLANNER_RESPONSE` step's text in a `GetCascadeTrajectorySteps` response,
/// both `modifiedResponse` and `response`.
pub fn planner_texts_in(response: &Value) -> Vec<String> {
    let Some(steps) = response.get("steps").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut texts = Vec::new();
    for step in steps {
        if step.get("type").and_then(Value::as_str) != Some("CORTEX_STEP_TYPE_PLANNER_RESPONSE") {
            continue;
        }
        let Some(planner) = step.get("plannerResponse").and_then(Value::as_object) else {
            continue;
        };
        if let Some(modified) = non_empty_str(planner.get("modifiedResponse")) {
            texts.push(modified);
        }
        if let Some(raw) = non_empty_str(planner.get("response")) {
            texts.push(raw);
        }
    }
    texts
}

/// The `HandleCascadeUserInteraction` request body that answers `pending`.
pub fn question_interaction(
    conversation_id: &str,
    pending: &AntigravityPendingQuestion,
    responses: &[AntigravityQuestionResponse],
) -> Value {
    let rendered_responses: Vec<Value> = pending
        .questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let fallback = AntigravityQuestionResponse {
                selected_option_ids: Vec::new(),
                write_in: None,
                skipped: true,
            };
            let response = responses.get(index).unwrap_or(&fallback);
            let write_in = response.write_in.as_deref().unwrap_or("").trim();
            let options: Vec<Value> = question
                .options
                .iter()
                .map(|o| json!({ "id": o.id, "text": o.text }))
                .collect();
            let mut map = Map::new();
            map.insert("question".into(), json!(question.question));
            map.insert("options".into(), Value::Array(options));
            if question.multi_select {
                map.insert("isMultiSelect".into(), json!(true));
            }
            map.insert(
                "selectedOptionIds".into(),
                json!(response.selected_option_ids),
            );
            if !write_in.is_empty() {
                map.insert("writeInResponse".into(), json!(write_in));
            }
            if response.skipped {
                map.insert("skipped".into(), json!(true));
            }
            Value::Object(map)
        })
        .collect();

    json!({
        "cascadeId": conversation_id,
        "interaction": {
            "trajectoryId": pending.trajectory_id,
            "stepIndex": pending.step_index,
            "askQuestion": {
                "responses": rendered_responses,
            }
        }
    })
}

/// Strip markdown links `[label](url)` to their label text, matching `plainQuestion`.
pub fn plain_question(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after_open = &rest[open + 1..];
        if let Some(close) = after_open.find("](") {
            let label = &after_open[..close];
            let after_paren = &after_open[close + 2..];
            if let Some(end_paren) = after_paren.find(')')
                && !after_paren[..end_paren].contains(char::is_whitespace)
                && end_paren > 0
            {
                out.push_str(label);
                rest = &after_paren[end_paren + 1..];
                continue;
            }
        }
        out.push('[');
        rest = after_open;
    }
    out.push_str(rest);
    out
}

/// Map Locust card answers (keyed by question index string `"0"`, `"1"`, ...) to
/// `AntigravityQuestionResponse`s by matching option labels to option ids, write-in
/// text, or skip (empty selection).
pub fn responses_from_answers(
    pending: &AntigravityPendingQuestion,
    answers: &HashMap<String, Vec<String>>,
) -> Vec<AntigravityQuestionResponse> {
    pending
        .questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let given = answers.get(&index.to_string()).map(Vec::as_slice).unwrap_or(&[]);
            if given.is_empty() {
                return AntigravityQuestionResponse {
                    selected_option_ids: Vec::new(),
                    write_in: None,
                    skipped: true,
                };
            }
            let selected_option_ids: Vec<String> = question
                .options
                .iter()
                .filter(|opt| given.iter().any(|g| g == &opt.text))
                .map(|opt| opt.id.clone())
                .collect();
            let written: Vec<&str> = given
                .iter()
                .filter(|entry| !question.options.iter().any(|opt| &opt.text == *entry))
                .map(String::as_str)
                .collect();
            let write_in = written.join("\n").trim().to_owned();
            AntigravityQuestionResponse {
                selected_option_ids,
                write_in: (!write_in.is_empty()).then_some(write_in),
                skipped: false,
            }
        })
        .collect()
}

fn wrong_port(result: &CascadeHttpResult) -> bool {
    result.status == 400
        && result
            .text
            .to_ascii_lowercase()
            .contains("http request to an https server")
}

fn failure(result: &CascadeHttpResult, token: &str) -> String {
    let raw = if let Ok(parsed) = serde_json::from_str::<Value>(&result.text) {
        if let Some(msg) = non_empty_str(parsed.get("message")) {
            msg
        } else if let Some(code) = parsed.get("code").and_then(Value::as_str) {
            code.to_owned()
        } else {
            format!("HTTP {}", result.status)
        }
    } else {
        format!("HTTP {}", result.status)
    };
    if token.is_empty() {
        raw
    } else {
        raw.replace(token, "[REDACTED]")
    }
}

pub struct CascadeClient<P: CascadePost = UreqCascadePost> {
    ports: Vec<u16>,
    csrf_token: String,
    post: P,
    answering: Mutex<Option<u16>>,
}

impl<P: CascadePost> std::fmt::Debug for CascadeClient<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CascadeClient")
            .field("ports", &self.ports)
            .field("csrf_token", &"[REDACTED]")
            .finish()
    }
}

impl CascadeClient<UreqCascadePost> {
    pub fn new(ports: &[u16], address: Option<&str>, csrf_token: impl Into<String>) -> Self {
        Self::with_post(ports, address, csrf_token, UreqCascadePost)
    }
}

impl<P: CascadePost> CascadeClient<P> {
    pub fn with_post(
        ports: &[u16],
        address: Option<&str>,
        csrf_token: impl Into<String>,
        post: P,
    ) -> Self {
        let mut deduped = Vec::new();
        for &port in ports {
            if port > 0 && !deduped.contains(&port) {
                deduped.push(port);
            }
        }
        if let Some(addr_port) = address
            .and_then(|a| a.rsplit(':').next())
            .and_then(|p| p.parse::<u16>().ok())
            && addr_port > 0
            && !deduped.contains(&addr_port)
        {
            deduped.push(addr_port);
        }
        Self {
            ports: deduped,
            csrf_token: csrf_token.into(),
            post,
            answering: Mutex::new(None),
        }
    }

    fn call(&self, method: &str, body: &Value) -> Result<Value, String> {
        let preferred = *crate::invariants::lock(&self.answering);
        let mut order = Vec::with_capacity(self.ports.len());
        if let Some(p) = preferred
            && self.ports.contains(&p)
        {
            order.push(p);
        }
        for &p in &self.ports {
            if Some(p) != preferred {
                order.push(p);
            }
        }
        let mut last = String::from("Antigravity is not listening.");
        for port in order {
            let result = match self.post.post(port, method, body, &self.csrf_token) {
                Ok(r) => r,
                Err(err) => {
                    last = if self.csrf_token.is_empty() {
                        err
                    } else {
                        err.replace(&self.csrf_token, "[REDACTED]")
                    };
                    continue;
                }
            };
            if wrong_port(&result) {
                continue;
            }
            *crate::invariants::lock(&self.answering) = Some(port);
            if result.status != 200 {
                return Err(failure(&result, &self.csrf_token));
            }
            if result.text.trim().is_empty() {
                return Ok(json!({}));
            }
            return serde_json::from_str(&result.text)
                .map_err(|e| format!("Invalid JSON from Antigravity: {e}"));
        }
        Err(last)
    }

    pub fn pending_question(
        &self,
        conversation_id: &str,
        from_step: usize,
    ) -> Result<Option<AntigravityPendingQuestion>, String> {
        let response = self.call(
            "GetCascadeTrajectorySteps",
            &json!({ "cascadeId": conversation_id, "stepOffset": from_step }),
        )?;
        Ok(pending_question_in(&response, from_step))
    }

    pub fn answer_question(
        &self,
        conversation_id: &str,
        pending: &AntigravityPendingQuestion,
        responses: &[AntigravityQuestionResponse],
    ) -> Result<(), String> {
        let body = question_interaction(conversation_id, pending, responses);
        self.call("HandleCascadeUserInteraction", &body)?;
        Ok(())
    }

    pub fn planner_texts(
        &self,
        conversation_id: &str,
        from_step: usize,
    ) -> Result<Vec<String>, String> {
        let response = self.call(
            "GetCascadeTrajectorySteps",
            &json!({ "cascadeId": conversation_id, "stepOffset": from_step }),
        )?;
        Ok(planner_texts_in(&response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    const CONVERSATION: &str = "ec3897f1-dc20-44a5-bf84-e9de9f183ea0";
    const TRAJECTORY: &str = "90cfc6a4-467b-438e-9beb-3752d4bc4b89";
    const QUESTION: &str = "How would you like to organize this folder (currently containing [README.md](file:///c:/Users/<home>/Documents/antigravtest/README.md) and [hello.txt](file:///c:/Users/<home>/Documents/antigravtest/hello.txt))?";

    fn options() -> Vec<AntigravityQuestionOption> {
        vec![
            AntigravityQuestionOption {
                id: "1".into(),
                text: "Standard Project Layout — Move documentation to `docs/` and data/source files to `src/`".into(),
            },
            AntigravityQuestionOption {
                id: "2".into(),
                text: "By File Type — Group files by extension into dedicated directories such as `markdown/` and `text/`".into(),
            },
            AntigravityQuestionOption {
                id: "3".into(),
                text: "Root with Scratch / Examples — Keep README.md at the root and move test files into a `scratch/` or `examples/` folder".into(),
            },
            AntigravityQuestionOption {
                id: "4".into(),
                text: "Flat Minimalist Layout — Retain all files at the root directory without subdirectories, using structured prefixes".into(),
            },
        ]
    }

    fn step_info(step_index: usize) -> Value {
        json!({
            "trajectoryId": TRAJECTORY,
            "stepIndex": step_index,
            "metadataIndex": 3,
            "cascadeId": CONVERSATION,
        })
    }

    fn options_json() -> Value {
        Value::Array(
            options()
                .into_iter()
                .map(|o| json!({ "id": o.id, "text": o.text }))
                .collect(),
        )
    }

    fn waiting() -> Value {
        json!({
            "steps": [
                {
                    "type": "CORTEX_STEP_TYPE_PLANNER_RESPONSE",
                    "status": "CORTEX_STEP_STATUS_DONE",
                    "metadata": { "sourceTrajectoryStepInfo": step_info(7) },
                },
                {
                    "type": "CORTEX_STEP_TYPE_ASK_QUESTION",
                    "status": "CORTEX_STEP_STATUS_RUNNING",
                    "metadata": {
                        "toolAction": "Asking folder organization preference",
                        "sourceTrajectoryStepInfo": step_info(8),
                    },
                    "askQuestion": {
                        "questions": [{ "question": QUESTION, "options": options_json() }],
                    },
                },
            ]
        })
    }

    fn ide_answer() -> Value {
        json!({
            "trajectoryId": TRAJECTORY,
            "stepIndex": 8,
            "askQuestion": {
                "responses": [{
                    "question": QUESTION,
                    "options": options_json(),
                    "selectedOptionIds": ["1"],
                }]
            }
        })
    }

    fn answered() -> Value {
        let mut w = waiting();
        w["steps"][1]["status"] = json!("CORTEX_STEP_STATUS_DONE");
        w["steps"][1]["completedInteractions"] = json!([{
            "request": { "askQuestion": { "questions": [{ "question": QUESTION, "options": options_json() }] } },
            "response": ide_answer(),
        }]);
        w
    }

    #[test]
    fn pending_question_is_the_step_holding_for_an_answer_with_the_ids_an_answer_has_to_name() {
        assert_eq!(
            pending_question_in(&waiting(), 7),
            Some(AntigravityPendingQuestion {
                trajectory_id: TRAJECTORY.into(),
                step_index: 8,
                action: Some("Asking folder organization preference".into()),
                questions: vec![AntigravityAskedQuestion {
                    question: QUESTION.into(),
                    options: options(),
                    multi_select: false,
                }],
            })
        );
    }

    #[test]
    fn pending_question_is_nothing_once_answered() {
        assert_eq!(pending_question_in(&answered(), 7), None);
        let mut done_no_interactions = waiting();
        done_no_interactions["steps"][1]["status"] = json!("CORTEX_STEP_STATUS_DONE");
        assert_eq!(pending_question_in(&done_no_interactions, 7), None);
        let mut running_with_interactions = waiting();
        running_with_interactions["steps"][1]["completedInteractions"] = json!([{
            "response": ide_answer(),
        }]);
        assert_eq!(pending_question_in(&running_with_interactions, 7), None);
        let mut wrong_type = waiting();
        wrong_type["steps"][1]["type"] = json!("CORTEX_STEP_TYPE_GENERIC");
        assert_eq!(pending_question_in(&wrong_type, 7), None);
        assert_eq!(pending_question_in(&json!({ "steps": [] }), 7), None);
        assert_eq!(pending_question_in(&json!("not a response"), 7), None);
    }

    #[test]
    fn answer_locust_sends_is_exact_ide_answer_for_same_choice() {
        let pending = pending_question_in(&waiting(), 7).expect("pending");
        assert_eq!(
            question_interaction(
                CONVERSATION,
                &pending,
                &[AntigravityQuestionResponse {
                    selected_option_ids: vec!["1".into()],
                    write_in: None,
                    skipped: false,
                }],
            ),
            json!({
                "cascadeId": CONVERSATION,
                "interaction": ide_answer(),
            })
        );
    }

    #[test]
    fn answer_carries_words_written_in_and_skip_the_way_the_card_offers_them() {
        let pending = pending_question_in(&waiting(), 7).expect("pending");
        let written = question_interaction(
            CONVERSATION,
            &pending,
            &[AntigravityQuestionResponse {
                selected_option_ids: Vec::new(),
                write_in: Some("  keep it flat, but add a docs folder ".into()),
                skipped: false,
            }],
        );
        assert_eq!(
            written["interaction"]["askQuestion"]["responses"][0]["writeInResponse"],
            json!("keep it flat, but add a docs folder")
        );
        assert_eq!(
            written["interaction"]["askQuestion"]["responses"][0]["selectedOptionIds"],
            json!([])
        );

        let skipped = question_interaction(
            CONVERSATION,
            &pending,
            &[AntigravityQuestionResponse {
                selected_option_ids: Vec::new(),
                write_in: None,
                skipped: true,
            }],
        );
        assert_eq!(
            skipped["interaction"]["askQuestion"]["responses"][0]["skipped"],
            json!(true)
        );
        assert_eq!(
            skipped["interaction"]["askQuestion"]["responses"][0]["selectedOptionIds"],
            json!([])
        );
    }

    struct FakePost {
        asked: Mutex<Vec<(u16, String, String)>>,
        responder: Box<dyn Fn(u16, &str, &Value, &str) -> Result<CascadeHttpResult, String> + Send + Sync>,
    }

    impl CascadePost for FakePost {
        fn post(
            &self,
            port: u16,
            method: &str,
            body: &Value,
            token: &str,
        ) -> Result<CascadeHttpResult, String> {
            crate::invariants::lock(&self.asked).push((port, method.into(), token.into()));
            (self.responder)(port, method, body, token)
        }
    }

    #[test]
    fn uses_plain_http_port_passing_over_tls_refusal_and_remembers_it() {
        let token = "60843f52-6d41-4d97-9b31-53157a780b5e";
        let post = FakePost {
            asked: Mutex::new(Vec::new()),
            responder: Box::new(|port, _, _, _| {
                if port == 57058 {
                    Ok(CascadeHttpResult {
                        status: 400,
                        text: "Client sent an HTTP request to an HTTPS server.\n".into(),
                    })
                } else {
                    Ok(CascadeHttpResult {
                        status: 200,
                        text: waiting().to_string(),
                    })
                }
            }),
        };
        let client = CascadeClient::with_post(&[57058, 57059], Some("localhost:57058"), token, post);
        let found = client
            .pending_question(CONVERSATION, 7)
            .expect("pending")
            .expect("some");
        assert_eq!(found.trajectory_id, TRAJECTORY);
        assert_eq!(found.step_index, 8);
        {
            let asked = crate::invariants::lock(&client.post.asked);
            let ports: Vec<u16> = asked.iter().map(|c| c.0).collect();
            assert_eq!(ports, vec![57058, 57059]);
            assert!(
                asked
                    .iter()
                    .all(|c| c.1 == "GetCascadeTrajectorySteps" && c.2 == token)
            );
        }
        // And remembers it.
        client.pending_question(CONVERSATION, 7).expect("second call");
        let asked = crate::invariants::lock(&client.post.asked);
        let ports: Vec<u16> = asked.iter().skip(2).map(|c| c.0).collect();
        assert_eq!(ports, vec![57059]);
    }

    #[test]
    fn says_why_when_antigravity_refuses_answer_and_redacts_token() {
        let token = "60843f52-6d41-4d97-9b31-53157a780b5e";
        let post = FakePost {
            asked: Mutex::new(Vec::new()),
            responder: Box::new(|_, _, _, tok| {
                Ok(CascadeHttpResult {
                    status: 400,
                    text: json!({
                        "code": "failed_precondition",
                        "message": format!("step 8 is not waiting for an interaction ({tok})"),
                    })
                    .to_string(),
                })
            }),
        };
        let client = CascadeClient::with_post(&[57059], None, token, post);
        let pending = pending_question_in(&waiting(), 7).expect("pending");
        let err = client
            .answer_question(
                CONVERSATION,
                &pending,
                &[AntigravityQuestionResponse {
                    selected_option_ids: vec!["1".into()],
                    write_in: None,
                    skipped: false,
                }],
            )
            .expect_err("should fail");
        assert!(err.contains("step 8 is not waiting for an interaction"));
        assert!(!err.contains(token));
    }

    #[test]
    fn plain_question_and_responses_from_answers_match_electron() {
        assert_eq!(
            plain_question(QUESTION),
            "How would you like to organize this folder (currently containing README.md and hello.txt)?"
        );
        let pending = pending_question_in(&waiting(), 7).expect("pending");
        let mut answers = HashMap::new();
        answers.insert("0".into(), vec![options()[0].text.clone()]);
        assert_eq!(
            responses_from_answers(&pending, &answers),
            vec![AntigravityQuestionResponse {
                selected_option_ids: vec!["1".into()],
                write_in: None,
                skipped: false,
            }]
        );
        answers.insert("0".into(), vec!["keep it flat".into()]);
        assert_eq!(
            responses_from_answers(&pending, &answers),
            vec![AntigravityQuestionResponse {
                selected_option_ids: Vec::new(),
                write_in: Some("keep it flat".into()),
                skipped: false,
            }]
        );
        answers.insert("0".into(), Vec::new());
        assert_eq!(
            responses_from_answers(&pending, &answers),
            vec![AntigravityQuestionResponse {
                selected_option_ids: Vec::new(),
                write_in: None,
                skipped: true,
            }]
        );
    }

    #[test]
    fn planner_texts_in_reads_modified_and_raw_planner_responses_only() {
        let steps = json!({
            "steps": [
                {
                    "type": "CORTEX_STEP_TYPE_PLANNER_RESPONSE",
                    "plannerResponse": { "modifiedResponse": "mod", "response": "raw" }
                },
                {
                    "type": "CORTEX_STEP_TYPE_GENERIC",
                    "plannerResponse": { "response": "ignored" }
                }
            ]
        });
        assert_eq!(planner_texts_in(&steps), vec!["mod", "raw"]);
    }
}
