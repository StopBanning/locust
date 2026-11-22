//! Ports `apps/desktop/src/main/antigravity-mission.test.ts`.

use crate::antigravity_host::{
    AgentApi, AntigravityHost, AntigravityHostProbe, AntigravityProbeOptions, CliRunResult,
    NewConversationInput, ParsedServerCommandLine, ProcessEntry, SendMessageInput,
    antigravity_executable_candidates, parse_server_command_line, transcript_path_for,
};
use crate::antigravity_mission::{
    ANTIGRAVITY_ASKING_NOTICE_MS, ANTIGRAVITY_IDLE_TIMEOUT_MS, ANTIGRAVITY_NOT_STARTED_NOTICE_MS,
    ANTIGRAVITY_POLL_MS, ANTIGRAVITY_QUESTION_LOOKUPS, AntigravityMissionOptions,
    AntigravityMissionService, AntigravityStartError, FollowUpInfo, MAX_LIVE_ANTIGRAVITY_MISSIONS,
    MissionLedger, MissionPeerContext, MissionPeerSelf, MissionRoute, MissionUpdate,
    ObserveDiskFn, ObservePatchesFn, PriorMissionRecord, SpendRefusalFn, StatTranscriptFn,
    TranscriptStat, antigravity_start_refusal, antigravity_tier,
};
use crate::disk_observation::WorkspaceSnapshot;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const NOW: &str = "2026-09-03T09:00:00.000Z";
const HOME: &str = "C:\\Users\\dev";
const WORKSPACE: &str = "C:\\work\\pebble";
const CONVERSATION: &str = "03a2fcb4-8fd9-468e-a683-6bf3a5acd077";
const SERVER_CMD: &str = "C:\\Users\\dev\\AppData\\Local\\Programs\\antigravity\\resources\\bin\\language_server.exe --standalone --override_ide_name antigravity --subclient_type hub --override_ide_version 2.11.0 --https_server_port 0 --csrf_token 60843f52-6d41-4d97-9b31-53157a780b5e --app_data_dir antigravity";

fn write_lines() -> Vec<String> {
    vec![
        json!({ "step_index": 0, "source": "USER_EXPLICIT", "type": "USER_INPUT", "status": "DONE", "created_at": NOW, "content": "<USER_REQUEST>make hello.txt</USER_REQUEST>" }).to_string(),
        json!({ "step_index": 1, "source": "SYSTEM", "type": "CHECKPOINT", "status": "DONE", "created_at": NOW, "content": "{{ CHECKPOINT 0 }} the whole conversation so far" }).to_string(),
        json!({
            "step_index": 2,
            "source": "MODEL",
            "type": "PLANNER_RESPONSE",
            "status": "DONE",
            "created_at": NOW,
            "thinking": "private reasoning",
            "tool_calls": [{ "name": "write_to_file", "args": { "TargetFile": "\"c:/work/pebble/hello.txt\"", "CodeContent": "\"hello\\n\"", "Overwrite": "true" } }]
        }).to_string(),
        json!({ "step_index": 3, "source": "MODEL", "type": "GENERIC", "status": "DONE", "created_at": NOW, "content": "Created file file:///c:/work/pebble/hello.txt with requested content." }).to_string(),
        json!({ "step_index": 4, "source": "MODEL", "type": "PLANNER_RESPONSE", "status": "DONE", "created_at": NOW, "content": "DONE" }).to_string(),
    ]
}

fn default_host() -> AntigravityHost {
    AntigravityHost {
        executable_path: "C:\\lang\\language_server.exe".into(),
        version: Some("2.11.0".into()),
        address: "localhost:49839".into(),
        csrf_token: "60843f52-6d41-4d97-9b31-53157a780b5e".into(),
        projects: HashMap::from([(
            "c:/work/pebble".into(),
            "daf0f8ec-bb8e-49e8-a445-954eb0a62d0f".into(),
        )]),
        ports: vec![49839],
    }
}

struct FakeLedger {
    refuse_create: bool,
    refuse_append: bool,
    prior: Mutex<Option<PriorMissionRecord>>,
    created: Mutex<Vec<Value>>,
    appended: Mutex<Vec<(String, Vec<Value>)>>,
}

impl FakeLedger {
    fn new(refuse_create: bool, refuse_append: bool) -> Arc<Self> {
        Arc::new(Self {
            refuse_create,
            refuse_append,
            prior: Mutex::new(None),
            created: Mutex::new(Vec::new()),
            appended: Mutex::new(Vec::new()),
        })
    }
}

impl MissionLedger for FakeLedger {
    fn create_mission(&self, input: Value) -> Result<(), String> {
        if self.refuse_create {
            return Err("no space left on device".into());
        }
        crate::invariants::lock(&self.created).push(input);
        Ok(())
    }

    fn append_events(&self, mission_id: &str, events: &[Value]) -> Result<(), String> {
        if self.refuse_append {
            return Err("no space left on device".into());
        }
        crate::invariants::lock(&self.appended).push((mission_id.to_owned(), events.to_vec()));
        Ok(())
    }

