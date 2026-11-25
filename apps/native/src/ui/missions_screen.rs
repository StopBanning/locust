//! The Conversations screen (Electron's `MissionsScreen` in `Screens.tsx`): every conversation on one list, a row each
//! standing where its newest turn stood, named by its first; when, who, on what, how many turns and how long, what it
//! cost and where it stands. Opened by Ctrl 1, the palette's "Go to → Conversations", and a teammate's own list.
//!
//! Its checkboxes sit beside each row, never inside it, and pick conversations to delete together: the bar appears
//! once something is picked, Delete asks again before it acts, and each conversation goes through the sidebar's own
//! Delete (`sidebar::delete_conversation`). A conversation still running is never offered, and the set is read again
//! at the moment of deleting, not only at the moment of picking.
use super::*;
use crate::conversation_list::SidebarMission;
use std::collections::{BTreeSet, HashSet};

/// `Screens.tsx` `FILTERS`.
pub(crate) const FILTERS: [&str; 4] = ["All", "Running", "Interrupted", "Completed"];
/// Said on a running conversation's box (Screens.tsx), which is drawn but cannot be ticked.
pub(crate) const STILL_RUNNING: &str = "Still running — stop it first";

#[derive(Default)]
pub(crate) struct Screen {
    pub(crate) open: bool,
    /// Only this teammate's conversations (a teammate id), as a teammate's card asks for; `None` is everyone's.
    pub(crate) owner: Option<String>,
    /// One of `FILTERS`; empty reads as "All".
    pub(crate) filter: String,
    /// Picked conversations, by key (their first turn's id).
    pub(crate) picked: BTreeSet<String>,
    /// Delete was pressed once and now asks "… for good?".
    pub(crate) armed: bool,
    /// The turns the engine is running, read on opening and with each fresh list (never every frame).
    pub(crate) running: HashSet<String>,
}

/// One conversation (`missionsList.ts` `conversationsOf`), from the sidebar's own grouping (`recent_as_missions`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    /// Its first turn's id: the row's key.
    pub(crate) key: String,
    pub(crate) root: String,
    /// Its newest turn, where the row stands and what a press opens.
    pub(crate) leaf: String,
    /// Every turn, oldest first.
    pub(crate) members: Vec<String>,
    pub(crate) title: String,
}

/// Every conversation, a row each, in the order of their newest turns in the listing (newest first).
pub(crate) fn conversations(s: &State) -> Vec<Entry> {
    let rows: Vec<SidebarMission> = recent_as_missions(s);
    let position = |id: &str| s.recent.iter().position(|r| r.id == id).unwrap_or(usize::MAX);
    let mut entries: Vec<(usize, Entry)> = rows
        .into_iter()
        .map(|row| {
            let root = row.root_id.clone().unwrap_or_else(|| row.mission_id.clone());
            let members = if row.member_ids.is_empty() { vec![row.mission_id.clone()] } else { row.member_ids.clone() };
            let newest = members.iter().map(|id| position(id)).min().unwrap_or(usize::MAX);
            (newest, Entry { key: root.clone(), root, leaf: row.mission_id, members, title: row.title })
        })
        .collect();
    entries.sort_by_key(|(at, _)| *at);
    entries.into_iter().map(|(_, entry)| entry).collect()
}

/// `missionRowPhase`: running when the engine runs it now, else what its record says.
pub(crate) fn phase(s: &State, mission_id: &str, running: &HashSet<String>) -> &'static str {
    if running.contains(mission_id) {
        return "running";
    }
    match s.ledger_summaries.get(mission_id) {
        Some(m) if m.cancelled => "cancelled",
        Some(m) if m.failed => "failed",
        // No end in its record and not running now: it stopped short.
        Some(m) if m.running => "interrupted",
        Some(_) => "completed",
        None => match s.recent.iter().find(|r| r.id == mission_id).map(|r| r.state.as_str()) {
            Some("failed") => "failed",
            Some("running") => "interrupted",
            _ => "completed",
        },
    }
}

/// `MissionPhaseView` (status.ts): the label, receipt tone, and uppercase tag for one turn's phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MissionPhaseView {
    pub(crate) label: &'static str,
    pub(crate) tone: &'static str,
    pub(crate) tag: &'static str,
}

