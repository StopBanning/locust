//! Import a conversation dialog (`apps/desktop/src/renderer/src/components/ImportDialog.tsx`).

use chrono::{DateTime, Utc};
use locust_core::session_import::{
    self, ImportRuntime, ImportSuccess, ImportableSession, OpenCodeRunner, SystemOpenCodeRunner,
};
use rune::{El, ScrollSpec, TextEdit, col, row, text, text_input};

use crate::look::controls::{Look, ghost_button, glyph, primary_button};
use crate::look::icons;
use crate::{Latest, Palette, State, refresh_recent, type_};

pub(super) struct Panel {
    pub open: bool,
    pub sessions: Option<Vec<ImportableSession>>,
    pub problem: Option<String>,
    pub importing: Option<String>,
    pub query: TextEdit,
    pub open_after_import: Option<String>,
    pub list_job: Latest<(Vec<ImportableSession>, Vec<String>)>,
    pub import_job: Latest<Result<ImportSuccess, String>>,
    pub opencode_enabled: bool,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            open: false,
            sessions: None,
            problem: None,
            importing: None,
            query: TextEdit::new(""),
            open_after_import: None,
            list_job: Latest::default(),
            import_job: Latest::default(),
            opencode_enabled: false,
        }
    }
}

struct NoOpOpenCodeRunner;

impl OpenCodeRunner for NoOpOpenCodeRunner {
    fn run_opencode(&self, _args: &[&str]) -> Result<Option<String>, String> {
        Ok(None)
    }
}

pub(super) fn open(s: &mut State) {
    s.add_menu_open = false;
    s.import_dialog.open = true;
    s.import_dialog.sessions = None;
    s.import_dialog.problem = None;
    s.import_dialog.importing = None;
    s.import_dialog.query.clear();
    s.import_dialog.query.focused = true;

    let (Some(store), Some(places)) = (s.data.clone(), s.session_places.clone()) else {
        s.import_dialog.sessions = Some(Vec::new());
        return;
    };
    let opencode_enabled = s.import_dialog.opencode_enabled;
    s.import_dialog.list_job.request((), move |()| {
        if opencode_enabled {
            session_import::list_all_for_window(&store, &places, &SystemOpenCodeRunner, None)
        } else {
            session_import::list_all_for_window(&store, &places, &NoOpOpenCodeRunner, None)
        }
    });
}

pub(super) fn close(s: &mut State) {
    s.import_dialog.open = false;
    s.import_dialog.query.focused = false;
    s.import_dialog.list_job.cancel();
    s.import_dialog.import_job.cancel();
}

pub(super) fn start_import(s: &mut State, session: ImportableSession) {
    if s.import_dialog.importing.is_some() || session.folder_missing {
        return;
    }
    let (Some(store), Some(places)) = (s.data.clone(), s.session_places.clone()) else {
        s.import_dialog.problem = Some("It could not be brought in.".into());
        return;
    };
    s.import_dialog.importing = Some(session.session_id.clone());
    s.import_dialog.problem = None;
    s.import_dialog.import_job.request((), move |()| {
        session_import::import_session(&store, &places, &session, None)
    });
}

pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    if let Some((sessions, warnings)) = s.import_dialog.list_job.take() {
        changed = true;
        s.import_dialog.sessions = Some(sessions);
        if !warnings.is_empty() {
            s.import_dialog.problem = Some(warnings.join(" "));
        }
    }
    if let Some(result) = s.import_dialog.import_job.take() {
        changed = true;
        s.import_dialog.importing = None;
        match result {
            Ok(outcome) => {
                if let Some(store) = &s.data {
                    let _ = crate::org::rename_mission(
                        store,
                        &mut s.org,
                        &outcome.first_mission_id,
                        &outcome.title,
                    );
                }
                s.import_dialog.open = false;
                s.import_dialog.query.focused = false;
                s.import_dialog.open_after_import = Some(outcome.mission_id);
                refresh_recent(s);
            }
            Err(message) => {
                s.import_dialog.problem = Some(message);
            }
        }
    }
    changed
}

/// "3m", "5h", "2d": as the sidebar says ages.
pub(crate) fn age_of(iso: &str, now_ms: i64) -> String {
    let parsed_ms = DateTime::parse_from_rfc3339(iso)
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(now_ms);
    let minutes = (((now_ms - parsed_ms) as f64) / 60_000.0).round().max(0.0) as i64;
    if minutes < 60 {
        return format!("{}m", minutes.max(1));
    }
    let hours = ((minutes as f64) / 60.0).round() as i64;
    if hours < 48 {
        return format!("{hours}h");
    }
    let days = ((hours as f64) / 24.0).round() as i64;
    format!("{days}d")
}

fn runtime_display_name(rt: ImportRuntime) -> &'static str {
    rt.display_name()
}

