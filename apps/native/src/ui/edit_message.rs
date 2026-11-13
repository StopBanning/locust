//! Edit an earlier message, optionally putting files back (Electron 0.498, 0.502, 0.512: Thread.tsx's Edit, Composer.tsx's
//! line over the box, App.tsx's `rewinding`). Each message the person typed carries Edit; its words go back in the box
//! under a line that says what sending will do, with Cancel. Sending starts again from the turn before it (the engine's
//! `Rewind`), and the new turn says above it that it started again. What the box offers is read in a job: the folder is
//! never read while drawing.
use super::*;
use locust_core::edit_earlier::{self, PutBackNote, TurnLink, Versions};
use rune_kit::Checkbox;
use std::sync::{Arc, Mutex};

/// What putting back the files the set-aside replies changed would do.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Offer {
    pub files: Vec<String>,
    pub cannot: Vec<(String, String)>,
}

impl Offer {
    /// Files that can all go back: the tick is offered. One that cannot holds the rest (all of them or none).
    fn tickable(&self) -> bool {
        !self.files.is_empty() && self.cannot.is_empty()
    }
}

/// An earlier message being edited: which conversation on screen, its newest turn, the turn before the edited one (absent
/// for the first message) and the turns the edit sets aside.
pub(super) struct Editing {
    pub conversation: String,
    pub tip: String,
    pub turn: String,
    pub before: Option<String>,
    pub set_aside: Vec<String>,
    pub folder: Option<std::path::PathBuf>,
    /// None until the job has read the records (or when they could not be read): files stay as they are.
    pub offer: Option<Offer>,
    pub put_back: bool,
}

/// The conversation this window started again, and what putting back came to for the turn that began it.
pub(super) struct Rewound {
    pub conversation: String,
    pub put_back: Option<PutBackNote>,
}

pub(super) type OfferAnswer = (String, Option<Offer>);

#[derive(Default)]
pub(super) struct Panel {
    pub editing: Option<Editing>,
    pub rewound: Option<Rewound>,
    /// What the send job's put-back came to, left for the poll that sees the send land.
    pub receipt: Arc<Mutex<Option<PutBackNote>>>,
    pub offer_job: Latest<OfferAnswer>,
    /// The other version of each turn an edit touched, from the recent list (never from the disk).
    pub versions: std::collections::HashMap<String, Versions>,
    versions_key: Option<(usize, usize, String, usize)>,
}

/// The turns that wear a bubble, in order: a check-fix turn is the host's own and has none.
fn bubbled(pane: &LedgerPane) -> Vec<&ledger::Metadata> {
    pane.snapshot.turn_metas.iter().filter(|m| m.check_fix.is_none()).collect()
}

/// The turn the `ui`th bubble belongs to, when it offers Edit: one the person typed, not while a turn runs (or a send is
/// in flight), and not one the host or another app started.
pub(super) fn editable_turn(s: &State, ui: usize) -> Option<String> {
    if s.demo || s.engine != Some(Kind::Native) || turn_running(s) || s.sending.is_pending() {
        return None;
    }
    let pane = s.ledger.as_ref()?;
    let meta = *bubbled(pane).get(ui)?;
    let typed = meta.started_by_kind.is_none() && !meta.side && meta.carried_on_for.is_none() && meta.continues_reason.as_deref() != Some("route-switch");
    typed.then(|| meta.mission_id.clone())
}

/// Edit on a sent message: its words back in the box, and the next send branches from before it.
pub(super) fn start(s: &mut State, turn: String, words: String) {
    if turn_running(s) || s.sending.is_pending() {
        return;
    }
    let Some(pane) = s.ledger.as_ref() else { return };
    let metas = &pane.snapshot.turn_metas;
    let Some(at) = metas.iter().position(|m| m.mission_id == turn) else { return };
    let Some(conversation) = pane.conversation.conversation_id.clone() else { return };
    let editing = Editing {
        conversation,
        tip: pane.snapshot.tip.clone(),
        before: at.checked_sub(1).map(|i| metas[i].mission_id.clone()),
        set_aside: metas[at..].iter().map(|m| m.mission_id.clone()).collect(),
        folder: pane.header.folder.as_ref().map(std::path::PathBuf::from),
        turn: turn.clone(),
        offer: None,
        put_back: false,
    };
    let (folder, set_aside, store) = (editing.folder.clone(), editing.set_aside.clone(), native::store());
    s.edit.editing = Some(editing);
    s.edit.rewound = None;
    s.edit.offer_job.cancel();
    if let (Some(folder), Some(store)) = (folder, store) {
        s.edit.offer_job.request((turn, folder, set_aside, store), |(turn, folder, set_aside, store)| {
            let offer = checkpoints::plan_put_back(&store, &folder, &set_aside).ok().map(|plan| Offer { files: plan.files, cannot: plan.cannot });
            (turn, offer)
        });
    }
    s.draft.replace_text(words);
}

