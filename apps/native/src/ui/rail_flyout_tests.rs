//! Electron's `railFlyout.test.ts`, case by case with its values and words, on `rail.rs`'s `rail_rows`,
//! `count_badge` (`railCountBadge`), `short_ago` and `empty_line` (`railEmptyLine`).
use super::*;

const NOW: &str = "2026-09-08T12:00:00.000Z";
fn now_ms() -> i64 {
    chrono::DateTime::parse_from_rfc3339(NOW).unwrap().timestamp_millis()
}
fn row(id: &str, phase: &str, last_at: Option<&str>) -> SidebarMission {
    SidebarMission { mission_id: id.into(), title: id.into(), phase: phase.into(), last_at: last_at.map(Into::into),
        root_id: None, member_ids: vec![], owner_id: None, said: String::new(), nested_under: None, integrity_issue_count: 0, needs_you: false, app_started: false }
}
fn ids(rows: &[SidebarMission]) -> Vec<&str> {
    rows.iter().map(|r| r.mission_id.as_str()).collect()
}

#[test]
fn puts_the_live_one_first_then_newest_first() {
    let (shown, _) = rail_rows(&[
        row("old", "completed", Some("2026-09-01T00:00:00.000Z")),
        row("new", "completed", Some("2026-09-08T11:00:00.000Z")),
        row("live", "running", Some("2026-09-07T00:00:00.000Z")),
    ]);
    assert_eq!(ids(&shown), ["live", "new", "old"]);
}

#[test]
fn never_lets_an_unknown_age_outrank_a_known_recent_one() {
    let (shown, _) = rail_rows(&[row("dated", "completed", Some("2026-09-08T11:00:00.000Z")), row("undated", "completed", None)]);
    assert_eq!(shown[0].mission_id, "dated");
}

#[test]
fn shows_at_most_six_and_says_how_many_there_are() {
    let many: Vec<SidebarMission> = (0..9).map(|i| row(&format!("m{i}"), "completed", Some(&format!("2026-09-0{}T00:00:00.000Z", 1 + i % 8)))).collect();
    let (shown, label) = rail_rows(&many);
    assert_eq!(shown.len(), RAIL_ROWS_SHOWN);
    assert_eq!(label, "6 of 9");
    assert_eq!(rail_rows(&many[..3]).1, "3");
}

#[test]
fn the_count_badge_is_drawn_only_past_one_conversation() {
    assert_eq!(count_badge(0), None);
    assert_eq!(count_badge(1), None);
    assert_eq!(count_badge(2).as_deref(), Some("2"));
    assert_eq!(count_badge(14).as_deref(), Some("14"));
}

#[test]
fn an_age_reads_now_minutes_hours_days_weeks() {
    let at = |seconds_ago: i64| (chrono::DateTime::parse_from_rfc3339(NOW).unwrap() - chrono::Duration::seconds(seconds_ago)).to_rfc3339();
    let ago = |seconds_ago: i64| short_ago(Some(&at(seconds_ago)), now_ms());
    assert_eq!(ago(5).as_deref(), Some("now"));
    assert_eq!(ago(5 * 60).as_deref(), Some("5m"));
    assert_eq!(ago(2 * 3_600).as_deref(), Some("2h"));
    assert_eq!(ago(3 * 86_400).as_deref(), Some("3d"));
    assert_eq!(ago(15 * 86_400).as_deref(), Some("2w"));
}

#[test]
fn an_age_says_nothing_for_a_missing_or_unreadable_time_rather_than_a_wrong_one() {
    assert_eq!(short_ago(None, now_ms()), None);
    assert_eq!(short_ago(Some("not a date"), now_ms()), None);
}

#[test]
fn the_empty_case_names_the_teammate_and_points_at_the_composer() {
    assert_eq!(empty_line("Juno"), "Nothing yet — pick Juno and write below.");
}
