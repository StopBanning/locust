use super::*;
fn turn(id:&str,prompt:&str,parent:Option<(&str,&str)>)->Metadata{Metadata{mission_id:id.into(),prompt:prompt.into(),runtime:"claude".into(),model:"sonnet".into(),created_at:"2026-09-02T10:00:00.000Z".into(),parent:parent.map(|p|p.0.into()),continues_reason:parent.map(|p|p.1.into()),..Default::default()}}
fn ordinary()->HashMap<String,Metadata>{[turn("m1","check the google stock price",None),turn("m2","cant you look it up for me?",Some(("m1","follow-up"))),turn("m3","what about yesterday?",Some(("m2","follow-up")))].into_iter().map(|m|(m.mission_id.clone(),m)).collect()}
fn ids(m:&Metadata,by:&HashMap<String,Metadata>)->Vec<String>{conversation_turns(m,by).into_iter().map(|t|t.mission_id).collect()}
#[test]fn walks_a_reply_back_to_every_earlier_turn_oldest_first(){let by=ordinary();assert_eq!(conversation_turns(&by["m3"],&by).iter().map(|t|t.prompt.as_str()).collect::<Vec<_>>(),["check the google stock price","cant you look it up for me?","what about yesterday?"]);}
#[test]fn is_just_itself_for_a_first_turn(){let by=ordinary();assert_eq!(ids(&by["m1"],&by),["m1"]);}
#[test]fn does_not_walk_a_route_switch_which_is_a_handoff_and_keeps_its_divider(){let mut by=ordinary();let handed=turn("m4","briefing text",Some(("m1","route-switch")));by.insert("m4".into(),handed.clone());assert_eq!(ids(&handed,&by),["m4"]);}
#[test]fn stops_at_a_missing_or_cyclic_link_rather_than_spinning(){let orphan=turn("m9","reply",Some(("gone","follow-up")));assert_eq!(ids(&orphan,&HashMap::from([("m9".into(),orphan.clone())])),["m9"]);let a=turn("a","a",Some(("b","follow-up")));let b=turn("b","b",Some(("a","follow-up")));assert!(ids(&a,&HashMap::from([("a".into(),a.clone()),("b".into(),b)])).len()<=2);}
#[test]fn draws_no_handoff_divider_across_an_ordinary_reply(){let by=ordinary();assert_eq!(stitched_handoff(&by["m2"],&by),None);}
fn switched()->HashMap<String,Metadata>{let mut a=turn("s1","Remember the word marigold. Reply with just OK.",None);a.runtime="codex".into();let mut b=turn("s2","You are continuing a conversation another agent (Codex) started.\n\nThe person now asks:\n\nWhat was the word?",Some(("s1","route-switch")));b.runtime="opencode".into();b.created_at="2026-09-24T04:13:00.000Z".into();let mut c=turn("s3","And spell it backwards.",Some(("s2","follow-up")));c.runtime="opencode".into();[a,b,c].into_iter().map(|m|(m.mission_id.clone(),m)).collect()}
#[test]fn walks_a_reply_sent_to_another_runtime_back_to_the_turns_before_it_and_marks_the_seam(){let by=switched();let turns=conversation_turns(&by["s2"],&by);assert_eq!(ids(&by["s2"],&by),["s1","s2"]);assert_eq!(turns[0].switched_from,None);let seam=Switch{from:"codex".into(),to:"opencode".into(),unsettled_count:0,omitted_briefing:vec![]};assert_eq!(turns[1].switched_from,Some(seam.clone()));assert_eq!(switch_of(&by["s2"],&by),Some(seam));assert_eq!(typed_prompt(&by["s2"],&by),"What was the word?");assert_eq!(stitched_handoff(&by["s2"],&by),None);}
#[test]fn keeps_the_seam_on_the_switched_turn_once_the_conversation_moves_on(){let by=switched();let turns=conversation_turns(&by["s3"],&by);assert_eq!(ids(&by["s3"],&by),["s1","s2","s3"]);assert_eq!(turns.iter().map(|t|t.switched_from.as_ref().map(|s|s.to.as_str())).collect::<Vec<_>>(),[None,Some("opencode"),None]);}
#[test]fn keeps_the_turns_before_a_running_mission_that_was_handed_over(){let mut by=ordinary();let handed=turn("m5","briefing text",Some(("m2","route-switch")));by.insert("m5".into(),handed.clone());assert_eq!(ids(&handed,&by),["m1","m5"]);assert!(stitched_handoff(&handed,&by).is_some());assert_eq!(switch_of(&handed,&by),None);}

