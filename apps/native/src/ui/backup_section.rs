//! Privacy & data settings sections: storage report, retention, and backup/restore
//! (Electron `Screens.tsx` `RetentionControl` + `BackupControl`; PARITY row 115).

use super::*;
use chrono::{Local, Utc};
use locust_core::engine_api;
use locust_core::profile_backup::{self, BackupCounts, BackupReading};
use locust_core::retention::{self, PruneResult, StorageReport};
use std::path::PathBuf;

const RETENTION_CHOICES: [i64; 3] = [30, 90, 365];

#[derive(Clone, Debug, Default)]
pub(crate) enum RetentionUiState {
    #[default]
    Idle,
    Preview {
        days: i64,
        summary: String,
        ids: Vec<String>,
    },
    Done(String),
    Error(String),
}

#[derive(Clone, Debug, Default)]
pub(crate) enum BackupUiState {
    #[default]
    Idle,
    BackedUp(String),
    Preview {
        reading: BackupReading,
        current: BackupCounts,
    },
    Restarting,
    Error(String),
}

#[derive(Clone, Debug)]
pub(crate) struct PrivacyPanelState {
    pub storage: Option<Result<StorageReport, String>>,
    pub retention_days: i64,
    pub retention: RetentionUiState,
    pub backup: BackupUiState,
    pub trash: crate::trash_section::TrashUiState,
}

impl Default for PrivacyPanelState {
    fn default() -> Self {
        Self {
            storage: None,
            retention_days: 90,
            retention: RetentionUiState::Idle,
            backup: BackupUiState::Idle,
            trash: crate::trash_section::TrashUiState::default(),
        }
    }
}

pub(super) fn restore_notice_banner(s: &State, p: &Palette) -> Option<El> {
    let outcome = s.last_restore.as_ref()?;
    let tone = if outcome.ok { p.green } else { p.amber };
    let line = profile_backup::restore_notice_line(outcome);
    Some(
        row()
            .key("restore-notice")
            .w_full()
            .center_y()
            .gap(8.0)
            .px(16.0)
            .py(8.0)
            .fill(p.raised)
            .border(1.0, p.hairline)
            .child(
                text(line)
                    .font(type_::LABEL)
                    .color(tone)
                    .grow(1.0)
                    .shrink()
                    .wrap(),
            )
            .child(
                action_button("OK", p)
                    .key("restore-notice-ok")
                    .on_press(|s: &mut State| {
                        s.last_restore = None;
                    }),
            ),
    )
}

pub(super) fn reload(s: &mut State) {
    crate::trash_section::reload(s);
    let Some(store) = s.data.clone() else {
        s.settings_panel.privacy.storage = None;
        return;
    };
    s.settings_panel.privacy.storage = Some(
        store
            .ledger_dir()
            .map_err(|e| e.to_string())
            .and_then(|dir| retention::read_storage_report(&dir)),
    );
}

fn chip(label: &str, on: bool, p: &Palette) -> El {
    text(label.to_owned())
        .font(type_::LABEL)
        .color(if on { p.text } else { p.secondary })
        .px(10.0)
        .py(6.0)
        .fill(if on { p.selected } else { p.field })
        .radius(6.0)
}

fn action_button(label: &str, p: &Palette) -> El {
    text(label.to_owned())
        .font(type_::LABEL)
        .color(p.text)
        .px(10.0)
        .py(6.0)
        .fill(p.field)
        .radius(6.0)
}

fn danger_button(label: &str, p: &Palette) -> El {
    text(label.to_owned())
        .font(type_::LABEL)
        .color(p.red)
        .px(10.0)
        .py(6.0)
        .fill(p.field)
        .radius(6.0)
}

