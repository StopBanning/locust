//! Quit guard modal and Startup settings (`main/quit-guard.ts`, `main/login-item.ts`, `main/keep-running.ts`).

use locust_core::keep_awake;
use locust_core::login_item::{LoginItem, UnwiredLoginItemHost};
use locust_core::quit_guard::{self, CLOSE_BUTTONS, CloseMoment};
use rune::{El, Tx, col, el, row, text};

use crate::look::controls::{Look, button, ghost_button, primary_button, switch, Tone};
use crate::{Palette, State, type_};

#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    pub message: String,
    pub detail: String,
    pub confirmed: bool,
    pub kept_in_background: bool,
    pub quit_requested: bool,
}

fn live_runs_and_names(s: &State) -> (usize, Vec<String>) {
    let mut names: Vec<String> = s
        .teammates
        .iter()
        .filter(|t| t.busy)
        .map(|t| t.name.clone())
        .collect();
    if names.is_empty() && s.flight.is_some() {
        names.push(
            s.teammates
                .get(s.selected)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| "A teammate".into()),
        );
    }
    let count = names.len();
    (count, names)
}

pub(super) fn request_close(s: &mut State, tx: &mut Tx) {
    let (live_runs, names) = live_runs_and_names(s);
    let moment = CloseMoment {
        live_runs,
        app_quitting: false,
        session_ending: false,
        confirmed: s.quit_guard.confirmed,
    };
    if quit_guard::should_ask_before_closing(&moment) {
        let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let q = quit_guard::close_question(&name_refs, live_runs);
        s.quit_guard.open = true;
        s.quit_guard.message = q.message;
        s.quit_guard.detail = q.detail;
        return;
    }
    if quit_guard::hide_to_tray_on_close(&moment, s.settings_panel.keep_running) {
        s.quit_guard.kept_in_background = true;
        tx.close_window();
        return;
    }
    s.quit_guard.quit_requested = true;
    tx.quit();
}

pub(super) fn view(s: &State, p: &Palette) -> El {
    let l = Look::new(p.dark);
    col()
        .w(480.0)
        .gap(12.0)
        .padding(16.0)
        .fill(p.raised)
        .radius(10.0)
        .child(
            text(s.quit_guard.message.clone())
                .font(type_::TITLE)
                .color(p.text),
        )
        .child(
            text(s.quit_guard.detail.clone())
                .font(type_::LABEL)
                .color(p.secondary)
                .wrap(),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(
                    primary_button(&l, CLOSE_BUTTONS[0], false)
                        .key("quit-guard-keep")
                        .on_press_tx(|s: &mut State, tx| {
                            s.quit_guard.open = false;
                            s.quit_guard.kept_in_background = true;
                            tx.close_window();
                        }),
                )
                .child(el().grow(1.0))
                .child(
                    ghost_button(&l, CLOSE_BUTTONS[1])
                        .key("quit-guard-quit")
                        .on_press_tx(|s: &mut State, tx| {
                            s.quit_guard.open = false;
                            s.quit_guard.confirmed = true;
                            s.quit_guard.quit_requested = true;
                            tx.quit();
                        }),
                )
                .child(
                    button(&l, CLOSE_BUTTONS[2], Tone::Neutral, false)
                        .key("quit-guard-cancel")
                        .on_press(|s: &mut State| {
                            s.quit_guard.open = false;
                        }),
                ),
        )
}

pub(super) fn startup_section(s: &State, p: &Palette) -> El {
    let l = Look::new(p.dark);
    let login = &s.settings_panel.login_item;
    let login_note = login.note().unwrap_or(
        "Start Locust when you sign in, so routines and background work are ready.",
    );

    col()
        .w_full()
        .gap(8.0)
        .padding(10.0)
        .fill(p.field)
        .radius(6.0)
        .child(text("Startup".to_owned()).font(type_::TITLE).color(p.text))
        .child(
            row()
                .w_full()
                .gap(12.0)
                .child(
                    col()
                        .grow(1.0)
                        .gap(2.0)
                        .child(
                            text("Start Locust when you sign in".to_owned())
                                .font(type_::LABEL)
                                .color(p.text),
                        )
                        .child(
                            text(login_note.to_owned())
                                .font(type_::LABEL)
                                .color(p.secondary)
                                .wrap(),
                        ),
                )
                .child(
                    switch(&l, login.open_at_login, "Start Locust when you sign in", !login.available)
                        .key("settings-login-item")
                        .on_press(|s: &mut State| {
                            if !s.settings_panel.login_item.available {
                                return;
                            }
                            let next = !s.settings_panel.login_item.open_at_login;
                            let mut item = LoginItem::new(
                                UnwiredLoginItemHost::default(),
                                false,
                                std::env::current_exe()
                                    .map(|p| p.display().to_string())
                                    .unwrap_or_default(),
                                match std::env::consts::OS {
                                    "macos" => "darwin",
                                    "windows" => "win32",
                                    other => other,
                                },
                            );
                            let updated = item.set(next);
                            if updated.available {
                                s.settings_panel.login_item = updated;
                            }
                        }),
                ),
        )
        .child(
            row()
                .w_full()
                .gap(12.0)
                .child(
                    col()
                        .grow(1.0)
                        .gap(2.0)
                        .child(
                            text("Keep Locust running in the background".to_owned())
                                .font(type_::LABEL)
                                .color(p.text),
                        )
                        .child(
                            text("Closing the window keeps routines and running teammates alive in the tray.".to_owned())
                                .font(type_::LABEL)
                                .color(p.secondary)
                                .wrap(),
                        ),
                )
                .child(
                    switch(
                        &l,
                        s.settings_panel.keep_running,
                        "Keep Locust running in the background",
                        false,
                    )
                    .key("settings-keep-running")
                    .on_press(|s: &mut State| {
                        let next = !s.settings_panel.keep_running;
                        s.settings_panel.keep_running = next;
                        if let Some(store) = &s.data {
                            let _ = keep_awake::write_keep_running(
                                &store.root().join(keep_awake::BACKGROUND_FILE),
                                next,
                            );
                        }
                    }),
                ),
        )
        .child(
            row()
                .w_full()
                .gap(12.0)
                .child(
                    col()
                        .grow(1.0)
                        .gap(2.0)
                        .child(
                            text("Keep this computer awake while it is plugged in".to_owned())
                                .font(type_::LABEL)
                                .color(p.text),
                        )
                        .child(
                            text("Prevents idle sleep on AC power so scheduled routines and Scout run on time.".to_owned())
                                .font(type_::LABEL)
                                .color(p.secondary)
                                .wrap(),
                        ),
                )
                .child(
                    switch(
                        &l,
                        s.settings_panel.stay_awake,
                        "Keep this computer awake while it is plugged in",
                        false,
                    )
                    .key("settings-stay-awake")
                    .on_press(|s: &mut State| {
                        let next = !s.settings_panel.stay_awake;
                        s.settings_panel.stay_awake = next;
                        if let Some(store) = &s.data {
                            let _ = keep_awake::write_stay_awake(
                                &store.root().join(keep_awake::AWAKE_FILE),
                                next,
                            );
                        }
                    }),
                ),
        )
}
