//! WHAT THE WINDOW ASKS OF THE ENGINE THAT OWNS TURNS (plan E1).
//!
//! Today a turn lives in whichever process started it: `native.rs` keeps the running turns and the waiting cards in
//! statics. The window, Quick Ask and `--ask` each reach those statics directly, so a card raised in one process is
//! invisible in another, and Stop can miss. This trait is the whole surface they use. E1 changes no behaviour:
//! [`InProcess`] forwards to `native.rs` as the callers did. E2 adds a client that speaks to `locust-native --engine`
//! over a socket, and E3 makes every caller use it, so one process owns every turn.
//!
//! `src/engine_calls.rs` pins the window's direct uses of those statics; the count may only fall.

use crate::{
    approval::{Decision, Request},
    approval_rules::RuleEffect,
    engine::Started,
    native::{self, Origin, Send},
};
use std::sync::{Arc, OnceLock};

/// Turns, cards and endings: what a window, Quick Ask or `--ask` asks of the engine.
pub trait EngineApi: std::marker::Send + Sync {
    /// Start or continue a turn. Returns once its ledger exists; the window follows the ledger file.
    fn send(&self, job: Send, origin: Origin) -> Result<Started, String>;
    /// Ask a running turn to stop. Errs when this engine does not own it.
    fn stop(&self, conversation: &str) -> Result<String, String>;
    /// Does this engine run a turn in this conversation (or tip)?
    fn owns(&self, conversation: &str) -> bool;
    /// The first of `tip` and `conversation` this engine runs a turn in.
    fn owned_key(&self, tip: Option<&str>, conversation: Option<&str>) -> Option<String> {
        [tip, conversation].into_iter().flatten().find(|k| !k.is_empty() && self.owns(k)).map(str::to_owned)
    }
    /// The conversations with a running turn.
    fn running(&self) -> Vec<String>;
    /// Post-edit checks currently running: `(mission_id, command)`.
    fn running_checks(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    /// Cards waiting on the person.
    fn waiting(&self) -> Vec<Request>;
    /// Cards waiting for one turn: the door says that turn waits on the person.
    fn waiting_for(&self, mission: &str) -> Vec<Request> {
        self.waiting().into_iter().filter(|r| r.mission_id == mission).collect()
    }
    /// Answer a card once.
    fn decide(&self, approval_id: &str, decision: Decision, reason: Option<String>) -> Result<(), String>;
    /// Save an allow or deny rule from the card, then answer it once.
    fn save_rule_and_decide(&self, card: &Request, effect: RuleEffect) -> Result<(), String>;
    /// Carry a stopped turn on with another agent: the new conversation, its folder and the mode it runs in.
    fn carry(&self, mission: &str, runtime: &str, model: Option<&str>) -> Result<(String, Option<String>, String), String>;
    /// Ask every running turn to stop and record its ending, without waiting.
    fn shut_down(&self);
    /// Wait until the endings asked for by [`EngineApi::shut_down`] are written.
    fn wait_for_shutdown(&self);
    /// Something the window should say about the engine itself, once: that it had stopped and was started again (E5).
    fn notice(&self) -> Option<String> {
        None
    }
    /// The door for other apps, served where the turns run (E4). The engine in this process serves none: a window
    /// that could not join the shared engine keeps its own.
    fn door(&self, change: DoorChange) -> Result<crate::door::State, String> {
        let _ = change;
        Err("This engine serves no door.".into())
    }
}

/// What a window asks of the door for other apps (plan E4): read it, open it as saved, or flip one of its switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorChange {
    Load,
    Read,
    Enabled(bool),
    OwnMode(bool),
    Conversations(bool),
}

/// The engine in this process: what every caller reached directly before E1.
pub struct InProcess;

impl EngineApi for InProcess {
    fn send(&self, job: Send, origin: Origin) -> Result<Started, String> {
        native::send_from(job, origin)
    }
    fn stop(&self, conversation: &str) -> Result<String, String> {
        native::stop(conversation)
    }
    fn owns(&self, conversation: &str) -> bool {
        native::owns(conversation)
    }
    fn running(&self) -> Vec<String> {
        native::running_ids()
    }
    fn running_checks(&self) -> Vec<(String, String)> {
        native::running_checks()
    }
    fn waiting(&self) -> Vec<Request> {
        native::waiting_approvals()
    }
    fn decide(&self, approval_id: &str, decision: Decision, reason: Option<String>) -> Result<(), String> {
        native::decide_approval(approval_id, decision, reason)
    }
    fn save_rule_and_decide(&self, card: &Request, effect: RuleEffect) -> Result<(), String> {
        crate::approval::save_rule_and_decide(card, effect)
    }
    fn carry(&self, mission: &str, runtime: &str, model: Option<&str>) -> Result<(String, Option<String>, String), String> {
        native::carry(mission, runtime, model)
    }
    fn shut_down(&self) {
        native::shutdown_in_background();
    }
    fn wait_for_shutdown(&self) {
        native::wait_for_shutdown();
    }
}

static INSTALLED: OnceLock<Arc<dyn EngineApi>> = OnceLock::new();

/// The engine this process talks to: the one [`install`]ed, else the one in this process.
pub fn api() -> &'static dyn EngineApi {
    static LOCAL: InProcess = InProcess;
    INSTALLED.get().map(|a| a.as_ref()).unwrap_or(&LOCAL)
}

/// Is this process talking to the shared engine (an installed one), not keeping turns of its own?
pub fn shared() -> bool {
    INSTALLED.get().is_some()
}

/// Choose the engine once, at startup (E3: a client of `locust-native --engine`). A second call is refused.
pub fn install(engine: Arc<dyn EngineApi>) -> Result<(), String> {
    INSTALLED.set(engine).map_err(|_| "The engine was already chosen for this process.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_engine_is_the_one_in_this_process() {
        let store = crate::store::Store::new(
            std::env::temp_dir().join(format!("engine-api-{}", std::process::id())).join("native"),
            std::env::temp_dir().join(format!("engine-api-{}", std::process::id())).join("electron"),
        )
        .unwrap();
        let _hook = native::hooks::install(store, None);
        // Nothing runs and nothing waits; asking through the trait answers as native.rs does.
        assert_eq!(api().running(), native::running_ids());
        assert!(api().waiting().is_empty());
        assert!(!api().owns("mission_none"));
        assert_eq!(api().owned_key(Some(""), Some("mission_none")), None);
        assert!(api().stop("mission_none").is_err());
    }
}
