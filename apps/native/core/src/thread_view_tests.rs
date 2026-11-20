use super::*;
use crate::ledger::Transcript;
use serde_json::json;
const AT: &str = "2026-09-04T21:47:00.000Z";
fn event(kind: &str, p: Value) -> Value {
    json!({"type":kind,"payload":p,"occurredAt":AT})
}
fn delta(text: &str, final_: bool) -> Value {
    event(
        "message.delta",
        json!({"itemId":"a","text":text,"operation":"append","final":final_}),
    )
}
fn fold(events: Vec<Value>) -> Transcript {
    let mut t = Transcript::default();
    t.apply(
        events
            .into_iter()
            .enumerate()
            .map(|(i, mut e)| {
                e["sequence"] = json!(i + 1);
                json!({"recordType":"mission.event","event":e})
            })
            .collect(),
    );
    t
}
fn live(events: &[Value], running: bool, awaiting: bool) -> Option<LiveLine> {
    live_line(events, running, AT, awaiting)
}
#[test]
fn shows_a_caret_only_while_text_is_genuinely_still_arriving() {
    assert!(streaming(false, true));
    assert!(!streaming(true, true));
    assert!(!streaming(false, false));
    for (text, final_, running, expected) in [
        ("partial", false, true, true),
        ("done", true, true, false),
        ("partial", false, false, false),
    ] {
        let t = fold(vec![delta(text, final_)]);
        let Item::Assistant { r#final, .. } = &t.items[0] else {
            panic!()
        };
        assert_eq!(streaming(*r#final, running), expected);
    }
}
#[test]
fn drops_the_live_step_once_it_completes() {
    let start = event(
        "step.started",
        json!({"stepKind":"turn","message":"Running the billing suite"}),
    );
    assert_eq!(
        live(&[start.clone()], true, false).unwrap().label,
        "Running the billing suite"
    );
    let done = live(
        &[start, event("step.completed", json!({"stepKind":"turn"}))],
        true,
        false,
    )
    .unwrap();
    assert_eq!(done.label, "Working");
    assert!(done.waiting);
}
#[test]
fn shows_a_line_the_moment_a_run_starts_before_any_event_arrives() {
    let line = live(&[], true, false).unwrap();
    assert_eq!(line.label, "Starting");
    assert!(line.waiting);
    assert_eq!(line.started_at, AT);
}
#[test]
fn says_nothing_above_an_approval_the_run_is_stopped_on() {
    assert_eq!(live(&[], true, true), None);
}
#[test]
fn says_nothing_above_an_approval_even_with_a_tool_open_the_tool_is_what_is_waiting() {
    let opened = event(
        "tool.started",
        json!({"itemId":"tool_7_0","toolKind":"ask_question","name":"ask_question","command":"How would you like to organize this folder?","phase":"started"}),
    );
    assert_eq!(
        live_line(&[opened.clone()], true, "2026-09-23T04:43:02.000Z", true),
        None
    );
    assert!(live_line(&[opened], true, "2026-09-23T04:43:02.000Z", false).is_some());
}
#[test]
fn draws_no_live_line_once_the_run_is_over() {
    assert_eq!(live(&[], false, false), None);
}
#[test]
fn does_not_put_a_waiting_line_under_a_message_that_is_still_streaming() {
    let line = live(&[delta("half a sen", false)], true, false).unwrap();
    assert!(!line.waiting);
    assert_ne!(line.register, "working");
}
#[test]
fn says_it_is_writing_while_the_reply_arrives_on_the_turn_clock_0_581() {
    let line = live(&[delta("half a sen", false)], true, false).unwrap();
    assert_eq!(line.label, "Writing");
    assert_eq!(line.register, "writing");
    assert_eq!(line.started_at, AT);
    assert_eq!(
        live(&[delta("half a sentence.", true)], true, false)
            .unwrap()
            .register,
        "working"
    );
}
#[test]
fn never_shows_a_live_step_for_a_run_that_is_not_running() {
    assert!(
        live(
            &[event(
                "step.started",
                json!({"stepKind":"turn","message":"Working"})
            )],
            false,
            false
        )
        .is_none()
    );
}
#[test]
fn surfaces_provider_limits_and_diagnostics_as_their_own_items() {
    let t = fold(vec![
        // Native's run.started supplies buildThread's running: true option.
        event("run.started", json!({})),
        event(
            "tool.started",
            json!({"itemId":"t1","name":"shell","command":"pnpm test"}),
        ),
        event(
            "route.limit_detected",
            json!({"kind":"temporary-rate-limit","message":"Slow down"}),
        ),
        event(
            "adapter.diagnostic",
            json!({"level":"warning","code":"x","message":"Heads up","terminal":false}),
        ),
    ]);
    assert!(
        t.items
            .iter()
            .any(|i| matches!(i,Item::Limit {kind,..} if kind=="temporary-rate-limit"))
    );
    assert!(
        t.items
            .iter()
            .any(|i| matches!(i,Item::Note {level,..} if level=="warning"))
    );
    assert!(t.running, "a temporary warning does not end the running turn");
    assert!(t.live.is_some());
}
#[test]
fn finds_the_run_a_peer_message_was_delivered_into_so_the_exchange_can_be_opened() {
    let link = |id: &str, direction: &str| PeerLink {
        direction: direction.into(),
        message_id: id.into(),
        peer_teammate_id: "tm_wren".into(),
        occurred_at: AT.into(),
    };
    let missions = [
        ("mission_booty", vec![link("wm_1", "posted")]),
        ("mission_wren", vec![link("wm_1", "received")]),
        ("mission_other", vec![link("wm_2", "received")]),
    ];
    let find = |id: &str, n: usize| {
        missions[..n]
            .iter()
            .find(|(_, links)| received_message(id, links).is_some())
            .map(|(id, _)| *id)
    };
    assert_eq!(find("wm_1", 3), Some("mission_wren"));
    assert_eq!(find("wm_1", 1), None);
    assert_eq!(find("wm_3", 2), None);
    assert_eq!(find("", 2), None);
}
#[test]
fn never_draws_a_turn_the_host_briefed_as_the_persons_own_words() {
    let briefing = "Wren (Code & Migrations) replied to you; it is quoted below. Do what it asks... end with one <locust-share to=\"Wren\"> block holding your reply.";
    for _messages_present in [true, false] {
        let shown = person_prompt(Some("relay")).then_some(briefing);
        assert_eq!(shown, None);
        assert!(!shown.unwrap_or("").contains("locust-share"));
    }
    assert_eq!(
        person_prompt(Some("resume")).then_some("resumed briefing"),
        None
    );
    assert_eq!(
        person_prompt(None).then_some("read status.ts"),
        Some("read status.ts")
    );
    assert_eq!(
        person_prompt(Some("routine")).then_some("read status.ts"),
        Some("read status.ts")
    );
}
#[test]
fn says_so_when_a_turn_finished_and_wrote_nothing_back() {
    let t = fold(vec![
        event("run.started", json!({})),
        event("run.completed", json!({"process":{}})),
    ]);
    let notes: Vec<_> = t
        .items
        .iter()
        .filter_map(|i| {
            if let Item::Note { text, .. } = i {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        notes,
        [
            "This turn ended without a reply: the runtime finished and wrote nothing back. Nothing was changed. Sending it again usually works."
        ]
    );
    let answered = fold(vec![
        delta("ALPHA", true),
        event("run.completed", json!({"process":{}})),
    ]);
    assert!(
        !answered
            .items
            .iter()
            .any(|i| matches!(i, Item::Note { .. }))
    );
    let worked = fold(vec![
        event(
            "tool.started",
            json!({"itemId":"t1","name":"shell","command":"pnpm test"}),
        ),
        event("tool.completed", json!({"itemId":"t1"})),
        event("run.completed", json!({"process":{}})),
    ]);
    assert!(!worked.items.iter().any(|i| matches!(i, Item::Note { .. })));
    assert!(
        !fold(vec![event("run.started", json!({}))])
            .items
            .iter()
            .any(|i| matches!(i, Item::Note { .. }))
    );
}
#[test]
fn says_a_quota_failure_once_not_as_a_card_and_again_as_a_red_line() {
    let said = "You've hit your usage limit. Upgrade to Pro or try again at Sep 7th, 2026 1:57 AM.";
    let t = fold(vec![
        event(
            "tool.started",
            json!({"itemId":"t1","name":"shell","command":"pnpm test"}),
        ),
        event(
            "route.limit_detected",
            json!({"kind":"quota-exhausted","message":said}),
        ),
        event(
            "adapter.diagnostic",
            json!({"level":"error","code":"codex.runtime_error","message":said,"terminal":true}),
        ),
        event(
            "adapter.diagnostic",
            json!({"level":"warning","code":"x","message":"Something else","terminal":false}),
        ),
    ]);
    assert_eq!(
        t.items
            .iter()
            .filter(|i| matches!(i, Item::Limit { .. }))
            .count(),
        1
    );
    assert_eq!(
        t.items
            .iter()
            .filter_map(|i| if let Item::Note { text, .. } = i {
                Some(text.as_str())
            } else {
                None
            })
            .collect::<Vec<_>>(),
        ["Something else"]
    );
    assert!(error_already_shown(
        &t.items,
        &format!("The run could not continue. {said}")
    ));
    assert!(!error_already_shown(&t.items, "The process died"));
    let warned = fold(vec![event(
        "route.limit_detected",
        json!({"kind":"temporary-rate-limit","message":said}),
    )]);
    assert!(!error_already_shown(&warned.items, said));
}
#[test]
fn does_not_invent_an_activity_card_when_nothing_ran() {
    assert!(
        !fold(vec![delta("hi", true)]).items.iter().any(|i| matches!(
            i,
            Item::Tool { .. } | Item::Plan { .. } | Item::Thought { .. }
        ))
    );
}

#[test]
fn the_run_failure_does_not_repeat_the_ending_quota_card() {
    let said = "You've hit your usage limit. Upgrade to Pro or try again at Sep 7th, 2026 1:57 AM.";
    let t = fold(vec![
        event("route.limit_detected", json!({"kind":"quota-exhausted","message":said})),
        event("run.failed", json!({"message":format!("The run could not continue. {said}"),"process":{}})),
    ]);
    assert_eq!(t.items.iter().filter(|i| matches!(i, Item::Limit{..})).count(), 1);
    assert!(!t.items.iter().any(|i| matches!(i, Item::Note{level,..} if level == "error")));
}

#[test]
fn the_ledger_caches_the_live_projection_when_events_change() {
    let t = fold(vec![event("run.started", json!({})), event("step.started", json!({"stepKind":"turn","message":"Running the billing suite"}))]);
    let line = t.live.as_ref().expect("cached live step");
    assert_eq!(line.label, "Running the billing suite");
    assert_eq!(line.started_at, AT);
    let stopped = fold(vec![event("run.started", json!({})), event("run.completed", json!({"process":{}}))]);
    assert!(stopped.live.is_none());
}

// missionView.test.ts: which register a live line is (one case per test).
#[test] fn is_thinking_for_a_reasoning_step_whatever_the_runtime_called_it() {
    assert_eq!(live(&[event("step.started",json!({"stepKind":"reasoning","message":"Writing the answer"}))],true,false).unwrap().register,"thinking");
}
#[test] fn is_working_for_a_turn() {
    assert_eq!(live(&[event("step.started",json!({"stepKind":"turn"}))],true,false).unwrap().register,"working");
}
#[test] fn is_writing_for_a_message_item_the_model_producing_prose_not_using_anything() {
    assert_eq!(live(&[event("step.started",json!({"stepKind":"item","itemType":"assistantMessage"}))],true,false).unwrap().register,"writing");
}
#[test] fn is_tool_for_any_other_item_and_drops_the_runtimes_word_for_the_kind() {
    let l=live(&[event("step.started",json!({"stepKind":"item","itemType":"mcpToolCall"}))],true,false).unwrap(); assert_eq!(l.register,"tool"); assert_eq!(l.detail,None);
}
fn opened(name:&str) -> Value {event("tool.started",json!({"itemId":"t1","toolKind":"tool_use","name":name,"phase":"started"}))}
#[test] fn is_the_open_tool_not_the_turn_around_it_while_one_is_running() {
    let l=live(&[event("step.started",json!({"stepKind":"turn"})),event("tool.started",json!({"itemId":"t1","toolKind":"tool_use","name":"Read","command":"README.md","phase":"started"}))],true,false).unwrap(); assert_eq!((l.register.as_str(),l.label.as_str()),("tool","README.md"));
}
#[test] fn reads_a_connector_name_that_carries_no_mcp_prefix() {
    let l=live(&[opened("Google_Drive__create_file")],true,false).unwrap(); assert_eq!((l.register.as_str(),l.label.as_str(),l.detail.as_deref()),("connector","create_file",Some("Google Drive")));
}
#[test] fn does_not_mistake_an_ordinary_tool_for_a_connector() {
    for name in ["Read","Write","Bash","Grep","Task","WebFetch","shell","apply_patch"] {assert_eq!(live(&[opened(name)],true,false).unwrap().register,"tool","{name}");}
}
#[test] fn keeps_a_tool_name_that_contains_a_double_underscore_with_the_server() {
    let l=live(&[opened("mcp__claude_ai_Zapier__run__action")],true,false).unwrap(); assert_eq!((l.register.as_str(),l.label.as_str(),l.detail.as_deref()),("connector","run__action",Some("Zapier")));
}
#[test] fn names_a_connector_as_a_connector_because_it_reaches_off_this_machine() {
    let l=live(&[opened("mcp__claude_ai_Robinhood__get_accounts")],true,false).unwrap(); assert_eq!((l.register.as_str(),l.label.as_str(),l.detail.as_deref()),("connector","get_accounts",Some("Robinhood")));
}
#[test] fn goes_back_to_the_turn_once_the_tool_closes() {
    let l=live(&[event("step.started",json!({"stepKind":"turn"})),event("tool.started",json!({"itemId":"t1","toolKind":"tool_use","name":"Read","command":"README.md","phase":"started"})),event("tool.completed",json!({"itemId":"t1"}))],true,false).unwrap(); assert_eq!(l.register,"working");
}
#[test] fn is_starting_before_the_first_event_and_working_after_it() {
    assert_eq!(live(&[],true,false).unwrap().register,"starting");
    assert_eq!(live(&[event("step.started",json!({"stepKind":"turn"})),event("step.completed",json!({}))],true,false).unwrap().register,"working");
}

