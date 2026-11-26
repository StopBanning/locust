//! What a paired phone may see and do (Electron `main/index.ts`'s `phoneCalls` and `phoneSend`, 0.826, 0.878): the
//! cards waiting on the person, the conversations and their turns, a card answered, and a message sent as the
//! person's next turn.
//!
//! The rules are here and tested with a fake [`PhoneEngine`]; [`NativePhoneEngine`] is the engine itself, reached
//! the way the window reaches it (`engine_api::api()`, the shared engine when there is one). A card is answered as the
//! window's card answers it (`api().decide`, approve once or deny): a paired phone is the person, as in Electron, and
//! `approvals_from_the_window_tests.rs` names this file for that reason. A message is sent through the engine's send
//! exactly as the door sends (`door.rs`'s `Engine::start`), with no change to the door.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::approval::{Kind, Mode, Request};
use crate::engine::Teammate;
use crate::phone_host::{
    KeyedOnce, PhoneCalls, PhoneConversation, PhoneMission, PhonePlain, PhoneScoutCard, PhoneSent, PhoneState,
    PhoneTurn, PhoneWaiting, phone_conversations, phone_turns,
};

pub const ALREADY_ON_ITS_WAY: &str = "That message is already on its way.";
pub const STILL_WORKING: &str = "This conversation is still working. Send when it finishes.";
pub const CANNOT_READ: &str = "This conversation could not be read in Locust. Continue it on the computer. Nothing was sent.";

/// Who a follow-up goes to and how: the owner's teammate in its saved mode, or, for a conversation with no teammate,
/// nobody on the turn's own agent, model and mode.
#[derive(Clone, Debug, PartialEq)]
pub struct PhoneRoute {
    pub teammate: Teammate,
    pub mode: Mode,
    /// The turn's own agent, kept for a conversation with no teammate.
    pub runtime: Option<String>,
}

/// The engine as the phone needs it.
pub trait PhoneEngine: Send + Sync {
    /// Every recent turn, in both folders.
    fn missions(&self) -> Vec<PhoneMission>;
    /// The turns running now.
    fn live(&self) -> Vec<String>;
    /// Changes whenever a turn is written: the conversations are read again only then.
    fn change_mark(&self) -> String;
    /// The teammate who answered a turn, by name.
    fn name_of(&self, mission_id: &str) -> Option<String>;
    fn waiting(&self) -> Vec<Request>;
    /// Approve once or deny, as the window's card does. True when the card was still waiting.
    fn answer_card(&self, approval_id: &str, approve: bool) -> bool;
    /// The conversation's newest turn.
    fn newest(&self, mission_id: &str) -> Option<String>;
    fn running(&self, mission_id: &str) -> bool;
    fn route_of(&self, newest: &str) -> Option<PhoneRoute>;
    fn start(&self, route: PhoneRoute, newest: &str, text: &str) -> Result<(), String>;
    fn scout_cards(&self) -> Option<Vec<PhoneScoutCard>> {
        None
    }
    fn dismiss_scout(&self, _card_id: &str) -> Option<bool> {
        None
    }
}

/// `shared/approval-data.ts::exactLine`: the first line of what is asked, when the summary does not already say it,
/// at most 200 characters, secrets scrubbed.
pub fn exact_line(summary: &str, detail: &str) -> Option<String> {
    let exact = detail.split('\n').map(str::trim).find(|line| !line.is_empty())?;
    if summary.contains(exact) {
        return None;
    }
    // JavaScript's slice counts UTF-16 units.
    let units: Vec<u16> = exact.encode_utf16().collect();
    let clipped = if units.len() > 200 {
        format!("{}…", String::from_utf16_lossy(&units[..200]))
    } else {
        exact.to_owned()
    };
    Some(crate::secrets::scrub_secrets(&clipped).0)
}

/// One waiting card as the phone shows it: who, what, the exact line it allows, and a command said plainly.
pub fn phone_waiting(request: &Request, name: Option<String>) -> PhoneWaiting {
    let exact = (request.kind != Kind::Question).then(|| exact_line(&request.summary, &request.detail)).flatten();
    let plain = (request.kind == Kind::Command)
        .then(|| crate::command_plain::plain_command(&request.detail))
        .flatten()
        .map(|plain| PhonePlain { said: crate::secrets::scrub_secrets(&plain.said).0, careful: plain.careful });
    PhoneWaiting {
        approval_id: request.approval_id.clone(),
        mission_id: request.mission_id.clone(),
        summary: request.summary.clone(),
        requested_at: request.requested_at.clone(),
        name,
        exact,
        plain,
    }
}

type History = (Vec<PhoneConversation>, BTreeMap<String, Vec<PhoneMission>>, Vec<String>);