// a-tagged-conversation-shows-the-message.test.ts
#[test]
fn a_tagged_conversation_is_titled_by_the_message() {
    let m = Metadata { prompt: "Can you double-check the tax rate?\n\n(You were tagged in Wren's conversation. Wren's latest answer there, for context:)\n\nI changed TAX_RATE.".into(), ..Default::default() };
    assert_eq!(typed_prompt(&m, &HashMap::new()), "Can you double-check the tax rate?");
    assert_eq!(crate::relay_title::typed_prompt(&m, &HashMap::new(), &HashMap::new()), "Can you double-check the tax rate?");
}

#[test]
fn is_shown_as_the_steps_own_words_not_the_answer_and_rule_it_was_given() {
    let prompt = crate::hand_off::hand_off_prompt("Check the plan.", Some(crate::hand_off::From {name:"Atlas", answer:Some("Plan: remove the second sum.")}), true, None);
    let m=Metadata {mission_id:"mission_3".into(),prompt,started_by_kind:Some("routine".into()),..Default::default()};
    assert_eq!(typed_prompt(&m,&HashMap::new()),"Check the plan.");
}
#[test]
fn are_the_step_not_the_answer_quoted_above_it() {
    let prompt=crate::hand_off::hand_off_prompt("Write a plan to fix it.",Some(crate::hand_off::From{name:"Wren",answer:Some("The total is summed twice.")}),false,None);
    let mut m=Metadata{prompt:prompt.clone(),started_by_kind:Some("routine".into()),..Default::default()};
    assert_eq!(shown_prompt(&m),"Write a plan to fix it.");
    assert_eq!(crate::relay_title::typed_prompt(&m,&HashMap::new(),&HashMap::new()),"Write a plan to fix it.");
    let mut transcript=crate::ledger::Transcript::default();
    transcript.apply(vec![serde_json::json!({"schemaVersion":27,"recordType":"mission.created","metadata":{
        "missionId":"mission_2","prompt":prompt,"startedBy":{"kind":"routine","routineId":"rt_1","step":2}
    }})]);
    assert!(matches!(&transcript.items[0],crate::ledger::Item::User(words) if words == "Write a plan to fix it."));
    m.started_by_kind=None; assert_eq!(shown_prompt(&m),prompt);
}

// apps/desktop/src/renderer/src/a-button-turn-says-what-was-asked.test.ts
fn line_of(prompt: &str, kind: Option<&str>) -> Option<String> {
    let m = Metadata { prompt: prompt.into(), started_by_kind: kind.map(str::to_owned), ..Default::default() };
    turn_line(&m).map(|l| l.into_owned())
}

#[test]
fn a_tidy_pass_what_the_person_asked_for_not_the_brief() {
    let brief = crate::memory_tidy::tidy_prompt();
    assert_eq!(line_of(&brief, None).as_deref(), Some("Tidy this folder's team memory."));
    // The runtime still gets every word, and so does a routine saved from it: only the bubble is the one line.
    let m = Metadata { prompt: brief.clone(), ..Default::default() };
    assert_eq!(typed_prompt(&m, &HashMap::new()), brief);
    let mut t = crate::ledger::Transcript::default();
    t.apply(vec![serde_json::json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"m_tidy","prompt":brief}})]);
    assert!(matches!(&t.items[0], crate::ledger::Item::User(words) if words == "Tidy this folder's team memory."));
}

#[test]
fn a_tidy_pass_sent_with_the_brief_before_0_372_its_example_in_a_code_fence_the_same() {
    let brief = crate::memory_tidy::tidy_prompt();
    let before = [brief.split('\n').next().unwrap(), "Suggest at most 10 changes...", "```", "<locust-tidy>", "</locust-tidy>", "```"].join("\n");
    assert_eq!(line_of(&before, None).as_deref(), Some("Tidy this folder's team memory."));
}

