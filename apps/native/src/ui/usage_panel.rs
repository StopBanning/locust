//! Usage dialog — Area 11 / P10.

use super::*;
use crate::context_usage::compact_count;
use crate::usage::{
    self, USAGE_ACTIVITY, USAGE_ADDING, USAGE_COUNTED, USAGE_FOOT_TITLE, USAGE_LIMITS,
    USAGE_LIMITS_EMPTY, USAGE_LIMITS_NOTE, USAGE_MODELS, USAGE_MODELS_NOTE, USAGE_PERCENT_LEFT,
    USAGE_READ_FAIL, USAGE_RESET_SINCE, USAGE_SUBTITLE, USAGE_TITLE, UsageRange, UsageSummary,
    UsageTurn, cached_text, paid_by, parse_plan_windows, range_phrase, share_of, spent_sentence,
    streak_text, total_tokens, usage_line_facts, usage_summary,
};
use crate::usage_ledger;

#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    pub range: UsageRange,
    pub summary: Option<Result<UsageSummary, String>>,
    pub windows: Vec<(String, Vec<usage::PlanWindow>)>,
    pub loading: bool,
}

fn button(label: impl Into<String>, p: &Palette) -> El {
    text(label.into())
        .font(type_::LABEL)
        .color(p.secondary)
        .padding(7.0)
}

fn active(label: impl Into<String>, p: &Palette, on: bool) -> El {
    text(label.into())
        .font(type_::LABEL)
        .color(if on { p.text } else { p.secondary })
        .padding(7.0)
        .fill(if on { p.selected } else { p.field })
        .radius(6.0)
}

pub(super) fn open(s: &mut State) {
    s.usage_panel.open = true;
    s.usage_panel.range = UsageRange::from_str("30d").unwrap_or(UsageRange::Days30);
    refresh(s);
}

fn refresh(s: &mut State) {
    s.usage_panel.loading = true;
    let range = s.usage_panel.range;
    match &s.data {
        None => {
            s.usage_panel.summary = Some(Err(USAGE_READ_FAIL.into()));
            s.usage_panel.loading = false;
        }
        Some(store) => match usage_ledger::usage_turns(store) {
            Ok(turns) => {
                let summary = usage_summary(&turns, range, chrono::Local::now());
                let mut windows = Vec::new();
                for (account, reading) in usage_ledger::usage_windows(store) {
                    let parsed = parse_plan_windows(&reading, chrono::Utc::now());
                    if !parsed.is_empty() {
                        windows.push((account, parsed));
                    }
                }
                windows.sort_by(|a, b| a.0.cmp(&b.0));
                s.usage_panel.windows = windows;
                s.usage_panel.summary = Some(Ok(summary));
                s.usage_panel.loading = false;
            }
            Err(e) => {
                s.usage_panel.summary = Some(Err(e));
                s.usage_panel.loading = false;
            }
        },
    }
}

