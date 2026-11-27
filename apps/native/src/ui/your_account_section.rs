//! Settings › General › "Your account" (Electron `components/YourAccountSettings.tsx`, accounts slice 3/4/4b).
//!
//! Draws the sealed profile copy card from `locust_core::your_account::YourAccountStatus`: signed-out guide,
//! sync switch and status, one-time recovery code alert, recovery-code bring-here form, computer-to-computer
//! ask/approve flow, and Sync now / Delete my copy actions.

use super::*;
use chrono::{DateTime, Utc};
use locust_core::your_account::{
    AccountSyncState, BringHereResult, YourAccount, YourAccountStatus,
};
use std::sync::OnceLock;

pub(crate) const HEADING: &str = "Your account";
const SWITCH_LABEL: &str = "Keep a copy on Locust\u{2019}s relay";
/// What the switch said off, once a key is kept (YourAccountSettings.tsx:99), after its "Off.": under its More.
pub(crate) const SYNC_OFF: &str = "Your copy stays as it was; turning sync on again needs no new code.";

#[derive(Default)]
pub(crate) struct AccountPanelState {
    pub status: YourAccountStatus,
    pub code: Option<String>,
    pub recovery_input: TextEdit,
    pub bringing: Option<String>,
    pub failure: Option<String>,
    pub armed_delete: bool,
    pub armed_bring: bool,
    pub armed_approve: Option<String>,
}

static ACCOUNT: OnceLock<YourAccount> = OnceLock::new();

#[cfg(not(test))]
pub(crate) fn open() {
    use locust_core::your_account::YourAccountOptions;
    use std::sync::Arc;

    if ACCOUNT.get().is_some() {
        return;
    }
    let Some(store) = locust_core::native::store() else {
        return;
    };
    let root = store.root().to_path_buf();
    let restore_root = root.clone();
    let app_version = env!("CARGO_PKG_VERSION").to_owned();
    let restore_version = app_version.clone();
    let account = YourAccount::new(YourAccountOptions {
        profile: root,
        link: Arc::new(|| None),
        secrets: locust_core::own_models::secrets(),
        computer: Arc::new(|| {
            std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .unwrap_or_else(|_| "This computer".into())
        }),
        app_version,
        busy: Arc::new(|| false),
        remote: None,
        now: None,
        quiet_ms: None,
        copies_fresh_ms: None,
        ask_for_ms: None,
        changed: None,
        restore: Some(Arc::new(move |folder| {
            locust_core::profile_backup::request_restore(
                folder,
                &restore_root,
                &restore_version,
                0,
                Utc::now(),
            )
            .map(|_| ())
        })),
    });
    let _ = ACCOUNT.set(account);
}

#[cfg(test)]
pub(crate) fn open() {}

pub(crate) fn read(s: &mut State) {
    if let Some(account) = ACCOUNT.get() {
        s.settings_panel.account.status = account.status();
    }
}

/// Formats byte sizes as Electron's `YourAccountSettings.tsx` `sizeOf`: KB, MB (1 decimal), GB (2 decimals).
pub(crate) fn size_of(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * 1024 * 1024;
    if bytes < MIB {
        let kb = ((bytes as f64) / 1024.0).round() as u64;
        format!("{} KB", kb.max(1))
    } else if bytes < GIB {
        format!("{:.1} MB", (bytes as f64) / (MIB as f64))
    } else {
        format!("{:.2} GB", (bytes as f64) / (GIB as f64))
    }
}

/// Formats an ISO-8601 timestamp relative to `now_ms`, matching Electron's `agoOf`.
pub(crate) fn ago_of(at: &str, now_ms: i64) -> String {
    let Ok(parsed) = DateTime::parse_from_rfc3339(at) else {
        return "just now".into();
    };
    let diff_ms = (now_ms - parsed.timestamp_millis()).max(0);
    let seconds = ((diff_ms as f64) / 1000.0).round() as u64;
    if seconds < 60 {
        "just now".into()
    } else if seconds < 3600 {
        format!("{} min ago", ((seconds as f64) / 60.0).round() as u64)
    } else if seconds < 86_400 {
        format!("{} h ago", ((seconds as f64) / 3600.0).round() as u64)
    } else {
        parsed.format("%b %-d").to_string()
    }
}

