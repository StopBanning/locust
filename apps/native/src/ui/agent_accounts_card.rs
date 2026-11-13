//! Settings › AI agents: other accounts for Claude Code and Codex (Electron `AgentAccountsSettings.tsx`, 0.797). One
//! row per account: this computer's own sign-in first, then each added one with where its sign-in stands, and Sign in,
//! Rename and Remove. Adding one opens its name in place; removing one asks first. The accounts and their sign-ins are
//! `locust_core::agent_accounts`'s.

use super::*;
use locust_core::agent_accounts::{Account, AccountState, Accounts, Agent};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct Card {
    pub accounts: Vec<Account>,
    pub adding: Option<Agent>,
    pub name: TextEdit,
    pub editing: Option<String>,
    pub removing: Option<Account>,
    pub message: Option<String>,
    pub read_at: Option<Instant>,
}

static ACCOUNTS: Mutex<Option<Arc<Accounts>>> = Mutex::new(None);

/// This computer's accounts, under the native data folder. Tests never run an agent's sign-in: without accounts of
/// their own there is no card.
fn accounts(s: &State) -> Option<Arc<Accounts>> {
    let mut held = ACCOUNTS.lock().unwrap_or_else(|e| e.into_inner());
    if cfg!(test) {
        return held.clone();
    }
    if held.is_none() {
        let store = s.data.clone()?;
        let login: Arc<locust_core::agent_accounts::Login> = Arc::new(|agent: Agent, home: &std::path::Path, env: &[(&'static str, String)], done| {
            let program = crate::runtime::find_executable(agent.as_str()).map_err(|e| e.to_string())?;
            locust_core::agent_accounts::start_login(&program, agent, home, env, done)
        });
        *held = Some(Arc::new(Accounts::new(
            store.root(),
            login,
            Box::new(|| locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "accounts")),
            // No run uses an added account in this window yet, so none is ever busy.
            Box::new(|_, _| false),
        )));
    }
    held.clone()
}

#[cfg(test)]
pub(super) fn use_accounts(accounts: Accounts) {
    *ACCOUNTS.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(accounts));
}

pub(super) fn reload(s: &mut State) {
    let Some(accounts) = accounts(s) else { return };
    match accounts.list() {
        Ok(list) => s.agent_accounts.accounts = list,
        Err(why) => s.agent_accounts.message = Some(why),
    }
    s.agent_accounts.read_at = Some(Instant::now());
}

/// While a sign-in waits, read again each second (Electron's refresh while waiting).
pub(super) fn poll(s: &mut State) -> bool {
    let waiting = s.agent_accounts.accounts.iter().any(|a| a.state == AccountState::Waiting);
    if !s.settings_panel.open || !waiting || s.agent_accounts.read_at.is_some_and(|at| at.elapsed() < Duration::from_secs(1)) {
        return false;
    }
    let before = s.agent_accounts.accounts.clone();
    reload(s);
    s.agent_accounts.accounts != before
}

fn act(s: &mut State, f: impl FnOnce(&Accounts) -> Result<(), String>) -> bool {
    let Some(accounts) = accounts(s) else { return false };
    s.agent_accounts.message = None;
    let done = f(&accounts);
    if let Err(why) = &done {
        s.agent_accounts.message = Some(why.clone());
    }
    reload(s);
    done.is_ok()
}

fn blocked() -> bool {
    locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "accounts").is_some()
}

fn state_sentence(state: AccountState) -> &'static str {
    match state {
        AccountState::Waiting => "Finish signing in in your browser. This waits for the agent.",
        AccountState::Ready => "Signed in.",
        AccountState::Failed => "Sign-in did not finish. Your other accounts are unchanged.",
        AccountState::New => "Ready to sign in.",
    }
}

fn button(l: &crate::look::controls::Look, label: &str, key: String) -> El {
    crate::look::controls::button(l, label, crate::look::controls::Tone::Neutral, false).key(key)
}