fn preview_retention(s: &mut State) {
    let Some(store) = s.data.clone() else {
        s.settings_panel.privacy.retention =
            RetentionUiState::Error("Native data folder could not be determined.".into());
        return;
    };
    let Ok(dir) = store.ledger_dir() else {
        s.settings_panel.privacy.retention =
            RetentionUiState::Error("The local mission history could not be measured.".into());
        return;
    };
    let days = s.settings_panel.privacy.retention_days;
    let running = engine_api::api().running();
    match retention::prune_missions(&dir, Some(days as f64), true, None, &running, Utc::now()) {
        Ok(result) => {
            let summary = retention::prune_preview_summary(&result);
            s.settings_panel.privacy.retention = RetentionUiState::Preview {
                days,
                summary,
                ids: result.deleted,
            };
        }
        Err(msg) => {
            s.settings_panel.privacy.retention = RetentionUiState::Error(msg);
        }
    }
}

fn confirm_retention(s: &mut State) {
    let RetentionUiState::Preview { days, ids, .. } = &s.settings_panel.privacy.retention else {
        return;
    };
    let days = *days;
    let ids = ids.clone();
    let Some(store) = s.data.clone() else {
        s.settings_panel.privacy.retention =
            RetentionUiState::Error("Native data folder could not be determined.".into());
        return;
    };
    let Ok(dir) = store.ledger_dir() else {
        s.settings_panel.privacy.retention =
            RetentionUiState::Error("The missions could not be pruned.".into());
        return;
    };
    let running = engine_api::api().running();
    match retention::prune_missions(
        &dir,
        Some(days as f64),
        false,
        Some(&ids),
        &running,
        Utc::now(),
    ) {
        Ok(PruneResult { deleted, .. }) => {
            let gone = deleted.len();
            s.settings_panel.privacy.retention = RetentionUiState::Done(format!(
                "Deleted {gone} turn{}.",
                if gone == 1 { "" } else { "s" }
            ));
            reload(s);
        }
        Err(msg) => {
            s.settings_panel.privacy.retention = RetentionUiState::Error(msg);
        }
    }
}

fn pick_backup_destination(tx: &mut Tx) {
    let options = rune::FileDialogOptions {
        title: "Choose a folder to back up to".into(),
        ..Default::default()
    };
    tx.open_folder(options, |s: &mut State, answer, _tx| {
        let Ok(Some(parent)) = answer else { return };
        perform_backup(s, &parent);
    });
}

pub(crate) fn perform_backup(s: &mut State, parent: &std::path::Path) {
    let Some(store) = s.data.clone() else {
        s.settings_panel.privacy.backup =
            BackupUiState::Error("Native data folder could not be determined.".into());
        return;
    };
    match profile_backup::write_backup(
        store.root(),
        parent,
        env!("CARGO_PKG_VERSION"),
        Local::now(),
    ) {
        Ok(written) => {
            let size = profile_backup::format_bytes(written.bytes);
            let line = profile_backup::backed_up_line(
                &written.folder.to_string_lossy(),
                &written.counts,
                &size,
            );
            s.settings_panel.privacy.backup = BackupUiState::BackedUp(line);
        }
        Err(reason) => {
            s.settings_panel.privacy.backup = BackupUiState::Error(reason);
        }
    }
}

fn pick_restore_source(tx: &mut Tx) {
    let options = rune::FileDialogOptions {
        title: "Choose a backup folder to restore from".into(),
        ..Default::default()
    };
    tx.open_folder(options, |s: &mut State, answer, _tx| {
        let Ok(Some(folder)) = answer else { return };
        preview_restore(s, &folder);
    });
}

pub(crate) fn preview_restore(s: &mut State, folder: &std::path::Path) {
    let Some(store) = s.data.clone() else {
        s.settings_panel.privacy.backup =
            BackupUiState::Error("Native data folder could not be determined.".into());
        return;
    };
    match profile_backup::read_backup(folder, env!("CARGO_PKG_VERSION")) {
        Ok(reading) => {
            let current = profile_backup::count_profile(store.root());
            s.settings_panel.privacy.backup = BackupUiState::Preview { reading, current };
        }
        Err(reason) => {
            s.settings_panel.privacy.backup = BackupUiState::Error(reason);
        }
    }
}

