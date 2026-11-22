//! Ports `apps/desktop/src/main/a-long-antigravity-answer-comes-back-whole.test.ts`.

use crate::antigravity_cascade::{
    AntigravityPendingQuestion, AntigravityQuestionResponse, planner_texts_in,
};
use crate::antigravity_host::{AgentApi, AntigravityHost, NewConversationInput, SendMessageInput};
use crate::antigravity_mission::{
    ANTIGRAVITY_ASKING_NOTICE_MS, ANTIGRAVITY_IDLE_TIMEOUT_MS, ANTIGRAVITY_NOT_STARTED_NOTICE_MS,
    ANTIGRAVITY_QUESTION_LOOKUPS, AntigravityMissionOptions, AntigravityMissionService, CascadeApi,
    MissionLedger, MissionRoute, TranscriptStat,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CONVERSATION: &str = "ec3897f1-dc20-44a5-bf84-e9de9f183ea0";
const BEFORE: &str =
    "Accessibility & Engine Core: Implements Windows High Contrast mode (`forced-colors: acti";
const AFTER: &str =
    "/kill-switch recommendations due 2026-11-16 and municipal AI hearing developments.";

fn whole() -> String {
    format!("{BEFORE}ve`) across the shell and every card, and owns the{AFTER}")
}

fn user_line() -> String {
    json!({
        "step_index": 0,
        "source": "USER_EXPLICIT",
        "type": "USER_INPUT",
        "status": "DONE",
        "created_at": "2026-09-27T04:42:52Z",
        "content": "<USER_REQUEST>\nWho takes what?\n</USER_REQUEST>"
    })
    .to_string()
}

fn answer_line(content: &str) -> String {
    json!({
        "step_index": 12,
        "source": "MODEL",
        "type": "PLANNER_RESPONSE",
        "status": "DONE",
        "created_at": "2026-09-27T04:44:52Z",
        "content": content
    })
    .to_string()
}

fn steps_response() -> Value {
    let w = whole();
    json!({
        "steps": [
            {
                "type": "CORTEX_STEP_TYPE_PLANNER_RESPONSE",
                "status": "CORTEX_STEP_STATUS_DONE",
                "plannerResponse": { "response": w, "modifiedResponse": w, "messageId": "bot-1" }
            },
            {
                "type": "CORTEX_STEP_TYPE_USER_INPUT",
                "status": "CORTEX_STEP_STATUS_DONE"
            }
        ]
    })
}

struct NoopLedger;
impl MissionLedger for NoopLedger {
    fn create_mission(&self, _input: Value) -> Result<(), String> {
        Ok(())
    }
    fn append_events(&self, _mission_id: &str, _events: &[Value]) -> Result<(), String> {
        Ok(())
    }
}

struct FakeAgentApi;
impl AgentApi for FakeAgentApi {
    fn new_conversation(&self, _input: NewConversationInput) -> Result<String, String> {
        Ok(CONVERSATION.into())
    }
    fn send_message(&self, _input: SendMessageInput) -> Result<(), String> {
        Ok(())
    }
}

struct FakeCascade {
    asked: Arc<Mutex<Vec<usize>>>,
    responder: Arc<dyn Fn() -> Result<Vec<String>, String> + Send + Sync>,
}

impl CascadeApi for FakeCascade {
    fn pending_question(
        &self,
        _conversation_id: &str,
        _from_step: usize,
    ) -> Result<Option<AntigravityPendingQuestion>, String> {
        Ok(None)
    }
    fn answer_question(
        &self,
        _conversation_id: &str,
        _pending: &AntigravityPendingQuestion,
        _responses: &[AntigravityQuestionResponse],
    ) -> Result<(), String> {
        Ok(())
    }
    fn planner_texts(
        &self,
        _conversation_id: &str,
        from_step: usize,
    ) -> Result<Vec<String>, String> {
        crate::invariants::lock(&self.asked).push(from_step);
        (self.responder)()
    }
}

fn test_host() -> AntigravityHost {
    AntigravityHost {
        executable_path: "C:\\lang\\language_server.exe".into(),
        version: Some("2.15.1".into()),
        address: "localhost:57058".into(),
        csrf_token: "60843f52-6d41-4d97-9b31-53157a780b5e".into(),
        projects: HashMap::from([(
            "c:/work/pebble".into(),
            "daf0f8ec-bb8e-49e8-a445-954eb0a62d0f".into(),
        )]),
        ports: vec![57058],
    }
}

fn harness(
    cascade: Arc<dyn CascadeApi>,
    read_transcript: Arc<dyn Fn(&str) -> Option<String> + Send + Sync>,
    stat_transcript: Option<Arc<dyn Fn(&str) -> Option<TranscriptStat> + Send + Sync>>,
) -> (AntigravityMissionService, Arc<Mutex<Vec<Value>>>) {
    let recorded: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let rec = recorded.clone();
    let ids = Arc::new(AtomicUsize::new(0));
    let service = AntigravityMissionService::new(AntigravityMissionOptions {
        workspace_path: "C:\\work\\pebble".into(),
        ledger: Arc::new(NoopLedger),
        probe: Arc::new(|| Some(test_host())),
        agent_api: Arc::new(|_| Arc::new(FakeAgentApi)),
        cascade_api: Some(Arc::new(move |_| cascade.clone())),
        read_transcript,
        stat_transcript,
        emit_event: Arc::new(move |_, _, ev| crate::invariants::lock(&rec).push(ev)),
        emit_update: None,
        notify: None,
        emit_approval: None,
        withdraw_approval: None,
        on_run_ended: None,
        spend_refusal: None,
        observe_disk: None,
        observe_patches: None,
        home: "C:\\Users\\dev".into(),
        create_id: Arc::new(move || (ids.fetch_add(1, Ordering::Relaxed) + 1).to_string()),
        now_iso: Arc::new(|| "2026-09-27T04:42:50.000Z".into()),
        poll_ms: 5,
        idle_timeout_ms: ANTIGRAVITY_IDLE_TIMEOUT_MS,
        asking_notice_ms: ANTIGRAVITY_ASKING_NOTICE_MS,
        not_started_notice_ms: ANTIGRAVITY_NOT_STARTED_NOTICE_MS,
        question_lookups: ANTIGRAVITY_QUESTION_LOOKUPS,
    });
    (service, recorded)
}

fn answer_text(recorded: &Arc<Mutex<Vec<Value>>>) -> Option<String> {
    crate::invariants::lock(recorded)
        .iter()
        .find(|ev| ev["type"] == "message.delta")
        .and_then(|ev| ev["payload"]["text"].as_str())
        .map(str::to_owned)
}

fn wait_until(cond: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if cond() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(cond(), "condition not met before timeout");
}

// case: reads the server's steps as the server writes them
#[test]
fn reads_the_servers_steps_as_the_server_writes_them() {
    let w = whole();
    assert_eq!(planner_texts_in(&steps_response()), vec![w.clone(), w]);
    assert!(
        planner_texts_in(&json!({
            "steps": [{ "type": "CORTEX_STEP_TYPE_GENERIC", "plannerResponse": { "response": "x" } }]
        }))
        .is_empty()
    );
}

// case: is recorded whole: the gap in the transcript is filled from the step, asked for from that step on
#[test]
fn is_recorded_whole_the_gap_in_the_transcript_is_filled_from_the_step_asked_for_from_that_step_on()
{
    let asked = Arc::new(Mutex::new(Vec::new()));
    let cascade = Arc::new(FakeCascade {
        asked: asked.clone(),
        responder: Arc::new(|| Ok(planner_texts_in(&steps_response()))),
    });
    let gapped = format!("{BEFORE}\n<truncated 1066 bytes>\n{AFTER}");
    let content = [user_line(), answer_line(&gapped)].join("\n");
    let (service, recorded) = harness(cascade, Arc::new(move |_| Some(content.clone())), None);
    service
        .start("Who takes what?", None, MissionRoute::default())
        .expect("start");
    wait_until(|| answer_text(&recorded).as_deref() == Some(whole().as_str()));
    assert_eq!(*crate::invariants::lock(&asked), vec![12]);
    service.dispose();
}

// case: keeps the gap, drawn as before, when the server cannot say
#[test]
fn keeps_the_gap_drawn_as_before_when_the_server_cannot_say() {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let cascade = Arc::new(FakeCascade {
        asked,
        responder: Arc::new(|| Err("Antigravity is not listening.".into())),
    });
    let gapped = format!("{BEFORE}\n<truncated 1066 bytes>\n{AFTER}");
    let expected = gapped.clone();
    let content = [user_line(), answer_line(&gapped)].join("\n");
    let (service, recorded) = harness(cascade, Arc::new(move |_| Some(content.clone())), None);
    service
        .start("Who takes what?", None, MissionRoute::default())
        .expect("start");
    wait_until(|| answer_text(&recorded).as_deref() == Some(expected.as_str()));
    service.dispose();
}

// case: is read again after a read of the transcript fails once (2026-10-10 sweep)
#[test]
fn is_read_again_after_a_read_of_the_transcript_fails_once() {
    let cascade = Arc::new(FakeCascade {
        asked: Arc::new(Mutex::new(Vec::new())),
        responder: Arc::new(|| Ok(Vec::new())),
    });
    let reads = Arc::new(AtomicUsize::new(0));
    let reads_clone = reads.clone();
    let second = [user_line(), answer_line("Read on the second try.")].join("\n");
    let (service, recorded) = harness(
        cascade,
        Arc::new(move |_| {
            let n = reads_clone.fetch_add(1, Ordering::SeqCst) + 1;
            if n == 1 { None } else { Some(second.clone()) }
        }),
        Some(Arc::new(|_| {
            Some(TranscriptStat {
                size: 100,
                mtime_ms: 1,
            })
        })),
    );
    service
        .start("Who takes what?", None, MissionRoute::default())
        .expect("start");
    wait_until(|| answer_text(&recorded).as_deref() == Some("Read on the second try."));
    assert!(reads.load(Ordering::SeqCst) >= 2);
    service.dispose();
}

// case: asks the server nothing when nothing is missing
#[test]
fn asks_the_server_nothing_when_nothing_is_missing() {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let cascade = Arc::new(FakeCascade {
        asked: asked.clone(),
        responder: Arc::new(|| Ok(vec![whole()])),
    });
    let content = [user_line(), answer_line("A short, whole answer.")].join("\n");
    let (service, recorded) = harness(cascade, Arc::new(move |_| Some(content.clone())), None);
    service
        .start("Who takes what?", None, MissionRoute::default())
        .expect("start");
    wait_until(|| answer_text(&recorded).as_deref() == Some("A short, whole answer."));
    assert!(crate::invariants::lock(&asked).is_empty());
    service.dispose();
}