fn who(l: &crate::look::controls::Look, name: &str, said: &str) -> El {
    col()
        .grow(1.0)
        .gap(2.0)
        .child(text(name.to_owned()).font(l.ty.control).color(l.c("text-primary")))
        .child(text(said.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap())
}

fn name_field(s: &State, l: &crate::look::controls::Look, placeholder: &str, submit: fn(&mut State, &mut Tx)) -> El {
    text_input(&s.agent_accounts.name, l.ty.body)
        .key("account-name")
        .label("Account name")
        .placeholder(placeholder, l.c("text-muted"))
        .w(220.0)
        .padding(6.0)
        .radius(6.0)
        .fill(l.c("bg-control"))
        .color(l.c("text-primary"))
        .on_edit(|s: &mut State| &mut s.agent_accounts.name)
        .on_submit(submit)
}

fn add_submit(s: &mut State, _: &mut Tx) {
    let (Some(agent), name) = (s.agent_accounts.adding, s.agent_accounts.name.text.clone()) else { return };
    if name.trim().is_empty() {
        return;
    }
    if act(s, |a| a.add(agent.as_str(), &name).map(|_| ())) {
        s.agent_accounts.adding = None;
        s.agent_accounts.name = TextEdit::default();
    }
}

fn rename_submit(s: &mut State, _: &mut Tx) {
    let (Some(id), name) = (s.agent_accounts.editing.clone(), s.agent_accounts.name.text.clone()) else { return };
    if act(s, |a| a.rename(&id, &name)) {
        s.agent_accounts.editing = None;
    }
}

fn agent_block(s: &State, l: &crate::look::controls::Look, agent: Agent, label: &str) -> El {
    let card = &s.agent_accounts;
    let mut block = col().w_full().gap(6.0).padding(10.0).key(format!("accounts-{}", agent.as_str())).label(format!("{} accounts", agent.as_str()));
    block = block.child(text(label.to_owned()).font(l.ty.control).color(l.c("text-primary")));
    block = block.child(row().w_full().child(who(l, "This computer's sign-in", "Used when a teammate has no account picked.")));
    for account in card.accounts.iter().filter(|a| a.agent == agent) {
        let id = account.id.clone();
        if card.editing.as_deref() == Some(&account.id) {
            block = block.child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(name_field(s, l, "", rename_submit))
                    .child(button(l, "Save name", "account-save-name".into()).on_press_tx(rename_submit))
                    .child(button(l, "Cancel", "account-rename-cancel".into()).on_press(|s: &mut State| s.agent_accounts.editing = None)),
            );
            continue;
        }
        let mut actions = row().gap(8.0);
        let short = account.id[8..16].to_owned();
        if account.state == AccountState::Waiting {
            let id = id.clone();
            actions = actions.child(button(l, "Cancel sign-in", format!("account-cancel-{short}")).on_press(move |s: &mut State| {
                act(s, |a| a.cancel(&id));
            }));
        } else {
            let id = id.clone();
            let words = if account.state == AccountState::Ready { "Sign in again" } else { "Sign in" };
            actions = actions.child(button(l, words, format!("account-sign-in-{short}")).on_press(move |s: &mut State| {
                if !blocked() {
                    act(s, |a| a.login(&id));
                }
            }));
        }
        {
            let (id, name) = (id.clone(), account.name.clone());
            actions = actions.child(button(l, "Rename", format!("account-rename-{short}")).on_press(move |s: &mut State| {
                if blocked() {
                    return;
                }
                s.agent_accounts.editing = Some(id.clone());
                s.agent_accounts.name = TextEdit::new(&name);
                s.agent_accounts.name.focused = true;
            }));
        }
        if account.state != AccountState::Waiting {
            let account = account.clone();
            actions = actions.child(button(l, "Remove", format!("account-remove-{short}")).on_press(move |s: &mut State| s.agent_accounts.removing = Some(account.clone())));
        }
        block = block.child(row().w_full().gap(12.0).center_y().key(format!("account-{short}")).child(who(l, &account.name, state_sentence(account.state))).child(actions));
    }
    if card.adding == Some(agent) {
        block = block.child(
            col()
                .w_full()
                .gap(4.0)
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(name_field(s, l, "Name it, like Work", add_submit))
                        .child(button(l, "Add account", "account-add".into()).on_press_tx(add_submit))
                        .child(button(l, "Cancel", "account-add-cancel".into()).on_press(|s: &mut State| s.agent_accounts.adding = None)),
                )
                .child(text("Then Sign in opens the agent's own sign-in in your browser.").font(l.ty.body).color(l.c("text-muted"))),
        );
    } else if !blocked() {
        block = block.child(button(l, "Add another account", format!("account-add-another-{}", agent.as_str())).on_press(move |s: &mut State| {
            s.agent_accounts.adding = Some(agent);
            s.agent_accounts.name = TextEdit::default();
            s.agent_accounts.name.focused = true;
        }));
    }
    block
}

pub(super) fn section(s: &State, p: &Palette) -> Option<El> {
    accounts(s)?;
    let l = crate::look::controls::Look::new(p.dark);
    let card = &s.agent_accounts;
    let mut out = col()
        .w_full()
        .gap(4.0)
        .key("agent-accounts")
        .child(agent_block(s, &l, Agent::Claude, "Claude Code accounts"))
        .child(agent_block(s, &l, Agent::Codex, "Codex accounts"));
    if let Some(why) = &card.message {
        out = out.child(text(why.clone()).font(l.ty.body).color(l.c("amber-text")).wrap().role("alert").px(10.0));
    }
    if let Some(removing) = &card.removing {
        let id = removing.id.clone();
        let home = accounts(s).map(|a| a.home_for(&removing.id).display().to_string()).unwrap_or_default();
        out = out.child(
            col()
                .w_full()
                .gap(6.0)
                .padding(10.0)
                .radius(8.0)
                .border(1.0, l.c("border-card"))
                .key("account-remove-confirm")
                .role("alertdialog")
                .label(format!("Remove {}?", removing.name))
                .child(text(format!("Remove {}?", removing.name)).font(l.ty.control).color(l.c("text-primary")))
                .child(
                    text(format!(
                        "This deletes only {}'s agent home that Locust created: its sign-in, settings, session history and plugins. This computer's sign-in and your project files stay where they are. Teammates using this account will need another account selected.",
                        removing.name
                    ))
                    .font(l.ty.body)
                    .color(l.c("text-secondary"))
                    .wrap(),
                )
                .child(text(home).font(type_::LABEL.mono()).color(l.c("text-muted")).wrap_anywhere())
                .child(
                    row()
                        .gap(8.0)
                        .child(button(&l, "Delete this account", "account-delete".into()).on_press(move |s: &mut State| {
                            if act(s, |a| a.remove(&id)) {
                                s.agent_accounts.removing = None;
                            }
                        }))
                        .child(button(&l, "Keep account", "account-keep".into()).on_press(|s: &mut State| s.agent_accounts.removing = None)),
                ),
        );
    }
    Some(out)
}
