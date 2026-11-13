//! Usage across agents and models — port of `shared/usage.ts` + `usageView.ts` pure helpers.

use crate::context_usage::compact_count;
use crate::spend::dollars;
use chrono::{Datelike, Local, NaiveDate, TimeZone};
use std::collections::{BTreeSet, HashMap};

pub const HEATMAP_WEEKS: usize = 12;

#[derive(Clone, Debug, PartialEq)]
pub struct UsageTurn {
    pub at: String,
    pub runtime: String,
    pub model: String,
    pub conversation: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub usd: Option<f64>,
    pub premium_requests: Option<f64>,
    pub plan: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UsageRange {
    Days7,
    #[default]
    Days30,
    All,
}

impl UsageRange {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Days7 => "7d",
            Self::Days30 => "30d",
            Self::All => "all",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        Some(match value {
            "7d" => Self::Days7,
            "30d" => Self::Days30,
            "all" => Self::All,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Days7 => "7 days",
            Self::Days30 => "30 days",
            Self::All => "All time",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UsageTokens {
    pub prompt: u64,
    pub cached: u64,
    pub output: u64,
    pub cache_counted: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UsageDay {
    pub date: String,
    pub turns: u64,
    pub tokens: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UsagePay {
    pub usd: Option<f64>,
    pub premium_requests: Option<f64>,
    pub plan_turns: u64,
    pub free_turns: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UsageModel {
    pub runtime: String,
    pub model: String,
    pub turns: u64,
    pub tokens: UsageTokens,
    pub pay: UsagePay,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UsageSummary {
    pub range: UsageRange,
    pub since: Option<String>,
    pub today: String,
    pub turns: u64,
    pub conversations: u64,
    pub tokens: UsageTokens,
    pub models: Vec<UsageModel>,
    pub pay: UsagePay,
    pub days: Vec<UsageDay>,
    pub heatmap: Vec<UsageDay>,
    pub active_days: u64,
    pub range_days: u64,
    pub streak_current: u64,
    pub streak_longest: u64,
    pub busiest: Option<UsageDay>,
    pub first_at: Option<String>,
}

fn count(value: Option<u64>) -> u64 {
    value.filter(|&n| n > 0).unwrap_or(0)
}

pub fn prompt_tokens(
    runtime: &str,
    input: Option<u64>,
    cache_read: Option<u64>,
    cache_write: Option<u64>,
) -> u64 {
    let input = count(input);
    if runtime == "codex" {
        input
    } else {
        input + count(cache_read) + count(cache_write)
    }
}

pub fn turn_tokens(turn: &UsageTurn) -> UsageTokens {
    let prompt = prompt_tokens(
        &turn.runtime,
        turn.input_tokens,
        turn.cache_read_tokens,
        turn.cache_write_tokens,
    );
    let counted = turn.cache_read_tokens.is_some() || turn.cache_write_tokens.is_some();
    UsageTokens {
        prompt,
        cached: count(turn.cache_read_tokens).min(prompt),
        output: count(turn.output_tokens),
        cache_counted: if counted { prompt } else { 0 },
    }
}

pub fn total_tokens(tokens: &UsageTokens) -> u64 {
    tokens.prompt + tokens.output
}

pub fn cached_percent(tokens: &UsageTokens) -> Option<u64> {
    if tokens.cache_counted == 0 {
        None
    } else {
        Some(
            ((tokens.cached.min(tokens.cache_counted) as f64 / tokens.cache_counted as f64) * 100.0)
                .round() as u64,
        )
    }
}

pub fn local_date(at: chrono::DateTime<Local>) -> String {
    format!("{:04}-{:02}-{:02}", at.year(), at.month(), at.day())
}

pub fn day_before(date: &str, days: i64) -> String {
    let parts: Vec<_> = date.split('-').collect();
    let (y, m, d): (i32, u32, u32) = (
        parts[0].parse().unwrap_or(1970),
        parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1),
        parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1),
    );
    let naive = NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default();
    let shifted = naive - chrono::Duration::days(days);
    format!(
        "{:04}-{:02}-{:02}",
        shifted.year(),
        shifted.month(),
        shifted.day()
    )
}

fn days_between(from: &str, to: &str) -> i64 {
    let parse = |date: &str| {
        let parts: Vec<_> = date.split('-').collect();
        NaiveDate::from_ymd_opt(
            parts[0].parse().unwrap_or(1970),
            parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1),
            parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1),
        )
        .unwrap_or_default()
    };
    (parse(to) - parse(from)).num_days()
}

fn is_free_model(model: &str) -> bool {
    model.to_ascii_lowercase().ends_with("-free")
}

fn pay_of(turns: &[&UsageTurn]) -> UsagePay {
    let mut pay = UsagePay::default();
    for turn in turns {
        if turn.plan {
            pay.plan_turns += 1;
        } else if let Some(usd) = turn.usd {
            pay.usd = Some(pay.usd.unwrap_or(0.0) + usd);
        }
        if let Some(p) = turn.premium_requests {
            pay.premium_requests = Some(pay.premium_requests.unwrap_or(0.0) + p);
        }
        if !turn.plan && is_free_model(&turn.model) {
            pay.free_turns += 1;
        }
    }
    pay
}

fn add_tokens(a: UsageTokens, b: UsageTokens) -> UsageTokens {
    UsageTokens {
        prompt: a.prompt + b.prompt,
        cached: a.cached + b.cached,
        output: a.output + b.output,
        cache_counted: a.cache_counted + b.cache_counted,
    }
}

fn parse_ended(at: &str) -> Option<chrono::DateTime<Local>> {
    chrono::DateTime::parse_from_rfc3339(at)
        .ok()
        .map(|d| d.with_timezone(&Local))
        .or_else(|| {
            at.parse::<chrono::DateTime<chrono::Utc>>()
                .ok()
                .map(|d| d.with_timezone(&Local))
        })
}

pub fn usage_summary(
    turns: &[UsageTurn],
    range: UsageRange,
    now: chrono::DateTime<Local>,
) -> UsageSummary {
    let today = local_date(now);
    let dated: Vec<_> = turns
        .iter()
        .filter_map(|turn| {
            let ended = parse_ended(&turn.at)?;
            Some((turn, local_date(ended)))
        })
        .collect();
    let range_days = match range {
        UsageRange::Days7 => Some(7u64),
        UsageRange::Days30 => Some(30),
        UsageRange::All => None,
    };
    let since = range_days.map(|d| day_before(&today, (d as i64) - 1));
    let in_range: Vec<_> = dated
        .iter()
        .filter(|(_, date)| match &since {
            None => true,
            Some(since) => date.as_str() >= since.as_str() && date.as_str() <= today.as_str(),
        })
        .map(|(turn, date)| (*turn, date.clone()))
        .collect();

    let mut by_day: HashMap<String, (u64, u64)> = HashMap::new();
    for (turn, date) in &dated {
        let entry = by_day.entry(date.clone()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += total_tokens(&turn_tokens(turn));
    }
    let day_of = |date: &str| -> UsageDay {
        let (turns, tokens) = by_day.get(date).copied().unwrap_or((0, 0));
        UsageDay {
            date: date.to_owned(),
            turns,
            tokens,
        }
    };

    let mut first: Option<String> = None;
    for (_, date) in &dated {
        if first.as_ref().is_none_or(|f| date < f) {
            first = Some(date.clone());
        }
    }
    let start = since.clone().unwrap_or_else(|| match &first {
        Some(f) if f.as_str() < today.as_str() => f.clone(),
        _ => today.clone(),
    });
    let span = days_between(&start, &today).max(0) as usize;
    let days: Vec<_> = (0..=span)
        .map(|index| day_of(&day_before(&today, (span - index) as i64)))
        .collect();

    let shown = (HEATMAP_WEEKS - 1) * 7 + now.weekday().num_days_from_sunday() as usize + 1;
    let heatmap: Vec<_> = (0..shown)
        .map(|index| day_of(&day_before(&today, (shown - 1 - index) as i64)))
        .collect();

    // Preserve first-seen order for exact sort ties, like Electron's Map and stable sort.
    let mut by_model: Vec<(String, String, Vec<&UsageTurn>)> = Vec::new();
    for (turn, _) in &in_range {
        if let Some(entry) = by_model
            .iter_mut()
            .find(|(runtime, model, _)| runtime == &turn.runtime && model == &turn.model)
        {
            entry.2.push(turn);
        } else {
            by_model.push((turn.runtime.clone(), turn.model.clone(), vec![turn]));
        }
    }
    let mut models: Vec<_> = by_model
        .into_iter()
        .map(|(runtime, model, model_turns)| {
            let tokens = model_turns
                .iter()
                .fold(UsageTokens::default(), |sum, turn| {
                    add_tokens(sum, turn_tokens(turn))
                });
            let pay = pay_of(&model_turns);
            UsageModel {
                runtime,
                model,
                turns: model_turns.len() as u64,
                tokens,
                pay,
            }
        })
        .collect();
    models.sort_by(|a, b| {
        b.turns
            .cmp(&a.turns)
            .then_with(|| total_tokens(&b.tokens).cmp(&total_tokens(&a.tokens)))
            .then_with(|| a.model.cmp(&b.model))
    });

    let every_day: BTreeSet<_> = dated.iter().map(|(_, d)| d.clone()).collect();
    let mut current = 0u64;
    let mut at = if every_day.contains(&today) {
        today.clone()
    } else {
        day_before(&today, 1)
    };
    while every_day.contains(&at) {
        current += 1;
        at = day_before(&at, 1);
    }
    let mut longest = 0u64;
    let mut run = 0u64;
    let mut previous: Option<String> = None;
    for date in &every_day {
        run = match &previous {
            Some(prev) if days_between(prev, date) == 1 => run + 1,
            _ => 1,
        };
        longest = longest.max(run);
        previous = Some(date.clone());
    }

    let mut busiest: Option<UsageDay> = None;
    for day in &days {
        if day.turns > 0 && busiest.as_ref().is_none_or(|b| day.turns > b.turns) {
            busiest = Some(day.clone());
        }
    }
    let mut first_at: Option<String> = None;
    for (turn, _) in &dated {
        if first_at.as_ref().is_none_or(|f| {
            parse_ended(&turn.at)
                .and_then(|a| parse_ended(f).map(|b| a < b))
                .unwrap_or(false)
        }) {
            first_at = Some(turn.at.clone());
        }
    }

    let conversations = in_range
        .iter()
        .map(|(t, _)| t.conversation.as_str())
        .collect::<BTreeSet<_>>()
        .len() as u64;
    let tokens = in_range
        .iter()
        .fold(UsageTokens::default(), |sum, (turn, _)| {
            add_tokens(sum, turn_tokens(turn))
        });
    let pay = pay_of(
        &in_range
            .iter()
            .map(|(t, _)| *t)
            .collect::<Vec<&UsageTurn>>(),
    );
    let active_days = in_range
        .iter()
        .map(|(_, d)| d.as_str())
        .collect::<BTreeSet<_>>()
        .len() as u64;

    let range_days = range_days.unwrap_or(days.len() as u64);
    UsageSummary {
        range,
        since,
        today,
        turns: in_range.len() as u64,
        conversations,
        tokens,
        models,
        pay,
        days,
        heatmap,
        active_days,
        range_days,
        streak_current: current,
        streak_longest: longest,
        busiest,
        first_at,
    }
}

// ---- usageView words ----

pub fn range_phrase(range: UsageRange) -> String {
    match range {
        UsageRange::All => "all time".into(),
        other => format!("the last {}", other.name()),
    }
}

/// Electron uses en-US locale counts (grouping and up to three fractional digits).
pub fn number(value: f64) -> String {
    let fixed = format!("{:.3}", (value * 1000.0).round() / 1000.0);
    let trimmed = fixed.trim_end_matches('0').trim_end_matches('.');
    let (whole, fraction) = trimmed.split_once('.').unwrap_or((trimmed, ""));
    let mut out = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if !fraction.is_empty() {
        out.push('.');
        out.push_str(fraction);
    }
    out
}

fn plural_number(count: f64, one: &str) -> String {
    format!(
        "{} {}{}",
        number(count),
        one,
        if count == 1.0 { "" } else { "s" }
    )
}

pub fn plural(count: u64, one: &str) -> String {
    let many = format!("{one}s");
    let word = if count == 1 { one } else { many.as_str() };
    format!("{} {word}", number(count as f64))
}

pub fn usage_line_facts(summary: &UsageSummary) -> String {
    let mut parts = vec![plural(summary.turns, "turn")];
    let tokens = total_tokens(&summary.tokens);
    if tokens > 0 {
        parts.push(format!("{} tokens", compact_count(tokens)));
    }
    if let Some(usd) = summary.pay.usd.filter(|u| *u > 0.0) {
        parts.push(format!("{} spent", dollars(usd)));
    } else if summary.turns > 0 && summary.pay.free_turns == summary.turns {
        parts.push("all free".into());
    }
    parts.join(" · ")
}

/// One day's bar on Home's usage line (usageView.ts sparkBars): the busiest day full height, a quiet day flat, any
/// day with turns at least a sixth tall so it reads as a day with turns.
#[derive(Clone, Debug, PartialEq)]
pub struct SparkBar {
    pub date: String,
    pub turns: u64,
    pub today: bool,
    pub height: f64,
}

pub fn spark_bars(days: &[UsageDay], today: &str) -> Vec<SparkBar> {
    let most = days.iter().map(|d| d.turns).max().unwrap_or(0).max(1) as f64;
    days.iter()
        .map(|d| SparkBar { date: d.date.clone(), turns: d.turns, today: d.date == today, height: if d.turns == 0 { 0.0 } else { (d.turns as f64 / most).max(0.16) } })
        .collect()
}

pub fn heat_level(turns: u64, busiest: u64) -> u8 {
    if turns == 0 || busiest == 0 {
        return 0;
    }
    let share = turns as f64 / busiest as f64;
    if share > 0.75 {
        4
    } else if share > 0.5 {
        3
    } else if share > 0.25 {
        2
    } else {
        1
    }
}

const ACCOUNT_RUNTIMES: &[&str] = &["codex", "cursor", "gemini", "antigravity", "muse"];

pub fn paid_by(model: &UsageModel) -> String {
    let mut parts = Vec::new();
    if let Some(usd) = model.pay.usd.filter(|u| *u > 0.0) {
        parts.push(dollars(usd));
    }
    if let Some(p) = model.pay.premium_requests.filter(|p| *p > 0.0) {
        parts.push(plural_number(p, "premium request"));
    }
    if model.pay.plan_turns > 0 {
        parts.push(if model.pay.plan_turns == model.turns {
            "your plan".into()
        } else {
            format!("{} in your plan", model.pay.plan_turns)
        });
    }
    if model.pay.free_turns > 0 {
        parts.push(if model.pay.free_turns == model.turns {
            "free".into()
        } else {
            format!("{} free", model.pay.free_turns)
        });
    }
    if !parts.is_empty() {
        return parts.join(" · ");
    }
    if ACCOUNT_RUNTIMES.contains(&model.runtime.as_str()) {
        "your account".into()
    } else {
        "not reported".into()
    }
}

pub fn cached_text(model: &UsageModel) -> String {
    match cached_percent(&model.tokens) {
        None => "—".into(),
        Some(p) => format!("{p}%"),
    }
}

pub fn share_of(model: &UsageModel, summary: &UsageSummary) -> u64 {
    if summary.turns == 0 || model.turns == 0 {
        0
    } else {
        ((model.turns as f64 / summary.turns as f64) * 100.0)
            .round()
            .max(1.0) as u64
    }
}

pub fn spent_sentence(summary: &UsageSummary) -> String {
    let mut parts = Vec::new();
    if let Some(usd) = summary.pay.usd.filter(|u| *u > 0.0) {
        parts.push(format!("{} on API keys and paid models", dollars(usd)));
    }
    if let Some(p) = summary.pay.premium_requests.filter(|p| *p > 0.0) {
        parts.push(plural_number(p, "Copilot premium request"));
    }
    if parts.is_empty() {
        "Nothing spent beyond your plans and free models.".into()
    } else {
        format!("Spent {}.", parts.join(" and "))
    }
}

pub fn streak_text(days: u64) -> String {
    if days == 0 {
        "none".into()
    } else {
        plural(days, "day")
    }
}

pub fn day_label(date: &str) -> String {
    let parts: Vec<_> = date.split('-').collect();
    let (y, m, d): (i32, u32, u32) = (
        parts[0].parse().unwrap_or(1970),
        parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1),
        parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1),
    );
    let naive = NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default();
    let dt = Local
        .from_local_datetime(
            &naive.and_time(chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap_or_default()),
        )
        .single()
        .unwrap_or_else(Local::now);
    format!("{} {}", dt.format("%b"), dt.day())
}

/// Plan meter reading — Electron UsageMeters parse.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanWindow {
    pub name: String,
    pub percent_left: u32,
    pub resets: Option<String>,
    pub expired: bool,
}

pub fn parse_plan_windows(reading: &str, now: chrono::DateTime<chrono::Utc>) -> Vec<PlanWindow> {
    // Electron's pattern uses a lookahead the `regex` crate cannot express.
    // Same capture groups without `(?= · |$)`: middle-dot bounds keep matches apart.
    let re = regex::Regex::new(r"([^·]+?) window (\d{1,3})% (used|left)(?: · resets ([^·]+))?")
        .expect("static meter regex");
    let mut out = Vec::new();
    for cap in re.captures_iter(reading) {
        let label = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        let percent: u32 = cap
            .get(2)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        let kind = cap.get(3).map(|m| m.as_str()).unwrap_or("used");
        let percent_left = if kind == "left" {
            percent.min(100)
        } else {
            100u32.saturating_sub(percent.min(100))
        };
        let resets = cap.get(4).map(|m| m.as_str().trim().to_owned());
        let expired = resets
            .as_ref()
            .and_then(|r| chrono::DateTime::parse_from_rfc3339(r).ok())
            .map(|t| t.with_timezone(&chrono::Utc) <= now)
            .unwrap_or(false);
        out.push(PlanWindow {
            name: format!("{label} window"),
            percent_left,
            resets,
            expired,
        });
    }
    out
}

pub const USAGE_TITLE: &str = "Usage";
pub const USAGE_SUBTITLE: &str = "every agent and model, on this machine";
pub const USAGE_ADDING: &str = "Adding it up…";
pub const USAGE_READ_FAIL: &str =
    "Usage could not be read. Nothing was changed; close this and open it again.";
pub const USAGE_ACTIVITY: &str = "Activity";
pub const USAGE_LIMITS: &str = "Limits";
pub const USAGE_LIMITS_NOTE: &str = "what each account said at its last reading";
pub const USAGE_LIMITS_EMPTY: &str =
    "No agent has reported a limit yet. Claude Code and Codex report theirs with each turn.";
pub const USAGE_MODELS: &str = "Models";
pub const USAGE_MODELS_NOTE: &str = "most turns first";
pub const USAGE_COUNTED: &str = "Counted from Locust's record on this machine.";
pub const USAGE_FOOT_TITLE: &str = "Locust's own turns, and the terminal sessions it opened or brought in. What you do in an agent's own app is not here.";
pub const USAGE_PERCENT_LEFT: &str = "% left";
pub const USAGE_RESET_SINCE: &str = "reset since";

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(at: &str, runtime: &str, model: &str, conversation: &str) -> UsageTurn {
        UsageTurn {
            at: at.into(),
            runtime: runtime.into(),
            model: model.into(),
            conversation: conversation.into(),
            input_tokens: Some(100),
            output_tokens: Some(20),
            cache_read_tokens: None,
            cache_write_tokens: None,
            usd: None,
            premium_requests: None,
            plan: true,
        }
    }

