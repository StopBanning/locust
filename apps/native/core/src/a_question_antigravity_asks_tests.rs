//! Ports `apps/desktop/src/main/a-question-antigravity-asks.test.ts`.

use crate::antigravity_cascade::{
    AntigravityAskedQuestion, AntigravityPendingQuestion, AntigravityQuestionOption,
    AntigravityQuestionResponse, CascadeClient, CascadeHttpResult, CascadePost,
    pending_question_in, question_interaction,
};
use crate::antigravity_host::{AgentApi, AntigravityHost, NewConversationInput, SendMessageInput};
use crate::antigravity_mission::{
    ANTIGRAVITY_ASKING_NOTICE_MS, ANTIGRAVITY_IDLE_TIMEOUT_MS, ANTIGRAVITY_NOT_STARTED_NOTICE_MS,
    AntigravityMissionOptions, AntigravityMissionService, CascadeApi, MissionLedger, MissionRoute,
    RaisedApproval,
};
use crate::approval::{Answer, Channel, Decision, Kind, approval_answer_from};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CONVERSATION: &str = "ec3897f1-dc20-44a5-bf84-e9de9f183ea0";
const TRAJECTORY: &str = "90cfc6a4-467b-438e-9beb-3752d4bc4b89";
const WORKSPACE: &str = "C:\\work\\pebble";
const HOME: &str = "C:\\Users\\dev";
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

fn options_json() -> Value {
    Value::Array(
        options()
            .into_iter()
            .map(|o| json!({ "id": o.id, "text": o.text }))
            .collect(),
    )
}

fn step_info(step_index: usize) -> Value {
    json!({
        "trajectoryId": TRAJECTORY,
        "stepIndex": step_index,
        "metadataIndex": 3,
        "cascadeId": CONVERSATION,
    })
}

fn waiting_steps() -> Value {
    json!({
        "steps": [
            {
                "type": "CORTEX_STEP_TYPE_PLANNER_RESPONSE",
                "status": "CORTEX_STEP_STATUS_DONE",
                "metadata": { "sourceTrajectoryStepInfo": step_info(7) }
            },
            {
                "type": "CORTEX_STEP_TYPE_ASK_QUESTION",
                "status": "CORTEX_STEP_STATUS_RUNNING",
                "metadata": {
                    "toolAction": "Asking folder organization preference",
                    "sourceTrajectoryStepInfo": step_info(8)
                },
                "askQuestion": {
                    "questions": [{ "question": QUESTION, "options": options_json() }]
                }
            }
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
                "selectedOptionIds": ["1"]
            }]
        }
    })
}

fn answered_steps() -> Value {
    let mut w = waiting_steps();
    w["steps"][1]["status"] = json!("CORTEX_STEP_STATUS_DONE");
    w["steps"][1]["completedInteractions"] = json!([{
        "request": { "askQuestion": { "questions": [{ "question": QUESTION, "options": options_json() }] } },
        "response": ide_answer()
    }]);
    w
}

fn user_line() -> String {
    json!({
        "step_index": 0,
        "source": "USER_EXPLICIT",
        "type": "USER_INPUT",
        "status": "DONE",
        "created_at": "2026-09-23T04:42:52Z",
        "content": "<USER_REQUEST>\nAsk me to choose.\n</USER_REQUEST>"
    })
    .to_string()
}

fn asks_line() -> String {
    let opt_texts: Vec<String> = options().into_iter().map(|o| o.text).collect();
    json!({
        "step_index": 7,
        "source": "MODEL",
        "type": "PLANNER_RESPONSE",
        "status": "DONE",
        "created_at": "2026-09-23T04:43:02Z",
        "tool_calls": [{
            "name": "ask_question",
            "args": {
                "questions": json!([{ "is_multi_select": false, "options": opt_texts, "question": QUESTION }]).to_string(),
                "toolAction": json!("Asking folder organization preference").to_string(),
                "toolSummary": json!("Folder organization options").to_string()
            }
        }]
    })
    .to_string()
}

fn answer_arrives_line() -> String {
    json!({
        "step_index": 8,
        "source": "MODEL",
        "type": "GENERIC",
        "status": "DONE",
        "created_at": "2026-09-23T04:43:04Z",
        "content": format!("Created At: 2026-09-23T00:43:04-04:00\nCompleted At: 2026-09-23T00:44:07-04:00\nA1: {}", options()[0].text)
    })
    .to_string()
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
    pending: bool,
    refuse_answer: Option<String>,
    answers: Mutex<Vec<Vec<AntigravityQuestionResponse>>>,
    looks: AtomicUsize,
}