pub(super) fn view(s: &State, p: &Palette) -> El {
    let l = Look::new(p.dark);
    let now_ms = Utc::now().timestamp_millis();
    let query = s.import_dialog.query.text.trim().to_lowercase();

    let mut card = col()
        .w(560.0)
        .padding(20.0)
        .gap(14.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .radius(12.0)
        .key("import-dialog")
        .child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(
                    text("Import a conversation")
                        .font(type_::TITLE)
                        .color(p.text),
                )
                .child(
                    text("the last 30 days")
                        .font(type_::LABEL)
                        .color(p.muted)
                        .grow(1.0),
                )
                .child(
                    glyph(&l, &icons::CLOSE, 18.0, Some(p.muted))
                        .padding(4.0)
                        .radius(6.0)
                        .key("import-dialog-x")
                        .on_press(close),
                ),
        )
        .child(
            text_input(&s.import_dialog.query, type_::BODY)
                .key("import-dialog-search")
                .placeholder("Search by title or folder", p.muted)
                .color(p.text)
                .w_full()
                .px(10.0)
                .py(7.0)
                .fill(p.bg)
                .border(1.0, p.hairline)
                .radius(8.0)
                .on_edit(|s: &mut State| &mut s.import_dialog.query),
        );

    if let Some(problem) = &s.import_dialog.problem {
        card = card.child(
            text(problem)
                .key("import-dialog-problem")
                .font(type_::LABEL)
                .color(p.red),
        );
    }

    match &s.import_dialog.sessions {
        None => {
            card = card.child(
                text("Looking for your Claude Code, Codex, OpenCode and Cursor Agent sessions…")
                    .key("import-dialog-loading")
                    .font(type_::BODY)
                    .color(p.muted),
            );
        }
        Some(sessions) => {
            let shown: Vec<&ImportableSession> = sessions
                .iter()
                .filter(|sess| {
                    query.is_empty()
                        || format!("{} {}", sess.title, sess.folder_name)
                            .to_lowercase()
                            .contains(&query)
                })
                .collect();
            if shown.is_empty() {
                let empty_msg = if sessions.is_empty() {
                    "No sessions from the last 30 days to bring in. Ones already in Locust are not listed."
                } else {
                    "Nothing matches that."
                };
                card = card.child(
                    text(empty_msg)
                        .key("import-dialog-empty")
                        .font(type_::BODY)
                        .color(p.muted),
                );
            } else {
                let mut list = col()
                    .key("import-dialog-list")
                    .w_full()
                    .max_h(340.0)
                    .gap(8.0)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    });
                for sess in shown {
                    let mut meta = format!(
                        "{} · {} · {}",
                        runtime_display_name(sess.runtime),
                        sess.folder_name,
                        age_of(&sess.updated_at, now_ms)
                    );
                    if sess.folder_missing {
                        meta.push_str(" · folder no longer available");
                    }
                    if sess.open_now {
                        meta.push_str(" · probably still open in a terminal");
                    }
                    let is_importing = s.import_dialog.importing.as_deref()
                        == Some(sess.session_id.as_str());
                    let disabled =
                        s.import_dialog.importing.is_some() || sess.folder_missing;
                    let btn_label = if is_importing {
                        "Importing…"
                    } else {
                        "Import"
                    };
                    let tip = if sess.folder_missing {
                        "Restore its folder and open this list again to continue this conversation."
                    } else if sess.open_now {
                        "It was written to in the last two minutes. Close it in the terminal first, or both will write to it."
                    } else {
                        "Bring it in. Your next message there continues this same session."
                    };
                    let session_clone = sess.clone();
                    let btn_key = format!("import-session-{}", sess.session_id);
                    let mut btn = primary_button(&l, btn_label, disabled)
                        .key(btn_key)
                        .help(tip);
                    if !disabled {
                        btn = btn.on_press(move |s: &mut State| {
                            start_import(s, session_clone.clone());
                        });
                    }
                    list = list.child(
                        row()
                            .w_full()
                            .center_y()
                            .gap(10.0)
                            .px(10.0)
                            .py(8.0)
                            .fill(p.bg)
                            .border(1.0, p.hairline)
                            .radius(8.0)
                            .child(
                                col()
                                    .grow(1.0)
                                    .min_w(0.0)
                                    .gap(2.0)
                                    .child(
                                        text(&sess.title)
                                            .font(type_::BODY)
                                            .color(p.text)
                                            .truncate(),
                                    )
                                    .child(
                                        text(meta)
                                            .font(type_::LABEL)
                                            .color(p.muted)
                                            .help(&sess.cwd)
                                            .truncate(),
                                    ),
                            )
                            .child(btn),
                    );
                }
                card = card.child(list);
            }
        }
    }

    card.child(
        row()
            .w_full()
            .center_y()
            .gap(12.0)
            .child(
                text("Only the words come in; the steps it took stay in the session. Your next message continues it in its own folder.")
                    .font(type_::LABEL)
                    .color(p.muted)
                    .grow(1.0),
            )
            .child(
                ghost_button(&l, "Close")
                    .key("import-dialog-close")
                    .on_press(close),
            ),
    )
}
