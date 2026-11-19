//! TurnUndo.tsx (0.674, 0.840): the way back from what one finished turn changed, drawn under its files card. Asked
//! first, in words, because it changes files; afterwards it says what it did and names each file it left alone. A turn
//! with no way back (it changed nothing, it shared its folder with another run, it has no record) draws nothing.
use super::*;
use locust_core::checkpoints::{self, UndoState};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// `.lc-turnundo__ask` and its two answers: quiet words that press.
fn button(label: &str, p: &Palette) -> El {
    text(label.to_owned()).font(type_::LABEL).color(p.secondary).padding(6.0).role("button").label(label.to_owned())
}

/// What each turn of the open conversation may offer, by mission id, and which one is being asked about.
#[derive(Default)]
pub(super) struct Panel {
    pub(super) states: HashMap<String, UndoState>,
    pub(super) asking: Option<String>,
    asked: Option<(PathBuf, Vec<String>, bool)>,
    tries: u32,
    last_ask: Option<Instant>,
}

/// The folder and each asked turn's state.
pub(super) type Answer = (PathBuf, Vec<(String, UndoState)>);

fn folder(s: &State) -> Option<PathBuf> {
    selected_conversation(s).and_then(|(t, _)| t.folder.as_deref()).map(PathBuf::from)
}

fn turns(s: &State) -> Vec<String> {
    s.ledger.as_ref().map(|pane| pane.snapshot.turn_metas.iter().map(|m| m.mission_id.clone()).collect()).unwrap_or_default()
}

fn ask(s: &mut State, folder: PathBuf, ids: Vec<String>) {
    s.turn_undo.last_ask = Some(Instant::now());
    let store = locust_core::native::store();
    s.turn_undo_job.request((folder, ids, store), |(folder, ids, store)| {
        let states = ids
            .into_iter()
            .map(|id| {
                let state = store.as_ref().map_or(UndoState::None, |store| checkpoints::undo_state(store, &folder, &id));
                (id, state)
            })
            .collect();
        (folder, states)
    });
}

/// Ask again whenever the folder, the turns or whether one is running change. A turn reads as over a moment before
/// its record is written, so the latest turn is asked again every 750 ms, up to 20 times, while it offers nothing.
pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    if let Some((folder, states)) = s.turn_undo_job.take()
        && s.turn_undo.asked.as_ref().is_some_and(|(f, _, _)| *f == folder)
    {
        s.turn_undo.tries += 1;
        s.turn_undo.states.extend(states);
        changed = true;
    }
    let Some(folder) = folder(s) else { return changed };
    let ids = turns(s);
    let running = turn_running(s);
    let key = (folder.clone(), ids.clone(), running);
    if s.turn_undo.asked.as_ref() != Some(&key) {
        s.turn_undo.asked = Some(key);
        s.turn_undo.tries = 0;
        if !ids.is_empty() {
            ask(s, folder, ids);
        }
        return changed;
    }
    let latest_offers_nothing = ids.last().is_some_and(|id| matches!(s.turn_undo.states.get(id), None | Some(UndoState::None)));
    let due = s.turn_undo.last_ask.is_none_or(|at| at.elapsed() >= Duration::from_millis(750));
    if !running && latest_offers_nothing && s.turn_undo.tries < 20 && due && !s.turn_undo_job.is_pending() {
        ask(s, folder, ids.last().cloned().into_iter().collect());
    }
    changed
}

/// Undo answered: the turn now says what it did, and no longer asks.
pub(super) fn heard(s: &mut State, run: &str, receipt: &serde_json::Value) {
    s.turn_undo.states.insert(run.to_owned(), UndoState::from_receipt(receipt));
    if s.turn_undo.asking.as_deref() == Some(run) {
        s.turn_undo.asking = None;
    }
}

/// Whether Undo put this turn's files back, for the card's heading ("· undone").
pub(super) fn put_back(s: &State, mission: &str) -> bool {
    s.turn_undo.states.get(mission).is_some_and(UndoState::put_back)
}

/// TurnUndo's view for one finished turn, keyed under `scope`.
pub(super) fn view(s: &State, mission: &str, scope: &str, p: &Palette) -> Option<El> {
    match s.turn_undo.states.get(mission)? {
        UndoState::None | UndoState::Shared => None,
        UndoState::Undone { restored, left_alone } => {
            let said = match restored.len() {
                0 => "Nothing was put back.".to_owned(),
                1 => "Undone: 1 file put back as it was before this turn.".to_owned(),
                n => format!("Undone: {n} files put back as they were before this turn."),
            };
            let mut done = col().w_full().gap(4.0).key(format!("{scope}/undo")).role("status").child(text(said).font(type_::LABEL).color(p.muted).w_full().wrap());
            for (path, why) in left_alone {
                done = done.child(
                    row()
                        .w_full()
                        .gap(4.0)
                        .child(text(path.clone()).font(type_::LABEL.mono()).color(p.muted).wrap_anywhere())
                        .child(text(format!("left as it is: {why}")).font(type_::LABEL).color(p.muted).shrink().wrap()),
                );
            }
            Some(done)
        }
        UndoState::Ready { files, not_kept } => {
            let run = mission.to_owned();
            if s.turn_undo.asking.as_deref() != Some(mission) {
                return Some(
                    row().w_full().key(format!("{scope}/undo")).child(
                        button("Undo these changes", p).key(format!("{scope}/undo-ask")).on_press(move |s: &mut State| s.turn_undo.asking = Some(run.clone())),
                    ),
                );
            }
            let files = if *files == 1 { "1 file".to_owned() } else { format!("{files} files") };
            let too_large = if not_kept.is_empty() { String::new() } else { format!(" Too large to have been kept: {}.", not_kept.join(", ")) };
            let working = s.undo_job.is_pending();
            let folder = folder(s);
            Some(
                col()
                    .w_full()
                    .gap(6.0)
                    .key(format!("{scope}/undo"))
                    .role("group")
                    .label("Undo this turn's changes")
                    .child(text(format!("Put back the {files} this turn changed, as they were before it? A file changed since is left as it is.{too_large}")).font(type_::LABEL).color(p.text).w_full().wrap())
                    .child(
                        row()
                            .gap(6.0)
                            .child(button(if working { "Undoing\u{2026}" } else { "Undo" }, p).key(format!("{scope}/undo-go")).on_press(move |s: &mut State| {
                                if let Some(folder) = folder.clone() {
                                    crate::changes_panel::undo_turn_in(s, folder, run.clone());
                                }
                            }))
                            .child(button("Keep them", p).key(format!("{scope}/undo-keep")).on_press(|s: &mut State| s.turn_undo.asking = None)),
                    ),
            )
        }
    }
}

#[cfg(test)]
#[path = "turn_undo_tests.rs"]
mod tests;
