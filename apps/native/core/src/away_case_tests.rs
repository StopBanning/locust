// Carries apps/desktop/src/shared/since-you-were-away.test.ts (Electron 4ce2f37a), the five cases not yet named, on the
// functions the window's home panel and board call (`since_you_were_away`, `away_counts`, `away_line`,
// `away_counts_from`, `was_away`). Expected values are Electron's.
use super::*;
use serde_json::json;
use std::collections::HashSet;

const SINCE: &str = "2026-10-04T01:00:00.000Z";

fn row(mission: &str, phase: &str, last_at: &str) -> AwayRow {
    AwayRow {
        mission_id: mission.into(),
        title: format!("turn {mission}"),
        phase: phase.into(),
        last_at: Some(last_at.into()),
        owner_id: None,
        routine_id: None,
        member_ids: Vec::new(),
        root_id: None,
    }
}
fn item(mission: &str, at: &str) -> AwayItem {
    AwayItem { mission_id: mission.into(), title: format!("turn {mission}"), at: at.into(), owner_id: None, routine_id: None }
}
fn miss(due: &str, recorded: &str) -> AwayRoutineMiss {
    AwayRoutineMiss { due_at: due.into(), recorded_at: recorded.into(), hook_calls: None }
}

// case: lists the runs that ended after you left, newest first, and leaves the earlier ones out
#[test]
fn the_away_list_holds_runs_ended_since_you_left_newest_first() {
    let rows = [
        AwayRow { owner_id: Some("tm_wren".into()), routine_id: Some("rt_1".into()), ..row("m1", "completed", "2026-10-04T03:00:00.000Z") },
        row("m2", "completed", "2026-10-04T05:00:00.000Z"),
        row("m3", "completed", "2026-10-04T00:30:00.000Z"), // before you left
        row("m4", "failed", "2026-10-04T02:00:00.000Z"),
        row("m5", "running", "2026-10-04T06:00:00.000Z"), // not ended
        row("m6", "cancelled", "2026-10-04T04:00:00.000Z"), // not one of the four
    ];
    let summary = since_you_were_away(SINCE, &rows, &[], &HashSet::new()).unwrap();
    assert_eq!(
        summary.ran,
        [
            item("m2", "2026-10-04T05:00:00.000Z"),
            AwayItem { owner_id: Some("tm_wren".into()), routine_id: Some("rt_1".into()), ..item("m1", "2026-10-04T03:00:00.000Z") },
        ]
    );
    assert_eq!(summary.failed, [item("m4", "2026-10-04T02:00:00.000Z")]);
    assert_eq!(summary.missed, []);
    assert_eq!(summary.waiting, 0);
    assert_eq!(summary.since, SINCE);
}

// case: counts the conversations waiting on you now, by any of their turns, and the slots missed while you were gone
#[test]
fn the_away_list_counts_who_waits_on_you_and_the_slots_missed() {
    let rows = [
        AwayRow { member_ids: vec!["m1".into(), "m1b".into()], ..row("m1", "running", "2026-10-04T06:00:00.000Z") },
        AwayRow { root_id: Some("r2".into()), ..row("m2", "running", "2026-10-04T06:00:00.000Z") },
        row("m3", "running", "2026-10-04T06:00:00.000Z"),
    ];
    let routines = [
        AwayRoutine {
            routine_id: "rt_1".into(),
            name: "Morning digest".into(),
            history: vec![
                miss("2026-10-04T05:00:00.000Z", "2026-10-04T07:00:00.000Z"),
                miss("2026-10-03T05:00:00.000Z", "2026-10-03T07:00:00.000Z"), // yesterday's, seen already
                miss("2026-10-04T05:00:00.000Z", "2026-10-04T07:00:00.000Z"), // the same slot twice
            ],
        },
        AwayRoutine { routine_id: "rt_2".into(), name: "Nightly tests".into(), history: vec![miss("2026-10-04T02:00:00.000Z", "2026-10-04T07:00:00.000Z")] },
        AwayRoutine { routine_id: "rt_3".into(), name: "No history".into(), history: Vec::new() },
    ];
    let needs: HashSet<String> = ["m1b", "r2"].into_iter().map(str::to_owned).collect();
    let summary = since_you_were_away(SINCE, &rows, &routines, &needs).unwrap();
    assert_eq!(summary.waiting, 2);
    assert_eq!(
        summary.missed,
        [
            AwayMissed { routine_id: "rt_2".into(), name: "Nightly tests".into(), due_at: "2026-10-04T02:00:00.000Z".into() },
            AwayMissed { routine_id: "rt_1".into(), name: "Morning digest".into(), due_at: "2026-10-04T05:00:00.000Z".into() },
        ]
    );
    assert_eq!(summary.ran, []);
}

// case: words the counts, leaving out what is zero, and says nothing for all zeros
#[test]
fn the_away_line_words_the_counts_and_leaves_out_zeros() {
    let rows = [
        row("m1", "completed", "2026-10-04T05:00:00.000Z"),
        row("m2", "completed", "2026-10-04T05:30:00.000Z"),
        row("m4", "failed", "2026-10-04T02:00:00.000Z"),
    ];
    let summary = since_you_were_away(SINCE, &rows, &[], &HashSet::new()).unwrap();
    let counts = away_counts(&summary);
    assert_eq!(counts, AwaySummaryCounts { ran: 2, failed: 1, missed: 0, waiting: 0 });
    assert_eq!(away_line(Some(counts)).as_deref(), Some("Since you were away: 2 ran · 1 failed"));
    assert_eq!(
        away_line(Some(AwaySummaryCounts { ran: 0, failed: 0, missed: 1, waiting: 3 })).as_deref(),
        Some("Since you were away: 1 missed · 3 waiting on you")
    );
    assert_eq!(away_line(Some(AwaySummaryCounts { ran: 0, failed: 0, missed: 0, waiting: 0 })), None);
    assert_eq!(away_line(None), None);
}

// case: takes the counts off the wire only when they are four whole numbers with something to say (control)
#[test]
fn the_away_counts_are_read_off_the_wire_only_as_four_whole_numbers() {
    assert_eq!(away_counts_from(&json!({"ran": 2, "failed": 0, "missed": 1, "waiting": 0})), Some(AwaySummaryCounts { ran: 2, failed: 0, missed: 1, waiting: 0 }));
    for refused in [
        json!({"ran": 0, "failed": 0, "missed": 0, "waiting": 0}),
        json!({"ran": "2", "failed": 0, "missed": 0, "waiting": 0}),
        json!({"ran": -1, "failed": 0, "missed": 0, "waiting": 0}),
        json!({"ran": 1}),
        json!(null),
        json!("two ran"),
    ] {
        assert_eq!(away_counts_from(&refused), None, "{refused}");
    }
}

// case: counts twenty minutes or more as away, less as a break, and no mark as nothing
#[test]
fn twenty_minutes_or_more_is_away_and_less_is_a_break() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-10-04T08:00:00.000Z").unwrap().timestamp_millis();
    assert_eq!(AWAY_MINIMUM_MS, 20 * 60_000);
    let away = |mark: Option<&str>| was_away(mark, now, AWAY_MINIMUM_MS);
    assert_eq!(away(Some("2026-10-04T07:40:00.000Z")).as_deref(), Some("2026-10-04T07:40:00.000Z"));
    assert_eq!(away(Some("2026-10-04T07:41:00.000Z")), None);
    assert_eq!(away(Some("2026-10-03T22:00:00.000Z")).as_deref(), Some("2026-10-03T22:00:00.000Z"));
    assert_eq!(away(None), None);
    assert_eq!(away(Some("not a time")), None);
}
