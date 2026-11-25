//! faceState.test.ts, one test per case, with Electron's events as JSON (so `live_event_of` is read too); then every
//! case of Electron's that drives `teammateStatusView`, `runtimeReach`, `runtimeIsUsable`, `stateInWords` and
//! `facePresenceFor`, with its file named above it and its values. faceState's two CSS-animation cases
//! (`FACE_MOTION`'s names and `done`'s single hop) are about the stylesheet and do not apply here.
use super::*;
use crate::bot_rig::{bot_motion, face_label, face_motion, is_motionless, BotAvatarState, BotMotionLevel};
use serde_json::json;

fn event(kind: &str, payload: Value) -> Value {
    json!({"type": kind, "payload": payload})
}
fn live(events: &[Value], running: bool) -> LiveActivity {
    let read: Vec<LiveRuntimeEvent> = events.iter().map(live_event_of).collect();
    live_activity_of(&read, running)
}
fn tool(kind: &str, item: &str, name: &str) -> Value {
    event(kind, json!({"itemId": item, "toolKind": "tool", "name": name}))
}

const ALL: [FaceActivity; 8] = [
    FaceActivity::Thinking,
    FaceActivity::Working,
    FaceActivity::Responding,
    FaceActivity::Waiting,
    FaceActivity::Receiving,
    FaceActivity::Blocked,
    FaceActivity::Done,
    FaceActivity::Idle,
];

// --- which states move ---

#[test]
fn idle_and_blocked_are_the_only_motionless_states_and_every_other_state_moves() {
    for activity in ALL {
        assert_eq!(is_motionless(activity), matches!(activity, FaceActivity::Idle | FaceActivity::Blocked), "{activity:?}");
    }
}

// case: names every animation for the state it means, and retires the generic one
#[test]
fn names_every_animation_for_the_state_it_means_and_retires_the_generic_one() {
    for activity in ALL {
        let motion = face_motion(activity);
        for name in [motion.chip, motion.eyes, motion.mouth].into_iter().flatten() {
            assert!(!name.starts_with("lcEyes "), "generic animation retired: {name}");
        }
    }
    assert!(face_motion(FaceActivity::Thinking).eyes.unwrap().starts_with("lcEyesUp"));
    assert!(face_motion(FaceActivity::Working).eyes.unwrap().starts_with("lcEyesDown"));
    assert!(face_motion(FaceActivity::Responding).eyes.unwrap().starts_with("lcEyesFwd"));
    assert!(face_motion(FaceActivity::Receiving).eyes.unwrap().starts_with("lcGlance"));
}

// case: done hops once; it is not a loop
#[test]
fn done_hops_once_it_is_not_a_loop() {
    assert!(face_motion(FaceActivity::Done).chip.unwrap().ends_with(" 1"));
    for level in [BotMotionLevel::Subtle, BotMotionLevel::Full] {
        let motion = bot_motion(FaceActivity::Done, level);
        assert!(motion.hops, "{level:?}");
        assert_eq!(motion.state, BotAvatarState::Default, "{level:?}: does not loop working");
        assert_eq!(motion.jump_every, Some(0.0), "{level:?}: no periodic jump loop");
    }
}

#[test]
fn thinking_is_not_still_a_still_face_is_idle_and_a_reasoning_teammate_is_not_abandoned() {
    assert!(!is_motionless(FaceActivity::Thinking));
}

#[test]
fn has_a_word_for_every_state() {
    for activity in ALL {
        assert!(!face_label(activity).is_empty());
    }
    assert_eq!(face_label(FaceActivity::Waiting), "waiting on you");
    assert_eq!(face_label(FaceActivity::Responding), "replying");
}

// --- what a live run is doing ---

#[test]
fn is_idle_when_not_running_whatever_the_events_say() {
    assert_eq!(live(&[event("tool.started", json!({"itemId": "t", "toolKind": "x", "name": "x", "phase": "started"}))], false), LiveActivity::Idle);
}