pub struct EnginePhoneCalls {
    engine: Arc<dyn PhoneEngine>,
    history: KeyedOnce<Arc<History>>,
    sending: Mutex<HashSet<String>>,
    minute: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl EnginePhoneCalls {
    pub fn new(engine: Arc<dyn PhoneEngine>, now_ms: Arc<dyn Fn() -> i64 + Send + Sync>) -> Self {
        Self { engine, history: KeyedOnce::new(), sending: Mutex::new(HashSet::new()), minute: now_ms }
    }

    /// Read once per change: a turn written, a turn starting or ending, or a new minute (the page's clocks).
    fn history(&self) -> Arc<History> {
        let live = self.engine.live();
        let key = format!("{}|{}|{}", self.engine.change_mark(), live.join(","), (self.minute)() / 60_000);
        let built: Result<_, ()> = self.history.get_or_build(&key, || {
            let missions = self.engine.missions();
            let names = |id: &str| self.engine.name_of(id);
            let (list, chains) = phone_conversations(&missions, &live, Some(&names));
            Ok(Arc::new((list, chains, live.clone())))
        });
        built.unwrap_or_default()
    }
}

impl PhoneCalls for EnginePhoneCalls {
    fn state(&self) -> PhoneState {
        let waiting = self
            .engine
            .waiting()
            .iter()
            .map(|request| phone_waiting(request, self.engine.name_of(&request.mission_id)))
            .collect();
        PhoneState { waiting, conversations: self.history().0.clone(), scout: self.engine.scout_cards() }
    }

    fn turns(&self, mission_id: &str) -> Option<Vec<PhoneTurn>> {
        let history = self.history();
        let names = |id: &str| self.engine.name_of(id);
        phone_turns(&history.1, mission_id, &history.2, Some(&names))
    }

    fn answer_card(&self, approval_id: &str, approve: bool) -> bool {
        self.engine.answer_card(approval_id, approve)
    }

    /// The phone's message as the person's next turn (`phoneSend`).
    fn send(&self, mission_id: &str, text: &str) -> PhoneSent {
        let err = |message: &str| PhoneSent::Err { message: message.to_owned() };
        let Some(newest) = self.engine.newest(mission_id) else {
            return err(CANNOT_READ);
        };
        // Two presses, or the phone and the window at once, must not both continue the same turn.
        if self.sending.lock().unwrap_or_else(|e| e.into_inner()).contains(&newest) {
            return err(ALREADY_ON_ITS_WAY);
        }
        if self.engine.running(&newest) {
            return err(STILL_WORKING);
        }
        if !self.sending.lock().unwrap_or_else(|e| e.into_inner()).insert(newest.clone()) {
            return err(ALREADY_ON_ITS_WAY);
        }
        let sent = match self.engine.route_of(&newest) {
            None => err(CANNOT_READ),
            Some(route) => match self.engine.start(route, &newest, text) {
                Ok(()) => PhoneSent::Ok,
                Err(message) => PhoneSent::Err { message },
            },
        };
        self.sending.lock().unwrap_or_else(|e| e.into_inner()).remove(&newest);
        sent
    }

    fn dismiss_scout(&self, card_id: &str) -> Option<bool> {
        self.engine.dismiss_scout(card_id)
    }
}

// ---- notes written on a phone while this computer was off (0.837) ------------------------------------------------

/// What a note's send came to, in the words the phone link keeps (index.ts `notes.deliver`: `sent.ok ? 'sent' :
/// sent.why`). Busy only for a send refused before anything was started (`phoneSend`'s two `why: 'busy'`), so the link
/// tries it again later; any other failure is held for the phone to decide, since whether anything was started is not
/// known, and a note is never sent twice.
pub fn note_delivery_of(sent: &PhoneSent) -> crate::phone_anywhere::NoteDelivery {
    use crate::phone_anywhere::NoteDelivery;
    match sent {
        PhoneSent::Ok => NoteDelivery::Sent,
        PhoneSent::Err { message } if message == ALREADY_ON_ITS_WAY || message == STILL_WORKING => NoteDelivery::Busy,
        PhoneSent::Err { .. } => NoteDelivery::Failed,
    }
}

/// The phone link's `notes` (index.ts: `deliver: async (missionId, text) => { const sent = await phoneSend(missionId,
/// text); return sent.ok ? 'sent' : sent.why }`): each note goes through the very path a message from the phone takes.
pub struct PhoneNotes {
    calls: Arc<dyn PhoneCalls>,
}

impl PhoneNotes {
    pub fn new(calls: Arc<dyn PhoneCalls>) -> Self {
        Self { calls }
    }
}

impl crate::phone_anywhere::NotesCallbacks for PhoneNotes {
    fn deliver(&self, mission_id: &str, text: &str) -> crate::phone_anywhere::NoteDelivery {
        note_delivery_of(&self.calls.send(mission_id, text))
    }
}

// ---- the engine itself -------------------------------------------------------------------------------------------

/// How many recent turns the phone reads to make its conversations.
const RECENT_TURNS: usize = 300;

/// A ledger's records as the phone's turn: door_tools' `Turn` plus how it continues its parent.
pub fn phone_mission_of(records: &[Value], live: bool) -> Option<PhoneMission> {
    let turn = crate::door_tools::Turn::from_records(records)?;
    let meta = records.iter().find(|r| r["recordType"] == "mission.created").map(|r| &r["metadata"])?;
    let text = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    let created_at = meta["createdAt"].as_str().unwrap_or_default().to_owned();
    Some(PhoneMission {
        phase: if live { "running".to_owned() } else { turn.phase().to_owned() },
        runtime: turn.runtime.clone().unwrap_or_default(),
        last_updated_at: if turn.updated.is_empty() { created_at.clone() } else { turn.updated.clone() },
        created_at,
        continues_from_mission_id: text(&meta["continuesFrom"]["missionId"]),
        continues_from_reason: text(&meta["continuesFrom"]["reason"]),
        started_by_kind: text(&turn.started_by["kind"]).or_else(|| (meta["side"] == true).then(|| "side".to_owned())),
        mission_id: turn.mission_id,
        prompt: turn.prompt,
        events: turn.events,
    })
}

pub struct NativePhoneEngine {
    store: crate::store::Store,
}

impl NativePhoneEngine {
    pub fn new(store: crate::store::Store) -> Self {
        Self { store }
    }

