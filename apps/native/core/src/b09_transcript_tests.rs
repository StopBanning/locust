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
