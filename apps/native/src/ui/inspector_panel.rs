//! Reply route/provenance, permissions, artifacts, receipt, and recovery inspector
//! (Electron `renderer/src/components/Inspector.tsx`, PARITY row 26).

use super::*;
use locust_core::inspector::{
    self, RunReport, SignalTone, build_conversation_reports, checkpoint_label, no_artifacts_line,
    short_mission_id,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tab {
    #[default]
    Activity,
    Details,
    Artifacts,
    Receipt,
}

impl Tab {
    const ALL: [Self; 4] = [Self::Activity, Self::Details, Self::Artifacts, Self::Receipt];

    fn label(self) -> &'static str {
        match self {
            Self::Activity => "Activity",
            Self::Details => "Details",
            Self::Artifacts => "Artifacts",
            Self::Receipt => "Receipt",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Activity => "inspector-tab-activity",
            Self::Details => "inspector-tab-details",
            Self::Artifacts => "inspector-tab-artifacts",
            Self::Receipt => "inspector-tab-receipt",
        }
    }
}

pub(super) fn control(s: &State, p: &Palette) -> Option<El> {
    selected_conversation(s)?;
    let l = crate::look::controls::Look::new(p.dark);
    let mut icon = crate::look::controls::head_icon(
        &crate::look::icons::ACTIVITY,
        "Activity",
        "Activity: what this conversation's runs did",
    )
    .on_press(|s: &mut State| {
        s.inspector_open = !s.inspector_open;
        if s.inspector_open {
            s.review = None;
            s.changes.open = false;
            s.terminal.open = false;
        }
    });
    if s.inspector_open {
        icon = icon.state(rune_kit::State::Pressed);
    }
    Some(icon.build(l.k).key("inspector-open"))
}

fn reports_for_state(s: &State) -> Vec<RunReport> {
    let Some(pane) = &s.ledger else {
        return Vec::new();
    };
    let workspace = pane
        .header
        .folder
        .as_deref()
        .or(s.home.folder_path.as_deref());
    let inputs = pane
        .worker
        .as_ref()
        .map(|w| w.inspector_inputs())
        .unwrap_or_default();
    build_conversation_reports(&inputs, workspace)
}

fn tone_color(tone: SignalTone, p: &Palette) -> Color {
    match tone {
        SignalTone::Muted => p.muted,
        SignalTone::Green => p.green,
        SignalTone::Amber => p.amber,
        SignalTone::Red => p.red,
        SignalTone::Live => p.text,
    }
}

fn detail_row(label: &str, value: String, _mono: bool, p: &Palette) -> El {
    row()
        .w_full()
        .gap(8.0)
        .py(3.0)
        .child(
            text(label.to_owned())
                .w(88.0)
                .font(type_::LABEL)
                .color(p.muted),
        )
        .child(
            text(value)
                .font(type_::LABEL)
                .color(p.text)
                .grow(1.0)
                .shrink()
                .wrap_anywhere(),
        )
}

