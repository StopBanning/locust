//! Live and saved voice call pieces, placed among the turns of a conversation.

use super::*;
use locust_core::call_log::{self, Piece, Place};
use locust_core::store::Store;

#[derive(Default)]
pub(super) struct ThreadCalls {
    pub(super) saved: Vec<serde_json::Value>,
    open: std::collections::HashSet<String>,
}

pub(super) fn read(store: &Store, metas: &[ledger::Metadata]) -> Vec<serde_json::Value> {
    let ids: Vec<_> = metas.iter().map(|meta| meta.mission_id.as_str()).collect();
    if ids.is_empty() {
        return Vec::new();
    }
    call_log::list(store, &ids)
}

fn length(call: &serde_json::Value) -> String {
    let start = call["startedAt"].as_f64().unwrap_or_default();
    let stop = call["endedAt"]
        .as_f64()
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis() as f64);
    let seconds = ((stop - start) / 1000.0).round().max(0.0) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

pub(super) fn pieces(s: &State) -> std::collections::BTreeMap<Place<'_>, Vec<Piece<'_>>> {
    let ids: Vec<_> = s
        .ledger
        .as_ref()
        .into_iter()
        .flat_map(|pane| &pane.snapshot.turn_metas)
        .map(|meta| meta.mission_id.as_str())
        .collect();
    let mut calls = call_log::placements(&s.calls.saved, &ids);
    if let Some(live) = crate::voice_panel::live_thread_call(s) {
        for (place, pieces) in call_log::placements(std::slice::from_ref(live), &ids) {
            calls.entry(place).or_default().extend(pieces);
        }
    }
    calls
}

pub(super) fn draw(s: &State, p: &Palette, place: &Place<'_>, pieces: &[Piece<'_>]) -> Vec<El> {
    draw_with_live(s, p, place, pieces, crate::voice_panel::live_thread_call(s))
}

fn draw_with_live(s: &State, p: &Palette, place: &Place<'_>, pieces: &[Piece<'_>], active: Option<&serde_json::Value>) -> Vec<El> {
    pieces
        .iter()
        .filter_map(|piece| {
            let call = piece.call;
            let id = call["callId"].as_str()?;
            let name = call["name"].as_str()?;
            // Disk data cannot impersonate the active call or offer its approval controls.
            let live = active.is_some_and(|active| std::ptr::eq(active, call));
            let open = live || s.calls.open.contains(id);
            let mut view = col().key(format!("call-{id}-{place:?}")).w_full().gap(8.0);
            let duration = length(call);
            if piece.first && live {
                view = view.child(crate::messages::thread_marker(&format!("Call with {name}"), &format!("call-live-{id}"), p));
            } else if piece.first {
                let n = call["lines"].as_array().map_or(0, Vec::len);
                let turns = call["turns"].as_array().map_or(0, Vec::len);
                let mut summary = format!("{n} {} said", if n == 1 { "line" } else { "lines" });
                if turns > 0 {
                    summary.push_str(&format!(
                        " · asked {name} {turns} {}",
                        if turns == 1 { "time" } else { "times" }
                    ));
                }
                let action = if open { "Hide" } else { "Show what was said" };
                let id = id.to_owned();
                view = view.child(
                    row()
                        .key(format!("call-words-{id}"))
                        .role("button")
                        .gap(12.0)
                        .w_full()
                        .padding(10.0)
                        .radius(8.0)
                        .fill(p.raised)
                        .child(
                            col()
                                .grow(1.0)
                                .gap(2.0)
                                .child(
                                    text(format!("Call with {name} · {duration}"))
                                        .font(type_::LABEL)
                                        .color(p.text),
                                )
                                .child(text(summary).font(type_::LABEL).color(p.muted)),
                        )
                        .child(text(action).font(type_::LABEL).color(p.secondary))
                        .on_press(move |s: &mut State| {
                            if !s.calls.open.remove(&id) {
                                s.calls.open.insert(id.clone());
                            }
                        }),
                );
            }
            if open {
                for line in &piece.lines {
                    let words = line["text"].as_str().unwrap_or_default();
                    let (who, tint) = if line["role"] == "you" {
                        ("You", p.selected)
                    } else {
                        (name, p.raised)
                    };
                    view = view.child(
                        col()
                            .w_full()
                            .gap(4.0)
                            .padding(8.0)
                            .radius(8.0)
                            .fill(tint)
                            .child(
                                text(format!("{who} · Said"))
                                    .font(type_::LABEL)
                                    .color(p.muted),
                            )
                            .child(
                                text(words.to_owned())
                                    .font(type_::BODY)
                                    .color(p.text)
                                    .wrap()
                                    .selectable(true),
                            ),
                    );
                }
                if live && piece.last {
                    for role in ["you", "voice"] {
                        if let Some(words) = call["partial"][role].as_str().filter(|words| !words.is_empty()) {
                            view = view.child(text(words.to_owned()).key(format!("call-partial-{id}-{role}"))
                                .font(type_::BODY).color(p.muted).wrap());
                        }
                    }
                    view = view.children(crate::voice_panel::connector_cards(s, p, Some(name)));
                } else if piece.last {
                    view = view.child(crate::messages::thread_marker(&format!("Call ended · {duration}"), &format!("call-ended-{id}"), p));
                }
            }
            if !live && piece.last
                && let Some(summary) = handed_summary(s, call, p)
            {
                view = view.child(summary);
            }
            Some(view)
        })
        .collect()
}

