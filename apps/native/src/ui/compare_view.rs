//! COMPARE & BLIND, AND THE JUDGES (Electron `CompareView.tsx` + `main/index.ts` compare handlers).

use super::*;
use locust_core::compare::{
    ChangesIn, Compare, CompareRoute, CompareSlotId, CreateCompareInput, JudgePanelMember,
    MAX_COMPARE_SLOTS, MIN_COMPARE_SLOTS, blind_name, changes_line, compare_refusal_of,
    compare_slot_key, has_column_instructions, judge_pick_of, newest_judge_mission_ids,
    shuffled_columns,
};
use locust_core::compare_copies::{
    bring_in_compare_worktree, bring_in_copy, compare_root, copy_diff, copy_line_changes,
    copy_refusal, discard_compare_worktrees, ensure_compare_worktree, git_head, is_git_repo,
    make_compare_copy, remove_compare_copies,
};
use locust_core::compare_judge::{
    JudgeChangesKind, JudgedAnswer, JudgedCheck, JudgedCheckOutcome, build_judge_brief,
};
use locust_core::compare_score::{
    Scorecard, criteria_lines, criteria_weights, drafted_points, panel_pick, panel_vote,
    parse_scorecard, points_draft_request, score_of, weighted, without_scorecard,
};
use locust_core::ledger;
use locust_core::native::{self, Origin, SlotOrigin};
use locust_core::routine_copy::BringIn;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum CompareComposerMode {
    #[default]
    Direct,
    Compare,
    Blind,
}

