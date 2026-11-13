//! A TEAMMATE'S SETUP (`TeammateSetup.tsx`, `App.tsx` `openSetup`).
//!
//! `/setup` in its conversation, or Setup on its card in Team: reads what shapes its turns
//! fresh from disk, in the order a turn is given them. It changes nothing; Edit teammate is
//! one press away.

use super::*;
use locust_core::store::Store;
use locust_core::teammate_setup::{
    TeammateSetup, read_teammate_setup_with_group, setup_sections,
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

/// Tests never read the person's real home folder (its skill folders): they get an empty one of their own.
#[cfg(test)]
fn home_dir() -> std::path::PathBuf {
    let home = std::env::temp_dir().join(format!("locust-test-home-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&home);
    home
}

#[cfg(not(test))]
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

fn active_group_brief(s: &State, store: &Store, teammate_id: &str) -> Option<(String, String)> {
    let listed = locust_core::groups::load_groups(store).ok()?;
    let tip = s
        .ledger
        .as_ref()
        .filter(|_| s.teammates.get(s.selected).is_some_and(|t| t.id == teammate_id))
        .and_then(|pane| {
            (!pane.snapshot.tip.is_empty())
                .then_some(pane.snapshot.tip.as_str())
                .or(pane.conversation.conversation_id.as_deref())
        })
        .or_else(|| {
            s.conversations
                .get(teammate_id)
                .and_then(|c| c.conversation_id.as_deref())
        });
    let mut keys = locust_core::groups::keys_before(store, tip);
    if let Some(tip_id) = tip
        && let Some(row) = crate::sidebar::recent_as_missions(s)
            .into_iter()
            .find(|m| conversation_list::conversation_keys(m).iter().any(|k| k == tip_id))
    {
        for k in conversation_list::conversation_keys(&row) {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
    }
    locust_core::groups::group_brief_for(&keys, &listed)
        .map(|g| (g.group_name, g.instructions))
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
            let group = active_group_brief(s, store, teammate_id);
            read_teammate_setup_with_group(
                store,
                group,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TICK: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn open_for_teammate_includes_active_group_standing_instructions() {
        let base = std::env::temp_dir().join(format!(
            "locust-setup-group-{}-{}",
            std::process::id(),
            TICK.fetch_add(1, Ordering::Relaxed)
        ));
        let native_dir = base.join("native");
        let electron_dir = base.join("electron");
        std::fs::create_dir_all(&native_dir).unwrap();
        std::fs::create_dir_all(&electron_dir).unwrap();
        let store = Store::new(native_dir, electron_dir).unwrap();
        store
            .save_json(
                "teammates.json",
                &serde_json::json!({
                    "schemaVersion": 1,
                    "teammates": [{
                        "teammateId": "tm_codec",
                        "name": "Codec",
                        "role": "Engineer",
                        "createdAt": "2026-09-16T08:00:00.000Z"
                    }],
                    "missionOwners": {},
                    "settings": {}
                }),
            )
            .unwrap();

        let grp = locust_core::groups::create_group(&store, "Trading").unwrap();
        locust_core::groups::set_group_instructions(&store, &grp.group_id, "Check risk limits.")
            .unwrap();
        locust_core::groups::assign_to_group(&store, "m_root", Some(&grp.group_id)).unwrap();
        let ledger = store.ledger_dir().unwrap();
        std::fs::create_dir_all(&ledger).unwrap();
        std::fs::write(
            ledger.join("m_root.jsonl"),
            serde_json::json!({"missionId": "m_root"}).to_string() + "\n",
        )
        .unwrap();
        std::fs::write(
            ledger.join("m_follow.jsonl"),
            serde_json::json!({
                "missionId": "m_follow",
                "continuesFrom": {"missionId": "m_root"}
            })
            .to_string()
                + "\n",
        )
        .unwrap();

        let mut s = State::default();
        s.data = Some(store);
        s.teammates.push(Teammate {
            id: "tm_codec".into(),
            name: "Codec".into(),
            role: "Engineer".into(),
            runtime: Some("codex".into()),
            model: Some("gpt-5".into()),
            account: None,
            busy: false,
            runs_in: "local".into(),
            folder: None,
        });
        s.selected = 0;
        s.conversations.insert(
            "tm_codec".into(),
            Conversation {
                conversation_id: Some("m_follow".into()),
                turns: Vec::new(),
            },
        );

        open_for_teammate(&mut s, "tm_codec");
        let setup = s
            .setup_card
            .as_ref()
            .and_then(|c| c.setup.as_ref())
            .expect("setup read succeeds");
        assert_eq!(setup.group.as_ref().map(|g| g.name.as_str()), Some("Trading"));
        assert_eq!(
            setup.group.as_ref().map(|g| g.characters),
            Some("Check risk limits.".encode_utf16().count())
        );

        let mut rt = rune::Runtime::new(
            s,
            |s, _| {
                view(s, &Palette::new(true)).unwrap_or_else(|| col())
            },
            (800.0, 800.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let texts = rt
            .elements()
            .into_iter()
            .filter_map(|e| e.text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            texts.contains("Trading\u{2019}s instructions, 18 characters"),
            "texts: {texts}"
        );
        assert!(texts.find("LOCUST.md") < texts.find("Group"));
        assert!(texts.find("Group") < texts.find("Its instructions"));
        let _ = std::fs::remove_dir_all(&base);
    }
}