/// `missionPhaseView` (status.ts): a single turn's receipt still says Completed in blue unless its record is incomplete.
pub(crate) fn mission_phase_view(phase: &str, has_integrity_issues: bool) -> MissionPhaseView {
    match phase {
        "running" => MissionPhaseView { label: "Running", tone: "live", tag: "RUNNING" },
        "interrupted" => MissionPhaseView { label: "Interrupted", tone: "red", tag: "INTERRUPTED" },
        "failed" => MissionPhaseView { label: "Failed", tone: "red", tag: "FAILED" },
        "cancelled" => MissionPhaseView { label: "Cancelled", tone: "muted", tag: "CANCELLED" },
        _ if has_integrity_issues => MissionPhaseView {
            label: "Completed · receipt incomplete",
            tone: "amber",
            tag: "COMPLETED",
        },
        _ => MissionPhaseView { label: "Completed", tone: "blue", tag: "COMPLETED" },
    }
}

/// `listTone` (status.ts): a finished turn is muted in a list; the ones that need a look keep their colour.
pub(crate) fn list_tone(view: MissionPhaseView) -> &'static str {
    if view.tone == "blue" && view.label == "Completed" {
        "muted"
    } else {
        view.tone
    }
}

/// `missionPhaseView` + `listTone`: the tag's word and its colour token (a settled Completed is quiet in a list).
fn phase_tag(phase: &str, integrity_issues: bool) -> (&'static str, &'static str) {
    let view = mission_phase_view(phase, integrity_issues);
    let color = match list_tone(view) {
        "live" => "text-primary",
        "red" => "red-text",
        "amber" => "amber",
        _ => "text-muted",
    };
    (view.label, color)
}

#[cfg(test)]
pub(crate) fn tag(phase: &str) -> (&'static str, &'static str) {
    phase_tag(phase, false)
}

/// `matchesFilter`.
fn matches(filter: &str, phase: &str) -> bool {
    match filter {
        "Running" => phase == "running",
        "Interrupted" => phase == "interrupted",
        "Completed" => phase == "completed",
        _ => true,
    }
}

/// Whose conversation it is: the first of its turns with an owner (`ownerOfEntry`), a teammate id.
fn owner_of<'a>(s: &'a State, entry: &Entry) -> Option<&'a str> {
    entry.members.iter().find_map(|id| s.mission_owners.get(id)).map(String::as_str)
}

/// What the filters and the owner leave on screen.
pub(crate) fn shown(s: &State, entries: &[Entry], running: &HashSet<String>) -> Vec<Entry> {
    entries
        .iter()
        .filter(|entry| matches(&s.missions.filter, phase(s, &entry.leaf, running)))
        .filter(|entry| s.missions.owner.as_deref().is_none_or(|owner| owner_of(s, entry) == Some(owner)))
        .cloned()
        .collect()
}

/// What may be deleted of what is shown: no conversation any of whose turns runs.
pub(crate) fn deletable(shown: &[Entry], running: &HashSet<String>) -> Vec<Entry> {
    shown.iter().filter(|entry| !entry.members.iter().any(|id| running.contains(id))).cloned().collect()
}

/// Whether the screen can delete at all (Electron's `onDeleteMissions`): with no data folder it offers no selection.
fn can_delete(s: &State) -> bool {
    s.data.is_some() || s.demo
}

pub(crate) fn refresh_running(s: &mut State) {
    s.missions.running = locust_core::engine_api::api().running().into_iter().collect();
}

/// Ctrl 1 and the palette's Conversations: everyone's, or the teammate it was last narrowed to.
pub(crate) fn open(s: &mut State) {
    crate::room_screen::close(s);
    s.home.board_open = false;
    s.home.home_open = false;
    s.home.boot_open = false;
    s.settings_panel.open = false;
    s.people_panel.open = false;
    s.routines.open = false;
    s.benchmarks_open = false;
    s.changes.open = false;
    s.missions.open = true;
    s.missions.armed = false;
    refresh_running(s);
}

/// Only this teammate's conversations (`missionsOwner`), as a teammate's card asks for.
pub(crate) fn open_for(s: &mut State, teammate_id: &str) {
    open(s);
    s.missions.owner = Some(teammate_id.to_owned());
}

pub(crate) fn close(s: &mut State) {
    s.missions.open = false;
    s.missions.armed = false;
}

fn toggle(s: &mut State, key: &str) {
    s.missions.armed = false;
    if !s.missions.picked.remove(key) {
        s.missions.picked.insert(key.to_owned());
    }
}

