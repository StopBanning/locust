//! Settings › Privacy & data › Trash (Electron `Screens.tsx` `TrashControl`).

use super::*;
use locust_core::trash::{self, TrashedMission};

#[derive(Clone, Debug, Default)]
pub(crate) struct TrashUiState {
    pub held: Option<Result<Vec<TrashedMission>, String>>,
    pub armed: bool,
    pub message: Option<String>,
}

pub(super) fn reload(s: &mut State) {
    let Some(store) = s.data.clone() else {
        s.settings_panel.privacy.trash.held = None;
        return;
    };
    let result = store
        .ledger_dir()
        .map_err(|_| trash::ERR_READ.to_string())
        .and_then(|dir| trash::list_trashed_missions(&dir));
    if result.as_ref().is_ok_and(|list| list.is_empty()) {
        s.settings_panel.privacy.trash.armed = false;
    }
    s.settings_panel.privacy.trash.held = Some(result);
}

fn restore_one(s: &mut State, mission_id: &str) {
    s.settings_panel.privacy.trash.message = None;
    let Some(store) = s.data.clone() else {
        s.settings_panel.privacy.trash.message = Some(trash::ERR_RESTORE.into());
        return;
    };
    let Ok(dir) = store.ledger_dir() else {
        s.settings_panel.privacy.trash.message = Some(trash::ERR_RESTORE.into());
        return;
    };
    match trash::restore_mission_or_err(&dir, mission_id) {
        Ok(()) => {
            s.settings_panel.privacy.trash.message = Some("Put back.".into());
            crate::backup_section::reload(s);
            load_team(s);
        }
        Err(msg) => {
            s.settings_panel.privacy.trash.message = Some(msg);
            reload(s);
        }
    }
}

fn empty_all(s: &mut State) {
    if !s.settings_panel.privacy.trash.armed {
        s.settings_panel.privacy.trash.armed = true;
        return;
    }
    s.settings_panel.privacy.trash.armed = false;
    s.settings_panel.privacy.trash.message = None;
    let Some(store) = s.data.clone() else {
        s.settings_panel.privacy.trash.message = Some(trash::ERR_EMPTY.into());
        return;
    };
    match trash::empty_trash(&store) {
        Ok(gone) => {
            s.settings_panel.privacy.trash.message = Some(format!("Deleted {gone} for good."));
            crate::backup_section::reload(s);
            load_team(s);
        }
        Err(msg) => {
            s.settings_panel.privacy.trash.message = Some(msg);
            reload(s);
        }
    }
}

fn format_deleted_at(iso: &str) -> String {
    if iso.len() >= 16 {
        iso[..16].replace('T', " ")
    } else {
        iso.to_owned()
    }
}

fn mission_label(entry: &TrashedMission) -> String {
    match entry.prompt.as_deref() {
        Some(prompt) if !prompt.trim().is_empty() => {
            prompt.lines().next().unwrap_or(&entry.mission_id).to_owned()
        }
        _ => entry.mission_id.clone(),
    }
}

pub(super) fn section(s: &State, p: &Palette) -> El {
    let state = &s.settings_panel.privacy.trash;
    let mut col_box = col()
        .key("trash-section")
        .w_full()
        .gap(8.0)
        .padding(10.0)
        .child(text("Trash").font(type_::TITLE).color(p.text));

    let note = match &state.held {
        None => "Reading the trash…".to_owned(),
        Some(Err(err)) => err.clone(),
        Some(Ok(list)) if list.is_empty() => {
            "Nothing deleted. A conversation you delete waits here until you empty it.".to_owned()
        }
        Some(Ok(list)) => {
            let count = list.len();
            let suffix = if count == 1 { "" } else { "s" };
            format!("{count} deleted conversation{suffix}, kept until you empty this.")
        }
    };
    col_box = col_box.child(text(note).font(type_::LABEL).color(p.secondary).wrap());

    if let Some(Ok(list)) = &state.held
        && !list.is_empty()
    {
        let mut rows = col().w_full().gap(6.0);
        for entry in list {
            let id = entry.mission_id.clone();
            let what = mission_label(entry);
            let when = format_deleted_at(&entry.deleted_at);
            rows = rows.child(
                row()
                    .key(format!("trash-row-{id}"))
                    .w_full()
                    .center_y()
                    .gap(8.0)
                    .padding(6.0)
                    .fill(p.field)
                    .radius(6.0)
                    .child(
                        text(what)
                            .font(type_::LABEL)
                            .color(p.text)
                            .grow(1.0)
                            .shrink()
                            .wrap_anywhere(),
                    )
                    .child(text(when).font(type_::LABEL).color(p.muted))
                    .child(
                        text("Put back")
                            .key(format!("trash-restore-{id}"))
                            .font(type_::LABEL)
                            .color(p.text)
                            .px(10.0)
                            .py(6.0)
                            .fill(p.raised)
                            .radius(6.0)
                            .on_press(move |s: &mut State| restore_one(s, &id)),
                    ),
            );
        }
        col_box = col_box.child(rows);

        let count = list.len();
        let empty_label = if state.armed {
            format!("Delete {count} for good?")
        } else {
            "Empty the trash".to_owned()
        };
        let empty_key = if state.armed {
            "trash-empty-confirm"
        } else {
            "trash-empty"
        };
        col_box = col_box.child(
            row().gap(8.0).child(
                text(empty_label)
                    .key(empty_key)
                    .font(type_::LABEL)
                    .color(p.red)
                    .px(10.0)
                    .py(6.0)
                    .fill(p.field)
                    .radius(6.0)
                    .on_press(|s: &mut State| empty_all(s)),
            ),
        );
    }

    if let Some(msg) = &state.message {
        col_box = col_box.child(
            text(msg.clone())
                .key("trash-message")
                .font(type_::LABEL)
                .color(p.secondary)
                .wrap(),
        );
    }

    col_box
}
