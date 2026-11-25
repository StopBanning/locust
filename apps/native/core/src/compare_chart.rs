//! Electron's `ScoreChart` (CompareBenchmark.tsx, 0.763; labels 0.811): where each part of the benchmark's score chart
//! sits in its 640 x 244 box. Up is better; left is cheaper, or faster where too few runs were priced in dollars.
//! Labels sit beside their points, turned to the left near the right edge, and a line apart when two would touch -- by
//! the width each label's words take, not by how far apart its points are (two models at 100, one label running right
//! and the other left, met in the middle and printed over each other in the packaged 0.810 benchmark). A label moved
//! down to clear another is joined to its point by a leader. The window draws it (`compare_chart.rs`).
use crate::benchmark::{BenchmarkChart, ChartAxis, TimeBecause};

pub const WIDTH: f32 = 640.0;
pub const HEIGHT: f32 = 244.0;
pub const LEFT: f32 = 40.0;
pub const RIGHT: f32 = 20.0;
pub const TOP: f32 = 16.0;
pub const BOTTOM: f32 = 44.0;
/// A label's character at its 12px, a little wide on purpose: a label measured short is one printed over another.
pub const LABEL_CHAR: f32 = 6.8;
/// How far a label moves to clear another: one line.
const LINE: f32 = 14.0;

pub const PLOT_WIDTH: f32 = WIDTH - LEFT - RIGHT;
pub const PLOT_HEIGHT: f32 = HEIGHT - TOP - BOTTOM;

/// Where a score sits up the chart.
pub fn y_at(score: f64) -> f32 {
    TOP + (1.0 - score as f32 / 100.0) * PLOT_HEIGHT
}

/// One point's label, placed.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartLabel {
    pub key: String,
    /// "Claude Opus 5.5 · 100", "… · free" for a free model.
    pub text: String,
    /// The point's own place.
    pub x: f32,
    pub point_y: f32,
    /// The label's line: the point's, or lower to clear a label already placed.
    pub y: f32,
    /// Reads to the right of its point; else to the left, ending at it.
    pub right: bool,
    /// The width its words take, at `LABEL_CHAR` a character.
    pub width: f32,
}

impl ChartLabel {
    /// The leader (`lc-benchmark__leader`) joining a label moved off its point's line to its point; none for a label on
    /// its point's line.
    pub fn leader(&self) -> Option<([f32; 2], [f32; 2])> {
        (self.y != self.point_y).then(|| ([if self.right { self.x + 8.0 } else { self.x - 8.0 }, self.y], [self.x, self.point_y]))
    }
    /// Where the label's words start: 10 past the point, or ending 10 before it.
    pub fn left(&self) -> f32 {
        if self.right { self.x + 10.0 } else { self.x - 10.0 - self.width }
    }
}

/// The whole chart, placed.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartLayout {
    pub cost: bool,
    pub labels: Vec<ChartLabel>,
    /// "Strong and cheap" / "Strong and fast" in the top-left corner, unless a label stands there.
    pub corner: Option<&'static str>,
    /// The x axis's first tick, at the left edge.
    pub zero_tick: String,
    /// The far tick, at the furthest point; none when every point is at 0 (every model free: "$0" once, not twice).
    pub far_tick: Option<(f32, String)>,
    /// The axis's name, a line under its ticks.
    pub axis_name: &'static str,
    /// The chart as a screen reader hears it: "Score against cost a task: Claude Haiku 5.5 100 at $0.012; …".
    pub said: String,
    /// The line under it.
    pub caption: String,
}

/// Dollars as the chart's axis says them: enough places for a cheap task.
pub fn dollar_tick(value: f64) -> String {
    if value == 0.0 {
        "$0".to_owned()
    } else if value < 0.1 {
        format!("${value:.3}")
    } else {
        format!("${value:.2}")
    }
}

pub fn chart_layout(chart: &BenchmarkChart) -> ChartLayout {
    let max_x = chart.points.iter().map(|p| p.x).fold(0.0_f64, f64::max);
    let span = if max_x > 0.0 { max_x * 1.15 } else { 1.0 };
    let x_at = |value: f64| LEFT + (value / span) as f32 * PLOT_WIDTH;
    let cost = chart.axis == ChartAxis::Cost;
    let tick = |value: f64| if cost { dollar_tick(value) } else { crate::ledger::duration_text(value.max(0.0).round() as u64) };
    let mut placed: Vec<(f32, f32, f32)> = Vec::new();
    let labels: Vec<ChartLabel> = chart
        .points
        .iter()
        .map(|point| {
            let x = x_at(point.x);
            let point_y = y_at(point.score as f64);
            let right = x < LEFT + PLOT_WIDTH * 0.62;
            let text = format!("{} · {}{}", point.name, point.score, if point.free { " · free" } else { "" });
            // JavaScript's length: UTF-16 units.
            let width = text.encode_utf16().count() as f32 * LABEL_CHAR;
            let (left, reach) = if right { (x - 6.0, x + 10.0 + width) } else { (x - 10.0 - width, x + 6.0) };
            let mut y = point_y;
            while placed.iter().any(|&(other_left, other_right, other_y)| {
                (other_y - y).abs() < LINE && other_left < reach && left < other_right
            }) {
                y += LINE;
            }
            placed.push((left, reach, y));
            ChartLabel { key: point.key.clone(), text, x, point_y, y, right, width }
        })
        .collect();
    let said = chart
        .points
        .iter()
        .map(|p| format!("{} {} at {}", p.name, p.score, if p.free { "free".to_owned() } else { tick(p.x) }))
        .collect::<Vec<_>>()
        .join("; ");
    // The corner's name gives way to a point's: a free model stands at the left edge, and a strong one at the top.
    let corner_clear = !labels.iter().any(|l| l.x < LEFT + 150.0 && l.y < TOP + 26.0);
    let mut caption = if cost {
        "Up is better, left is cheaper. Time a task is in the table; it does not move the chart.".to_owned()
    } else if chart.because == Some(TimeBecause::SamePrice) {
        "Up is better, left is faster: every one cost the same here, so the chart is against time.".to_owned()
    } else {
        "Up is better, left is faster: too few of these runs were priced in dollars to chart cost.".to_owned()
    };
    if !chart.left.is_empty() {
        caption.push_str(&format!(
            " Not on the chart: {}{}.",
            chart.left.join(", "),
            if cost { ", whose runs carry no price in dollars" } else { ", whose runs have no time" }
        ));
    }
    ChartLayout {
        cost,
        corner: corner_clear.then_some(if cost { "Strong and cheap" } else { "Strong and fast" }),
        zero_tick: tick(0.0),
        far_tick: (max_x > 0.0).then(|| (x_at(max_x), tick(max_x))),
        axis_name: if cost { "cost a task" } else { "time a task" },
        said: format!("Score against {} a task: {said}", if cost { "cost" } else { "time" }),
        caption,
        labels,
    }
}
