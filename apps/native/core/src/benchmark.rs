//! Reusable benchmarks, record tables, and across-benchmark analysis
//! (`renderer/src/benchmark.ts`, `renderer/src/compareRecord.ts`, `BenchmarksScreen.tsx`).

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::compare::{
    Compare, CompareRoute, CompareSlotId, RerunProgress, RerunStep, has_column_instructions,
};
use crate::ledger::duration_text;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunCost {
    pub usd: Option<f64>,
    pub premium_requests: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub plan: bool,
}

pub fn cost_line(cost: Option<&RunCost>) -> Option<String> {
    let c = cost?;
    if let Some(usd) = c.usd {
        if usd == 0.0 {
            return Some("$0".into());
        }
        return Some(if usd < 0.01 {
            format!("${usd:.4}")
        } else {
            format!("${usd:.2}")
        });
    }
    if let Some(pr) = c.premium_requests {
        let rounded = (pr * 10.0).round() / 10.0;
        let num = if (rounded - rounded.round()).abs() < 1e-6 {
            format!("{}", rounded.round() as u64)
        } else {
            format!("{rounded:.1}")
        };
        return Some(format!(
            "{num} premium {}",
            if (rounded - 1.0).abs() < 1e-6 {
                "request"
            } else {
                "requests"
            }
        ));
    }
    if c.input_tokens.is_some() || c.output_tokens.is_some() {
        let inp = c.input_tokens.unwrap_or(0);
        let out = c.output_tokens.unwrap_or(0);
        return Some(format!("{} in · {} out", compact_tok(inp), compact_tok(out)));
    }
    if c.plan {
        return Some("in your plan".into());
    }
    None
}

fn compact_tok(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{}k", (n as f64 / 1_000.0).round() as u64)
    } else {
        n.to_string()
    }
}