pub(super) fn panel(s: &State, p: &Palette) -> Option<El> {
    if !s.inspector_open {
        return None;
    }
    selected_conversation(s)?;
    let reports = reports_for_state(s);

    let header = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .px(12.0)
        .py(10.0)
        .border(1.0, p.hairline)
        .child(
            text("About this reply")
                .key("inspector-title")
                .font(type_::BODY)
                .color(p.text)
                .grow(1.0),
        )
        .child(
            text("Close")
                .key("inspector-close")
                .font(type_::LABEL)
                .color(p.secondary)
                .px(8.0)
                .py(4.0)
                .fill(p.field)
                .radius(6.0)
                .on_press(|s: &mut State| {
                    s.inspector_open = false;
                }),
        );

    let mut tabs = row().w_full().gap(4.0).px(12.0).py(6.0);
    for tab in Tab::ALL {
        let active = s.inspector_tab == tab;
        tabs = tabs.child(
            text(tab.label().to_owned())
                .key(tab.key())
                .font(type_::LABEL)
                .color(if active { p.text } else { p.secondary })
                .px(8.0)
                .py(5.0)
                .fill(if active { p.selected } else { p.field })
                .radius(6.0)
                .on_press(move |s: &mut State| {
                    s.inspector_tab = tab;
                }),
        );
    }

    let mut body = col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .gap(10.0)
        .padding(12.0);

    if reports.is_empty() {
        body = body.child(
            text("No events yet.")
                .font(type_::LABEL)
                .color(p.muted)
                .wrap(),
        );
    } else {
        let selected_idx = s
            .inspector_turn
            .unwrap_or_else(|| reports.len().saturating_sub(1))
            .min(reports.len().saturating_sub(1));

        if reports.len() > 1 {
            let mut switcher = row().w_full().gap(4.0).wrap();
            for (idx, r) in reports.iter().enumerate() {
                let on = idx == selected_idx;
                let label = format!("Turn {} · {}", idx + 1, inspector::runtime_display_name(&r.runtime));
                switcher = switcher.child(
                    text(label)
                        .key(format!("inspector-turn-{idx}"))
                        .font(type_::LABEL)
                        .color(if on { p.text } else { p.secondary })
                        .px(8.0)
                        .py(4.0)
                        .fill(if on { p.selected } else { p.field })
                        .radius(6.0)
                        .on_press(move |s: &mut State| {
                            s.inspector_turn = Some(idx);
                        }),
                );
            }
            body = body.child(switcher);
        }

        let report = &reports[selected_idx];
        match s.inspector_tab {
            Tab::Activity => {
                body = body.child(
                    text("What happened")
                        .font(type_::LABEL)
                        .color(p.muted),
                );
                if report.signal_rail.is_empty() {
                    body = body.child(text("No events yet.").font(type_::LABEL).color(p.muted));
                } else {
                    let mut rail = col().w_full().gap(6.0);
                    for r in &report.signal_rail {
                        rail = rail.child(
                            col()
                                .key(format!("rail-{}", r.key))
                                .w_full()
                                .gap(1.0)
                                .padding(6.0)
                                .fill(p.field)
                                .radius(6.0)
                                .child(
                                    text(r.name.clone())
                                        .font(type_::LABEL)
                                        .color(tone_color(r.tone, p))
                                        .wrap_anywhere(),
                                )
                                .child(
                                    text(r.meta.clone())
                                        .font(type_::LABEL)
                                        .color(p.muted),
                                ),
                        );
                    }
                    body = body.child(rail);
                }

                if let Some(note) = &report.recovery_note {
                    body = body.child(
                        col()
                            .key("inspector-recovery")
                            .w_full()
                            .gap(4.0)
                            .padding(8.0)
                            .fill(p.field)
                            .radius(6.0)
                            .child(text("Recovery").font(type_::LABEL).color(p.amber))
                            .child(
                                text(note.clone())
                                    .font(type_::LABEL)
                                    .color(p.text)
                                    .wrap(),
                            ),
                    );
                }

                if let Some(may) = &report.may_do {
                    let mut perms = col()
                        .key("inspector-permissions")
                        .w_full()
                        .gap(6.0)
                        .padding(8.0)
                        .fill(p.field)
                        .radius(6.0)
                        .child(
                            row()
                                .w_full()
                                .center_y()
                                .gap(8.0)
                                .child(
                                    text("What it may do")
                                        .font(type_::LABEL)
                                        .color(p.text)
                                        .grow(1.0),
                                )
                                .child(
                                    text(report.sandbox_label.to_owned())
                                        .font(type_::LABEL)
                                        .color(p.muted),
                                ),
                        );
                    for row_item in &may.rows {
                        let color = if row_item.verdict == "deny" {
                            p.red
                        } else if row_item.wide {
                            p.amber
                        } else {
                            p.green
                        };
                        perms = perms.child(
                            row()
                                .w_full()
                                .gap(6.0)
                                .child(text(row_item.verdict.to_owned()).w(36.0).font(type_::LABEL).color(color))
                                .child(
                                    text(row_item.text.clone())
                                        .font(type_::LABEL)
                                        .color(p.secondary)
                                        .grow(1.0)
                                        .shrink()
                                        .wrap(),
                                ),
                        );
                    }
                    perms = perms.child(
                        text(format!(
                            "Locust sets the folder, the tool and these limits when the mission starts; nothing here changes while it runs. {}",
                            may.rest
                        ))
                        .font(type_::LABEL)
                        .color(p.muted)
                        .wrap(),
                    );
                    body = body.child(perms);
                }
            }
            Tab::Details => {
                body = body
                    .child(detail_row("Runtime", report.runtime_label.clone(), false, p))
                    .child(detail_row("Model", report.model.clone(), false, p))
                    .child(detail_row("Account", report.account.clone(), false, p))
                    .child(detail_row("Run", short_mission_id(&report.mission_id), true, p))
                    .child(detail_row("Sandbox", report.sandbox_label.to_owned(), false, p))
                    .child(detail_row(
                        "Events",
                        format!("{} recorded in this view", report.event_count),
                        false,
                        p,
                    ))
                    .child(detail_row(report.cost_label, report.cost_line.clone(), true, p));
            }
            Tab::Artifacts => {
                if report.artifacts.is_empty() {
                    let sandbox_opt = (!report.sandbox.is_empty()).then_some(report.sandbox.as_str());
                    body = body.child(
                        text(no_artifacts_line(sandbox_opt, report.running, false).to_owned())
                            .font(type_::LABEL)
                            .color(p.muted)
                            .wrap(),
                    );
                } else {
                    for art in &report.artifacts {
                        body = body.child(
                            row()
                                .w_full()
                                .gap(6.0)
                                .padding(6.0)
                                .fill(p.field)
                                .radius(6.0)
                                .child(
                                    text(art.shown.clone())
                                        .font(type_::LABEL)
                                        .color(p.text)
                                        .grow(1.0)
                                        .shrink()
                                        .wrap_anywhere(),
                                )
                                .children(
                                    art.status
                                        .as_ref()
                                        .map(|st| text(st.clone()).font(type_::LABEL).color(p.muted)),
                                ),
                        );
                    }
                }
            }
            Tab::Receipt => {
                if report.running {
                    body = body.child(
                        text("The receipt is written when this run finishes: its phase, checkpoints, event count and whether the ledger verifies.")
                            .font(type_::LABEL)
                            .color(p.muted)
                            .wrap(),
                    );
                } else {
                    let cp_line = if report.checkpoints_count == 0 {
                        "none written".to_owned()
                    } else if let Some(epoch) = report.last_checkpoint_epoch {
                        format!("{} · last {}", report.checkpoints_count, checkpoint_label(epoch))
                    } else {
                        report.checkpoints_count.to_string()
                    };
                    body = body
                        .child(detail_row("Phase", report.phase.clone(), false, p))
                        .child(detail_row("Checkpoints", cp_line, false, p))
                        .child(detail_row("Events", report.event_count.to_string(), false, p))
                        .child(detail_row("Ledger", report.ledger_label.to_owned(), false, p));
                }
            }
        }
    }

    Some(
        col()
            .key("inspector-panel")
            .w(340.0)
            .h_full()
            .fill(p.raised)
            .border(1.0, p.hairline)
            .child(header)
            .child(tabs)
            .child(body),
    )
}