#[test]
fn thinks_during_an_open_reasoning_step() {
    assert_eq!(live(&[event("step.started", json!({"stepKind": "reasoning", "itemType": "r"}))], true), LiveActivity::Thinking);
}

#[test]
fn works_during_an_open_tool_even_one_begun_mid_reasoning() {
    let events = [
        event("step.started", json!({"stepKind": "reasoning", "itemType": "r"})),
        event("tool.started", json!({"itemId": "t1", "toolKind": "command_execution", "name": "shell", "phase": "started"})),
    ];
    assert_eq!(live(&events, true), LiveActivity::Working);
}

#[test]
fn replies_while_a_message_is_still_streaming_and_stops_when_it_is_final() {
    let streaming = event("message.delta", json!({"itemId": "m", "operation": "append", "text": "Hel", "final": false}));
    assert_eq!(live(std::slice::from_ref(&streaming), true), LiveActivity::Responding);
    let done = event("message.delta", json!({"itemId": "m", "operation": "append", "text": "lo", "final": true}));
    assert_eq!(live(&[streaming, done], true), LiveActivity::Thinking);
}

#[test]
fn is_never_idle_while_the_run_is_live_even_between_steps() {
    assert_ne!(live(&[], true), LiveActivity::Idle);
}

#[test]
fn calls_a_live_run_with_nothing_open_thinking_not_working() {
    assert_eq!(live(&[], true), LiveActivity::Thinking);
    let settled = [
        event("step.started", json!({"stepKind": "turn", "message": "Working"})),
        event("step.completed", json!({"stepKind": "turn"})),
    ];
    assert_eq!(live(&settled, true), LiveActivity::Thinking);
}

#[test]
fn calls_a_named_turn_step_working_because_its_line_shows_no_dots() {
    assert_eq!(live(&[event("step.started", json!({"stepKind": "turn", "message": "Running the billing suite"}))], true), LiveActivity::Working);
}

#[test]
fn still_calls_an_open_tool_working() {
    assert_eq!(live(&[event("tool.started", json!({"itemId": "t", "toolKind": "x", "name": "x", "phase": "started"}))], true), LiveActivity::Working);
}

// --- one teammate, one state ---

fn base() -> (bool, bool, LiveActivity, bool, bool) {
    (false, false, LiveActivity::Idle, false, false)
}

#[test]
fn blocked_outranks_everything() {
    let (_, _, _, done, received) = base();
    assert_eq!(teammate_activity(true, true, LiveActivity::Working, done, received), FaceActivity::Blocked);
}

#[test]
fn waiting_on_you_never_resolves_to_still_and_outranks_live_work() {
    let (blocked, _, _, done, received) = base();
    let state = teammate_activity(blocked, true, LiveActivity::Working, done, received);
    assert_eq!(state, FaceActivity::Waiting);
    assert!(!is_motionless(state));
}

#[test]
fn follows_the_live_run_when_there_is_one() {
    let (blocked, waiting, _, done, received) = base();
    assert_eq!(teammate_activity(blocked, waiting, LiveActivity::Thinking, done, received), FaceActivity::Thinking);
    assert_eq!(teammate_activity(blocked, waiting, LiveActivity::Responding, done, received), FaceActivity::Responding);
}

#[test]
fn hops_once_when_a_mission_just_finished_then_glances_at_a_message_that_just_arrived_then_rests() {
    let (blocked, waiting, live, _, _) = base();
    assert_eq!(teammate_activity(blocked, waiting, live, true, true), FaceActivity::Done);
    assert_eq!(teammate_activity(blocked, waiting, live, false, true), FaceActivity::Receiving);
    assert_eq!(teammate_activity(blocked, waiting, live, false, false), FaceActivity::Idle);
}

// --- a run waiting on its own subagent ---

