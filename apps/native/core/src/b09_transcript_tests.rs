use super::*;

#[test]
fn b09_live_snapshots_distinguish_partial_from_said() {
    let mut t = Transcript::new("tm_ada".into(), "Ada".into(), None, 1_000);
    t.said(Speaker::You, "Yes, send it.", 5_000);
    t.delta(Speaker::Voice, " Not yet ", 6_000);
    assert_eq!(t.live_snapshot()["partial"], json!({"you":"","voice":"Not yet"}));
    t.said(Speaker::Voice, "Not yet.", 7_000);
    assert_eq!(t.live_snapshot()["partial"]["voice"], "");
    assert_eq!(t.finish(9_000)["lines"][1], json!({"role":"voice","text":"Not yet.","at":6000}));
}

// case: keeps the words still being said, from when they began, and nothing blank
#[test]
fn b09_keeps_spoken_words_and_nonblank_cutoff_words() {
    let mut t = Transcript::new("tm_ada".into(), "Ada".into(), None, 1_000);
    t.said(Speaker::You, "What is the secret ingredient?", 5_000);
    t.record_turn("m_asked", 6_000);
    t.delta(Speaker::Voice, " It is cardamom, and ", 9_000);
    t.delta(Speaker::You, " ", 9_500);
    let saved = t.finish(12_000);
    assert_eq!(saved["lines"], json!([
        {"role":"you","text":"What is the secret ingredient?","at":5000},
        {"role":"voice","text":"It is cardamom, and","at":9000}
    ]));
    assert_eq!(saved["endedAt"], 12000);
    assert!(saved.get("phase").is_none() && saved.get("partial").is_none());
    let saved = [saved];
    assert_eq!(call_log::placements(&saved, &["m_asked"])[&call_log::Place::After("m_asked")][0].lines[0]["text"], "It is cardamom, and");
}

// case: stamps cut-off words with the end when nothing says when they began
#[test]
fn b09_stamps_unknown_start_with_the_end() {
    let mut t = Transcript::new("tm_ada".into(), "Ada".into(), None, 1_000);
    t.delta(Speaker::You, "Thanks", 0);
    assert_eq!(t.finish(12_000)["lines"][0], json!({"role":"you","text":"Thanks","at":12000}));
}

// case: is kept, with its first 40 turns and every line
#[test]
fn b09_keeps_the_first_forty_turns_and_the_words_after_them() {
    let mut t = Transcript::new("tm_ada".into(), "Ada".into(), None, 1_000);
    let ids: Vec<String> = (0..41).map(|n| format!("m_{n}")).collect();
    for (n,id) in ids.iter().enumerate() { t.record_turn(id, 2_000 + n as u64 * 1_000); }
    t.said(Speaker::You, "And one more.", 60_000);
    let saved = [t.finish(70_000)];
    assert_eq!(saved[0]["turns"].as_array().unwrap().len(), 40);
    assert!(call_log::is_saved_call(&saved[0]));
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    assert_eq!(call_log::placements(&saved, &ids)[&call_log::Place::After("m_39")][0].lines[0]["text"], "And one more.");
}

// case: lists each turn it starts once, and a call with a teammate keeps them
#[test]
fn b09_handoffs_are_kept_once_each() {
    let mut t = Transcript::new("tm_ada".into(), "Ada".into(), None, 1_000);
    for _ in 0..2 { t.record_handoff("mis_fix", "tm_ada", "Ada", "Fix the flaky login test", 2_000); }
    t.record_handoff("mis_note", "tm_wren", "Wren", "Draft the release note", 3_000);
    let kept = t.finish(9_000);
    assert!(call_log::is_saved_call(&kept));
    assert_eq!(kept["handed"], json!([
        {"missionId":"mis_fix","teammateId":"tm_ada","name":"Ada","task":"Fix the flaky login test","at":2000},
        {"missionId":"mis_note","teammateId":"tm_wren","name":"Wren","task":"Draft the release note","at":3000}
    ]));
    assert_eq!(kept["turns"], json!([{"missionId":"mis_fix","at":2000}]));
}

// case: leaves out what a kept call could not hold, rather than have the whole call refused when it is saved
#[test]
fn b09_handoffs_refuse_bad_entries_and_keep_forty() {
    let mut t = Transcript::new("tm_ada".into(), "Ada".into(), None, 1_000);
    t.record_handoff("not an id!", "tm_wren", "Wren", "Draft the release note", 3_000);
    t.record_handoff("mis_2", "", "Wren", "Draft the release note", 3_000);
    t.record_handoff("mis_3", "tm_wren", "Wren", "   ", 3_000);
    t.record_handoff("mis_4", "tm_wren", "", "Draft the release note", 3_000);
    t.record_handoff("mis_5", "tm_wren", "Wren", &"x".repeat(900), 3_000);
    assert_eq!(t.snapshot()["handed"], json!([
        {"missionId":"mis_4","teammateId":"tm_wren","name":"Teammate","task":"Draft the release note","at":3000},
        {"missionId":"mis_5","teammateId":"tm_wren","name":"Wren","task":"x".repeat(400),"at":3000}
    ]));
    for i in 0..45 { t.record_handoff(&format!("mis_many_{i}"), "tm_wren", "Wren", "Draft the release note", 3_000); }
    assert_eq!(t.snapshot()["handed"].as_array().unwrap().len(), 40);
    assert!(call_log::is_saved_call(&t.finish(9_000)));
}