#[test]
fn a_review_whose_work_not_the_review_material() {
    // reviewBrief.ts's opening and sections; Rust has no review button yet, so the brief is written out here.
    let brief = "Jimothy finished a piece of work and you are reviewing it. You did not do this work and you cannot see their conversation; everything known about it is below.\n\nWHAT WAS ASKED FOR\nMake the path handler work on Windows.\n\nWHAT THEY SAID\nI normalised the separators.\n\nWHAT CHANGED\n- src/paths.ts";
    assert_eq!(line_of(brief, None).as_deref(), Some("Review Jimothy's work."));
}

#[test]
fn a_persons_own_words_even_ones_that_begin_the_same_way_are_left_as_they_are() {
    let own = "Jimothy finished a piece of work and you are reviewing it -- tell me what you think.";
    assert_eq!(line_of(own, None).as_deref(), Some(own));
    let tidy_ish = "Tidy this folder's team memory, please.";
    assert_eq!(line_of(tidy_ish, None).as_deref(), Some(tidy_ish));
}

// apps/desktop/src/renderer/src/a-turn-sent-back-to-fix-is-said-as-locusts.test.tsx (the core cases)
fn check_fix_turn(id: &str, fix: u32) -> Metadata {
    let prompt = crate::edit_check::check_fix_prompt("npm test", &["FAIL adds two numbers".into()], false, fix, 2);
    Metadata { mission_id: id.into(), prompt, started_by_kind: Some("check-fix".into()), check_fix: Some(crate::edit_check::CheckFixStarter { fix, of: 2, command: "npm test".into() }), ..Default::default() }
}

#[test]
fn draws_no_bubble_in_the_persons_voice() {
    let m = check_fix_turn("m_fix1", 1);
    assert_eq!(turn_line(&m), None);
    assert_eq!(fix_of_turn(&m), m.check_fix);
    // Recorded with its starter, it is read back as the fix line, never a bubble.
    let mut t = crate::ledger::Transcript::default();
    t.apply(vec![serde_json::json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"m_fix1","prompt":m.prompt,"startedBy":{"kind":"check-fix","fix":1,"of":2,"command":"npm test"}}})]);
    assert!(matches!(&t.items[0], crate::ledger::Item::CheckFix { command, fix: 1, of: 2, .. } if command == "npm test"), "{:?}", t.items);
    assert!(!t.items.iter().any(|i| matches!(i, crate::ledger::Item::User(_))));
}

#[test]
fn a_routines_own_fix_turn_is_locusts_too_read_from_its_words_a_routines_step_stays_the_persons() {
    let check = crate::routine_runner::GoalCheck { command: "node check.js".into(), passed: false, tail: vec!["CHECK FAILED: answer.txt says 41".into()] };
    let fix = crate::routine_runner::goal_fix_prompt(&check, 1, 2);
    let routine = |prompt: &str| Metadata { prompt: prompt.into(), started_by_kind: Some("routine".into()), ..Default::default() };
    assert_eq!(fix_of_turn(&routine(&fix)), Some(crate::edit_check::CheckFixStarter { command: "node check.js".into(), fix: 1, of: 2 }));
    assert_eq!(turn_line(&routine(&fix)), None);
    assert_eq!(turn_line(&routine("Create answer.txt holding 41.")).as_deref(), Some("Create answer.txt holding 41."));
    // The same words typed by the person are the person's: only a routine's turn is read this way.
    let typed = Metadata { prompt: fix.clone(), ..Default::default() };
    assert_eq!(fix_of_turn(&typed), None);
    assert_eq!(turn_line(&typed).as_deref(), Some(fix.as_str()));
    // And the thread draws the routine's fix as the fix line.
    let mut t = crate::ledger::Transcript::default();
    t.apply(vec![serde_json::json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"m_rf","prompt":fix,"startedBy":{"kind":"routine","routineId":"rt_x","step":1}}})]);
    assert!(matches!(&t.items[0], crate::ledger::Item::CheckFix { fix: 1, of: 2, .. }), "{:?}", t.items);
}