impl CompareComposerMode {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Direct => "Direct",
            Self::Compare => "Compare",
            Self::Blind => "Blind",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ComparePick {
    pub(super) runtime: String,
    pub(super) model: String,
    pub(super) effort: Option<String>,
    pub(super) label: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct ModelChoice {
    pub(super) key: String,
    pub(super) runtime: String,
    pub(super) model: String,
    pub(super) label: String,
    pub(super) is_free: bool,
}

#[derive(Default)]
pub(super) struct CompareState {
    pub(super) composer_mode: CompareComposerMode,
    pub(super) mode_menu: bool,
    pub(super) picks: Vec<ComparePick>,
    pub(super) changes: bool,
    pub(super) effort_menu: Option<usize>,
    pub(super) model_menu: Option<usize>,
    pub(super) compares: Vec<Compare>,
    pub(super) loaded: bool,
    pub(super) active_id: Option<String>,
    pub(super) problem: Option<String>,
    pub(super) snapshots: HashMap<String, ledger::Snapshot>,
    pub(super) criteria: TextEdit,
    pub(super) three_judges: bool,
    pub(super) judge_picks: [Option<String>; 3],
    pub(super) drafting: bool,
    pub(super) draft_mission_id: Option<String>,
}

pub(super) fn runtime_display_name(runtime: &str) -> &'static str {
    match runtime {
        "claude" => "Claude Code",
        "codex" => "Codex CLI",
        "opencode" => "OpenCode",
        "cursor" => "Cursor",
        "copilot" => "Copilot CLI",
        "antigravity" => "Antigravity",
        "muse" => "Muse Code",
        "kilo" => "Kilo",
        "acp" => "ACP",
        _ => "Agent",
    }
}

pub(super) fn joined_names(names: &[String]) -> String {
    match names.len() {
        0 => String::new(),
        1 => names[0].clone(),
        _ => format!(
            "{} and {}",
            names[..names.len() - 1].join(", "),
            names[names.len() - 1]
        ),
    }
}

/// Port of `threeJudges` in `CompareView.tsx`.
pub(super) fn three_judges(
    choices: &[ModelChoice],
    picked: &[Option<String>; 3],
    count: usize,
) -> Vec<String> {
    let mut chosen: Vec<String> = Vec::new();
    for index in 0..count {
        let want = picked.get(index).and_then(|o| o.as_ref());
        let free = |one: &ModelChoice| -> bool {
            !chosen.contains(&one.key)
                && !picked[index + 1..]
                    .iter()
                    .flatten()
                    .any(|p| p == &one.key)
        };
        let offered = (index > 0)
            .then(|| choices.iter().find(|one| free(one) && one.is_free))
            .flatten()
            .or_else(|| choices.iter().find(|one| free(one)))
            .or_else(|| choices.iter().find(|one| !chosen.contains(&one.key)));
        let held = if let Some(w) = want
            && choices.iter().any(|one| &one.key == w)
            && !chosen.contains(w)
        {
            Some(w.clone())
        } else {
            offered.map(|o| o.key.clone())
        };
        if let Some(h) = held {
            chosen.push(h);
        }
    }
    chosen
}

/// Capitalize effort name (`"high"` -> `"High"`).
fn effort_title(effort: &str) -> String {
    let mut chars = effort.chars();
    match chars.next() {
        Some(c) => format!("{}{}", c.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

/// Effective effort for a pick against the model catalog (`each-compared-model-has-its-own-effort`).
pub(super) fn effective_effort(pick: &ComparePick, models: &[locust_core::engine::Models]) -> Option<String> {
    let entry = models
        .iter()
        .find(|m| m.runtime == pick.runtime)
        .and_then(|m| m.models.iter().find(|model| model.id == pick.model));
    let supported = entry.map(|m| m.efforts.as_slice()).unwrap_or(&[]);
    if supported.is_empty() && pick.effort.is_none() {
        if pick.runtime == "cursor" {
            for level in ["low", "medium", "high", "xhigh", "max"] {
                if pick.model.ends_with(&format!("-{level}")) {
                    return Some(level.to_owned());
                }
            }
        }
        return None;
    }
    if let Some(e) = &pick.effort {
        return Some(e.clone());
    }
    supported.first().cloned()
}

pub(super) fn effort_aria_label(
    slot_index: usize,
    pick: &ComparePick,
    models: &[locust_core::engine::Models],
) -> Option<String> {
    let eff = effective_effort(pick, models)?;
    let letter = ["A", "B", "C"].get(slot_index).copied().unwrap_or("A");
    Some(format!("Effort for model {letter}: {}", effort_title(&eff)))
}

fn available_choices(s: &State) -> Vec<ModelChoice> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for agent in s.models.values().flatten() {
        for m in &agent.models {
            let key = format!("{}:{}", agent.runtime, m.id);
            if seen.insert(key.clone()) {
                let label = if m.name.is_empty() {
                    m.id.clone()
                } else {
                    m.name.clone()
                };
                let is_free = m.id.ends_with("-free") || label.to_lowercase().contains("free");
                out.push(ModelChoice {
                    key,
                    runtime: agent.runtime.clone(),
                    model: m.id.clone(),
                    label,
                    is_free,
                });
            }
        }
    }
    if out.is_empty() {
        // Fallback to teammates' routes
        for t in &s.teammates {
            let rt = t.runtime.as_deref().unwrap_or("codex");
            let model = t.model.as_deref().unwrap_or("account-default");
            let key = format!("{rt}:{model}");
            if seen.insert(key.clone()) {
                out.push(ModelChoice {
                    key,
                    runtime: rt.to_owned(),
                    model: model.to_owned(),
                    label: format!("{} ({})", runtime_display_name(rt), model),
                    is_free: false,
                });
            }
        }
    }
    out
}

pub(super) fn ensure_default_picks(s: &mut State) {
    if s.compare.picks.len() >= MIN_COMPARE_SLOTS {
        return;
    }
    let choices = available_choices(s);
    for c in choices {
        if s.compare.picks.len() >= MIN_COMPARE_SLOTS {
            break;
        }
        if !s
            .compare
            .picks
            .iter()
            .any(|p| p.runtime == c.runtime && p.model == c.model)
        {
            s.compare.picks.push(ComparePick {
                runtime: c.runtime,
                model: c.model,
                effort: None,
                label: Some(c.label),
            });
        }
    }
    while s.compare.picks.len() < MIN_COMPARE_SLOTS {
        let rt = selected_conversation(s)
            .and_then(|(t, _)| t.runtime.clone())
            .unwrap_or_else(|| "codex".into());
        let model = if s.compare.picks.is_empty() {
            "gpt-6".to_owned()
        } else {
            "sonnet".to_owned()
        };
        let rt_pick = if s.compare.picks.is_empty() {
            rt
        } else {
            "claude".to_owned()
        };
        s.compare.picks.push(ComparePick {
            runtime: rt_pick,
            model,
            effort: None,
            label: None,
        });
    }
}

fn active_folder(s: &State) -> Option<std::path::PathBuf> {
    selected_conversation(s)
        .and_then(|(t, _)| t.folder.as_deref())
        .or(s.home.folder_path.as_deref())
        .filter(|f| !f.is_empty())
        .map(std::path::PathBuf::from)
}

fn synthetic_teammate(s: &State, teammate_id: Option<&str>, route: &CompareRoute) -> locust_core::engine::Teammate {
    if let Some(tid) = teammate_id
        && let Some(t) = s.teammates.iter().find(|t| t.id == tid)
    {
        return locust_core::engine::Teammate {
            id: t.id.clone(),
            name: t.name.clone(),
            role: t.role.clone(),
            runtime: Some(route.runtime.clone()),
            model: Some(route.model.clone()),
            busy: false,
            runs_in: t.runs_in.clone(),
            folder: t.folder.clone(),
        };
    }
    locust_core::engine::Teammate {
        id: "nobody".into(),
        name: route.label.clone().unwrap_or_else(|| route.model.clone()),
        role: String::new(),
        runtime: Some(route.runtime.clone()),
        model: Some(route.model.clone()),
        busy: false,
        runs_in: "ask".into(),
        folder: active_folder(s).map(|p| p.to_string_lossy().into_owned()),
    }
}

pub(super) fn start_comparison(s: &mut State) {
    let prompt = s.draft.text.trim().to_owned();
    if prompt.is_empty() {
        s.compare.problem = Some("There is nothing to compare yet. Write the ask first.".into());
        return;
    }
    let Some(store) = s.data.clone() else {
        s.compare.problem = Some("Native data folder is not ready.".into());
        return;
    };
    ensure_default_picks(s);
    if !(MIN_COMPARE_SLOTS..=MAX_COMPARE_SLOTS).contains(&s.compare.picks.len()) {
        s.compare.problem = Some(format!(
            "Pick {MIN_COMPARE_SLOTS} or {MAX_COMPARE_SLOTS} different models to compare."
        ));
        return;
    }
    let blind = s.compare.composer_mode == CompareComposerMode::Blind;
    let changes = s.compare.changes;
    let auto_on = locust_core::workspace_settings::load(&store)
        .map(|ws| ws.auto_mode)
        .unwrap_or(false);
    let mode_auto = s.composer_mode.as_deref() == Some("auto") && auto_on;

    let mut routes: Vec<CompareRoute> = s
        .compare
        .picks
        .iter()
        .map(|p| CompareRoute {
            runtime: p.runtime.clone(),
            model: p.model.clone(),
            account: None,
            effort: p.effort.clone(),
            label: p.label.clone(),
            mode: if changes && mode_auto {
                Some("auto".into())
            } else {
                None
            },
            instructions: None,
            why: None,
        })
        .collect();

    if blind {
        let mut seed = locust_core::invariants::unique_nanos() as usize;
        routes = shuffled_columns(&routes, |upper| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 16) % upper
        });
    }

    let folder = active_folder(s);
    let (changes_in, base) = if changes {
        if let Some(ref f) = folder {
            if is_git_repo(f) {
                (None, git_head(f))
            } else if copy_refusal(f, None).is_none() {
                (Some(ChangesIn::Copy), None)
            } else {
                (Some(ChangesIn::Folder), None)
            }
        } else {
            (Some(ChangesIn::Folder), None)
        }
    } else {
        (None, None)
    };

    let teammate_id = selected_conversation(s).map(|(t, _)| t.id.clone());
    let Ok(cmp_store) = locust_core::compare::CompareStore::from_store(&store) else {
        s.compare.problem = Some("Could not open comparisons store.".into());
        return;
    };
    let created = match cmp_store.create(CreateCompareInput {
        teammate_id: teammate_id.clone(),
        prompt: prompt.clone(),
        routes,
        changes,
        changes_in,
        blind,
        base,
        rerun_of: None,
    }) {
        Ok(c) => c,
        Err(e) => {
            s.compare.problem = Some(e);
            return;
        }
    };

    s.draft.clear();
    s.compare.problem = None;
    let copy_root = compare_root(&store);
    let mut current = created;

    for col in current.slots.clone() {
        if let Some(reason) = compare_refusal_of(&col.route.runtime, true) {
            if let Ok(updated) = cmp_store.refuse(&current.compare_id, col.slot, reason) {
                current = updated;
            }
            continue;
        }
        let cwd: Option<std::path::PathBuf> = if current.changes {
            if current.changes_in == Some(ChangesIn::Folder) {
                folder.clone()
            } else if current.changes_in == Some(ChangesIn::Copy) {
                match folder.as_deref() {
                    Some(f) => match make_compare_copy(f, &current.compare_id, col.slot, &copy_root, None) {
                        Ok(p) => Some(p),
                        Err(e) => {
                            if let Ok(updated) = cmp_store.refuse(&current.compare_id, col.slot, &e) {
                                current = updated;
                            }
                            continue;
                        }
                    },
                    None => None,
                }
            } else {
                match folder.as_deref() {
                    Some(f) => match ensure_compare_worktree(
                        f,
                        &current.compare_id,
                        col.slot,
                        &copy_root,
                        current.base.as_deref(),
                    ) {
                        Ok(p) => Some(p),
                        Err(e) => {
                            if let Ok(updated) = cmp_store.refuse(&current.compare_id, col.slot, &e) {
                                current = updated;
                            }
                            continue;
                        }
                    },
                    None => None,
                }
            }
        } else {
            folder.clone()
        };

        let mode = if !current.changes {
            locust_core::approval::Mode::Ask
        } else if col.route.mode.as_deref() == Some("auto") && auto_on {
            locust_core::approval::Mode::Auto
        } else {
            locust_core::approval::Mode::AcceptEdits
        };

        let slot_key = compare_slot_key(current.teammate_id.as_deref(), &current.compare_id, col.slot);
        let tm = synthetic_teammate(s, current.teammate_id.as_deref(), &col.route);
        let model_opt = if col.route.model == "account-default" {
            None
        } else {
            Some(col.route.model.clone())
        };
        let res = locust_core::engine_api::api().send(
            native::Send {
                teammate: tm,
                conversation: None,
                prompt: prompt.clone(),
                model: model_opt,
                effort: col.route.effort.clone(),
            },
            Origin {
                mode: Some(mode),
                folder: cwd.clone(),
                runtime: Some(col.route.runtime.clone()),
                slot: Some(SlotOrigin {
                    key: slot_key,
                    cwd,
                    instructions: col.route.instructions.clone(),
                    sealed: current.blind,
                }),
                ..Default::default()
            },
        );
        match res {
            Ok(started) => {
                if let Ok(updated) = cmp_store.add_turn(&current.compare_id, col.slot, &started.conversation_id) {
                    current = updated;
                }
            }
            Err(why) => {
                if let Ok(updated) = cmp_store.refuse(&current.compare_id, col.slot, &why) {
                    current = updated;
                }
            }
        }
    }

    s.compare.active_id = Some(current.compare_id.clone());
    s.compare.loaded = false;
    reload_compares(s);
}

pub(super) fn keep_column(s: &mut State, slot: CompareSlotId) {
    let Some(store) = s.data.clone() else { return };
    let Some(cmp_id) = s.compare.active_id.clone() else { return };
    let Ok(cmp_store) = locust_core::compare::CompareStore::from_store(&store) else { return };
    let Ok(Some(cmp)) = cmp_store.get(&cmp_id) else { return };
    if cmp.kept.is_some() {
        return;
    }
    let copy_root = compare_root(&store);
    let folder = active_folder(s);
    let mut brought: Option<Vec<String>> = None;

    if cmp.changes && cmp.changes_in != Some(ChangesIn::Folder)
        && let Some(ref f) = folder
    {
        let res = if cmp.changes_in == Some(ChangesIn::Copy) {
            bring_in_copy(f, &cmp.compare_id, slot, &copy_root)
        } else {
            bring_in_compare_worktree(f, &cmp.compare_id, slot, &copy_root, cmp.base.as_deref())
        };
        match res {
            Ok(BringIn::YourChanges(files)) => {
                let list = files.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
                s.compare.problem = Some(format!(
                    "You have changed {list} yourself, and this answer changes them too, so nothing was put into your folder. Save or undo your change first, then keep it again."
                ));
                return;
            }
            Ok(BringIn::Brought(files)) => brought = Some(files),
            Ok(BringIn::Nothing) => brought = Some(Vec::new()),
            Err(e) => {
                s.compare.problem = Some(format!(
                    "Its changes no longer fit your folder, which has changed since the comparison began, so nothing was put in. ({e})"
                ));
                return;
            }
        }
    }

    if let Err(e) = cmp_store.keep(&cmp.compare_id, slot, brought) {
        s.compare.problem = Some(e);
        return;
    }
    if cmp.changes && cmp.changes_in.is_none()
        && let Some(ref f) = folder
    {
        let slots: Vec<CompareSlotId> = cmp.slots.iter().map(|c| c.slot).collect();
        discard_compare_worktrees(f, &cmp.compare_id, &slots, &copy_root);
    }
    let _ = remove_compare_copies(&cmp.compare_id, &copy_root);
    s.compare.problem = None;
    reload_compares(s);
}

fn snapshot_final_text(snap: &ledger::Snapshot) -> String {
    for item in snap.items.iter().rev() {
        if let ledger::Item::Assistant { text, .. } = item {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return trimmed.to_owned();
            }
        }
    }
    String::new()
}