fn toggle_sync(s: &mut State) {
    let on = s.settings_panel.account.status.sync == AccountSyncState::On;
    s.settings_panel.account.failure = None;
    if let Some(account) = ACCOUNT.get() {
        if on {
            s.settings_panel.account.status = account.turn_off();
        } else {
            let res = account.turn_on();
            s.settings_panel.account.status = res.status;
            if let Some(code) = res.code {
                s.settings_panel.account.code = Some(code);
            }
        }
    }
}

fn sync_now(s: &mut State) {
    s.settings_panel.account.failure = None;
    if let Some(account) = ACCOUNT.get() {
        s.settings_panel.account.status = account.sync_now();
    }
}

fn delete_copy(s: &mut State) {
    if !s.settings_panel.account.armed_delete {
        s.settings_panel.account.armed_delete = true;
        return;
    }
    s.settings_panel.account.armed_delete = false;
    s.settings_panel.account.failure = None;
    if let Some(account) = ACCOUNT.get() {
        s.settings_panel.account.status = account.delete_copy();
    }
}

fn bring_here(s: &mut State) {
    if !s.settings_panel.account.armed_bring {
        s.settings_panel.account.armed_bring = true;
        return;
    }
    s.settings_panel.account.armed_bring = false;
    s.settings_panel.account.failure = None;
    let typed = s.settings_panel.account.recovery_input.text.clone();
    if let Some(account) = ACCOUNT.get() {
        match account.bring_here(&typed) {
            BringHereResult::Ok { computer } => {
                s.settings_panel.account.bringing = Some(computer);
                s.settings_panel.account.recovery_input.replace_text(String::new());
            }
            BringHereResult::Err { reason } => {
                s.settings_panel.account.failure = Some(reason);
            }
        }
    }
}

fn ask_computer(s: &mut State) {
    s.settings_panel.account.failure = None;
    if let Some(account) = ACCOUNT.get() {
        s.settings_panel.account.status = account.ask();
    }
}

fn stop_asking(s: &mut State) {
    s.settings_panel.account.failure = None;
    if let Some(account) = ACCOUNT.get() {
        s.settings_panel.account.status = account.stop_asking();
    }
}

fn approve_ask(s: &mut State, room: String, match_code: String) {
    if s.settings_panel.account.armed_approve.as_deref() != Some(room.as_str()) {
        s.settings_panel.account.armed_approve = Some(room);
        return;
    }
    s.settings_panel.account.armed_approve = None;
    s.settings_panel.account.failure = None;
    if let Some(account) = ACCOUNT.get() {
        s.settings_panel.account.status = account.approve(&room, &match_code);
    }
}

fn ignore_ask(s: &mut State, room: String) {
    s.settings_panel.account.armed_approve = None;
    s.settings_panel.account.failure = None;
    if let Some(account) = ACCOUNT.get() {
        s.settings_panel.account.status = account.ignore(&room);
    }
}

fn boxed_button(label: impl Into<String>, l: &crate::look::controls::Look) -> El {
    text(label.into())
        .font(l.ty.control)
        .color(l.c("text-primary"))
        .py(6.0)
        .px(10.0)
        .radius(l.t.px("radius-control"))
        .fill(l.c("bg-control"))
        .role("button")
}

pub(crate) fn section(s: &State, p: &Palette) -> El {
    let now_ms = Utc::now().timestamp_millis();
    section_at(s, p, now_ms)
}