pub(super) fn view(s: &State, p: &Palette) -> El {
    let mut body = col().w_full().gap(10.0).padding(16.0);
    body = body
        .child(
            text(USAGE_TITLE.to_owned())
                .font(type_::TITLE)
                .color(p.text),
        )
        .child(
            text(USAGE_SUBTITLE.to_owned())
                .font(type_::LABEL)
                .color(p.muted),
        )
        .child(
            row()
                .gap(6.0)
                .child(
                    active(
                        UsageRange::Days7.name(),
                        p,
                        s.usage_panel.range == UsageRange::Days7,
                    )
                    .on_press(|s: &mut State| {
                        s.usage_panel.range = UsageRange::Days7;
                        refresh(s);
                    }),
                )
                .child(
                    active(
                        UsageRange::Days30.name(),
                        p,
                        s.usage_panel.range == UsageRange::Days30,
                    )
                    .on_press(|s: &mut State| {
                        s.usage_panel.range = UsageRange::Days30;
                        refresh(s);
                    }),
                )
                .child(
                    active(
                        UsageRange::All.name(),
                        p,
                        s.usage_panel.range == UsageRange::All,
                    )
                    .on_press(|s: &mut State| {
                        s.usage_panel.range = UsageRange::All;
                        refresh(s);
                    }),
                ),
        );

    if s.usage_panel.loading {
        body = body.child(
            text(USAGE_ADDING.to_owned())
                .font(type_::LABEL)
                .color(p.muted),
        );
    } else {
        match &s.usage_panel.summary {
            Some(Err(e)) => {
                body = body.child(text(e.clone()).font(type_::LABEL).color(p.red).wrap());
            }
            Some(Ok(summary)) => {
                body = body
                    .child(
                        text(USAGE_ACTIVITY.to_owned())
                            .font(type_::TITLE)
                            .color(p.text),
                    )
                    .child(
                        text(range_phrase(summary.range))
                            .font(type_::LABEL)
                            .color(p.muted),
                    )
                    .child(stat_row("Turns", &usage::number(summary.turns as f64), p))
                    .child(stat_row(
                        "Conversations",
                        &usage::number(summary.conversations as f64),
                        p,
                    ))
                    .child(stat_row(
                        "Active days",
                        &format!("{} of {}", summary.active_days, summary.range_days),
                        p,
                    ))
                    .child(stat_row(
                        "Busiest day",
                        &summary
                            .busiest
                            .as_ref()
                            .map(|d| {
                                format!(
                                    "{} · {}",
                                    usage::day_label(&d.date),
                                    usage::plural(d.turns, "turn")
                                )
                            })
                            .unwrap_or_else(|| "none".into()),
                        p,
                    ))
                    .child(stat_row(
                        "Current streak",
                        &streak_text(summary.streak_current),
                        p,
                    ))
                    .child(stat_row(
                        "Longest streak",
                        &streak_text(summary.streak_longest),
                        p,
                    ))
                    .child(stat_row(
                        "Tokens",
                        &{
                            let n = total_tokens(&summary.tokens);
                            if n == 0 {
                                "none reported".into()
                            } else {
                                compact_count(n)
                            }
                        },
                        p,
                    ));
                body = body.child(stat_row(
                    "From the cache",
                    &usage::cached_percent(&summary.tokens)
                        .map(|n| format!("{n}%"))
                        .unwrap_or_else(|| "—".into()),
                    p,
                ));
                let busiest = summary.busiest.as_ref().map(|d| d.turns).unwrap_or(0);
                let heat = summary
                    .heatmap
                    .iter()
                    .rev()
                    .take(14)
                    .map(|d| match usage::heat_level(d.turns, busiest) {
                        0 => "·",
                        1 => "░",
                        2 => "▒",
                        3 => "▓",
                        _ => "█",
                    })
                    .collect::<Vec<_>>()
                    .join("");
                if !heat.is_empty() {
                    body = body.child(stat_row("Recent heat", &heat, p));
                }
                let _ = summary.range.as_str();

                body = body
                    .child(
                        text(USAGE_LIMITS.to_owned())
                            .font(type_::TITLE)
                            .color(p.text),
                    )
                    .child(
                        text(USAGE_LIMITS_NOTE.to_owned())
                            .font(type_::LABEL)
                            .color(p.muted),
                    );
                if s.usage_panel.windows.is_empty() {
                    body = body.child(
                        text(USAGE_LIMITS_EMPTY.to_owned())
                            .font(type_::LABEL)
                            .color(p.secondary)
                            .wrap(),
                    );
                } else {
                    for (account, windows) in &s.usage_panel.windows {
                        body = body.child(text(account.clone()).font(type_::LABEL).color(p.text));
                        for window in windows {
                            let line = if window.expired {
                                format!("{} · {USAGE_RESET_SINCE}", window.name)
                            } else {
                                format!(
                                    "{} · {}{USAGE_PERCENT_LEFT}",
                                    window.name, window.percent_left
                                )
                            };
                            body = body.child(text(line).font(type_::LABEL).color(p.secondary));
                        }
                    }
                }

                body = body
                    .child(
                        text(USAGE_MODELS.to_owned())
                            .font(type_::TITLE)
                            .color(p.text),
                    )
                    .child(
                        text(USAGE_MODELS_NOTE.to_owned())
                            .font(type_::LABEL)
                            .color(p.muted),
                    );
                if summary.models.is_empty() {
                    body = body.child(
                        text(format!("Nothing ran in {}.", range_phrase(summary.range)))
                            .font(type_::LABEL)
                            .color(p.secondary),
                    );
                } else {
                    body = body.child(
                        text(
                            "Model · Share of turns · Turns · Tokens · Cached · Paid by".to_owned(),
                        )
                        .font(type_::LABEL)
                        .color(p.muted),
                    );
                    for model in &summary.models {
                        let line = format!(
                            "{} · {}% · {} · {} · {} · {}",
                            model.model,
                            share_of(model, summary),
                            model.turns,
                            compact_count(total_tokens(&model.tokens)),
                            cached_text(model),
                            paid_by(model),
                        );
                        body = body.child(text(line).font(type_::LABEL).color(p.text).wrap());
                    }
                }

                body = body
                    .child(
                        text(usage_line_facts(summary))
                            .font(type_::LABEL)
                            .color(p.text)
                            .wrap(),
                    )
                    .child(
                        text(spent_sentence(summary))
                            .font(type_::LABEL)
                            .color(p.secondary)
                            .wrap(),
                    )
                    .child(
                        text(USAGE_COUNTED.to_owned())
                            .font(type_::LABEL)
                            .color(p.muted)
                            .wrap(),
                    )
                    .child(
                        text(USAGE_FOOT_TITLE.to_owned())
                            .font(type_::LABEL)
                            .color(p.muted)
                            .wrap(),
                    );
            }
            None => {}
        }
    }

    body = body.child(button("Close", p).on_press(|s: &mut State| s.usage_panel.open = false));

    col()
        .key("usage-dialog")
        .w(640.0)
        .h(720.0)
        .radius(10.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .child(
            col()
                .w_full()
                .h_full()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(body),
        )
}

fn stat_row(label: &str, value: &str, p: &Palette) -> El {
    row()
        .w_full()
        .child(
            text(label.to_owned())
                .font(type_::LABEL)
                .color(p.secondary)
                .grow(1.0),
        )
        .child(text(value.to_owned()).font(type_::LABEL).color(p.text))
}

pub(super) fn scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    let turns = demo_turns();
    let summary = usage_summary(&turns, UsageRange::Days30, chrono::Local::now());
    s.usage_panel = Panel {
        open: true,
        range: UsageRange::Days30,
        summary: Some(Ok(summary)),
        windows: vec![(
            "codex".into(),
            parse_plan_windows(
                "5-hour window 40% used · week window 22% left",
                chrono::Utc::now(),
            ),
        )],
        loading: false,
    };
}

pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
}

fn demo_turns() -> Vec<UsageTurn> {
    vec![
        UsageTurn {
            at: "2026-11-10T12:00:00.000Z".into(),
            runtime: "codex".into(),
            model: "gpt-5.6-sol".into(),
            conversation: "c1".into(),
            input_tokens: Some(12_000),
            output_tokens: Some(800),
            cache_read_tokens: None,
            cache_write_tokens: None,
            usd: None,
            premium_requests: None,
            plan: true,
        },
        UsageTurn {
            at: "2026-11-09T15:00:00.000Z".into(),
            runtime: "claude".into(),
            model: "opus".into(),
            conversation: "c2".into(),
            input_tokens: Some(4_000),
            output_tokens: Some(1_200),
            cache_read_tokens: Some(2_000),
            cache_write_tokens: Some(100),
            usd: Some(0.37),
            premium_requests: None,
            plan: false,
        },
        UsageTurn {
            at: "2026-11-08T09:00:00.000Z".into(),
            runtime: "opencode".into(),
            model: "deepseek-v4.1-flash-free".into(),
            conversation: "c3".into(),
            input_tokens: Some(900),
            output_tokens: Some(200),
            cache_read_tokens: None,
            cache_write_tokens: None,
            usd: None,
            premium_requests: None,
            plan: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_seeds_activity() {
        let mut s = State::default();
        scene(&mut s);
        assert!(s.usage_panel.open);
        assert_eq!(s.usage_panel.range.as_str(), "30d");
        assert_eq!(UsageRange::from_str("7d"), Some(UsageRange::Days7));
        let summary = s.usage_panel.summary.as_ref().unwrap().as_ref().unwrap();
        assert!(summary.turns >= 1);
        assert!(!usage_line_facts(summary).is_empty());
    }
}