#[test]
fn reads_subagent_working_while_an_agent_or_task_call_is_open_and_working_again_once_it_reports_back() {
    let (started, done) = (|name: &str, item: &str| tool("tool.started", item, name), |name: &str, item: &str| tool("tool.completed", item, name));
    assert_eq!(live(&[started("Read", "r1"), done("Read", "r1"), started("Agent", "a1")], true), LiveActivity::Delegating);
    assert_eq!(face_label(FaceActivity::Delegating), "subagent working");
    assert_eq!(live(&[started("Agent", "a1"), done("Agent", "a1"), started("Read", "r2")], true), LiveActivity::Working);
    assert_eq!(live(&[started("task", "t1")], true), LiveActivity::Delegating);
}

// --- a tool whose start is said twice ---

fn started_twice(item: &str, name: &str, command: &str) -> [Value; 2] {
    [tool("tool.started", item, name), event("tool.started", json!({"itemId": item, "toolKind": "tool", "name": name, "command": command}))]
}

#[test]
fn is_closed_by_its_one_completion() {
    let mut agent = started_twice("toolu_1", "Agent", "Survey the tests").to_vec();
    agent.push(tool("tool.completed", "toolu_1", "Agent"));
    assert_eq!(live(&agent, true), LiveActivity::Thinking);
    let mut read = started_twice("toolu_2", "Read", "src/app.ts").to_vec();
    read.push(tool("tool.completed", "toolu_2", "Read"));
    assert_eq!(live(&read, true), LiveActivity::Thinking);
}

#[test]
fn is_still_open_until_then() {
    assert_eq!(live(&started_twice("toolu_1", "Agent", "Survey the tests"), true), LiveActivity::Delegating);
    assert_eq!(live(&started_twice("toolu_2", "Bash", "ls"), true), LiveActivity::Working);
}

// --- a helper's own calls are the helper's (byHelper) ---

#[test]
fn a_helpers_own_tool_call_is_not_the_teammates_work() {
    let helper = event("tool.started", json!({"itemId": "h1", "toolKind": "tool", "name": "Bash", "parentItemId": "a1"}));
    assert_eq!(live(&[helper], true), LiveActivity::Thinking);
}

// ===== status.ts: one teammate's status =====

use crate::agents::Probe;

/// status.test.ts `runtime()`: Codex, installed, signed in, ready.
fn runtime() -> RuntimeFacts {
    RuntimeFacts { id: "codex".into(), ready: true, status: Probe::Ready, installed: true, checking: false }
}
fn with(status: Probe, ready: bool, installed: bool) -> RuntimeFacts {
    RuntimeFacts { status, ready, installed, ..runtime() }
}
fn known(usable: bool, installed: bool) -> RuntimeReach {
    RuntimeReach { any_usable: Some(usable), any_installed: Some(installed) }
}
fn usable_only(usable: bool) -> RuntimeReach {
    RuntimeReach { any_usable: Some(usable), any_installed: None }
}
fn view(input: StatusInputs<'_>) -> StatusView {
    teammate_status_view(&input)
}

// --- status.test.ts: teammate status ---

#[test]
fn reports_a_blocked_runtime_even_while_a_mission_looks_like_it_is_running() {
    let signed_out = with(Probe::AuthRequired, false, true);
    let said = view(StatusInputs { own_runtime: Some(&signed_out), running: true, role_label: "Code & Migrations", ..Default::default() });
    assert_eq!(said.status, TeammateStatus::Blocked);
    assert_eq!(said.tone, Tone::Red);
}

#[test]
fn puts_a_pending_approval_ahead_of_working() {
    let ready = runtime();
    let said = view(StatusInputs { own_runtime: Some(&ready), running: true, pending_approvals: 1, role_label: "Code & Migrations", ..Default::default() });
    assert_eq!(said.status, TeammateStatus::ApprovalNeeded);
}

