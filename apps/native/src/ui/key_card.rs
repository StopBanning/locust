//! A KEY IS ASKED FOR ON A CARD (Electron `KeyCard.tsx`, 0.833). Under the teammate's last answer that asked for keys
//! (`locust-key` blocks): each key's name, what it is for, a box that never shows what is typed, and Keep; once kept,
//! only that the teammate has it. A name no key may take is refused there, and an agent that cannot pass keys says so
//! instead of offering a box. The teammate's edit form lists its keys by name with Remove, and adds one masked
//! (`TeammateKeysField`).
//!
//! The names are read, and keys kept or removed, off the window's thread (the keychain may take a moment); the window
//! draws only what came back. A key's value goes from the box straight to the keychain and the box is emptied.

use super::*;
use locust_core::teammate_keys::{self, KEYS_REACH_SAID, KeyRequest, key_name_refusal, key_requests_in};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Keys {
    /// Each teammate's key names, as last read.
    pub names: HashMap<String, Vec<String>>,
    /// The box for each key asked for, by teammate and key name.
    pub drafts: HashMap<(String, String), TextEdit>,
    /// The edit form's new key.
    pub new_name: TextEdit,
    pub new_value: TextEdit,
    /// What the last change said went wrong, for which teammate.
    pub problem: Option<(String, String)>,
}

/// A change's answer: for whom, and the names after it (or why it did not happen).
pub(super) type Answer = (String, teammate_keys::Changed);

enum Change {
    Read,
    Keep(String, String),
    Remove(String),
}

fn store(s: &State) -> Option<teammate_keys::TeammateKeys> {
    let data = s.data.as_ref()?;
    // The window's tests never reach the real keychain: without a stand-in of their own, nothing keeps a key.
    if cfg!(test) && !locust_core::own_models::has_stand_in() {
        return Some(teammate_keys::TeammateKeys::new(data.root(), std::sync::Arc::new(locust_core::own_models::NoBox)));
    }
    Some(teammate_keys::for_store(data))
}

fn change(s: &mut State, teammate: String, what: Change) {
    if s.keys_job.is_pending() {
        return;
    }
    let Some(keys) = store(s) else { return };
    s.keys.problem = None;
    s.keys_job.request((keys, teammate, what), |(keys, teammate, what)| {
        let answer = match what {
            Change::Read => Ok(keys.names(&teammate)),
            Change::Keep(name, value) => keys.set(&teammate, &name, &value),
            Change::Remove(name) => keys.remove(&teammate, &name),
        };
        (teammate, answer)
    });
}

/// The teammate and key requests under the last finished answer of the shown conversation, if it asked for any.
fn asked(s: &State) -> Option<(Teammate, Vec<KeyRequest>)> {
    if turn_running(s) || s.flight.is_some() || s.sending.is_pending() {
        return None;
    }
    let (teammate, conversation) = selected_conversation(s)?;
    if teammate.id == LEDGER_TARGET || teammate.name.trim().is_empty() {
        return None;
    }
    let last: &str = match &s.ledger {
        Some(pane) => {
            if pane.loading || pane.snapshot.running {
                return None;
            }
            let start = pane.items.iter().rposition(|i| matches!(i, ledger::Item::User(_))).map_or(0, |i| i + 1);
            pane.items[start..].iter().rev().find_map(|i| match i {
                ledger::Item::Assistant { text, r#final: true, .. } => Some(text.as_str()),
                _ => None,
            })?
        }
        // A conversation read from Electron's list, before its record opens: its last turn, when it is the teammate's.
        None => conversation.turns.last().filter(|(who, _)| who != "You").map(|(_, body)| body.as_str())?,
    };
    let requests = key_requests_in(last);
    (!requests.is_empty()).then(|| (teammate.clone(), requests))
}

/// Read the names a card or the edit form needs and does not have yet; and take a change's answer when it lands.
pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    if let Some((teammate, answer)) = s.keys_job.take() {
        match answer {
            Ok(names) => {
                s.keys.names.insert(teammate, names);
            }
            Err((why, names)) => {
                s.keys.names.insert(teammate.clone(), names);
                s.keys.problem = Some((teammate, why));
            }
        }
        changed = true;
    }
    let wanted = asked(s).map(|(t, _)| t.id).or_else(|| s.people_panel.form_open.then(|| s.people_panel.editing.clone()).flatten());
    if let Some(teammate) = wanted
        && !s.keys.names.contains_key(&teammate)
        && !s.keys_job.is_pending()
    {
        change(s, teammate, Change::Read);
    }
    changed
}

fn note(l: &crate::look::controls::Look, words: String) -> El {
    text(words).font(type_::LABEL).color(l.c("text-secondary")).wrap()
}

fn masked(l: &crate::look::controls::Look, edit: &TextEdit, key: String, label: String, pick: impl Fn(&mut State) -> &mut TextEdit + 'static) -> El {
    text_input(edit, l.ty.body)
        .key(key)
        .single_line()
        .secure()
        .label(label)
        .placeholder("Paste the key", l.c("text-muted"))
        .grow(1.0)
        .h(30.0)
        .padding(6.0)
        .radius(6.0)
        .fill(l.c("bg-control"))
        .color(l.c("text-primary"))
        .on_edit(pick)
}

