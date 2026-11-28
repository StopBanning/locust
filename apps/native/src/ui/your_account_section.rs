//! Settings › General › "Your account" (Electron `components/YourAccountSettings.tsx`, accounts slice 3/4/4b).
//!
//! Draws the sealed profile copy card from `locust_core::your_account::YourAccountStatus`: signed-out guide,
//! sync switch and status, one-time recovery code alert, recovery-code bring-here form, computer-to-computer
//! ask/approve flow, and Sync now / Delete my copy actions.

use super::*;
use chrono::{DateTime, Utc};
use locust_core::your_account::{
    AccountLink, AccountSyncState, BringHereResult, YourAccount, YourAccountOptions, YourAccountStatus,
};
use crate::phone_section::SharedLink;
use std::sync::{Arc, Mutex, mpsc};
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(test))]
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
    worker: Option<AccountWorker>,
    identity: Option<Identity>,
    pending: bool,
    restore_pending: bool,
    #[cfg(test)]
    pub(super) service_fixture: Option<YourAccountOptions>,
}

type Identity = (String, String, u64);
fn identity(link: &AccountLink) -> Identity {
    (link.relay.clone(), link.room.clone(), link.user)
}

enum Action {
    On,
    Off,
    Sync,
    Delete,
    Bring(String),
    Ask,
    Stop,
    Approve(String, String),
    Ignore(String),
}

struct Answer {
    who: Option<Identity>,
    status: Option<YourAccountStatus>,
    action: bool,
    code: Option<String>,
    brought: Option<BringHereResult>,
    restore_pending: bool,
}

enum Request {
    Refresh,
    Act(Identity, Action),
}

struct AccountWorker {
    phone: SharedLink,
    send: mpsc::Sender<Request>,
    answers: mpsc::Receiver<Answer>,
}

#[cfg(not(test))]
struct AccountHost {
    profile: std::path::PathBuf,
    secrets: Arc<dyn locust_core::own_models::SecretBox>,
}

#[cfg(not(test))]
static HOST: OnceLock<AccountHost> = OnceLock::new();

#[cfg(not(test))]
pub(crate) fn open() {
    if let Some(store) = locust_core::native::store() {
        let _ = HOST.set(AccountHost {
            profile: store.root().to_path_buf(),
            secrets: locust_core::own_models::secrets(),
        });
    }
}

#[cfg(test)]
pub(crate) fn open() {}

#[cfg(not(test))]
fn host_options(s: &mut State) -> Option<YourAccountOptions> {
    let host = HOST.get()?;
    if s.demo || s.engine != Some(Kind::Native) || s.data.as_ref().is_none_or(|store| store.root() != host.profile) {
        return None;
    }
    let restore_root = host.profile.clone();
    let version = env!("CARGO_PKG_VERSION");
    Some(YourAccountOptions {
        profile: host.profile.clone(),
        link: Arc::new(|| None), // Replaced by the window's live, local-only phone host in setup.
        secrets: host.secrets.clone(),
        computer: Arc::new(|| {
            let name = std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .unwrap_or_else(|_| "This computer".into());
            if name.to_ascii_lowercase().ends_with(".local") {
                name[..name.len() - 6].to_owned()
            } else {
                name
            }
        }),
        app_version: version.into(),
        busy: Arc::new(|| !locust_core::engine_api::api().running().is_empty()),
        remote: None,
        now: None,
        quiet_ms: None,
        copies_fresh_ms: None,
        ask_for_ms: None,
        changed: None,
        restore: Some(Arc::new(move |folder| {
            use locust_core::profile_backup::{PENDING_RESTORE_FILE, request_restore};
            if restore_root.join(PENDING_RESTORE_FILE).exists() {
                return Err("A restore is already waiting. Restart Locust before bringing in another copy.".into());
            }
            request_restore(folder, &restore_root, version, locust_core::engine_api::api().running().len(), Utc::now())
                .map(|_| ())
        })),
    })
}