#[test]
fn pulses_only_while_working() {
    let ready = runtime();
    let working = view(StatusInputs { own_runtime: Some(&ready), running: true, role_label: "Code & Migrations", ..Default::default() });
    let idle = view(StatusInputs { own_runtime: Some(&ready), running: false, role_label: "Code & Migrations", ..Default::default() });
    assert!(working.pulse);
    assert!(!idle.pulse);
    assert!(idle.label.ends_with("idle"), "{}", idle.label);
}

#[test]
fn treats_usability_as_needing_both_the_flag_and_the_probe() {
    assert!(runtime_is_usable(&runtime()));
    assert!(!runtime_is_usable(&RuntimeFacts { ready: false, ..runtime() }));
    assert!(!runtime_is_usable(&RuntimeFacts { status: Probe::ProbeFailed, ..runtime() }));
}

// --- status.test.ts: face activity ---

#[test]
fn decides_the_face_from_the_same_inputs_as_the_label_so_the_two_agree() {
    let base = StatusInputs { reach: usable_only(true), role_label: "Docs & QA", ..Default::default() };
    let thinking = view(StatusInputs { running: true, live: Some(LiveActivity::Thinking), ..base.clone() });
    assert_eq!((thinking.activity, thinking.label.as_str()), (FaceActivity::Thinking, "Docs & QA · thinking"));
    let working = view(StatusInputs { running: true, ..base.clone() });
    assert_eq!((working.activity, working.label.as_str()), (FaceActivity::Working, "Docs & QA · working"));
    let waiting = view(StatusInputs { pending_approvals: 1, ..base.clone() });
    assert_eq!((waiting.activity, waiting.label.as_str()), (FaceActivity::Waiting, "Docs & QA · waiting on you"));
    assert_eq!(view(StatusInputs { reach: usable_only(false), ..base.clone() }).activity, FaceActivity::Blocked);
    let done = view(StatusInputs { recently_done: true, ..base.clone() });
    assert_eq!((done.activity, done.label.as_str()), (FaceActivity::Done, "Docs & QA · done"));
    let idle = view(base);
    assert_eq!((idle.activity, idle.label.as_str()), (FaceActivity::Idle, "Docs & QA · idle"));
}

#[test]
fn carries_every_state_on_the_presence_dot_so_a_still_face_still_reads() {
    assert_eq!(face_presence_for(TeammateStatus::Working), Presence::Working);
    assert_eq!(face_presence_for(TeammateStatus::ApprovalNeeded), Presence::Approval);
    assert_eq!(face_presence_for(TeammateStatus::Blocked), Presence::Blocked);
    assert_eq!(face_presence_for(TeammateStatus::Idle), Presence::None);
}

// --- status.test.ts: a teammate with no work of their own ---

#[test]
fn a_teammate_with_no_work_is_idle_not_blocked_when_something_could_run() {
    assert_eq!(view(StatusInputs { reach: usable_only(true), role_label: "Docs & QA", ..Default::default() }).status, TeammateStatus::Idle);
}

#[test]
fn a_teammate_with_no_work_is_blocked_when_nothing_is_signed_in_at_all() {
    assert_eq!(view(StatusInputs { reach: usable_only(false), role_label: "Docs & QA", ..Default::default() }).status, TeammateStatus::Blocked);
}

#[test]
fn a_teammate_is_judged_against_the_runtime_its_own_mission_is_on() {
    let claude = RuntimeFacts { id: "claude".into(), ..runtime() };
    let said = view(StatusInputs { own_runtime: Some(&claude), reach: usable_only(true), running: true, role_label: "Code & Migrations", ..Default::default() });
    assert_eq!(said.status, TeammateStatus::Working);
}

// --- the-bare-machine.test.ts: a teammate on a machine with no CLI is not asked to sign in ---

const BARE_ROLE: &str = "Code & Migrations";

#[test]
fn says_nothing_is_installed_when_nothing_is() {
    let said = view(StatusInputs { reach: known(false, false), role_label: BARE_ROLE, ..Default::default() });
    assert_eq!(said.label, "No AI agent installed");
    assert!(!said.label.contains("sign in"));
}