/// The second press of Delete: only what is still deletable NOW goes, each conversation with every turn, through
/// the sidebar's own delete. A pick the filter hides stays picked; the first refusal is said in its own words.
fn delete_picked(s: &mut State) {
    s.missions.armed = false;
    let mut running = s.missions.running.clone();
    running.extend(locust_core::engine_api::api().running());
    s.missions.running = running.clone();
    let entries = conversations(s);
    let going: Vec<Entry> = deletable(&shown(s, &entries, &running), &running)
        .into_iter()
        .filter(|entry| s.missions.picked.contains(&entry.key))
        .collect();
    let Some(store) = s.data.clone() else {
        s.status = "No native data folder for conversation organization.".into();
        s.status_tone = StatusTone::Amber;
        return;
    };
    let mut refused = None;
    let mut any = false;
    for entry in &going {
        match crate::sidebar::delete_conversation(s, &store, &entry.members) {
            Ok(()) => any = true,
            Err(words) => {
                refused.get_or_insert(words);
            }
        }
        s.missions.picked.remove(&entry.key);
    }
    if any {
        crate::sidebar::after_deleting(s);
        // The list on screen loses what went now, not when the listing is read again.
        let gone: HashSet<&String> = going.iter().flat_map(|e| e.members.iter()).collect();
        let kept: Vec<ConversationRow> = s.recent.iter().filter(|r| !gone.contains(&r.id)).cloned().collect();
        s.recent = kept;
    }
    if let Some(words) = refused {
        s.status = words;
        s.status_tone = StatusTone::Amber;
    }
}

/// The header's one line (`ScreenHeader` meta): how many, the records' damage, what was priced, and how much is listed.
pub(crate) fn meta(s: &State, entries: &[Entry], shown: &[Entry]) -> String {
    let n = entries.len();
    let mut line = format!("{n} {}", if n == 1 { "conversation" } else { "conversations" });
    if let Some(damage) = s.recent_damage.words() {
        line.push_str(" · ");
        line.push_str(&damage);
    }
    let costs: Vec<_> = shown
        .iter()
        .flat_map(|e| e.members.iter())
        .map(|id| s.ledger_summaries.get(id).and_then(|m| m.cost.clone()))
        .collect();
    if let Some(total) = locust_core::benchmark::cost_total(&costs)
        && total.word == "priced"
    {
        line.push_str(&format!(" · {} across {} priced", total.line, total.runs));
    }
    if let Some(listed) = locust_core::native::listed_words(s.recent.len(), s.recent_total) {
        // The sidebar's sentence, inside this line as a clause.
        let mut chars = listed.chars();
        let first = chars.next().map(|c| c.to_lowercase().collect::<String>()).unwrap_or_default();
        line.push_str(&format!(" · {first}{}", chars.as_str()));
    }
    line
}

fn parse_ms(iso: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(iso).ok().map(|t| t.timestamp_millis())
}

/// The row's time column: every turn's span, summed (`durationText`), from its record's first write to its last.
pub(crate) fn elapsed(s: &State, entry: &Entry) -> String {
    let ms: i64 = entry
        .members
        .iter()
        .filter_map(|id| s.ledger_summaries.get(id))
        .map(|m| match (parse_ms(&m.created_at), parse_ms(&m.updated)) {
            (Some(a), Some(b)) => (b - a).max(0),
            _ => 0,
        })
        .sum();
    locust_core::ledger::duration_text(ms.max(0) as u64)
}

/// The counts cell: `3 turns · 1m 39s`, or the time alone for one turn (a count of none says nothing).
pub(crate) fn stats(s: &State, entry: &Entry) -> String {
    let elapsed = elapsed(s, entry);
    if entry.members.len() > 1 { format!("{} turns · {elapsed}", entry.members.len()) } else { elapsed }
}

/// `costGlanceCell`: money, else `free` on a free model, else nothing (counted tokens say nothing).
pub(crate) fn cost_glance_cell(costs: &[Option<locust_core::benchmark::RunCost>], model: &str) -> String {
    // The turns' costs summed (`sumCosts`), said only when that sum is money (`moneyLine`).
    locust_core::benchmark::cost_total(costs)
        .filter(|total| total.word == "priced")
        .map(|total| total.line)
        .unwrap_or_else(|| if model.to_ascii_lowercase().ends_with("-free") { "free".into() } else { String::new() })
}