    fn records(mission_id: &str) -> Option<Vec<Value>> {
        let path = crate::native::find_ledger(mission_id)?;
        Some(crate::ledger::Tail::new(&path, 0).poll())
    }

    fn saved_mode(&self, teammate_id: &str) -> Option<Mode> {
        let team = crate::people::team(&self.store).ok()?;
        team["teammates"]
            .as_array()?
            .iter()
            .find(|r| r["teammateId"] == teammate_id)
            .and_then(|r| r["route"]["mode"].as_str())
            .and_then(Mode::parse)
    }
}

impl PhoneEngine for NativePhoneEngine {
    fn missions(&self) -> Vec<PhoneMission> {
        let live: HashSet<String> = self.live().into_iter().collect();
        let Some(listed) = crate::native::list_recent(RECENT_TURNS, true) else {
            return Vec::new();
        };
        listed
            .summaries
            .iter()
            .filter_map(|s| phone_mission_of(&Self::records(&s.mission_id)?, live.contains(&s.mission_id)))
            .collect()
    }

    fn live(&self) -> Vec<String> {
        crate::engine_api::api().running()
    }

    fn change_mark(&self) -> String {
        crate::native::list_recent(RECENT_TURNS, true)
            .map(|listed| {
                let mut mark = listed.summaries.len().to_string();
                for s in listed.summaries.iter().take(PHONE_MARK_TURNS) {
                    mark.push_str(&format!("|{}@{}", s.mission_id, s.updated));
                }
                mark
            })
            .unwrap_or_default()
    }

    fn name_of(&self, mission_id: &str) -> Option<String> {
        let owner = crate::people::owner_of(&self.store, mission_id)?;
        crate::native::roster_in(&self.store).ok()?.into_iter().find(|t| t.id == owner).map(|t| t.name)
    }

    fn waiting(&self) -> Vec<Request> {
        crate::engine_api::api().waiting()
    }

    fn answer_card(&self, approval_id: &str, approve: bool) -> bool {
        let decision = if approve { crate::approval::Decision::ApproveOnce } else { crate::approval::Decision::Deny };
        // The window's own press (src/ui/approval_card.rs): a paired phone is the person.
        crate::engine_api::api().decide(approval_id, decision, None).is_ok()
    }

    fn newest(&self, mission_id: &str) -> Option<String> {
        crate::native::real_tip(mission_id).ok()
    }

    fn running(&self, mission_id: &str) -> bool {
        crate::engine_api::api().owns(mission_id)
    }

    fn route_of(&self, newest: &str) -> Option<PhoneRoute> {
        let owner = crate::people::owner_of(&self.store, newest)
            .and_then(|id| crate::native::roster_in(&self.store).ok()?.into_iter().find(|t| t.id == id));
        if let Some(teammate) = owner {
            let mode = self.saved_mode(&teammate.id).unwrap_or(Mode::Ask);
            return Some(PhoneRoute { teammate, mode, runtime: None });
        }
        // No teammate, still yours (0.878): the turn's own agent, model and mode.
        let turn = crate::door_tools::Turn::from_records(&Self::records(newest)?)?;
        let mode = turn.mode.as_deref().and_then(Mode::parse).unwrap_or(Mode::Ask);
        Some(PhoneRoute {
            teammate: crate::native::nobody(turn.runtime.clone(), turn.model.clone()),
            mode,
            runtime: turn.runtime,
        })
    }

    fn start(&self, route: PhoneRoute, newest: &str, text: &str) -> Result<(), String> {
        let job = crate::native::Send {
            teammate: route.teammate,
            conversation: Some(newest.to_owned()),
            prompt: text.to_owned(),
            model: None,
            effort: None,
        };
        // Sent by the person, as the window sends: no `startedBy`, so the phone shows the words as typed.
        let origin = crate::native::Origin { mode: Some(route.mode), runtime: route.runtime, ..Default::default() };
        crate::engine_api::api().send(job, origin).map(|_| ())
    }
}

/// The newest turns whose clocks make the change mark: any write moves the newest turn's clock.
const PHONE_MARK_TURNS: usize = 40;

#[cfg(test)]
#[path = "phone_calls_tests.rs"]
mod tests;
