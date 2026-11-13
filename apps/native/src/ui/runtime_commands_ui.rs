//! Each runtime's own slash commands in the `/` menu (Electron 0.426, `slashCommands.ts` `runtimeSlashCommands`), and
//! Compact now from the context ring (0.797, `compactNow.ts`, `ContextRing.tsx`).
//!
//! The lists are kept by the engine as each runtime tells them (`locust_core::runtime_commands`); the window reads them
//! in a job at start and again whenever a turn ends, never while drawing.

use super::*;
use locust_core::compact_now::{self, CompactOffer, OfferInput};
use locust_core::runtime_commands::CommandInfo;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Commands {
    pub by_runtime: BTreeMap<String, Vec<CommandInfo>>,
    pub asked: bool,
    pub was_running: bool,
    /// The context ring's menu is open.
    pub ring_open: bool,
}

/// Read the lists at start and after every turn ends.
pub(super) fn poll(s: &mut State) -> bool {
    let mut changed = false;
    if let Some(lists) = s.runtime_commands_job.take() {
        s.runtime_commands.by_runtime = lists;
        changed = true;
    }
    let running = turn_running(s);
    let ended = s.runtime_commands.was_running && !running;
    s.runtime_commands.was_running = running;
    if (!s.runtime_commands.asked || ended) && !s.runtime_commands_job.is_pending()
        && let Some(store) = s.data.clone()
    {
        s.runtime_commands.asked = true;
        s.runtime_commands_job.request(store, |store| locust_core::runtime_commands::for_store(&store).list());
    }
    changed
}

/// The conversation's runtime: the one its last turn ran on, else the teammate's.
fn conversation_runtime(s: &State) -> Option<String> {
    selected_conversation(s).and_then(|(t, _)| t.runtime.clone())
}

fn runtime_name(runtime: &str) -> &'static str {
    crate::compare_view::runtime_display_name(runtime)
}

/// The menu rows for the conversation's runtime's own commands.
pub(super) fn menu_rows(s: &State) -> Vec<composer::SlashCommand> {
    let Some(runtime) = conversation_runtime(s) else { return Vec::new() };
    let Some(rows) = s.runtime_commands.by_runtime.get(&runtime) else { return Vec::new() };
    let rows: Vec<(String, String, String)> = rows.iter().map(|c| (c.name.clone(), c.description.clone(), c.argument_hint.clone())).collect();
    composer::runtime_slash_commands(runtime_name(&runtime), &rows)
}

/// Writing a runtime command's arguments (`/name ` and what follows): the menu stays shut so Enter sends.
pub(super) fn writing_arguments(s: &State) -> bool {
    let text = s.draft.text.trim_start();
    let Some(rest) = text.strip_prefix('/') else { return false };
    let name: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
    rest.len() > name.len() && menu_rows(s).iter().any(|c| c.name.eq_ignore_ascii_case(&name))
}

/// What Compact now offers for this conversation, or nothing (the ring then stays a ring).
pub(super) fn offer(s: &State) -> Option<(CompactOffer, &'static str)> {
    let runtime = conversation_runtime(s)?;
    let name = runtime_name(&runtime);
    // The route chip's runtime for the next message: another agent picked for this turn, else the conversation's.
    let teammate = selected_conversation(s).map(|(t, _)| t.id.clone())?;
    let next = s.picks.get(&teammate).and_then(|p| p.runtime.clone()).unwrap_or_else(|| runtime.clone());
    let offer = compact_now::compact_offer(&OfferInput {
        runtime: Some(&runtime),
        next_runtime: &next,
        runtime_name: name,
        commands: &s.runtime_commands.by_runtime,
        running: turn_running(s),
        queued: crate::composer_wiring::queue_front(s).is_some(),
        elsewhere: s.compare.composer_mode != crate::compare_view::CompareComposerMode::Direct || s.edit.editing.is_some(),
    })?;
    Some((offer, name))
}

/// Compact now: `/compact` as the conversation's next message, exactly what typing it would send, with whatever is in
/// the box kept; while a turn runs it waits in line instead.
pub(super) fn compact(s: &mut State, tx: &mut Tx) {
    let Some((offer, _)) = offer(s) else { return };
    s.runtime_commands.ring_open = false;
    let kept = s.draft.text.clone();
    match offer {
        CompactOffer::Refused(_) => {}
        CompactOffer::AfterTurn => {
            crate::composer_wiring::enqueue_follow_up(s, "/compact".into(), Vec::new());
        }
        CompactOffer::Ready => {
            s.draft.replace_text("/compact");
            crate::send(s, tx);
            if !kept.is_empty() {
                s.draft.replace_text(kept);
            }
        }
    }
}

/// The ring's menu: what it says, and Compact now beneath (`ContextRing.tsx`).
pub(super) fn menu(s: &State, p: &Palette) -> Option<El> {
    if !s.runtime_commands.ring_open {
        return None;
    }
    let (offer, name) = offer(s)?;
    let l = crate::look::controls::Look::new(p.dark);
    let reading = s.ledger.as_ref()?.snapshot.context.as_ref()?;
    let refused = matches!(offer, CompactOffer::Refused(_));
    let mut item = col()
        .key("compact-now")
        .gap(2.0)
        .padding(8.0)
        .radius(6.0)
        .role("button")
        .label(compact_now::item_name(&offer))
        .child(text(compact_now::item_name(&offer)).font(l.ty.control).color(if refused { l.c("text-muted") } else { l.c("text-primary") }))
        .child(text(compact_now::item_detail(&offer, name)).font(type_::LABEL).color(l.c("text-secondary")).wrap());
    if !refused {
        item = item.hover(l.c("hover-surface")).on_press_tx(compact);
    }
    Some(
        col()
            .key("context-ring-menu")
            .w(300.0)
            .gap(6.0)
            .padding(8.0)
            .radius(l.t.px("radius-card"))
            .fill(l.c("bg-raised"))
            .border(1.0, l.c("border-card"))
            .child(text(reading.sentence()).font(type_::LABEL).color(l.c("text-secondary")).wrap().px(8.0))
            .child(item),
    )
}