/// `costGlanceCell`: money, else `free` on a free model, else nothing.
fn cost(s: &State, entry: &Entry) -> String {
    let costs: Vec<_> = entry.members.iter().map(|id| s.ledger_summaries.get(id).and_then(|m| m.cost.clone())).collect();
    let model = s.ledger_summaries.get(&entry.leaf).map(|m| m.model.as_str()).unwrap_or("");
    cost_glance_cell(&costs, model)
}

/// The route as the composer's chip spells it (`routeChrome`).
fn route(s: &State, entry: &Entry) -> String {
    let Some(m) = s.ledger_summaries.get(&entry.leaf) else { return String::new() };
    if m.runtime.is_empty() {
        return String::new();
    }
    let name = locust_core::teammate_tooltip::model_display_name(&m.runtime, &m.model);
    locust_core::teammate_tooltip::route_chrome(&m.runtime, &m.model, &name, " / ")
}

/// The leaf's newest write, for its "when" (`shortAgo`).
fn last_at<'a>(s: &'a State, entry: &Entry) -> Option<&'a str> {
    s.recent.iter().find(|r| r.id == entry.leaf).map(|r| r.updated.as_str())
}

fn open_row(s: &mut State, leaf: &str, tx: &mut Tx) {
    if let Some(row) = s.recent.iter().find(|r| r.id == leaf).cloned() {
        close(s);
        open_recent(s, row, tx);
    }
}

/// The list is narrower than its six fixed columns allow (`.lc-missionrows` below 960 points): rows take two lines.
pub(crate) const NARROW_BELOW: f32 = 960.0;

/// `narrow`: the list was narrower than `NARROW_BELOW` when last laid out (the window reads `missions-rows`).
pub(crate) fn view(s: &State, p: &Palette, narrow: bool) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let running = &s.missions.running;
    let entries = conversations(s);
    let shown = shown(s, &entries, running);
    let deletable = deletable(&shown, running);
    let picked_here: Vec<&Entry> = deletable.iter().filter(|e| s.missions.picked.contains(&e.key)).collect();
    let all_picked = !deletable.is_empty() && picked_here.len() == deletable.len();
    let can_delete = can_delete(s);
    let now_ms = chrono::Utc::now().timestamp_millis();

    let mut body = col().key("missions-screen").w_full().h_full().gap(12.0).padding(16.0);
    body = body.child(
        row()
            .w_full()
            .center_y()
            .gap(12.0)
            .child(text("Conversations".to_owned()).font(type_::TITLE).color(p.text))
            .child(text(meta(s, &entries, &shown)).key("missions-meta").font(l.ty.mono).color(l.c("text-muted")).grow(1.0).truncate()),
    );
    if let Some(owner) = &s.missions.owner {
        let name = s.teammates.iter().find(|t| &t.id == owner).map(|t| t.name.clone()).unwrap_or_else(|| owner.clone());
        body = body.child(
            row().w_full().child(
                filter_chip(&l, &format!("Only {name}’s · Show everyone’s"), true)
                    .key("missions-everyone")
                    .on_press(|s: &mut State| s.missions.owner = None),
            ),
        );
    }
    if !s.recent.is_empty() {
        let mut filters = row().key("missions-filters").gap(6.0);
        for name in FILTERS {
            let on = s.missions.filter.as_str() == name || (name == "All" && s.missions.filter.is_empty());
            filters = filters.child(
                filter_chip(&l, name, on)
                    .key(format!("missions-filter-{}", name.to_lowercase()))
                    .on_press(move |s: &mut State| s.missions.filter = name.to_owned()),
            );
        }
        body = body.child(filters);
    }
    if can_delete && !picked_here.is_empty() {
        let count = picked_here.len();
        let armed = s.missions.armed;
        let delete_words = if armed { format!("Delete {count} for good?") } else { format!("Delete {count}") };
        let select_words = if all_picked { "Select none".to_owned() } else { format!("Select all {}", deletable.len()) };
        let every: Vec<String> = deletable.iter().map(|e| e.key.clone()).collect();
        body = body.child(
            row()
                .key("missions-pickbar")
                .w_full()
                .gap(16.0)
                .px(16.0)
                .py(12.0)
                .center_y()
                .radius(l.t.px("radius-control"))
                .fill(l.c("bg-raised"))
                .border(1.0, l.c("border-hairline"))
                .role("status")
                .child(text(format!("{count} selected")).key("missions-pickbar-count").font(l.ty.body).color(l.c("text-name")))
                .child(
                    text("Clear".to_owned())
                        .key("missions-clear")
                        .font(l.ty.mono)
                        .color(l.c("text-muted"))
                        .role("button")
                        .on_press(|s: &mut State| {
                            s.missions.picked.clear();
                            s.missions.armed = false;
                        }),
                )
                .child(
                    text(select_words)
                        .key("missions-select-all")
                        .font(l.ty.mono)
                        .color(l.c("text-muted"))
                        .role("button")
                        .on_press(move |s: &mut State| {
                            s.missions.armed = false;
                            if all_picked {
                                s.missions.picked.clear();
                            } else {
                                s.missions.picked = every.iter().cloned().collect();
                            }
                        }),
                )
                // Pushed to the far end, away from Clear: not under the pointer that just did something harmless.
                .child(row().grow(1.0))
                .child(
                    row()
                        .key("missions-delete")
                        .px(16.0)
                        .py(4.0)
                        .radius(l.t.px("radius-control"))
                        .border(1.0, l.c("red-border"))
                        .fill(if armed { l.c("red-tint") } else { Color::TRANSPARENT })
                        .role("button")
                        .label(delete_words.clone())
                        .child(text(delete_words).font(l.ty.body).color(l.c("red-text")))
                        .on_press(|s: &mut State| {
                            if s.missions.armed {
                                delete_picked(s);
                            } else {
                                s.missions.armed = true;
                            }
                        }),
                ),
        );
    }

    let list: El = if shown.is_empty() && s.recent.is_empty() {
        col()
            .key("missions-empty")
            .w_full()
            .gap(6.0)
            .padding(32.0)
            .items_center()
            .child(text("No conversations yet".to_owned()).font(l.ty.control).color(p.text))
            .child(
                text("Start one from Home, or pick a teammate on the left. Every conversation is kept here, on this computer.".to_owned())
                    .font(l.ty.body)
                    .color(p.muted)
                    .wrap(),
            )
    } else if shown.is_empty() {
        text("No conversations match this filter.".to_owned()).key("missions-none-match").font(l.ty.body).color(p.muted)
    } else {
        let mut rows = col()
            .key("missions-rows")
            .w_full()
            .radius(12.0)
            .fill(l.c("bg-raised"))
            .border(1.0, l.c("border-hairline"))
            .clip_rounded(12.0);
        for (i, entry) in shown.iter().enumerate() {
            rows = rows.child(mission_row(s, &l, entry, running, can_delete, now_ms, i > 0, narrow));
        }
        rows
    };
    body = body.child(col().key("missions-scroll").w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list));
    body
}