impl FakeCascade {
    fn new(pending: bool) -> Arc<Self> {
        Arc::new(Self {
            pending,
            refuse_answer: None,
            answers: Mutex::new(Vec::new()),
            looks: AtomicUsize::new(0),
        })
    }

    fn refusing(message: &str) -> Arc<Self> {
        Arc::new(Self {
            pending: true,
            refuse_answer: Some(message.into()),
            answers: Mutex::new(Vec::new()),
            looks: AtomicUsize::new(0),
        })
    }
}

impl CascadeApi for FakeCascade {
    fn pending_question(
        &self,
        _conversation_id: &str,
        from_step: usize,
    ) -> Result<Option<AntigravityPendingQuestion>, String> {
        self.looks.fetch_add(1, Ordering::SeqCst);
        Ok(if self.pending {
            pending_question_in(&waiting_steps(), from_step)
        } else {
            None
        })
    }
    fn answer_question(
        &self,
        _conversation_id: &str,
        _pending: &AntigravityPendingQuestion,
        responses: &[AntigravityQuestionResponse],
    ) -> Result<(), String> {
        if let Some(ref err) = self.refuse_answer {
            return Err(err.clone());
        }
        crate::invariants::lock(&self.answers).push(responses.to_vec());
        Ok(())
    }
    fn planner_texts(
        &self,
        _conversation_id: &str,
        _from_step: usize,
    ) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
}

struct Harness {
    service: AntigravityMissionService,
    transcript: Arc<Mutex<Vec<String>>>,
    raised: Arc<Mutex<Vec<RaisedApproval>>>,
    withdrawn: Arc<Mutex<Vec<String>>>,
    notices: Arc<Mutex<Vec<String>>>,
}

