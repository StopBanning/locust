use super::*;
use locust_core::benchmark::{
    AcrossRow, BenchmarkEntry, across_benchmarks, benchmark_leader, benchmark_models_line,
    cost_cell, leader_parts, ordered_benchmarks,
};
use locust_core::compare::RerunStep;
use locust_core::ledger::duration_text;

pub(super) fn open(s: &mut State) {
    crate::room_screen::close(s);
    s.benchmarks_open = true;
    s.compare.active_id = None;
    s.home.home_open = false;
    s.home.board_open = false;
    s.home.boot_open = false;
    s.people_panel.open = false;
    s.settings_panel.open = false;
    s.routines.open = false;
    s.changes.open = false;
}

pub(super) fn view(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let entries = crate::compare_view::benchmark_entries(s);
    let ordered = ordered_benchmarks(&entries);
    let total_runs: usize = entries.iter().map(|e| e.runs).sum();
    let meta = if entries.is_empty() {
        "none yet".to_owned()
    } else {
        let b_word = if entries.len() == 1 {
            "1 benchmark".to_owned()
        } else {
            format!("{} benchmarks", entries.len())
        };
        format!("{b_word} · {total_runs} runs")
    };

    let mut root = col()
        .key("benchmarks-screen")
        .grow(1.0)
        .h_full()
        .fill(p.bg);

    let header = row()
        .key("benchmarks-header")
        .w_full()
        .px(20.0)
        .py(14.0)
        .gap(12.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
        .child(
            text("Benchmarks")
                .font(rune::TextStyle::new(16.0, 600.0))
                .color(p.text),
        )
        .child(text(meta).font(l.ty.label).color(p.muted).grow(1.0))
        .child(
            crate::look::controls::button(
                &l,
                "Back",
                crate::look::controls::Tone::Neutral,
                false,
            )
            .key("benchmarks-back")
            .on_press(|s: &mut State| {
                s.benchmarks_open = false;
            }),
        );
    root = root.child(header);

    let mut body = col().w_full().gap(16.0).padding(20.0);

    if entries.is_empty() {
        body = body.child(
            col()
                .key("benchmarks-empty")
                .w_full()
                .padding(18.0)
                .gap(8.0)
                .radius(8.0)
                .fill(p.raised)
                .border(1.0, p.hairline)
                .child(
                    text("A benchmark is a comparison its judge scored, run again: the same models on the same question, scored on the same points, so one lucky run does not decide which model a teammate uses.")
                        .font(l.ty.body)
                        .color(p.text)
                        .wrap(),
                )
                .child(
                    text("Compare two models, ask a judge to score them, then press Run 3 more times.")
                        .font(l.ty.label)
                        .color(p.muted)
                        .wrap(),
                ),
        );
    } else {
        let row_slices: Vec<&[locust_core::benchmark::BenchmarkRow]> =
            entries.iter().map(|e| e.rows.as_slice()).collect();
        let across = across_benchmarks(&row_slices);
        if !across.is_empty() {
            body = body.child(render_across(&across, p, &l));
        }
        let mut list = col()
            .key("benchmarks-list")
            .w_full()
            .gap(6.0)
            .padding(10.0)
            .radius(8.0)
            .fill(p.raised)
            .border(1.0, p.hairline);
        for entry in ordered {
            list = list.child(render_row(&entry, p, &l));
        }
        body = body.child(list);
    }

    root.child(
        col()
            .grow(1.0)
            .w_full()
            .scroll(ScrollSpec { stick_bottom: false })
            .child(body),
    )
}

fn render_across(
    rows: &[AcrossRow],
    p: &Palette,
    l: &crate::look::controls::Look,
) -> El {
    let best = rows.iter().filter_map(|r| r.score).max();
    let alone = best.is_some_and(|b| rows.iter().filter(|r| r.score == Some(b)).count() == 1);
    let checked = rows.iter().any(|r| r.checked > 0);

    let mut card = col()
        .key("benchmarks-across")
        .w_full()
        .padding(14.0)
        .gap(8.0)
        .radius(8.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .child(
            text("ACROSS YOUR BENCHMARKS")
                .font(l.ty.label)
                .color(p.muted),
        );

    let mut hdr = row()
        .w_full()
        .gap(8.0)
        .pb(4.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
        .child(text("Model").font(l.ty.label).color(p.muted).grow(2.0))
        .child(text("Benchmarks").font(l.ty.label).color(p.muted).grow(1.0))
        .child(text("Score").font(l.ty.label).color(p.muted).grow(1.0))
        .child(text("Met every point").font(l.ty.label).color(p.muted).grow(1.0));
    if checked {
        hdr = hdr.child(text("Check passed").font(l.ty.label).color(p.muted).grow(1.0));
    }
    hdr = hdr
        .child(text("Time a task").font(l.ty.label).color(p.muted).grow(1.0))
        .child(text("Cost a task").font(l.ty.label).color(p.muted).grow(1.0));
    card = card.child(hdr);

    for r in rows {
        let is_best = alone && best.is_some() && r.score == best;
        let score_txt = r
            .score
            .map_or_else(|| "not scored".to_owned(), |s| s.to_string());
        let every_txt = if r.scored == 0 {
            "—".to_owned()
        } else {
            format!("{} of {}", r.every_point, r.scored)
        };
        let time_txt = r.span_ms.map_or_else(|| "—".to_owned(), duration_text);
        let cost_txt = cost_cell(r.cost.as_ref(), &r.model);

        let mut tr = row()
            .key(format!("across-row-{}", r.key))
            .w_full()
            .gap(8.0)
            .py(4.0)
            .child(text(r.name.clone()).font(l.ty.body).color(p.text).grow(2.0))
            .child(
                text(r.benchmarks.to_string())
                    .font(l.ty.body)
                    .color(p.text)
                    .grow(1.0),
            )
            .child(
                text(score_txt)
                    .font(l.ty.body)
                    .color(if is_best { p.green } else { p.text })
                    .grow(1.0),
            )
            .child(text(every_txt).font(l.ty.body).color(p.text).grow(1.0));
        if checked {
            let chk_txt = if r.checked == 0 {
                "—".to_owned()
            } else {
                format!("{} of {}", r.check_passed, r.checked)
            };
            let chk_col = if r.checked == 0 {
                p.text
            } else if r.check_passed == r.checked {
                p.green
            } else {
                p.amber
            };
            tr = tr.child(text(chk_txt).font(l.ty.body).color(chk_col).grow(1.0));
        }
        tr = tr
            .child(text(time_txt).font(l.ty.body).color(p.text).grow(1.0))
            .child(text(cost_txt).font(l.ty.body).color(p.text).grow(1.0));
        card = card.child(tr);
    }

    let note = format!(
        "Each model's score is the average of its scores in the benchmarks it ran in, each benchmark counted once. Met every point{} its runs in all of them.",
        if checked {
            " and Check passed count"
        } else {
            " counts"
        }
    );
    card.child(text(note).font(l.ty.label).color(p.muted).wrap())
}

fn render_row(
    entry: &BenchmarkEntry,
    p: &Palette,
    l: &crate::look::controls::Look,
) -> El {
    let models_line = benchmark_models_line(&entry.models);
    let judge_part = entry
        .judge_name
        .as_deref()
        .map(|n| format!(" · judged by {n}"))
        .unwrap_or_default();
    let changes_part = if entry.changes { " · changes files" } else { "" };
    let meta = format!("{models_line} · {} runs{judge_part}{changes_part}", entry.runs);

    let chip_el = if let Some(prog) = &entry.progress {
        let prefix = if prog.of == 1 {
            "running once more".to_owned()
        } else {
            format!("running again, {} of {}", prog.run, prog.of)
        };
        let step = match prog.step {
            RerunStep::Checking => "waiting for checks",
            RerunStep::Judging => "judging",
            RerunStep::Answering => "answering",
        };
        crate::look::controls::chip(l, &format!("{prefix} · {step}"), true)
    } else if let Some(leader) = benchmark_leader(&entry.rows) {
        let (who, tie) = leader_parts(&leader.names);
        let label = if tie {
            format!("{who} tie · {}", leader.score)
        } else {
            format!("{who} · {}", leader.score)
        };
        crate::look::controls::chip(l, &label, true)
    } else {
        crate::look::controls::chip(l, "not scored yet", false)
    };

    let cid = entry.compare_id.clone();
    row()
        .key(format!("benchmark-row-{cid}"))
        .w_full()
        .padding(10.0)
        .gap(12.0)
        .center_y()
        .radius(6.0)
        .fill(p.bg)
        .hover(l.c("hover-surface"))
        .role("button")
        .label(entry.question.clone())
        .child(
            col()
                .grow(1.0)
                .gap(2.0)
                .child(
                    text(entry.question.clone())
                        .font(rune::TextStyle::new(13.5, 600.0))
                        .color(p.text)
                        .truncate(),
                )
                .child(text(meta).font(l.ty.label).color(p.muted).truncate()),
        )
        .child(chip_el)
        .on_press(move |s: &mut State| {
            s.benchmarks_open = false;
            s.compare.active_id = Some(cid.clone());
        })
}