fn snapshot_check(snap: &ledger::Snapshot, running_checks: &HashMap<String, String>) -> Option<JudgedCheck> {
    if running_checks.contains_key(&snap.tip) {
        return Some(JudgedCheck {
            outcome: JudgedCheckOutcome::Running,
            lines: Vec::new(),
            unchanged: false,
        });
    }
    let _ = snap;
    None
}

pub(super) fn ask_judge(s: &mut State) {
    let Some(store) = s.data.clone() else { return };
    let Some(cmp_id) = s.compare.active_id.clone() else { return };
    let Ok(cmp_store) = locust_core::compare::CompareStore::from_store(&store) else { return };
    let Ok(Some(cmp)) = cmp_store.get(&cmp_id) else { return };
    let choices = available_choices(s);
    if choices.is_empty() {
        s.compare.problem = Some("Pick a model to judge.".into());
        return;
    }

    let wanted_count = if s.compare.three_judges { 3 } else { 1 };
    let picked_keys = three_judges(&choices, &s.compare.judge_picks, wanted_count);
    let judge_routes: Vec<CompareRoute> = picked_keys
        .iter()
        .filter_map(|k| choices.iter().find(|c| &c.key == k))
        .map(|c| CompareRoute {
            runtime: c.runtime.clone(),
            model: c.model.clone(),
            account: None,
            effort: None,
            label: Some(c.label.clone()),
            mode: None,
            instructions: None,
            why: None,
        })
        .collect();
    if judge_routes.is_empty() {
        s.compare.problem = Some("Pick a model to judge.".into());
        return;
    }
    let criteria = s.compare.criteria.text.trim().to_owned();
    let criteria_opt = (!criteria.is_empty()).then_some(criteria.as_str());
    if judge_routes.len() > 1 && criteria_lines(criteria_opt).is_empty() {
        s.compare.problem = Some(
            "Three judges vote on the points you write: say what a good answer does, one point a line.".into(),
        );
        return;
    }

    let copy_root = compare_root(&store);
    let folder = active_folder(s);
    let mut answers: Vec<JudgedAnswer> = Vec::new();
    let check_command: Option<String> = None;

    for col in &cmp.slots {
        let Some(newest) = col.mission_ids.last() else { continue };
        let Some(snap) = s.compare.snapshots.get(newest) else { continue };
        if snap.running {
            continue;
        }
        let text = snapshot_final_text(snap);
        if text.is_empty() {
            continue;
        }
        let mut diff = None;
        let mut check = None;
        if cmp.changes && cmp.changes_in != Some(ChangesIn::Folder)
            && let Some(ref f) = folder
        {
            match copy_diff(f, &cmp.compare_id, col.slot, &copy_root, cmp.base.as_deref()) {
                Ok(d) => diff = Some(d),
                Err(_) => {
                    s.compare.problem = Some(format!(
                        "Answer {}'s original changes could not be read. Its copy or starting files may no longer be here. Nothing was sent to a judge.",
                        col.slot.upper()
                    ));
                    return;
                }
            }
            check = snapshot_check(snap, &s.running_checks);
        }
        answers.push(JudgedAnswer {
            letter: col.slot.upper().to_owned(),
            text,
            diff,
            file: None,
            check,
        });
    }

    if answers.len() < 2 {
        s.compare.problem = Some("A judge needs at least two finished answers to read.".into());
        return;
    }
    if answers.iter().any(|a| a.check.is_some()) {
        for a in &mut answers {
            if a.check.is_none() {
                a.check = Some(JudgedCheck {
                    outcome: JudgedCheckOutcome::NotRun,
                    lines: Vec::new(),
                    unchanged: false,
                });
            }
        }
    }

    let changes_kind = if cmp.changes {
        Some(if cmp.changes_in == Some(ChangesIn::Folder) {
            JudgeChangesKind::Folder
        } else {
            JudgeChangesKind::Copies
        })
    } else {
        None
    };

    let brief = match build_judge_brief(
        folder.as_deref(),
        &cmp.prompt,
        &answers,
        criteria_opt,
        changes_kind,
        check_command.as_deref(),
        has_column_instructions(&cmp),
    ) {
        Ok(b) => b,
        Err(e) => {
            s.compare.problem = Some(e);
            return;
        }
    };

    let start_one = |route: &CompareRoute, key: String| {
        let tm = synthetic_teammate(s, None, route);
        let model_opt = if route.model == "account-default" {
            None
        } else {
            Some(route.model.clone())
        };
        locust_core::engine_api::api().send(
            native::Send {
                teammate: tm,
                conversation: None,
                prompt: brief.clone(),
                model: model_opt,
                effort: route.effort.clone(),
            },
            Origin {
                mode: Some(locust_core::approval::Mode::Ask),
                folder: folder.clone(),
                runtime: Some(route.runtime.clone()),
                slot: Some(SlotOrigin {
                    key,
                    cwd: folder.clone(),
                    instructions: None,
                    sealed: true,
                }),
                ..Default::default()
            },
        )
    };

    let first_route = &judge_routes[0];
    let first_started = match start_one(first_route, format!("judge:{}", cmp.compare_id)) {
        Ok(started) => started,
        Err(e) => {
            s.compare.problem = Some(e);
            return;
        }
    };

    let mut panel_members = Vec::new();
    for (idx, r) in judge_routes.iter().skip(1).enumerate() {
        if let Ok(started) = start_one(r, format!("judge:{}:{}", cmp.compare_id, idx + 2)) {
            panel_members.push(JudgePanelMember {
                route: r.clone(),
                mission_id: started.conversation_id,
            });
        }
    }

    if let Err(e) = cmp_store.judge(
        &cmp.compare_id,
        first_route.clone(),
        &first_started.conversation_id,
        criteria_opt,
        panel_members,
    ) {
        s.compare.problem = Some(e);
        return;
    }
    s.compare.problem = None;
    reload_compares(s);
}