fn relaunch_for_restore() {
    #[cfg(not(test))]
    if !crate::HEADLESS.load(std::sync::atomic::Ordering::Relaxed) {
        // The engine has the stores in memory: it writes every turn's ending and goes, so the restore is applied by the
        // next start with nothing holding the old state.
        let engine = locust_core::engine_api::api();
        engine.shut_down();
        engine.wait_for_shutdown();
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
        }
        std::process::exit(0);
    }
}

pub(crate) fn confirm_restore(s: &mut State) {
    let folder: PathBuf = match &s.settings_panel.privacy.backup {
        BackupUiState::Preview { reading, .. } => reading.folder.clone(),
        _ => return,
    };
    let Some(store) = s.data.clone() else {
        s.settings_panel.privacy.backup =
            BackupUiState::Error("Native data folder could not be determined.".into());
        return;
    };
    let live_runs = engine_api::api().running().len();
    match profile_backup::request_restore(
        &folder,
        store.root(),
        env!("CARGO_PKG_VERSION"),
        live_runs,
        Utc::now(),
    ) {
        Ok(_) => {
            s.settings_panel.privacy.backup = BackupUiState::Restarting;
            relaunch_for_restore();
        }
        Err(reason) => {
            s.settings_panel.privacy.backup = BackupUiState::Error(reason);
        }
    }
}

pub(super) fn storage_and_retention_section(s: &State, p: &Palette) -> El {
    let priv_state = &s.settings_panel.privacy;
    let mut section = col()
        .key("privacy-storage-section")
        .w_full()
        .gap(8.0)
        .padding(10.0)
        .child(text("Privacy & local data").font(type_::TITLE).color(p.text))
        .child(
            text("Nothing here is ever deleted on a timer. Turns go when you ask, after you have been shown exactly what would go, and a turn an ongoing conversation continues from is kept even when it is old.")
                .font(type_::LABEL)
                .color(p.secondary)
                .wrap(),
        );

    let storage_line = match &priv_state.storage {
        None => "Measuring local history…".to_owned(),
        Some(Err(msg)) => msg.clone(),
        Some(Ok(r)) if r.mission_count == 0 => "No native conversations on disk yet.".to_owned(),
        Some(Ok(r)) => format!(
            "{} native conversation{} · {}",
            r.mission_count,
            if r.mission_count == 1 { "" } else { "s" },
            profile_backup::format_bytes(r.byte_total)
        ),
    };
    section = section.child(text(storage_line).font(type_::LABEL).color(p.muted));

    let mut row_el = row()
        .gap(6.0)
        .align(rune::Align::Center)
        .child(text("Delete finished turns older than").font(type_::LABEL).color(p.secondary));
    for choice in RETENTION_CHOICES {
        let label = if choice == 365 {
            "1 year".to_owned()
        } else {
            format!("{choice} days")
        };
        let selected = priv_state.retention_days == choice;
        row_el = row_el.child(
            chip(&label, selected, p)
                .key(format!("retention-days-{choice}"))
                .on_press(move |s: &mut State| {
                    s.settings_panel.privacy.retention_days = choice;
                    s.settings_panel.privacy.retention = RetentionUiState::Idle;
                }),
        );
    }
    row_el = row_el.child(
        action_button("Review", p)
            .key("retention-review")
            .on_press(|s: &mut State| preview_retention(s)),
    );
    section = section.child(row_el);

    match &priv_state.retention {
        RetentionUiState::Idle => {}
        RetentionUiState::Preview { summary, ids, .. } => {
            let mut plan = row()
                .key("retention-plan")
                .gap(8.0)
                .align(rune::Align::Center)
                .child(text(summary.clone()).font(type_::LABEL).color(p.secondary).wrap());
            if !ids.is_empty() {
                plan = plan.child(
                    danger_button("Delete them for good", p)
                        .key("retention-confirm")
                        .on_press(|s: &mut State| confirm_retention(s)),
                );
            }
            section = section.child(plan);
        }
        RetentionUiState::Done(msg) => {
            section = section.child(
                text(msg.clone())
                    .key("retention-done")
                    .font(type_::LABEL)
                    .color(p.secondary),
            );
        }
        RetentionUiState::Error(msg) => {
            section = section.child(
                text(msg.clone())
                    .key("retention-error")
                    .font(type_::LABEL)
                    .color(p.red),
            );
        }
    }

    section
}

