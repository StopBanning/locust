use serde_json::Value;
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub used: u64,
    pub window: u64,
    pub percent: u64,
}
impl Reading {
    pub fn from_usage(v: &Value) -> Option<Self> {
        let used = v.get("contextTokens")?.as_u64()?;
        let window = v.get("contextWindow")?.as_u64()?;
        if used == 0 || window == 0 {
            return None;
        }
        Some(Self {
            used,
            window,
            percent: ((used as f64 / window as f64 * 100.0).round() as u64).min(100),
        })
    }
    pub fn sentence(&self) -> String {
        format!(
            "Context: {} of {} used, {}%",
            compact_count(self.used),
            compact_count(self.window),
            self.percent
        )
    }
}

/// Conversation-scoped ring wording. Subscription receipts carry no per-turn charge.
pub fn ring_sentence(reading: &Reading, cost: Option<&crate::benchmark::RunCost>) -> String {
    let line = crate::benchmark::cost_total(&[cost.cloned()]).map(|total| total.line);
    match line {
        Some(line) => format!("{} — This conversation: {line}", reading.sentence()),
        None => reading.sentence(),
    }
}

/// Clockwise used share, starting at twelve o'clock; a full window is a disc.
/// The native renderer tessellates this with the same SVG parser as bot outlines.
pub fn pie_slice_path(percent: f64, centre: f64, radius: f64) -> Option<String> {
    let share = percent.clamp(0.0, 100.0) / 100.0;
    if share <= 0.0 { return None; }
    if share >= 0.999 {
        return Some(format!("M {} {} a {radius} {radius} 0 1 0 {} 0 a {radius} {radius} 0 1 0 {} 0 Z",
            centre - radius, centre, radius * 2.0, -radius * 2.0));
    }
    let angle = share * std::f64::consts::TAU;
    let round = |v: f64| (v * 100.0).round() / 100.0;
    Some(format!("M {} {} L {} {} A {} {} 0 {} 1 {} {} Z",
        round(centre), round(centre), round(centre), round(centre - radius),
        round(radius), round(radius), u8::from(angle > std::f64::consts::PI),
        round(centre + radius * angle.sin()), round(centre - radius * angle.cos())))
}

/// Electron `usageView.compactCount`: 1.1B, 3.1M, 410k, 920.
pub fn compact_count(value: u64) -> String {
    const UNITS: [(u64, &str); 3] = [(1_000_000_000, "B"), (1_000_000, "M"), (1_000, "k")];
    for (index, (size, unit)) in UNITS.iter().enumerate() {
        if value < *size {
            continue;
        }
        let scaled = value as f64 / *size as f64;
        let text = if scaled >= 9.95 {
            format!("{scaled:.0}")
        } else {
            format!("{scaled:.1}")
        };
        if text == "1000"
            && let Some((larger_size, larger_unit)) =
                index.checked_sub(1).and_then(|i| UNITS.get(i)).copied()
        {
            let _ = larger_size;
            return format!("1.0{larger_unit}");
        }
        return format!("{text}{unit}");
    }
    value.to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn only_runtime_measured_context_is_shown() {
        assert!(Reading::from_usage(&json!({"inputTokens":100,"contextWindow":200})).is_none());
        assert!(Reading::from_usage(&json!({"contextTokens":100,"contextWindow":0})).is_none());
        let r =
            Reading::from_usage(&json!({"contextTokens":164000,"contextWindow":200000})).unwrap();
        assert_eq!(r.sentence(), "Context: 164k of 200k used, 82%");
        assert_eq!(
            Reading::from_usage(&json!({"contextTokens":300,"contextWindow":100}))
                .unwrap()
                .percent,
            100
        );
    }

    #[test]
    fn compact_count_matches_electron_words() {
        assert_eq!(compact_count(920), "920");
        assert_eq!(compact_count(1_000), "1.0k");
        assert_eq!(compact_count(1_500), "1.5k");
        assert_eq!(compact_count(410_000), "410k");
        assert_eq!(compact_count(999_500), "1.0M");
        assert_eq!(compact_count(1_100_000), "1.1M");
        assert_eq!(compact_count(9_950_000), "10M");
        assert_eq!(compact_count(999_600_000), "1.0B");
        assert_eq!(compact_count(1_133_000_000), "1.1B");
    }
}

