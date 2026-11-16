//! The teammate's monthly receipt and editable limit, separate from all-time window usage.
use crate::spend::{Spend, dollars, is_monthly_limit, limit_reached};
use serde_json::{Value, json};

pub const UNPRICED_NOTE: &str = "A model of your own reports no price, so its runs do not count toward the limit. Check its provider for what they cost.";
#[derive(Debug, PartialEq)]
pub struct MonthLine {
    pub text: String,
    pub reached: bool,
    pub unpriced: Option<String>,
    pub note: Option<&'static str>,
}
pub fn month_spend_line(spend: Option<&Spend>, limit: Option<f64>) -> Option<MonthLine> {
    let empty = Spend::default();
    let spend = spend.unwrap_or(&empty);
    let requests = spend
        .premium_requests
        .map(|n| format!("{n} premium request{}", if n == 1.0 { "" } else { "s" }));
    let text = if let Some(limit) = limit {
        let base = format!(
            "{} of {}",
            dollars(spend.usd.unwrap_or(0.0)),
            dollars(limit)
        );
        requests.map_or(base.clone(), |r| format!("{base} · {r}"))
    } else {
        match (spend.usd, requests) {
            (Some(n), Some(_)) => dollars(n),
            (Some(n), None) => dollars(n),
            (None, Some(r)) => r,
            (None, None) => return None,
        }
    };
    let unpriced = spend
        .unpriced_runs
        .filter(|n| *n > 0 && limit.is_some())
        .map(|n| format!("{n} run{} with no price", if n == 1 { "" } else { "s" }));
    Some(MonthLine {
        text,
        reached: limit.is_some_and(|l| limit_reached(Some(spend), l)),
        note: unpriced.as_ref().map(|_| UNPRICED_NOTE),
        unpriced,
    })
}
pub fn parsed_limit(text: &str) -> Result<Option<f64>, &'static str> {
    let text = text
        .trim()
        .strip_prefix('$')
        .unwrap_or(text.trim())
        .replace(',', "");
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    // Decimal input only: exponent notation and signs are never a person's dollar amount here.
    if !text.chars().all(|c| c.is_ascii_digit() || c == '.')
        || text.matches('.').count() > 1
        || !text.chars().any(|c| c.is_ascii_digit())
    {
        return Err("Enter a monthly limit between $0.01 and $100,000, or leave it empty.");
    }
    let amount = text.parse::<f64>().unwrap_or(f64::NAN);
    let amount = (amount * 100.0).round() / 100.0;
    if is_monthly_limit(amount) {
        Ok(Some(amount))
    } else {
        Err("Enter a monthly limit between $0.01 and $100,000, or leave it empty.")
    }
}
pub fn limit_said(limit: Option<f64>, editing: bool, before: Option<f64>) -> Value {
    if editing && limit == before {
        return json!({});
    }
    match limit {
        Some(n) => json!({"monthlyLimitUsd": n}),
        None if editing => json!({"monthlyLimitUsd": null}),
        None => json!({}),
    }
}
pub fn limit_hint(name: &str, spent: Option<&Spend>, editing: bool) -> String {
    let how = format!(
        "Checked before each run: once this month's priced runs reach it, {name} starts nothing until you raise it or the month ends. A run already going finishes. Plans and free models are not priced, so they never count. Nor do models of your own, which report no price; {name}'s card counts those runs."
    );
    if let Some(usd) = spent.and_then(|s| s.usd).filter(|_| editing) {
        format!("{} spent this month so far. {how}", dollars(usd))
    } else {
        how
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn is_money_or_no_row_and_this_month_not_a_total_of_whatever_the_window_held() {
        assert_eq!(
            month_spend_line(
                Some(&Spend {
                    usd: Some(3.2),
                    ..Default::default()
                }),
                None
            )
            .unwrap()
            .text,
            "$3.20"
        );
        assert_eq!(month_spend_line(None, None), None);
    }
    #[test]
    fn says_it_against_the_limit_whenever_there_is_one_even_at_nothing_spent() {
        assert_eq!(
            month_spend_line(None, Some(5.)).unwrap().text,
            "$0.00 of $5.00"
        );
        for (usd, text, reached) in [
            (5.02, "$5.02 of $5.00", true),
            (1., "$1.00 of $5.00", false),
        ] {
            let line = month_spend_line(
                Some(&Spend {
                    usd: Some(usd),
                    ..Default::default()
                }),
                Some(5.),
            )
            .unwrap();
            assert_eq!((line.text, line.reached), (text.into(), reached));
        }
    }
    #[test]
    fn says_runs_on_a_model_of_your_own_beside_the_dollars_with_why_unpriced_not_zero() {
        for (usd, count, text, unpriced) in [
            (Some(0.4), 2, "$0.40 of $5.00", "2 runs with no price"),
            (None, 1, "$0.00 of $5.00", "1 run with no price"),
        ] {
            assert_eq!(
                month_spend_line(
                    Some(&Spend {
                        usd,
                        unpriced_runs: Some(count),
                        ..Default::default()
                    }),
                    Some(5.)
                ),
                Some(MonthLine {
                    text: text.into(),
                    reached: false,
                    unpriced: Some(unpriced.into()),
                    note: Some(UNPRICED_NOTE)
                })
            );
        }
        assert_eq!(
            month_spend_line(
                Some(&Spend {
                    unpriced_runs: Some(3),
                    ..Default::default()
                }),
                None
            ),
            None
        );
    }
    #[test]
    fn says_premium_requests_beside_the_dollars_never_toward_them() {
        let s = Spend {
            usd: Some(1.),
            premium_requests: Some(3.),
            ..Default::default()
        };
        assert_eq!(
            month_spend_line(Some(&s), Some(5.)).unwrap().text,
            "$1.00 of $5.00 · 3 premium requests"
        );
        assert!(!month_spend_line(Some(&s), Some(5.)).unwrap().reached);
        assert_eq!(
            month_spend_line(
                Some(&Spend {
                    premium_requests: Some(3.),
                    ..Default::default()
                }),
                None
            )
            .unwrap()
            .text,
            "3 premium requests"
        );
    }
    #[test]
    fn reads_amounts_as_a_person_types_them_and_nothing_as_no_limit() {
        for text in ["", "  "] {
            assert_eq!(parsed_limit(text), Ok(None));
        }
        for (text, amount) in [
            ("5", 5.),
            ("$12.50", 12.5),
            ("12.", 12.),
            ("1,000", 1000.),
            (".5", 0.5),
            ("12.345", 12.35),
        ] {
            assert_eq!(parsed_limit(text), Ok(Some(amount)));
        }
    }
    #[test]
    fn says_a_limit_on_an_edit_only_when_it_changed() {
        for (limit, editing, before, want) in [
            (Some(5.), true, Some(5.), json!({})),
            (None, true, None, json!({})),
            (Some(8.), true, Some(5.), json!({"monthlyLimitUsd":8.0})),
            (None, true, Some(5.), json!({"monthlyLimitUsd":null})),
            (Some(5.), false, None, json!({"monthlyLimitUsd":5.0})),
            (None, false, None, json!({})),
        ] {
            assert_eq!(limit_said(limit, editing, before), want);
        }
    }
    #[test]
    fn refuses_what_is_not_an_amount_rather_than_saving_a_limit_nobody_meant() {
        for bad in ["0", "-5", "five", "1e3", "5$", "100001"] {
            assert!(parsed_limit(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn says_how_the_limit_works_and_what_has_been_spent_when_editing() {
        let hint = limit_hint("Wren", None, false);
        assert!(hint.contains("Checked before each run"));
        assert!(hint.contains("Plans and free models are not priced"));
        assert!(
            limit_hint(
                "Wren",
                Some(&Spend {
                    usd: Some(3.2),
                    ..Default::default()
                }),
                true
            )
            .starts_with("$3.20 spent this month so far.")
        );
    }
}

/// The calendar month from the complete host usage ledger, never the window's recent rows.
pub fn spend_for_month(
    turns: &[crate::usage::UsageTurn],
    month: &str,
    owns: impl Fn(&str) -> bool,
) -> Option<Spend> {
    let mut total = Spend::default();
    for turn in turns.iter().filter(|t| {
        crate::spend::month_of(&t.at).as_deref() == Some(month) && owns(&t.conversation)
    }) {
        if let Some(n) = turn.usd {
            *total.usd.get_or_insert(0.) += n;
        }
        if let Some(n) = turn.premium_requests {
            *total.premium_requests.get_or_insert(0.) += n;
        }
        if turn.usd.is_none()
            && turn.premium_requests.is_none()
            && !turn.plan
            && turn
                .input_tokens
                .unwrap_or(0)
                .saturating_add(turn.output_tokens.unwrap_or(0))
                > 0
            && crate::spend::is_own_model(&turn.runtime, &turn.model)
        {
            *total.unpriced_runs.get_or_insert(0) += 1;
        }
    }
    (total != Spend::default()).then_some(total)
}

#[cfg(test)]
mod ledger_tests {
    use super::*;
    #[test]
    fn counts_only_the_calendar_month_and_owner_without_pricing_a_plan_or_free_model() {
        let turn = |at: &str, conversation: &str, model: &str, usd, requests, plan| {
            crate::usage::UsageTurn {
                at: at.into(),
                conversation: conversation.into(),
                runtime: "opencode".into(),
                model: model.into(),
                usd,
                premium_requests: requests,
                plan,
                input_tokens: Some(1),
                output_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
            }
        };
        let at = "2026-09-15T12:00:00Z";
        let turns = vec![
            turn(at, "m", "own-a/model", Some(0.4), Some(3.), false),
            turn(at, "m", "own-a/model", None, None, false),
            turn(at, "m", "free", None, None, false),
            turn(at, "m", "own-a/model", None, None, true),
            turn(at, "other", "own-a/model", Some(999.), None, false),
            turn(
                "2026-08-15T12:00:00Z",
                "m",
                "own-a/model",
                Some(999.),
                None,
                false,
            ),
        ];
        assert_eq!(
            spend_for_month(&turns, "2026-09", |c| c == "m"),
            Some(Spend {
                usd: Some(0.4),
                premium_requests: Some(3.),
                unpriced_runs: Some(1)
            })
        );
        assert_eq!(spend_for_month(&turns, "2026-10", |_| true), None);
    }
}