    #[test]
    fn synthetic_receipts_match_actual_electron_helpers() {
        use serde_json::json;
        use std::{
            io::Write,
            process::{Command, Stdio},
        };
        let now = Local.with_ymd_and_hms(2026, 11, 10, 12, 0, 0).unwrap();
        let mut turns = vec![
            turn("2026-11-10T12:00:00Z", "claude", "opus", "a"),
            turn("2026-11-09T12:00:00Z", "copilot", "gpt", "b"),
            turn("2026-10-01T12:00:00Z", "codex", "gpt", "a"),
            turn("2026-11-08T12:00:00Z", "opencode", "model-free", "c"),
        ];
        turns[0].cache_read_tokens = Some(0);
        turns[0].usd = Some(0.125);
        turns[0].plan = false;
        turns[1].premium_requests = Some(1.25);
        turns[1].plan = false;
        turns[3].plan = false;
        let encoded: Vec<_> = turns
            .iter()
            .map(|t| {
                json!({"at":t.at,"runtime":t.runtime,"model":t.model,"conversation":t.conversation,
            "inputTokens":t.input_tokens,"outputTokens":t.output_tokens,
            // Omitted cache fields stay absent: null is a reported value in JS.
            "plan":t.plan,"usd":t.usd,"premiumRequests":t.premium_requests})
            })
            .collect();
        let encoded: Vec<_> = encoded
            .into_iter()
            .enumerate()
            .map(|(i, mut t)| {
                t.as_object_mut().unwrap().retain(|_, v| !v.is_null());
                if i == 0 {
                    t["cacheReadTokens"] = json!(0);
                }
                t
            })
            .collect();
        for range in [UsageRange::Days7, UsageRange::Days30, UsageRange::All] {
            let mut child = Command::new("node")
                .arg("--disable-warning=ExperimentalWarning")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../scripts/usage-oracle.mjs"
                ))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("Node is required for Electron parity");
            child.stdin.take().unwrap().write_all(json!({"turns":encoded,"range":range.as_str(),"now":now.to_rfc3339(),"dollars":[0.125,2.675,1.005,0.004]}).to_string().as_bytes()).unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let expected: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            let s = usage_summary(&turns, range, now);
            let days = |days: &[UsageDay]| {
                days.iter()
                    .map(|d| json!({"date":d.date,"turns":d.turns,"tokens":d.tokens}))
                    .collect::<Vec<_>>()
            };
            let mut pay = json!({"usd":s.pay.usd,"premiumRequests":s.pay.premium_requests,"planTurns":s.pay.plan_turns,"freeTurns":s.pay.free_turns});
            pay.as_object_mut().unwrap().retain(|_, v| !v.is_null());
            let actual = json!({"turns":s.turns,"conversations":s.conversations,
                "tokens":{"prompt":s.tokens.prompt,"cached":s.tokens.cached,"output":s.tokens.output,"cacheCounted":s.tokens.cache_counted},
                "pay":pay,"activeDays":s.active_days,"rangeDays":s.range_days,"streak":{"current":s.streak_current,"longest":s.streak_longest},
                "days":days(&s.days),"heatmap":days(&s.heatmap),
                "models":s.models.iter().map(|m| json!({"model":m.model,"turns":m.turns,"paidBy":paid_by(m),"cached":cached_text(m),"share":share_of(m,&s)})).collect::<Vec<_>>(),
                "facts":usage_line_facts(&s),"spent":spent_sentence(&s),"dollars":[dollars(0.125),dollars(2.675),dollars(1.005),dollars(0.004)]});
            assert_eq!(actual, expected, "range {}", range.as_str());
        }
        assert_eq!(plural(1234, "turn"), "1,234 turns");
    }

    #[test]
    fn prompt_tokens_codex_vs_claude() {
        assert_eq!(prompt_tokens("codex", Some(10), Some(5), Some(2)), 10);
        assert_eq!(prompt_tokens("claude", Some(10), Some(5), Some(2)), 17);
    }

    #[test]
    fn summary_counts_range_and_models() {
        let now = Local.with_ymd_and_hms(2026, 11, 10, 12, 0, 0).unwrap();
        let turns = vec![
            turn("2026-11-10T10:00:00.000Z", "codex", "gpt", "a"),
            turn("2026-11-09T10:00:00.000Z", "claude", "opus", "b"),
            turn("2026-10-01T10:00:00.000Z", "codex", "gpt", "a"),
        ];
        let week = usage_summary(&turns, UsageRange::Days7, now);
        assert_eq!(week.turns, 2);
        assert_eq!(week.conversations, 2);
        assert_eq!(week.models.len(), 2);
        assert_eq!(range_phrase(UsageRange::Days30), "the last 30 days");
        assert_eq!(
            spent_sentence(&week),
            "Nothing spent beyond your plans and free models."
        );
        assert_eq!(paid_by(&week.models[0]), "your plan");
    }

    // usage-is-drawn.test.tsx: a bar a day, the busiest full height, a quiet day flat, today marked.
    #[test]
    fn a_bar_a_day_the_busiest_full_height_a_quiet_day_flat_today_marked() {
        let day = |date: &str, turns: u64| UsageDay { date: date.into(), turns, tokens: 0 };
        let bars = spark_bars(&[day("2026-11-11", 10), day("2026-11-12", 0), day("2026-11-13", 1)], "2026-11-13");
        assert_eq!(bars.iter().map(|b| b.height).collect::<Vec<_>>(), vec![1.0, 0.0, 0.16]);
        assert_eq!(bars.iter().map(|b| b.today).collect::<Vec<_>>(), vec![false, false, true]);
        assert!(spark_bars(&[day("2026-11-13", 0)], "2026-11-13")[0].height == 0.0);
    }

    #[test]
    fn heat_and_compact_words() {
        assert_eq!(heat_level(0, 10), 0);
        assert_eq!(heat_level(9, 10), 4);
        assert_eq!(streak_text(0), "none");
        assert_eq!(streak_text(1), "1 day");
        assert_eq!(streak_text(2), "2 days");
    }

    #[test]
    fn plan_windows_say_percent_left() {
        let now = chrono::Utc::now();
        let windows = parse_plan_windows("5-hour window 40% used · week window 10% left", now);
        assert_eq!(windows[0].percent_left, 60);
        assert_eq!(windows[0].name, "5-hour window");
        assert_eq!(windows[1].percent_left, 10);
        assert_eq!(USAGE_PERCENT_LEFT, "% left");
    }
}