#[test]
fn still_says_sign_in_when_something_is_installed_and_merely_unsigned() {
    assert_eq!(view(StatusInputs { reach: known(false, true), role_label: BARE_ROLE, ..Default::default() }).label, "Sign-in needed");
}

#[test]
fn makes_the_same_distinction_for_the_teammates_own_runtime() {
    let missing = RuntimeFacts { id: "opencode".into(), ..with(Probe::Missing, false, false) };
    assert_eq!(view(StatusInputs { own_runtime: Some(&missing), role_label: BARE_ROLE, ..Default::default() }).label, "AI agent not installed");
    let unsigned = RuntimeFacts { id: "opencode".into(), ..with(Probe::AuthRequired, false, true) };
    assert_eq!(view(StatusInputs { own_runtime: Some(&unsigned), role_label: BARE_ROLE, ..Default::default() }).label, "Sign-in needed");
}

#[test]
fn is_still_blocked_and_still_red_either_way_because_neither_can_run() {
    for installed in [true, false] {
        let said = view(StatusInputs { reach: known(false, installed), role_label: BARE_ROLE, ..Default::default() });
        assert_eq!(said.status, TeammateStatus::Blocked);
        assert_eq!(said.tone, Tone::Red);
    }
}

#[test]
fn does_not_reach_for_either_sentence_when_a_runtime_is_fine() {
    assert!(!view(StatusInputs { reach: known(true, true), role_label: BARE_ROLE, ..Default::default() }).label.contains("sign in"));
}

// --- a-teammate-is-not-stuck-while-agents-are-still-being-found.test.ts ---

/// That file's `runtime(id, usable, installed)`.
fn agent(id: &str, usable: bool, installed: bool) -> RuntimeFacts {
    let status = if usable { Probe::Ready } else if installed { Probe::AuthRequired } else { Probe::Missing };
    RuntimeFacts { id: id.into(), ready: usable, status, installed, checking: false }
}
fn pending() -> RuntimeFacts {
    RuntimeFacts { status: Probe::ProbeFailed, checking: true, ..agent("opencode", false, true) }
}
fn reach_view(reach: RuntimeReach) -> StatusView {
    view(StatusInputs { reach, role_label: "Docs & QA", ..Default::default() })
}
fn own_view(own: &RuntimeFacts, others: &[RuntimeFacts]) -> StatusView {
    let all: Vec<RuntimeFacts> = std::iter::once(own.clone()).chain(others.iter().cloned()).collect();
    view(StatusInputs { own_runtime: Some(own), reach: runtime_reach(&all, true), role_label: "Docs & QA", ..Default::default() })
}

#[test]
fn has_no_global_blocked_verdict_while_the_only_possible_agent_is_still_checking() {
    assert_eq!(runtime_reach(&[pending(), agent("claude", false, false)], true), RuntimeReach { any_usable: None, any_installed: Some(true) });
    let said = reach_view(runtime_reach(&[pending()], true));
    assert_eq!((said.status, said.activity), (TeammateStatus::Idle, FaceActivity::Idle));
}

#[test]
fn keeps_its_pending_own_agent_idle_even_when_another_agent_is_ready() {
    let said = own_view(&pending(), &[agent("claude", true, true)]);
    assert_eq!((said.status, said.activity, said.tone), (TeammateStatus::Idle, FaceActivity::Idle, Tone::Muted));
}

#[test]
fn keeps_a_checked_unusable_own_agent_blocked_while_another_check_is_pending() {
    let missing = own_view(&agent("claude", false, false), &[pending()]);
    assert_eq!((missing.status, missing.activity, missing.label.as_str()), (TeammateStatus::Blocked, FaceActivity::Blocked, "AI agent not installed"));
    let unsigned = own_view(&agent("claude", false, true), &[pending()]);
    assert_eq!((unsigned.status, unsigned.activity, unsigned.label.as_str()), (TeammateStatus::Blocked, FaceActivity::Blocked, "Sign-in needed"));
}

