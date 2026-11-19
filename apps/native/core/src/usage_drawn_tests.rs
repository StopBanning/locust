//! usage-is-drawn.test.tsx, the words and shapes: Home's line, the dialog's paid-by and spent words, the heatmap's
//! shades and month names, and a token count as the app writes one. Electron's own turns, on its 9 Oct 2026 clock.
use super::*;
use chrono::{Local, TimeZone};

fn now() -> chrono::DateTime<Local> {
    Local.with_ymd_and_hms(2026, 10, 9, 18, 0, 0).unwrap()
}

/// Noon, `back` days before 9 Oct, as an instant (Electron's `at`).
fn at(back: i64) -> String {
    let noon = Local.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap() - chrono::Duration::days(back);
    noon.with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn turn(at: String) -> UsageTurn {
    UsageTurn {
        conversation: format!("c{at}"),
        at,
        runtime: "claude".into(),
        model: "claude-opus-4-6".into(),
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_write_tokens: None,
        usd: None,
        premium_requests: None,
        plan: false,
    }
}

#[test]
fn says_the_month_in_one_line_turns_tokens_and_money_only_when_something_was_priced() {
    let planned = usage_summary(
        &[UsageTurn { plan: true, input_tokens: Some(100), cache_read_tokens: Some(1_200_000), output_tokens: Some(5_000), ..turn(at(0)) }],
        UsageRange::Days30,
        now(),
    );
    assert_eq!(usage_line_facts(&planned), "1 turn \u{b7} 1.2M tokens");
    let priced = usage_summary(
        &[UsageTurn { usd: Some(0.31), input_tokens: Some(900), output_tokens: Some(100), ..turn(at(0)) }, UsageTurn { usd: Some(0.2), ..turn(at(1)) }],
        UsageRange::Days30,
        now(),
    );
    assert_eq!(usage_line_facts(&priced), "2 turns \u{b7} 1.0k tokens \u{b7} $0.51 spent");
}

// case: a bar a day, the busiest full height, a quiet day flat, today marked
#[test]
fn thirty_bars_from_electrons_own_turns_the_busiest_full_today_marked() {
    let summary = usage_summary(&[turn(at(0)), turn(at(0)), turn(at(2))], UsageRange::Days30, now());
    let bars = spark_bars(&summary.days, &summary.today);
    assert_eq!(bars.len(), 30);
    let last = bars.last().unwrap();
    assert!(last.today && last.height == 1.0 && last.turns == 2, "{last:?}");
    assert_eq!(bars[bars.len() - 3].height, 0.5);
    assert_eq!(bars[bars.len() - 2].height, 0.0);
}

fn model(runtime: &str, plan_turns: u64, free_turns: u64, usd: Option<f64>, premium: Option<f64>) -> UsageModel {
    UsageModel {
        runtime: runtime.into(),
        model: "m".into(),
        turns: 4,
        tokens: UsageTokens::default(),
        pay: UsagePay { usd, premium_requests: premium, plan_turns, free_turns },
    }
}

#[test]
fn names_how_each_model_was_paid_for_in_the_receipts_words() {
    assert_eq!(paid_by(&model("claude", 4, 0, None, None)), "your plan");
    assert_eq!(paid_by(&model("claude", 0, 0, Some(0.37), None)), "$0.37");
    assert_eq!(paid_by(&model("opencode", 0, 4, None, None)), "free");
    assert_eq!(paid_by(&model("copilot", 0, 0, None, Some(3.0))), "3 premium requests");
    // Codex runs on the account it is signed in to and prices nothing per turn.
    assert_eq!(paid_by(&model("codex", 0, 0, None, None)), "your account");
    assert_eq!(paid_by(&model("cursor-unknown", 0, 0, None, None)), "not reported");
    assert_eq!(paid_by(&model("claude", 3, 0, Some(0.02), None)), "$0.02 \u{b7} 3 in your plan");
}

#[test]
fn says_what_was_spent_or_that_nothing_was_beyond_the_plans() {
    let planned = usage_summary(&[UsageTurn { plan: true, ..turn(at(0)) }], UsageRange::Days30, now());
    assert_eq!(spent_sentence(&planned), "Nothing spent beyond your plans and free models.");
    let spent = usage_summary(
        &[UsageTurn { usd: Some(1.5), ..turn(at(0)) }, UsageTurn { runtime: "copilot".into(), model: "auto".into(), premium_requests: Some(2.0), ..turn(at(0)) }],
        UsageRange::Days30,
        now(),
    );
    assert_eq!(spent_sentence(&spent), "Spent $1.50 on API keys and paid models and 2 Copilot premium requests.");
}

#[test]
fn a_heatmap_cell_is_one_of_five_shades_by_steps_of_the_busiest_day() {
    assert_eq!([0, 1, 3, 6, 8, 10].map(|t| heat_level(t, 10)), [0, 1, 2, 3, 4, 4]);
    assert_eq!(heat_level(0, 0), 0);
}

#[test]
fn names_a_month_over_the_week_it_starts_in_and_never_two_names_run_together() {
    let summary = usage_summary(&[turn(at(0))], UsageRange::All, now());
    let columns = heat_columns(&summary.heatmap);
    assert_eq!(columns.len(), 12);
    let named: Vec<(usize, String)> = columns.iter().enumerate().filter_map(|(i, c)| c.month.clone().map(|m| (i, m))).collect();
    for month in ["Aug", "Sep", "Oct"] {
        assert!(named.iter().any(|(_, m)| m == month), "{named:?}");
    }
    for pair in named.windows(2) {
        assert!(pair[1].0 - pair[0].0 >= 3, "{named:?}");
    }
}

#[test]
fn writes_a_token_count_the_way_the_app_does_everywhere() {
    assert_eq!([920, 12_400, 3_100_000, 19_000_000].map(compact_count), ["920", "12k", "3.1M", "19M"]);
}

#[test]
fn writes_a_billion_and_more_as_b_never_as_a_thousand_of_the_unit_below() {
    assert_eq!([1_133_000_000, 12_500_000_000, 999_600_000, 999_999, 9_960_000].map(compact_count), ["1.1B", "13B", "1.0B", "1.0M", "10M"]);
}