pub fn cost_cell(cost: Option<&RunCost>, model: &str) -> String {
    if model.to_ascii_lowercase().ends_with("-free")
        && cost.and_then(|c| c.usd).unwrap_or(0.0) == 0.0
    {
        return "free".into();
    }
    cost_line(cost).unwrap_or_else(|| "—".into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchmarkCheck {
    Passed,
    Failed,
    TimedOut,
    CouldNotRun,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BenchmarkAnswer {
    pub key: String,
    pub runtime: String,
    pub model: String,
    pub effort: Option<String>,
    pub name: String,
    pub answered: bool,
    pub met: Option<usize>,
    pub of: Option<usize>,
    pub span_ms: Option<u64>,
    pub cost: Option<RunCost>,
    pub check: Option<BenchmarkCheck>,
    pub slot: Option<CompareSlotId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BenchmarkRunInput {
    pub compare_id: String,
    pub answers: Vec<BenchmarkAnswer>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BenchmarkRow {
    pub key: String,
    pub runtime: String,
    pub model: String,
    pub effort: Option<String>,
    pub name: String,
    pub runs: usize,
    pub scored: usize,
    pub score: Option<u64>,
    pub every_point: usize,
    pub span_ms: Option<u64>,
    pub cost: Option<RunCost>,
    pub checked: usize,
    pub check_passed: usize,
    pub slot: Option<CompareSlotId>,
}

pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        Some(sorted[mid])
    } else {
        Some((sorted[mid - 1] + sorted[mid]) / 2.0)
    }
}

pub fn average_cost(costs: &[Option<&RunCost>]) -> Option<RunCost> {
    let reported: Vec<&RunCost> = costs.iter().flatten().copied().collect();
    if reported.is_empty() {
        return None;
    }
    let mean_f = |pick: fn(&RunCost) -> Option<f64>| -> Option<f64> {
        let vals: Vec<f64> = reported.iter().filter_map(|c| pick(c)).collect();
        if vals.is_empty() {
            None
        } else {
            Some(vals.iter().sum::<f64>() / vals.len() as f64)
        }
    };
    let usd = mean_f(|c| c.usd);
    let premium_requests =
        mean_f(|c| c.premium_requests).map(|v| (v * 10.0).round() / 10.0);
    let input_tokens =
        mean_f(|c| c.input_tokens.map(|t| t as f64)).map(|v| v.round() as u64);
    let output_tokens =
        mean_f(|c| c.output_tokens.map(|t| t as f64)).map(|v| v.round() as u64);
    let plan = reported.iter().all(|c| c.plan);
    Some(RunCost {
        usd,
        premium_requests,
        input_tokens,
        output_tokens,
        plan,
    })
}

pub fn benchmark_rows(runs: &[BenchmarkRunInput]) -> Vec<BenchmarkRow> {
    let mut by_key: BTreeMap<String, Vec<&BenchmarkAnswer>> = BTreeMap::new();
    for run in runs {
        for answer in &run.answers {
            by_key.entry(answer.key.clone()).or_default().push(answer);
        }
    }
    let mut rows = Vec::with_capacity(by_key.len());
    for (key, answers) in by_key {
        let first = answers[0];
        let finished: Vec<&BenchmarkAnswer> =
            answers.iter().copied().filter(|a| a.answered).collect();
        let scored: Vec<&BenchmarkAnswer> = finished
            .iter()
            .copied()
            .filter(|a| a.met.is_some() && a.of.is_some_and(|o| o > 0))
            .collect();
        let score = if scored.is_empty() {
            None
        } else {
            let sum: f64 = scored
                .iter()
                .map(|a| a.met.unwrap() as f64 / a.of.unwrap() as f64)
                .sum();
            Some(((sum / scored.len() as f64) * 100.0).round() as u64)
        };
        let spans: Vec<f64> = finished
            .iter()
            .filter_map(|a| a.span_ms.map(|ms| ms as f64))
            .collect();
        let span_ms = median(&spans).map(|v| v.round() as u64);
        let cost_refs: Vec<Option<&RunCost>> =
            finished.iter().map(|a| a.cost.as_ref()).collect();
        let cost = average_cost(&cost_refs);
        let checked: Vec<&BenchmarkAnswer> =
            finished.iter().copied().filter(|a| a.check.is_some()).collect();
        let check_passed = checked
            .iter()
            .filter(|a| a.check == Some(BenchmarkCheck::Passed))
            .count();
        let every_point = scored.iter().filter(|a| a.met == a.of).count();
        rows.push(BenchmarkRow {
            key,
            runtime: first.runtime.clone(),
            model: first.model.clone(),
            effort: first.effort.clone(),
            name: first.name.clone(),
            runs: finished.len(),
            scored: scored.len(),
            score,
            every_point,
            span_ms,
            cost,
            checked: checked.len(),
            check_passed,
            slot: first.slot,
        });
    }
    rows.sort_by(|left, right| {
        let ls = left.score.map_or(-1_i64, |s| s as i64);
        let rs = right.score.map_or(-1_i64, |s| s as i64);
        rs.cmp(&ls).then_with(|| {
            left.span_ms
                .unwrap_or(u64::MAX)
                .cmp(&right.span_ms.unwrap_or(u64::MAX))
        })
    });
    rows
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchmarkLeader {
    pub names: Vec<String>,
    pub score: u64,
}

pub fn benchmark_leader(rows: &[BenchmarkRow]) -> Option<BenchmarkLeader> {
    let scored: Vec<&BenchmarkRow> = rows.iter().filter(|r| r.score.is_some()).collect();
    if scored.is_empty() {
        return None;
    }
    let best = scored.iter().filter_map(|r| r.score).max()?;
    let names = scored
        .iter()
        .filter(|r| r.score == Some(best))
        .map(|r| r.name.clone())
        .collect();
    Some(BenchmarkLeader { names, score: best })
}

pub fn leader_parts(names: &[String]) -> (String, bool) {
    if names.len() == 1 {
        (names[0].clone(), false)
    } else if names.len() == 2 {
        (format!("{} and {}", names[0], names[1]), true)
    } else {
        (format!("{} models", names.len()), true)
    }
}

pub fn leader_words(names: &[String]) -> String {
    let (who, tie) = leader_parts(names);
    if tie { format!("{who} tie") } else { who }
}

pub fn benchmark_models_line(models: &[String]) -> String {
    if models.len() <= 3 {
        models.join(" vs ")
    } else {
        format!("{} models", models.len())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AcrossRow {
    pub key: String,
    pub runtime: String,
    pub model: String,
    pub effort: Option<String>,
    pub name: String,
    pub benchmarks: usize,
    pub runs: usize,
    pub scored: usize,
    pub score: Option<u64>,
    pub every_point: usize,
    pub span_ms: Option<u64>,
    pub cost: Option<RunCost>,
    pub checked: usize,
    pub check_passed: usize,
}

pub fn across_benchmarks(benchmarks: &[&[BenchmarkRow]]) -> Vec<AcrossRow> {
    let mut by_key: BTreeMap<String, Vec<&BenchmarkRow>> = BTreeMap::new();
    for rows in benchmarks {
        for row in *rows {
            if row.slot.is_none() {
                by_key.entry(row.key.clone()).or_default().push(row);
            }
        }
    }
    let mut out = Vec::new();
    for (key, held) in by_key {
        if held.len() < 2 {
            continue;
        }
        let first = held[0];
        let scores: Vec<u64> = held.iter().filter_map(|r| r.score).collect();
        let score = if scores.is_empty() {
            None
        } else {
            let sum: u64 = scores.iter().sum();
            Some((sum as f64 / scores.len() as f64).round() as u64)
        };
        let spans: Vec<f64> = held
            .iter()
            .filter_map(|r| r.span_ms.map(|ms| ms as f64))
            .collect();
        let span_ms = median(&spans).map(|v| v.round() as u64);
        let cost_refs: Vec<Option<&RunCost>> = held.iter().map(|r| r.cost.as_ref()).collect();
        let cost = average_cost(&cost_refs);
        out.push(AcrossRow {
            key,
            runtime: first.runtime.clone(),
            model: first.model.clone(),
            effort: first.effort.clone(),
            name: first.name.clone(),
            benchmarks: held.len(),
            runs: held.iter().map(|r| r.runs).sum(),
            scored: held.iter().map(|r| r.scored).sum(),
            score,
            every_point: held.iter().map(|r| r.every_point).sum(),
            span_ms,
            cost,
            checked: held.iter().map(|r| r.checked).sum(),
            check_passed: held.iter().map(|r| r.check_passed).sum(),
        });
    }
    if out.len() < 2 {
        return Vec::new();
    }
    out.sort_by(|left, right| {
        let ls = left.score.map_or(-1_i64, |s| s as i64);
        let rs = right.score.map_or(-1_i64, |s| s as i64);
        rs.cmp(&ls).then_with(|| {
            left.span_ms
                .unwrap_or(u64::MAX)
                .cmp(&right.span_ms.unwrap_or(u64::MAX))
        })
    });
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartAxis {
    Cost,
    Time,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeBecause {
    Unpriced,
    SamePrice,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChartPoint {
    pub key: String,
    pub name: String,
    pub score: u64,
    pub x: f64,
    pub free: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BenchmarkChart {
    pub axis: ChartAxis,
    pub because: Option<TimeBecause>,
    pub points: Vec<ChartPoint>,
    pub left: Vec<String>,
}

pub fn benchmark_chart(rows: &[BenchmarkRow]) -> Option<BenchmarkChart> {
    let scored: Vec<&BenchmarkRow> = rows.iter().filter(|r| r.score.is_some()).collect();
    if scored.len() < 2 {
        return None;
    }
    let priced: Vec<&BenchmarkRow> = scored
        .iter()
        .copied()
        .filter(|r| r.cost.as_ref().and_then(|c| c.usd).is_some())
        .collect();
    let prices: HashSet<String> = priced
        .iter()
        .filter_map(|r| r.cost.as_ref().and_then(|c| c.usd))
        .map(|u| format!("{u:.4}"))
        .collect();
    if priced.len() >= 2 && prices.len() >= 2 {
        return Some(BenchmarkChart {
            axis: ChartAxis::Cost,
            because: None,
            points: priced
                .iter()
                .map(|r| {
                    let usd = r.cost.as_ref().and_then(|c| c.usd).unwrap_or(0.0);
                    ChartPoint {
                        key: r.key.clone(),
                        name: r.name.clone(),
                        score: r.score.unwrap(),
                        x: usd,
                        free: usd == 0.0,
                    }
                })
                .collect(),
            left: scored
                .iter()
                .filter(|r| r.cost.as_ref().and_then(|c| c.usd).is_none())
                .map(|r| r.name.clone())
                .collect(),
        });
    }
    let timed: Vec<&BenchmarkRow> = scored
        .iter()
        .copied()
        .filter(|r| r.span_ms.is_some())
        .collect();
    if timed.len() < 2 {
        return None;
    }
    Some(BenchmarkChart {
        axis: ChartAxis::Time,
        because: Some(if priced.len() >= 2 {
            TimeBecause::SamePrice
        } else {
            TimeBecause::Unpriced
        }),
        points: timed
            .iter()
            .map(|r| ChartPoint {
                key: r.key.clone(),
                name: r.name.clone(),
                score: r.score.unwrap(),
                x: r.span_ms.unwrap() as f64,
                free: false,
            })
            .collect(),
        left: scored
            .iter()
            .filter(|r| r.span_ms.is_none())
            .map(|r| r.name.clone())
            .collect(),
    })
}

pub fn step_words(progress: &RerunProgress, three_judges: bool) -> String {
    let step = match progress.step {
        RerunStep::Checking => "waiting for checks before judging",
        RerunStep::Judging => {
            if three_judges {
                "the judges are reading the answers"
            } else {
                "the judge is reading the answers"
            }
        }
        RerunStep::Answering => "the models are answering",
    };
    if progress.of == 1 {
        format!("Running it once more: {step}")
    } else {
        format!(
            "Running it again, {} of {}: {step}",
            progress.run, progress.of
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BenchmarkEntry {
    pub compare_id: String,
    pub question: String,
    pub teammate_id: Option<String>,
    pub models: Vec<String>,
    pub runs: usize,
    pub judge_name: Option<String>,
    pub last_at: String,
    pub rows: Vec<BenchmarkRow>,
    pub progress: Option<RerunProgress>,
    pub changes: bool,
}

pub fn ordered_benchmarks(entries: &[BenchmarkEntry]) -> Vec<BenchmarkEntry> {
    let mut out = entries.to_vec();
    out.sort_by(|first, second| {
        let fr = usize::from(first.progress.is_none());
        let sr = usize::from(second.progress.is_none());
        fr.cmp(&sr).then_with(|| second.last_at.cmp(&first.last_at))
    });
    out
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColumnTook {
    pub ms: Option<u64>,
    pub cost: Option<RunCost>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompareRecordRow {
    pub key: String,
    pub name: String,
    pub kept: usize,
    pub compared: usize,
    pub time: Option<String>,
    pub cost: Option<String>,
}

fn middle_by<T>(values: &[T], weight: impl Fn(&T) -> f64) -> Option<&T> {
    if values.is_empty() {
        return None;
    }
    let mut indices: Vec<usize> = (0..values.len()).collect();
    indices.sort_by(|&a, &b| {
        weight(&values[a])
            .partial_cmp(&weight(&values[b]))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Some(&values[indices[(indices.len() - 1) / 2]])
}

fn cost_weight(cost: &RunCost) -> f64 {
    cost.usd
        .or(cost.premium_requests)
        .unwrap_or((cost.input_tokens.unwrap_or(0) + cost.output_tokens.unwrap_or(0)) as f64)
}

pub fn compare_record_rows(
    compares: &[Compare],
    took: impl Fn(&Compare, CompareSlotId) -> Option<ColumnTook>,
    name_of: impl Fn(&CompareRoute) -> String,
) -> Vec<CompareRecordRow> {
    struct Acc {
        name: String,
        kept: usize,
        compared: usize,
        times: Vec<u64>,
        costs: Vec<RunCost>,
    }
    let mut rows: HashMap<String, Acc> = HashMap::new();
    for cmp in compares {
        let Some(kept) = &cmp.kept else { continue };
        if has_column_instructions(cmp) {
            continue;
        }
        for col in &cmp.slots {
            if col.mission_ids.is_empty() {
                continue;
            }
            let key = format!("{}:{}", col.route.runtime, col.route.model);
            let entry = rows.entry(key).or_insert_with(|| Acc {
                name: name_of(&col.route),
                kept: 0,
                compared: 0,
                times: Vec::new(),
                costs: Vec::new(),
            });
            entry.compared += 1;
            if col.slot == kept.slot {
                entry.kept += 1;
            }
            if let Some(spent) = took(cmp, col.slot) {
                if let Some(ms) = spent.ms
                    && ms > 0
                {
                    entry.times.push(ms);
                }
                if let Some(c) = spent.cost {
                    entry.costs.push(c);
                }
            }
        }
    }
    let mut out: Vec<CompareRecordRow> = rows
        .into_iter()
        .map(|(key, row)| CompareRecordRow {
            key,
            name: row.name,
            kept: row.kept,
            compared: row.compared,
            time: middle_by(&row.times, |&ms| ms as f64).map(|&ms| duration_text(ms)),
            cost: cost_line(middle_by(&row.costs, cost_weight)),
        })
        .collect();
    out.sort_by(|left, right| {
        let lr = left.kept as f64 / left.compared as f64;
        let rr = right.kept as f64 / right.compared as f64;
        rr.partial_cmp(&lr)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.compared.cmp(&left.compared))
            .then_with(|| left.name.cmp(&right.name))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::{CompareKept, CompareSlot};

    const SONNET: &str = "claude:claude-sonnet-5-5";
    const BUNNY: &str = "opencode:opencode/space-bunny-free";
    const SOL: &str = "codex:gpt-6-sol";

    fn ans(key: &str, met: Option<usize>, of: Option<usize>, span_ms: Option<u64>, cost: Option<RunCost>) -> BenchmarkAnswer {
        let (rt, model) = key.split_once(':').unwrap();
        BenchmarkAnswer {
            key: key.into(),
            runtime: rt.into(),
            model: model.into(),
            effort: None,
            name: model.into(),
            answered: true,
            met,
            of,
            span_ms,
            cost,
            check: None,
            slot: None,
        }
    }

    #[test]
    fn benchmark_reads_every_run_and_averages_points() {
        let mut bunny_unanswered = ans(BUNNY, None, None, None, None);
        bunny_unanswered.answered = false;
        let rows = benchmark_rows(&[
            BenchmarkRunInput {
                compare_id: "cmp_1".into(),
                answers: vec![
                    ans(SONNET, Some(2), Some(2), Some(60_000), Some(RunCost { usd: Some(0.02), ..Default::default() })),
                    ans(BUNNY, Some(1), Some(2), Some(90_000), Some(RunCost { usd: Some(0.0), ..Default::default() })),
                ],
            },
            BenchmarkRunInput {
                compare_id: "cmp_2".into(),
                answers: vec![
                    ans(SONNET, Some(2), Some(2), Some(80_000), Some(RunCost { usd: Some(0.04), ..Default::default() })),
                    ans(BUNNY, Some(2), Some(2), Some(100_000), Some(RunCost { usd: Some(0.0), ..Default::default() })),
                ],
            },
            BenchmarkRunInput {
                compare_id: "cmp_3".into(),
                answers: vec![
                    ans(SONNET, None, None, Some(70_000), Some(RunCost { usd: Some(0.03), ..Default::default() })),
                    bunny_unanswered,
                ],
            },
        ]);
        assert_eq!(rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), vec![SONNET, BUNNY]);
        assert_eq!(rows[0].score, Some(100));
        assert_eq!(rows[0].scored, 2);
        assert_eq!(rows[0].every_point, 2);
        assert_eq!(rows[0].runs, 3);
        assert_eq!(rows[0].span_ms, Some(70_000));
        assert!((rows[0].cost.as_ref().unwrap().usd.unwrap() - 0.03).abs() < 1e-9);
        assert_eq!(rows[1].score, Some(75));
        assert_eq!(rows[1].scored, 2);
        assert_eq!(rows[1].every_point, 1);
        assert_eq!(rows[1].runs, 2);
        assert_eq!(rows[1].span_ms, Some(95_000));
    }

    #[test]
    fn keeps_each_runtime_in_its_own_unit() {
        let c1 = RunCost { usd: Some(0.01), ..Default::default() };
        let c2 = RunCost { usd: Some(0.03), ..Default::default() };
        assert_eq!(average_cost(&[Some(&c1), Some(&c2), None]).unwrap().usd, Some(0.02));

        let p1 = RunCost { premium_requests: Some(1.0), ..Default::default() };
        let p2 = RunCost { premium_requests: Some(2.0), ..Default::default() };
        assert_eq!(average_cost(&[Some(&p1), Some(&p2)]).unwrap().premium_requests, Some(1.5));

        let t1 = RunCost { plan: true, input_tokens: Some(1000), output_tokens: Some(100), ..Default::default() };
        let t2 = RunCost { plan: true, input_tokens: Some(3000), output_tokens: Some(300), ..Default::default() };
        assert_eq!(
            average_cost(&[Some(&t1), Some(&t2)]).unwrap(),
            RunCost { usd: None, premium_requests: None, input_tokens: Some(2000), output_tokens: Some(200), plan: true }
        );
        assert_eq!(average_cost(&[None]), None);
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn charts_cost_or_time_and_handles_two_free_models() {
        let rows = benchmark_rows(&[BenchmarkRunInput {
            compare_id: "cmp_1".into(),
            answers: vec![
                ans(SONNET, Some(2), Some(2), Some(1_000), Some(RunCost { usd: Some(0.02), ..Default::default() })),
                ans(BUNNY, Some(1), Some(2), Some(2_000), Some(RunCost { usd: Some(0.0), ..Default::default() })),
                ans(SOL, Some(2), Some(2), Some(1_500), Some(RunCost { plan: true, ..Default::default() })),
            ],
        }]);
        let chart = benchmark_chart(&rows).unwrap();
        assert_eq!(chart.axis, ChartAxis::Cost);
        assert!(chart.points.iter().find(|p| p.key == BUNNY).unwrap().free);
        assert_eq!(chart.left, vec!["gpt-6-sol"]);

        let timed = benchmark_chart(&benchmark_rows(&[BenchmarkRunInput {
            compare_id: "cmp_1".into(),
            answers: vec![
                ans(SONNET, Some(2), Some(2), Some(1_000), Some(RunCost { plan: true, ..Default::default() })),
                ans(SOL, Some(1), Some(2), Some(2_000), Some(RunCost { plan: true, ..Default::default() })),
            ],
        }]))
        .unwrap();
        assert_eq!(timed.axis, ChartAxis::Time);
        assert_eq!(timed.because, Some(TimeBecause::Unpriced));

        assert_eq!(
            benchmark_chart(&benchmark_rows(&[BenchmarkRunInput {
                compare_id: "cmp_1".into(),
                answers: vec![ans(SONNET, Some(2), Some(2), None, None), ans(SOL, None, None, None, None)],
            }])),
            None
        );

        let free = benchmark_chart(&benchmark_rows(&[BenchmarkRunInput {
            compare_id: "cmp_1".into(),
            answers: vec![
                ans(BUNNY, Some(2), Some(2), Some(41_000), Some(RunCost { usd: Some(0.0), ..Default::default() })),
                ans("opencode:opencode/muse-spark-1.3-contributor-free", Some(0), Some(2), Some(33_000), Some(RunCost { usd: Some(0.0), ..Default::default() })),
            ],
        }]))
        .unwrap();
        assert_eq!(free.axis, ChartAxis::Time);
        assert_eq!(free.because, Some(TimeBecause::SamePrice));
    }

    #[test]
    fn across_your_benchmarks_averages_and_excludes_setups() {
        let mk_row = |name: &str, score: Option<u64>, runs: usize, scored: usize, every_point: usize, span_ms: Option<u64>, usd: Option<f64>| BenchmarkRow {
            key: format!("opencode:own-0a1b2c3d/{name}"),
            runtime: "opencode".into(),
            model: format!("own-0a1b2c3d/{name}"),
            effort: None,
            name: name.into(),
            runs,
            scored,
            score,
            every_point,
            span_ms,
            cost: usd.map(|u| RunCost { usd: Some(u), ..Default::default() }),
            checked: 0,
            check_passed: 0,
            slot: None,
        };
        let b1 = vec![
            mk_row("Haiku", Some(100), 3, 3, 3, Some(24_000), Some(0.06)),
            mk_row("Opus", Some(100), 3, 3, 2, Some(32_000), Some(0.46)),
        ];
        let b2 = vec![
            mk_row("Haiku", Some(80), 15, 15, 9, Some(42_000), Some(0.1)),
            mk_row("Opus", Some(70), 3, 3, 3, Some(55_000), Some(0.7)),
            mk_row("Sonnet", Some(100), 3, 3, 3, None, None),
        ];
        let across = across_benchmarks(&[&b1, &b2]);
        assert_eq!(across.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), vec!["Haiku", "Opus"]);
        assert_eq!(across[0].benchmarks, 2);
        assert_eq!(across[0].score, Some(90));
        assert_eq!(across[0].runs, 18);
        assert_eq!(across[0].scored, 18);
        assert_eq!(across[0].every_point, 12);
        assert_eq!(across[0].span_ms, Some(33_000));
        assert!((across[0].cost.as_ref().unwrap().usd.unwrap() - 0.08).abs() < 1e-10);
        assert_eq!(across[1].score, Some(85));
        assert_eq!(across[1].every_point, 5);

        // Excludes tune setups
        let mut s_a = mk_row("Setup A", Some(100), 3, 3, 3, None, None);
        s_a.slot = Some(CompareSlotId::A);
        let mut s_b = mk_row("Setup B", Some(100), 3, 3, 3, None, None);
        s_b.slot = Some(CompareSlotId::B);
        assert!(across_benchmarks(&[&[s_a.clone(), s_b.clone()], &[s_a, s_b]]).is_empty());

        // Fewer than two shared models -> empty
        assert!(across_benchmarks(&[&b1]).is_empty());
    }

    #[test]
    fn every_benchmark_is_listed_in_one_place() {
        let r_a = BenchmarkRow {
            key: "opencode:Space Bunny".into(),
            runtime: "opencode".into(),
            model: "Space Bunny".into(),
            effort: None,
            name: "Space Bunny".into(),
            runs: 2,
            scored: 2,
            score: Some(83),
            every_point: 0,
            span_ms: None,
            cost: None,
            checked: 0,
            check_passed: 0,
            slot: None,
        };
        let mut r_b = r_a.clone();
        r_b.name = "Muse Spark".into();
        r_b.score = Some(50);
        assert_eq!(
            benchmark_leader(&[r_a.clone(), r_b.clone()]),
            Some(BenchmarkLeader { names: vec!["Space Bunny".into()], score: 83 })
        );
        let mut r_tie_a = r_a.clone();
        r_tie_a.name = "A".into();
        r_tie_a.score = Some(80);
        let mut r_tie_b = r_a;
        r_tie_b.name = "B".into();
        r_tie_b.score = Some(80);
        assert_eq!(
            benchmark_leader(&[r_tie_a, r_tie_b]),
            Some(BenchmarkLeader { names: vec!["A".into(), "B".into()], score: 80 })
        );
        assert_eq!(leader_words(&["Space Bunny".into()]), "Space Bunny");
        assert_eq!(leader_words(&["A".into(), "B".into()]), "A and B tie");
        assert_eq!(leader_words(&["A".into(), "B".into(), "C".into()]), "3 models tie");
        assert_eq!(benchmark_models_line(&["A".into(), "B".into(), "C".into()]), "A vs B vs C");
        assert_eq!(
            benchmark_models_line(&["A".into(), "B".into(), "C".into(), "D".into()]),
            "4 models"
        );

        let e = |id: &str, last_at: &str, running: bool| BenchmarkEntry {
            compare_id: id.into(),
            question: id.into(),
            teammate_id: None,
            models: vec!["A".into(), "B".into()],
            runs: 4,
            judge_name: Some("Codex".into()),
            last_at: last_at.into(),
            rows: Vec::new(),
            progress: running.then(|| RerunProgress {
                origin_id: id.into(),
                run: 2,
                of: 3,
                step: RerunStep::Answering,
                compare_id: None,
            }),
            changes: false,
        };
        let ordered = ordered_benchmarks(&[
            e("cmp_old", "2026-10-10T10:00:00.000Z", false),
            e("cmp_new", "2026-10-12T10:00:00.000Z", false),
            e("cmp_going", "2026-10-01T10:00:00.000Z", true),
        ]);
        assert_eq!(
            ordered.iter().map(|x| x.compare_id.as_str()).collect::<Vec<_>>(),
            vec!["cmp_going", "cmp_new", "cmp_old"]
        );
    }

    #[test]
    fn compare_record_rows_calculates_middle_time_and_cost() {
        let cmp = Compare {
            compare_id: "cmp_1".into(),
            teammate_id: None,
            prompt: "Ask".into(),
            created_at: "2026-11-13T00:00:00.000Z".into(),
            slots: vec![
                CompareSlot {
                    slot: CompareSlotId::A,
                    route: CompareRoute::new("claude", "sonnet"),
                    mission_ids: vec!["m1".into()],
                    refused: None,
                    retried: Vec::new(),
                    folder: None,
                },
                CompareSlot {
                    slot: CompareSlotId::B,
                    route: CompareRoute::new("codex", "gpt"),
                    mission_ids: vec!["m2".into()],
                    refused: None,
                    retried: Vec::new(),
                    folder: None,
                },
            ],
            changes: false,
            changes_in: None,
            blind: false,
            kept: Some(CompareKept {
                slot: CompareSlotId::A,
                at: "2026-11-13T00:01:00.000Z".into(),
                brought: None,
            }),
            judge: None,
            base: None,
            rerun_of: None,
            reruns: Vec::new(),
            settled: false,
            point_drafts: Vec::new(),
        };
        let rows = compare_record_rows(
            &[cmp],
            |_, slot| {
                Some(if slot == CompareSlotId::A {
                    ColumnTook {
                        ms: Some(41_000),
                        cost: Some(RunCost { usd: Some(0.02), ..Default::default() }),
                    }
                } else {
                    ColumnTook {
                        ms: Some(15_000),
                        cost: Some(RunCost { plan: true, ..Default::default() }),
                    }
                })
            },
            |r| r.model.clone(),
        );
        assert_eq!(rows[0].key, "claude:sonnet");
        assert_eq!(rows[0].kept, 1);
        assert_eq!(rows[0].compared, 1);
        assert_eq!(rows[0].time.as_deref(), Some("41s"));
        assert_eq!(rows[0].cost.as_deref(), Some("$0.02"));
        assert_eq!(rows[1].cost.as_deref(), Some("in your plan"));
    }
}