#[test]
fn returns_to_idle_when_a_late_ready_answer_replaces_a_checked_failure() {
    let failed = RuntimeFacts { checking: false, ..pending() };
    let said = own_view(&failed, &[]);
    assert_eq!((said.status, said.activity), (TeammateStatus::Blocked, FaceActivity::Blocked));
    let said = own_view(&pending(), &[]);
    assert_eq!((said.status, said.activity), (TeammateStatus::Idle, FaceActivity::Idle));
    let said = own_view(&agent("opencode", true, true), &[]);
    assert_eq!((said.status, said.activity), (TeammateStatus::Idle, FaceActivity::Idle));
}

#[test]
fn is_idle_not_blocked_while_discovery_has_not_answered() {
    assert_eq!(runtime_reach(&[], false), RuntimeReach::default());
    let said = reach_view(runtime_reach(&[], false));
    assert_eq!(said.activity, FaceActivity::Idle);
    assert_ne!(said.status, TeammateStatus::Blocked);
}

#[test]
fn is_blocked_once_discovery_says_nothing_can_run_and_says_why() {
    let none = reach_view(runtime_reach(&[agent("opencode", false, false), agent("claude", false, false)], true));
    assert_eq!((none.activity, none.label.as_str()), (FaceActivity::Blocked, "No AI agent installed"));
    let unsigned = reach_view(runtime_reach(&[agent("opencode", false, true)], true));
    assert_eq!((unsigned.activity, unsigned.label.as_str()), (FaceActivity::Blocked, "Sign-in needed"));
}

#[test]
fn is_idle_once_one_agent_can_run() {
    let said = reach_view(runtime_reach(&[agent("opencode", true, true), agent("claude", false, false)], true));
    assert_eq!(said.activity, FaceActivity::Idle);
}

// --- a-signed-out-teammate-is-not-idle.test.ts ---

fn claude_signed_out() -> RuntimeFacts {
    // Electron's 'signed-out' is no status discovery gives; it is "not ready", which AuthRequired is here.
    RuntimeFacts { id: "claude".into(), ready: false, installed: true, status: Probe::AuthRequired, checking: false }
}

#[test]
fn reads_as_blocked_not_idle_when_its_runtime_is_signed_out() {
    let signed_out = claude_signed_out();
    let said = view(StatusInputs { own_runtime: Some(&signed_out), reach: usable_only(true), role_label: "Docs & QA", ..Default::default() });
    assert_eq!(said.status, TeammateStatus::Blocked);
    assert_eq!(said.label, "Sign-in needed");
    assert!(!said.label.contains("idle"));
    assert_eq!(said.tone, Tone::Red);
    assert_eq!(face_presence_for(said.status), Presence::Blocked);
}

#[test]
fn still_reads_as_idle_when_its_own_runtime_is_fine() {
    let codex = runtime();
    let said = view(StatusInputs { own_runtime: Some(&codex), reach: usable_only(true), role_label: "Docs & QA", ..Default::default() });
    assert_eq!(said.status, TeammateStatus::Idle);
    assert_eq!(face_presence_for(said.status), Presence::None);
}

#[test]
fn keeps_a_mission_waiting_on_an_approval_as_its_own_state_not_blocked() {
    let codex = runtime();
    let said = view(StatusInputs { own_runtime: Some(&codex), reach: usable_only(true), running: true, pending_approvals: 1, role_label: "Builder", ..Default::default() });
    assert_eq!(said.status, TeammateStatus::ApprovalNeeded);
    assert_eq!(face_presence_for(said.status), Presence::Approval);
    assert_eq!(said.tone, Tone::Amber);
}

#[test]
fn is_blocked_when_nothing_at_all_is_signed_in_whatever_it_is_set_to() {
    assert_eq!(view(StatusInputs { reach: usable_only(false), role_label: "Docs & QA", ..Default::default() }).status, TeammateStatus::Blocked);
}

