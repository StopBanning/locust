//! Ask for a review from a conversation's More actions menu (Electron `App.tsx` `reviewMenuItem`, `reviewPairFor`,
//! `reviewMaterialFor`, `askForReview`, `askTwoForReview`).
use super::*;
use crate::context_menu::Item;
use locust_core::{
    activity_summary::is_edit,
    approval::Mode,
    command_outcomes::is_shell,
    inspector::runtime_display_name,
    path_words::relative_path,
    protocol_tags::shown_text,
    ran_on::{folder_name, ran_on_line},
    review_brief::{Command as ReviewCommand, ReviewCandidate, ReviewMaterial, review_brief, review_pair_of},
    review_menu::{MenuRow, Named, review_menu_item},
    route_at_start::{PickerRoute, StartAs, TeammateRoute, as_reviewer, pick_key_for, start_as},
    shell_words::shell_command_text,
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

fn tool_target<'a>(name: &'a str, summary: &'a str, detail: &'a ledger::ToolDetail) -> &'a str {
    if !detail.arguments.is_empty() && !detail.arguments.starts_with(['{', '[']) {
        &detail.arguments
    } else if !summary.is_empty() {
        summary
    } else {
        name
    }
}

fn edited_paths(items: &[ledger::Item], workspace: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        if let ledger::Item::Tool { name, status, summary, detail } = item
            && is_edit(item)
            && *status != ledger::Status::Failed
        {
            for raw in tool_target(name, summary, detail).lines().map(str::trim).filter(|p| !p.is_empty()) {
                let rel = relative_path(raw, workspace);
                if !out.contains(&rel) {
                    out.push(rel);
                }
            }
        }
    }
    out
}

fn host_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "win32"
    } else {
        "linux"
    }
}

/// Assemble the factual `ReviewMaterial` for the conversation on screen (Electron `App.tsx` `reviewMaterialFor`).
pub(super) fn review_material_for(s: &State) -> Option<ReviewMaterial> {
    let pane = s.ledger.as_ref()?;
    if pane.items.is_empty() {
        return None;
    }
    let last_user_at = pane.items.iter().rposition(|i| matches!(i, ledger::Item::User(_)));
    let (earlier_items, this_turn_items) = match last_user_at {
        Some(at) => (&pane.items[..at], &pane.items[at..]),
        None => (&[][..], pane.items.as_slice()),
    };
    let workspace = pane
        .snapshot
        .turn_metas
        .last()
        .and_then(|m| m.cwd.as_deref())
        .filter(|p| !p.is_empty())
        .or_else(|| pane.header.folder.as_deref().filter(|p| !p.is_empty()))
        .or_else(|| s.home.folder_path.as_deref().filter(|p| !p.is_empty()));

    let changed = edited_paths(this_turn_items, workspace);
    let changed_earlier = edited_paths(earlier_items, workspace);

    let mut commands = Vec::new();
    for item in this_turn_items {
        if let ledger::Item::Tool { name, summary, detail, .. } = item
            && !is_edit(item)
            && is_shell(name)
        {
            let raw = tool_target(name, summary, detail);
            let cleaned = shell_command_text(raw);
            let first = cleaned.lines().next().unwrap_or(raw).trim().to_owned();
            commands.push(ReviewCommand { name: first, exit_code: detail.exit_code });
        }
    }

    let latest_meta = pane.snapshot.turn_metas.last();
    let root_meta = (pane.snapshot.turn_metas.len() > 1).then(|| &pane.snapshot.turn_metas[0]);
    let last_user_text = this_turn_items.iter().find_map(|i| match i {
        ledger::Item::User(t) => Some(t.as_str()),
        _ => None,
    });
    let first_earlier_user = earlier_items.iter().find_map(|i| match i {
        ledger::Item::User(t) => Some(t.as_str()),
        _ => None,
    });
    let run_prompt = latest_meta
        .map(|m| m.prompt.as_str())
        .filter(|p| !p.is_empty())
        .or(last_user_text)
        .unwrap_or("");
    let this_turn_ask = if latest_meta.is_some_and(|m| m.started_by_kind.is_some()) { None } else { Some(run_prompt) };
    let root_ask = root_meta.map(|m| m.prompt.as_str()).filter(|p| !p.is_empty()).or(first_earlier_user);
    let request = this_turn_ask.or(root_ask).unwrap_or(run_prompt).to_owned();
    let opened_with = root_ask
        .zip(this_turn_ask)
        .filter(|(root, this)| root.trim() != this.trim())
        .map(|(root, _)| root.to_owned());

    let mut said_parts = Vec::new();
    for item in this_turn_items {
        if let ledger::Item::Assistant { text, .. } = item {
            let clean = shown_text(text);
            let trimmed = clean.trim();
            if !trimmed.is_empty() {
                said_parts.push(trimmed.to_owned());
            }
        }
    }
    let said = said_parts.join("\n\n");

    let folder = folder_name(workspace);
    let ran_on = ran_on_line(host_platform(), folder.as_deref());
    let author = owner_id(s)
        .and_then(|id| s.teammates.iter().find(|t| t.id == id))
        .map(|t| t.name.clone())
        .unwrap_or_else(|| "A teammate".into());

    Some(ReviewMaterial {
        request,
        opened_with,
        said,
        changed,
        commands,
        changed_earlier,
        ran_on,
        author,
    })
}