// Review (14 Nov): only an error note already says the failure; a quieter note with the same words does not.
#[test]
fn a_note_that_is_not_an_error_does_not_count_as_the_failure_already_shown() {
    let said = "Codex could not reach its server.";
    let info = vec![Item::Note { level: "info".into(), text: said.into() }];
    assert!(!error_already_shown(&info, said));
    let error = vec![Item::Note { level: "error".into(), text: said.into() }];
    assert!(error_already_shown(&error, said));
}

#[test]
fn carries_the_step_kind_so_thought_and_action_draw_differently() {
    // Native draws the meaning directly from the register rather than carrying a second enum.
    let thinking=live(&[event("step.started",json!({"stepKind":"reasoning"}))],true,false).unwrap();
    let acting=live(&[event("step.started",json!({"stepKind":"turn"}))],true,false).unwrap();
    assert_eq!((thinking.label.as_str(),thinking.register.as_str()),("Thinking","thinking"));
    assert_eq!((acting.label.as_str(),acting.register.as_str()),("Working","working"));
}

/// the-start-says-its-phase-on-every-route.test.ts: Electron's Codex app server, OpenCode server and Copilot ACP routes
/// went from "Reading the folder" straight to "Working" before their CLI had booted. Rust has no start-phase words and
/// one live line for every route: after each runtime's own `run.started`, the line still says "Starting" until the run
/// does something.
#[test]
fn the_start_says_starting_on_every_route_until_the_run_does_something() {
    for runtime in ["codex", "claude", "opencode", "copilot", "cursor", "antigravity", "kilo", "muse"] {
        let mut started = event("run.started", json!({"runtime": runtime}));
        started["sourceAdapter"] = json!(runtime);
        let line = live(&[started.clone()], true, false).unwrap();
        assert_eq!(line.label, "Starting", "{runtime}");
        let working = live(&[started, event("step.started", json!({"stepKind": "turn"}))], true, false).unwrap();
        assert_eq!(working.label, "Working", "{runtime}");
    }
}
