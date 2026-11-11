//! Money helpers — port of `shared/spend.ts`.

use serde_json::Value;

#[cfg(test)]
pub const MAX_MONTHLY_LIMIT_USD: f64 = 100_000.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spend {
    pub usd: Option<f64>,
    pub premium_requests: Option<f64>,
    pub unpriced_runs: Option<u32>,
}

pub fn amount(value: &Value) -> Option<f64> {
    value.as_f64().filter(|n| n.is_finite() && *n >= 0.0)
}

pub fn money_of_usage(usage: &Value) -> (Spend, bool) {
    let plan = usage.get("billing").and_then(Value::as_str) == Some("subscription");
    let usd = if plan {
        None
    } else {
        usage
            .get("usd")
            .filter(|v| !v.is_null())
            .or_else(|| usage.get("totalCostUsd").filter(|v| !v.is_null()))
            .or_else(|| usage.get("total_cost_usd"))
            .and_then(amount)
    };
    let premium_requests = usage.get("premiumRequests").and_then(amount);
    (
        Spend {
            usd,
            premium_requests,
            unpriced_runs: None,
        },
        plan,
    )
}

#[cfg(test)]
pub fn is_own_model(runtime: &str, model: &str) -> bool {
    runtime == "opencode" && model.starts_with("own-")
}

#[cfg(test)]
pub fn sum_spend(list: &[Spend]) -> Option<Spend> {
    let mut usd = None;
    let mut premium_requests = None;
    let mut unpriced_runs = None;
    for spend in list {
        if let Some(v) = spend.usd {
            usd = Some(usd.unwrap_or(0.0) + v);
        }
        if let Some(v) = spend.premium_requests {
            premium_requests = Some(premium_requests.unwrap_or(0.0) + v);
        }
        if let Some(v) = spend.unpriced_runs {
            unpriced_runs = Some(unpriced_runs.unwrap_or(0) + v);
        }
    }
    if usd.is_none() && premium_requests.is_none() && unpriced_runs.is_none() {
        None
    } else {
        Some(Spend {
            usd,
            premium_requests,
            unpriced_runs,
        })
    }
}

#[cfg(test)]
pub fn month_of(at: &str) -> Option<String> {
    use chrono::Datelike;
    let date = chrono::DateTime::parse_from_rfc3339(at)
        .ok()
        .map(|d| d.with_timezone(&chrono::Local))
        .or_else(|| {
            at.parse::<chrono::DateTime<chrono::Utc>>()
                .ok()
                .map(|d| d.with_timezone(&chrono::Local))
        })?;
    Some(format!("{:04}-{:02}", date.year(), date.month()))
}

#[cfg(test)]
pub fn next_month_starts(now: chrono::DateTime<chrono::Local>) -> String {
    use chrono::Datelike;
    let (y, m) = if now.month() == 12 {
        (now.year() + 1, 1)
    } else {
        (now.year(), now.month() + 1)
    };
    let next = chrono::NaiveDate::from_ymd_opt(y, m, 1).expect("valid month start");
    // Portable day number (chrono `%-d` is platform-specific).
    format!("{} {}", next.format("%B"), next.day())
}

#[cfg(test)]
pub fn is_monthly_limit(value: f64) -> bool {
    value.is_finite() && (0.01..=MAX_MONTHLY_LIMIT_USD).contains(&value)
}

pub fn dollars(usd: f64) -> String {
    if usd > 0.0 && usd < 0.01 {
        "< $0.01".into()
    } else {
        // toFixed(2) rounds the exact binary value, with half ties away from zero.
        // Rust's formatter uses ties-to-even (e.g. 0.125), so round integer cents.
        let value = usd.abs();
        let bits = value.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023 - 52;
        let significand = u128::from((bits & ((1u64 << 52) - 1)) | (1u64 << 52)) * 100;
        let cents = if value == 0.0 {
            Some(0)
        } else if exponent < 0 && -exponent < 128 {
            let shift = (-exponent) as u32;
            Some(
                (significand >> shift)
                    + u128::from((significand & ((1u128 << shift) - 1)) >= (1u128 << (shift - 1))),
            )
        } else if exponent >= 0 {
            significand.checked_shl(exponent as u32)
        } else {
            Some(0)
        };
        match cents {
            Some(cents) if value.is_finite() => format!(
                "${}{}.{:02}",
                if usd < 0.0 { "-" } else { "" },
                cents / 100,
                cents % 100
            ),
            _ => format!("${usd:.2}"),
        }
    }
}

#[cfg(test)]
pub fn limit_reached(spent: Option<&Spend>, limit_usd: f64) -> bool {
    spent.and_then(|s| s.usd).unwrap_or(0.0) >= limit_usd
}

#[cfg(test)]
pub fn limit_refusal(
    name: &str,
    spent: Option<&Spend>,
    limit_usd: f64,
    now: chrono::DateTime<chrono::Local>,
) -> String {
    format!(
        "{name} has reached this month's limit: {} of {}. Raise the limit by editing {name}, or it starts again on {}.",
        dollars(spent.and_then(|s| s.usd).unwrap_or(0.0)),
        dollars(limit_usd),
        next_month_starts(now)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nullish_money_and_binary_rounding_match_javascript() {
        assert_eq!(
            money_of_usage(&json!({"usd":null,"totalCostUsd":0.125}))
                .0
                .usd,
            Some(0.125)
        );
        assert_eq!(dollars(0.125), "$0.13");
        assert_eq!(dollars(2.675), "$2.67");
        assert_eq!(dollars(1.005), "$1.00");
    }

    #[test]
    fn money_of_usage_plan_and_api() {
        let (spend, plan) = money_of_usage(&json!({"billing":"subscription","usd":1.5}));
        assert!(plan);
        assert!(spend.usd.is_none());
        let (spend, plan) = money_of_usage(&json!({"usd":0.37,"premiumRequests":2}));
        assert!(!plan);
        assert_eq!(spend.usd, Some(0.37));
        assert_eq!(spend.premium_requests, Some(2.0));
    }

    #[test]
    fn dollars_and_limit_words() {
        assert_eq!(dollars(5.0), "$5.00");
        assert_eq!(dollars(0.004), "< $0.01");
        assert!(is_monthly_limit(0.01));
        assert!(!is_monthly_limit(0.0));
        assert!(limit_reached(
            Some(&Spend {
                usd: Some(10.0),
                ..Default::default()
            }),
            10.0
        ));
        let now = chrono::Local::now();
        let words = limit_refusal(
            "Codec",
            Some(&Spend {
                usd: Some(5.0),
                ..Default::default()
            }),
            5.0,
            now,
        );
        assert!(words.contains("Codec has reached this month's limit: $5.00 of $5.00."));
    }

    #[test]
    fn sum_spend_adds() {
        let sum = sum_spend(&[
            Spend {
                usd: Some(1.0),
                premium_requests: Some(1.0),
                unpriced_runs: None,
            },
            Spend {
                usd: Some(0.5),
                premium_requests: None,
                unpriced_runs: Some(2),
            },
        ])
        .unwrap();
        assert_eq!(sum.usd, Some(1.5));
        assert_eq!(sum.premium_requests, Some(1.0));
        assert_eq!(sum.unpriced_runs, Some(2));
    }

    #[test]
    fn own_model_and_month() {
        assert!(is_own_model("opencode", "own-local"));
        assert!(!is_own_model("codex", "own-local"));
        assert_eq!(
            month_of("2026-09-30T21:00:00.000Z").map(|m| m.len()),
            Some(7)
        );
    }
}