/// Build the reviewer's `Send` and `Origin` using `as_reviewer(start_as(...))` (read-only Ask mode for this run).
pub(super) fn review_send_for(s: &State, reviewer: &Teammate, brief: String, antigravity_cli: bool) -> (native::Send, native::Origin) {
    let reviewed = as_reviewer(start_as_for(s, reviewer), antigravity_cli);
    let model = (!reviewed.route.model.is_empty() && reviewed.route.model != "account-default").then_some(reviewed.route.model);
    let job = native::Send {
        teammate: reviewer.clone(),
        conversation: None,
        prompt: brief,
        model,
        effort: reviewed.effort,
    };
    let origin = native::Origin {
        mode: Mode::parse(&reviewed.mode),
        runtime: Some(reviewed.route.runtime),
        account: reviewed.route.account.map(Some),
        ..Default::default()
    };
    (job, origin)
}

#[cfg(not(test))]
fn dispatch_send(job: native::Send, origin: native::Origin) -> Result<engine::Started, String> {
    locust_core::engine_api::api().send(job, origin)
}

#[cfg(test)]
type SendHook = Box<dyn FnMut(native::Send, native::Origin) -> Result<engine::Started, String> + std::marker::Send>;

#[cfg(test)]
static SEND_HOOK: std::sync::Mutex<Option<SendHook>> = std::sync::Mutex::new(None);

#[cfg(test)]
fn dispatch_send(job: native::Send, origin: native::Origin) -> Result<engine::Started, String> {
    if let Some(hook) = SEND_HOOK.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        return hook(job, origin);
    }
    locust_core::engine_api::api().send(job, origin)
}

fn prepare_reviewer_flight(s: &mut State, reviewer: &Teammate, brief: String) {
    s.conversations.remove(&reviewer.id);
    select_teammate(s, &reviewer.id, &mut Tx::default());
    s.own_turn = None;
    s.flight = Some(Flight {
        teammate_id: reviewer.id.clone(),
        conversation_id: None,
        header: Some(reviewer.clone()),
        after_tip: None,
        message: Some(brief),
    });
    set_status(s, &reviewer.id, "Sending…", StatusTone::Muted);
}

/// Start a review turn for one reviewer (Electron `App.tsx` `askForReview`).
pub(super) fn ask_for_review(s: &mut State, reviewer_id: &str) -> bool {
    let Some(reviewer) = s.teammates.iter().find(|t| t.id == reviewer_id).cloned() else {
        return false;
    };
    let Some(material) = review_material_for(s) else {
        return false;
    };
    let brief = review_brief(&material);
    let (job, origin) = review_send_for(s, &reviewer, brief.clone(), false);
    prepare_reviewer_flight(s, &reviewer, brief);
    s.sending.request((job, origin), |(job, origin)| dispatch_send(job, origin));
    true
}

/// Start review turns for a pair of reviewers on two different agents (Electron `App.tsx` `askTwoForReview`).
pub(super) fn ask_two_for_review(s: &mut State, first_id: &str, second_id: &str) -> bool {
    let Some(first) = s.teammates.iter().find(|t| t.id == first_id).cloned() else {
        return false;
    };
    let Some(second) = s.teammates.iter().find(|t| t.id == second_id).cloned() else {
        return false;
    };
    let Some(material) = review_material_for(s) else {
        return false;
    };
    let brief = review_brief(&material);
    let (job_a, origin_a) = review_send_for(s, &first, brief.clone(), false);
    let (job_b, origin_b) = review_send_for(s, &second, brief.clone(), false);
    prepare_reviewer_flight(s, &first, brief);
    s.sending.request(((job_a, origin_a), (job_b, origin_b)), |((job_a, origin_a), (job_b, origin_b))| {
        let first_res = dispatch_send(job_a, origin_a);
        let second_res = dispatch_send(job_b, origin_b);
        if first_res.is_ok() { first_res } else if second_res.is_ok() { second_res } else { first_res }
    });
    true
}

/// Dispatch the header menu's review actions (`header:review:<id>` and `header:review-pair:<a>:<b>`).
pub(super) fn run_action(s: &mut State, action: &str) -> bool {
    if let Some(reviewer_id) = action.strip_prefix("header:review:") {
        ask_for_review(s, reviewer_id);
        return true;
    }
    if let Some(pair) = action.strip_prefix("header:review-pair:") {
        if let Some((a, b)) = pair.split_once(':') {
            ask_two_for_review(s, a, b);
        }
        return true;
    }
    false
}

#[cfg(test)]
#[path = "review_action_tests.rs"]
mod tests;