fn harness(cascade: Arc<dyn CascadeApi>, idle_timeout_ms: Option<u64>) -> Harness {
    let transcript = Arc::new(Mutex::new(vec![user_line()]));
    let tr = transcript.clone();
    let raised: Arc<Mutex<Vec<RaisedApproval>>> = Arc::new(Mutex::new(Vec::new()));
    let r = raised.clone();
    let withdrawn: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let w = withdrawn.clone();
    let notices: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let n = notices.clone();
    let ids = Arc::new(AtomicUsize::new(0));
    let service = AntigravityMissionService::new(AntigravityMissionOptions {
        workspace_path: WORKSPACE.into(),
        ledger: Arc::new(NoopLedger),
        probe: Arc::new(|| {
            Some(AntigravityHost {
                executable_path: "C:\\lang\\language_server.exe".into(),
                version: Some("2.15.1".into()),
                address: "localhost:57058".into(),
                csrf_token: "60843f52-6d41-4d97-9b31-53157a780b5e".into(),
                projects: HashMap::from([(
                    "c:/work/pebble".into(),
                    "daf0f8ec-bb8e-49e8-a445-954eb0a62d0f".into(),
                )]),
                ports: vec![57058, 57059],
            })
        }),
        agent_api: Arc::new(|_| Arc::new(FakeAgentApi)),
        cascade_api: Some(Arc::new(move |_| cascade.clone())),
        read_transcript: Arc::new(move |_| Some(crate::invariants::lock(&tr).join("\n"))),
        stat_transcript: None,
        emit_event: Arc::new(|_, _, _| {}),
        emit_update: None,
        notify: Some(Arc::new(move |notice| {
            crate::invariants::lock(&n).push(notice.message);
        })),
        emit_approval: Some(Arc::new(move |card| {
            crate::invariants::lock(&r).push(card);
        })),
        withdraw_approval: Some(Arc::new(move |id| {
            crate::invariants::lock(&w).push(id.to_owned());
        })),
        on_run_ended: None,
        spend_refusal: None,
        observe_disk: None,
        observe_patches: None,
        home: HOME.into(),
        create_id: Arc::new(move || (ids.fetch_add(1, Ordering::Relaxed) + 1).to_string()),
        now_iso: Arc::new(|| "2026-09-23T04:42:50.000Z".into()),
        poll_ms: 5,
        idle_timeout_ms: idle_timeout_ms.unwrap_or(ANTIGRAVITY_IDLE_TIMEOUT_MS),
        asking_notice_ms: ANTIGRAVITY_ASKING_NOTICE_MS,
        not_started_notice_ms: ANTIGRAVITY_NOT_STARTED_NOTICE_MS,
        question_lookups: 3,
    });
    Harness {
        service,
        transcript,
        raised,
        withdrawn,
        notices,
    }
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

struct FakePost {
    asked: Arc<Mutex<Vec<(u16, String, String)>>>,
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

// case: is the step holding for an answer, with the ids an answer has to name
#[test]
fn is_the_step_holding_for_an_answer_with_the_ids_an_answer_has_to_name() {
    assert_eq!(
        pending_question_in(&waiting_steps(), 7),
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

// case: is nothing once answered
#[test]
fn is_nothing_once_answered() {
    assert_eq!(pending_question_in(&answered_steps(), 7), None);
    assert_eq!(pending_question_in(&json!({ "steps": [] }), 7), None);
    assert_eq!(pending_question_in(&json!("not a response"), 7), None);
}

// case: is exactly the one Antigravity's own window sent for the same choice
#[test]
fn is_exactly_the_one_antigravitys_own_window_sent_for_the_same_choice() {
    let pending = pending_question_in(&waiting_steps(), 7).expect("pending");
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
        json!({ "cascadeId": CONVERSATION, "interaction": ide_answer() })
    );
}

// case: carries words written in, and a skip, the way the card offers them
#[test]
fn carries_words_written_in_and_a_skip_the_way_the_card_offers_them() {
    let pending = pending_question_in(&waiting_steps(), 7).expect("pending");
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

// case: uses the plain-HTTP port, passing over the TLS one's refusal, and sends the CLI's token
#[test]
fn uses_the_plain_http_port_passing_over_the_tls_ones_refusal_and_sends_the_clis_token() {
    let token = "60843f52-6d41-4d97-9b31-53157a780b5e";
    let asked = Arc::new(Mutex::new(Vec::new()));
    let post = FakePost {
        asked: asked.clone(),
        responder: Box::new(|port, _, _, _| {
            if port == 57058 {
                Ok(CascadeHttpResult {
                    status: 400,
                    text: "Client sent an HTTP request to an HTTPS server.\n".into(),
                })
            } else {
                Ok(CascadeHttpResult {
                    status: 200,
                    text: waiting_steps().to_string(),
                })
            }
        }),
    };
    let api = CascadeClient::with_post(&[57058, 57059], Some("localhost:57058"), token, post);
    let found = api
        .pending_question(CONVERSATION, 7)
        .expect("pending")
        .expect("some");
    assert_eq!(found.trajectory_id, TRAJECTORY);
    assert_eq!(found.step_index, 8);
    {
        let calls = crate::invariants::lock(&asked);
        let ports: Vec<u16> = calls.iter().map(|c| c.0).collect();
        assert_eq!(ports, vec![57058, 57059]);
        assert!(
            calls
                .iter()
                .all(|c| c.1 == "GetCascadeTrajectorySteps" && c.2 == token)
        );
    }
    // And remembers it.
    let found2 = api
        .pending_question(CONVERSATION, 7)
        .expect("second")
        .expect("some");
    assert_eq!(found2.step_index, 8);
    let calls = crate::invariants::lock(&asked);
    let later_ports: Vec<u16> = calls.iter().skip(2).map(|c| c.0).collect();
    assert_eq!(later_ports, vec![57059]);
}

// case: says why when Antigravity refuses the answer, in the server's own words
#[test]
fn says_why_when_antigravity_refuses_the_answer_in_the_servers_own_words() {
    let post = FakePost {
        asked: Arc::new(Mutex::new(Vec::new())),
        responder: Box::new(|_, _, _, _| {
            Ok(CascadeHttpResult {
                status: 400,
                text: json!({
                    "code": "failed_precondition",
                    "message": "step 8 is not waiting for an interaction"
                })
                .to_string(),
            })
        }),
    };
    let api = CascadeClient::with_post(
        &[57059],
        None,
        "60843f52-6d41-4d97-9b31-53157a780b5e",
        post,
    );
    let pending = pending_question_in(&waiting_steps(), 7).expect("pending");
    let err = api
        .answer_question(
            CONVERSATION,
            &pending,
            &[AntigravityQuestionResponse {
                selected_option_ids: vec!["1".into()],
                write_in: None,
                skipped: false,
            }],
        )
        .expect_err("refused");
    assert!(err.contains("step 8 is not waiting for an interaction"));
}

// case: raises Locust's question card: the question in words, its options, a skip, and who is asking
#[test]
fn raises_locusts_question_card_the_question_in_words_its_options_a_skip_and_who_is_asking() {
    let cascade = FakeCascade::new(true);
    let h = harness(cascade, None);
    let run = h
        .service
        .start("Ask me to choose.", None, MissionRoute::default())
        .expect("start");
    crate::invariants::lock(&h.transcript).push(asks_line());
    wait_until(|| crate::invariants::lock(&h.raised).len() == 1);
    let raised = crate::invariants::lock(&h.raised);
    let card = &raised[0];
    assert_eq!(card.run_id, run.run_id);
    assert_eq!(card.request.mission_id, run.mission_id);
    assert_eq!(card.request.runtime, "antigravity");
    assert_eq!(card.request.kind, Kind::Question);
    assert!(card.request.skippable);
    assert_eq!(card.request.summary, "Asking folder organization preference");
    assert_eq!(card.request.answer_in, None);
    assert_eq!(
        card.request.questions[0].question,
        "How would you like to organize this folder (currently containing README.md and hello.txt)?"
    );
    let labels: Vec<String> = card.request.questions[0]
        .options
        .iter()
        .map(|o| o.label.clone())
        .collect();
    let expected: Vec<String> = options().into_iter().map(|o| o.text).collect();
    assert_eq!(labels, expected);
    assert!(card.request.questions[0].is_other);
    drop(raised);
    h.service.dispose();
}

// case: answers it through Antigravity: the option picked, by its id
#[test]
fn answers_it_through_antigravity_the_option_picked_by_its_id() {
    let cascade = FakeCascade::new(true);
    let h = harness(cascade.clone(), None);
    h.service
        .start("Ask me to choose.", None, MissionRoute::default())
        .expect("start");
    crate::invariants::lock(&h.transcript).push(asks_line());
    wait_until(|| crate::invariants::lock(&h.raised).len() == 1);
    let approval_id = crate::invariants::lock(&h.raised)[0].request.approval_id.clone();
    let ok = h.service.decide(&Answer::Answers {
        approval_id: approval_id.clone(),
        answers: vec![("0".into(), vec![options()[0].text.clone()])],
    });
    assert!(ok);
    assert_eq!(
        *crate::invariants::lock(&cascade.answers),
        vec![vec![AntigravityQuestionResponse {
            selected_option_ids: vec!["1".into()],
            write_in: None,
            skipped: false,
        }]]
    );
    // Once only: a double click does not answer twice.
    assert!(!h.service.decide(&Answer::Answers {
        approval_id,
        answers: vec![("0".into(), vec![options()[1].text.clone()])],
    }));
    h.service.dispose();
}

// case: sends words written in, and Skip as a skip
#[test]
fn sends_words_written_in_and_skip_as_a_skip() {
    let cascade = FakeCascade::new(true);
    let h = harness(cascade.clone(), None);
    h.service
        .start("Ask me to choose.", None, MissionRoute::default())
        .expect("start");
    crate::invariants::lock(&h.transcript).push(asks_line());
    wait_until(|| crate::invariants::lock(&h.raised).len() == 1);
    let approval_id = crate::invariants::lock(&h.raised)[0].request.approval_id.clone();
    assert!(h.service.decide(&Answer::Answers {
        approval_id,
        answers: vec![("0".into(), vec!["keep it flat".into()])],
    }));
    assert_eq!(
        crate::invariants::lock(&cascade.answers)[0],
        vec![AntigravityQuestionResponse {
            selected_option_ids: Vec::new(),
            write_in: Some("keep it flat".into()),
            skipped: false,
        }]
    );
    h.service.dispose();

    let second = FakeCascade::new(true);
    let other = harness(second.clone(), None);
    other
        .service
        .start("Ask me to choose.", None, MissionRoute::default())
        .expect("start");
    crate::invariants::lock(&other.transcript).push(asks_line());
    wait_until(|| crate::invariants::lock(&other.raised).len() == 1);
    let approval_id = crate::invariants::lock(&other.raised)[0]
        .request
        .approval_id
        .clone();
    assert!(other.service.decide(&Answer::Answers {
        approval_id,
        answers: vec![("0".into(), Vec::new())],
    }));
    assert_eq!(
        crate::invariants::lock(&second.answers)[0],
        vec![AntigravityQuestionResponse {
            selected_option_ids: Vec::new(),
            write_in: None,
            skipped: true,
        }]
    );
    other.service.dispose();
}

// case: takes the card down when it was answered in Antigravity's own window
#[test]
fn takes_the_card_down_when_it_was_answered_in_antigravitys_own_window() {
    let h = harness(FakeCascade::new(true), None);
    h.service
        .start("Ask me to choose.", None, MissionRoute::default())
        .expect("start");
    crate::invariants::lock(&h.transcript).push(asks_line());
    wait_until(|| crate::invariants::lock(&h.raised).len() == 1);
    let id = crate::invariants::lock(&h.raised)[0].request.approval_id.clone();
    crate::invariants::lock(&h.transcript).push(answer_arrives_line());
    wait_until(|| *crate::invariants::lock(&h.withdrawn) == vec![id.clone()]);
    h.service.dispose();
}

// case: never takes a run that is waiting on the person for one that went quiet
#[test]
fn never_takes_a_run_that_is_waiting_on_the_person_for_one_that_went_quiet() {
    let h = harness(FakeCascade::new(true), Some(40));
    let run = h
        .service
        .start("Ask me to choose.", None, MissionRoute::default())
        .expect("start");
    crate::invariants::lock(&h.transcript).push(asks_line());
    wait_until(|| crate::invariants::lock(&h.raised).len() == 1);
    std::thread::sleep(Duration::from_millis(80));
    assert!(h.service.has(&run.run_id));
    let notices = crate::invariants::lock(&h.notices);
    assert!(
        !notices
            .iter()
            .any(|m| m.to_lowercase().contains("might") || m.to_lowercase().contains("without reporting"))
    );
    drop(notices);
    h.service.dispose();
}

// case: still shows the question when the waiting step cannot be found -- and says to answer it in Antigravity
#[test]
fn still_shows_the_question_when_the_waiting_step_cannot_be_found_and_says_to_answer_it_in_antigravity()
{
    let cascade = FakeCascade::new(false);
    let h = harness(cascade.clone(), None);
    h.service
        .start("Ask me to choose.", None, MissionRoute::default())
        .expect("start");
    crate::invariants::lock(&h.transcript).push(asks_line());
    wait_until(|| {
        cascade.looks.load(Ordering::SeqCst) == 3 && crate::invariants::lock(&h.raised).len() == 1
    });
    let card = crate::invariants::lock(&h.raised)[0].request.clone();
    assert_eq!(card.answer_in.as_deref(), Some("Antigravity"));
    assert_eq!(card.kind, Kind::Question);
    assert!(!card.skippable);
    assert!(!h.service.decide(&Answer::Answers {
        approval_id: card.approval_id,
        answers: vec![("0".into(), vec![options()[0].text.clone()])],
    }));
    h.service.dispose();
}

// case: takes the card down when the run ends with it still up
#[test]
fn takes_the_card_down_when_the_run_ends_with_it_still_up() {
    let h = harness(FakeCascade::new(true), None);
    let run = h
        .service
        .start("Ask me to choose.", None, MissionRoute::default())
        .expect("start");
    crate::invariants::lock(&h.transcript).push(asks_line());
    wait_until(|| crate::invariants::lock(&h.raised).len() == 1);
    let id = crate::invariants::lock(&h.raised)[0].request.approval_id.clone();
    assert!(h.service.cancel(&run.run_id));
    wait_until(|| *crate::invariants::lock(&h.withdrawn) == vec![id.clone()]);
}

// case: says so in the thread when Antigravity would not take the answer, and keeps the card
#[test]
fn says_so_in_the_thread_when_antigravity_would_not_take_the_answer_and_keeps_the_card() {
    let cascade = FakeCascade::refusing("step 8 is not waiting for an interaction");
    let h = harness(cascade, None);
    h.service
        .start("Ask me to choose.", None, MissionRoute::default())
        .expect("start");
    crate::invariants::lock(&h.transcript).push(asks_line());
    wait_until(|| crate::invariants::lock(&h.raised).len() == 1);
    let id = crate::invariants::lock(&h.raised)[0].request.approval_id.clone();
    assert!(!h.service.decide(&Answer::Answers {
        approval_id: id,
        answers: vec![("0".into(), vec![options()[0].text.clone()])],
    }));
    let notices = crate::invariants::lock(&h.notices);
    assert!(notices.iter().any(|m| {
        m.contains("step 8 is not waiting for an interaction")
            && m.contains("Antigravity's own window")
    }));
    assert!(crate::invariants::lock(&h.withdrawn).is_empty());
    drop(notices);
    h.service.dispose();
}

// case: forwards a question's answers, which used to be dropped as a denial
#[test]
fn forwards_a_questions_answers_which_used_to_be_dropped_as_a_denial() {
    match approval_answer_from(&json!({
        "approvalId": "ap_1",
        "answers": { "0": ["Standard Project Layout"] }
    }))
    .expect("answers")
    {
        Answer::Answers {
            approval_id,
            answers,
        } => {
            assert_eq!(approval_id, "ap_1");
            assert_eq!(answers, vec![("0".into(), vec!["Standard Project Layout".into()])]);
        }
        other => panic!("expected Answers, got {other:?}"),
    }
    match approval_answer_from(&json!({ "approvalId": "ap_1", "answers": { "0": [] } }))
        .expect("empty answers")
    {
        Answer::Answers {
            approval_id,
            answers,
        } => {
            assert_eq!(approval_id, "ap_1");
            assert_eq!(answers, vec![("0".into(), Vec::<String>::new())]);
        }
        other => panic!("expected Answers, got {other:?}"),
    }
}

// case: refuses answers that are not lists of words, and a message with no id
#[test]
fn refuses_answers_that_are_not_lists_of_words_and_a_message_with_no_id() {
    assert!(approval_answer_from(&json!({ "approvalId": "ap_1", "answers": { "0": [3] } })).is_none());
    assert!(approval_answer_from(&json!({ "approvalId": "ap_1", "answers": ["yes"] })).is_none());
    assert!(approval_answer_from(&json!({ "answers": { "0": ["x"] } })).is_none());
}

// case: is what the decide handler reads, and Antigravity is asked too
#[test]
fn is_what_the_decide_handler_reads_and_antigravity_is_asked_too() {
    let channel = Channel::new();
    let cascade = FakeCascade::new(true);
    let h = harness(cascade.clone(), None);
    h.service
        .start("Ask me to choose.", None, MissionRoute::default())
        .expect("start");
    crate::invariants::lock(&h.transcript).push(asks_line());
    wait_until(|| crate::invariants::lock(&h.raised).len() == 1);
    let approval_id = crate::invariants::lock(&h.raised)[0].request.approval_id.clone();
    let raw = json!({ "approvalId": approval_id, "answers": { "0": [options()[0].text] } });
    let parsed = approval_answer_from(&raw).expect("parsed answer");
    // Funnel: channel first, then Antigravity's waiting question.
    assert!(channel.decide(parsed.clone()).is_err());
    assert!(h.service.decide(&parsed));
    assert_eq!(
        *crate::invariants::lock(&cascade.answers),
        vec![vec![AntigravityQuestionResponse {
            selected_option_ids: vec!["1".into()],
            write_in: None,
            skipped: false,
        }]]
    );
    h.service.dispose();
}

// case: still reads anything but a recognized decision as a denial
#[test]
fn still_reads_anything_but_a_recognized_decision_as_a_denial() {
    match approval_answer_from(&json!({ "approvalId": "ap_1", "decision": "approve-once" }))
        .expect("approve-once")
    {
        Answer::Decision {
            approval_id,
            decision,
            reason,
        } => {
            assert_eq!(approval_id, "ap_1");
            assert_eq!(decision, Decision::ApproveOnce);
            assert_eq!(reason, None);
        }
        other => panic!("expected Decision, got {other:?}"),
    }
    match approval_answer_from(&json!({ "approvalId": "ap_1", "decision": "yes please" }))
        .expect("unrecognized")
    {
        Answer::Decision {
            approval_id,
            decision,
            ..
        } => {
            assert_eq!(approval_id, "ap_1");
            assert_eq!(decision, Decision::Deny);
        }
        other => panic!("expected Decision, got {other:?}"),
    }
    match approval_answer_from(&json!({ "approvalId": "ap_1" })).expect("missing decision") {
        Answer::Decision {
            approval_id,
            decision,
            ..
        } => {
            assert_eq!(approval_id, "ap_1");
            assert_eq!(decision, Decision::Deny);
        }
        other => panic!("expected Decision, got {other:?}"),
    }
}
