//! The benchmark's score chart (Electron's `ScoreChart` in CompareBenchmark.tsx), drawn where core's
//! `compare_chart::chart_layout` places each part: the grid at 0, 50 and 100, the axis, its ticks and name, each point,
//! its label, and a leader joining a label moved off its point's line to its point. Colours and sizes are shell.css's
//! `.lc-benchmark__*`: hairline grid, control-border axis and leaders, primary points, secondary 12px labels, muted 11px
//! mono ticks.
use super::*;
use crate::look::controls::Look;
use locust_core::benchmark::BenchmarkChart;
use locust_core::compare_chart::{self as cc, chart_layout};

/// A text's top for an SVG baseline at `y`, at `size` points.
fn top_for(y: f32, size: f32) -> f32 {
    y - size
}

pub(super) fn score_chart(chart: &BenchmarkChart, l: &Look) -> El {
    let layout = chart_layout(chart);
    let tick = rune::TextStyle::new(11.0, 400.0).mono();
    let words = rune::TextStyle::new(12.0, 400.0);
    let corner = rune::TextStyle::new(11.0, 400.0).tracking(0.44);
    let muted = l.c("text-muted");
    let line = |from: [f32; 2], to: [f32; 2], color: Color, shape: rune::Shape| shape.line(from, to, 1.0, rune::LineCap::Butt, color);
    let layer = |shape: rune::Shape| rune::canvas(shape.view_box(cc::WIDTH, cc::HEIGHT)).absolute(0.0, 0.0).size(cc::WIDTH, cc::HEIGHT);

    let mut lines = rune::Shape::new();
    for score in [0.0, 50.0, 100.0] {
        let y = cc::y_at(score);
        lines = line([cc::LEFT, y], [cc::WIDTH - cc::RIGHT, y], l.c("border-hairline"), lines);
    }
    lines = line([cc::LEFT, cc::TOP], [cc::LEFT, cc::TOP + cc::PLOT_HEIGHT], l.c("border-control"), lines);
    let mut plot = el()
        .key("benchmark-chart-plot")
        .size(cc::WIDTH, cc::HEIGHT)
        .role("img")
        .label(layout.said.clone())
        .child(layer(lines));

    if let Some(name) = layout.corner {
        plot = plot.child(text(name).key("benchmark-chart-corner").font(corner).color(muted).absolute(cc::LEFT + 8.0, top_for(cc::TOP + 12.0, 11.0)));
    }
    for score in [0u32, 50, 100] {
        let y = cc::y_at(score as f64) + 4.0;
        plot = plot.child(
            row().absolute(0.0, top_for(y, 11.0)).w(cc::LEFT - 8.0).justify_end().child(text(score.to_string()).font(tick).color(muted)),
        );
    }
    let ticks_y = top_for(cc::TOP + cc::PLOT_HEIGHT + 18.0, 11.0);
    plot = plot.child(text(layout.zero_tick.clone()).key("benchmark-chart-zero").font(tick).color(muted).absolute(cc::LEFT, ticks_y));
    // Every model free: one tick, not "$0" twice at the same place.
    if let Some((x, said)) = &layout.far_tick {
        plot = plot.child(
            row().key("benchmark-chart-far").absolute(x - 60.0, ticks_y).w(120.0).justify_center().child(text(said.clone()).font(tick).color(muted)),
        );
    }
    // The axis's name a line under its ticks: beside them, the far tick ran into it ("34stime a task").
    plot = plot.child(
        row()
            .absolute(0.0, top_for(cc::HEIGHT - 4.0, 11.0))
            .w(cc::WIDTH - cc::RIGHT)
            .justify_end()
            .child(text(layout.axis_name).font(tick).color(muted)),
    );
    for label in &layout.labels {
        // Moved down to clear another label, it is joined to its point: three at 100 read as three loose names.
        if let Some((from, to)) = label.leader() {
            plot = plot.child(layer(line(from, to, l.c("border-control"), rune::Shape::new())).key(format!("benchmark-leader-{}", label.key)));
        }
        plot = plot
            .child(
                el().key(format!("benchmark-point-{}", label.key))
                    .absolute(label.x - 5.0, label.point_y - 5.0)
                    .size(10.0, 10.0)
                    .radius(5.0)
                    .fill(l.c("text-primary")),
            )
            .child(
                text(label.text.clone())
                    .key(format!("benchmark-label-{}", label.key))
                    .font(words)
                    .color(l.c("text-secondary"))
                    .absolute(label.left(), top_for(label.y + 4.0, 12.0)),
            );
    }
    col()
        .key("benchmark-chart")
        .w_full()
        .gap(4.0)
        .child(plot)
        .child(text(layout.caption.clone()).key("benchmark-chart-caption").font(l.ty.body).color(l.c("text-secondary")).wrap())
}

#[cfg(test)]
#[path = "compare_chart_tests.rs"]
mod compare_chart_tests;