// case: is refused when saved only for a to-do no call could hold
#[test]
fn b09_invalid_handoffs_do_not_make_a_saved_call() {
    let t = Transcript::new("tm_ada".into(), "Ada".into(), None, 1_000);
    let base = t.finish(9_000);
    assert!(call_log::is_saved_call(&base));
    let hand = |i: usize| json!({"missionId":format!("mis_{i}"),"teammateId":"tm_wren","name":"Wren","task":"Draft the release note","at":3000});
    let mut blank = hand(0); blank["task"] = json!("");
    let mut long = hand(0); long["name"] = json!("x".repeat(81));
    for invalid in [json!([blank]), json!([long]), json!("all of them"), json!((0..41).map(hand).collect::<Vec<_>>())] {
        let mut invalid_call = base.clone();
        invalid_call["handed"] = invalid;
        assert!(!call_log::is_saved_call(&invalid_call));
    }
}

struct MissionFixture(std::path::PathBuf);
impl Drop for MissionFixture {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}
fn mission_fixture() -> MissionFixture {
    let root = std::env::temp_dir().join(format!("locust-voice-origin-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&root).unwrap();
    MissionFixture(root)
}

fn mission_header(id: &str, voice: bool) -> Value {
    json!({"schemaVersion":23,"recordType":"mission.created","metadata":{
        "missionId":id,"runId":"run_1","prompt": if voice {"Fix the login loop"} else {"Review this"},
        "runtime":"codex","model":"account-default","requestedRouteId":"codex","resolvedRouteId":"codex-account:default",
        "cliVersion":null,"workspaceId":"ws_test","sandbox":if voice {"workspace-write"} else {"read-only"},
        "mode":if voice {"accept-edits"} else {"ask"},"executionPolicyVersion":1,"createdAt":"2026-10-07T00:00:00.000Z",
        "startedBy":if voice {json!({"kind":"mcp","via":"voice"})} else {json!({"kind":"mcp"})}
    }})
}

// case: records schema 23 or later, recovers the origin, and projects it into history without forgetting Ask mode
#[test]
fn b09_external_origin_survives_a_read_but_not_a_version_that_cannot_hold_it() {
    let f = mission_fixture();
    let file = f.0.join("mission_1.jsonl");
    let mut header = mission_header("mission_1", false);
    std::fs::write(&file, format!("{header}\n")).unwrap();
    let read = crate::ledger::read_metadata(&file).unwrap();
    assert_eq!(read.started_by_kind.as_deref(), Some("mcp"));
    assert_eq!(read.mode.as_deref(), Some("ask"));
    assert_eq!(read.prompt, "Review this");
    let history = crate::ledger::list_recent(&f.0, 8);
    assert_eq!(history[0].started_by_kind.as_deref(), Some("mcp"));
    assert_eq!(history[0].mission_id, "mission_1");
    header["schemaVersion"] = json!(22);
    std::fs::write(&file, format!("{header}\n")).unwrap();
    assert!(crate::ledger::read_metadata(&file).is_none(), "schema 22 cannot claim an MCP origin");
}

// case: keeps a voice call’s hand-off as one, across a restart (0.739)
#[test]
fn b09_voice_origin_survives_reopening_the_mission_ledger() {
    let f = mission_fixture();
    let file = f.0.join("mission_2.jsonl");
    std::fs::write(&file, format!("{}\n", mission_header("mission_2", true))).unwrap();
    let read = crate::ledger::read_metadata(&file).unwrap();
    assert!(read.started_by_voice);
    assert_eq!(read.started_by_kind.as_deref(), Some("mcp"));
    assert_eq!(read.mode.as_deref(), Some("accept-edits"));
    assert_eq!(read.prompt, "Fix the login loop");
}

// case: classifies the token file and opt-in as excluded from every profile backup
#[test]
fn b09_profile_backup_excludes_other_apps_token_and_opt_in() {
    let exclusions = crate::profile_backup::LEFT_OUT;
    assert!(exclusions.iter().any(|(file, why)| *file == "locust-mcp-connection.json" && why.contains("token")));
    assert!(exclusions.iter().any(|(file, why)| *file == "locust-mcp.json" && why.contains("enable again")));
    for file in ["locust-mcp-connection.json", "locust-mcp.json"] {
        assert!(!crate::profile_backup::BACKED_UP_FILES.contains(&file));
    }
}