pub(super) fn backup_section(s: &State, p: &Palette) -> El {
    let priv_state = &s.settings_panel.privacy;
    let mut section = col()
        .key("privacy-backup-section")
        .w_full()
        .gap(8.0)
        .padding(10.0)
        .child(text("Back up and restore").font(type_::TITLE).color(p.text))
        .child(
            text("A backup is a folder you choose, holding your teammates, routines, memories, saved approvals and conversations. Your own model keys are never in it.")
                .font(type_::LABEL)
                .color(p.secondary)
                .wrap(),
        );

    let busy = matches!(priv_state.backup, BackupUiState::Restarting);
    let mut buttons = row().gap(8.0);
    let back_up_btn = action_button("Back up…", p).key("backup-choose");
    let restore_btn = action_button("Restore from a backup…", p).key("backup-restore-choose");
    if !busy {
        buttons = buttons
            .child(back_up_btn.on_press_tx(|_s: &mut State, tx| pick_backup_destination(tx)))
            .child(restore_btn.on_press_tx(|_s: &mut State, tx| pick_restore_source(tx)));
    } else {
        buttons = buttons.child(back_up_btn).child(restore_btn);
    }
    section = section.child(buttons);

    match &priv_state.backup {
        BackupUiState::Idle => {}
        BackupUiState::BackedUp(line) => {
            section = section.child(
                text(line.clone())
                    .key("backup-done")
                    .font(type_::LABEL)
                    .color(p.secondary)
                    .wrap(),
            );
        }
        BackupUiState::Preview { reading, current } => {
            let folder_label =
                profile_backup::folder_name(&reading.folder.to_string_lossy());
            let holds_line = format!(
                "{} · {}",
                profile_backup::counts_line(&reading.manifest.counts),
                profile_backup::format_bytes(reading.bytes)
            );
            let replaces_line = format!("{}, here now", profile_backup::counts_line(current));
            let made_line = format!(
                "{} by Locust {}",
                reading.manifest.created_at, reading.manifest.app_version
            );
            section = section.child(
                col()
                    .key("backup-preview")
                    .w_full()
                    .gap(6.0)
                    .padding(10.0)
                    .fill(p.field)
                    .radius(8.0)
                    .child(text(format!("Backup: {folder_label}")).font(type_::LABEL).color(p.text))
                    .child(text(format!("Made: {made_line}")).font(type_::LABEL).color(p.secondary))
                    .child(text(format!("Holds: {holds_line}")).font(type_::LABEL).color(p.secondary))
                    .child(text(format!("Replaces: {replaces_line}")).font(type_::LABEL).color(p.secondary))
                    .child(
                        text("Locust restarts to restore it. What is here now is moved aside in its profile folder, not deleted, so this can be undone. Your own model keys are never in a backup, and stay as they are.")
                            .font(type_::LABEL)
                            .color(p.muted)
                            .wrap(),
                    )
                    .child(
                        row()
                            .gap(8.0)
                            .child(
                                danger_button("Restore and restart", p)
                                    .key("backup-restore-confirm")
                                    .on_press(|s: &mut State| confirm_restore(s)),
                            )
                            .child(
                                action_button("Cancel", p)
                                    .key("backup-restore-cancel")
                                    .on_press(|s: &mut State| {
                                        s.settings_panel.privacy.backup = BackupUiState::Idle;
                                    }),
                            ),
                    ),
            );
        }
        BackupUiState::Restarting => {
            section = section.child(
                text("Restarting to restore…")
                    .key("backup-restarting")
                    .font(type_::LABEL)
                    .color(p.secondary),
            );
        }
        BackupUiState::Error(msg) => {
            section = section.child(
                text(msg.clone())
                    .key("backup-error")
                    .font(type_::LABEL)
                    .color(p.red)
                    .wrap(),
            );
        }
    }

    section
}