    fn get_mission(&self, _mission_id: &str) -> Option<PriorMissionRecord> {
        crate::invariants::lock(&self.prior).clone()
    }
}

#[derive(Clone, Debug)]
enum ApiCall {
    New(NewConversationInput),
    Send(SendMessageInput),
}

struct FakeAgentApi {
    calls: Arc<Mutex<Vec<ApiCall>>>,
    forbid_new: bool,
}

impl AgentApi for FakeAgentApi {
    fn new_conversation(&self, input: NewConversationInput) -> Result<String, String> {
        if self.forbid_new {
            return Err("must not open a new conversation for a follow-up".into());
        }
        crate::invariants::lock(&self.calls).push(ApiCall::New(input));
        Ok(CONVERSATION.into())
    }

    fn send_message(&self, input: SendMessageInput) -> Result<(), String> {
        crate::invariants::lock(&self.calls).push(ApiCall::Send(input));
        Ok(())
    }
}

#[derive(Default)]
struct HarnessOpts {
    host: Option<Option<AntigravityHost>>,
    lines: Vec<String>,
    stat_transcript: Option<StatTranscriptFn>,
    idle_timeout_ms: Option<u64>,
    asking_notice_ms: Option<u64>,
    not_started_notice_ms: Option<u64>,
    refuse_create: bool,
    refuse_append: bool,
    forbid_new: bool,
    prior: Option<PriorMissionRecord>,
    spend_refusal: Option<SpendRefusalFn>,
    observe_disk: Option<ObserveDiskFn>,
    observe_patches: Option<ObservePatchesFn>,
}

struct Harness {
    service: AntigravityMissionService,
    api_calls: Arc<Mutex<Vec<ApiCall>>>,
    transcript: Arc<Mutex<Vec<String>>>,
    emitted: Arc<Mutex<Vec<Value>>>,
    ledger: Arc<FakeLedger>,
    updates: Arc<Mutex<Vec<MissionUpdate>>>,
    notices: Arc<Mutex<Vec<String>>>,
    reads: Arc<Mutex<Vec<String>>>,
    ended: Arc<Mutex<Vec<String>>>,
}

fn harness(opts: HarnessOpts) -> Harness {
    let ledger = FakeLedger::new(opts.refuse_create, opts.refuse_append);
    if let Some(prior) = opts.prior {
        *crate::invariants::lock(&ledger.prior) = Some(prior);
    }
    let api_calls: Arc<Mutex<Vec<ApiCall>>> = Arc::new(Mutex::new(Vec::new()));
    let calls_ref = api_calls.clone();
    let forbid_new = opts.forbid_new;
    let transcript = Arc::new(Mutex::new(opts.lines));
    let tr = transcript.clone();
    let emitted: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let em = emitted.clone();
    let updates: Arc<Mutex<Vec<MissionUpdate>>> = Arc::new(Mutex::new(Vec::new()));
    let up = updates.clone();
    let notices: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let nt = notices.clone();
    let reads: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let rd = reads.clone();
    let ended: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let en = ended.clone();
    let host_choice = opts.host.unwrap_or_else(|| Some(default_host()));
    let ids = Arc::new(AtomicUsize::new(0));
    let expected_path = transcript_path_for(HOME, CONVERSATION);

    let service = AntigravityMissionService::new(AntigravityMissionOptions {
        workspace_path: WORKSPACE.into(),
        ledger: ledger.clone(),
        probe: Arc::new(move || host_choice.clone()),
        agent_api: Arc::new(move |_| {
            Arc::new(FakeAgentApi {
                calls: calls_ref.clone(),
                forbid_new,
            })
        }),
        cascade_api: None,
        read_transcript: Arc::new(move |path| {
            crate::invariants::lock(&rd).push(path.to_owned());
            if path == expected_path {
                Some(crate::invariants::lock(&tr).join("\n"))
            } else {
                None
            }
        }),
        stat_transcript: opts.stat_transcript,
        emit_event: Arc::new(move |_, _, ev| crate::invariants::lock(&em).push(ev)),
        emit_update: Some(Arc::new(move |u| crate::invariants::lock(&up).push(u))),
        notify: Some(Arc::new(move |n| crate::invariants::lock(&nt).push(n.message))),
        emit_approval: None,
        withdraw_approval: None,
        on_run_ended: Some(Arc::new(move |m| crate::invariants::lock(&en).push(m.to_owned()))),
        spend_refusal: opts.spend_refusal,
        observe_disk: opts.observe_disk,
        observe_patches: opts.observe_patches,
        home: HOME.into(),
        create_id: Arc::new(move || (ids.fetch_add(1, Ordering::Relaxed) + 1).to_string()),
        now_iso: Arc::new(|| NOW.into()),
        poll_ms: 5,
        idle_timeout_ms: opts.idle_timeout_ms.unwrap_or(ANTIGRAVITY_IDLE_TIMEOUT_MS),
        asking_notice_ms: opts.asking_notice_ms.unwrap_or(ANTIGRAVITY_ASKING_NOTICE_MS),
        not_started_notice_ms: opts
            .not_started_notice_ms
            .unwrap_or(ANTIGRAVITY_NOT_STARTED_NOTICE_MS),
        question_lookups: ANTIGRAVITY_QUESTION_LOOKUPS,
    });
    Harness {
        service,
        api_calls,
        transcript,
        emitted,
        ledger,
        updates,
        notices,
        reads,
        ended,
    }
}

