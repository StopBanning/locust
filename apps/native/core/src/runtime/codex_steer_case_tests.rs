//! Electron's `packages/runtime-adapters/test/a-running-codex-turn-can-be-steered.test.ts` (A2.10), case by case,
//! through the caller: the host's drive loop hands a message for a running turn to `Runtime::steer`
//! (native.rs `drive_runtime`), here on a Codex turn whose `codex app-server` is the stand-in (codex_stand_in.rs).
//! Fixed 24 Nov: Codex could not be steered at all (`can_steer` was false), so a message for a running Codex turn
//! waited for its end.
use super::codex_stand_in::*;
use super::*;

const STEER: &str = "Stop editing app.ts: Wren is changing it.";

fn running(turn_start: Value, steer: Value) -> (Codex, Told) {
    let (mut codex, told) = stand_in(json!({"turn/start": turn_start, "turn/steer": steer}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    assert!(codex.can_steer());
    (codex, told)
}

// case: sends turn/steer on the thread, with the running turn's id, and says it was taken
#[test]
fn sends_turn_steer_with_the_running_turns_id_and_says_it_was_taken() {
    let (mut codex, told) = running(json!([{"result": {"turn": {"id": "turn_1"}}}]), json!([{"result": {"turnId": "turn_1"}}]));
    assert!(codex.steer(STEER));
    assert_eq!(
        told.params_of("turn/steer").unwrap(),
        json!({"threadId": "thread_9", "expectedTurnId": "turn_1", "input": [{"type": "text", "text": STEER}]})
    );
}

// case: learns the turn from turn/started as well
#[test]
fn learns_the_turn_from_turn_started() {
    let (mut codex, told) = running(
        json!([{"result": {}}, {"send": {"jsonrpc": "2.0", "method": "turn/started", "params": {"threadId": "thread_9", "turn": {"id": "turn_7"}}}}]),
        json!([{"result": {"turnId": "turn_7"}}]),
    );
    assert_eq!(kinds(&events_until_end(&mut codex, Duration::from_millis(500))), ["run.started"]);
    assert!(codex.steer("heads up"));
    assert_eq!(told.params_of("turn/steer").unwrap()["expectedTurnId"], "turn_7");
}

// case: says no, without asking, before a turn has started
#[test]
fn says_no_without_asking_before_a_turn_has_started() {
    let (mut codex, told) = running(json!([{"result": {}}]), json!([{"result": {}}]));
    assert!(!codex.steer("too early"));
    assert_eq!(told.id_of("turn/steer"), None);
}

// case: says no when the server refuses it -- the turn has just ended, say
#[test]
fn says_no_when_the_server_refuses_it() {
    let (mut codex, told) = running(json!([{"result": {"turn": {"id": "turn_1"}}}]), json!([{"error": "no active turn"}]));
    assert!(!codex.steer("late"));
    assert!(told.id_of("turn/steer").is_some());
}

// case: says no, without asking, once the turn is over
#[test]
fn says_no_without_asking_once_the_turn_is_over() {
    let (mut codex, told) = running(
        json!([
            {"result": {"turn": {"id": "turn_1"}}},
            {"send": {"jsonrpc": "2.0", "method": "turn/completed", "params": {"threadId": "thread_9", "turn": {"id": "turn_1", "status": "completed"}}}},
        ]),
        json!([{"result": {}}]),
    );
    assert_eq!(to_the_end(&mut codex).last().unwrap().kind, "run.completed");
    assert!(!codex.steer("after"));
    assert_eq!(told.id_of("turn/steer"), None);
}
