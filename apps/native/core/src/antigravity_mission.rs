//! Antigravity IDE mission service (`apps/desktop/src/main/antigravity-mission.ts`).
//!
//! Drives a conversation inside an open Antigravity IDE via its language server RPC
//! (`AgentApi` + `CascadeApi`), polls its JSONL transcript (statting before reading),
//! fills `<truncated N bytes>` gaps from `GetCascadeTrajectorySteps`, raises and answers
//! `ask_question` cards, reports silence / not-started / persistence-error notices, and
//! observes disk edits after the run ends.

use crate::antigravity_cascade::{
    AntigravityPendingQuestion, AntigravityQuestionResponse, CascadeClient, CascadePost,
    plain_question, responses_from_answers,
};
use crate::antigravity_host::{
    AgentApi, AntigravityHost, NewConversationInput, SendMessageInput, project_id_for,
    transcript_path_for,
};
use crate::approval::{Answer, Kind, Question, QuestionOption, Request};
use crate::disk_observation::{
    WorkspaceSnapshot, changed_paths, observed_edit_events, shared_tree_notice, unreported_paths,
};
use crate::runtime::antigravity_events::{
    TranscriptNormalizer, antigravity_message_step, has_antigravity_gap, with_restored_gaps,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const ANTIGRAVITY_TIERS: [&str; 3] = ["flash", "pro", "flash_lite"];

/// Matches `shared/live-missions.ts:MAX_LIVE_MISSIONS`.
pub const MAX_LIVE_ANTIGRAVITY_MISSIONS: usize = 8;

/// No new transcript line for this long means the agent is not coming back.
pub const ANTIGRAVITY_IDLE_TIMEOUT_MS: u64 = 10 * 60_000;

/// Live runs are polled four times a second; statting first skips unmoved files.
pub const ANTIGRAVITY_POLL_MS: u64 = 250;

/// How long an open tool may be silent before the person is told it might be a question.
pub const ANTIGRAVITY_ASKING_NOTICE_MS: u64 = 90_000;

/// How long a turn Antigravity took may show nothing at all before the person is told.
pub const ANTIGRAVITY_NOT_STARTED_NOTICE_MS: u64 = 60_000;

/// How many polls for the waiting `ask_question` step before raising the card without it.
pub const ANTIGRAVITY_QUESTION_LOOKUPS: usize = 8;

const NOBODY: &str = "";

/// The tier a chosen model maps to. `account-default` and anything unknown is `flash`.
pub fn antigravity_tier(model: Option<&str>) -> &'static str {
    match model {
        Some("pro") => "pro",
        Some("flash_lite") => "flash_lite",
        _ => "flash",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusyKind {
    Teammate,
    Pool,
    Limit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AntigravityStartError {
    pub message: String,
    pub busy: Option<BusyKind>,
}

impl AntigravityStartError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            busy: None,
        }
    }

    pub fn busy(message: impl Into<String>, kind: BusyKind) -> Self {
        Self {
            message: message.into(),
            busy: Some(kind),
        }
    }
}

impl std::fmt::Display for AntigravityStartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AntigravityStartError {}

/// A failed Antigravity start, as the relay reads a start (`antigravityStartRefusal`).
pub fn antigravity_start_refusal(error: &AntigravityStartError) -> Value {
    match error.busy {
        Some(BusyKind::Limit) => json!({
            "ok": false,
            "error": {
                "code": "SPEND_LIMIT_REACHED",
                "message": error.message,
            }
        }),
        Some(BusyKind::Pool) => json!({
            "ok": false,
            "error": {
                "code": "RUN_ALREADY_ACTIVE",
                "message": error.message,
                "busy": "pool",
            }
        }),
        Some(BusyKind::Teammate) => json!({
            "ok": false,
            "error": {
                "code": "RUN_ALREADY_ACTIVE",
                "message": error.message,
            }
        }),
        None => json!({
            "ok": false,
            "error": {
                "code": "RUNTIME_START_FAILED",
                "message": error.message,
            }
        }),
    }
}

pub trait CascadeApi: Send + Sync {
    fn pending_question(
        &self,
        conversation_id: &str,
        from_step: usize,
    ) -> Result<Option<AntigravityPendingQuestion>, String>;
    fn answer_question(
        &self,
        conversation_id: &str,
        pending: &AntigravityPendingQuestion,
        responses: &[AntigravityQuestionResponse],
    ) -> Result<(), String>;
    fn planner_texts(
        &self,
        conversation_id: &str,
        from_step: usize,
    ) -> Result<Vec<String>, String>;
}

impl<P: CascadePost> CascadeApi for CascadeClient<P> {
    fn pending_question(
        &self,
        conversation_id: &str,
        from_step: usize,
    ) -> Result<Option<AntigravityPendingQuestion>, String> {
        CascadeClient::pending_question(self, conversation_id, from_step)
    }

    fn answer_question(
        &self,
        conversation_id: &str,
        pending: &AntigravityPendingQuestion,
        responses: &[AntigravityQuestionResponse],
    ) -> Result<(), String> {
        CascadeClient::answer_question(self, conversation_id, pending, responses)
    }