pub(super) fn draft_points(s: &mut State) {
    let Some(store) = s.data.clone() else { return };
    let Some(cmp_id) = s.compare.active_id.clone() else { return };
    let Ok(cmp_store) = locust_core::compare::CompareStore::from_store(&store) else { return };
    let Ok(Some(cmp)) = cmp_store.get(&cmp_id) else { return };
    let choices = available_choices(s);
    let key = s.compare.judge_picks[0].clone().or_else(|| choices.first().map(|c| c.key.clone()));
    let Some(choice) = key.and_then(|k| choices.into_iter().find(|c| c.key == k)) else {
        return;
    };
    let route = CompareRoute::new(choice.runtime, choice.model);
    let tm = synthetic_teammate(s, None, &route);
    let prompt = points_draft_request(&cmp.prompt);
    let folder = active_folder(s);
    if let Ok(started) = locust_core::engine_api::api().send(
        native::Send {
            teammate: tm,
            conversation: None,
            prompt,
            model: Some(route.model.clone()),
            effort: None,
        },
        Origin {
            mode: Some(locust_core::approval::Mode::Ask),
            folder: folder.clone(),
            runtime: Some(route.runtime.clone()),
            slot: Some(SlotOrigin {
                key: format!("draft:{}", cmp.compare_id),
                cwd: folder,
                instructions: None,
                sealed: true,
            }),
            ..Default::default()
        },
    ) {
        let _ = cmp_store.add_point_draft(&cmp.compare_id, &started.conversation_id);
        s.compare.drafting = true;
        s.compare.draft_mission_id = Some(started.conversation_id);
    }
}

pub(super) fn reload_compares(s: &mut State) {
    let Some(store) = s.data.as_ref() else { return };
    if let Ok(cmp_store) = locust_core::compare::CompareStore::from_store(store)
        && let Ok(list) = cmp_store.list()
    {
        s.compare.compares = list;
        s.compare.loaded = true;
    }
}