/// Cancel: the edit is let go and the box is emptied.
pub(super) fn cancel(s: &mut State) {
    s.edit.editing = None;
    s.edit.offer_job.cancel();
    s.draft.replace_text(String::new());
}

/// What a send should do for the edit open on this conversation: the engine's `Rewind`, and the files to put back first
/// (only when the tick is on and every one can go back).
pub(super) struct ForSend {
    pub rewind: native::Rewind,
    pub put_back: Option<(std::path::PathBuf, Vec<String>)>,
}

pub(super) fn for_send(s: &State) -> Option<ForSend> {
    let editing = s.edit.editing.as_ref()?;
    let here = s.ledger.as_ref()?.conversation.conversation_id.as_deref();
    if here != Some(editing.conversation.as_str()) {
        return None;
    }
    let put_back = match (&editing.offer, &editing.folder) {
        (Some(offer), Some(folder)) if editing.put_back && offer.tickable() => Some((folder.clone(), editing.set_aside.clone())),
        _ => None,
    };
    Some(ForSend {
        rewind: native::Rewind { tip: editing.tip.clone(), before: editing.before.clone(), set_aside: editing.set_aside.clone() },
        put_back,
    })
}

/// The put-back's answer (`{"putBack", "leftAlone", "heldBack"}`) as the thread says it.
pub(super) fn note_of(answer: &serde_json::Value) -> PutBackNote {
    let words = |key: &str| -> Vec<String> { answer[key].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect() };
    PutBackNote {
        put_back: words("putBack"),
        left_alone: answer["leftAlone"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|f| (f["path"].as_str().unwrap_or_default().to_owned(), f["why"].as_str().unwrap_or_default().to_owned()))
            .collect(),
        held_back: answer["heldBack"].as_array().map_or(0, Vec::len),
    }
}

/// The send landed: the edit is done, and the new conversation says it started again.
pub(super) fn sent(s: &mut State, started: &str) {
    if s.edit.editing.take().is_none() {
        return;
    }
    let put_back = crate::invariants::lock(&s.edit.receipt).take();
    s.edit.rewound = Some(Rewound { conversation: started.to_owned(), put_back });
}

pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    if let Some((turn, offer)) = s.edit.offer_job.take() {
        if let Some(editing) = s.edit.editing.as_mut().filter(|e| e.turn == turn) {
            editing.offer = offer;
            changed = true;
        }
    }
    // Another conversation opened: the edit was of that one, and is let go.
    let here = s.ledger.as_ref().and_then(|p| p.conversation.conversation_id.clone());
    if s.edit.editing.as_ref().is_some_and(|e| here.as_deref() != Some(e.conversation.as_str())) {
        s.edit.editing = None;
        s.edit.offer_job.cancel();
        changed = true;
    }
    if refresh_versions(s) {
        changed = true;
    }
    changed
}

/// The other version of each turn on screen an edit touched, worked out from the recent list when it or the thread moves.
fn refresh_versions(s: &mut State) -> bool {
    let Some(pane) = s.ledger.as_ref() else {
        return !std::mem::take(&mut s.edit.versions).is_empty();
    };
    let key = (
        s.ledger_summaries.len(),
        pane.snapshot.turn_metas.len(),
        pane.snapshot.tip.clone(),
        s.ledger_summaries.values().filter(|m| m.edited).count(),
    );
    if s.edit.versions_key.as_ref() == Some(&key) {
        return false;
    }
    s.edit.versions_key = Some(key);
    let ids: Vec<&str> = pane.snapshot.turn_metas.iter().map(|m| m.mission_id.as_str()).collect();
    let links: Vec<TurnLink> = s
        .ledger_summaries
        .values()
        .map(|m| TurnLink {
            mission_id: m.mission_id.clone(),
            created_at: m.created_at.clone(),
            parent: m.parent.clone(),
            follow_up: m.continues_reason.as_deref() == Some("follow-up"),
            edited: m.edited,
        })
        .collect();
    let next = edit_earlier::turn_versions_of(&ids, &links);
    let changed = next != s.edit.versions;
    s.edit.versions = next;
    changed
}

