use crate::thread_view::live_line;
use serde_json::json;
const NOW: &str = "2026-08-31T16:00:00.000Z";
#[test]
fn counts_from_the_start_of_the_turn_so_it_climbs_instead_of_looping() {
    let events = [json!({"type":"run.started","occurredAt":NOW,"payload":{}}), json!({"type":"adapter.diagnostic","occurredAt":NOW,"payload":{"level":"info","message":"something later"}})];
    let line = live_line(&events, true, "2026-09-05T10:00:00.000Z", false).unwrap();
    assert_eq!(line.started_at, "2026-09-05T10:00:00.000Z");
}
#[test]
fn falls_back_to_the_first_event_when_the_turn_start_is_unknown() {
    let events = [json!({"type":"run.started","occurredAt":NOW,"payload":{}})];
    assert_eq!(live_line(&events, true, "", false).unwrap().started_at, NOW);
}