pub(super) fn poll(s: &mut State) -> bool {
    let Some(store) = s.data.clone() else { return false };
    let mut changed = false;
    if !s.compare.loaded {
        reload_compares(s);
        changed = true;
    }
    let Ok(ledger_dir) = store.ledger_dir() else { return changed };
    let electron_ledger = store.electron().join("mission-ledger");

    let load_mission = |id: &str| -> Option<ledger::Snapshot> {
        ledger::Session::new(ledger_dir.clone(), id.to_owned())
            .also_read(electron_ledger.clone())
            .poll()
            .ok()
    };

    if let Some(draft_id) = s.compare.draft_mission_id.clone()
        && let Some(snap) = load_mission(&draft_id)
        && !snap.running
    {
        let text = snapshot_final_text(&snap);
        let pts = drafted_points(&text);
        if !pts.is_empty() {
            s.compare.criteria.replace_text(pts.join("\n"));
        }
        s.compare.drafting = false;
        s.compare.draft_mission_id = None;
        changed = true;
    }

    let Some(active_id) = s.compare.active_id.as_deref() else {
        return changed;
    };
    let Some(cmp) = s.compare.compares.iter().find(|c| c.compare_id == active_id).cloned() else {
        return changed;
    };
    let mut ids: Vec<String> = Vec::new();
    for col in &cmp.slots {
        ids.extend(col.mission_ids.iter().cloned());
    }
    ids.extend(newest_judge_mission_ids(cmp.judge.as_ref()));
    for id in ids {
        if let Some(snap) = load_mission(&id)
            && s.compare.snapshots.get(&id) != Some(&snap)
        {
            s.compare.snapshots.insert(id, snap);
            changed = true;
        }
    }
    changed
}

fn format_span(_snap: &ledger::Snapshot) -> Option<String> {
    None
}

fn format_cost(_snap: &ledger::Snapshot) -> Option<String> {
    None
}

// ---- Composer controls ----------------------------------------------------------------------------------------------

pub(super) fn composer_mode_chip(s: &State, l: &crate::look::controls::Look) -> El {
    let label = s.compare.composer_mode.label();
    crate::look::controls::composer_chip(l, None, label, s.compare.composer_mode != CompareComposerMode::Direct)
        .key("compare-mode-chip")
        .role("button")
        .label(format!("Comparison: {label}"))
        .on_press(|s: &mut State| {
            s.compare.mode_menu = !s.compare.mode_menu;
            s.mode_menu = false;
            s.reference_menu = false;
        })
}

pub(super) fn composer_mode_menu(s: &State, l: &crate::look::controls::Look) -> Option<El> {
    if !s.compare.mode_menu {
        return None;
    }
    let options = [
        (CompareComposerMode::Direct, "compare-mode-direct", "Direct", "One model answers."),
        (CompareComposerMode::Compare, "compare-mode-compare", "Compare", "Two or three models answer side by side."),
        (CompareComposerMode::Blind, "compare-mode-blind", "Blind", "Names stay hidden until you keep one."),
    ];
    let mut menu = col()
        .key("compare-mode-menu")
        .w(280.0)
        .padding(4.0)
        .radius(11.0)
        .fill(l.c("bg-raised-2"))
        .border(1.0, l.c("border-strong"))
        .role("menu")
        .label("Comparison mode");
    for (mode, key, title, desc) in options {
        let chosen = s.compare.composer_mode == mode;
        let mut head = row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(text(title).font(l.ty.control).color(l.c("text-primary")).grow(1.0));
        if chosen {
            head = head.child(crate::look::controls::glyph(l, &crate::look::icons::CHECK, 13.0, Some(l.c("text-secondary"))));
        }
        menu = menu.child(
            col()
                .key(key)
                .w_full()
                .gap(2.0)
                .py(7.0)
                .px(10.0)
                .radius(l.t.px("radius-control"))
                .hover(l.c("bg-selected"))
                .child(head)
                .child(text(desc).font(l.ty.body).color(l.c("text-muted")).w_full().wrap())
                .role("menuitemradio")
                .label(title)
                .on_press(move |s: &mut State| {
                    s.compare.composer_mode = mode;
                    s.compare.mode_menu = false;
                    if mode != CompareComposerMode::Direct {
                        ensure_default_picks(s);
                    }
                }),
        );
    }
    Some(menu)
}

pub(super) fn composer_picks_row(s: &State, l: &crate::look::controls::Look) -> Option<El> {
    if s.compare.composer_mode == CompareComposerMode::Direct {
        return None;
    }
    let models: Vec<locust_core::engine::Models> = s.models.values().flatten().cloned().collect();
    let mut r = row().w_full().gap(8.0).px(6.0).center_y().wrap();
    for (idx, pick) in s.compare.picks.iter().enumerate() {
        let name = pick.label.clone().unwrap_or_else(|| pick.model.clone());
        let mut slot_row = row().gap(4.0).center_y().child(
            crate::look::controls::composer_chip(l, None, &name, false)
                .key(format!("compare-pick-{idx}"))
                .role("button")
                .on_press(move |s: &mut State| {
                    s.compare.model_menu = if s.compare.model_menu == Some(idx) {
                        None
                    } else {
                        Some(idx)
                    };
                }),
        );
        if let Some(eff_label) = effort_aria_label(idx, pick, &models) {
            let eff_word = effective_effort(pick, &models).map(|e| effort_title(&e)).unwrap_or_default();
            slot_row = slot_row.child(
                crate::look::controls::composer_chip(l, None, &eff_word, false)
                    .key(format!("compare-effort-{idx}"))
                    .role("button")
                    .label(eff_label)
                    .on_press(move |s: &mut State| {
                        s.compare.effort_menu = if s.compare.effort_menu == Some(idx) {
                            None
                        } else {
                            Some(idx)
                        };
                    }),
            );
        }
        r = r.child(slot_row);
    }
    let changes_label = if s.compare.changes { "Changes" } else { "Answers" };
    r = r.child(
        crate::look::controls::composer_chip(l, None, changes_label, s.compare.changes)
            .key("compare-changes-toggle")
            .role("button")
            .on_press(|s: &mut State| s.compare.changes = !s.compare.changes),
    );
    Some(r)
}

// ---- Main CompareView -----------------------------------------------------------------------------------------------