#[cfg(test)]
fn host_options(s: &mut State) -> Option<YourAccountOptions> {
    s.settings_panel.account.service_fixture.take()
}

pub(super) fn setup(s: &mut State, phone: SharedLink, wake: Waker) {
    if s.settings_panel.account.worker.is_some() {
        return;
    }
    let Some(mut options) = host_options(s) else { return };
    let source = phone.clone();
    // The service uses one consistent, authenticated room per command. It is never populated
    // from a desk payload, a renderer login string, or a GitHub CLI account.
    let active = Arc::new(Mutex::new(None));
    let access = active.clone();
    options.link = Arc::new(move || access.lock().unwrap().clone());
    let restoring = Arc::new(AtomicBool::new(false));
    if let Some(stage) = options.restore.take() {
        let accepted = restoring.clone();
        options.restore = Some(Arc::new(move |folder| {
            stage(folder)?;
            accepted.store(true, Ordering::Release);
            Ok(())
        }));
    }
    let account = YourAccount::new(options);
    let (send, requests) = mpsc::channel();
    let (reply, answers) = mpsc::channel();
    let _ = send.send(Request::Refresh);
    std::thread::spawn(move || {
        let interval = std::time::Duration::from_millis(locust_core::your_account::ASK_EVERY_MS);
        let mut last = None;
        let mut last_who = None;
        loop {
            let request = match requests.recv_timeout(interval) {
                Ok(request) => Some(request),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            let link = source.account_link();
            let who = link.as_ref().map(identity);
            if who != last_who {
                account.link_changed();
                last_who = who.clone();
            }
            *active.lock().unwrap() = link.clone();
            let mut code = None;
            let mut brought = None;
            let action = matches!(&request, Some(Request::Act(..)));
            let already_pending = restoring.load(Ordering::Acquire);
            let status = match request {
                Some(Request::Act(expected, action))
                    if !already_pending && who.as_ref() == Some(&expected) =>
                {
                    match action {
                        Action::On => {
                            let result = account.turn_on();
                            code = result.code;
                            Some(result.status)
                        }
                        Action::Off => Some(account.turn_off()),
                        Action::Sync => Some(account.sync_now()),
                        Action::Delete => Some(account.delete_copy()),
                        Action::Bring(typed) => {
                            brought = Some(account.bring_here(&typed));
                            // In particular, a mistyped code must not cause a fresh relay lookup.
                            None
                        }
                        Action::Ask => Some(account.ask()),
                        Action::Stop => Some(account.stop_asking()),
                        Action::Approve(room, match_code) => Some(account.approve(&room, &match_code)),
                        Action::Ignore(room) => Some(account.ignore(&room)),
                    }
                }
                None if !already_pending => Some(account.maintain()),
                _ => Some(account.status()),
            };
            *active.lock().unwrap() = None;
            let restore_pending = restoring.load(Ordering::Acquire);
            // Drop an old room's result (especially its one-time code) after a change of sign-in.
            if source.account_link() != link {
                last = None;
                if reply.send(Answer { who, status: None, action, code: None, brought: None, restore_pending }).is_err() {
                    break;
                }
                wake();
                continue;
            }
            if let Some(status) = &status {
                if !action && last.as_ref().is_some_and(|(a, b)| a == &who && b == status) {
                    continue;
                }
                last = Some((who.clone(), status.clone()));
            }
            if reply.send(Answer { who, status, action, code, brought, restore_pending }).is_err() {
                break;
            }
            wake();
        }
        account.stop();
    });
    s.settings_panel.account.worker = Some(AccountWorker { phone, send, answers });
}

pub(crate) fn read(s: &mut State) {
    if let Some(worker) = &s.settings_panel.account.worker {
        let _ = worker.send.send(Request::Refresh);
    }
}

pub(super) fn poll(s: &mut State) -> bool {
    let acct = &mut s.settings_panel.account;
    let Some(worker) = &acct.worker else { return false };
    let now = worker.phone.account_link().as_ref().map(identity);
    let mut changed = false;
    if now != acct.identity {
        acct.identity = now;
        acct.status = YourAccountStatus::default();
        acct.code = None;
        acct.failure = None;
        acct.armed_delete = false;
        acct.armed_bring = false;
        acct.armed_approve = None;
        acct.recovery_input.replace_text(String::new());
        let _ = worker.send.send(Request::Refresh);
        changed = true;
    }
    while let Ok(answer) = worker.answers.try_recv() {
        if answer.restore_pending && !acct.restore_pending {
            acct.restore_pending = true;
            acct.recovery_input.replace_text(String::new());
            changed = true;
        }
        if answer.action {
            acct.pending = false;
            changed = true;
        }
        if answer.who != acct.identity {
            continue;
        }
        if let Some(status) = answer.status
            && acct.status != status
        {
            acct.status = status;
            changed = true;
        }
        if let Some(code) = answer.code {
            acct.code = Some(code);
            changed = true;
        }
        match answer.brought {
            Some(BringHereResult::Ok { computer }) => {
                acct.bringing = Some(computer);
                acct.recovery_input.replace_text(String::new());
                changed = true;
            }
            Some(BringHereResult::Err { reason }) => {
                acct.failure = Some(reason);
                changed = true;
            }
            None => {}
        }
    }
    changed
}

fn act(s: &mut State, action: Action) {
    let acct = &mut s.settings_panel.account;
    if acct.pending || acct.restore_pending {
        return;
    }
    if let (Some(worker), Some(who)) = (&acct.worker, &acct.identity) {
        acct.failure = None;
        acct.pending = worker.send.send(Request::Act(who.clone(), action)).is_ok();
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
    let action = if s.settings_panel.account.status.sync == AccountSyncState::On { Action::Off } else { Action::On };
    act(s, action);
}

fn sync_now(s: &mut State) {
    act(s, Action::Sync);
}

fn delete_copy(s: &mut State) {
    let acct = &mut s.settings_panel.account;
    if acct.pending || acct.restore_pending {
        return;
    }
    if !acct.armed_delete {
        acct.armed_delete = true;
        return;
    }
    acct.armed_delete = false;
    act(s, Action::Delete);
}

fn bring_here(s: &mut State) {
    let acct = &mut s.settings_panel.account;
    if acct.pending || acct.restore_pending || acct.recovery_input.text.trim().len() < 10 {
        return;
    }
    if !acct.armed_bring {
        acct.armed_bring = true;
        return;
    }
    acct.armed_bring = false;
    let typed = acct.recovery_input.text.clone();
    if typed.len() <= 200 {
        act(s, Action::Bring(typed));
    }
}

fn ask_computer(s: &mut State) {
    act(s, Action::Ask);
}

fn stop_asking(s: &mut State) {
    act(s, Action::Stop);
}

fn approve_ask(s: &mut State, room: String, match_code: String) {
    let acct = &mut s.settings_panel.account;
    if acct.pending {
        return;
    }
    if acct.armed_approve.as_deref() != Some(room.as_str()) {
        acct.armed_approve = Some(room);
        return;
    }
    acct.armed_approve = None;
    act(s, Action::Approve(room, match_code));
}

fn ignore_ask(s: &mut State, room: String) {
    s.settings_panel.account.armed_approve = None;
    act(s, Action::Ignore(room));
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

    let switch = crate::look::controls::switch(&l, on, SWITCH_LABEL, acct.pending || acct.restore_pending)
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
            "Prepare the copy"
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
                            text("Restart Locust to bring in the copy. What it replaces on this computer is moved aside first, not deleted.")
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

        if !acct.restore_pending {
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
                                text("If the codes differ, choose Not mine there. Restart Locust here once it is approved.")
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
                "Your Locust from {computer} is ready. Restart Locust to bring it in."
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
    let text_out = if acct.restore_pending && acct.bringing.is_none() && acct.status.message.is_none() {
        Some("A copy is ready. Restart Locust to bring it in.")
    } else {
        acct.failure.as_deref().or(acct.status.message.as_deref())
    };
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
