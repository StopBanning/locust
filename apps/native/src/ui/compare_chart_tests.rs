//! Electron's `judges-see-the-check.test.tsx`, the two chart cases: `ScoreChart` drawn with Electron's points, its
//! leaders (`lc-benchmark__leader`) counted as drawn, here `benchmark-leader-*`.
use super::*;
use locust_core::benchmark::{BenchmarkChart, ChartAxis, ChartPoint, TimeBecause};

fn point(key: &str, name: &str, score: u64, x: f64, free: bool) -> ChartPoint {
    ChartPoint { key: key.into(), name: name.into(), score, x, free }
}
fn timed(points: Vec<ChartPoint>) -> BenchmarkChart {
    BenchmarkChart { axis: ChartAxis::Time, because: Some(TimeBecause::Unpriced), points, left: Vec::new() }
}
/// The chart drawn headless: every element, the leaders by their keys.
fn drawn(chart: BenchmarkChart) -> (Vec<rune::ElementInfo>, Vec<String>) {
    let mut rt = rune::Runtime::new(chart, |c, _| score_chart(c, &Look::new(true)), (900.0, 600.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let all: Vec<rune::ElementInfo> = rt.elements();
    let leaders = all.iter().filter_map(|e| e.key.clone()).filter(|k| k.starts_with("benchmark-leader-")).collect();
    (all, leaders)
}
fn rect(all: &[rune::ElementInfo], key: &str) -> [f32; 4] {
    all.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("{key} drawn")).rect
}

// case: joins a label moved off its point's line to its point
#[test]
fn joins_a_label_moved_off_its_points_line_to_its_point() {
    let (all, leaders) = drawn(timed(vec![
        point("haiku", "Claude Haiku 5.5", 100, 16_000.0, false),
        point("sonnet", "Claude Sonnet 5.5", 100, 17_000.0, false),
        point("opus", "Claude Opus 5.5", 100, 21_000.0, false),
    ]));
    // Three at 100: the first sits on its point's line, the two moved below it each keep a line to theirs.
    assert_eq!(leaders, vec!["benchmark-leader-sonnet".to_owned(), "benchmark-leader-opus".to_owned()]);
    let (haiku, sonnet, opus) = (rect(&all, "benchmark-label-haiku"), rect(&all, "benchmark-label-sonnet"), rect(&all, "benchmark-label-opus"));
    assert!(haiku[1] < sonnet[1] && sonnet[1] < opus[1], "a line apart, top to bottom: {haiku:?} {sonnet:?} {opus:?}");
    // The points themselves stay on 100's line.
    let tops: Vec<f32> = ["haiku", "sonnet", "opus"].iter().map(|k| rect(&all, &format!("benchmark-point-{k}"))[1]).collect();
    assert!(tops.windows(2).all(|w| (w[0] - w[1]).abs() < 0.5), "{tops:?}");
}

// case: moves a label that would run into another from the other side, though their points are far apart (0.811)
#[test]
fn moves_a_label_that_would_run_into_another_from_the_other_side_though_their_points_are_far_apart() {
    // The packaged 0.810 benchmark: one at 30s reads to the right, one at 1m 44s to the left, and both at 100 met.
    let chart = |second: u64| {
        timed(vec![
            point("deepseek", "Deepseek V4.1 Flash Free", 100, 30_000.0, true),
            point("nemotron", "Nemotron 3 Ultra Free", second, 104_000.0, true),
        ])
    };
    let (all, leaders) = drawn(chart(100));
    assert_eq!(leaders, vec!["benchmark-leader-nemotron".to_owned()]);
    let (deepseek, nemotron) = (rect(&all, "benchmark-label-deepseek"), rect(&all, "benchmark-label-nemotron"));
    assert!(deepseek[0] < rect(&all, "benchmark-point-nemotron")[0], "the first reads right, from the left");
    assert!(nemotron[0] + nemotron[2] <= rect(&all, "benchmark-point-nemotron")[0] + 1.0, "the second reads left, ending at its point");
    assert!(nemotron[1] > deepseek[1], "moved a line down: {deepseek:?} {nemotron:?}");
    // A line apart already: nothing moves.
    assert!(drawn(chart(50)).1.is_empty());
}

/// The chart says what Electron's does: its points aloud, the corner, the ticks and the caption.
#[test]
fn the_chart_says_its_points_its_ticks_and_its_caption_as_electrons_does() {
    let (all, _) = drawn(timed(vec![
        point("deepseek", "Deepseek V4.1 Flash Free", 100, 30_000.0, true),
        point("nemotron", "Nemotron 3 Ultra Free", 50, 104_000.0, true),
    ]));
    let plot = all.iter().find(|e| e.key.as_deref() == Some("benchmark-chart-plot")).unwrap();
    assert_eq!(plot.label.as_deref(), Some("Score against time a task: Deepseek V4.1 Flash Free 100 at free; Nemotron 3 Ultra Free 50 at free"));
    let shown: Vec<String> = all.iter().filter_map(|e| e.text.clone()).collect();
    for words in ["0", "50", "100", "0s", "1m 44s", "time a task", "Deepseek V4.1 Flash Free · 100 · free", "Nemotron 3 Ultra Free · 50 · free",
        "Up is better, left is faster: too few of these runs were priced in dollars to chart cost."] {
        assert!(shown.iter().any(|s| s == words), "{words} in {shown:?}");
    }
    // A free model at the left edge and a strong one at the top: the corner's name gives way.
    assert!(!shown.iter().any(|s| s == "Strong and fast"));
}

/// The chart where the person sees it: a comparison run twice, both models scoring 100 by the judge, one at 30s and one
/// at 1m 44s, unpriced -- drawn through `compare_view::view`, its labels met, so one is moved and joined to its point.
#[test]
fn a_benchmark_run_again_draws_its_score_chart_with_the_leader() {
    use locust_core::compare::{Compare, CompareJudge, CompareRoute, CompareSlot, CompareSlotId};
    use locust_core::ledger::{Item, Snapshot};
    use locust_core::time_marker::TurnTiming;
    fn slot(id: CompareSlotId, model: &str, mission: String) -> CompareSlot {
        CompareSlot { slot: id, route: CompareRoute::new("codex", model), mission_ids: vec![mission], refused: None, retried: vec![], folder: None }
    }
    fn comparison(id: &str, reruns: Vec<String>, rerun_of: Option<String>) -> Compare {
        Compare {
            compare_id: id.into(),
            teammate_id: None,
            prompt: "Which sort is stable?".into(),
            created_at: "2026-11-20T12:00:00.000Z".into(),
            slots: vec![slot(CompareSlotId::A, "deepseek-v4.1-flash-preview-long", format!("{id}_a")), slot(CompareSlotId::B, "nemotron-3-ultra-preview-long", format!("{id}_b"))],
            changes: false,
            changes_in: None,
            blind: false,
            kept: None,
            judge: Some(CompareJudge { route: CompareRoute::new("codex", "judge-model"), mission_ids: vec![format!("{id}_judge")],
                criteria: None, panel: vec![], panel_mission_ids: vec![] }),
            base: None,
            rerun_of,
            reruns,
            settled: false,
            point_drafts: vec![],
        }
    }
    fn said(words: &str, seconds: i64) -> Snapshot {
        let at = |s: i64| (chrono::DateTime::parse_from_rfc3339("2026-11-20T12:00:00.000Z").unwrap() + chrono::Duration::seconds(s)).to_rfc3339();
        Snapshot {
            items: vec![Item::Assistant { item_id: "i1".into(), text: words.into(), r#final: true }],
            timings: vec![TurnTiming { before_item: 0, first_at: Some(at(0)), last_at: Some(at(seconds)) }],
            ..Default::default()
        }
    }
    let mut s = State::default();
    s.demo = true;
    s.compare.compares = vec![comparison("cmp_origin", vec!["cmp_rerun".into()], None), comparison("cmp_rerun", vec![], Some("cmp_origin".into()))];
    s.compare.active_id = Some("cmp_origin".into());
    for id in ["cmp_origin", "cmp_rerun"] {
        s.compare.snapshots.insert(format!("{id}_a"), said("Merge sort is stable.", 30));
        s.compare.snapshots.insert(format!("{id}_b"), said("Merge sort, and insertion sort.", 104));
        s.compare.snapshots.insert(format!("{id}_judge"), said("POINT 1: Names a stable sort\nSCORE A 1: met. Merge sort.\nSCORE B 1: met. Merge sort.", 5));
    }
    let mut rt = rune::Runtime::new(s, |s, _| crate::compare_view::view(s, &Palette::new(true)), (1280.0, 4000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let keys: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key).collect();
    assert!(keys.iter().any(|k| k == "benchmark-chart-plot"), "the chart is drawn: {keys:?}");
    let leaders: Vec<&String> = keys.iter().filter(|k| k.starts_with("benchmark-leader-")).collect();
    assert_eq!(leaders, vec!["benchmark-leader-codex:nemotron-3-ultra-preview-long"], "the second label met the first and was moved");
}