pub(crate) fn section_at(s: &State, p: &Palette, now_ms: i64) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let acct = &s.settings_panel.account;
    let status = &acct.status;

    let mut root = col()
        .key("your-account-section")
        .w_full()
        .gap(8.0)
        .child(
            text(HEADING)
                .role("heading")
                .level(2)
                .font(l.ty.control)
                .color(l.c("text-primary"))
                .px(10.0),
        );

    let Some(signed_in) = &status.signed_in else {
        root = root.child(
            col()
                .w_full()
                .gap(4.0)
                .padding(10.0)
                .child(
                    text("Sign in once, and your teammates, routines, memories and conversations follow you to any computer where you sign in.")
                        .font(l.ty.body)
                        .color(l.c("text-primary"))
                        .wrap(),
                )
                .child(
                    text("It is your GitHub sign-in. Turn on From anywhere under Your phone, then press Sign in with GitHub there. Locust works fully without one.")
                        .font(l.ty.body)
                        .color(l.c("text-secondary"))
                        .wrap(),
                ),
        );
        return append_footer(root, acct, &l);
    };

    let on = status.sync == AccountSyncState::On;
    let note = if on {
        let sync_part = if status.syncing {
            "Syncing now.".to_owned()
        } else if let Some(last) = &status.last_synced {
            format!(
                "Last synced {}: {} files, {}.",
                ago_of(&last.at, now_ms),
                last.files,
                size_of(last.bytes as u64)
            )
        } else {
            "Not synced yet.".to_owned()
        };
        let storage_part = status.storage.map_or_else(String::new, |st| {
            format!(" Using {} of {}.", size_of(st.used), size_of(st.quota))
        });
        format!("{sync_part}{storage_part}")
    } else if status.has_key {
        String::new()
    } else {
        "Each file is sealed on this computer before it leaves, with a key only your computers hold, so the relay cannot read it. Agent sign-ins and keys are never sent.".to_owned()
    };

    let switch = crate::look::controls::switch(&l, on, SWITCH_LABEL, false)
        .key("account-switch")
        .on_press(toggle_sync);

    root = root.child(
        row()
            .w_full()
            .gap(16.0)
            .padding(10.0)
            .center_y()
            .child(
                col()
                    .grow(1.0)
                    .gap(4.0)
                    .child(
                        text(format!(
                            "Signed in with GitHub as {}. Keep a copy of this Locust on Locust\u{2019}s relay.",
                            signed_in.login
                        ))
                        .font(l.ty.body)
                        .color(l.c("text-primary"))
                        .wrap(),
                    )
                    .child(if !on && status.has_key {
                        // Off with a key: the line above says what the switch does, and what "Off." went on to say is
                        // under More (review change 6).
                        crate::settings_panel::switch_more(s, &l, "account-sync", &[SYNC_OFF])
                    } else {
                        text(note).font(l.ty.body).color(l.c("text-secondary")).wrap()
                    }),
            )
            .child(switch),
    );

    if let Some(code) = &acct.code {
        root = root.child(
            row()
                .key("account-recovery-alert")
                .w_full()
                .gap(12.0)
                .padding(10.0)
                .center_y()
                .child(
                    col()
                        .grow(1.0)
                        .gap(4.0)
                        .child(
                            text("Write down your recovery code. It is shown only now.")
                                .font(l.ty.body)
                                .color(l.c("text-primary"))
                                .wrap(),
                        )
                        .child(
                            text(code.clone())
                                .key("account-recovery-code")
                                .font(l.ty.mono)
                                .color(l.c("text-primary"))
                                .wrap_anywhere(),
                        )
                        .child(
                            text("A new computer opens your copy with this code, or with an OK from a computer already signed in. Lose every computer and this code, and the copy cannot be opened by anyone, Locust included.")
                                .font(l.ty.body)
                                .color(l.c("text-secondary"))
                                .wrap(),
                        ),
                )
                .child(
                    boxed_button("I wrote it down", &l)
                        .key("account-code-dismiss")
                        .on_press(|s: &mut State| {
                            s.settings_panel.account.code = None;
                        }),
                ),
        );
    }

    if !on && let Some(elsewhere) = &status.elsewhere {
        let who = if elsewhere.computers == 1 {
            "Another computer keeps".to_owned()
        } else {
            format!("{} other computers keep", elsewhere.computers)
        };
        let bring_label = if acct.armed_bring {
            "Restart and bring it"
        } else {
            "Bring it here"
        };
        root = root.child(
            row()
                .key("account-bring-code")
                .w_full()
                .gap(12.0)
                .padding(10.0)
                .center_y()
                .child(
                    col()
                        .grow(1.0)
                        .gap(4.0)
                        .child(
                            text(format!(
                                "{who} a copy of your Locust, last changed {}. Bring it here with your recovery code.",
                                ago_of(&elsewhere.at, now_ms)
                            ))
                            .font(l.ty.body)
                            .color(l.c("text-primary"))
                            .wrap(),
                        )
                        .child(
                            text("Locust restarts once. What it replaces on this computer is moved aside first, not deleted.")
                                .font(l.ty.body)
                                .color(l.c("text-secondary"))
                                .wrap(),
                        )
                        .child(
                            text_input(&acct.recovery_input, type_::BODY)
                                .key("account-code-input")
                                .label("Recovery code")
                                .padding(8.0)
                                .radius(6.0)
                                .fill(p.field)
                                .color(p.text)
                                .placeholder("XXXXX-XXXXX-\u{2026}", p.muted)
                                .on_edit(|s: &mut State| &mut s.settings_panel.account.recovery_input),
                        ),
                )
                .child(
                    boxed_button(bring_label, &l)
                        .key("account-bring-button")
                        .label("Bring your Locust here")
                        .on_press(bring_here),
                ),
        );

        if acct.bringing.is_none() {
            let ask_row = if let Some(asking) = &status.asking {
                row()
                    .key("account-asking")
                    .w_full()
                    .gap(12.0)
                    .padding(10.0)
                    .center_y()
                    .child(
                        col()
                            .grow(1.0)
                            .gap(4.0)
                            .child(
                                text("Asking. On your other computer, open Settings \u{203a} General \u{203a} Your account, and approve there only if it shows this code:")
                                    .font(l.ty.body)
                                    .color(l.c("text-primary"))
                                    .wrap(),
                            )
                            .child(
                                text(asking.match_code.clone())
                                    .key("account-asking-match")
                                    .font(l.ty.mono)
                                    .color(l.c("text-primary")),
                            )
                            .child(
                                text("If the codes differ, choose Not mine there. Locust restarts here once it is approved.")
                                    .font(l.ty.body)
                                    .color(l.c("text-secondary"))
                                    .wrap(),
                            ),
                    )
                    .child(
                        boxed_button("Stop asking", &l)
                            .key("account-stop-asking")
                            .on_press(stop_asking),
                    )
            } else {
                row()
                    .key("account-ask-offer")
                    .w_full()
                    .gap(12.0)
                    .padding(10.0)
                    .center_y()
                    .child(
                        col().grow(1.0).child(
                            text("No code to hand? Ask the computer that has your Locust, and approve this one there.")
                                .font(l.ty.body)
                                .color(l.c("text-primary"))
                                .wrap(),
                        ),
                    )
                    .child(
                        boxed_button("Ask that computer", &l)
                            .key("account-ask-button")
                            .on_press(ask_computer),
                    )
            };
            root = root.child(ask_row);
        }
    }

    if let Some(computer) = &acct.bringing {
        root = root.child(
            text(format!(
                "Your Locust from {computer} is ready. Restarting to bring it in\u{2026}"
            ))
            .key("account-bringing")
            .px(10.0)
            .font(l.ty.body)
            .color(l.c("text-secondary"))
            .wrap(),
        );
    }

    if on || status.has_key {
        let mut acts = row().key("account-acts").w_full().gap(8.0).padding(10.0);
        if on {
            acts = acts.child(
                boxed_button("Sync now", &l)
                    .key("account-sync-now")
                    .on_press(sync_now),
            );
        }
        let delete_label = if acct.armed_delete {
            "Delete it"
        } else {
            "Delete my copy"
        };
        acts = acts.child(
            boxed_button(delete_label, &l)
                .key("account-delete-copy")
                .label("Delete my copy on Locust\u{2019}s relay")
                .on_press(delete_copy),
        );
        root = root.child(acts);
    }

    for ask in &status.asks {
        let room_approve = ask.room.clone();
        let match_approve = ask.match_code.clone();
        let room_ignore = ask.room.clone();
        let armed = acct.armed_approve.as_deref() == Some(ask.room.as_str());
        let approve_label = if armed { "Approve it" } else { "Approve" };
        root = root.child(
            row()
                .key(format!("account-ask-{}", ask.room))
                .w_full()
                .gap(12.0)
                .padding(10.0)
                .center_y()
                .child(
                    col()
                        .grow(1.0)
                        .gap(4.0)
                        .child(
                            text(format!(
                                "{} asks for your Locust. Approve only if its screen shows this code:",
                                ask.name
                            ))
                            .font(l.ty.body)
                            .color(l.c("text-primary"))
                            .wrap(),
                        )
                        .child(
                            text(ask.match_code.clone())
                                .font(l.ty.mono)
                                .color(l.c("text-primary")),
                        )
                        .child(
                            text("Approving hands that computer the key to your copy, sealed so only it can open it. If the codes differ, or it is not your computer, choose Not mine.")
                                .font(l.ty.body)
                                .color(l.c("text-secondary"))
                                .wrap(),
                        ),
                )
                .child(
                    row()
                        .gap(8.0)
                        .child(
                            boxed_button(approve_label, &l)
                                .key(format!("account-approve-{}", ask.room))
                                .label(format!("Approve {}", ask.name))
                                .on_press(move |s: &mut State| {
                                    approve_ask(s, room_approve.clone(), match_approve.clone());
                                }),
                        )
                        .child(
                            boxed_button("Not mine", &l)
                                .key(format!("account-ignore-{}", ask.room))
                                .on_press(move |s: &mut State| {
                                    ignore_ask(s, room_ignore.clone());
                                }),
                        ),
                ),
        );
    }

    append_footer(root, acct, &l)
}