fn filter_chip(l: &crate::look::controls::Look, label: &str, on: bool) -> El {
    row()
        .h(26.0)
        .px(10.0)
        .center_y()
        .radius(l.t.px("radius-control"))
        .fill(if on { l.c("bg-selected") } else { Color::TRANSPARENT })
        .border(1.0, if on { l.c("border-hairline") } else { Color::TRANSPARENT })
        .hover(l.c("hover-surface"))
        .role("button")
        .label(label.to_owned())
        .child(text(label.to_owned()).font(l.ty.body).color(l.c(if on { "text-primary" } else { "text-secondary" })))
}

#[allow(clippy::too_many_arguments)]
fn mission_row(
    s: &State,
    l: &crate::look::controls::Look,
    entry: &Entry,
    running: &HashSet<String>,
    can_delete: bool,
    now_ms: i64,
    ruled: bool,
    narrow: bool,
) -> El {
    let key = entry.key.clone();
    let is_picked = s.missions.picked.contains(&key);
    let live = entry.members.iter().any(|id| running.contains(id));
    let integrity_issues = s.ledger_summaries.get(&entry.leaf).is_some_and(|mission| mission.integrity_issues);
    let (word, ink) = phase_tag(phase(s, &entry.leaf, running), integrity_issues);
    let owner = owner_of(s, entry).and_then(|id| s.teammates.iter().find(|t| t.id == id)).map(|t| t.name.clone());
    let mut wrap = row()
        .key(format!("missions-row-{key}"))
        .w_full()
        .center_y()
        .pl(16.0)
        .fill(if is_picked { l.c("bg-selected") } else { Color::TRANSPARENT })
        .hover(l.c("hover-surface"));
    if ruled {
        wrap = wrap.border_sides([1.0, 0.0, 0.0, 0.0], l.c("border-hairline"));
    }
    // Beside the row, never inside it: the row is a button, and a box inside one is neither valid nor reachable.
    if can_delete {
        let label = if live { format!("{} is still running", entry.title) } else { format!("Select {}", entry.title) };
        let mut boxed = row()
            .key(format!("missions-pick-{key}"))
            .w(14.0)
            .h(14.0)
            
            .center()
            .radius(3.0)
            .border(1.0, if is_picked { l.c("lime") } else { l.c("border-control") })
            .fill(if is_picked { l.c("lime") } else { Color::TRANSPARENT })
            .role("checkbox")
            .label(label)
            .checked(is_picked);
        if is_picked {
            boxed = boxed.child(text("✓".to_owned()).font(l.ty.mono).color(l.c("bg-raised")));
        }
        if live {
            boxed = crate::hover::tip(s, l, &format!("missions-pick-tip-{key}"), STILL_RUNNING, boxed.opacity(0.35).disabled(true));
        } else {
            let key = key.clone();
            boxed = boxed.focusable(true).on_press(move |s: &mut State| toggle(s, &key));
        }
        wrap = wrap.child(boxed);
    }
    let leaf = entry.leaf.clone();
    let cell = |words: String, k: &str, width: f32| {
        text(words).key(format!("missions-{k}-{key}")).font(l.ty.mono).color(l.c("text-muted")).w(width).truncate()
    };
    let dot = row().w(8.0).h(8.0).radius(4.0).fill(l.c(ink));
    let title = text(entry.title.clone())
        .key(format!("missions-title-{key}"))
        .font(l.ty.body)
        .color(l.c("text-primary"))
        .grow(1.0)
        .min_w(0.0)
        .truncate();
    let when = text(crate::rail::short_ago(last_at(s, entry), now_ms).unwrap_or_default())
        .key(format!("missions-when-{key}"))
        .font(l.ty.mono)
        .color(l.c("text-muted"))
        .w(56.0)
        .align(rune::Align::End);
    // The narrow form's third and fourth columns are wider (170/128): the counts and the cost sit under them.
    let (who_w, tag_w) = if narrow { (170.0, 128.0) } else { (130.0, 104.0) };
    let who = text(owner.unwrap_or_else(|| "—".into()))
        .key(format!("missions-owner-{key}"))
        .font(l.ty.body)
        .color(l.c("text-control-fg"))
        .w(who_w)
        .truncate();
    let state = text(word.to_owned())
        .key(format!("missions-tag-{key}"))
        .font(l.ty.body)
        .color(l.c(ink))
        .w(tag_w)
        .align(rune::Align::End)
        .truncate();
    let button = row()
        .key(format!("missions-open-{key}"))
        .grow(1.0)
        .min_w(0.0)
        .px(16.0)
        .py(13.0)
        .role("button")
        .label(entry.title.clone())
        .on_press_tx(move |s: &mut State, tx| open_row(s, &leaf, tx));
    let button = if narrow {
        // Below 960 points of list (shell.css's container query): who and what on the first line, the route, the
        // counts and the cost under them, so the title is never the column squeezed to nothing.
        let first = row().w_full().gap(14.0).center_y().child(dot).child(title).child(when).child(who).child(state);
        let second = row()
            .w_full()
            .gap(14.0)
            .center_y()
            .child(row().w(8.0))
            .child(text(route(s, entry)).key(format!("missions-route-{key}")).font(l.ty.mono).color(l.c("text-muted")).grow(1.0).min_w(0.0).truncate())
            .child(row().w(56.0))
            .child(cell(stats(s, entry), "stats", 170.0))
            .child(cell(cost(s, entry), "cost", 128.0).align(rune::Align::End));
        button.child(col().w_full().min_w(0.0).gap(4.0).child(first).child(second))
    } else {
        button
            .gap(14.0)
            .center_y()
            .child(dot)
            .child(title)
            .child(when)
            .child(who)
            .child(cell(route(s, entry), "route", 190.0))
            .child(cell(stats(s, entry), "stats", 124.0))
            .child(cell(cost(s, entry), "cost", 118.0))
            .child(state)
    };
    wrap.child(button)
}

#[cfg(test)]
#[path = "missions_screen_tests.rs"]
mod missions_screen_tests;

#[cfg(test)]
#[path = "design_pass_holds_tests.rs"]
mod design_pass_holds_tests;

#[cfg(test)]
#[path = "missions_screen_integrity_tests.rs"]
mod integrity_tests;
