//! usage-is-drawn.test.tsx, drawn: Home's usage line (not drawn before there is anything to count; the label, the
//! facts, thirty bars and the way in) and the dialog's heatmap grid (twelve weeks, today ringed, the days to come left
//! out, a five-step key).
use super::*;
use chrono::{Local, TimeZone};
use locust_core::usage::{UsageRange, UsageTurn, usage_summary};
use rune::{Env, Runtime};

fn now() -> chrono::DateTime<Local> {
    Local.with_ymd_and_hms(2026, 10, 9, 18, 0, 0).unwrap()
}

fn turn(back: i64, input: Option<u64>, output: Option<u64>) -> UsageTurn {
    let at = (Local.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap() - chrono::Duration::days(back)).with_timezone(&chrono::Utc).to_rfc3339();
    UsageTurn { conversation: format!("c{at}"), at, runtime: "claude".into(), model: "claude-opus-4-6".into(), input_tokens: input, output_tokens: output, cache_read_tokens: None, cache_write_tokens: None, usd: None, premium_requests: None, plan: false }
}

fn line_of(summary: Option<locust_core::usage::UsageSummary>) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    s.usage_line = summary;
    let mut rt = Runtime::new(s, |s, _| col().w(1000.0).children(super::usage_line::line(s, &crate::look::controls::Look::new(true))), (1100.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

#[test]
fn is_not_drawn_before_there_is_anything_to_count_a_new_persons_home_is_unchanged() {
    let empty = usage_summary(&[], UsageRange::Days30, now());
    assert!(line_of(Some(empty)).iter().all(|e| e.key.as_deref() != Some("usage-line")));
    assert!(line_of(None).iter().all(|e| e.key.as_deref() != Some("usage-line")));
}

#[test]
fn draws_the_label_the_facts_thirty_bars_and_the_way_in() {
    let summary = usage_summary(&[turn(0, Some(5_000), Some(500))], UsageRange::Days30, now());
    let all = line_of(Some(summary));
    let said: Vec<String> = all.iter().filter_map(|e| e.text.clone()).collect();
    for words in ["Usage", "last 30 days", "1 turn \u{b7} 5.5k tokens", "Details"] {
        assert!(said.iter().any(|t| t == words), "{words} not in {said:?}");
    }
    let bars = all.iter().filter(|e| e.label.as_deref().is_some_and(|l| l.ends_with(" turn") || l.ends_with(" turns"))).count();
    assert_eq!(bars, 30);
    assert!(all.iter().any(|e| e.key.as_deref() == Some("usage-line-open") && e.role == "button"));
}

#[test]
fn the_grid_twelve_weeks_of_cells_today_ringed_the_days_to_come_left_out() {
    let summary = usage_summary(&[turn(0, None, None), turn(1, None, None)], UsageRange::Days30, now());
    let mut rt = Runtime::new(summary.clone(), |summary, _| super::usage_panel::heatmap(summary, &Palette::new(true)), (600.0, 300.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let keys: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key).collect();
    let count = |prefix: &str| keys.iter().filter(|k| k.starts_with(prefix)).count();
    assert_eq!(count("heat-cell-") + count("heat-legend-"), summary.heatmap.len() + 5);
    assert_eq!(count("heat-week-"), 12);
    assert_eq!(count("heat-today"), 1);
    assert!(keys.iter().any(|k| *k == format!("heat-cell-{}", summary.today)));
    // 9 Oct 2026 is a Friday: Saturday is still to come.
    use chrono::Datelike;
    assert_eq!(count("heat-future-"), 7 - (now().weekday().num_days_from_sunday() as usize + 1));
}

// The dialog draws the grid where it says the month's activity (UsageDialog.tsx puts `<Heatmap>` in its body).
#[test]
fn the_dialog_draws_its_heatmap_grid() {
    let mut s = State::default();
    super::usage_panel::scene(&mut s);
    let mut rt = Runtime::new(s, |s, _| super::usage_panel::view(s, &Palette::new(true)), (900.0, 3000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let keys: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key).collect();
    assert!(keys.iter().any(|k| k == "usage-heatmap"), "{keys:?}");
    assert_eq!(keys.iter().filter(|k| k.starts_with("heat-week-")).count(), 12);
}