fn todo_status(waiting: bool, live: Option<&str>, record: Option<&str>) -> &'static str {
    if waiting { return "Needs you"; }
    match live.or(record) {
        Some("completed") => "Done",
        Some("failed") => "Failed",
        Some("cancelled" | "interrupted") => "Stopped",
        Some("running" | "cancelling") => "Working",
        _ => "Starting",
    }
}

fn handed_summary(s: &State, call: &serde_json::Value, p: &Palette) -> Option<El> {
    let handed = call["handed"]
        .as_array()
        .filter(|handed| !handed.is_empty())?;
    let on_screen = |id: &str| {
        s.ledger.as_ref().is_some_and(|pane| {
            pane.snapshot
                .turn_metas
                .iter()
                .any(|meta| meta.mission_id == id)
        })
    };
    if handed.len() == 1 && handed[0]["missionId"].as_str().is_some_and(on_screen) {
        return None;
    }
    let count = handed.len();
    let title = format!(
        "From the call · {count} to-do{}",
        if count == 1 { "" } else { "s" }
    );
    let mut list = col()
        .key(format!("call-handoffs-{}", call["callId"].as_str()?))
        .w_full()
        .gap(8.0)
        .padding(10.0)
        .radius(8.0)
        .fill(p.raised)
        .child(text(title).font(type_::LABEL).color(p.text));
    for hand in handed {
        let (Some(id), Some(name), Some(task)) = (
            hand["missionId"].as_str(),
            hand["name"].as_str(),
            hand["task"].as_str(),
        ) else {
            continue;
        };
        let live = s.ledger.as_ref().filter(|pane| pane.snapshot.tip == id && pane.snapshot.running)
            .map(|_| if s.stopping.is_pending() { "cancelling" } else { "running" })
            .or_else(|| s.flight.as_ref().filter(|flight| flight.conversation_id.as_deref() == Some(id)).map(|_| "starting"));
        let record = s.recent.iter().find(|row| row.id == id).map(|row| row.state.as_str())
            .or_else(|| s.ledger_summaries.get(id).map(|row| {
                if row.running { "running" }
                else if row.cancelled { "cancelled" }
                else if row.failed { "failed" }
                else { "completed" }
            }));
        let status = todo_status(s.home.waiting_for.contains_key(id), live, record);
        let color = match status {
            "Needs you" => p.amber,
            "Working" => p.secondary,
            "Failed" => p.red,
            "Done" => p.green,
            _ => p.muted,
        };
        let mut row = row()
            .key(format!("call-todo-{id}"))
            .w_full()
            .gap(8.0)
            .child(text(name.to_owned()).font(type_::LABEL).color(p.text))
            .child(
                text(task.to_owned())
                    .grow(1.0)
                    .font(type_::LABEL)
                    .color(p.text)
                    .wrap(),
            )
            .child(text(status).font(type_::LABEL).color(color));
        if !on_screen(id) {
            let label = format!("Open {name}’s conversation: {task}");
            let id = id.to_owned();
            row = row.child(
                text("Open")
                    .key(format!("call-todo-open-{id}"))
                    .role("button")
                    .label(label)
                    .font(type_::LABEL)
                    .color(p.secondary)
                    .on_press_tx(move |s: &mut State, tx: &mut Tx| {
                        peer_thread::open_mission_by_id(s, id.clone(), tx)
                    }),
            );
        }
        list = list.child(row);
    }
    Some(list)
}

#[cfg(test)]
#[path = "call_words_tests.rs"]
mod tests;