#[cfg(test)]
mod ledger_tests {
    use super::*;
    use crate::ledger;
    use serde_json::json;
    #[test]
    fn compaction_is_a_line_and_invalidates_context() {
        let mut t = ledger::Transcript::default();
        let event = |n, kind: &str, payload: Value| json!({"recordType":"mission.event","event":{"sequence":n,"type":kind,"payload":payload}});
        t.apply(vec![event(
            1,
            "run.completed",
            json!({"usage":{"contextTokens":164000,"contextWindow":200000}}),
        )]);
        assert_eq!(t.context.as_ref().unwrap().percent, 82);
        t.apply(vec![event(2,"adapter.diagnostic",json!({"code":"opencode.context_compacted","level":"info","message":"The conversation outgrew the model's context, so OpenCode summarized it and carried on from the summary."}))]);
        assert!(t.context.is_none());
        assert!(
            matches!(&t.items[..],[ledger::Item::Note {level,text}] if level.is_empty() && text.contains("OpenCode summarized"))
        );
        t.apply(vec![event(
            3,
            "run.completed",
            json!({"usage":{"contextTokens":10,"contextWindow":100}}),
        )]);
        assert_eq!(t.context.as_ref().unwrap().percent, 10);
    }
}

#[cfg(test)]
mod renderer_ring_tests {
    use crate::{ledger::Transcript, relay::mission_cost_from_events};
    use serde_json::{Value, json};

    fn long_run() -> Value {
        json!({"usd":3.06,"inputTokens":46,"outputTokens":31000,
            "cacheReadTokens":4796000,"cacheWriteTokens":182697,"contextWindow":1000000})
    }
    fn receipt(usage: Value) -> Value {
        json!({"id":"e","runId":"r","missionId":"m","sequence":1,
            "occurredAt":"2026-09-23T04:22:00.000Z","sourceAdapter":"claude",
            "type":"run.completed","payload":{"usage":usage,"process":{}}})
    }
    fn transcript(usage: Value) -> Transcript {
        let mut t = Transcript::default();
        t.apply(vec![json!({"recordType":"mission.event","event":receipt(usage)})]);
        t
    }
    #[test]
    fn reads_what_the_conversation_held_after_the_last_call_not_the_run_added_up() {
        let mut usage = long_run();
        usage["contextTokens"] = json!(236757);
        let t = transcript(usage);
        let reading = t.context.unwrap();
        assert_eq!((reading.used, reading.window, reading.percent), (236757, 1000000, 24));
        assert_eq!(reading.sentence(), "Context: 237k of 1.0M used, 24%");
    }
    #[test]
    fn draws_nothing_for_a_receipt_that_cannot_say_what_the_conversation_held() {
        assert!(super::Reading::from_usage(&long_run()).is_none());
        assert!(transcript(long_run()).context.is_none());
    }
    #[test]
    fn carries_no_dollar_figure_nobody_paid_it() {
        let mut usage = long_run();
        usage["contextTokens"] = json!(236757);
        usage["billing"] = json!("subscription");
        let cost = mission_cost_from_events(&[receipt(usage)]).unwrap();
        assert_eq!(cost.usd, None);
        assert!(cost.plan);
    }
    #[test]
    fn says_on_its_receipt_that_the_plan_covered_it() {
        let mut usage = long_run();
        usage["contextTokens"] = json!(236757);
        usage["billing"] = json!("subscription");
        let event = receipt(usage);
        let cost = mission_cost_from_events(&[event.clone()]).unwrap();
        assert_eq!(crate::benchmark::cost_line(Some(&cost)).as_deref(), Some("in your plan"));
        assert_eq!(crate::inspector::cost_label_and_line(&[event], false), ("Cost", "in your plan".into()));
    }
    #[test]
    fn keeps_the_dollars_on_a_run_with_no_subscription_behind_it() {
        let cost = mission_cost_from_events(&[receipt(long_run())]).unwrap();
        assert_eq!(crate::benchmark::cost_line(Some(&cost)).as_deref(), Some("$3.06"));
    }
    #[test]
    fn gives_the_ring_nothing_to_say_about_cost_only_the_context() {
        let mut usage = long_run();
        usage["contextTokens"] = json!(236757);
        usage["billing"] = json!("subscription");
        let cost = mission_cost_from_events(&[receipt(usage.clone())]).unwrap();
        let t = transcript(usage);
        let hover = super::ring_sentence(t.context.as_ref().unwrap(), Some(&cost));
        assert_eq!(hover, "Context: 237k of 1.0M used, 24%");
        assert!(!hover.contains('$'));
    }
    #[test]
    fn still_prices_what_was_priced_beside_it() {
        let mut usage = long_run();
        usage["contextTokens"] = json!(236757);
        usage["billing"] = json!("subscription");
        let plan = mission_cost_from_events(&[receipt(usage.clone())]).unwrap();
        let priced = mission_cost_from_events(&[receipt(json!({"usd":0.4,"inputTokens":10,"outputTokens":5}))]).unwrap();
        let cost = crate::relay::sum_exchange_costs([Some(plan), Some(priced)].into_iter()).unwrap();
        let t = transcript(usage);
        assert_eq!(super::ring_sentence(t.context.as_ref().unwrap(), Some(&cost)),
            "Context: 237k of 1.0M used, 24% — This conversation: $0.40");
    }