    fn planner_texts(
        &self,
        conversation_id: &str,
        from_step: usize,
    ) -> Result<Vec<String>, String> {
        CascadeClient::planner_texts(self, conversation_id, from_step)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranscriptStat {
    pub size: u64,
    pub mtime_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionPeerSelf {
    pub teammate_id: String,
    pub name: String,
    pub role: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionPeerContext {
    pub self_peer: MissionPeerSelf,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MissionRoute {
    pub model: Option<String>,
    pub follow_up_of: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowUpInfo {
    pub mission_id: String,
    pub runtime_thread_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AntigravityMission {
    pub run_id: String,
    pub mission_id: String,
    pub conversation_id: String,
    pub model: String,
    pub cli_version: Option<String>,
    pub follows_up: Option<FollowUpInfo>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PriorMissionRecord {
    pub mission_id: String,
    pub runtime: String,
    pub runtime_thread_id: Option<String>,
}

pub trait MissionLedger: Send + Sync {
    fn create_mission(&self, input: Value) -> Result<(), String>;
    fn append_events(&self, mission_id: &str, events: &[Value]) -> Result<(), String>;
    fn get_mission(&self, _mission_id: &str) -> Option<PriorMissionRecord> {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionUpdate {
    pub kind: String,
    pub run_id: String,
    pub mission_id: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionNotice {
    pub run_id: String,
    pub mission_id: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct RaisedApproval {
    pub run_id: String,
    pub request: Request,
}

pub type ProbeFn = Arc<dyn Fn() -> Option<AntigravityHost> + Send + Sync>;
pub type AgentApiFactory = Arc<dyn Fn(&AntigravityHost) -> Arc<dyn AgentApi> + Send + Sync>;
pub type CascadeApiFactory = Arc<dyn Fn(&AntigravityHost) -> Arc<dyn CascadeApi> + Send + Sync>;
pub type ReadTranscriptFn = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;
pub type StatTranscriptFn = Arc<dyn Fn(&str) -> Option<TranscriptStat> + Send + Sync>;
pub type EmitEventFn = Arc<dyn Fn(&str, &str, Value) + Send + Sync>;
pub type EmitUpdateFn = Arc<dyn Fn(MissionUpdate) + Send + Sync>;
pub type NotifyFn = Arc<dyn Fn(MissionNotice) + Send + Sync>;
pub type EmitApprovalFn = Arc<dyn Fn(RaisedApproval) + Send + Sync>;
pub type WithdrawApprovalFn = Arc<dyn Fn(&str) + Send + Sync>;
pub type OnRunEndedFn = Arc<dyn Fn(&str) + Send + Sync>;
pub type SpendRefusalFn = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;
pub type ObserveDiskFn = Arc<dyn Fn(&str) -> Option<WorkspaceSnapshot> + Send + Sync>;
pub type ObservePatchesFn = Arc<
    dyn Fn(&str, &WorkspaceSnapshot, &[String], Option<&WorkspaceSnapshot>) -> BTreeMap<String, Value>
        + Send
        + Sync,
>;
pub type CreateIdFn = Arc<dyn Fn() -> String + Send + Sync>;
pub type NowIsoFn = Arc<dyn Fn() -> String + Send + Sync>;

pub struct AntigravityMissionOptions {
    pub workspace_path: String,
    pub ledger: Arc<dyn MissionLedger>,
    pub probe: ProbeFn,
    pub agent_api: AgentApiFactory,
    pub cascade_api: Option<CascadeApiFactory>,
    pub read_transcript: ReadTranscriptFn,
    pub stat_transcript: Option<StatTranscriptFn>,
    pub emit_event: EmitEventFn,
    pub emit_update: Option<EmitUpdateFn>,
    pub notify: Option<NotifyFn>,
    pub emit_approval: Option<EmitApprovalFn>,
    pub withdraw_approval: Option<WithdrawApprovalFn>,
    pub on_run_ended: Option<OnRunEndedFn>,
    pub spend_refusal: Option<SpendRefusalFn>,
    pub observe_disk: Option<ObserveDiskFn>,
    pub observe_patches: Option<ObservePatchesFn>,
    pub home: String,
    pub create_id: CreateIdFn,
    pub now_iso: NowIsoFn,
    pub poll_ms: u64,
    pub idle_timeout_ms: u64,
    pub asking_notice_ms: u64,
    pub not_started_notice_ms: u64,
    pub question_lookups: usize,
}

#[derive(Clone, Debug)]
struct ToolQuestionInfo {
    question: String,
    options: Vec<String>,
    asked_at_step: usize,
}

struct Asking {
    item_id: String,
    question: ToolQuestionInfo,
    approval_id: String,
    pending: Option<AntigravityPendingQuestion>,
    lookups: usize,
    raised: bool,
    answered: bool,
}

struct LiveRun {
    run_id: String,
    mission_id: String,
    conversation_id: String,
    peer: Option<MissionPeerContext>,
    normalizer: TranscriptNormalizer,
    transcript_path: String,
    fed: usize,
    fed_at_start: usize,
    said_not_started: bool,
    last_progress_at: Instant,
    said_it_might_be_asking: bool,
    cascade: Option<Arc<dyn CascadeApi>>,
    asking: Option<Asking>,
    polling: AtomicBool,
    stop_flag: Arc<AtomicBool>,
    seen: Option<TranscriptStat>,
    ended: bool,
    disk_before: Option<WorkspaceSnapshot>,
    persisted: Vec<Value>,
    last_sequence: u64,
    shared_tree: bool,
}

struct ServiceInner {
    options: AntigravityMissionOptions,
    asking_notice_ms: u64,
    not_started_notice_ms: u64,
    question_lookups: usize,
    runs: Mutex<HashMap<String, Arc<Mutex<LiveRun>>>>,
    starting: Mutex<HashSet<String>>,
    disposed: AtomicBool,
}

pub struct AntigravityMissionService {
    inner: Arc<ServiceInner>,
}

fn owner_key_of(peer: Option<&MissionPeerContext>) -> String {
    peer.map(|p| p.self_peer.teammate_id.clone())
        .unwrap_or_else(|| NOBODY.to_owned())
}

fn minutes_rounded(silent_ms: u64) -> (u64, u64) {
    let raw = ((silent_ms as f64) / 60_000.0).round() as u64;
    (raw.max(1), raw)
}

fn question_card(
    workspace_path: &str,
    now_iso: &str,
    run_id: &str,
    mission_id: &str,
    asking: &Asking,
) -> RaisedApproval {
    let (questions, summary, skippable, answer_in) = match &asking.pending {
        None => (
            vec![Question {
                id: "0".into(),
                header: None,
                question: plain_question(&asking.question.question),
                options: asking
                    .question
                    .options
                    .iter()
                    .map(|label| QuestionOption {
                        label: label.clone(),
                        description: None,
                    })
                    .collect(),
                is_other: false,
                is_secret: false,
            }],
            "Antigravity is asking you something".to_owned(),
            false,
            Some("Antigravity".to_owned()),
        ),
        Some(pending) => (
            pending
                .questions
                .iter()
                .enumerate()
                .map(|(idx, q)| Question {
                    id: idx.to_string(),
                    header: None,
                    question: plain_question(&q.question),
                    options: q
                        .options
                        .iter()
                        .map(|opt| QuestionOption {
                            label: opt.text.clone(),
                            description: None,
                        })
                        .collect(),
                    is_other: true,
                    is_secret: false,
                })
                .collect(),
            pending
                .action
                .clone()
                .unwrap_or_else(|| "Antigravity is asking you something".to_owned()),
            true,
            None,
        ),
    };
    RaisedApproval {
        run_id: run_id.to_owned(),
        request: Request {
            approval_id: asking.approval_id.clone(),
            mission_id: mission_id.to_owned(),
            runtime: "antigravity".into(),
            kind: Kind::Question,
            summary,
            detail: String::new(),
            cwd: Some(workspace_path.to_owned()),
            requested_at: now_iso.to_owned(),
            always_covers: None,
            always_key: None,
            data_sent_says: None,
            reversible_says: None,
            patch: None,
            claude_input: None,
            skippable,
            answer_in,
            questions,
        },
    }
}

fn wrap_events(
    run_id: &str,
    mission_id: &str,
    at: &str,
    start_seq: u64,
    events: Vec<crate::runtime::Event>,
) -> Vec<Value> {
    let mut seq = start_seq;
    events
        .into_iter()
        .map(|ev| {
            let mut obj = json!({
                "runId": run_id,
                "missionId": mission_id,
                "occurredAt": at,
                "sourceAdapter": "antigravity",
                "sequence": seq,
                "type": ev.kind,
                "payload": ev.payload,
            });
            if let Some(thread) = ev.thread {
                obj["runtimeThreadId"] = json!(thread);
            }
            seq += 1;
            obj
        })
        .collect()
}

fn persist_and_emit(
    inner: &ServiceInner,
    run_arc: &Arc<Mutex<LiveRun>>,
    events: Vec<Value>,
) -> Result<(), String> {
    if events.is_empty() {
        return Ok(());
    }
    let (run_id, mission_id) = {
        let run = crate::invariants::lock(run_arc);
        (run.run_id.clone(), run.mission_id.clone())
    };
    // Persist-before-emit: a ledger write failure aborts before anything is emitted or cursor advances.
    inner.options.ledger.append_events(&mission_id, &events)?;
    {
        let mut run = crate::invariants::lock(run_arc);
        for ev in &events {
            if let Some(seq) = ev.get("sequence").and_then(Value::as_u64)
                && seq > run.last_sequence
            {
                run.last_sequence = seq;
            }
            run.persisted.push(ev.clone());
        }
    }
    for ev in events {
        (inner.options.emit_event)(&run_id, &mission_id, ev);
    }
    Ok(())
}

fn restore_gaps(
    cascade: Option<&Arc<dyn CascadeApi>>,
    conversation_id: &str,
    events: Vec<crate::runtime::Event>,
) -> Vec<crate::runtime::Event> {
    let gapped: Vec<&crate::runtime::Event> =
        events.iter().filter(|e| has_antigravity_gap(e)).collect();
    if gapped.is_empty() {
        return events;
    }
    let Some(cascade) = cascade else {
        return events;
    };
    let steps: Vec<Option<usize>> = gapped
        .iter()
        .map(|ev| {
            ev.payload["itemId"]
                .as_str()
                .and_then(antigravity_message_step)
        })
        .collect();
    let from = if steps.iter().all(Option::is_some) {
        steps.into_iter().flatten().min().unwrap_or(0)
    } else {
        0
    };
    match cascade.planner_texts(conversation_id, from) {
        Ok(texts) => with_restored_gaps(events, &texts),
        Err(_) => events,
    }
}

fn note_questions(inner: &ServiceInner, run_arc: &Arc<Mutex<LiveRun>>, events: &[Value]) {
    let mut to_withdraw: Vec<String> = Vec::new();
    {
        let mut run = crate::invariants::lock(run_arc);
        for ev in events {
            let ty = ev["type"].as_str().unwrap_or_default();
            let payload = &ev["payload"];
            let item_id = payload["itemId"].as_str();
            if ty == "tool.started"
                && let Some(item_id) = item_id
                && let Some(q_obj) = payload.get("question").and_then(Value::as_object)
                && let Some(q_text) = q_obj.get("question").and_then(Value::as_str)
            {
                if let Some(prev) = run.asking.take()
                    && prev.raised
                {
                    to_withdraw.push(prev.approval_id);
                }
                let options = q_obj
                    .get("options")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                let asked_at_step = q_obj
                    .get("askedAtStep")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize;
                run.asking = Some(Asking {
                    item_id: item_id.to_owned(),
                    question: ToolQuestionInfo {
                        question: q_text.to_owned(),
                        options,
                        asked_at_step,
                    },
                    approval_id: format!("ap_{}", (inner.options.create_id)()),
                    pending: None,
                    lookups: 0,
                    raised: false,
                    answered: false,
                });
            } else if matches!(ty, "tool.completed" | "tool.failed")
                && let Some(item_id) = item_id
                && run.asking.as_ref().is_some_and(|a| a.item_id == item_id)
                && let Some(prev) = run.asking.take()
                && prev.raised
            {
                to_withdraw.push(prev.approval_id);
            }
        }
    }
    if let Some(withdraw) = &inner.options.withdraw_approval {
        for id in to_withdraw {
            withdraw(&id);
        }
    }
}

fn raise_question(inner: &ServiceInner, run_arc: &Arc<Mutex<LiveRun>>) {
    let Some(emit_approval) = &inner.options.emit_approval else {
        return;
    };
    let lookup_info = {
        let mut run = crate::invariants::lock(run_arc);
        let Some(asking) = run.asking.as_mut() else {
            return;
        };
        if asking.raised {
            return;
        }
        asking.lookups += 1;
        (
            asking.approval_id.clone(),
            asking.question.asked_at_step,
            run.conversation_id.clone(),
            run.cascade.clone(),
        )
    };
    let (approval_id, asked_at_step, conversation_id, cascade) = lookup_info;
    let found = cascade
        .as_ref()
        .and_then(|c| c.pending_question(&conversation_id, asked_at_step).ok())
        .flatten();
    let maybe_card = {
        let mut run = crate::invariants::lock(run_arc);
        if run.ended {
            return;
        }
        let run_id = run.run_id.clone();
        let mission_id = run.mission_id.clone();
        let Some(asking) = run.asking.as_mut() else {
            return;
        };
        if asking.approval_id != approval_id || asking.raised {
            return;
        }
        asking.pending = found;
        if asking.pending.is_none() && asking.lookups < inner.question_lookups {
            return;
        }
        asking.raised = true;
        let now_iso = (inner.options.now_iso)();
        Some(question_card(
            &inner.options.workspace_path,
            &now_iso,
            &run_id,
            &mission_id,
            asking,
        ))
    };
    if let Some(card) = maybe_card {
        emit_approval(card);
    }
}

fn end_on_receipt_failure(inner: &ServiceInner, run_arc: &Arc<Mutex<LiveRun>>) {
    let (run_id, mission_id) = {
        let mut run = crate::invariants::lock(run_arc);
        if run.ended {
            return;
        }
        run.ended = true;
        run.stop_flag.store(true, Ordering::SeqCst);
        (run.run_id.clone(), run.mission_id.clone())
    };
    crate::invariants::lock(&inner.runs).remove(&run_id);
    if let Some(emit_update) = &inner.options.emit_update {
        emit_update(MissionUpdate {
            kind: "persistence-error".into(),
            run_id,
            mission_id: mission_id.clone(),
            error_code: Some("MISSION_PERSISTENCE_FAILED".into()),
            error_message: Some(
                "The mission could not be written to the durable local ledger.".into(),
            ),
        });
    }
    if let Some(on_ended) = &inner.options.on_run_ended {
        on_ended(&mission_id);
    }
}

fn observe_after(
    inner: &ServiceInner,
    run_arc: &Arc<Mutex<LiveRun>>,
    disk_before: Option<WorkspaceSnapshot>,
    shared_tree: bool,
) {
    let Some(before) = disk_before else {
        return;
    };
    let Some(observe_disk) = &inner.options.observe_disk else {
        return;
    };
    let Some(after) = observe_disk(&inner.options.workspace_path) else {
        return;
    };
    let changed = changed_paths(&before, &after);
    if changed.is_empty() {
        return;
    }
    let at = (inner.options.now_iso)();
    let (run_id, mission_id, next_seq, persisted) = {
        let run = crate::invariants::lock(run_arc);
        (
            run.run_id.clone(),
            run.mission_id.clone(),
            run.last_sequence + 1,
            run.persisted.clone(),
        )
    };
    if shared_tree {
        let notice = shared_tree_notice(&run_id, &mission_id, "antigravity", next_seq, &at, &changed);
        let _ = persist_and_emit(inner, run_arc, vec![notice]);
        return;
    }
    let unreported: BTreeSet<String> = unreported_paths(&changed, &persisted).into_iter().collect();
    let patches = inner
        .options
        .observe_patches
        .as_ref()
        .map(|f| f(&inner.options.workspace_path, &after, &changed, Some(&before)))
        .unwrap_or_default();
    let reported: BTreeSet<String> = changed
        .iter()
        .filter(|p| !unreported.contains(*p))
        .cloned()
        .collect();
    let observed = observed_edit_events(
        &run_id,
        &mission_id,
        "antigravity",
        next_seq,
        &changed,
        &at,
        &patches,
        &reported,
    );
    let _ = persist_and_emit(inner, run_arc, observed);
}

fn end_run(
    inner: &ServiceInner,
    run_arc: &Arc<Mutex<LiveRun>>,
    cancelled: bool,
    stderr: &str,
) {
    let (run_id, mission_id, withdraw_id, finished_events, disk_before, shared_tree) = {
        let mut run = crate::invariants::lock(run_arc);
        if run.ended {
            return;
        }
        run.ended = true;
        run.stop_flag.store(true, Ordering::SeqCst);
        let withdraw_id = run
            .asking
            .take()
            .filter(|a| a.raised)
            .map(|a| a.approval_id);
        let raw = run.normalizer.finish_with_stderr(cancelled, None, stderr);
        let at = (inner.options.now_iso)();
        let wrapped = wrap_events(
            &run.run_id,
            &run.mission_id,
            &at,
            run.last_sequence + 1,
            raw,
        );
        (
            run.run_id.clone(),
            run.mission_id.clone(),
            withdraw_id,
            wrapped,
            run.disk_before.clone(),
            run.shared_tree,
        )
    };
    crate::invariants::lock(&inner.runs).remove(&run_id);
    if let Some(id) = withdraw_id
        && let Some(withdraw) = &inner.options.withdraw_approval
    {
        withdraw(&id);
    }
    let _ = persist_and_emit(inner, run_arc, finished_events);
    observe_after(inner, run_arc, disk_before, shared_tree);
    if let Some(on_ended) = &inner.options.on_run_ended {
        on_ended(&mission_id);
    }
}

fn poll_run(inner: &ServiceInner, run_arc: &Arc<Mutex<LiveRun>>) {
    let (transcript_path, prev_seen, fed, conversation_id, cascade, next_seq, run_id, mission_id) = {
        let run = crate::invariants::lock(run_arc);
        if run.ended {
            return;
        }
        if run
            .polling
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        (
            run.transcript_path.clone(),
            run.seen.clone(),
            run.fed,
            run.conversation_id.clone(),
            run.cascade.clone(),
            run.last_sequence + 1,
            run.run_id.clone(),
            run.mission_id.clone(),
        )
    };

    let poll_result = (|| {
        let info = inner
            .options
            .stat_transcript
            .as_ref()
            .and_then(|stat_fn| stat_fn(&transcript_path));
        let unmoved = matches!((&info, &prev_seen), (Some(a), Some(b)) if a == b);
        let text = if unmoved {
            None
        } else {
            (inner.options.read_transcript)(&transcript_path)
        };
        if !unmoved && text.is_some() {
            crate::invariants::lock(run_arc).seen = info;
        }
        let lines: Vec<&str> = text
            .as_deref()
            .map(|t| t.split('\n').filter(|l| !l.trim().is_empty()).collect())
            .unwrap_or_default();

        let read_events = if lines.len() > fed {
            let mut run = crate::invariants::lock(run_arc);
            let mut out = Vec::new();
            for line in &lines[fed..] {
                out.extend(run.normalizer.accept_line(line));
            }
            out
        } else {
            Vec::new()
        };

        let restored = restore_gaps(cascade.as_ref(), &conversation_id, read_events);
        let at = (inner.options.now_iso)();
        let wrapped = wrap_events(&run_id, &mission_id, &at, next_seq, restored);
        if !wrapped.is_empty() {
            if persist_and_emit(inner, run_arc, wrapped.clone()).is_err() {
                end_on_receipt_failure(inner, run_arc);
                return;
            }
            note_questions(inner, run_arc, &wrapped);
        }

        let finished_now = {
            let mut run = crate::invariants::lock(run_arc);
            if lines.len() > run.fed {
                run.fed = lines.len();
                run.last_progress_at = Instant::now();
                run.said_it_might_be_asking = false;
            }
            run.normalizer.latest_final && run.normalizer.pending_background() == 0
        };
        if finished_now {
            end_run(inner, run_arc, false, "");
            return;
        }

        raise_question(inner, run_arc);

        let (waiting_on_person, silent_for_ms, pending_now, fed_now, fed_at_start, mut notices) = {
            let mut run = crate::invariants::lock(run_arc);
            if run.ended {
                return;
            }
            let waiting = run.asking.as_ref().is_some_and(|a| !a.answered);
            let silent = run.last_progress_at.elapsed().as_millis() as u64;
            let pending = run.normalizer.pending_tool_name().map(str::to_owned);
            let mut to_notify = Vec::new();
            if !waiting
                && silent > inner.asking_notice_ms
                && silent <= inner.options.idle_timeout_ms
                && let Some(ref tool) = pending
                && !run.said_it_might_be_asking
            {
                run.said_it_might_be_asking = true;
                let (mins, raw) = minutes_rounded(silent);
                let plural = if raw == 1 { "" } else { "s" };
                to_notify.push(MissionNotice {
                    run_id: run.run_id.clone(),
                    mission_id: run.mission_id.clone(),
                    message: format!(
                        "Antigravity has been on its own \"{tool}\" step for {mins} minute{plural} without reporting anything. It asks questions in its own window, not here -- if it is waiting on you, answer it there."
                    ),
                });
            }
            if !waiting
                && run.fed == run.fed_at_start
                && silent > inner.not_started_notice_ms
                && silent <= inner.options.idle_timeout_ms
                && !run.said_not_started
            {
                run.said_not_started = true;
                let (mins, raw) = minutes_rounded(silent);
                let plural = if raw <= 1 { "" } else { "s" };
                to_notify.push(MissionNotice {
                    run_id: run.run_id.clone(),
                    mission_id: run.mission_id.clone(),
                    message: format!(
                        "Antigravity took this turn but has not started it: after {mins} minute{plural} nothing has reached its own record. If Antigravity's window was closed or crashed, open it again and send this again; Stop ends this turn now."
                    ),
                });
            }
            (waiting, silent, pending, run.fed, run.fed_at_start, to_notify)
        };
        let _ = (fed_now, fed_at_start);
        if let Some(notify) = &inner.options.notify {
            for notice in notices.drain(..) {
                notify(notice);
            }
        }
        if !waiting_on_person && silent_for_ms > inner.options.idle_timeout_ms {
            let minutes = ((inner.options.idle_timeout_ms as f64) / 60_000.0).round() as u64;
            let stderr = match pending_now {
                None => format!(
                    "Antigravity's agent wrote nothing to its transcript for {minutes} minutes."
                ),
                Some(tool) => format!(
                    "Antigravity's agent has been waiting {minutes} minutes on its own \"{tool}\" step. If it is asking you something, Locust cannot see the question or answer it yet — answer it in Antigravity's own window."
                ),
            };
            end_run(inner, run_arc, false, &stderr);
        }
    })();
    let _ = poll_result;
    crate::invariants::lock(run_arc)
        .polling
        .store(false, Ordering::SeqCst);
}

impl AntigravityMissionService {
    pub fn new(options: AntigravityMissionOptions) -> Self {
        let half_idle = options.idle_timeout_ms / 2;
        let asking_notice_ms = options.asking_notice_ms.min(half_idle);
        let not_started_notice_ms = options.not_started_notice_ms.min(half_idle);
        let question_lookups = options.question_lookups.max(1);
        Self {
            inner: Arc::new(ServiceInner {
                options,
                asking_notice_ms,
                not_started_notice_ms,
                question_lookups,
                runs: Mutex::new(HashMap::new()),
                starting: Mutex::new(HashSet::new()),
                disposed: AtomicBool::new(false),
            }),
        }
    }

    pub fn has(&self, run_id: &str) -> bool {
        crate::invariants::lock(&self.inner.runs).contains_key(run_id)
    }

    pub fn has_mission(&self, mission_id: &str) -> bool {
        let runs = crate::invariants::lock(&self.inner.runs);
        runs.values()
            .any(|r| crate::invariants::lock(r).mission_id == mission_id)
    }

    pub fn live_mission_ids(&self) -> Vec<String> {
        let runs = crate::invariants::lock(&self.inner.runs);
        runs.values()
            .map(|r| crate::invariants::lock(r).mission_id.clone())
            .collect()
    }

    pub fn run_id_owned_by(&self, teammate_id: &str) -> Option<String> {
        let runs = crate::invariants::lock(&self.inner.runs);
        runs.values().find_map(|r| {
            let run = crate::invariants::lock(r);
            run.peer
                .as_ref()
                .filter(|p| p.self_peer.teammate_id == teammate_id)
                .map(|_| run.run_id.clone())
        })
    }

    pub fn start(
        &self,
        prompt: &str,
        peer: Option<MissionPeerContext>,
        route: MissionRoute,
    ) -> Result<AntigravityMission, AntigravityStartError> {
        if self.inner.disposed.load(Ordering::SeqCst) {
            return Err(AntigravityStartError::new(
                "The mission service is shutting down.",
            ));
        }
        if let Some(spend_refusal) = &self.inner.options.spend_refusal {
            let tid = peer
                .as_ref()
                .map(|p| p.self_peer.teammate_id.as_str())
                .unwrap_or("");
            if let Some(said) = spend_refusal(tid) {
                return Err(AntigravityStartError::busy(said, BusyKind::Limit));
            }
        }
        let owner = owner_key_of(peer.as_ref());
        {
            let mut starting = crate::invariants::lock(&self.inner.starting);
            let runs = crate::invariants::lock(&self.inner.runs);
            let owner_busy = starting.contains(&owner)
                || runs
                    .values()
                    .any(|r| owner_key_of(crate::invariants::lock(r).peer.as_ref()) == owner);
            if owner_busy {
                let message = match &peer {
                    None => "A mission is already running.".to_owned(),
                    Some(p) => format!(
                        "{} already has a mission running. Wait for it to finish or stop it first.",
                        p.self_peer.name
                    ),
                };
                return Err(AntigravityStartError::busy(message, BusyKind::Teammate));
            }
            if starting.len() + runs.len() >= MAX_LIVE_ANTIGRAVITY_MISSIONS {
                return Err(AntigravityStartError::busy(
                    format!(
                        "Up to {MAX_LIVE_ANTIGRAVITY_MISSIONS} Antigravity missions can run at once."
                    ),
                    BusyKind::Pool,
                ));
            }
            starting.insert(owner.clone());
        }

        let result = self.start_inner(prompt, peer, route);
        crate::invariants::lock(&self.inner.starting).remove(&owner);
        result
    }

    fn start_inner(
        &self,
        prompt: &str,
        peer: Option<MissionPeerContext>,
        route: MissionRoute,
    ) -> Result<AntigravityMission, AntigravityStartError> {
        let host = (self.inner.options.probe)().ok_or_else(|| {
            AntigravityStartError::new(
                "Antigravity is not open. Open it with this folder, then try again.",
            )
        })?;
        let project_id =
            project_id_for(&host, &self.inner.options.workspace_path).ok_or_else(|| {
                AntigravityStartError::new(format!(
                    "Antigravity has not opened {}. Open that folder in Antigravity first; this route only works inside a folder it knows.",
                    self.inner.options.workspace_path
                ))
            })?;
        let model = antigravity_tier(route.model.as_deref());
        let api = (self.inner.options.agent_api)(&host);

        let prior_conversation = if let Some(ref follow_up_of) = route.follow_up_of {
            let prior = self.inner.options.ledger.get_mission(follow_up_of);
            let thread_id = prior.as_ref().and_then(|p| p.runtime_thread_id.clone());
            let (Some(prior), Some(thread_id)) = (prior, thread_id) else {
                return Err(AntigravityStartError::new(
                    "That conversation cannot be continued: the earlier mission did not record a conversation to resume.",
                ));
            };
            if prior.runtime != "antigravity" {
                return Err(AntigravityStartError::new(
                    "That conversation belongs to another runtime. Switch the route back, or start a new mission.",
                ));
            }
            Some(FollowUpInfo {
                mission_id: prior.mission_id,
                runtime_thread_id: thread_id,
            })
        } else {
            None
        };

        let run_id = format!("run_{}", (self.inner.options.create_id)());
        let mission_id = format!("mission_{}", (self.inner.options.create_id)());
        let created_at = (self.inner.options.now_iso)();

        let disk_before = self
            .inner
            .options
            .observe_disk
            .as_ref()
            .and_then(|f| f(&self.inner.options.workspace_path));

        let (conversation_id, fed) = match &prior_conversation {
            None => {
                let title_raw: String = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
                let title: String = if title_raw.is_empty() {
                    "Locust mission".into()
                } else {
                    title_raw.chars().take(60).collect()
                };
                let conv = api
                    .new_conversation(NewConversationInput {
                        project_id: project_id.clone(),
                        model: model.into(),
                        title,
                        prompt: prompt.into(),
                    })
                    .map_err(AntigravityStartError::new)?;
                (conv, 0)
            }
            Some(prior) => {
                let existing = (self.inner.options.read_transcript)(&transcript_path_for(
                    &self.inner.options.home,
                    &prior.runtime_thread_id,
                ));
                let fed = existing
                    .as_deref()
                    .map(|t| t.split('\n').filter(|l| !l.trim().is_empty()).count())
                    .unwrap_or(0);
                api.send_message(SendMessageInput {
                    project_id: project_id.clone(),
                    conversation_id: prior.runtime_thread_id.clone(),
                    content: prompt.into(),
                })
                .map_err(AntigravityStartError::new)?;
                (prior.runtime_thread_id.clone(), fed)
            }
        };

        let mut mission_record = json!({
            "missionId": mission_id,
            "runId": run_id,
            "prompt": prompt,
            "runtime": "antigravity",
            "model": model,
            "requestedRouteId": "antigravity",
            "resolvedRouteId": "antigravity:hub",
            "cliVersion": host.version,
            "sandbox": "workspace-write",
            "executionPolicyVersion": 1,
            "createdAt": created_at,
        });
        if let Some(ref prior) = prior_conversation {
            mission_record["continuesFrom"] = json!({
                "missionId": prior.mission_id,
                "checkpointEpoch": 1,
                "reason": "follow-up",
                "runtimeThreadId": conversation_id,
            });
        }
        if self.inner.options.ledger.create_mission(mission_record).is_err() {
            return Err(AntigravityStartError::new(
                "Antigravity has already started on this, but Locust could not record the mission, so it will not appear here. Open Antigravity to follow it, and do not send it again.",
            ));
        }

        let mut normalizer = TranscriptNormalizer::default();
        normalizer.thread = Some(conversation_id.clone());
        let cascade = self.inner.options.cascade_api.as_ref().map(|f| f(&host));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let transcript_path = transcript_path_for(&self.inner.options.home, &conversation_id);

        let run_arc = {
            let mut runs = crate::invariants::lock(&self.inner.runs);
            let shared_tree = !runs.is_empty();
            for other in runs.values() {
                crate::invariants::lock(other).shared_tree = true;
            }
            let arc = Arc::new(Mutex::new(LiveRun {
                run_id: run_id.clone(),
                mission_id: mission_id.clone(),
                conversation_id: conversation_id.clone(),
                peer,
                normalizer,
                transcript_path,
                fed,
                fed_at_start: fed,
                said_not_started: false,
                last_progress_at: Instant::now(),
                said_it_might_be_asking: false,
                cascade,
                asking: None,
                polling: AtomicBool::new(false),
                stop_flag: stop_flag.clone(),
                seen: None,
                ended: false,
                disk_before,
                persisted: Vec::new(),
                last_sequence: 0,
                shared_tree,
            }));
            runs.insert(run_id.clone(), arc.clone());
            arc
        };

        let inner_bg = self.inner.clone();
        let run_bg = run_arc.clone();
        let poll_ms = self.inner.options.poll_ms.max(1);
        std::thread::spawn(move || {
            loop {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }
                poll_run(&inner_bg, &run_bg);
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(poll_ms));
            }
        });

        Ok(AntigravityMission {
            run_id,
            mission_id,
            conversation_id,
            model: model.into(),
            cli_version: host.version,
            follows_up: prior_conversation,
        })
    }

    pub fn decide(&self, answer: &Answer) -> bool {
        let approval_id = answer.approval_id();
        let Answer::Answers { answers, .. } = answer else {
            return false;
        };
        let target = {
            let runs = crate::invariants::lock(&self.inner.runs);
            runs.values()
                .find(|r| {
                    let run = crate::invariants::lock(r);
                    run.asking
                        .as_ref()
                        .is_some_and(|a| a.approval_id == approval_id)
                })
                .cloned()
        };
        let Some(run_arc) = target else {
            return false;
        };
        let (conversation_id, run_id, mission_id, pending, cascade) = {
            let run = crate::invariants::lock(&run_arc);
            let Some(asking) = run.asking.as_ref() else {
                return false;
            };
            if !asking.raised || asking.answered {
                return false;
            }
            let Some(pending) = asking.pending.clone() else {
                return false;
            };
            let Some(cascade) = run.cascade.clone() else {
                return false;
            };
            (
                run.conversation_id.clone(),
                run.run_id.clone(),
                run.mission_id.clone(),
                pending,
                cascade,
            )
        };
        let map: HashMap<String, Vec<String>> = answers.iter().cloned().collect();
        let responses = responses_from_answers(&pending, &map);
        if let Err(why) = cascade.answer_question(&conversation_id, &pending, &responses) {
            if let Some(notify) = &self.inner.options.notify {
                notify(MissionNotice {
                    run_id,
                    mission_id,
                    message: format!(
                        "Locust could not hand your answer to Antigravity ({why}). Answer it in Antigravity's own window; this run carries on from there."
                    ),
                });
            }
            return false;
        }
        let mut run = crate::invariants::lock(&run_arc);
        if let Some(asking) = run.asking.as_mut()
            && asking.approval_id == approval_id
        {
            asking.answered = true;
            run.last_progress_at = Instant::now();
            return true;
        }
        false
    }

    pub fn cancel(&self, run_id: &str) -> bool {
        let run_arc = crate::invariants::lock(&self.inner.runs).get(run_id).cloned();
        let Some(run_arc) = run_arc else {
            return false;
        };
        end_run(
            &self.inner,
            &run_arc,
            true,
            "Stopped watching. Antigravity's agent may still be working inside Antigravity.",
        );
        true
    }

    pub fn dispose(&self) {
        self.inner.disposed.store(true, Ordering::SeqCst);
        let live: Vec<Arc<Mutex<LiveRun>>> = crate::invariants::lock(&self.inner.runs)
            .values()
            .cloned()
            .collect();
        for run_arc in live {
            end_run(&self.inner, &run_arc, true, "The app is closing.");
        }
    }
}

impl Drop for AntigravityMissionService {
    fn drop(&mut self) {
        self.dispose();
    }
}

#[cfg(test)]
#[path = "a_long_antigravity_answer_tests.rs"]
mod a_long_antigravity_answer_tests;

#[cfg(test)]
#[path = "a_question_antigravity_asks_tests.rs"]
mod a_question_antigravity_asks_tests;

#[cfg(test)]
#[path = "antigravity_mission_tests.rs"]
mod antigravity_mission_tests;

#[cfg(test)]
#[path = "antigravity_gaps_tests.rs"]
mod antigravity_gaps_tests;