fn append_footer(mut root: El, acct: &AccountPanelState, l: &crate::look::controls::Look) -> El {
    let text_out = acct
        .failure
        .as_deref()
        .or(acct.status.message.as_deref());
    if let Some(msg) = text_out {
        root = root.child(
            text(msg.to_owned())
                .key("account-message")
                .px(10.0)
                .font(l.ty.body)
                .color(l.c("text-secondary"))
                .wrap(),
        );
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_pages::SettingsPageId;
    use chrono::Duration;
    use locust_core::profile_sync::{LastSync, SyncUsage};
    use locust_core::your_account::{
        AccountAsk, AccountAsking, ElsewhereCopies, SignedInAccount,
    };
    use rune::{ElementInfo, Env, Runtime};

    fn settings_view(s: &State, _: &mut Cx<State>) -> El {
        crate::settings_panel::view(s, &Palette::new(true))
    }

    fn render(status: YourAccountStatus, code: Option<&str>) -> Vec<ElementInfo> {
        let mut s = State::default();
        s.settings_panel.open = true;
        s.settings_panel.page = Some(SettingsPageId::App);
        s.settings_panel.account.status = status;
        s.settings_panel.account.code = code.map(str::to_owned);
        let mut rt = Runtime::new(s, settings_view, (1200.0, 5000.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt.elements().into_iter().filter(|e| !e.exiting).collect()
    }

    fn all_text(els: &[ElementInfo]) -> String {
        els.iter()
            .filter_map(|e| e.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn account_switch(els: &[ElementInfo]) -> Option<&ElementInfo> {
        els.iter()
            .find(|e| e.key.as_deref() == Some("account-switch") && e.role == "switch")
    }

    #[test]
    // case: signed out, the card says what an account does and where to sign in, with no switch
    fn signed_out_the_card_says_what_an_account_does_and_where_to_sign_in_with_no_switch() {
        let els = render(YourAccountStatus::default(), None);
        let said = all_text(&els);
        assert!(said.contains("follow you to any computer where you sign in"), "{said}");
        assert!(
            said.contains("Turn on From anywhere under Your phone, then press Sign in with GitHub there."),
            "{said}"
        );
        assert!(account_switch(&els).is_none());
        assert!(!said.contains("Delete my copy"));
    }

    #[test]
    // case: signed in and off, it says the relay cannot read the copy before anything is sent
    fn signed_in_and_off_it_says_the_relay_cannot_read_the_copy_before_anything_is_sent() {
        let els = render(
            YourAccountStatus {
                signed_in: Some(SignedInAccount { login: "colin".into() }),
                sync: AccountSyncState::Off,
                has_key: false,
                syncing: false,
                ..Default::default()
            },
            None,
        );
        let said = all_text(&els);
        assert!(said.contains("Signed in with GitHub as colin."), "{said}");
        assert!(
            said.contains("so the relay cannot read it. Agent sign-ins and keys are never sent."),
            "{said}"
        );
        let look = crate::look::controls::Look::new(true);
        let sw = account_switch(&els).expect("account switch");
        assert_eq!(sw.fill, look.c("switch-off"));
        assert!(!said.contains("Delete my copy"));
    }

    #[test]
    // case: on, it says when it last synced, how much, and the storage used, with Sync now and Delete my copy
    fn on_it_says_when_it_last_synced_how_much_and_the_storage_used_with_sync_now_and_delete_my_copy()
    {
        let at = (Utc::now() - Duration::minutes(4))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let els = render(
            YourAccountStatus {
                signed_in: Some(SignedInAccount { login: "colin".into() }),
                sync: AccountSyncState::On,
                has_key: true,
                syncing: false,
                last_synced: Some(LastSync {
                    at,
                    files: 214,
                    bytes: 31 * 1024 * 1024,
                }),
                storage: Some(SyncUsage {
                    used: 33 * 1024 * 1024,
                    quota: 1024 * 1024 * 1024,
                }),
                ..Default::default()
            },
            None,
        );
        let said = all_text(&els);
        assert!(
            said.contains("Last synced 4 min ago: 214 files, 31.0 MB. Using 33.0 MB of 1.00 GB."),
            "{said}"
        );
        let look = crate::look::controls::Look::new(true);
        assert_eq!(account_switch(&els).unwrap().fill, look.c("switch-on"));
        assert!(said.contains("Sync now"));
        assert!(said.contains("Delete my copy"));
    }

    #[test]
    // case: the recovery code is shown with what losing it costs, until it is put away
    fn the_recovery_code_is_shown_with_what_losing_it_costs_until_it_is_put_away() {
        let els = render(
            YourAccountStatus {
                signed_in: Some(SignedInAccount { login: "colin".into() }),
                sync: AccountSyncState::On,
                has_key: true,
                syncing: true,
                ..Default::default()
            },
            Some("ABCDE-FGHJK"),
        );
        let said = all_text(&els);
        assert!(said.contains("Write down your recovery code. It is shown only now."));
        assert!(said.contains("ABCDE-FGHJK"));
        assert!(said.contains("cannot be opened by anyone, Locust included"));
        assert!(said.contains("I wrote it down"));
        assert!(said.contains("Syncing now."));
    }

    #[test]
    // case: sizes and times read as a person says them
    fn sizes_and_times_read_as_a_person_says_them() {
        assert_eq!(size_of(300), "1 KB");
        assert_eq!(size_of(500 * 1024), "500 KB");
        assert_eq!(size_of(5 * 1024 * 1024), "5.0 MB");
        let now = DateTime::parse_from_rfc3339("2026-10-31T18:00:00Z")
            .unwrap()
            .timestamp_millis();
        assert_eq!(ago_of("2026-10-31T17:59:30Z", now), "just now");
        assert_eq!(ago_of("2026-10-31T15:00:00Z", now), "3 h ago");
    }

    #[test]
    // case: on a new computer it offers to bring the copy here with the code, and says what a restart does
    fn on_a_new_computer_it_offers_to_bring_the_copy_here_with_the_code_and_says_what_a_restart_does()
    {
        let at = (Utc::now() - Duration::hours(2))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let els = render(
            YourAccountStatus {
                signed_in: Some(SignedInAccount { login: "colin".into() }),
                sync: AccountSyncState::Off,
                has_key: false,
                syncing: false,
                elsewhere: Some(ElsewhereCopies {
                    computers: 1,
                    at: at.clone(),
                }),
                ..Default::default()
            },
            None,
        );
        let said = all_text(&els);
        assert!(
            said.contains("Another computer keeps a copy of your Locust, last changed 2 h ago. Bring it here with your recovery code."),
            "{said}"
        );
        assert!(said.contains("What it replaces on this computer is moved aside first, not deleted."));
        assert!(els.iter().any(|e| e.label.as_deref() == Some("Recovery code")));
        assert!(said.contains("Bring it here"));

        // Already syncing here, it is not offered.
        let syncing_els = render(
            YourAccountStatus {
                signed_in: Some(SignedInAccount { login: "colin".into() }),
                sync: AccountSyncState::On,
                has_key: true,
                syncing: false,
                elsewhere: Some(ElsewhereCopies { computers: 1, at }),
                ..Default::default()
            },
            None,
        );
        assert!(!all_text(&syncing_els).contains("Bring it here"));
    }

    #[test]
    // case: a new computer can ask the one that has your Locust, and shows the code to compare while it waits
    fn a_new_computer_can_ask_the_one_that_has_your_locust_and_shows_the_code_to_compare_while_it_waits()
    {
        let at = (Utc::now() - Duration::minutes(1))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let offer = render(
            YourAccountStatus {
                signed_in: Some(SignedInAccount { login: "colin".into() }),
                sync: AccountSyncState::Off,
                has_key: false,
                syncing: false,
                elsewhere: Some(ElsewhereCopies {
                    computers: 1,
                    at: at.clone(),
                }),
                ..Default::default()
            },
            None,
        );
        let offer_text = all_text(&offer);
        assert!(offer_text.contains(
            "No code to hand? Ask the computer that has your Locust, and approve this one there."
        ));
        assert!(offer_text.contains("Ask that computer"));

        let waiting = render(
            YourAccountStatus {
                signed_in: Some(SignedInAccount { login: "colin".into() }),
                sync: AccountSyncState::Off,
                has_key: false,
                syncing: false,
                elsewhere: Some(ElsewhereCopies {
                    computers: 1,
                    at: at.clone(),
                }),
                asking: Some(AccountAsking {
                    match_code: "ERDZ-QGKN".into(),
                    since: at,
                }),
                ..Default::default()
            },
            None,
        );
        let waiting_text = all_text(&waiting);
        assert!(waiting_text.contains("approve there only if it shows this code:"));
        assert!(waiting_text.contains("ERDZ-QGKN"));
        assert!(waiting_text.contains("Stop asking"));
        assert!(!waiting_text.contains("Ask that computer"));
    }

    #[test]
    // case: the computer that has your Locust shows each ask with its code, Approve (asked twice) and Not mine
    fn the_computer_that_has_your_locust_shows_each_ask_with_its_code_approve_asked_twice_and_not_mine()
    {
        let room = "R".repeat(22);
        let els = render(
            YourAccountStatus {
                signed_in: Some(SignedInAccount { login: "colin".into() }),
                sync: AccountSyncState::On,
                has_key: true,
                syncing: false,
                asks: vec![AccountAsk {
                    room: room.clone(),
                    name: "Colin's Mac".into(),
                    match_code: "ERDZ-QGKN".into(),
                }],
                ..Default::default()
            },
            None,
        );
        let said = all_text(&els);
        assert!(
            said.contains("Colin's Mac asks for your Locust. Approve only if its screen shows this code:"),
            "{said}"
        );
        assert!(said.contains("ERDZ-QGKN"));
        assert!(said.contains("If the codes differ, or it is not your computer, choose Not mine."));
        assert!(els.iter().any(|e| e.label.as_deref() == Some("Approve Colin's Mac")));
        assert!(said.contains("Not mine"));
        // Below Sync now and Delete my copy, set apart.
        let delete_el = els
            .iter()
            .find(|e| e.text.as_deref() == Some("Delete my copy"))
            .expect("Delete my copy");
        let ask_el = els
            .iter()
            .find(|e| e.key.as_deref() == Some(&format!("account-ask-{room}")))
            .expect("account-ask row");
        assert!(
            delete_el.rect[1] < ask_el.rect[1],
            "Delete my copy ({}) must appear above ask ({})",
            delete_el.rect[1],
            ask_el.rect[1]
        );
    }
}