fn wait_until(cond: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if cond() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(cond(), "condition not met before timeout");
}

fn settle(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn peer(id: &str, name: &str) -> MissionPeerContext {
    MissionPeerContext {
        self_peer: MissionPeerSelf {
            teammate_id: id.into(),
            name: name.into(),
            role: "Code & Migrations".into(),
        },
    }
}

// case: says the agent may already be working, and not to send it again
#[test]
fn says_the_agent_may_already_be_working_and_not_to_send_it_again() {
    let h = harness(HarnessOpts {
        refuse_create: true,
        ..Default::default()
    });
    let err = h
        .service
        .start("Fix the build.", None, MissionRoute::default())
        .expect_err("create_mission failed");
    let lower = err.message.to_lowercase();
    assert!(lower.contains("already started on this") && lower.contains("do not send it again"));
}

// case: reads the token and version off the server's own command line, and only Antigravity's
#[test]
fn reads_the_token_and_version_off_the_servers_own_command_line_and_only_antigravitys() {
    assert_eq!(
        parse_server_command_line(SERVER_CMD),
        Some(ParsedServerCommandLine {
            csrf_token: "60843f52-6d41-4d97-9b31-53157a780b5e".into(),
            version: Some("2.11.0".into()),
        })
    );
    assert_eq!(
        parse_server_command_line(
            &SERVER_CMD.replace("--override_ide_name antigravity", "--override_ide_name windsurf")
        ),
        None
    );
    assert_eq!(
        parse_server_command_line("language_server.exe --override_ide_name antigravity"),
        None
    );
}

// case: looks under both spellings of the install folder
#[test]
fn looks_under_both_spellings_of_the_install_folder() {
    assert_eq!(
        antigravity_executable_candidates("C:\\lad"),
        vec![
            "C:\\lad\\Programs\\antigravity\\resources\\bin\\language_server.exe",
            "C:\\lad\\Programs\\Antigravity\\resources\\bin\\language_server.exe",
        ]
    );
}

// case: picks the listening port that speaks the RPC, not the one that drops the connection
#[test]
fn picks_the_listening_port_that_speaks_the_rpc_not_the_one_that_drops_the_connection() {
    let asked: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let asked_ref = asked.clone();
    let probe = AntigravityHostProbe::new(AntigravityProbeOptions {
        platform: "win32".into(),
        local_app_data: "C:\\lad".into(),
        home: HOME.into(),
        list_processes: Arc::new(|| {
            vec![ProcessEntry {
                pid: 5344,
                command_line: SERVER_CMD.into(),
            }]
        }),
        listening_ports: Arc::new(|_| vec![49838, 49839]),
        run: Arc::new(move |_, _, env| {
            let addr = env.get("ANTIGRAVITY_LS_ADDRESS").cloned().unwrap_or_default();
            crate::invariants::lock(&asked_ref).push(addr.clone());
            if addr == "localhost:49838" {
                CliRunResult {
                    stdout: r#"{"error":"rpc error: connection error: wsarecv: An established connection was aborted"}"#.into(),
                    stderr: String::new(),
                    code: Some(0),
                }
            } else {
                CliRunResult {
                    stdout: r#"{"error":"conversation not found"}"#.into(),
                    stderr: String::new(),
                    code: Some(0),
                }
            }
        }),
        exists: None,
        read_bytes: None,
    });
    // The executable check is a real filesystem look; nothing is installed at C:\lad.
    let record = probe.discovery_record();
    assert_eq!(record.id, "antigravity");
    assert_eq!(record.availability, "unavailable");
    assert!(crate::invariants::lock(&asked).is_empty());
}

// case: maps any model to a tier, defaulting to flash
#[test]
fn maps_any_model_to_a_tier_defaulting_to_flash() {
    assert_eq!(antigravity_tier(Some("pro")), "pro");
    assert_eq!(antigravity_tier(Some("flash_lite")), "flash_lite");
    assert_eq!(antigravity_tier(Some("account-default")), "flash");
    assert_eq!(antigravity_tier(None), "flash");
}

// case: refuses, recording nothing, when Antigravity is not open
#[test]
fn refuses_recording_nothing_when_antigravity_is_not_open() {
    let h = harness(HarnessOpts {
        host: Some(None),
        ..Default::default()
    });
    let err = h
        .service
        .start("hi", None, MissionRoute::default())
        .expect_err("not open");
    assert!(err.message.contains("not open"));
    assert!(crate::invariants::lock(&h.ledger.created).is_empty());
    assert!(crate::invariants::lock(&h.api_calls).is_empty());
}

// case: refuses, naming the folder, when Antigravity has not opened the workspace
#[test]
fn refuses_naming_the_folder_when_antigravity_has_not_opened_the_workspace() {
    let mut no_projects = default_host();
    no_projects.projects.clear();
    let h = harness(HarnessOpts {
        host: Some(Some(no_projects)),
        ..Default::default()
    });
    let err = h
        .service
        .start("hi", None, MissionRoute::default())
        .expect_err("workspace not open");
    assert!(err.message.contains(&format!("has not opened {WORKSPACE}")));
    assert!(crate::invariants::lock(&h.ledger.created).is_empty());
}

// case: opens the conversation with the project id, records the mission, and follows the transcript to the final answer
#[test]
fn opens_the_conversation_with_the_project_id_records_the_mission_and_follows_the_transcript_to_the_final_answer()
{
    let h = harness(HarnessOpts::default());
    let mission = h
        .service
        .start(
            "make hello.txt",
            None,
            MissionRoute {
                model: Some("flash".into()),
                follow_up_of: None,
            },
        )
        .expect("start");
    assert_eq!(mission.conversation_id, CONVERSATION);
    {
        let calls = crate::invariants::lock(&h.api_calls);
        match &calls[0] {
            ApiCall::New(input) => {
                assert_eq!(input.project_id, "daf0f8ec-bb8e-49e8-a445-954eb0a62d0f");
                assert_eq!(input.model, "flash");
            }
            other => panic!("expected New, got {other:?}"),
        }
    }
    {
        let created = crate::invariants::lock(&h.ledger.created);
        assert_eq!(created[0]["runtime"], "antigravity");
        assert_eq!(created[0]["model"], "flash");
        assert_eq!(created[0]["sandbox"], "workspace-write");
        assert_eq!(created[0]["resolvedRouteId"], "antigravity:hub");
    }
    assert!(h.service.has(&mission.run_id));

    *crate::invariants::lock(&h.transcript) = write_lines()[..4].to_vec();
    wait_until(|| {
        crate::invariants::lock(&h.emitted)
            .iter()
            .any(|ev| ev["type"] == "tool.started" && ev["payload"]["name"] == "write_to_file")
    });
    assert!(h.service.has(&mission.run_id));

    *crate::invariants::lock(&h.transcript) = write_lines();
    wait_until(|| {
        let em = crate::invariants::lock(&h.emitted);
        em.iter().any(|ev| ev["type"] == "message.delta")
            && em.last().is_some_and(|ev| ev["type"] == "run.completed")
    });
    {
        let em = crate::invariants::lock(&h.emitted);
        let last = em.last().unwrap();
        assert!(
            last["payload"]["runtimeThreadId"].is_string() || last["runtimeThreadId"].is_string()
        );
    }
    assert!(!h.service.has(&mission.run_id));
    let persisted = serde_json::to_string(&*crate::invariants::lock(&h.ledger.appended)).unwrap();
    assert!(!persisted.contains("private reasoning"));
    assert!(!persisted.contains("the whole conversation so far"));
}

// case: a follow-up sends into the same conversation and reads only the lines after the earlier turns
#[test]
fn a_follow_up_sends_into_the_same_conversation_and_reads_only_the_lines_after_the_earlier_turns()
{
    let h = harness(HarnessOpts {
        lines: write_lines(),
        forbid_new: true,
        prior: Some(PriorMissionRecord {
            mission_id: "mission_prior".into(),
            runtime: "antigravity".into(),
            runtime_thread_id: Some(CONVERSATION.into()),
        }),
        ..Default::default()
    });
    let mission = h
        .service
        .start(
            "what did you reply?",
            None,
            MissionRoute {
                model: None,
                follow_up_of: Some("mission_prior".into()),
            },
        )
        .expect("follow-up");
    assert_eq!(
        mission.follows_up,
        Some(FollowUpInfo {
            mission_id: "mission_prior".into(),
            runtime_thread_id: CONVERSATION.into(),
        })
    );
    match &crate::invariants::lock(&h.api_calls)[0] {
        ApiCall::Send(input) => assert_eq!(input.conversation_id, CONVERSATION),
        other => panic!("expected Send, got {other:?}"),
    }
    {
        let created = crate::invariants::lock(&h.ledger.created);
        assert_eq!(created[0]["continuesFrom"]["missionId"], "mission_prior");
        assert_eq!(created[0]["continuesFrom"]["reason"], "follow-up");
        assert_eq!(created[0]["continuesFrom"]["runtimeThreadId"], CONVERSATION);
    }
    settle(50);
    assert!(
        !crate::invariants::lock(&h.emitted)
            .iter()
            .any(|ev| ev["type"] == "tool.started")
    );
    {
        let mut tr = crate::invariants::lock(&h.transcript);
        tr.push(json!({ "step_index": 5, "source": "SYSTEM", "type": "SYSTEM_MESSAGE", "status": "DONE", "created_at": NOW, "content": "<SYSTEM_MESSAGE>what did you reply?</SYSTEM_MESSAGE>" }).to_string());
        tr.push(json!({ "step_index": 6, "source": "MODEL", "type": "PLANNER_RESPONSE", "status": "DONE", "created_at": NOW, "content": "DONE" }).to_string());
    }
    wait_until(|| {
        crate::invariants::lock(&h.emitted)
            .last()
            .is_some_and(|ev| ev["type"] == "run.completed")
    });
    assert!(!crate::invariants::lock(&h.ledger.appended).is_empty());
}

// case: says the agent may be asking, long before the timeout and without ending the run
#[test]
fn says_the_agent_may_be_asking_long_before_the_timeout_and_without_ending_the_run() {
    let h = harness(HarnessOpts {
        lines: write_lines()[..3].to_vec(),
        asking_notice_ms: Some(15),
        idle_timeout_ms: Some(100_000),
        ..Default::default()
    });
    let mission = h
        .service
        .start("hi", None, MissionRoute::default())
        .expect("start");
    wait_until(|| {
        let said = crate::invariants::lock(&h.notices).join(" | ");
        said.contains("write_to_file") && said.to_lowercase().contains("its own window")
    });
    assert!(h.service.has(&mission.run_id));
    assert!(
        !crate::invariants::lock(&h.emitted)
            .iter()
            .any(|ev| ev["type"] == "run.failed")
    );
    h.service.dispose();
}

// case: says a turn Antigravity took has not started, once, without ending the run
#[test]
fn says_a_turn_antigravity_took_has_not_started_once_without_ending_the_run() {
    let h = harness(HarnessOpts {
        lines: Vec::new(),
        not_started_notice_ms: Some(15),
        idle_timeout_ms: Some(100_000),
        ..Default::default()
    });
    let mission = h
        .service
        .start("hi", None, MissionRoute::default())
        .expect("start");
    wait_until(|| {
        let said = crate::invariants::lock(&h.notices).join(" | ").to_lowercase();
        said.contains("has not started it") && said.contains("open it again")
    });
    settle(40);
    assert_eq!(crate::invariants::lock(&h.notices).len(), 1);
    assert!(h.service.has(&mission.run_id));
    assert!(
        !crate::invariants::lock(&h.emitted)
            .iter()
            .any(|ev| ev["type"] == "run.failed")
    );
    h.service.dispose();
}

// case: says nothing of the kind once the turn has written a line
#[test]
fn says_nothing_of_the_kind_once_the_turn_has_written_a_line() {
    let h = harness(HarnessOpts {
        lines: write_lines()[..1].to_vec(),
        not_started_notice_ms: Some(15),
        asking_notice_ms: Some(100_000),
        idle_timeout_ms: Some(400_000),
        ..Default::default()
    });
    h.service
        .start("hi", None, MissionRoute::default())
        .expect("start");
    settle(60);
    assert!(
        !crate::invariants::lock(&h.notices)
            .join(" | ")
            .to_lowercase()
            .contains("has not started")
    );
    h.service.dispose();
}

// case: says it once, not on every tick
#[test]
fn says_it_once_not_on_every_tick() {
    let h = harness(HarnessOpts {
        lines: write_lines()[..3].to_vec(),
        asking_notice_ms: Some(15),
        idle_timeout_ms: Some(100_000),
        ..Default::default()
    });
    h.service
        .start("hi", None, MissionRoute::default())
        .expect("start");
    wait_until(|| crate::invariants::lock(&h.notices).len() == 1);
    settle(40);
    assert_eq!(crate::invariants::lock(&h.notices).len(), 1);
    h.service.dispose();
}

// case: never lets the notice sit at or past the ending, where it would be dead code
#[test]
fn never_lets_the_notice_sit_at_or_past_the_ending_where_it_would_be_dead_code() {
    let h = harness(HarnessOpts {
        lines: write_lines()[..3].to_vec(),
        idle_timeout_ms: Some(300),
        ..Default::default()
    });
    h.service
        .start("hi", None, MissionRoute::default())
        .expect("start");
    wait_until(|| {
        let notices = crate::invariants::lock(&h.notices);
        !notices.is_empty() && notices[0].contains("write_to_file")
    });
    h.service.dispose();
}

// case: says nothing while the agent is still writing
#[test]
fn says_nothing_while_the_agent_is_still_writing() {
    let h = harness(HarnessOpts {
        lines: write_lines(),
        asking_notice_ms: Some(15),
        idle_timeout_ms: Some(100_000),
        ..Default::default()
    });
    h.service
        .start("hi", None, MissionRoute::default())
        .expect("start");
    wait_until(|| {
        crate::invariants::lock(&h.emitted)
            .last()
            .is_some_and(|ev| ev["type"] == "run.completed")
    });
    settle(30);
    assert!(crate::invariants::lock(&h.notices).is_empty());
}

// case: gives up when the agent writes nothing for too long, as a failure and not a completion
#[test]
fn gives_up_when_the_agent_writes_nothing_for_too_long_as_a_failure_and_not_a_completion() {
    let h = harness(HarnessOpts {
        lines: Vec::new(),
        idle_timeout_ms: Some(20),
        ..Default::default()
    });
    let mission = h
        .service
        .start("hi", None, MissionRoute::default())
        .expect("start");
    wait_until(|| {
        !h.service.has(&mission.run_id)
            && crate::invariants::lock(&h.emitted)
                .last()
                .is_some_and(|ev| ev["type"] == "run.failed")
    });
}

// case: stopping stops the watch and says the agent may still be working
#[test]
fn stopping_stops_the_watch_and_says_the_agent_may_still_be_working() {
    let h = harness(HarnessOpts {
        lines: write_lines()[..3].to_vec(),
        ..Default::default()
    });
    let mission = h
        .service
        .start("hi", None, MissionRoute::default())
        .expect("start");
    assert!(h.service.cancel(&mission.run_id));
    wait_until(|| {
        !h.service.has(&mission.run_id)
            && crate::invariants::lock(&h.emitted)
                .last()
                .is_some_and(|ev| ev["type"] == "run.cancelled")
    });
    assert!(!h.service.cancel(&mission.run_id));
}

// case: one live mission per teammate
#[test]
fn one_live_mission_per_teammate() {
    let h = harness(HarnessOpts::default());
    let wren = peer("tm_w", "Wren");
    h.service
        .start("one", Some(wren.clone()), MissionRoute::default())
        .expect("first");
    let err = h
        .service
        .start("two", Some(wren), MissionRoute::default())
        .expect_err("second refused");
    assert!(err.message.contains("already has a mission running"));
    h.service.dispose();
}

// case: says a busy teammate is busy, so a relayed reply is held and not dropped (A2.19)
#[test]
fn says_a_busy_teammate_is_busy_so_a_relayed_reply_is_held_and_not_dropped() {
    let h = harness(HarnessOpts::default());
    let wren = peer("tm_w", "Wren");
    h.service
        .start("one", Some(wren.clone()), MissionRoute::default())
        .expect("first");
    let refused = h
        .service
        .start("two", Some(wren), MissionRoute::default())
        .expect_err("busy");
    assert_eq!(
        antigravity_start_refusal(&refused),
        json!({
            "ok": false,
            "error": {
                "code": "RUN_ALREADY_ACTIVE",
                "message": "Wren already has a mission running. Wait for it to finish or stop it first."
            }
        })
    );
    assert_eq!(
        antigravity_start_refusal(&AntigravityStartError::new("Antigravity is not open."))["error"]
            ["code"],
        "RUNTIME_START_FAILED"
    );
    h.service.dispose();
}

// case: says every slot being taken is busy too, and which kind
#[test]
fn says_every_slot_being_taken_is_busy_too_and_which_kind() {
    let h = harness(HarnessOpts::default());
    for idx in 0..MAX_LIVE_ANTIGRAVITY_MISSIONS {
        h.service
            .start(
                &format!("run {idx}"),
                Some(peer(&format!("tm_{idx}"), &format!("Mate{idx}"))),
                MissionRoute::default(),
            )
            .expect("slot");
    }
    let refused = h
        .service
        .start("one more", Some(peer("tm_extra", "Extra")), MissionRoute::default())
        .expect_err("pool full");
    let body = antigravity_start_refusal(&refused);
    assert_eq!(body["error"]["code"], "RUN_ALREADY_ACTIVE");
    assert_eq!(body["error"]["busy"], "pool");
    h.service.dispose();
}

// case: refuses a teammate at their monthly limit before Antigravity is even asked, and it is not 'busy'
#[test]
fn refuses_a_teammate_at_their_monthly_limit_before_antigravity_is_even_asked_and_it_is_not_busy() {
    let said = "Wren has reached this month's limit: $5.02 of $5.00. Raise the limit by editing Wren, or it starts again on October 1.";
    let said_owned = said.to_owned();
    let h = harness(HarnessOpts {
        spend_refusal: Some(Arc::new(move |tid| {
            (tid == "tm_w").then(|| said_owned.clone())
        })),
        ..Default::default()
    });
    let refused = h
        .service
        .start("one", Some(peer("tm_w", "Wren")), MissionRoute::default())
        .expect_err("limit");
    assert_eq!(
        antigravity_start_refusal(&refused),
        json!({
            "ok": false,
            "error": {
                "code": "SPEND_LIMIT_REACHED",
                "message": said
            }
        })
    );
    assert!(crate::invariants::lock(&h.api_calls).is_empty());
    assert!(crate::invariants::lock(&h.ledger.created).is_empty());
}

// case: ends the run and says so, instead of polling on with nobody recording
#[test]
fn ends_the_run_and_says_so_instead_of_polling_on_with_nobody_recording() {
    let h = harness(HarnessOpts {
        lines: write_lines(),
        refuse_append: true,
        ..Default::default()
    });
    h.service
        .start("Write hello.txt", None, MissionRoute::default())
        .expect("start");
    wait_until(|| {
        crate::invariants::lock(&h.updates)
            .iter()
            .any(|u| u.kind == "persistence-error")
    });
}

// case: says the run ended, as every other end does
#[test]
fn says_the_run_ended_as_every_other_end_does() {
    let h = harness(HarnessOpts {
        lines: write_lines(),
        refuse_append: true,
        ..Default::default()
    });
    let mission = h
        .service
        .start("Write hello.txt", None, MissionRoute::default())
        .expect("start");
    wait_until(|| *crate::invariants::lock(&h.ended) == vec![mission.mission_id.clone()]);
}

// case: does NOT advance past events it failed to write
#[test]
fn does_not_advance_past_events_it_failed_to_write() {
    let h = harness(HarnessOpts {
        lines: write_lines(),
        refuse_append: true,
        ..Default::default()
    });
    h.service
        .start("Write hello.txt", None, MissionRoute::default())
        .expect("start");
    wait_until(|| {
        crate::invariants::lock(&h.updates)
            .iter()
            .any(|u| u.kind == "persistence-error")
    });
    assert!(crate::invariants::lock(&h.ledger.appended).is_empty());
    assert!(crate::invariants::lock(&h.emitted).is_empty());
}

// case: leaves a healthy run completely alone
#[test]
fn leaves_a_healthy_run_completely_alone() {
    let h = harness(HarnessOpts {
        lines: write_lines(),
        ..Default::default()
    });
    h.service
        .start("Write hello.txt", None, MissionRoute::default())
        .expect("start");
    wait_until(|| !crate::invariants::lock(&h.ledger.appended).is_empty());
    assert!(
        !crate::invariants::lock(&h.updates)
            .iter()
            .any(|u| u.kind == "persistence-error")
    );
}

fn disk_looks() -> (ObserveDiskFn, Arc<Mutex<Vec<String>>>) {
    let looked: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let lk = looked.clone();
    let observe: ObserveDiskFn = Arc::new(move |folder| {
        let mut guard = crate::invariants::lock(&lk);
        guard.push(folder.to_owned());
        if guard.len() == 1 {
            Some(WorkspaceSnapshot::default())
        } else {
            let mut entries = BTreeMap::new();
            entries.insert("hello.txt".into(), "??".into());
            Some(WorkspaceSnapshot {
                entries,
                ..Default::default()
            })
        }
    });
    (observe, looked)
}

fn hello_patch() -> Value {
    json!({
        "text": "--- /dev/null\n+++ b/hello.txt\n@@ -0,0 +1 @@\n+hello\n",
        "added": 1,
        "removed": 0,
        "truncated": false
    })
}

// case: reads the change behind a file the agent named and puts it after its own events, in sequence
#[test]
fn reads_the_change_behind_a_file_the_agent_named_and_puts_it_after_its_own_events_in_sequence() {
    let (observe_disk, looked) = disk_looks();
    let h = harness(HarnessOpts {
        observe_disk: Some(observe_disk),
        observe_patches: Some(Arc::new(|_, _, _, _| {
            BTreeMap::from([("hello.txt".into(), hello_patch())])
        })),
        ..Default::default()
    });
    h.service
        .start(
            "make hello.txt",
            None,
            MissionRoute {
                model: Some("flash".into()),
                follow_up_of: None,
            },
        )
        .expect("start");
    assert_eq!(*crate::invariants::lock(&looked), vec![WORKSPACE]);
    assert!(matches!(crate::invariants::lock(&h.api_calls)[0], ApiCall::New(_)));

    *crate::invariants::lock(&h.transcript) = write_lines();
    wait_until(|| {
        let all: Vec<Value> = crate::invariants::lock(&h.ledger.appended)
            .iter()
            .flat_map(|(_, evs)| evs.clone())
            .collect();
        crate::invariants::lock(&looked).len() == 2
            && all.iter().any(|ev| ev["payload"]["toolKind"] == "observed_edit")
    });
    let all: Vec<Value> = crate::invariants::lock(&h.ledger.appended)
        .iter()
        .flat_map(|(_, evs)| evs.clone())
        .collect();
    let completed = all
        .iter()
        .position(|ev| ev["type"] == "run.completed")
        .expect("run.completed");
    assert!(completed > 0);
    let observed = &all[completed + 1..];
    let obs_types: Vec<&str> = observed
        .iter()
        .map(|ev| ev["type"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(obs_types, vec!["tool.started", "tool.completed"]);
    assert_eq!(observed[0]["payload"]["toolKind"], "observed_edit");
    assert_eq!(observed[0]["payload"]["command"], "hello.txt");
    assert_eq!(
        observed[0]["payload"]["status"],
        "reported by the runtime, read from disk"
    );
    assert_eq!(observed[1]["payload"]["toolKind"], "observed_edit");
    assert_eq!(observed[1]["payload"]["phase"], "completed");
    assert_eq!(observed[1]["payload"]["patch"], hello_patch());
    let comp_seq = all[completed]["sequence"].as_u64().unwrap();
    assert_eq!(observed[0]["sequence"].as_u64().unwrap(), comp_seq + 1);
    assert_eq!(observed[1]["sequence"].as_u64().unwrap(), comp_seq + 2);
    let em = crate::invariants::lock(&h.emitted);
    let last_two: Vec<&str> = em[em.len() - 2..]
        .iter()
        .map(|ev| ev["type"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(last_two, vec!["tool.started", "tool.completed"]);
}

// case: says nothing when nothing changed, and nothing when it could not look first
#[test]
fn says_nothing_when_nothing_changed_and_nothing_when_it_could_not_look_first() {
    let looks_made = Arc::new(AtomicUsize::new(0));
    let lm = looks_made.clone();
    let quiet = harness(HarnessOpts {
        observe_disk: Some(Arc::new(move |_| {
            lm.fetch_add(1, Ordering::SeqCst);
            Some(WorkspaceSnapshot::default())
        })),
        observe_patches: Some(Arc::new(|_, _, _, _| panic!("nothing to read"))),
        ..Default::default()
    });
    quiet
        .service
        .start("make hello.txt", None, MissionRoute { model: Some("flash".into()), follow_up_of: None })
        .expect("start");
    *crate::invariants::lock(&quiet.transcript) = write_lines();
    wait_until(|| {
        looks_made.load(Ordering::SeqCst) == 2
            && crate::invariants::lock(&quiet.ledger.appended)
                .iter()
                .flat_map(|(_, evs)| evs.iter())
                .last()
                .is_some_and(|ev| ev["type"] == "run.completed")
    });

    let blind_looks = Arc::new(AtomicUsize::new(0));
    let bl = blind_looks.clone();
    let blind = harness(HarnessOpts {
        observe_disk: Some(Arc::new(move |_| {
            bl.fetch_add(1, Ordering::SeqCst);
            None
        })),
        ..Default::default()
    });
    blind
        .service
        .start("make hello.txt", None, MissionRoute { model: Some("flash".into()), follow_up_of: None })
        .expect("start");
    *crate::invariants::lock(&blind.transcript) = write_lines();
    wait_until(|| {
        blind_looks.load(Ordering::SeqCst) == 1
            && crate::invariants::lock(&blind.ledger.appended)
                .iter()
                .flat_map(|(_, evs)| evs.iter())
                .last()
                .is_some_and(|ev| ev["type"] == "run.completed")
    });
}

// case: from a folder two Antigravity runs shared, says the reading names nobody rather than counting it
#[test]
fn from_a_folder_two_antigravity_runs_shared_says_the_reading_names_nobody_rather_than_counting_it()
{
    let (observe_disk, _) = disk_looks();
    let h = harness(HarnessOpts {
        observe_disk: Some(observe_disk),
        observe_patches: Some(Arc::new(|_, _, _, _| {
            BTreeMap::from([("hello.txt".into(), hello_patch())])
        })),
        ..Default::default()
    });
    h.service
        .start(
            "make hello.txt",
            Some(peer("tm_wren", "Wren")),
            MissionRoute { model: Some("flash".into()), follow_up_of: None },
        )
        .expect("wren");
    h.service
        .start(
            "make hello.txt",
            Some(peer("tm_booty", "Booty")),
            MissionRoute { model: Some("flash".into()), follow_up_of: None },
        )
        .expect("booty");
    *crate::invariants::lock(&h.transcript) = write_lines();
    wait_until(|| {
        crate::invariants::lock(&h.ledger.appended)
            .iter()
            .flat_map(|(_, evs)| evs.iter())
            .any(|ev| ev["type"] == "adapter.diagnostic" && ev["payload"]["code"] == "host.shared_workspace")
    });
    let has_observed_edit = crate::invariants::lock(&h.ledger.appended)
        .iter()
        .flat_map(|(_, evs)| evs.iter())
        .any(|ev| ev["payload"]["toolKind"] == "observed_edit");
    assert!(!has_observed_edit);
}

// case: polls four times a second by default
#[test]
fn polls_four_times_a_second_by_default() {
    assert_eq!(ANTIGRAVITY_POLL_MS, 250);
}

// case: reads the transcript only when its size or time has moved
#[test]
fn reads_the_transcript_only_when_its_size_or_time_has_moved() {
    let stat = Arc::new(Mutex::new(TranscriptStat {
        size: 10,
        mtime_ms: 1,
    }));
    let st = stat.clone();
    let h = harness(HarnessOpts {
        stat_transcript: Some(Arc::new(move |_| Some(crate::invariants::lock(&st).clone()))),
        ..Default::default()
    });
    h.service
        .start("Fix the build.", None, MissionRoute::default())
        .expect("start");
    wait_until(|| !crate::invariants::lock(&h.reads).is_empty());
    let after_first = crate::invariants::lock(&h.reads).len();
    settle(50);
    assert_eq!(crate::invariants::lock(&h.reads).len(), after_first);
    *crate::invariants::lock(&stat) = TranscriptStat {
        size: 12,
        mtime_ms: 2,
    };
    wait_until(|| crate::invariants::lock(&h.reads).len() == after_first + 1);
    settle(30);
    assert_eq!(crate::invariants::lock(&h.reads).len(), after_first + 1);
    h.service.dispose();
}

// case: reads every time when the file cannot be stated, as before
#[test]
fn reads_every_time_when_the_file_cannot_be_stated_as_before() {
    let h = harness(HarnessOpts {
        stat_transcript: Some(Arc::new(|_| None)),
        ..Default::default()
    });
    h.service
        .start("Fix the build.", None, MissionRoute::default())
        .expect("start");
    wait_until(|| crate::invariants::lock(&h.reads).len() > 3);
    h.service.dispose();
}
