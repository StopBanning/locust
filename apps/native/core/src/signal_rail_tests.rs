use super::*;
use crate::inspector::build_signal_rail as build;
use serde_json::json;
fn event(t: &str, p: Value) -> Value {
    json!({"type":t,"payload":p,"sourceAdapter":"codex","occurredAt":"2026-08-31T16:00:00.000Z"})
}
fn start(command: &str) -> Value {
    event(
        "tool.started",
        json!({"itemId":"t1","name":"shell","toolKind":"command_execution","command":command}),
    )
}
fn done() -> Value {
    event("tool.completed", json!({"itemId":"t1"}))
}
fn names(rows: &[SignalRow]) -> Vec<&str> {
    rows.iter().map(|r| r.name.as_str()).collect()
}
#[test]
fn shows_newest_first() {
    let rows = build(
        &[
            event("run.started", json!({"runtimeThreadId":"t"})),
            start("pnpm test"),
        ],
        true,
    );
    assert_eq!(rows[0].name, "Ran pnpm test");
    assert!(rows[1].name.starts_with("Started on "));
}
#[test]
fn marks_a_tool_live_only_while_it_is_open_and_the_run_is_going() {
    assert_eq!(build(&[start("pnpm test")], true)[0].tone, SignalTone::Live);
    assert_ne!(
        build(&[start("pnpm test")], false)[0].tone,
        SignalTone::Live
    );
    let settled = build(&[start("pnpm test"), done()], true);
    assert_eq!(settled.len(), 1);
    assert_ne!(settled[0].tone, SignalTone::Live);
    assert!(settled[0].meta.ends_with("· done"));
}
#[test]
fn colours_by_meaning_not_decoration() {
    let rows = build(
        &[
            event(
                "route.limit_detected",
                json!({"kind":"temporary-rate-limit","message":"slow down"}),
            ),
            event(
                "run.failed",
                json!({"kind":"process-failed","message":"died","runtimeTerminal":"failed","process":{}}),
            ),
            event("run.completed", json!({"process":{}})),
        ],
        false,
    );
    assert_eq!(
        rows.iter()
            .find(|r| r.name.starts_with("Usage limit"))
            .unwrap()
            .tone,
        SignalTone::Amber
    );
    assert_eq!(
        rows.iter()
            .find(|r| r.name.starts_with("Failed"))
            .unwrap()
            .tone,
        SignalTone::Red
    );
    assert_eq!(
        rows.iter().find(|r| r.name == "Finished").unwrap().tone,
        SignalTone::Blue
    );
}
#[test]
fn says_what_happened_in_words_not_the_ledgers_event_names() {
    let rows = build(
        &[
            event("run.started", json!({"runtimeThreadId":"t"})),
            event("step.started", json!({"stepKind":"agent_reasoning"})),
            event("plan.updated", json!({"final":true,"steps":[]})),
            event(
                "route.limit_detected",
                json!({"kind":"temporary-rate-limit","message":"slow down"}),
            ),
            event("run.cancelled", json!({"process":{}})),
        ],
        false,
    );
    assert_eq!(
        &names(&rows)[..3],
        [
            "Stopped",
            "Usage limit · temporary rate limit",
            "Plan updated · final"
        ]
    );
    assert_eq!(rows.len(), 4);
    assert!(rows[3].name.starts_with("Started on "));
    for r in rows {
        assert!(
            !crate::invariants::regex(r"\b(run|tool|step|route|plan|runtime|adapter)\.[a-z_]+")
                .is_match(&r.name)
        );
    }
}
#[test]
fn says_how_far_the_plan_had_got_not_only_that_it_moved_0_360() {
    let at = |statuses: &[&str]| {
        event(
            "plan.updated",
            json!({"plan":statuses.iter().enumerate().map(|(i,s)|json!({"id":i.to_string(),"content":format!("step {i}"),"status":s})).collect::<Vec<_>>()}),
        )
    };
    let rows = build(
        &[
            at(&["pending", "pending"]),
            at(&["completed", "in_progress"]),
            at(&["completed", "completed"]),
        ],
        false,
    );
    assert_eq!(
        names(&rows),
        [
            "Plan: 2 of 2 done",
            "Plan: 1 of 2 done",
            "Plan: 0 of 2 done"
        ]
    );
}
#[test]
fn says_nothing_about_events_it_does_not_understand() {
    assert!(build(&[event("nonsense.event", json!({}))], true).is_empty());
}
#[test]
fn says_each_call_as_a_sentence_one_row_a_call_with_how_it_ended() {
    let rows = build(
        &[
            event("run.started", json!({"runtimeThreadId":"t"})),
            event("step.started", json!({"stepKind":"turn"})),
            start("npm test"),
            event(
                "tool.completed",
                json!({"itemId":"t1","toolKind":"command_execution","name":"shell","phase":"completed","exitCode":1}),
            ),
            event(
                "tool.started",
                json!({"itemId":"t2","toolKind":"file_change","name":"file_change","phase":"started","command":"C:/work/app/src/signup.ts"}),
            ),
            event(
                "tool.completed",
                json!({"itemId":"t2","toolKind":"file_change","name":"file_change","phase":"completed"}),
            ),
            event(
                "tool.completed",
                json!({"itemId":"t3","toolKind":"tool","name":"read","phase":"completed","command":"C:/work/app/README.md"}),
            ),
            event("step.completed", json!({"stepKind":"reasoning"})),
            event("step.completed", json!({"stepKind":"turn"})),
            event("run.completed", json!({"process":{}})),
        ],
        false,
    );
    assert_eq!(
        &names(&rows)[..5],
        [
            "Finished",
            "Thought",
            "Read README.md",
            "Changed signup.ts",
            "Ran npm test"
        ]
    );
    assert_eq!(rows.len(), 6);
    assert!(rows[5].name.starts_with("Started on "));
    assert!(
        rows.iter()
            .find(|r| r.name == "Ran npm test")
            .unwrap()
            .meta
            .ends_with("· exit code 1")
    );
    assert!(
        rows.iter()
            .find(|r| r.name == "Changed signup.ts")
            .unwrap()
            .meta
            .ends_with("· done")
    );
    for r in rows {
        assert!(!crate::invariants::regex("file_change|finished|Step ").is_match(&r.name));
    }
}
#[test]
fn names_a_call_it_does_not_recognise_by_its_own_name_never_a_guess() {
    assert_eq!(
        tool_name(&json!({"name":"robinhood.get_quotes","command":"NVDA"})),
        "robinhood.get_quotes · NVDA"
    );
    assert_eq!(
        tool_name(&json!({"name":"shell","toolKind":"command_execution"})),
        "Ran a command"
    );
    assert_eq!(
        tool_name(&json!({"name":"WebSearch","command":"oracle force majeure"})),
        "Searched the web for oracle force majeure"
    );
    assert_eq!(
        tool_name(
            &json!({"name":"web_search","toolKind":"web_search","command":"https://releases.electronjs.org"})
        ),
        "Opened https://releases.electronjs.org"
    );
}

// Additional integration coverage: Electron cases above retain their own tests.
#[test]
fn the_ledger_caches_local_rail_clocks_before_the_window_draws() {
    let mut e = event("run.started", json!({}));
    e["sequence"] = json!(1);
    let mut transcript = crate::ledger::Transcript::default();
    transcript.apply(vec![json!({"event":e.clone()})]);
    assert_eq!(transcript.signal_rail, build(&[e], true));
    assert_eq!(transcript.signal_rail[0].meta, crate::time_marker::clock_of("2026-08-31T16:00:00.000Z"));
}
#[test]
fn helper_calls_belong_to_their_activity_row_and_refused_commands_never_ran() {
    let rows = build(&[
        event("tool.completed", json!({"itemId":"child","name":"shell","command":"hidden","parentItemId":"parent"})),
        start("pnpm test"),
        event("tool.failed", json!({"itemId":"t1","status":"refused"})),
    ], false);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Did not run pnpm test");
    assert!(rows[0].meta.ends_with("· refused"));
    assert_eq!(rows[0].tone, SignalTone::Red);
}