fn unanswered_base(runtime: &RuntimeFacts) -> StatusInputs<'_> {
    StatusInputs { own_runtime: Some(runtime), reach: known(true, true), role_label: "Code & Migrations", ..Default::default() }
}

#[test]
fn a_runtime_that_has_not_answered_yet_is_not_told_to_sign_in() {
    for status in [Probe::ProbeFailed, Probe::Offline] {
        let claude = RuntimeFacts { id: "claude".into(), ready: false, installed: true, status, checking: false };
        let said = view(unanswered_base(&claude));
        assert!(!said.label.to_lowercase().contains("sign-in"), "{status:?}: {}", said.label);
        assert!(said.label.to_lowercase().contains("not answering"), "{status:?}: {}", said.label);
    }
}

#[test]
fn a_runtime_that_says_it_is_signed_out_still_is() {
    let claude = claude_signed_out();
    assert_eq!(view(unanswered_base(&claude)).label, "Sign-in needed");
}

// --- a-question-makes-the-face-wait-on-you.test.ts ---

fn approval_item(teammate_id: Option<&str>) -> crate::needs_you::NeedsYouItem {
    crate::needs_you::NeedsYouItem::Approval {
        key: format!("approval:{}", teammate_id.unwrap_or("none")),
        mission_id: "mission_a".into(),
        teammate_id: teammate_id.map(str::to_owned),
        name: "Ada".into(),
        what: "Run npm test".into(),
        asking: false,
        command: true,
    }
}

fn decision_item(teammate_id: Option<&str>, mission_id: &str) -> crate::needs_you::NeedsYouItem {
    crate::needs_you::NeedsYouItem::Decision {
        key: format!("decision:{mission_id}"),
        mission_id: mission_id.into(),
        teammate_id: teammate_id.map(str::to_owned),
        name: "Ada".into(),
        what: "Cyan or amber?".into(),
    }
}

// case: counts questions a teammate stopped on beside its paused runs
#[test]
fn counts_questions_a_teammate_stopped_on_beside_its_paused_runs() {
    let counts = waiting_by_teammate(&[
        approval_item(Some("tm_ada")),
        decision_item(Some("tm_ada"), "mission_b"),
        decision_item(Some("tm_bo"), "mission_c"),
    ]);
    assert_eq!(counts.get("tm_ada"), Some(&2));
    assert_eq!(counts.get("tm_bo"), Some(&1));
}

// case: leaves out memory suggestions, Scout cards, waiting routines, and items no teammate owns
#[test]
fn leaves_out_memory_suggestions_scout_cards_waiting_routines_and_items_no_teammate_owns() {
    use crate::needs_you::NeedsYouItem;
    let counts = waiting_by_teammate(&[
        NeedsYouItem::Memory { key: "memory".into(), count: 3 },
        decision_item(None, "mission_b"),
        approval_item(None),
        NeedsYouItem::Scout { key: "scout:card_now".into(), card_id: "card_now".into(), name: "Scout".into(), what: "Checks failed.".into() },
        NeedsYouItem::Routine { key: "routine:rt_1".into(), routine_id: "rt_1".into(), name: "Morning digest".into(), what: "Waiting for you".into() },
    ]);
    assert!(counts.is_empty());
}

// case: a question alone makes the face wait, in amber, with nothing running
#[test]
fn a_question_alone_makes_the_face_wait_in_amber_with_nothing_running() {
    let waiting = waiting_by_teammate(&[decision_item(Some("tm_ada"), "mission_b")]).get("tm_ada").copied().unwrap_or(0);
    let said = view(StatusInputs { pending_approvals: waiting, role_label: "Builder", ..Default::default() });
    assert_eq!(said.activity, FaceActivity::Waiting);
    assert_eq!(said.tone, Tone::Amber);
    assert_eq!(said.status, TeammateStatus::ApprovalNeeded);
}

