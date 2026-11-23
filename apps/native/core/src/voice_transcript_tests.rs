use super::*;
use crate::call_log::Place;

fn new() -> Transcript {
    Transcript::new("tm_ada".into(), "Ada".into(), None, 1_000)
}

#[test]
fn a_partial_line_keeps_its_own_start_when_the_call_ends() {
    let mut t = new();
    t.said(Speaker::You, "What is the secret ingredient?", 5_000);
    t.record_turn("m_asked", 6_000);
    t.delta(Speaker::Voice, " It is cardamom, ", 9_000);
    t.delta(Speaker::Voice, "and ", 9_100);
    t.delta(Speaker::You, "  ", 9_500);
    let saved = t.finish(12_000);
    assert_eq!(saved["endedAt"], 12_000);
    assert_eq!(
        saved["lines"],
        json!([
            {"role":"you","text":"What is the secret ingredient?","at":5000},
            {"role":"voice","text":"It is cardamom, and","at":9000}
        ])
    );
    assert!(saved.get("partial").is_none());
}

#[test]
fn the_first_forty_turns_are_retained_and_later_words_follow_the_last() {
    let mut t = new();
    for index in 0..41 {
        t.record_turn(&format!("m_{index}"), 2_000 + index * 1_000);
    }
    t.said(Speaker::You, "And one more.", 60_000);
    let saved = [t.finish(70_000)];
    assert!(call_log::is_saved_call(&saved[0]));
    assert_eq!(saved[0]["turns"].as_array().unwrap().len(), 40);
    assert_eq!(
        call_log::placements(&saved, &["m_39", "m_40"])[&Place::After("m_39")][0].lines[0]["text"],
        "And one more."
    );
}