#[test]
fn says_why_it_stopped_under_the_turn_it_stopped_after_when_the_conversation_is_opened_again() {
    let stopped = crate::edit_check::stopped_reason::out_of_fixes(2);
    let m = check_fix_turn("m_fix2", 2);
    let mut t = crate::ledger::Transcript::default();
    t.apply(vec![
        serde_json::json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"m_fix2","prompt":m.prompt,"startedBy":{"kind":"check-fix","fix":2,"of":2,"command":"npm test"}}}),
        serde_json::json!({"schemaVersion":27,"recordType":"event","event":{"id":"e1","runId":"r","missionId":"m_fix2","sequence":1,"occurredAt":"2026-10-21T20:00:00.000Z","sourceAdapter":"opencode","type":"message.delta","payload":{"itemId":"m","text":"Tried again.","final":true}}}),
        serde_json::json!({"schemaVersion":27,"recordType":"event","event":{"id":"e2","runId":"r","missionId":"m_fix2","sequence":2,"occurredAt":"2026-10-21T20:00:05.000Z","sourceAdapter":"opencode","type":"adapter.diagnostic","payload":{"level":"warning","code":"host.check_fix","message":stopped,"terminal":false,"evidence":{"redacted":true}}}}),
    ]);
    let at = t.items.iter().position(|i| matches!(i, crate::ledger::Item::Note { text, .. } if *text == stopped));
    let answer = t.items.iter().position(|i| matches!(i, crate::ledger::Item::Assistant { .. }));
    assert!(at.is_some() && answer.is_some() && at > answer, "{:?}", t.items);
}

// apps/desktop/src/renderer/src/the-terminal-turns-are-drawn-as-yours.test.tsx (the core cases)
fn terminal_turns() -> HashMap<String, Metadata> {
    let mut typed_there = turn("mission_2", "Now add tests.", Some(("mission_1", "follow-up")));
    typed_there.started_by_kind = Some("terminal".into());
    let mut also_there = turn("mission_3", "Commit them.", Some(("mission_2", "follow-up")));
    also_there.started_by_kind = Some("terminal".into());
    let back_here = turn("mission_4", "What changed?", Some(("mission_3", "follow-up")));
    [turn("mission_1", "Who takes what?", None), typed_there, also_there, back_here].into_iter().map(|m| (m.mission_id.clone(), m)).collect()
}

#[test]
fn is_the_persons_own_words_with_nothing_attached() {
    let m = Metadata { prompt: "Now add tests.".into(), started_by_kind: Some("terminal".into()), ..Default::default() };
    assert_eq!(turn_line(&m).as_deref(), Some("Now add tests."));
    assert!(turn_attachments(&m).is_empty());
    // Even words that read like Locust's attachment line: nothing came in through Locust.
    let like = Metadata { prompt: "Read this file in the workspace before you answer:\n- NOTES.md\n\nGo.".into(), started_by_kind: Some("terminal".into()), ..Default::default() };
    assert!(turn_attachments(&like).is_empty());
    let own = Metadata { prompt: like.prompt.clone(), ..Default::default() };
    assert_eq!(turn_attachments(&own), ["NOTES.md"]);
}

#[test]
fn carries_its_runtime_through_the_conversations_turns_and_locusts_own_turns_do_not() {
    let by = terminal_turns();
    let got: Vec<(String, Option<String>)> = conversation_turns(&by["mission_4"], &by).into_iter().map(|t| (t.mission_id, t.in_terminal)).collect();
    assert_eq!(got, [("mission_1".into(), None), ("mission_2".into(), Some("claude".into())), ("mission_3".into(), Some("claude".into())), ("mission_4".into(), None)]);
}

#[test]
fn go_into_the_terminal_before_its_first_turn_and_back_in_locust_before_the_next_of_its_own() {
    assert_eq!(terminal_seam_before(None, Some("claude")), Some(TerminalSeam::Into("claude".into())));
    assert_eq!(terminal_seam_before(Some("claude"), Some("claude")), None);
    assert_eq!(terminal_seam_before(Some("claude"), None), Some(TerminalSeam::Back));
    assert_eq!(terminal_seam_before(None, None), None);
    assert_eq!(terminal_seam_before(Some("claude"), Some("codex")), Some(TerminalSeam::Into("codex".into())));
}
