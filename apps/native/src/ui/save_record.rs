//! Save the record dialog (`apps/desktop/src/renderer/src/components/SaveRecordDialog.tsx`).

use std::path::PathBuf;

use locust_core::mission_export;
use rune::{El, FileDialogOptions, Tx, col, el, row, text};
use rune_kit::Checkbox;

use crate::look::controls::{Look, ghost_button, glyph, primary_button};
use crate::look::icons;
use crate::{Palette, State, type_};

const LOCUST_VERSION: &str = env!("CARGO_PKG_VERSION");
const RECORD_CLAIM: &str = "This file contains the conversation. Read it before you send it.";

#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    pub mission_id: String,
    pub include_raw: bool,
    pub busy: bool,
    pub said: Option<String>,
    pub error: bool,
}

pub(super) fn open(s: &mut State, mission_id: impl Into<String>) {
    s.save_record = Panel {
        open: true,
        mission_id: mission_id.into(),
        include_raw: false,
        busy: false,
        said: None,
        error: false,
    };
}

fn close(s: &mut State) {
    s.save_record.open = false;
}

fn downloads_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(|h| PathBuf::from(h).join("Downloads"))
        .filter(|p| p.is_dir())
}

fn start_save(s: &mut State, tx: &mut Tx) {
    if s.save_record.busy {
        return;
    }
    let mission_id = s.save_record.mission_id.clone();
    let store = s.data.clone();
    let prepared = match mission_export::prepare_record_for_mission(
        &mission_id,
        store.as_ref(),
        LOCUST_VERSION,
        chrono::Local::now(),
    ) {
        Ok(prepared) => prepared,
        Err(why) => {
            s.save_record.said = Some(why);
            s.save_record.error = true;
            return;
        }
    };

    s.save_record.busy = true;
    s.save_record.said = None;
    s.save_record.error = false;

    let options = FileDialogOptions {
        title: "Save the record".into(),
        directory: downloads_dir(),
        file_name: Some(prepared.suggested_file_name.clone()),
        ..Default::default()
    }
    .filter("Markdown", &["md"]);

    let include_raw = s.save_record.include_raw;
    tx.save_file(options, move |s: &mut State, answer, _tx| {
        s.save_record.busy = false;
        match answer {
            Ok(None) => {
                // Cancelled in the save dialog: nothing was written and the dialog stays open.
                s.save_record.said = None;
                s.save_record.error = false;
            }
            Err(_) => {
                s.save_record.said =
                    Some("The record could not be saved. Nothing was written.".into());
                s.save_record.error = true;
            }
            Ok(Some(path)) => match mission_export::write_record_files(
                &path,
                &prepared,
                include_raw,
                LOCUST_VERSION,
                store.as_ref(),
            ) {
                Ok((md_path, Some(raw_path))) => {
                    s.save_record.said = Some(format!(
                        "Saved to {}, and the raw record beside it at {}.",
                        md_path.display(),
                        raw_path.display()
                    ));
                    s.save_record.error = false;
                }
                Ok((md_path, None)) => {
                    s.save_record.said = Some(format!("Saved to {}.", md_path.display()));
                    s.save_record.error = false;
                }
                Err(why) => {
                    s.save_record.said = Some(if why.contains("Electron") || why.contains("Codex") {
                        format!("{why}. Nothing was written.")
                    } else {
                        "The record could not be saved. Nothing was written.".into()
                    });
                    s.save_record.error = true;
                }
            },
        }
    });
}

pub(super) fn view(s: &State, p: &Palette) -> El {
    let l = Look::new(p.dark);
    let save_label = if s.save_record.busy {
        "Saving…"
    } else {
        "Save the record…"
    };

    let mut card = col()
        .w(440.0)
        .padding(20.0)
        .gap(14.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .radius(12.0)
        .key("save-record-dialog")
        .child(
            row()
                .w_full()
                .center_y()
                .child(
                    text("Save the record")
                        .font(type_::TITLE)
                        .color(p.text)
                        .grow(1.0),
                )
                .child(
                    glyph(&l, &icons::CLOSE, 18.0, Some(p.muted))
                        .padding(4.0)
                        .radius(6.0)
                        .key("save-record-x")
                        .on_press(|s: &mut State| close(s)),
                ),
        )
        .child(
            text(RECORD_CLAIM)
                .font(type_::BODY)
                .color(p.secondary)
                .wrap()
                .key("save-record-claim"),
        )
        .child(
            Checkbox::new(s.save_record.include_raw)
                .label("Include the raw record (JSON, not scrubbed)")
                .on_toggle(|s: &mut State| {
                    s.save_record.include_raw = !s.save_record.include_raw;
                })
                .build(l.k)
                .key("save-record-include-raw"),
        );

    if let Some(said) = &s.save_record.said {
        card = card.child(
            text(said.clone())
                .font(type_::LABEL)
                .color(if s.save_record.error {
                    p.red
                } else {
                    p.secondary
                })
                .wrap()
                .key("save-record-status"),
        );
    }

    card.child(
        row()
            .w_full()
            .gap(8.0)
            .pt(4.0)
            .child(el().grow(1.0))
            .child(
                ghost_button(&l, "Close")
                    .key("save-record-close")
                    .on_press(|s: &mut State| close(s)),
            )
            .child(
                primary_button(&l, save_label, s.save_record.busy)
                    .key("save-record-submit")
                    .on_press_tx(|s: &mut State, tx: &mut Tx| start_save(s, tx)),
            ),
    )
}