/// The card under the answer that asked.
pub(super) fn card(s: &State, p: &Palette) -> Option<El> {
    let (teammate, requests) = asked(s)?;
    let l = crate::look::controls::Look::new(p.dark);
    let name = teammate.name.clone();
    let mut body = col().key("key-card").label(format!("Keys {name} asked for")).w_full().gap(8.0).padding(12.0).radius(8.0).fill(l.c("bg-raised")).border(1.0, l.c("border-hairline"));
    if !teammate.runtime.as_deref().is_some_and(teammate_keys::reaches) {
        return Some(body.child(note(&l, format!("{name} asked for a key, but Locust does not give keys to its agent yet, so it could not use one. Keys reach {KEYS_REACH_SAID} teammates for now: give {name} one of those in its settings to use a key."))));
    }
    let kept = s.keys.names.get(&teammate.id);
    for request in requests {
        let why = if request.why.is_empty() { String::new() } else { format!(" -- {}", request.why) };
        if let Some(refused) = key_name_refusal(&request.name) {
            body = body.child(note(&l, format!("{name} asked for a key named {}{why}. {refused} Ask {name} for it under another name.", request.name)));
            continue;
        }
        if kept.is_some_and(|k| k.contains(&request.name)) {
            body = body.child(note(&l, format!("{name} has {}. It can use it from its next turn; remove it any time in {name}\u{2019}s settings.", request.name)).key(format!("key-card-has-{}", request.name)));
            continue;
        }
        let at = (teammate.id.clone(), request.name.clone());
        let draft = s.keys.drafts.get(&at).cloned().unwrap_or_default();
        let filled = !draft.text.trim().is_empty();
        let (pick_at, keep_at, keep_name) = (at.clone(), at.clone(), name.clone());
        body = body
            .child(note(&l, format!("{name} asks for a key: {}{why}", request.name)))
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(masked(&l, &draft, format!("key-card-value-{}", request.name), format!("The key {} for {name}", request.name), move |s: &mut State| s.keys.drafts.entry(pick_at.clone()).or_default()))
                    .child(crate::look::controls::button(&l, &format!("Keep for {keep_name}"), crate::look::controls::Tone::Neutral, !filled || s.keys_job.is_pending()).key(format!("key-card-keep-{}", request.name)).on_press(move |s: &mut State| {
                        let Some(edit) = s.keys.drafts.remove(&keep_at) else { return };
                        if !edit.text.trim().is_empty() {
                            change(s, keep_at.0.clone(), Change::Keep(keep_at.1.clone(), edit.text.clone()));
                        }
                    })),
            )
            .child(note(&l, format!("Kept encrypted on this computer, for {name} alone. Its runs get it as {}, and Locust takes the value out of anything it records or shows.", request.name)));
    }
    if let Some((who, why)) = &s.keys.problem
        && who == &teammate.id
    {
        body = body.child(text(why.clone()).font(type_::LABEL).color(l.c("amber-text")).wrap().key("key-card-problem"));
    }
    Some(body)
}

/// Keys, in a teammate's edit form: its keys by name with Remove, and a new one added masked.
pub(super) fn field(s: &State, p: &Palette, teammate: &str, name: &str) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let mut out = col().key("team-keys").gap(6.0).child(text("Keys").font(type_::LABEL).color(p.secondary));
    if let Some(kept) = s.keys.names.get(teammate) {
        for one in kept {
            let (id, key_name) = (teammate.to_owned(), one.clone());
            out = out.child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(text(one.clone()).font(l.ty.mono).color(l.c("text-primary")).grow(1.0))
                    .child(crate::look::controls::button(&l, "Remove", crate::look::controls::Tone::Neutral, s.keys_job.is_pending()).key(format!("team-key-remove-{one}")).label(format!("Remove {one}")).on_press(move |s: &mut State| change(s, id.clone(), Change::Remove(key_name.clone())))),
            );
        }
    }
    let filled = !s.keys.new_name.text.trim().is_empty() && !s.keys.new_value.text.trim().is_empty();
    let id = teammate.to_owned();
    out = out.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(
                text_input(&s.keys.new_name, l.ty.mono)
                    .key("team-key-name")
                    .single_line()
                    .label("The key\u{2019}s name")
                    .placeholder("STRIPE_KEY", l.c("text-muted"))
                    .w(160.0)
                    .h(30.0)
                    .padding(6.0)
                    .radius(6.0)
                    .fill(l.c("bg-control"))
                    .color(l.c("text-primary"))
                    .on_edit(|s: &mut State| &mut s.keys.new_name),
            )
            .child(masked(&l, &s.keys.new_value, "team-key-value".into(), "The key".into(), |s: &mut State| &mut s.keys.new_value))
            .child(crate::look::controls::button(&l, "Add", crate::look::controls::Tone::Neutral, !filled || s.keys_job.is_pending()).key("team-key-add").on_press(move |s: &mut State| {
                let (key_name, value) = (std::mem::take(&mut s.keys.new_name).text.clone(), std::mem::take(&mut s.keys.new_value).text.clone());
                if !key_name.trim().is_empty() && !value.trim().is_empty() {
                    change(s, id.clone(), Change::Keep(key_name, value));
                }
            })),
    );
    if let Some((who, why)) = &s.keys.problem
        && who == teammate
    {
        out = out.child(text(why.clone()).font(type_::LABEL).color(l.c("amber-text")).wrap().key("team-key-problem"));
    }
    out.child(note(&l, format!("Kept encrypted on this computer, for {name} alone. Its runs get each as an environment variable of that name; its brief names them, and Locust takes their values out of anything it records or shows. Keys reach {KEYS_REACH_SAID} teammates for now.")))
}