fn pencil(s: &State, ui: usize, words: &str, p: &Palette) -> Option<El> {
    let turn = editable_turn(s, ui)?;
    let l = crate::look::controls::Look::new(p.dark);
    let words = words.to_owned();
    let id = turn.clone();
    Some(
        crate::look::controls::head_icon(&crate::look::icons::PENCIL, "Edit this message", "Edit this message and start again from here")
            .on_press(move |s: &mut State| start(s, id.clone(), words.clone()))
            .build(l.k)
            .key(format!("edit-message-{turn}")),
    )
}

/// Inside the row of a person's bubble, left of it: Edit, when this message offers it. Not on one with notes on a diff.
pub(super) fn edit_control(s: &State, ui: usize, words: &str, has_notes: bool, p: &Palette) -> Option<El> {
    if has_notes {
        return None;
    }
    pencil(s, ui, words, p)
}

fn link(l: &crate::look::controls::Look, label: &str, key: String, target: String) -> El {
    text(label.to_owned())
        .font(l.ty.label)
        .color(l.c("text-primary"))
        .py(2.0)
        .role("button")
        .hover(l.c("hover-surface"))
        .key(key)
        .on_press_tx(move |s: &mut State, tx: &mut Tx| peer_thread::open_mission_by_id(s, target.clone(), tx))
}

/// Above a turn an edit touched: a turn this window started again says so (and what was put back), another says it was
/// edited, and each names the way to the other version.
pub(super) fn note_above(s: &State, meta: &ledger::Metadata, p: &Palette) -> Option<El> {
    let l = crate::look::controls::Look::new(p.dark);
    let pane = s.ledger.as_ref()?;
    let versions = s.edit.versions.get(&meta.mission_id);
    let ours = s
        .edit
        .rewound
        .as_ref()
        .filter(|r| meta.edited && pane.conversation.conversation_id.as_deref() == Some(r.conversation.as_str()) && pane.snapshot.tip == meta.mission_id);
    let words = match (ours, versions) {
        (Some(r), _) => edit_earlier::started_again_note(r.put_back.as_ref()),
        (None, Some(v)) => format!("{} ", edit_earlier::edited_note(v)?),
        (None, None) if meta.edited => format!("{} ", edit_earlier::STARTED_AGAIN),
        (None, None) => return None,
    };
    let mut line = row().w_full().wrap().center_y().gap(6.0).child(text(words.trim_end().to_owned()).font(l.ty.label).color(l.c("text-muted")).wrap());
    if let Some(v) = versions {
        if let Some(before) = &v.before {
            line = line.child(link(&l, "Show the version before", format!("version-before-{}", meta.mission_id), before.clone()));
        }
        if let Some(after) = &v.after {
            line = line.child(link(&l, "Show the edited version", format!("version-after-{}", meta.mission_id), after.clone()));
        }
    }
    Some(line.key(format!("edit-note-{}", meta.mission_id)))
}

/// Over the box while an earlier message is edited: what sending will do, the tick for the files, and Cancel.
pub(super) fn line(s: &State, p: &Palette) -> Option<El> {
    let editing = s.edit.editing.as_ref()?;
    let l = crate::look::controls::Look::new(p.dark);
    let (total, cannot) = match &editing.offer {
        Some(offer) => (offer.files.len() + offer.cannot.len(), offer.cannot.len()),
        None => (0, 0),
    };
    let mut strip = col()
        .key("edit-line")
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .radius(9.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .child(text(edit_earlier::editing_line(total, cannot)).font(type_::LABEL).color(p.muted).w_full().wrap());
    if let Some(offer) = editing.offer.as_ref().filter(|o| o.tickable()) {
        strip = strip.child(
            Checkbox::new(editing.put_back)
                .label(edit_earlier::put_back_label(offer.files.len()))
                .on_toggle(|s: &mut State| {
                    if let Some(e) = s.edit.editing.as_mut() {
                        e.put_back = !e.put_back;
                    }
                })
                .build(l.k)
                .key("edit-put-back"),
        );
    }
    Some(
        strip.child(
            row().gap(8.0).child(
                el().padding(6.0)
                    .child(text("Cancel").font(type_::LABEL).color(p.text))
                    .role("button")
                    .key("edit-cancel")
                    .on_press(|s: &mut State| cancel(s)),
            ),
        ),
    )
}
