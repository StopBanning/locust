//! Automatic teammate relay engine, conversation nesting, and exchange overview
//! (`main/relay.ts`, `shared/nested-conversations.ts`, `renderer/src/exchange.ts`).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;

use serde_json::{Map, Value, json};

use crate::benchmark::RunCost;
use crate::carry_on::display_name;
use crate::invariants::regex;
use crate::peer_share::{bounded_share_text, strip_share_blocks};
use crate::protocol_tags::{
    defang_protocol_blocks, strip_blocks_outside_code, strip_decision_blocks, strip_file_blocks,
    strip_next_blocks,
};
use crate::spend::money_of_usage;

pub const MAX_RELAY_HOPS: usize = 12;
pub const EXCHANGES_REMEMBERED: usize = 400;

/// How a returned answer is labelled in the asker's messages (A2.1).
pub const RETURNED_ANSWER: &str =
    "(Returned by Locust: this is how they answered in their own conversation; they did not write back.)";

/// One entry on the roster (`PeerRosterEntry`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnRoute {
    pub runtime: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub account: Option<String>,
    pub mode: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerRosterEntry {
    pub teammate_id: String,
    pub name: String,
    pub role: String,
    pub route: Option<TurnRoute>,
}

/// Exchange origin passed onto relayed missions (`RelayOrigin`).
/// `last_mission_of` keeps insertion order so index 0 is always the person-started teammate.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RelayOrigin {
    pub hop: usize,
    pub root_mission_id: Option<String>,
    pub last_mission_of: Vec<(String, String)>,
    pub relayed_to: Vec<String>,
    pub answering: Vec<String>,
}

impl RelayOrigin {
    pub fn last_mission_for(&self, teammate_id: &str) -> Option<&str> {
        self.last_mission_of
            .iter()
            .rev()
            .find(|(id, _)| id == teammate_id)
            .map(|(_, m)| m.as_str())
    }

    pub fn first_teammate_id(&self) -> Option<&str> {
        self.last_mission_of.first().map(|(id, _)| id.as_str())
    }

    pub fn with_last_mission(&self, teammate_id: &str, mission_id: &str) -> Vec<(String, String)> {
        let mut next = self.last_mission_of.clone();
        if let Some(slot) = next.iter_mut().find(|(id, _)| id == teammate_id) {
            slot.1 = mission_id.to_owned();
        } else {
            next.push((teammate_id.to_owned(), mission_id.to_owned()));
        }
        next
    }

    pub fn to_json(&self) -> Value {
        let mut last = Map::new();
        for (k, v) in &self.last_mission_of {
            last.insert(k.clone(), Value::String(v.clone()));
        }
        let mut obj = Map::new();
        obj.insert("hop".into(), Value::from(self.hop));
        if let Some(root) = &self.root_mission_id {
            obj.insert("rootMissionId".into(), Value::String(root.clone()));
        }
        obj.insert("lastMissionOf".into(), Value::Object(last));
        if !self.relayed_to.is_empty() {
            obj.insert(
                "relayedTo".into(),
                Value::Array(self.relayed_to.iter().cloned().map(Value::String).collect()),
            );
        }
        if !self.answering.is_empty() {
            obj.insert(
                "answering".into(),
                Value::Array(self.answering.iter().cloned().map(Value::String).collect()),
            );
        }
        Value::Object(obj)
    }