// case: a teammate with no question and no paused run stays idle
#[test]
fn a_teammate_with_no_question_and_no_paused_run_stays_idle() {
    let waiting = waiting_by_teammate(&[decision_item(Some("tm_bo"), "mission_b")]).get("tm_ada").copied().unwrap_or(0);
    assert_eq!(view(StatusInputs { pending_approvals: waiting, role_label: "Builder", ..Default::default() }).activity, FaceActivity::Idle);
}

// --- a-face-says-its-state-in-words.test.tsx: a state in words ---

#[test]
fn a_state_in_words_is_what_the_teammate_is_doing_or_why_they_are_blocked() {
    let working = StatusView { status: TeammateStatus::Working, activity: FaceActivity::Working, label: "Code & Migrations · working".into(), tone: Tone::Live, pulse: true };
    let waiting = StatusView { status: TeammateStatus::ApprovalNeeded, activity: FaceActivity::Waiting, label: "Code & Migrations · waiting on you".into(), tone: Tone::Amber, pulse: false };
    let signed_out = StatusView { status: TeammateStatus::Blocked, activity: FaceActivity::Blocked, label: "Runtime sign-in required".into(), tone: Tone::Red, pulse: false };
    assert_eq!(state_in_words(&working), "working");
    assert_eq!(state_in_words(&waiting), "waiting on you");
    assert_eq!(state_in_words(&signed_out), "Runtime sign-in required");
}

// --- the runtime-reach cases that were here before (status.ts 145-157) ---

#[test]
fn a_partial_sweep_can_prove_one_works_but_not_that_none_do() {
    assert_eq!(runtime_reach(&[pending()], true).any_usable, None);
    assert_eq!(runtime_reach(&[pending(), agent("claude", true, true)], true).any_usable, Some(true));
    assert_eq!(runtime_reach(&[agent("claude", false, true)], true).any_usable, Some(false));
}

// --- a-teammate-who-just-finished-last-worked-just-now.test.ts ---

// case: last worked just now: her conversation is dated by its newest event, not by when it was sent
#[test]
fn last_worked_just_now_her_conversation_is_dated_by_its_newest_event_not_by_when_it_was_sent() {
    use crate::home_team_state::last_worked_phrase;
    let sent = "2026-10-12T15:00:00.000Z";
    let ended = "2026-10-12T15:02:10.000Z";
    let events = ["2026-10-12T15:00:02.000Z", "2026-10-12T15:01:30.000Z", ended];
    let now: chrono::DateTime<chrono::Utc> = "2026-10-12T15:02:20.000Z".parse().unwrap();
    assert_eq!(live_run_last_at(&events, Some(sent), None), Some(ended));
    assert_eq!(last_worked_phrase(live_run_last_at(&events, Some(sent), None), now), "last worked just now");
    assert_eq!(last_worked_phrase(Some(sent), now), "last worked 2 minutes ago");
    let ledger = std::env::temp_dir().join(format!("locust-face-status-last-at-{}.jsonl", std::process::id()));
    let body = events.iter().map(|at| serde_json::json!({"event": {"occurredAt": at}}).to_string()).collect::<Vec<_>>().join("\n");
    std::fs::write(&ledger, body).unwrap();
    let got = live_last_at_in_ledger(&ledger, Some(sent), None);
    let _ = std::fs::remove_file(&ledger);
    assert_eq!(got.as_deref(), Some(ended));
}

// case: is dated by when it was sent before anything has happened, and by the ledger for a run it restored
#[test]
fn is_dated_by_when_it_was_sent_before_anything_has_happened_and_by_the_ledger_for_a_run_it_restored() {
    let sent = "2026-10-12T15:00:00.000Z";
    let ended = "2026-10-12T15:02:10.000Z";
    assert_eq!(live_run_last_at(&[], Some(sent), None), Some(sent));
    assert_eq!(live_run_last_at(&[], None, Some(ended)), Some(ended));
    assert_eq!(live_run_last_at(&[], None, None), None);
}
