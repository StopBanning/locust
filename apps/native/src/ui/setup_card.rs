//! A TEAMMATE'S SETUP (`TeammateSetup.tsx`, `App.tsx` `openSetup`).
//!
//! `/setup` in its conversation, or Setup on its card in Team: reads what shapes its turns
//! fresh from disk, in the order a turn is given them. It changes nothing; Edit teammate is
//! one press away.

use super::*;
use locust_core::teammate_setup::{
    TeammateSetup, read_teammate_setup, setup_sections,
};
use serde_json::Value;

#[derive(Clone, Debug)]
pub(super) struct SetupCard {
    pub(super) teammate_id: String,
    pub(super) name: String,
    pub(super) runtime_display: String,
    pub(super) model_line: String,
    pub(super) setup: Option<TeammateSetup>,
    pub(super) problem: Option<String>,
}

fn home_dir() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn project_dir(s: &State) -> std::path::PathBuf {
    s.home
        .folder_path
        .as_deref()
        .filter(|p| !p.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn last_turn_events(s: &State, teammate_id: &str) -> Vec<Value> {
    let Some(store) = &s.data else {
        return Vec::new();
    };
    let Some(pane) = &s.ledger else {
        return Vec::new();
    };
    let tip = &pane.snapshot.tip;
    if tip.is_empty() {
        return Vec::new();
    }
    let belongs = s
        .teammates
        .get(s.selected)
        .is_some_and(|t| t.id == teammate_id);
    if !belongs {
        return Vec::new();
    }
    let path = store.root().join("mission-ledger").join(format!("{tip}.jsonl"));
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
}

pub(super) fn open_for_teammate(s: &mut State, teammate_id: &str) {
    let Some(mate) = s.teammates.iter().find(|t| t.id == teammate_id) else {
        s.composer_note = Some("Open a teammate’s conversation to see its setup.".into());
        return;
    };
    let runtime_id = mate.runtime.as_deref().unwrap_or("codex");
    let runtime_display = locust_core::carry_on::display_name(runtime_id).to_owned();
    let model = mate
        .model
        .as_deref()
        .filter(|m| !m.is_empty())
        .unwrap_or("account-default");
    let mode_label = locust_core::mode_facts::fact(&mate.runs_in).chip;
    let model_line = format!("{runtime_display} / {model} · {mode_label}");
    let name = mate.name.clone();

    let result = s
        .data
        .as_ref()
        .ok_or_else(|| "This teammate’s setup could not be read. Nothing was changed.".to_owned())
        .and_then(|store| {
            let events = last_turn_events(s, teammate_id);
            read_teammate_setup(
                store,
                None,
                &home_dir(),
                &project_dir(s),
                teammate_id,
                &events,
            )
        });

    s.setup_card = Some(match result {
        Ok(setup) => SetupCard {
            teammate_id: teammate_id.to_owned(),
            name,
            runtime_display,
            model_line,
            setup: Some(setup),
            problem: None,
        },
        Err(problem) => SetupCard {
            teammate_id: teammate_id.to_owned(),
            name,
            runtime_display,
            model_line,
            setup: None,
            problem: Some(problem),
        },
    });
}

pub(super) fn open_from_composer(s: &mut State) {
    let Some((teammate, _)) = selected_conversation(s) else {
        s.composer_note = Some("Open a teammate’s conversation to see its setup.".into());
        return;
    };
    if teammate.id == LEDGER_TARGET {
        s.composer_note = Some("Open a teammate’s conversation to see its setup.".into());
        return;
    }
    let id = teammate.id.clone();
    open_for_teammate(s, &id);
}

pub(super) fn view(s: &State, p: &Palette) -> Option<El> {
    let card = s.setup_card.as_ref()?;
    let l = crate::look::controls::Look::new(p.dark);
    let mut body = col()
        .w_full()
        .gap(12.0)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(text("Model").font(type_::LABEL).color(p.muted).w(150.0))
                .child(
                    text(card.model_line.clone())
                        .font(type_::BODY)
                        .color(p.text)
                        .grow(1.0)
                        .wrap(),
                ),
        );

    if let Some(problem) = &card.problem {
        body = body.child(
            text(problem.clone())
                .font(type_::LABEL)
                .color(p.red)
                .wrap(),
        );
    } else if let Some(setup) = &card.setup {
        for section in setup_sections(setup, &card.name, &card.runtime_display) {
            let mut sec = col()
                .w_full()
                .gap(6.0)
                .pt(8.0)
                .border_sides([1.0, 0.0, 0.0, 0.0], l.c("border-hairline"))
                .child(
                    text(section.title)
                        .font(rune::TextStyle::new(11.5, 600.0).mono())
                        .color(p.muted),
                )
                .child(
                    text(section.note)
                        .font(type_::LABEL)
                        .color(p.muted)
                        .wrap(),
                );
            for r in section.rows {
                sec = sec.child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .child(text(r.label).font(type_::LABEL).color(p.secondary).w(150.0))
                        .child(
                            text(r.value)
                                .font(type_::LABEL)
                                .color(p.text)
                                .grow(1.0)
                                .wrap(),
                        ),
                );
            }
            body = body.child(sec);
        }
    } else {
        body = body.child(
            text(format!("Reading what shapes {}’s turns…", card.name))
                .font(type_::LABEL)
                .color(p.muted)
                .wrap(),
        );
    }

    let edit_id = card.teammate_id.clone();
    Some(
        col()
            .key("setup-card")
            .w(380.0)
            .h_full()
            .gap(12.0)
            .padding(18.0)
            .fill(l.c("bg-raised"))
            .border_sides([0.0, 0.0, 0.0, 1.0], l.c("border-hairline"))
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(text("Setup").font(type_::TITLE).color(p.text))
                    .child(
                        text(card.name.clone())
                            .font(type_::LABEL)
                            .color(p.muted)
                            .grow(1.0),
                    )
                    .child(
                        text("Close")
                            .font(type_::LABEL)
                            .color(p.secondary)
                            .padding(6.0)
                            .key("setup-close")
                            .on_press(|s: &mut State| s.setup_card = None),
                    ),
            )
            .child(
                body.grow(1.0)
                    .min_h(0.0)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    }),
            )
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .pt(4.0)
                    .child(
                        text("Close")
                            .font(type_::LABEL)
                            .color(p.secondary)
                            .padding(8.0)
                            .key("setup-close-foot")
                            .on_press(|s: &mut State| s.setup_card = None),
                    )
                    .child(
                        text("Edit teammate")
                            .font(type_::LABEL)
                            .color(p.text)
                            .padding(8.0)
                            .radius(6.0)
                            .fill(p.selected)
                            .key("setup-edit")
                            .on_press(move |s: &mut State| {
                                s.setup_card = None;
                                crate::people_panel::open_edit(s, &edit_id);
                            }),
                    ),
            ),
    )
}
