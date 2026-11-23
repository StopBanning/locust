//! Ask for a review from a conversation's More actions menu (Electron `App.tsx` `reviewMenuItem`, `reviewPairFor`,
//! `reviewMaterialFor`, `askForReview`, `askTwoForReview`).
use super::*;
use crate::context_menu::Item;
use locust_core::{
    inspector::runtime_display_name,
    review_brief::{ReviewCandidate, review_pair_of},
    review_menu::{MenuRow, Named, review_menu_item},
    route_at_start::{PickerRoute, StartAs, TeammateRoute, pick_key_for, start_as},
};
use std::collections::HashMap;

/// A teammate who could review the conversation on screen, with the agent they would run on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Reviewer {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) runtime: String,
}

impl Named for Reviewer {
    fn name(&self) -> &str {
        &self.name
    }
}

impl ReviewCandidate for Reviewer {
    fn runtime(&self) -> &str {
        &self.runtime
    }
}

/// The teammate who owns the conversation on screen, if any (Electron `ownerOf(liveRun)`).
pub(super) fn owner_id(s: &State) -> Option<String> {
    let pane = s.ledger.as_ref()?;
    pane.snapshot
        .turn_metas
        .iter()
        .rev()
        .find_map(|m| s.mission_owners.get(&m.mission_id))
        .or_else(|| pane.conversation.conversation_id.as_deref().and_then(|id| s.mission_owners.get(id)))
        .cloned()
        .or_else(|| (!pane.header.id.is_empty() && pane.header.id != LEDGER_TARGET).then(|| pane.header.id.clone()))
}

/// The coding agent the conversation on screen ran on (`run.data?.runtime ?? run.runtime`).
pub(super) fn author_runtime(s: &State) -> Option<String> {
    let pane = s.ledger.as_ref()?;
    pane.snapshot
        .turn_metas
        .last()
        .map(|m| m.runtime.as_str())
        .filter(|r| !r.is_empty())
        .or_else(|| pane.header.runtime.as_deref().filter(|r| !r.is_empty()))
        .map(str::to_owned)
}

/// How `teammate` would start a new conversation right now (Electron `startAs(teammate, composer, pickerRoutes)`).
pub(super) fn start_as_for(s: &State, teammate: &Teammate) -> StartAs {
    let saved = teammate.runtime.as_deref().filter(|r| !r.is_empty()).map(|runtime| TeammateRoute {
        runtime: runtime.to_owned(),
        model: teammate.model.clone().unwrap_or_else(|| "account-default".into()),
        mode: if teammate.runs_in.is_empty() { "ask".into() } else { teammate.runs_in.clone() },
        account: teammate.account.clone(),
        effort: None,
    });
    let fallback = PickerRoute {
        runtime: "codex".into(),
        model: "account-default".into(),
        account: None,
    };
    let mut explicit: HashMap<String, PickerRoute> = HashMap::new();
    if let Some(pick) = s.picks.get(&teammate.id) {
        let rt = pick
            .runtime
            .clone()
            .or_else(|| teammate.runtime.clone().filter(|r| !r.is_empty()))
            .unwrap_or_else(|| fallback.runtime.clone());
        explicit.insert(
            pick_key_for(None, Some(&teammate.id)),
            PickerRoute {
                runtime: rt,
                model: pick.model.clone(),
                account: pick.account.clone().flatten(),
            },
        );
    }
    let composer_mode = s.composer_mode.as_deref().unwrap_or("ask");
    start_as(&teammate.id, saved.as_ref(), &fallback, composer_mode, None, &explicit)
}

/// Every teammate who could review this conversation — anyone on the team except its author.
pub(super) fn reviewers_for(s: &State) -> Vec<Reviewer> {
    let author = owner_id(s);
    s.teammates
        .iter()
        .filter(|t| !t.id.is_empty() && t.id != LEDGER_TARGET && author.as_deref() != Some(t.id.as_str()))
        .map(|t| Reviewer {
            id: t.id.clone(),
            name: t.name.clone(),
            runtime: start_as_for(s, t).route.runtime,
        })
        .collect()
}

fn menu_row_to_item(row: MenuRow<String>) -> Item {
    let mut item = Item::new(&row.label, row.on_select.as_deref().unwrap_or(""));
    if row.divider_above {
        item = item.divided();
    }
    if let Some(sub) = row.submenu {
        item = item.list(sub.into_iter().map(menu_row_to_item).collect());
    }
    item
}

/// The "Ask for a review" row for the conversation's More actions menu, or `None` while running or with nobody to ask.
pub(super) fn menu_item(s: &State) -> Option<Item> {
    if s.ledger.is_none() || turn_running(s) {
        return None;
    }
    let reviewers = reviewers_for(s);
    let author_rt = author_runtime(s);
    let pair = review_pair_of(&reviewers, author_rt.as_deref());
    let row = review_menu_item(
        &reviewers,
        pair,
        |one| runtime_display_name(&one.runtime).to_owned(),
        |one| format!("header:review:{}", one.id),
        |a, b| format!("header:review-pair:{}:{}", a.id, b.id),
    )?;
    Some(menu_row_to_item(row))
}