    #[test]
    fn fills_a_slice_from_twelve_oclock_the_size_of_the_share_used() {
        assert_eq!(super::pie_slice_path(0.0, 7.0, 4.5), None);
        assert_eq!(super::pie_slice_path(25.0, 7.0, 4.5).as_deref(), Some("M 7 7 L 7 2.5 A 4.5 4.5 0 0 1 11.5 7 Z"));
        assert!(super::pie_slice_path(75.0, 7.0, 4.5).unwrap().contains(" 0 1 1 "));
        let full = super::pie_slice_path(100.0, 7.0, 4.5).unwrap();
        assert!(full.starts_with("M 2.5 7 a 4.5 4.5 0 1 0 9 0"));
        let paths = crate::bot_spec::parse_svg_subpaths(&full);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].iter().any(|p| p[0] > 11.4), "the full disc reaches its right edge");
    }

    #[test]
    fn states_no_total_for_runs_a_subscription_covered() {
        let mut usage = long_run();
        usage["billing"] = json!("subscription");
        let plan = mission_cost_from_events(&[receipt(usage)]);
        assert_eq!(crate::benchmark::cost_total(&[plan.clone(), plan]), None);
    }
    #[test]
    fn counts_as_priced_only_the_runs_that_were() {
        use crate::benchmark::{cost_total, CostTotal};
        let mut usage = long_run();
        usage["billing"] = json!("subscription");
        let plan = mission_cost_from_events(&[receipt(usage)]);
        let priced = mission_cost_from_events(&[receipt(json!({"usd":0.05,"inputTokens":1,"outputTokens":1}))]);
        let free = mission_cost_from_events(&[receipt(json!({"inputTokens":1200,"outputTokens":300}))]);
        assert_eq!(cost_total(&[priced, free.clone(), plan]), Some(CostTotal {
            line: "$0.05".into(), runs: 1, word: "priced",
        }));
        assert_eq!(cost_total(&[free.clone(), free]), Some(CostTotal {
            line: "2.4k in · 600 out".into(), runs: 2, word: "measured",
        }));
    }

}