pub(super) fn view(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let Some(active_id) = s.compare.active_id.as_deref() else {
        return col().grow(1.0).h_full();
    };
    let Some(cmp) = s.compare.compares.iter().find(|c| c.compare_id == active_id) else {
        return col().grow(1.0).h_full();
    };

    let veiled = cmp.blind && cmp.kept.is_none();
    let setups = has_column_instructions(cmp);
    let kept_slot = cmp.kept.as_ref().map(|k| k.slot);
    let folder = active_folder(s);
    let copy_root = s.data.as_ref().map(compare_root);

    let mut col_names: Vec<String> = Vec::new();
    let mut col_answers: Vec<String> = Vec::new();
    let mut col_spans: Vec<Option<String>> = Vec::new();
    let mut col_costs: Vec<Option<String>> = Vec::new();
    let mut col_checks: Vec<Option<JudgedCheck>> = Vec::new();
    let mut col_running: Vec<bool> = Vec::new();
    let mut col_keepable: Vec<bool> = Vec::new();

    for slot in &cmp.slots {
        let name = if veiled {
            blind_name(slot.slot, setups)
        } else {
            slot.route
                .label
                .clone()
                .unwrap_or_else(|| slot.route.model.clone())
        };
        let newest = slot.mission_ids.last().and_then(|id| s.compare.snapshots.get(id));
        let running = newest.is_some_and(|snap| snap.running);
        let ans = newest.map(snapshot_final_text).unwrap_or_default();
        let span = newest.and_then(format_span);
        let cost = if veiled { None } else { newest.and_then(format_cost) };
        let check = newest.and_then(|snap| snapshot_check(snap, &s.running_checks));
        let keepable = !running && (!ans.is_empty() || slot.mission_ids.last().is_some());

        col_names.push(name);
        col_answers.push(ans);
        col_spans.push(span);
        col_costs.push(cost);
        col_checks.push(check);
        col_running.push(running);
        col_keepable.push(keepable);
    }

    // Judge evaluation
    let judge_view = cmp.judge.as_ref().map(|j| {
        let first_id = j.mission_ids.last();
        let first_snap = first_id.and_then(|id| s.compare.snapshots.get(id));
        let mut all_snaps: Vec<(&CompareRoute, Option<&ledger::Snapshot>)> = vec![(&j.route, first_snap)];
        for m in &j.panel {
            all_snaps.push((&m.route, s.compare.snapshots.get(&m.mission_id)));
        }
        let any_running = all_snaps.iter().any(|(_, snap)| snap.is_some_and(|sn| sn.running));
        let first_text = first_snap.map(snapshot_final_text).unwrap_or_default();
        let slot_ids: Vec<CompareSlotId> = cmp.slots.iter().map(|c| c.slot).collect();
        let slot_strs: Vec<&str> = slot_ids.iter().map(|s| s.as_str()).collect();
        let points = criteria_lines(j.criteria.as_deref());
        let weights = criteria_weights(j.criteria.as_deref());

        let (scorecard, pick, panel_names, silent_names) = if j.panel.is_empty() {
            let card = weighted(parse_scorecard(&first_text, &slot_strs, &points), &weights);
            let p = judge_pick_of(&first_text, &slot_ids);
            (card, p, Vec::new(), Vec::new())
        } else {
            let mut cards: Vec<Option<Scorecard>> = Vec::new();
            let mut picks: Vec<Option<CompareSlotId>> = Vec::new();
            let mut answered = 0usize;
            let mut p_names = Vec::new();
            let mut s_names = Vec::new();
            for (r, snap_opt) in &all_snaps {
                let r_name = r.label.clone().unwrap_or_else(|| r.model.clone());
                p_names.push(r_name.clone());
                if let Some(snap) = snap_opt
                    && !snap.running
                {
                    let txt = snapshot_final_text(snap);
                    if txt.is_empty() {
                        s_names.push(r_name);
                        cards.push(None);
                        picks.push(None);
                    } else {
                        answered += 1;
                        cards.push(parse_scorecard(&txt, &slot_strs, &points));
                        picks.push(judge_pick_of(&txt, &slot_ids));
                    }
                } else {
                    cards.push(None);
                    picks.push(None);
                }
            }
            let card_refs: Vec<Option<&Scorecard>> = cards.iter().map(|c| c.as_ref()).collect();
            let voted = weighted(panel_vote(&card_refs), &weights);
            let p = panel_pick(&picks, answered);
            (voted, p, p_names, s_names)
        };

        let judge_title = if j.panel.is_empty() {
            let name = j.route.label.clone().unwrap_or_else(|| j.route.model.clone());
            format!(
                "THE JUDGE'S VIEW · {name}{}",
                if any_running { " · reading" } else { "" }
            )
        } else {
            let names = joined_names(&panel_names);
            format!(
                "THE JUDGES' VIEW · {names}{}",
                if any_running { " · reading" } else { "" }
            )
        };
        let prose = without_scorecard(&first_text);
        let words_of = j.route.label.clone().unwrap_or_else(|| j.route.model.clone());
        (judge_title, any_running, scorecard, pick, prose, j.panel.is_empty(), panel_names, silent_names, words_of)
    });

    // Top bar sentence
    let bar_sentence = if let Some(ks) = kept_slot {
        let kept_idx = cmp.slots.iter().position(|c| c.slot == ks).unwrap_or(0);
        let kept_name = &col_names[kept_idx];
        format!("You kept {kept_name}.")
    } else {
        let names = joined_names(&col_names);
        let hidden = if veiled {
            ", names hidden until you keep one"
        } else {
            ""
        };
        let mode_sentence = if !cmp.changes {
            "They answer without changing files."
        } else if cmp.changes_in == Some(ChangesIn::Folder) {
            "This folder is too big to give each its own copy, so they all work in your folder itself."
        } else {
            "Each changes its own copy of your project; only the one you keep comes into your folder."
        };
        format!("Comparing {names}{hidden}. {mode_sentence}")
    };

    let mut root_col = col().grow(1.0).h_full().fill(p.bg);

    // Top bar
    let mut bar = row()
        .key("compare-bar")
        .w_full()
        .px(20.0)
        .py(12.0)
        .gap(12.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
        .child(text(bar_sentence).font(l.ty.body).color(p.text).grow(1.0).wrap());
    if let Some(prob) = &s.compare.problem {
        bar = bar.child(text(prob.clone()).font(l.ty.body).color(p.red));
    }
    bar = bar.child(
        crate::look::controls::button(&l, "Back", crate::look::controls::Tone::Neutral, false)
            .key("compare-back")
            .on_press(|s: &mut State| {
                s.compare.active_id = None;
            }),
    );
    root_col = root_col.child(bar);

    // Scrollable body: columns + judge
    let mut scroll_body = col().w_full().gap(16.0).padding(20.0);
    scroll_body = scroll_body.child(
        text(cmp.prompt.clone())
            .font(rune::TextStyle::new(14.0, 600.0))
            .color(p.text)
            .wrap(),
    );

    // Side-by-side columns
    let mut cols_row = row().w_full().gap(12.0);
    for (idx, slot) in cmp.slots.iter().enumerate() {
        let name = &col_names[idx];
        let ans = &col_answers[idx];
        let running = col_running[idx];
        let keepable = col_keepable[idx];
        let span = col_spans[idx].as_deref();
        let cost = col_costs[idx].as_deref();
        let check = col_checks[idx].as_ref();
        let is_kept = kept_slot == Some(slot.slot);
        let is_pick = judge_view.as_ref().and_then(|jv| jv.3) == Some(slot.slot);

        let mut head = row()
            .key(format!("compare-head-{}", slot.slot.as_str()))
            .w_full()
            .gap(8.0)
            .center_y()
            .pb(8.0)
            .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
            .child(
                text(name.clone())
                    .font(rune::TextStyle::new(13.5, 600.0))
                    .color(p.text)
                    .grow(1.0)
                    .truncate(),
            );
        if is_kept {
            head = head.child(crate::look::controls::chip(&l, "Kept", true));
        }
        if is_pick {
            let badge_label = if judge_view.as_ref().is_some_and(|jv| !jv.5) {
                "Judges' pick"
            } else {
                "Judge's pick"
            };
            head = head.child(crate::look::controls::chip(&l, badge_label, true));
        }
        if !veiled {
            head = head.child(
                text(runtime_display_name(&slot.route.runtime))
                    .font(l.ty.label)
                    .color(p.muted),
            );
        }
        let state_word = if slot.refused.is_some() {
            "refused"
        } else if running {
            "working"
        } else if !ans.is_empty() {
            "done"
        } else {
            "waiting"
        };
        head = head.child(text(state_word).font(l.ty.label).color(p.muted));

        let body_text = if let Some(refusal) = &slot.refused {
            refusal.clone()
        } else if ans.is_empty() {
            if running {
                "Working…".to_owned()
            } else {
                "No answer yet.".to_owned()
            }
        } else {
            ans.clone()
        };

        let mut cell = col()
            .key(format!("compare-cell-{}", slot.slot.as_str()))
            .key(format!("compare-col-{}", slot.slot.as_str()))
            .grow(1.0)
            .shrink()
            .padding(12.0)
            .gap(10.0)
            .radius(8.0)
            .fill(p.raised)
            .border(1.0, if is_kept { l.c("lime-border") } else { p.hairline })
            .child(head)
            .child(text(body_text).font(l.ty.body).color(p.text).w_full().wrap());

        if let Some(chk) = check {
            let chk_line = match chk.outcome {
                JudgedCheckOutcome::Passed => "passed after this turn.".to_owned(),
                JudgedCheckOutcome::Failed => format!("failed after this turn\n{}", chk.lines.join("\n")),
                JudgedCheckOutcome::Running => "running after this turn…".to_owned(),
                _ => format!("{} after this turn.", chk.outcome.word()),
            };
            cell = cell.child(text(chk_line).font(l.ty.label).color(p.muted).wrap());
        }

        // Footer with line changes, span, cost (hidden when veiled), and Keep button
        let mut parts: Vec<String> = Vec::new();
        if cmp.changes && cmp.changes_in != Some(ChangesIn::Folder)
            && let (Some(f), Some(cr)) = (&folder, &copy_root)
            && let Ok((files, added, removed)) = copy_line_changes(f, &cmp.compare_id, slot.slot, cr)
        {
            parts.push(changes_line(files, added, removed));
        }
        if let Some(sp) = span {
            parts.push(sp.to_owned());
        }
        if let Some(cs) = cost {
            parts.push(cs.to_owned());
        }

        let mut foot = row()
            .key(format!("compare-foot-{}", slot.slot.as_str()))
            .w_full()
            .pt(8.0)
            .gap(8.0)
            .center_y()
            .border_sides([1.0, 0.0, 0.0, 0.0], p.hairline)
            .child(text(parts.join(" · ")).font(l.ty.label).color(p.muted).grow(1.0).wrap());

        if cmp.kept.is_none() {
            let slot_id = slot.slot;
            let mut keep_btn = crate::look::controls::button(&l, "Keep this one", crate::look::controls::Tone::Active, !keepable)
                .key(format!("compare-keep-{}", slot.slot.as_str()));
            if keepable {
                keep_btn = keep_btn.on_press(move |s: &mut State| keep_column(s, slot_id));
            }
            foot = foot.child(keep_btn);
        }
        cell = cell.child(foot);
        cols_row = cols_row.child(cell);
    }
    scroll_body = scroll_body.child(cols_row);

    // Judge's view & Scorecard
    if let Some((judge_title, any_running, scorecard_opt, _pick, prose, is_single, panel_names, silent_names, words_of)) = judge_view {
        let mut jbox = col()
            .key("compare-judge-view")
            .w_full()
            .padding(14.0)
            .gap(10.0)
            .radius(8.0)
            .fill(p.raised)
            .border(1.0, p.hairline)
            .child(text(judge_title).font(l.ty.label).color(p.muted));

        if let Some(sc) = scorecard_opt {
            jbox = jbox.child(render_scorecard(&sc, &cmp.slots, &col_names, &col_spans, &col_costs, &col_checks, veiled, p, &l));
        }

        if !is_single && !any_running {
            let intro = if silent_names.is_empty() {
                format!(
                    "The {} judges scored every point on their own; a point counts when more of them say met than not. The dots are their votes. The words below are {}'s.",
                    panel_names.len(),
                    words_of
                )
            } else {
                let active_count = panel_names.len().saturating_sub(silent_names.len());
                format!(
                    "{} did not answer, so the other {} scored every point on their own; a point counts when more of them say met than not. The dots are their votes. The words below are {}'s.",
                    joined_names(&silent_names),
                    if active_count == 1 { "judge".into() } else { format!("{active_count} judges") },
                    words_of
                )
            };
            jbox = jbox.child(text(intro).font(l.ty.label).color(p.muted).wrap());
        }

        if !prose.is_empty() {
            jbox = jbox.child(text(prose).font(l.ty.body).color(p.text).wrap());
        } else if any_running {
            jbox = jbox.child(text("Starting the judge…").font(l.ty.body).color(p.muted));
        }

        let note = if is_single {
            "It read the answers as Answer A, Answer B and so on, not by model. Its view, not a decision: you keep the answer."
        } else {
            "They read the answers as Answer A, Answer B and so on, not by model. Their view, not a decision: you keep the answer."
        };
        jbox = jbox.child(text(note).font(l.ty.label).color(p.muted).wrap());
        scroll_body = scroll_body.child(jbox);
    }

    // Ask a judge form (when >= 2 keepable and none running)
    let finished_count = col_keepable.iter().filter(|&&k| k).count();
    let any_col_running = col_running.iter().any(|&r| r);
    if finished_count >= 2 && !any_col_running {
        let mut ask_box = col()
            .key("compare-judge-ask")
            .w_full()
            .padding(14.0)
            .gap(8.0)
            .radius(8.0)
            .fill(p.raised)
            .border(1.0, p.hairline)
            .child(text("ASK A JUDGE").font(l.ty.label).color(p.muted));

        if s.compare.criteria.text.trim().is_empty() {
            ask_box = ask_box.child(
                crate::look::controls::button(&l, "Draft the points", crate::look::controls::Tone::Neutral, false)
                    .key("compare-draft-points")
                    .on_press(|s: &mut State| draft_points(s)),
            );
        }

        let mut btn_row = row().gap(8.0).center_y();
        let three_label = if s.compare.three_judges {
            "Three judges (voting)"
        } else {
            "One judge"
        };
        btn_row = btn_row.child(
            crate::look::controls::button(
                &l,
                three_label,
                if s.compare.three_judges {
                    crate::look::controls::Tone::Active
                } else {
                    crate::look::controls::Tone::Neutral
                },
                false,
            )
            .key("compare-judge-three-toggle")
            .on_press(|s: &mut State| s.compare.three_judges = !s.compare.three_judges),
        );
        let ask_label = if s.compare.three_judges {
            "Ask three judges"
        } else {
            "Ask a judge"
        };
        btn_row = btn_row.child(
            crate::look::controls::button(&l, ask_label, crate::look::controls::Tone::Active, false)
                .key("compare-ask-judge")
                .on_press(|s: &mut State| ask_judge(s)),
        );
        ask_box = ask_box.child(btn_row);
        scroll_body = scroll_body.child(ask_box);
    }

    root_col.child(
        col()
            .grow(1.0)
            .w_full()
            .scroll(ScrollSpec { stick_bottom: false })
            .child(scroll_body),
    )
}

#[allow(clippy::too_many_arguments)]
fn render_scorecard(
    sc: &Scorecard,
    slots: &[locust_core::compare::CompareSlot],
    col_names: &[String],
    col_spans: &[Option<String>],
    col_costs: &[Option<String>],
    col_checks: &[Option<JudgedCheck>],
    veiled: bool,
    p: &Palette,
    l: &crate::look::controls::Look,
) -> El {
    let mut table = col()
        .key("compare-scorecard")
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .radius(6.0)
        .fill(p.bg)
        .border(1.0, p.hairline);

    let first_col_header = if sc.by_judge {
        "The judge’s points"
    } else {
        "What a good answer does"
    };
    let mut hdr = row()
        .w_full()
        .gap(8.0)
        .pb(4.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
        .child(text(first_col_header).font(l.ty.label).color(p.muted).grow(1.0));
    for name in col_names {
        hdr = hdr.child(text(name.clone()).font(l.ty.label).color(p.text).grow(1.0));
    }
    table = table.child(hdr);

    for (idx, point) in sc.points.iter().enumerate() {
        let weight = sc.weights.as_ref().and_then(|w| w.get(idx).copied()).unwrap_or(1);
        let point_label = if weight > 1 {
            format!("{point} ×{weight}")
        } else {
            point.clone()
        };
        let mut r = row().w_full().gap(8.0).child(
            text(point_label).font(l.ty.body).color(p.text).grow(1.0).wrap(),
        );
        for slot in slots {
            let cell_text = match sc.verdicts.get(slot.slot.as_str()).and_then(|v| v.get(idx)).and_then(|v| v.as_ref()) {
                None => "not scored".to_owned(),
                Some(v) => {
                    let status = if v.met { "met" } else { "not met" };
                    let votes = match v.votes {
                        Some((m, of)) if of > 1 => format!(" ({m} of {of} judges said met)"),
                        _ => String::new(),
                    };
                    if v.why.is_empty() {
                        format!("{status}{votes}")
                    } else {
                        format!("{status}{votes} — {}", v.why)
                    }
                }
            };
            r = r.child(text(cell_text).font(l.ty.body).color(p.muted).grow(1.0).wrap());
        }
        table = table.child(r);
    }

    // Score footer
    let scores: Vec<(usize, usize)> = slots
        .iter()
        .map(|s| score_of(sc.verdicts.get(s.slot.as_str()).map(Vec::as_slice), sc.weights.as_deref()))
        .collect();
    let best = scores.iter().map(|s| s.0).max().unwrap_or(0);
    let alone = scores.iter().filter(|s| s.0 == best).count() == 1;
    let mut score_row = row()
        .w_full()
        .gap(8.0)
        .pt(4.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], p.hairline)
        .child(text("Score").font(l.ty.label).color(p.muted).grow(1.0));
    for (met, of) in &scores {
        let is_best = alone && *met == best;
        score_row = score_row.child(
            text(format!("{met} of {of}"))
                .font(l.ty.label)
                .color(if is_best { p.green } else { p.text })
                .grow(1.0),
        );
    }
    table = table.child(score_row);

    if col_checks.iter().any(|c| c.is_some()) {
        let mut chk_row = row().w_full().gap(8.0).child(text("Your check").font(l.ty.label).color(p.muted).grow(1.0));
        for c in col_checks {
            let word = c.as_ref().map(|chk| chk.outcome.word()).unwrap_or("—");
            chk_row = chk_row.child(text(word).font(l.ty.label).color(p.text).grow(1.0));
        }
        table = table.child(chk_row);
    }

    if col_spans.iter().any(|s| s.is_some()) {
        let mut time_row = row().w_full().gap(8.0).child(text("Time").font(l.ty.label).color(p.muted).grow(1.0));
        for sp in col_spans {
            time_row = time_row.child(text(sp.as_deref().unwrap_or("—")).font(l.ty.label).color(p.text).grow(1.0));
        }
        table = table.child(time_row);
    }

    if !veiled && col_costs.iter().any(|c| c.is_some()) {
        let mut spent_row = row().w_full().gap(8.0).child(text("Spent").font(l.ty.label).color(p.muted).grow(1.0));
        for cs in col_costs {
            spent_row = spent_row.child(text(cs.as_deref().unwrap_or("—")).font(l.ty.label).color(p.text).grow(1.0));
        }
        table = table.child(spent_row);
    }

    if sc.by_judge {
        table = table.child(
            text("No points were written, so the judge set these from your request.")
                .font(l.ty.label)
                .color(p.muted)
                .wrap(),
        );
    }

    table
}
