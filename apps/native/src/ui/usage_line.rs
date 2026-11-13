//! Usage on Home (Electron `UsageLine.tsx`, 0.714): one line, the last 30 days' turns, tokens and money, a bar a day,
//! and Details, which opens the usage dialog. Drawn only once there is something to count, so a new person's Home is
//! unchanged. The ledgers are read off the window's thread when Home opens.

use super::*;
use locust_core::usage::{UsageRange, UsageSummary, day_label, plural, spark_bars, usage_line_facts, usage_summary};

/// Read the last 30 days, once per opening of Home.
pub(super) fn request(s: &mut State) {
    let Some(store) = s.data.clone() else { return };
    if s.usage_line_job.is_pending() {
        return;
    }
    s.usage_line_job.request(store, |store| {
        locust_core::usage_ledger::usage_turns(&store).ok().map(|turns| usage_summary(&turns, UsageRange::Days30, chrono::Local::now()))
    });
}

pub(super) fn poll(s: &mut State) -> bool {
    let Some(summary) = s.usage_line_job.take() else { return false };
    s.usage_line = summary;
    true
}

pub(super) fn line(s: &State, l: &crate::look::controls::Look) -> Option<El> {
    let summary: &UsageSummary = s.usage_line.as_ref().filter(|u| u.turns > 0)?;
    let tall = 14.0_f32;
    let mut spark = row().gap(1.5).h(tall).items_end();
    for bar in spark_bars(&summary.days, &summary.today) {
        let h = if bar.turns == 0 { 1.5 } else { (bar.height as f32 * tall).max(2.0) };
        let colour = if bar.today { l.c("text-primary") } else if bar.turns == 0 { l.c("border-hairline") } else { l.c("text-muted") };
        spark = spark.child(el().w(3.0).h(h).radius(1.0).fill(colour).label(format!("{}: {}", day_label(&bar.date), plural(bar.turns, "turn"))));
    }
    Some(
        row()
            .w_full()
            .gap(10.0)
            .center_y()
            .key("usage-line")
            .child(text("Usage").font(l.ty.control).color(l.c("text-primary")))
            .child(text("last 30 days").font(l.ty.body).color(l.c("text-muted")))
            .child(rune::spacer())
            .child(
                row()
                    .gap(10.0)
                    .center_y()
                    .key("usage-line-open")
                    .role("button")
                    .label("Every agent and model: limits, activity and what each used")
                    .child(text(usage_line_facts(summary)).font(l.ty.body).color(l.c("text-secondary")))
                    .child(spark)
                    .child(text("Details").font(l.ty.body).color(l.c("text-primary")))
                    .child(crate::look::controls::glyph(l, &crate::look::icons::CHEVRON_RIGHT, 12.0, Some(l.c("text-muted"))))
                    .on_press(crate::usage_panel::open),
            ),
    )
}