    pub fn from_json(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        let hop = obj.get("hop")?.as_u64()? as usize;
        let root_mission_id = obj
            .get("rootMissionId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let mut last_mission_of = Vec::new();
        if let Some(last) = obj.get("lastMissionOf").and_then(Value::as_object) {
            for (k, v) in last {
                if let Some(m) = v.as_str() {
                    last_mission_of.push((k.clone(), m.to_owned()));
                }
            }
        }
        let relayed_to = obj
            .get("relayedTo")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let answering = obj
            .get("answering")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        Some(Self {
            hop,
            root_mission_id,
            last_mission_of,
            relayed_to,
            answering,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MissionStarter {
    Person,
    Relay { hop: usize },
    Mcp,
    Other(String),
}

impl MissionStarter {
    pub fn from_json(value: &Value) -> Option<Self> {
        let kind = value.get("kind").and_then(Value::as_str)?;
        Some(match kind {
            "person" => Self::Person,
            "mcp" => Self::Mcp,
            "relay" => Self::Relay {
                hop: value.get("hop").and_then(Value::as_u64).unwrap_or(0) as usize,
            },
            other => Self::Other(other.to_owned()),
        })
    }

    pub fn to_json(&self) -> Value {
        match self {
            Self::Person => json!({"kind": "person"}),
            Self::Mcp => json!({"kind": "mcp"}),
            Self::Relay { hop } => json!({"kind": "relay", "hop": hop}),
            Self::Other(kind) => json!({"kind": kind}),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecideRelayInput<'a> {
    pub enabled: bool,
    pub defer: bool,
    pub closes: bool,
    pub hop: usize,
    pub recipient_name: &'a str,
    pub cap: Option<usize>,
    pub spent: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelayDecision {
    Start { hop: usize },
    Skip { reason: String },
}

/// Whether a posted message earns an automatic run for its recipient (`decideRelay`).
pub fn decide_relay(input: DecideRelayInput<'_>) -> RelayDecision {
    let cap = input.cap.unwrap_or(MAX_RELAY_HOPS);
    if input.defer {
        return RelayDecision::Skip {
            reason: format!(
                "Sent to {} to read on their next run, rather than starting one.",
                input.recipient_name
            ),
        };
    }
    if input.closes {
        return RelayDecision::Skip {
            reason: format!(
                "Sent to {} to read on their next run: the message says nothing more is needed from them.",
                input.recipient_name
            ),
        };
    }
    if !input.enabled {
        return RelayDecision::Skip {
            reason: "Teammate replies are switched off in Settings; the message waits for their next run."
                .to_owned(),
        };
    }
    let spent = input.spent.unwrap_or(input.hop);
    if spent >= cap {
        let noun = if cap == 1 { "reply" } else { "replies" };
        return RelayDecision::Skip {
            reason: format!(
                "Stopped after {cap} automatic {noun}. {} will see this on their next run.",
                input.recipient_name
            ),
        };
    }
    RelayDecision::Start { hop: input.hop + 1 }
}

/// True when a message says, in so many words, that nothing is needed from the recipient
/// and asks nothing (`saysNothingIsNeeded`).
pub fn says_nothing_is_needed(text: &str) -> bool {
    let nothing = [
        regex(
            r"(?i)\bno (?:further |more |other )?(?:action|work) (?:is |will be )?(?:needed|required|necessary) (?:from you|on your (?:part|end|side))\b",
        ),
        regex(
            r"(?i)\bnothing (?:further|more|else) is (?:needed|required) (?:from you|on your (?:part|end|side))\b",
        ),
        regex(
            r"(?i)\byou (?:don't|do not|needn't|need not) (?:need to )?(?:do anything|take any action|act on this)\b",
        ),
        regex(r"(?i)\bno need for you to (?:do anything|take any action|act on this)\b"),
    ];
    let asks = regex(
        r"(?i)\?|\b(?:please|could you|can you|would you|will you|need you to|i need|make sure|go ahead)\b",
    );
    nothing.iter().any(|pat| pat.is_match(text)) && !asks.is_match(text)
}

/// What a reply that runs READ-ONLY is told (`readOnlyReplySentence`).
pub fn read_only_reply_sentence() -> &'static str {
    "You are replying in a read-only mode: a shell command is refused here and ends this run with nothing written. What you were sent is quoted in this message -- answer from it and from your read tools, and write back in the share block."
}

/// Where this reply sits in the budget (`budgetSentence`).
pub fn budget_sentence(hop: usize, cap: Option<usize>) -> String {
    let budget = cap.unwrap_or(MAX_RELAY_HOPS);
    let left = budget as isize - hop as isize;
    if left <= 0 {
        return "This is the LAST automatic reply in this exchange: anything you send back now waits for a person rather than reaching them, so finish what you can say here.".to_owned();
    }
    if left == 1 {
        return format!(
            "This is automatic reply {hop} of {budget}. One more would be the last, so do not write back unless it genuinely needs saying."
        );
    }
    format!("This is automatic reply {hop} of {budget}.")
}

fn no_person_here(sender: &str) -> String {
    format!(
        "The person is not in this exchange: {sender} is a teammate, and the question came from them. If you need a decision or more context before you can help, ask {sender} inside that share block. A <locust-ask> block reaches only a person, so here it would strand the exchange."
    )
}

#[derive(Clone, Debug)]
pub struct RelayPromptInput<'a> {
    pub sender: &'a PeerRosterEntry,
    pub recipient: &'a PeerRosterEntry,
    pub hop: usize,
    pub cap: Option<usize>,
    pub read_by_person: bool,
    pub reply_number: Option<usize>,
    pub briefed_before: bool,
}

/// The brief a relayed run is started with (`relayPrompt`).
pub fn relay_prompt(input: &RelayPromptInput<'_>) -> String {
    let who = format!("{} ({})", input.sender.name, input.sender.role);
    let opening = if input.hop <= 1 {
        format!("{who} sent you a message; it is quoted below with anything else waiting for you.")
    } else {
        format!("{who} replied to you; it is quoted below.")
    };
    let read_by_person = input.read_by_person && input.hop > 1;
    let reply_num = input.reply_number.unwrap_or(input.hop);

    if input.briefed_before && input.hop > 1 {
        let mut parts = vec![opening];
        if read_by_person {
            parts.push(format!(
                "The person who started this conversation reads it: say in a line or two what {}'s reply means for what they asked -- the answer, where it is, what is still open -- written to them, and then stop.",
                input.sender.name
            ));
        }
        parts.push(format!(
            "The rules for replies from your earlier turn in this exchange still hold: {} is a model, a reply costs a whole run, so write back in one <locust-share to=\"{}\"> block only when it moves the work, and end with no share block when nothing more is needed.",
            input.sender.name, input.sender.name
        ));
        parts.push(budget_sentence(reply_num, input.cap));
        parts.push("Stay on what was asked.".to_owned());
        return parts.join(" ");
    }

    let mut parts = vec![opening];
    if input.hop > 1 {
        if read_by_person {
            parts.push(format!(
                "{} is a MODEL, not a person, and every reply you send THEM starts another whole mission that costs money.",
                input.sender.name
            ));
            parts.push(format!(
                "But the person who started this conversation reads it, and this is what they were waiting for: say in a line or two what {}'s reply means for what they asked -- the answer, where it is, what is still open -- written to them, not about what you are going to write -- and then stop.",
                input.sender.name
            ));
            parts.push(format!(
                "Write back to {} only when the work needs it; thanks and receipts reach nobody.",
                input.sender.name
            ));
        } else {
            parts.push(format!(
                "{} is a MODEL, not a person, and every reply you send starts another whole mission that costs money.",
                input.sender.name
            ));
            parts.push("A reply is worth that only when it moves the work: an answer with its evidence, a decision, or a question they need answered before they can continue. If their message settles things or needs nothing from you, END HERE with no share block -- that is an exchange finishing, and it is the most common good outcome. Thanks, receipts and recaps reach nobody and cost a run each.".to_owned());
        }
    }
    parts.push("Do what it asks if that is within your role and this workspace, using what you actually know, and say plainly what is blocked or unverified rather than implying it is done; if you cannot help, say so and why.".to_owned());
    parts.push(format!(
        "When a reply is needed, end with one <locust-share to=\"{}\"> block holding it, written for a capable colleague who has not seen your turn: lead with the answer, said as what is true rather than as a negation, then the evidence by name (paths, numbers, names), then what you need from them, if anything, in complete sentences.",
        input.sender.name
    ));
    parts.push(format!(
        "If {} said what shape the reply should take -- one word, a number, a list -- give it in that shape inside the share block; that shape wins over the sentence above.",
        input.sender.name
    ));
    parts.push("Add when=\"now\" to a share ONLY if waiting for them to finish would make it useless or cause a conflict, such as telling them to stop touching a file you are changing; an ordinary answer never needs it.".to_owned());
    if read_by_person {
        parts.push(format!(
            "If you need the person's decision, a <locust-ask> block reaches them here; anything for {} goes in the share block.",
            input.sender.name
        ));
    } else {
        parts.push(no_person_here(&input.sender.name));
    }
    parts.push(
        "If nothing more is needed, end with no share block -- that is how an exchange finishes."
            .to_owned(),
    );
    parts.push(budget_sentence(reply_num, input.cap));
    parts.push("Stay on what was asked.".to_owned());
    parts.join(" ")
}

const TO_THE_PERSON: &str = "The person who started this conversation reads it and is waiting on this: say in a line or two what came back and what it means for what they asked -- the answer, where it is, what is still open -- written to them. If you need their decision, a <locust-ask> block reaches them here.";

/// The brief for the turn after a meeting (`meetingPrompt`).
pub fn meeting_prompt(repliers: &[String], silent: &[String], read_by_person: bool) -> String {
    let who = if repliers.len() == 1 {
        repliers[0].clone()
    } else if repliers.len() >= 2 {
        format!(
            "{} and {}",
            repliers[..repliers.len() - 1].join(", "),
            repliers[repliers.len() - 1]
        )
    } else {
        String::new()
    };
    let mut parts = vec![format!(
        "{who} replied to your message; the replies are quoted below."
    )];
    if !silent.is_empty() {
        parts.push(format!(
            "{} finished without replying.",
            silent.join(", ")
        ));
    }
    parts.push("Take them together. Write back to anyone only if that helps finish the work: one <locust-share to=\"Name\"> block per teammate.".to_owned());
    parts.push(if read_by_person {
        TO_THE_PERSON.to_owned()
    } else {
        "There is no person in this exchange to answer you; a question for any of them goes in their share block, never in a <locust-ask> block, which reaches only a person.".to_owned()
    });
    parts.push("If nothing more is needed, end with no share block -- that is how an exchange finishes.".to_owned());
    parts.push("Do not start unrelated work.".to_owned());
    parts.join(" ")
}

/// The asker's turn when an answer is brought back to them (`returnedAnswerPrompt`).
pub fn returned_answer_prompt(recipient_name: &str, read_by_person: bool) -> String {
    let mut parts = vec![format!(
        "{recipient_name} answered your message in their own conversation and did not write back, so Locust brought their answer to you: it is quoted below as a message from them."
    )];
    if read_by_person {
        parts.push(TO_THE_PERSON.to_owned());
    }
    parts.push("Take it into account and carry on. Write back only if that moves the work; if nothing more is needed, end with no share block.".to_owned());
    parts.push("Do not start unrelated work.".to_owned());
    parts.join(" ")
}

/// A final reply, as another teammate may be shown it: every protocol block out, whatever is left defanged.
pub fn without_protocol_blocks(text: &str) -> String {
    let mem_re = regex(r"(?i)<locust-memory\b[^>]*>[\s\S]*?</locust-memory>");
    let task_re = regex(r"(?i)<locust-task\b[^>]*>[\s\S]*?</locust-task>");
    let s1 = strip_share_blocks(text);
    let s2 = strip_decision_blocks(&s1);
    let s3 = strip_blocks_outside_code(&s2, &task_re);
    let s4 = strip_blocks_outside_code(&s3, &mem_re);
    let s5 = strip_file_blocks(&s4);
    let s6 = strip_next_blocks(&s5);
    defang_protocol_blocks(&s6)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharingMission {
    pub run_id: String,
    pub mission_id: String,
    pub runtime: String,
    pub sandbox: String,
    pub model: Option<String>,
    pub peer: PeerRosterEntry,
    pub relay: Option<RelayOrigin>,
    pub started_by: Option<MissionStarter>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndedMission {
    pub mission_id: String,
    pub peer: Option<PeerRosterEntry>,
    pub relay: Option<RelayOrigin>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayedMessage {
    pub message_id: String,
    pub from_teammate_id: String,
    pub from_name: String,
    pub from_mission_id: String,
    pub to_teammate_id: String,
    pub to_name: String,
    pub text: String,
    pub urgent: bool,
    pub defer: bool,
    pub wants_answer: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelayNoticeLevel {
    Info,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelayUpdate {
    Notice {
        run_id: String,
        mission_id: String,
        message: String,
        level: RelayNoticeLevel,
    },
    Starting {
        teammate_id: String,
        name: String,
        answering: String,
        hop: usize,
    },
    StartSettled {
        teammate_id: String,
    },
    MissionStarted {
        run_id: String,
        mission_id: String,
        teammate_id: String,
        prompt: String,
        started_by_hop: usize,
        hub_mission_id: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartResult {
    Started { run_id: String, mission_id: String },
    Busy { pool: bool },
    Refused { message: String },
}

#[derive(Clone, Debug)]
pub struct StartRequest {
    pub prompt: String,
    pub runtime: String,
    pub mode: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub account: Option<String>,
    pub peer: PeerRosterEntry,
    pub follow_up_of: Option<String>,
    pub relay: RelayOrigin,
}

/// Host callbacks connecting the relay engine to the runtime/store layer (`RelayOptions`).
pub trait RelayHost: Send + Sync {
    fn enabled(&self) -> bool;
    fn hop_cap(&self) -> usize {
        MAX_RELAY_HOPS
    }
    fn exchange_of(&self, _mission_id: &str) -> Option<String> {
        None
    }
    fn peer_context_for(&self, teammate_id: &str) -> Option<PeerRosterEntry>;
    fn final_reply_of(&self, _mission_id: &str) -> Option<String> {
        None
    }
    fn how_ended(&self, _mission_id: &str) -> Option<String> {
        None
    }
    fn post_returned_answer(
        &self,
        _from_teammate_id: &str,
        _from_name: &str,
        _from_mission_id: &str,
        _to_teammate_id: &str,
        _to_name: &str,
        _text: &str,
    ) -> Option<String> {
        None
    }
    fn cursor_holds_read_only(&self) -> bool {
        true
    }
    fn route_of(&self, _mission_id: &str) -> Option<TurnRoute> {
        None
    }
    fn out_of_plan(&self, _runtime: &str, _account: Option<&str>) -> Option<String> {
        None
    }
    fn start(&self, req: StartRequest) -> Result<StartResult, String>;
    fn assign_owner(&self, teammate_id: &str, mission_id: &str);
    fn hub_of(&self, _teammate_id: &str) -> Option<String> {
        None
    }
    fn remember_hub(&self, _teammate_id: &str, _mission_id: &str, _began: bool) {}
    fn still_waiting(&self, _teammate_id: &str) -> Option<bool> {
        None
    }
    fn may_interrupt(&self) -> bool {
        false
    }
    fn stop_work_of(&self, _teammate_id: &str) -> bool {
        false
    }
    fn steer_work_of(&self, _teammate_id: &str, _text: &str) -> bool {
        false
    }
    fn notify(&self, update: RelayUpdate);
    fn note(&self, _mission_id: &str, _message: &str) {}
}

#[derive(Clone, Debug)]
struct OpenExchange {
    asker_run_id: String,
    asker_mission_id: String,
    asker_id: String,
    recipient_name: String,
    wants_answer: bool,
    asker: Option<SharingMission>,
    origin: Option<RelayOrigin>,
}

#[derive(Clone, Debug)]
struct DeferredReply {
    recipient: PeerRosterEntry,
    prompt: String,
    prompt_kind: DeferredPromptKind,
    from: SharingMission,
    origin: RelayOrigin,
    asker_run_id: String,
    asker_mission_id: String,
    exchange: Option<OpenExchange>,
}

#[derive(Clone, Debug)]
enum DeferredPromptKind {
    Fixed,
    Relay {
        sender: PeerRosterEntry,
        cap: usize,
        read_by_person: bool,
        briefed_before: bool,
    },
}

impl DeferredReply {
    fn prompt_for(&self, reply_number: usize) -> String {
        match &self.prompt_kind {
            DeferredPromptKind::Fixed => self.prompt.clone(),
            DeferredPromptKind::Relay {
                sender,
                cap,
                read_by_person,
                briefed_before,
            } => relay_prompt(&RelayPromptInput {
                sender,
                recipient: &self.recipient,
                hop: self.origin.hop,
                cap: Some(*cap),
                read_by_person: *read_by_person,
                reply_number: Some(reply_number),
                briefed_before: *briefed_before,
            }),
        }
    }
}

#[derive(Clone, Debug)]
struct Meeting {
    asker_id: String,
    asker_run_id: String,
    asker_mission_id: String,
    origin: RelayOrigin,
    /// Ordered awaiting teammates: `(teammate_id, name)`.
    awaiting: Vec<(String, String)>,
    answered: Vec<String>,
    silent: Vec<String>,
}

impl Meeting {
    fn awaiting_names(&self) -> Vec<String> {
        self.awaiting.iter().map(|(_, n)| n.clone()).collect()
    }

    fn has_awaiting(&self, teammate_id: &str) -> bool {
        self.awaiting.iter().any(|(id, _)| id == teammate_id)
    }

    fn take_awaiting(&mut self, teammate_id: &str) -> bool {
        if let Some(pos) = self.awaiting.iter().position(|(id, _)| id == teammate_id) {
            self.awaiting.remove(pos);
            true
        } else {
            false
        }
    }
}

#[derive(Default)]
struct RelayState {
    meetings: HashMap<String, Meeting>,
    exchanges: HashMap<String, OpenExchange>,
    held_also: HashMap<String, Vec<OpenExchange>>,
    also_waiting: HashMap<String, Vec<OpenExchange>>,
    answering: HashMap<String, String>,
    spent_by_exchange: HashMap<String, usize>,
    spent_order: VecDeque<String>,
    deferred: HashMap<String, DeferredReply>,
}

impl RelayState {
    fn spend_of(&self, root: Option<&str>, hop: usize) -> usize {
        let recorded = root
            .and_then(|r| self.spent_by_exchange.get(r).copied())
            .unwrap_or(0);
        hop.max(recorded)
    }

    fn reserve(&mut self, root: Option<&str>) {
        let Some(root) = root else { return };
        let next = self.spent_by_exchange.get(root).copied().unwrap_or(0) + 1;
        self.spent_by_exchange.insert(root.to_owned(), next);
        self.spent_order.retain(|k| k != root);
        self.spent_order.push_back(root.to_owned());
        while self.spent_by_exchange.len() > EXCHANGES_REMEMBERED {
            if let Some(oldest) = self.spent_order.pop_front() {
                self.spent_by_exchange.remove(&oldest);
            } else {
                break;
            }
        }
    }

    fn release(&mut self, root: Option<&str>) {
        let Some(root) = root else { return };
        let Some(held) = self.spent_by_exchange.get(root).copied() else {
            return;
        };
        if held <= 1 {
            self.spent_by_exchange.remove(root);
            self.spent_order.retain(|k| k != root);
        } else {
            self.spent_by_exchange.insert(root.to_owned(), held - 1);
        }
    }
}

struct StartedReply {
    teammate_id: String,
    name: String,
    mission_id: String,
    wants_answer: bool,
    origin: RelayOrigin,
}

pub struct Relay<H: RelayHost> {
    host: H,
    state: Mutex<RelayState>,
}

impl<H: RelayHost> Relay<H> {
    pub fn new(host: H) -> Self {
        Self {
            host,
            state: Mutex::new(RelayState::default()),
        }
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    fn notify(&self, run_id: &str, mission_id: &str, message: String, level: RelayNoticeLevel) {
        self.host.notify(RelayUpdate::Notice {
            run_id: run_id.to_owned(),
            mission_id: mission_id.to_owned(),
            message,
            level,
        });
    }

    fn notify_and_keep(&self, run_id: &str, mission_id: &str, message: String) {
        self.notify(run_id, mission_id, message.clone(), RelayNoticeLevel::Warning);
        self.host.note(mission_id, &message);
    }

    fn start_for(
        &self,
        recipient: &PeerRosterEntry,
        prompt: &str,
        from: &SharingMission,
        origin: &RelayOrigin,
        asker_run_id: &str,
        asker_mission_id: &str,
    ) -> StartOutcome {
        let in_exchange = origin
            .last_mission_for(&recipient.teammate_id)
            .map(str::to_owned);
        let hub = if in_exchange.is_none() {
            self.host.hub_of(&recipient.teammate_id)
        } else {
            None
        };
        let follow_up_of = in_exchange.clone().or_else(|| hub.clone());
        let onto_hub = in_exchange.is_none();

        let kept = in_exchange
            .as_deref()
            .and_then(|m| self.host.route_of(m));
        let own: Option<TurnRoute> = match kept {
            Some(mut r) => {
                if r.model.is_none() {
                    r.model = Some("account-default".to_owned());
                }
                Some(r)
            }
            None => recipient.route.clone(),
        };
        let lent = own.clone().unwrap_or_else(|| TurnRoute {
            runtime: from.runtime.clone(),
            model: Some(
                from.model
                    .clone()
                    .unwrap_or_else(|| "account-default".to_owned()),
            ),
            effort: None,
            account: None,
            mode: if from.sandbox == "workspace-write" {
                "accept-edits".to_owned()
            } else {
                "ask".to_owned()
            },
        });
        let cursor_widened = lent.runtime == "cursor"
            && (lent.mode == "ask" || lent.mode == "plan")
            && !self.host.cursor_holds_read_only();
        let mut route = lent.clone();
        if cursor_widened {
            route.mode = "accept-edits".to_owned();
            self.notify(
                asker_run_id,
                asker_mission_id,
                format!(
                    "{} replies in Edit mode rather than read-only: Cursor Agent cannot be held read-only on this system.",
                    recipient.name
                ),
                RelayNoticeLevel::Warning,
            );
        }
        if own.is_none() {
            let suffix = if route.mode == "accept-edits" {
                " and may edit this folder"
            } else {
                ", read-only"
            };
            self.notify(
                asker_run_id,
                asker_mission_id,
                format!(
                    "{} has no model of their own yet, so this reply uses {}'s ({}){suffix}. To give {} their own, edit {} on the Team screen.",
                    recipient.name,
                    from.peer.name,
                    display_name(&route.runtime),
                    recipient.name,
                    recipient.name
                ),
                RelayNoticeLevel::Info,
            );
        }
        if let Some(out) = self
            .host
            .out_of_plan(&route.runtime, own.as_ref().and_then(|r| r.account.as_deref()))
        {
            let why = format!(
                "{} did not reply on their own: {out} The message waits for their next run. To have it answered sooner, give {} another model on the Team screen.",
                recipient.name, recipient.name
            );
            self.notify(asker_run_id, asker_mission_id, why.clone(), RelayNoticeLevel::Info);
            self.host.note(&from.mission_id, &why);
            return StartOutcome::Refused;
        }

        self.host.notify(RelayUpdate::Starting {
            teammate_id: recipient.teammate_id.clone(),
            name: recipient.name.clone(),
            answering: from.mission_id.clone(),
            hop: origin.hop,
        });
        let runtime_prompt = if route.mode == "ask" || route.mode == "plan" {
            format!("{prompt} {}", read_only_reply_sentence())
        } else {
            prompt.to_owned()
        };
        let model = route
            .model
            .clone()
            .filter(|m| m != "account-default");
        let response = self.host.start(StartRequest {
            prompt: runtime_prompt,
            runtime: route.runtime,
            mode: route.mode,
            model,
            effort: own.as_ref().and_then(|r| r.effort.clone()),
            account: own.as_ref().and_then(|r| r.account.clone()),
            peer: recipient.clone(),
            follow_up_of,
            relay: origin.clone(),
        });
        self.host.notify(RelayUpdate::StartSettled {
            teammate_id: recipient.teammate_id.clone(),
        });

        match response {
            Err(_) => {
                self.notify(
                    asker_run_id,
                    asker_mission_id,
                    format!(
                        "{} could not reply on their own: the run could not be started.",
                        recipient.name
                    ),
                    RelayNoticeLevel::Warning,
                );
                StartOutcome::Refused
            }
            Ok(StartResult::Busy { pool }) => StartOutcome::Busy { pool },
            Ok(StartResult::Refused { message }) => {
                self.notify(
                    asker_run_id,
                    asker_mission_id,
                    format!(
                        "{} could not reply on their own: {message} The message waits for their next run.",
                        recipient.name
                    ),
                    RelayNoticeLevel::Warning,
                );
                StartOutcome::Refused
            }
            Ok(StartResult::Started {
                run_id,
                mission_id,
            }) => {
                self.host.assign_owner(&recipient.teammate_id, &mission_id);
                if onto_hub {
                    self.host
                        .remember_hub(&recipient.teammate_id, &mission_id, hub.is_none());
                }
                self.host.notify(RelayUpdate::MissionStarted {
                    run_id,
                    mission_id: mission_id.clone(),
                    teammate_id: recipient.teammate_id.clone(),
                    prompt: prompt.to_owned(),
                    started_by_hop: origin.hop,
                    hub_mission_id: if onto_hub { Some(mission_id.clone()) } else { None },
                });
                StartOutcome::Started { mission_id }
            }
        }
    }

    fn take_now_for(
        &self,
        recipient: &PeerRosterEntry,
        from_name: &str,
        text: &str,
        asker_run_id: &str,
        asker_mission_id: &str,
    ) {
        let shown: String = without_protocol_blocks(text)
            .trim()
            .chars()
            .take(2000)
            .collect();
        let steered = !shown.is_empty()
            && self.host.steer_work_of(
                &recipient.teammate_id,
                &format!(
                    "{from_name} sent you this while you were working, asking for it to be taken now: \"{shown}\" If it changes what you are doing, act on it before you go on. You will also get it as your next turn, to answer."
                ),
            );
        if steered {
            self.notify(
                asker_run_id,
                asker_mission_id,
                format!(
                    "{} was shown this part-way through their run, without stopping it. It is also their next turn.",
                    recipient.name
                ),
                RelayNoticeLevel::Info,
            );
            return;
        }
        self.interrupt_for(recipient, asker_run_id, asker_mission_id);
    }

    fn interrupt_for(
        &self,
        recipient: &PeerRosterEntry,
        asker_run_id: &str,
        asker_mission_id: &str,
    ) {
        if !self.host.may_interrupt() {
            self.notify(
                asker_run_id,
                asker_mission_id,
                format!(
                    "{} was asked to take this before finishing. Teammates interrupting each other is switched off in Settings, so it waits for their run to end.",
                    recipient.name
                ),
                RelayNoticeLevel::Info,
            );
            return;
        }
        if self.host.stop_work_of(&recipient.teammate_id) {
            self.notify(
                asker_run_id,
                asker_mission_id,
                format!(
                    "{} was stopped part-way so they can take this next. Whatever they had done is in their own conversation.",
                    recipient.name
                ),
                RelayNoticeLevel::Warning,
            );
        }
    }

    fn defer(&self, entry: DeferredReply, pool: bool) {
        let teammate_id = entry.recipient.teammate_id.clone();
        let recipient_name = entry.recipient.name.clone();
        let run_id = entry.asker_run_id.clone();
        let mission_id = entry.asker_mission_id.clone();
        {
            let mut st = self.state.lock().unwrap();
            if st.deferred.contains_key(&teammate_id) {
                if let Some(ex) = entry.exchange {
                    st.held_also.entry(teammate_id).or_default().push(ex);
                }
                return;
            }
            st.deferred.insert(teammate_id, entry);
        }
        let msg = if pool {
            format!("Every run slot is in use, so {recipient_name}'s reply starts when one frees up.")
        } else {
            format!("{recipient_name} is part-way through another mission. Their reply starts when it ends.")
        };
        self.notify(&run_id, &mission_id, msg, RelayNoticeLevel::Warning);
    }

    fn deliver_deferred(&self, teammate_id: &str) {
        let entry = {
            let mut st = self.state.lock().unwrap();
            st.deferred.remove(teammate_id)
        };
        let Some(entry) = entry else { return };
        let enabled = self.host.enabled();
        let cap = self.host.hop_cap();
        let root = entry.origin.root_mission_id.clone();
        let prior_hop = entry.origin.hop.saturating_sub(1);
        let (decision, spent) = {
            let st = self.state.lock().unwrap();
            let spent = st.spend_of(root.as_deref(), prior_hop);
            let decision = decide_relay(DecideRelayInput {
                enabled,
                defer: false,
                closes: false,
                hop: prior_hop,
                recipient_name: &entry.recipient.name,
                cap: Some(cap),
                spent: Some(spent),
            });
            (decision, spent)
        };
        if let RelayDecision::Skip { reason } = decision {
            self.notify(
                &entry.asker_run_id,
                &entry.asker_mission_id,
                reason.clone(),
                RelayNoticeLevel::Warning,
            );
            let held_others = {
                let mut st = self.state.lock().unwrap();
                st.held_also.remove(teammate_id).unwrap_or_default()
            };
            for other in held_others {
                self.notify_and_keep(&other.asker_run_id, &other.asker_mission_id, reason.clone());
            }
            return;
        }
        {
            let mut st = self.state.lock().unwrap();
            st.reserve(root.as_deref());
        }
        if let Some(waiting) = self.host.still_waiting(teammate_id)
            && !waiting
        {
            let mut st = self.state.lock().unwrap();
            st.release(root.as_deref());
            st.held_also.remove(teammate_id);
            return;
        }
        let prompt = entry.prompt_for(spent + 1);
        let outcome = self.start_for(
            &entry.recipient,
            &prompt,
            &entry.from,
            &entry.origin,
            &entry.asker_run_id,
            &entry.asker_mission_id,
        );
        if !matches!(outcome, StartOutcome::Started { .. }) {
            let mut st = self.state.lock().unwrap();
            st.release(root.as_deref());
        }
        match outcome {
            StartOutcome::Busy { .. } => {
                let mut st = self.state.lock().unwrap();
                st.deferred.insert(teammate_id.to_owned(), entry);
            }
            StartOutcome::Started { mission_id } => {
                let mut st = self.state.lock().unwrap();
                if let Some(ex) = entry.exchange {
                    st.exchanges.insert(mission_id.clone(), ex);
                }
                if let Some(others) = st.held_also.remove(teammate_id) {
                    st.also_waiting.insert(mission_id, others);
                }
            }
            StartOutcome::Refused => {
                let held_others = {
                    let mut st = self.state.lock().unwrap();
                    st.held_also.remove(teammate_id).unwrap_or_default()
                };
                for other in held_others {
                    self.notify_and_keep(
                        &other.asker_run_id,
                        &other.asker_mission_id,
                        format!(
                            "{} could not reply on their own. The message waits for their next run.",
                            entry.recipient.name
                        ),
                    );
                }
            }
        }
    }

    fn return_answer(&self, exchange: &OpenExchange, ended: &EndedMission) -> bool {
        let Some(ref asker_mission) = exchange.asker else {
            return false;
        };
        let Some(ref exchange_origin) = exchange.origin else {
            return false;
        };
        let Some(ref peer) = ended.peer else {
            return false;
        };
        let Some(said) = self.host.final_reply_of(&ended.mission_id) else {
            return false;
        };
        let words = without_protocol_blocks(&said);
        let words = words.trim();
        if words.is_empty() {
            return false;
        }
        let Some(asker) = self.host.peer_context_for(&exchange.asker_id) else {
            return false;
        };
        let text = bounded_share_text(&format!("{RETURNED_ANSWER} {words}"));
        let Some(message_id) = self.host.post_returned_answer(
            &peer.teammate_id,
            &peer.name,
            &ended.mission_id,
            &asker.teammate_id,
            &asker.name,
            &text,
        ) else {
            return false;
        };
        let root_id = exchange_origin
            .root_mission_id
            .clone()
            .unwrap_or_else(|| exchange.asker_mission_id.clone());
        let origin = RelayOrigin {
            hop: exchange_origin.hop + 1,
            root_mission_id: Some(root_id),
            last_mission_of: exchange_origin.with_last_mission(&peer.teammate_id, &ended.mission_id),
            relayed_to: exchange_origin.relayed_to.clone(),
            answering: vec![message_id],
        };
        let read_by_person = origin.first_teammate_id() == Some(asker.teammate_id.as_str());
        let prompt = returned_answer_prompt(&peer.name, read_by_person);
        {
            let mut st = self.state.lock().unwrap();
            st.reserve(origin.root_mission_id.as_deref());
        }
        let result = self.start_for(
            &asker,
            &prompt,
            asker_mission,
            &origin,
            &exchange.asker_run_id,
            &exchange.asker_mission_id,
        );
        if !matches!(result, StartOutcome::Started { .. }) {
            let mut st = self.state.lock().unwrap();
            st.release(origin.root_mission_id.as_deref());
        }
        if let StartOutcome::Busy { pool } = result {
            self.defer(
                DeferredReply {
                    recipient: asker.clone(),
                    prompt,
                    prompt_kind: DeferredPromptKind::Fixed,
                    from: asker_mission.clone(),
                    origin,
                    asker_run_id: exchange.asker_run_id.clone(),
                    asker_mission_id: exchange.asker_mission_id.clone(),
                    exchange: None,
                },
                pool,
            );
        }
        let msg = if matches!(result, StartOutcome::Refused) {
            format!(
                "{} answered in their own conversation without writing back. The answer was brought to {}'s messages, to read on their next run.",
                exchange.recipient_name, asker.name
            )
        } else {
            format!(
                "{} answered in their own conversation without writing back, so the answer was brought back to {}.",
                exchange.recipient_name, asker.name
            )
        };
        self.notify_and_keep(&exchange.asker_run_id, &exchange.asker_mission_id, msg);
        true
    }

    fn close_meeting(&self, mut meeting: Meeting, from: &SharingMission) {
        {
            let mut st = self.state.lock().unwrap();
            st.meetings.remove(&meeting.asker_mission_id);
        }
        let Some(asker) = self.host.peer_context_for(&meeting.asker_id) else {
            return;
        };
        if meeting.answered.is_empty() {
            self.notify(
                &meeting.asker_run_id,
                &meeting.asker_mission_id,
                "Nobody replied. Your message waits with them for their next run.".to_owned(),
                RelayNoticeLevel::Warning,
            );
            return;
        }
        let mut silent = std::mem::take(&mut meeting.silent);
        silent.extend(meeting.awaiting_names());
        let origin = RelayOrigin {
            hop: meeting.origin.hop + 1,
            root_mission_id: meeting.origin.root_mission_id.clone(),
            last_mission_of: meeting
                .origin
                .with_last_mission(&from.peer.teammate_id, &from.mission_id),
            relayed_to: meeting.origin.relayed_to.clone(),
            answering: Vec::new(),
        };
        let cap = self.host.hop_cap();
        let spent = {
            let mut st = self.state.lock().unwrap();
            let spent = st.spend_of(origin.root_mission_id.as_deref(), origin.hop);
            if spent < cap {
                st.reserve(origin.root_mission_id.as_deref());
            }
            spent
        };
        if spent >= cap {
            let noun = if cap == 1 { "reply" } else { "replies" };
            self.notify_and_keep(
                &meeting.asker_run_id,
                &meeting.asker_mission_id,
                format!("Stopped after {cap} automatic {noun}. The replies wait for your next run."),
            );
            return;
        }
        let read_by_person = origin.first_teammate_id() == Some(asker.teammate_id.as_str());
        let prompt = meeting_prompt(&meeting.answered, &silent, read_by_person);
        let outcome = self.start_for(
            &asker,
            &prompt,
            from,
            &origin,
            &meeting.asker_run_id,
            &meeting.asker_mission_id,
        );
        if !matches!(outcome, StartOutcome::Started { .. }) {
            let mut st = self.state.lock().unwrap();
            st.release(origin.root_mission_id.as_deref());
        }
        if let StartOutcome::Busy { pool } = outcome {
            self.defer(
                DeferredReply {
                    recipient: asker,
                    prompt,
                    prompt_kind: DeferredPromptKind::Fixed,
                    from: from.clone(),
                    origin,
                    asker_run_id: meeting.asker_run_id,
                    asker_mission_id: meeting.asker_mission_id,
                    exchange: None,
                },
                pool,
            );
        }
    }

    pub fn on_shared(&self, mission: &SharingMission, posted: &[RelayedMessage]) {
        if posted.is_empty() {
            return;
        }
        if matches!(mission.started_by, Some(MissionStarter::Mcp)) {
            let message =
                "Shares from another app are visible in Locust. No teammate was started automatically."
                    .to_owned();
            self.host.note(&mission.mission_id, &message);
            self.notify(&mission.run_id, &mission.mission_id, message, RelayNoticeLevel::Info);
            return;
        }
        let enabled = self.host.enabled();
        let cap = self.host.hop_cap();
        let hop = mission.relay.as_ref().map_or(0, |r| r.hop);
        let shared = if mission
            .relay
            .as_ref()
            .and_then(|r| r.root_mission_id.as_ref())
            .is_none()
        {
            self.host.exchange_of(&mission.mission_id)
        } else {
            None
        };
        let root = mission
            .relay
            .as_ref()
            .and_then(|r| r.root_mission_id.clone())
            .or(shared)
            .unwrap_or_else(|| mission.mission_id.clone());

        // Handle open meeting reply if this mission is answering one.
        let mut held: HashSet<String> = HashSet::new();
        enum MeetingAction {
            None,
            NotifyWaiting {
                asker_run_id: String,
                asker_mission_id: String,
                rest: Vec<String>,
            },
            Close(Meeting),
        }
        let meeting_action = {
            let mut st = self.state.lock().unwrap();
            if let Some(asker_mission_id) = st.answering.get(&mission.mission_id).cloned()
                && let Some(meeting) = st.meetings.get_mut(&asker_mission_id)
                && meeting.has_awaiting(&mission.peer.teammate_id)
                && posted.iter().any(|m| m.to_teammate_id == meeting.asker_id)
            {
                meeting.take_awaiting(&mission.peer.teammate_id);
                meeting.answered.push(mission.peer.name.clone());
                held.insert(meeting.asker_id.clone());
                if !meeting.awaiting.is_empty() {
                    MeetingAction::NotifyWaiting {
                        asker_run_id: meeting.asker_run_id.clone(),
                        asker_mission_id: meeting.asker_mission_id.clone(),
                        rest: meeting.awaiting_names(),
                    }
                } else {
                    let removed = st.meetings.remove(&asker_mission_id).unwrap();
                    MeetingAction::Close(removed)
                }
            } else {
                MeetingAction::None
            }
        };
        match meeting_action {
            MeetingAction::None => {}
            MeetingAction::NotifyWaiting {
                asker_run_id,
                asker_mission_id,
                rest,
            } => {
                self.notify(
                    &asker_run_id,
                    &asker_mission_id,
                    format!(
                        "{} replied. Still waiting on {}.",
                        mission.peer.name,
                        rest.join(", ")
                    ),
                    RelayNoticeLevel::Info,
                );
            }
            MeetingAction::Close(meeting) => {
                self.close_meeting(meeting, mission);
            }
        }

        // Answer clears open 1:1 exchange silence waiters.
        {
            let mut st = self.state.lock().unwrap();
            if let Some(waiting) = st.exchanges.get(&mission.mission_id)
                && posted.iter().any(|m| m.to_teammate_id == waiting.asker_id)
            {
                st.exchanges.remove(&mission.mission_id);
            }
            if let Some(others) = st.also_waiting.remove(&mission.mission_id) {
                let still: Vec<_> = others
                    .into_iter()
                    .filter(|other| !posted.iter().any(|m| m.to_teammate_id == other.asker_id))
                    .collect();
                if !still.is_empty() {
                    st.also_waiting.insert(mission.mission_id.clone(), still);
                }
            }
        }

        let mut seen: HashSet<String> = HashSet::new();
        let mut started: Vec<StartedReply> = Vec::new();
        let base_last_mission = match &mission.relay {
            Some(r) => r.with_last_mission(&mission.peer.teammate_id, &mission.mission_id),
            None => vec![(mission.peer.teammate_id.clone(), mission.mission_id.clone())],
        };
        let earlier_relayed = mission
            .relay
            .as_ref()
            .map(|r| r.relayed_to.clone())
            .unwrap_or_default();

        for message in posted {
            let recipient_id = message.to_teammate_id.clone();
            if !seen.insert(recipient_id.clone()) || held.contains(&recipient_id) {
                continue;
            }
            let read_by_person =
                base_last_mission.first().map(|(id, _)| id.as_str()) == Some(recipient_id.as_str());
            let closes = !read_by_person
                && !message.wants_answer
                && says_nothing_is_needed(&message.text);

            let (decision, spent) = {
                let mut st = self.state.lock().unwrap();
                let spent = st.spend_of(Some(&root), hop);
                let decision = decide_relay(DecideRelayInput {
                    enabled,
                    defer: message.defer,
                    closes,
                    hop,
                    recipient_name: &message.to_name,
                    cap: Some(cap),
                    spent: Some(spent),
                });
                if matches!(decision, RelayDecision::Start { .. }) {
                    st.reserve(Some(&root));
                }
                (decision, spent)
            };

            let next_hop = match decision {
                RelayDecision::Skip { reason } => {
                    self.host.note(&mission.mission_id, &reason);
                    self.notify(&mission.run_id, &mission.mission_id, reason, RelayNoticeLevel::Warning);
                    continue;
                }
                RelayDecision::Start { hop: h } => h,
            };

            let Some(recipient) = self.host.peer_context_for(&recipient_id) else {
                let mut st = self.state.lock().unwrap();
                st.release(Some(&root));
                self.notify(
                    &mission.run_id,
                    &mission.mission_id,
                    format!("{} is no longer on the roster; nothing was started.", message.to_name),
                    RelayNoticeLevel::Warning,
                );
                continue;
            };

            let briefed_before = earlier_relayed.iter().any(|id| id == &recipient_id);
            let mut next_relayed = earlier_relayed.clone();
            if next_hop > 1 && !briefed_before {
                next_relayed.push(recipient_id.clone());
            }
            let answering_ids: Vec<String> = posted
                .iter()
                .filter(|entry| entry.to_teammate_id == recipient_id)
                .map(|entry| entry.message_id.clone())
                .collect();
            let origin = RelayOrigin {
                hop: next_hop,
                root_mission_id: Some(root.clone()),
                last_mission_of: base_last_mission.clone(),
                relayed_to: next_relayed,
                answering: answering_ids,
            };
            let prompt = relay_prompt(&RelayPromptInput {
                sender: &mission.peer,
                recipient: &recipient,
                hop: origin.hop,
                cap: Some(cap),
                read_by_person,
                reply_number: Some(spent + 1),
                briefed_before,
            });
            let result = self.start_for(
                &recipient,
                &prompt,
                mission,
                &origin,
                &mission.run_id,
                &mission.mission_id,
            );
            if !matches!(result, StartOutcome::Started { .. }) {
                let mut st = self.state.lock().unwrap();
                st.release(Some(&root));
            }
            match result {
                StartOutcome::Busy { pool } => {
                    self.defer(
                        DeferredReply {
                            recipient: recipient.clone(),
                            prompt,
                            prompt_kind: DeferredPromptKind::Relay {
                                sender: mission.peer.clone(),
                                cap,
                                read_by_person,
                                briefed_before,
                            },
                            from: mission.clone(),
                            origin: origin.clone(),
                            asker_run_id: mission.run_id.clone(),
                            asker_mission_id: mission.mission_id.clone(),
                            exchange: Some(OpenExchange {
                                asker_run_id: mission.run_id.clone(),
                                asker_mission_id: mission.mission_id.clone(),
                                asker_id: mission.peer.teammate_id.clone(),
                                recipient_name: recipient.name.clone(),
                                wants_answer: message.wants_answer,
                                asker: Some(mission.clone()),
                                origin: Some(origin),
                            }),
                        },
                        pool,
                    );
                    if message.urgent {
                        self.take_now_for(
                            &recipient,
                            &mission.peer.name,
                            &message.text,
                            &mission.run_id,
                            &mission.mission_id,
                        );
                    }
                }
                StartOutcome::Started { mission_id } => {
                    started.push(StartedReply {
                        teammate_id: recipient_id,
                        name: recipient.name.clone(),
                        mission_id,
                        wants_answer: message.wants_answer,
                        origin,
                    });
                }
                StartOutcome::Refused => {}
            }
        }

        if started.len() >= 2 {
            let names: Vec<String> = started.iter().map(|s| s.name.clone()).collect();
            let opened = Meeting {
                asker_id: mission.peer.teammate_id.clone(),
                asker_run_id: mission.run_id.clone(),
                asker_mission_id: mission.mission_id.clone(),
                origin: RelayOrigin {
                    hop,
                    root_mission_id: Some(root),
                    last_mission_of: base_last_mission,
                    relayed_to: earlier_relayed,
                    answering: Vec::new(),
                },
                awaiting: started
                    .iter()
                    .map(|s| (s.teammate_id.clone(), s.name.clone()))
                    .collect(),
                answered: Vec::new(),
                silent: Vec::new(),
            };
            {
                let mut st = self.state.lock().unwrap();
                st.meetings.insert(mission.mission_id.clone(), opened);
                for s in &started {
                    st.answering
                        .insert(s.mission_id.clone(), mission.mission_id.clone());
                }
            }
            self.notify(
                &mission.run_id,
                &mission.mission_id,
                format!("Waiting on {} to reply before your next turn.", names.join(", ")),
                RelayNoticeLevel::Info,
            );
        } else {
            let mut st = self.state.lock().unwrap();
            for s in started {
                st.exchanges.insert(
                    s.mission_id,
                    OpenExchange {
                        asker_run_id: mission.run_id.clone(),
                        asker_mission_id: mission.mission_id.clone(),
                        asker_id: mission.peer.teammate_id.clone(),
                        recipient_name: s.name,
                        wants_answer: s.wants_answer,
                        asker: Some(mission.clone()),
                        origin: Some(s.origin),
                    },
                );
            }
        }
    }

    pub fn on_run_ended(&self, mission: &EndedMission) {
        let (exchange, others) = {
            let mut st = self.state.lock().unwrap();
            let ex = st.exchanges.remove(&mission.mission_id);
            let other = st.also_waiting.remove(&mission.mission_id).unwrap_or_default();
            (ex, other)
        };
        let ended = self.host.how_ended(&mission.mission_id);
        for open in exchange.into_iter().chain(others.into_iter()) {
            let returned =
                ended.is_none() && open.wants_answer && self.return_answer(&open, mission);
            if !returned {
                let msg = match &ended {
                    None => format!(
                        "{} finished without writing back. Anything they said is in their own conversation.",
                        open.recipient_name
                    ),
                    Some(why) => format!(
                        "{} stopped before writing back: {why}. Anything they said is in their own conversation.",
                        open.recipient_name
                    ),
                };
                self.notify_and_keep(&open.asker_run_id, &open.asker_mission_id, msg);
            }
        }

        let teammate_id = mission.peer.as_ref().map(|p| p.teammate_id.clone());
        enum EndedMeetingAction {
            None,
            NotifyWaiting {
                asker_run_id: String,
                asker_mission_id: String,
                name: String,
                rest: Vec<String>,
            },
            Close {
                meeting: Meeting,
                peer: PeerRosterEntry,
            },
        }
        let meeting_action = {
            let mut st = self.state.lock().unwrap();
            if let Some(asker_mission_id) = st.answering.remove(&mission.mission_id)
                && let Some(ref peer) = mission.peer
                && let Some(meeting) = st.meetings.get_mut(&asker_mission_id)
                && meeting.has_awaiting(&peer.teammate_id)
            {
                meeting.take_awaiting(&peer.teammate_id);
                meeting.silent.push(peer.name.clone());
                if !meeting.awaiting.is_empty() {
                    EndedMeetingAction::NotifyWaiting {
                        asker_run_id: meeting.asker_run_id.clone(),
                        asker_mission_id: meeting.asker_mission_id.clone(),
                        name: peer.name.clone(),
                        rest: meeting.awaiting_names(),
                    }
                } else {
                    let removed = st.meetings.remove(&asker_mission_id).unwrap();
                    EndedMeetingAction::Close {
                        meeting: removed,
                        peer: peer.clone(),
                    }
                }
            } else {
                EndedMeetingAction::None
            }
        };
        match meeting_action {
            EndedMeetingAction::None => {}
            EndedMeetingAction::NotifyWaiting {
                asker_run_id,
                asker_mission_id,
                name,
                rest,
            } => {
                let status = match &ended {
                    None => "finished without replying".to_owned(),
                    Some(why) => format!("stopped before replying: {why}"),
                };
                self.notify(
                    &asker_run_id,
                    &asker_mission_id,
                    format!("{name} {status}. Still waiting on {}.", rest.join(", ")),
                    RelayNoticeLevel::Warning,
                );
            }
            EndedMeetingAction::Close { meeting, peer } => {
                self.close_meeting(
                    meeting,
                    &SharingMission {
                        run_id: String::new(),
                        mission_id: mission.mission_id.clone(),
                        runtime: "codex".to_owned(),
                        sandbox: "read-only".to_owned(),
                        model: None,
                        peer,
                        relay: mission.relay.clone(),
                        started_by: None,
                    },
                );
            }
        }

        if let Some(ref id) = teammate_id {
            self.deliver_deferred(id);
        }
        let other_deferred: Vec<String> = {
            let st = self.state.lock().unwrap();
            st.deferred
                .keys()
                .filter(|k| teammate_id.as_deref() != Some(k.as_str()))
                .cloned()
                .collect()
        };
        for other in other_deferred {
            self.deliver_deferred(&other);
        }
    }
}

enum StartOutcome {
    Started { mission_id: String },
    Busy { pool: bool },
    Refused,
}

// ---------------------------------------------------------------------------
// Sidebar nesting (`shared/nested-conversations.ts`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NestedRow {
    pub mission_id: String,
    pub member_ids: Vec<String>,
}

/// Which sidebar row each relayed row sits under (`nestedUnder`):
/// one level deep, never under an already-nested row, never pointing at missing rows or itself.
pub fn nested_under<'a, F>(rows: &'a [NestedRow], sent_from: F) -> HashMap<String, String>
where
    F: Fn(&'a NestedRow) -> Option<String>,
{
    let mut owner: HashMap<&str, &str> = HashMap::new();
    for row in rows {
        if row.member_ids.is_empty() {
            owner.insert(&row.mission_id, &row.mission_id);
        } else {
            for id in &row.member_ids {
                owner.insert(id, &row.mission_id);
            }
        }
    }
    let mut wanted: HashMap<String, String> = HashMap::new();
    for row in rows {
        let Some(from) = sent_from(row) else {
            continue;
        };
        let Some(&parent) = owner.get(from.as_str()) else {
            continue;
        };
        if parent != row.mission_id {
            wanted.insert(row.mission_id.clone(), parent.to_owned());
        }
    }
    let mut nested = HashMap::new();
    for (child, parent) in &wanted {
        if !wanted.contains_key(parent) {
            nested.insert(child.clone(), parent.clone());
        }
    }
    nested
}

// ---------------------------------------------------------------------------
// Exchange overview (`renderer/src/exchange.ts`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExchangePeerMessage {
    pub message_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExchangeMission {
    pub mission_id: String,
    pub runtime: String,
    pub model: String,
    pub cost: Option<RunCost>,
    pub peer_messages: Vec<ExchangePeerMessage>,
    pub started_by_hop: Option<usize>,
    pub teammate_id: Option<String>,
    pub teammate_name: Option<String>,
    pub live: bool,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExchangeParticipant {
    pub teammate_id: String,
    pub name: String,
    pub runtime: String,
    pub model: String,
    pub mission_ids: Vec<String>,
    pub live: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExchangeOverview {
    pub participants: Vec<ExchangeParticipant>,
    pub hops: usize,
    pub cost: Option<RunCost>,
    pub live_run_ids: Vec<String>,
    pub mission_ids: Vec<String>,
}

/// Extract a mission's RunCost from its normalized events (`missionCost`).
pub fn mission_cost_from_events(events: &[Value]) -> Option<RunCost> {
    let mut cost: Option<RunCost> = None;
    for ev in events {
        if ev.get("type").and_then(Value::as_str) != Some("run.completed") {
            continue;
        }
        let Some(usage) = ev.get("payload").and_then(|p| p.get("usage")) else {
            continue;
        };
        let (spend, plan) = money_of_usage(usage);
        let input_tokens = usage.get("inputTokens").and_then(Value::as_u64);
        let output_tokens = usage.get("outputTokens").and_then(Value::as_u64);
        if spend.usd.is_none()
            && spend.premium_requests.is_none()
            && input_tokens.is_none()
            && output_tokens.is_none()
            && !plan
        {
            continue;
        }
        let slot = cost.get_or_insert_with(RunCost::default);
        if let Some(u) = spend.usd {
            slot.usd = Some(slot.usd.unwrap_or(0.0) + u);
        }
        if let Some(pr) = spend.premium_requests {
            slot.premium_requests = Some(slot.premium_requests.unwrap_or(0.0) + pr);
        }
        if let Some(it) = input_tokens {
            slot.input_tokens = Some(slot.input_tokens.unwrap_or(0) + it);
        }
        if let Some(ot) = output_tokens {
            slot.output_tokens = Some(slot.output_tokens.unwrap_or(0) + ot);
        }
        slot.plan |= plan;
    }
    cost
}

fn sum_exchange_costs(costs: impl Iterator<Item = Option<RunCost>>) -> Option<RunCost> {
    let mut out = RunCost::default();
    let mut any = false;
    for c in costs.flatten() {
        any = true;
        if let Some(u) = c.usd {
            out.usd = Some(out.usd.unwrap_or(0.0) + u);
        }
        if let Some(pr) = c.premium_requests {
            out.premium_requests = Some(out.premium_requests.unwrap_or(0.0) + pr);
        }
        if let Some(it) = c.input_tokens {
            out.input_tokens = Some(out.input_tokens.unwrap_or(0) + it);
        }
        if let Some(ot) = c.output_tokens {
            out.output_tokens = Some(out.output_tokens.unwrap_or(0) + ot);
        }
        out.plan |= c.plan;
    }
    any.then_some(out)
}

/// Every mission reachable from `mission_id` through shared workroom messages (`exchangeOf`).
pub fn exchange_of(mission_id: &str, missions: &[ExchangeMission]) -> Option<ExchangeOverview> {
    exchange_across(&[mission_id], missions)
}

/// Walk from several seed missions at once as one exchange (`exchangeAcross`).
pub fn exchange_across(roots: &[&str], missions: &[ExchangeMission]) -> Option<ExchangeOverview> {
    let by_id: HashMap<&str, usize> = missions
        .iter()
        .enumerate()
        .map(|(i, m)| (m.mission_id.as_str(), i))
        .collect();
    let mut queue: Vec<usize> = roots
        .iter()
        .filter_map(|r| by_id.get(r).copied())
        .collect();
    if queue.is_empty() {
        return None;
    }

    let mut by_message: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, m) in missions.iter().enumerate() {
        for msg in &m.peer_messages {
            by_message.entry(msg.message_id.as_str()).or_default().push(i);
        }
    }

    let mut reached_indices: Vec<usize> = Vec::new();
    let mut reached_set: HashSet<usize> = HashSet::new();
    while let Some(idx) = queue.pop() {
        if !reached_set.insert(idx) {
            continue;
        }
        reached_indices.push(idx);
        for msg in &missions[idx].peer_messages {
            if let Some(holders) = by_message.get(msg.message_id.as_str()) {
                for &other in holders {
                    if !reached_set.contains(&other) {
                        queue.push(other);
                    }
                }
            }
        }
    }

    let talked = reached_indices
        .iter()
        .any(|&i| !missions[i].peer_messages.is_empty());
    if !talked {
        return None;
    }

    let mut part_order: Vec<String> = Vec::new();
    let mut part_map: HashMap<String, ExchangeParticipant> = HashMap::new();
    let mut hops = 0usize;
    let mut live_run_ids = Vec::new();
    let mut mission_ids = Vec::new();

    for &idx in &reached_indices {
        let m = &missions[idx];
        mission_ids.push(m.mission_id.clone());
        if let Some(h) = m.started_by_hop {
            hops = hops.max(h);
        }
        if m.live
            && let Some(ref run_id) = m.run_id
        {
            live_run_ids.push(run_id.clone());
        }
        let key = m
            .teammate_id
            .clone()
            .unwrap_or_else(|| format!("nobody:{}", m.mission_id));
        let name = m.teammate_name.clone().unwrap_or_else(|| {
            m.teammate_id
                .clone()
                .unwrap_or_else(|| "Nobody".to_owned())
        });
        if let Some(entry) = part_map.get_mut(&key) {
            entry.mission_ids.push(m.mission_id.clone());
            entry.runtime = m.runtime.clone();
            entry.model = m.model.clone();
            entry.live |= m.live;
        } else {
            part_order.push(key.clone());
            part_map.insert(
                key.clone(),
                ExchangeParticipant {
                    teammate_id: key,
                    name,
                    runtime: m.runtime.clone(),
                    model: m.model.clone(),
                    mission_ids: vec![m.mission_id.clone()],
                    live: m.live,
                },
            );
        }
    }

    let participants = part_order
        .into_iter()
        .filter_map(|k| part_map.remove(&k))
        .collect();
    let cost = sum_exchange_costs(reached_indices.iter().map(|&i| missions[i].cost.clone()));

    Some(ExchangeOverview {
        participants,
        hops,
        cost,
        live_run_ids,
        mission_ids,
    })
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
