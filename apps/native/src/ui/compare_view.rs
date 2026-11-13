//! COMPARE & BLIND, AND THE JUDGES (Electron `CompareView.tsx` + `main/index.ts` compare handlers).

use super::*;
use locust_core::benchmark::{
    BenchmarkAnswer, BenchmarkCheck, BenchmarkEntry, BenchmarkRow, BenchmarkRunInput, ChartAxis,
    RunCost, TimeBecause, benchmark_chart, benchmark_rows, cost_cell, cost_line, step_words,
};
use locust_core::compare::{
    ChangesIn, Compare, CompareRoute, CompareSlotId, CreateCompareInput, JudgePanelMember,
    MAX_COMPARE_SLOTS, MIN_COMPARE_SLOTS, RerunProgress, blind_name, changes_line,
    compare_refusal_of, compare_slot_key, has_column_instructions, judge_pick_of,
    newest_judge_mission_ids, rerun_refusal_of, shuffled_columns,
};
use locust_core::compare_copies::{
    bring_in_compare_worktree, bring_in_copy, compare_root, copy_diff, copy_line_changes,
    copy_refusal, discard_compare_worktrees, ensure_compare_worktree, git_head, is_git_repo,
    make_compare_copy, remove_compare_copies, resolve_compare_page_path,
};
use locust_core::compare_judge::{
    JudgeChangesKind, JudgedAnswer, JudgedCheck, JudgedCheckOutcome, build_judge_brief,
};
use locust_core::compare_reruns::{CompareReruns, CompareRerunsHost};
use locust_core::compare_score::{
    Scorecard, criteria_lines, criteria_weights, drafted_points, panel_pick, panel_vote,
    parse_scorecard, points_draft_request, score_of, weighted, without_scorecard,
};
use locust_core::ledger;
use locust_core::native::{self, Origin, SlotOrigin};
use locust_core::routine_copy::BringIn;
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

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
    pub(super) followup: TextEdit,
    pub(super) three_judges: bool,
    pub(super) judge_picks: [Option<String>; 3],
    pub(super) drafting: bool,
    pub(super) draft_mission_id: Option<String>,
    pub(super) reruns: Option<Arc<CompareReruns>>,
    pub(super) rerun_progress: Arc<Mutex<Vec<RerunProgress>>>,
    pub(super) rerun_dirty: Arc<AtomicBool>,
    pub(super) add_model_menu: bool,
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
                    // Electron gives a column a slot of its own and never seals it (index.ts 7974): a blind column still works.
                    sealed: false,
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
                    // Electron runs a judge and the rubric draft in Ask with a slot of their own, unsealed (index.ts 8274, 8420).
                    sealed: false,
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
                // Electron runs a judge and the rubric draft in Ask with a slot of their own, unsealed (index.ts 8274, 8420).
                    sealed: false,
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

fn slot_origin_for(
    cmp: &Compare,
    col: &locust_core::compare::CompareSlot,
    folder: Option<&std::path::Path>,
    copy_root: &std::path::Path,
) -> (Option<std::path::PathBuf>, Origin) {
    let slot_key = compare_slot_key(cmp.teammate_id.as_deref(), &cmp.compare_id, col.slot);
    let slot_cwd = if cmp.changes && cmp.changes_in != Some(ChangesIn::Folder) {
        if let Some(f) = folder {
            let name = format!("{}-{}", cmp.compare_id, col.slot.as_str());
            let dir = copy_root.join(name);
            if dir.exists() {
                Some(dir)
            } else if cmp.changes_in == Some(ChangesIn::Copy) {
                make_compare_copy(f, &cmp.compare_id, col.slot, copy_root, None).ok()
            } else {
                ensure_compare_worktree(f, &cmp.compare_id, col.slot, copy_root, cmp.base.as_deref()).ok()
            }
        } else {
            None
        }
    } else {
        folder.map(std::path::Path::to_path_buf)
    };
    let mode = if !cmp.changes {
        locust_core::approval::Mode::Ask
    } else if col.route.mode.as_deref() == Some("auto") {
        locust_core::approval::Mode::Auto
    } else {
        locust_core::approval::Mode::AcceptEdits
    };
    let origin = Origin {
        mode: Some(mode),
        folder: slot_cwd.clone(),
        runtime: Some(col.route.runtime.clone()),
        slot: Some(SlotOrigin {
            key: slot_key,
            cwd: slot_cwd.clone(),
            instructions: col.route.instructions.clone(),
            sealed: false,
        }),
        ..Default::default()
    };
    (slot_cwd, origin)
}

fn send_column_turn(
    tm: locust_core::engine::Teammate,
    route: &CompareRoute,
    prompt: &str,
    conversation: Option<String>,
    origin: Origin,
) -> Result<locust_core::engine::Started, String> {
    let model = (route.model != "account-default").then(|| route.model.clone());
    locust_core::engine_api::api().send(
        native::Send {
            teammate: tm,
            conversation,
            prompt: prompt.to_owned(),
            model,
            effort: route.effort.clone(),
        },
        origin,
    )
}

pub(super) fn retry_column(s: &mut State, slot: CompareSlotId) {
    let Some(store) = s.data.clone() else { return };
    let Some(cmp_id) = s.compare.active_id.clone() else { return };
    let Ok(cmp_store) = locust_core::compare::CompareStore::from_store(&store) else { return };
    let Ok(Some(cmp)) = cmp_store.get(&cmp_id) else { return };
    if cmp.kept.is_some() {
        s.compare.problem = Some("You kept one already; this conversation carries on with it.".into());
        return;
    }
    if cmp.rerun_of.is_some() {
        s.compare.problem = Some(
            "A run of your benchmark is a measurement: nothing in it is asked again, kept or added to. Use the comparison it runs again."
                .into(),
        );
        return;
    }
    let Some(col) = cmp.slots.iter().find(|c| c.slot == slot).cloned() else {
        s.compare.problem = Some("That comparison has no such column.".into());
        return;
    };
    let folder = active_folder(s);
    let copy_root = compare_root(&store);
    let tm = synthetic_teammate(s, cmp.teammate_id.as_deref(), &col.route);
    let follow_id = if col.mission_ids.len() >= 2 {
        col.mission_ids.get(col.mission_ids.len() - 2).cloned()
    } else {
        None
    };
    let (_, origin) = slot_origin_for(&cmp, &col, folder.as_deref(), &copy_root);
    match send_column_turn(tm, &col.route, &cmp.prompt, follow_id, origin) {
        Ok(started) => {
            let _ = cmp_store.retry(&cmp.compare_id, slot, &started.conversation_id);
            s.compare.problem = None;
            reload_compares(s);
        }
        Err(why) => {
            let _ = cmp_store.refuse(&cmp.compare_id, slot, &why);
            s.compare.problem = Some(why);
            reload_compares(s);
        }
    }
}

pub(super) fn ask_compare(s: &mut State) {
    let prompt = s.compare.followup.text.trim().to_owned();
    if prompt.is_empty() {
        s.compare.problem = Some("There is nothing to ask.".into());
        return;
    }
    let Some(store) = s.data.clone() else { return };
    let Some(cmp_id) = s.compare.active_id.clone() else { return };
    let Ok(cmp_store) = locust_core::compare::CompareStore::from_store(&store) else { return };
    let Ok(Some(cmp)) = cmp_store.get(&cmp_id) else { return };
    if cmp.kept.is_some() {
        s.compare.problem = Some("You kept one already; this conversation carries on with it.".into());
        return;
    }
    if cmp.rerun_of.is_some() {
        s.compare.problem = Some(
            "A run of your benchmark is a measurement: nothing in it is asked again, kept or added to. Use the comparison it runs again."
                .into(),
        );
        return;
    }
    let folder = active_folder(s);
    let copy_root = compare_root(&store);
    for col in &cmp.slots {
        let tm = synthetic_teammate(s, cmp.teammate_id.as_deref(), &col.route);
        let follow_id = col.mission_ids.last().cloned();
        let (_, origin) = slot_origin_for(&cmp, col, folder.as_deref(), &copy_root);
        match send_column_turn(tm, &col.route, &prompt, follow_id, origin) {
            Ok(started) => {
                let _ = cmp_store.add_turn(&cmp.compare_id, col.slot, &started.conversation_id);
            }
            Err(why) => {
                let _ = cmp_store.refuse(&cmp.compare_id, col.slot, &why);
            }
        }
    }
    s.compare.followup.clear();
    s.compare.problem = None;
    reload_compares(s);
}

pub(super) fn add_compare_column(s: &mut State, choice: ModelChoice) {
    let Some(store) = s.data.clone() else { return };
    let Some(cmp_id) = s.compare.active_id.clone() else { return };
    let Ok(cmp_store) = locust_core::compare::CompareStore::from_store(&store) else { return };
    let Ok(Some(cmp)) = cmp_store.get(&cmp_id) else { return };
    if cmp.rerun_of.is_some() {
        s.compare.problem = Some(
            "A run of your benchmark is a measurement: nothing in it is asked again, kept or added to. Use the comparison it runs again."
                .into(),
        );
        return;
    }
    if cmp.changes {
        s.compare.problem = Some(
            "A comparison that changes files cannot take another model after it starts. Start a new comparison instead."
                .into(),
        );
        return;
    }
    if cmp.slots.iter().any(|c| c.mission_ids.len() > 1) {
        s.compare.problem = Some(
            "This comparison went on past its first question, so another model cannot be asked the same thing. Start a new comparison instead."
                .into(),
        );
        return;
    }
    let route = CompareRoute {
        runtime: choice.runtime,
        model: choice.model,
        account: None,
        effort: None,
        label: Some(choice.label),
        mode: None,
        instructions: None,
        why: None,
    };
    let added = match cmp_store.add_column(&cmp.compare_id, route) {
        Ok(c) => c,
        Err(e) => {
            s.compare.problem = Some(e);
            return;
        }
    };
    if let Some(col) = added.slots.last() {
        let folder = active_folder(s);
        let tm = synthetic_teammate(s, added.teammate_id.as_deref(), &col.route);
        let origin = Origin {
            mode: Some(locust_core::approval::Mode::Ask),
            folder: folder.clone(),
            runtime: Some(col.route.runtime.clone()),
            slot: Some(SlotOrigin {
                key: compare_slot_key(added.teammate_id.as_deref(), &added.compare_id, col.slot),
                cwd: folder,
                instructions: col.route.instructions.clone(),
                sealed: false,
            }),
            ..Default::default()
        };
        match send_column_turn(tm, &col.route, &added.prompt, None, origin) {
            Ok(started) => {
                let _ = cmp_store.add_turn(&added.compare_id, col.slot, &started.conversation_id);
            }
            Err(why) => {
                let _ = cmp_store.refuse(&added.compare_id, col.slot, &why);
            }
        }
    }
    s.compare.add_model_menu = false;
    s.compare.problem = None;
    reload_compares(s);
}

fn column_html_pages(snap: &ledger::Snapshot) -> Vec<String> {
    let mut out = Vec::new();
    let re = locust_core::invariants::regex(r#"(?i)([^\s"'`<>]+\.html?)\b"#);
    for item in &snap.items {
        let mut texts = Vec::new();
        match item {
            ledger::Item::Assistant { text, .. } => texts.push(text.as_str()),
            ledger::Item::Tool { summary, detail, .. } => {
                texts.push(summary.as_str());
                texts.push(detail.arguments.as_str());
                texts.push(detail.output.as_str());
            }
            _ => continue,
        }
        for text_src in texts {
            for cap in re.captures_iter(text_src) {
                if let Some(m) = cap.get(1) {
                    let p = m.as_str().to_owned();
                    if !out.contains(&p) {
                        out.push(p);
                    }
                }
            }
        }
    }
    out
}

pub(super) fn preview_column_page(s: &mut State, slot: CompareSlotId, requested: &str) {
    let Some(store) = s.data.as_ref() else { return };
    let Some(cmp_id) = s.compare.active_id.as_deref() else { return };
    let Some(cmp) = s.compare.compares.iter().find(|c| c.compare_id == cmp_id) else {
        return;
    };
    let Some(folder) = active_folder(s) else {
        s.compare.problem = Some("No conversation folder.".into());
        return;
    };
    let copy_root = compare_root(store);
    let resolved = match resolve_compare_page_path(requested, &folder, cmp, slot, &copy_root) {
        Ok(p) => p,
        Err(why) => {
            s.compare.problem = Some(why);
            return;
        }
    };
    let copy_dir = copy_root.join(format!("{}-{}", cmp.compare_id, slot.as_str()));
    let workspace = if resolved.starts_with(&copy_dir) {
        copy_dir
    } else {
        folder
    };
    let url = {
        let mut host = crate::invariants::lock(&s.browser.host);
        let url = match host.preview.url_for(&resolved, Some(&workspace)) {
            Ok(u) => u,
            Err(why) => {
                s.compare.problem = Some(why);
                return;
            }
        };
        let tab_id = if let Some(id) = host.active {
            id
        } else {
            match host.add(crate::browser::Profile(None)) {
                Ok(id) => id,
                Err(why) => {
                    s.compare.problem = Some(why);
                    return;
                }
            }
        };
        if let Err(why) = host.navigate(tab_id, &url, true) {
            s.compare.problem = Some(why);
            return;
        }
        host.open = true;
        url
    };
    let _ = url;
    s.compare.problem = None;
}

struct NativeRerunsHost {
    store: locust_core::store::Store,
    cmp_store: locust_core::compare::CompareStore,
    folder: Option<std::path::PathBuf>,
    teammates: Vec<locust_core::engine::Teammate>,
    progress: Arc<Mutex<Vec<RerunProgress>>>,
    dirty: Arc<AtomicBool>,
}

impl CompareRerunsHost for NativeRerunsHost {
    fn begin(&self, compare: &Compare) {
        let copy_root = compare_root(&self.store);
        for col in &compare.slots {
            let tm = compare
                .teammate_id
                .as_deref()
                .and_then(|id| self.teammates.iter().find(|t| t.id == id))
                .cloned()
                .unwrap_or_else(|| locust_core::engine::Teammate {
                    id: "nobody".into(),
                    name: col.route.label.clone().unwrap_or_else(|| col.route.model.clone()),
                    role: String::new(),
                    runtime: Some(col.route.runtime.clone()),
                    model: Some(col.route.model.clone()),
                    busy: false,
                    runs_in: "ask".into(),
                    folder: self.folder.as_ref().map(|p| p.to_string_lossy().into_owned()),
                });
            let (_, origin) = slot_origin_for(compare, col, self.folder.as_deref(), &copy_root);
            match send_column_turn(tm, &col.route, &compare.prompt, None, origin) {
                Ok(started) => {
                    let _ = self.cmp_store.add_turn(&compare.compare_id, col.slot, &started.conversation_id);
                }
                Err(why) => {
                    let _ = self.cmp_store.refuse(&compare.compare_id, col.slot, &why);
                }
            }
        }
    }

    fn judge(
        &self,
        compare: &Compare,
        route: &CompareRoute,
        criteria: Option<&str>,
        panel: &[CompareRoute],
    ) -> Option<String> {
        let Ok(ledger_dir) = self.store.ledger_dir() else {
            return Some("Could not open ledger.".into());
        };
        let electron_ledger = self.store.electron().join("mission-ledger");
        let copy_root = compare_root(&self.store);
        let mut answers = Vec::new();
        for col in &compare.slots {
            let Some(mid) = col.mission_ids.last() else { continue };
            let Ok(snap) = ledger::Session::new(ledger_dir.clone(), mid.clone())
                .also_read(electron_ledger.clone())
                .poll()
            else {
                continue;
            };
            let text = snapshot_final_text(&snap);
            if text.is_empty() {
                continue;
            }
            let diff = if compare.changes && compare.changes_in != Some(ChangesIn::Folder) {
                self.folder
                    .as_deref()
                    .and_then(|f| copy_diff(f, &compare.compare_id, col.slot, &copy_root, compare.base.as_deref()).ok())
                    .filter(|d| !d.is_empty())
            } else {
                None
            };
            answers.push(JudgedAnswer {
                letter: col.slot.upper().to_owned(),
                text,
                diff,
                file: None,
                check: None,
            });
        }
        if answers.len() < 2 {
            return Some("A judge needs at least two finished answers to read.".into());
        }
        let changes_kind = if compare.changes {
            Some(if compare.changes_in == Some(ChangesIn::Folder) {
                JudgeChangesKind::Folder
            } else {
                JudgeChangesKind::Copies
            })
        } else {
            None
        };
        let brief = match build_judge_brief(
            self.folder.as_deref(),
            &compare.prompt,
            &answers,
            criteria,
            changes_kind,
            None,
            has_column_instructions(compare),
        ) {
            Ok(b) => b,
            Err(e) => return Some(e),
        };
        let start_one = |r: &CompareRoute, suffix: &str| -> Result<String, String> {
            let tm = locust_core::engine::Teammate {
                id: "nobody".into(),
                name: r.label.clone().unwrap_or_else(|| r.model.clone()),
                role: String::new(),
                runtime: Some(r.runtime.clone()),
                model: Some(r.model.clone()),
                busy: false,
                runs_in: "ask".into(),
                folder: self.folder.as_ref().map(|p| p.to_string_lossy().into_owned()),
            };
            let key = format!("{}:{}:{suffix}", compare.compare_id, r.model);
            let origin = Origin {
                mode: Some(locust_core::approval::Mode::Ask),
                folder: self.folder.clone(),
                runtime: Some(r.runtime.clone()),
                slot: Some(SlotOrigin {
                    key,
                    cwd: None,
                    instructions: None,
                    sealed: false,
                }),
                ..Default::default()
            };
            send_column_turn(tm, r, &brief, None, origin).map(|s| s.conversation_id)
        };
        let first_id = match start_one(route, "judge") {
            Ok(id) => id,
            Err(e) => return Some(e),
        };
        let mut members = Vec::new();
        for (i, pr) in panel.iter().enumerate() {
            if let Ok(mid) = start_one(pr, &format!("panel-{i}")) {
                members.push(JudgePanelMember {
                    route: pr.clone(),
                    mission_id: mid,
                });
            }
        }
        let _ = self.cmp_store.judge(
            &compare.compare_id,
            route.clone(),
            &first_id,
            criteria,
            members,
        );
        None
    }

    fn live(&self, mission_id: &str) -> bool {
        let Ok(ledger_dir) = self.store.ledger_dir() else {
            return false;
        };
        let electron_ledger = self.store.electron().join("mission-ledger");
        ledger::Session::new(ledger_dir, mission_id.to_owned())
            .also_read(electron_ledger)
            .poll()
            .is_ok_and(|s| s.running)
    }

    fn phase_of(&self, mission_id: &str) -> Option<String> {
        let ledger_dir = self.store.ledger_dir().ok()?;
        let electron_ledger = self.store.electron().join("mission-ledger");
        let snap = ledger::Session::new(ledger_dir, mission_id.to_owned())
            .also_read(electron_ledger)
            .poll()
            .ok()?;
        if snap.running {
            None
        } else if snap.failed {
            Some("failed".into())
        } else {
            Some("completed".into())
        }
    }

    fn retry(&self, compare: &Compare, slot: CompareSlotId) -> Option<String> {
        let col = compare.slots.iter().find(|c| c.slot == slot)?;
        let copy_root = compare_root(&self.store);
        let (_, origin) = slot_origin_for(compare, col, self.folder.as_deref(), &copy_root);
        let tm = locust_core::engine::Teammate {
            id: "nobody".into(),
            name: col.route.label.clone().unwrap_or_else(|| col.route.model.clone()),
            role: String::new(),
            runtime: Some(col.route.runtime.clone()),
            model: Some(col.route.model.clone()),
            busy: false,
            runs_in: "ask".into(),
            folder: self.folder.as_ref().map(|p| p.to_string_lossy().into_owned()),
        };
        match send_column_turn(tm, &col.route, &compare.prompt, None, origin) {
            Ok(started) => {
                let _ = self
                    .cmp_store
                    .retry(&compare.compare_id, slot, &started.conversation_id);
                None
            }
            Err(why) => Some(why),
        }
    }

    fn stop_mission(&self, mission_id: &str) {
        let _ = locust_core::engine_api::api().stop(mission_id);
    }

    fn tidy(&self, compare: &Compare) {
        let copy_root = compare_root(&self.store);
        if compare.changes && compare.changes_in.is_none()
            && let Some(f) = self.folder.as_deref()
        {
            let slots: Vec<CompareSlotId> = compare.slots.iter().map(|c| c.slot).collect();
            discard_compare_worktrees(f, &compare.compare_id, &slots, &copy_root);
        }
        let _ = remove_compare_copies(&compare.compare_id, &copy_root);
    }

    fn tell(&self, _compares: Vec<Compare>, progress: Vec<RerunProgress>) {
        *self.progress.lock().unwrap_or_else(|e| e.into_inner()) = progress;
        self.dirty.store(true, Ordering::Release);
    }
}

fn ensure_reruns(s: &mut State) -> Option<Arc<CompareReruns>> {
    if let Some(r) = &s.compare.reruns {
        return Some(Arc::clone(r));
    }
    let store = s.data.clone()?;
    let cmp_store = locust_core::compare::CompareStore::from_store(&store).ok()?;
    let host = Arc::new(NativeRerunsHost {
        store,
        cmp_store: cmp_store.clone(),
        folder: active_folder(s),
        teammates: s.teammates.clone(),
        progress: Arc::clone(&s.compare.rerun_progress),
        dirty: Arc::clone(&s.compare.rerun_dirty),
    });
    let runner = Arc::new(CompareReruns::new(cmp_store, host));
    s.compare.reruns = Some(Arc::clone(&runner));
    Some(runner)
}

pub(super) fn start_benchmark_rerun(s: &mut State, times: usize) {
    let Some(cmp_id) = s.compare.active_id.clone() else { return };
    if active_folder(s).is_none() {
        s.compare.problem = Some("No conversation folder.".into());
        return;
    }
    let Some(runner) = ensure_reruns(s) else { return };
    match runner.start(&cmp_id, times, false) {
        Ok(_) => {
            s.compare.problem = None;
            reload_compares(s);
        }
        Err(why) => {
            s.compare.problem = Some(why);
        }
    }
}

pub(super) fn stop_benchmark_rerun(s: &mut State) {
    let Some(cmp_id) = s.compare.active_id.as_deref() else { return };
    if let Some(runner) = &s.compare.reruns {
        runner.stop(cmp_id);
    }
}

pub(super) fn use_model_for_teammate(s: &mut State, runtime: &str, model: &str) {
    let Some(cmp_id) = s.compare.active_id.as_deref() else { return };
    let Some(tm_id) = s
        .compare
        .compares
        .iter()
        .find(|c| c.compare_id == cmp_id)
        .and_then(|c| c.teammate_id.clone())
    else {
        return;
    };
    if let Some(tm) = s.teammates.iter_mut().find(|t| t.id == tm_id) {
        tm.runtime = Some(runtime.to_owned());
        tm.model = Some(model.to_owned());
    }
}

pub(super) fn poll(s: &mut State) -> bool {
    let Some(store) = s.data.clone() else { return false };
    let mut changed = false;
    if !s.compare.loaded || s.compare.rerun_dirty.swap(false, Ordering::AcqRel) {
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
    let mut collect_cmp = |c: &Compare| {
        for col in &c.slots {
            ids.extend(col.mission_ids.iter().cloned());
        }
        ids.extend(newest_judge_mission_ids(c.judge.as_ref()));
    };
    collect_cmp(&cmp);
    for rid in &cmp.reruns {
        if let Some(rc) = s.compare.compares.iter().find(|c| &c.compare_id == rid) {
            collect_cmp(rc);
        }
    }
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

fn snapshot_span_ms(snap: &ledger::Snapshot) -> Option<u64> {
    let first = snap.timings.first()?.first_at.as_deref()?;
    let last = snap.timings.last()?.last_at.as_deref()?;
    let t0 = chrono::DateTime::parse_from_rfc3339(first).ok()?;
    let t1 = chrono::DateTime::parse_from_rfc3339(last).ok()?;
    let ms = (t1 - t0).num_milliseconds();
    (ms > 0).then_some(ms as u64)
}

fn format_span(snap: &ledger::Snapshot) -> Option<String> {
    snapshot_span_ms(snap).map(ledger::duration_text)
}

fn snapshot_cost(_snap: &ledger::Snapshot) -> Option<RunCost> {
    None
}

fn format_cost(snap: &ledger::Snapshot) -> Option<String> {
    cost_line(snapshot_cost(snap).as_ref())
}

pub(super) fn benchmark_runs_for(s: &State, origin: &Compare) -> Vec<BenchmarkRunInput> {
    let mut runs_cmps: Vec<&Compare> = vec![origin];
    for rid in &origin.reruns {
        if let Some(rc) = s.compare.compares.iter().find(|c| &c.compare_id == rid) {
            runs_cmps.push(rc);
        }
    }
    let setups = has_column_instructions(origin);
    runs_cmps
        .into_iter()
        .map(|cmp| {
            let slot_ids: Vec<CompareSlotId> = cmp.slots.iter().map(|c| c.slot).collect();
            let slot_strs: Vec<&str> = slot_ids.iter().map(|sl| sl.as_str()).collect();
            let scorecard = cmp.judge.as_ref().and_then(|j| {
                let points = criteria_lines(j.criteria.as_deref());
                let weights = criteria_weights(j.criteria.as_deref());
                let first_text = j
                    .mission_ids
                    .last()
                    .and_then(|id| s.compare.snapshots.get(id))
                    .map(snapshot_final_text)
                    .unwrap_or_default();
                if j.panel.is_empty() {
                    weighted(parse_scorecard(&first_text, &slot_strs, &points), &weights)
                } else {
                    let mut cards: Vec<Option<Scorecard>> = Vec::new();
                    let mut all_ids: Vec<Option<&str>> = vec![j.mission_ids.last().map(String::as_str)];
                    for m in &j.panel {
                        all_ids.push(Some(m.mission_id.as_str()));
                    }
                    for id_opt in all_ids {
                        let card = id_opt
                            .and_then(|id| s.compare.snapshots.get(id))
                            .filter(|sn| !sn.running)
                            .map(snapshot_final_text)
                            .filter(|t| !t.is_empty())
                            .and_then(|t| parse_scorecard(&t, &slot_strs, &points));
                        cards.push(card);
                    }
                    let refs: Vec<Option<&Scorecard>> = cards.iter().map(|c| c.as_ref()).collect();
                    weighted(panel_vote(&refs), &weights)
                }
            });

            let answers = cmp
                .slots
                .iter()
                .map(|col| {
                    let snap = col.mission_ids.last().and_then(|id| s.compare.snapshots.get(id));
                    let answered = snap.is_some_and(|sn| !sn.running && !sn.failed);
                    let (met, of) = scorecard
                        .as_ref()
                        .map(|sc| {
                            score_of(
                                sc.verdicts.get(col.slot.as_str()).map(Vec::as_slice),
                                sc.weights.as_deref(),
                            )
                        })
                        .filter(|(_, of)| *of > 0)
                        .map_or((None, None), |(m, o)| (Some(m), Some(o)));
                    let check = snap
                        .and_then(|sn| snapshot_check(sn, &s.running_checks))
                        .map(|c| match c.outcome {
                            JudgedCheckOutcome::Passed => BenchmarkCheck::Passed,
                            JudgedCheckOutcome::Failed => BenchmarkCheck::Failed,
                            JudgedCheckOutcome::TimedOut => BenchmarkCheck::TimedOut,
                            _ => BenchmarkCheck::CouldNotRun,
                        });
                    let key = if setups {
                        format!("{}:{}#{}", col.route.runtime, col.route.model, col.slot.as_str())
                    } else {
                        format!("{}:{}", col.route.runtime, col.route.model)
                    };
                    let name = col
                        .route
                        .label
                        .clone()
                        .unwrap_or_else(|| col.route.model.clone());
                    BenchmarkAnswer {
                        key,
                        runtime: col.route.runtime.clone(),
                        model: col.route.model.clone(),
                        effort: col.route.effort.clone(),
                        name,
                        answered,
                        met,
                        of,
                        span_ms: snap.and_then(snapshot_span_ms),
                        cost: snap.and_then(snapshot_cost),
                        check,
                        slot: setups.then_some(col.slot),
                    }
                })
                .collect();

            BenchmarkRunInput {
                compare_id: cmp.compare_id.clone(),
                answers,
            }
        })
        .collect()
}

pub(super) fn benchmark_entries(s: &State) -> Vec<BenchmarkEntry> {
    let prog_list = s
        .compare
        .rerun_progress
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    s.compare
        .compares
        .iter()
        .filter(|c| c.rerun_of.is_none() && !c.reruns.is_empty())
        .map(|origin| {
            let runs_input = benchmark_runs_for(s, origin);
            let rows = benchmark_rows(&runs_input);
            let models = origin
                .slots
                .iter()
                .map(|c| c.route.label.clone().unwrap_or_else(|| c.route.model.clone()))
                .collect();
            let judge_name = origin
                .judge
                .as_ref()
                .map(|j| j.route.label.clone().unwrap_or_else(|| j.route.model.clone()));
            let last_at = origin
                .reruns
                .last()
                .and_then(|rid| s.compare.compares.iter().find(|c| &c.compare_id == rid))
                .map_or_else(|| origin.created_at.clone(), |c| c.created_at.clone());
            let progress = prog_list
                .iter()
                .find(|p| p.origin_id == origin.compare_id)
                .cloned();
            BenchmarkEntry {
                compare_id: origin.compare_id.clone(),
                question: origin.prompt.clone(),
                teammate_id: origin.teammate_id.clone(),
                models,
                runs: 1 + origin.reruns.len(),
                judge_name,
                last_at,
                rows,
                progress,
                changes: origin.changes,
            }
        })
        .collect()
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
    let mut col_failed: Vec<bool> = Vec::new();
    let mut col_keepable: Vec<bool> = Vec::new();
    let mut col_pages: Vec<Vec<String>> = Vec::new();

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
        let failed = newest.is_some_and(|snap| snap.failed);
        let ans = newest.map(snapshot_final_text).unwrap_or_default();
        let span = newest.and_then(format_span);
        let cost = if veiled { None } else { newest.and_then(format_cost) };
        let check = newest.and_then(|snap| snapshot_check(snap, &s.running_checks));
        let keepable = !running && !failed && !ans.is_empty();
        let pages = newest.map(column_html_pages).unwrap_or_default();

        col_names.push(name);
        col_answers.push(ans);
        col_spans.push(span);
        col_costs.push(cost);
        col_checks.push(check);
        col_running.push(running);
        col_failed.push(failed);
        col_keepable.push(keepable);
        col_pages.push(pages);
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
        let failed = col_failed[idx];
        let keepable = col_keepable[idx];
        let pages = &col_pages[idx];
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
        } else if failed {
            "failed"
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

        for page in pages {
            let slot_id = slot.slot;
            let req = page.clone();
            let short = page.rsplit(['/', '\\']).next().unwrap_or(page.as_str()).to_owned();
            cell = cell.child(
                crate::look::controls::button(
                    &l,
                    &format!("Preview {short}"),
                    crate::look::controls::Tone::Neutral,
                    false,
                )
                .key(format!("compare-preview-{}-{short}", slot.slot.as_str()))
                .on_press(move |s: &mut State| preview_column_page(s, slot_id, &req)),
            );
        }

        // Footer with line changes, span, cost (hidden when veiled), and Keep / Try again buttons
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

        if cmp.kept.is_none() && cmp.rerun_of.is_none() {
            let slot_id = slot.slot;
            if !keepable && !running {
                foot = foot.child(
                    crate::look::controls::button(
                        &l,
                        "Try again",
                        crate::look::controls::Tone::Neutral,
                        false,
                    )
                    .key(format!("compare-retry-{}", slot.slot.as_str()))
                    .on_press(move |s: &mut State| retry_column(s, slot_id)),
                );
            }
            let mut keep_btn = crate::look::controls::button(
                &l,
                "Keep this one",
                crate::look::controls::Tone::Active,
                !keepable,
            )
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

    // Follow-up / build on answers & add another model
    if cmp.kept.is_none() && cmp.rerun_of.is_none() && !any_col_running {
        let mut follow_box = col()
            .key("compare-followup-box")
            .w_full()
            .padding(14.0)
            .gap(8.0)
            .radius(8.0)
            .fill(p.raised)
            .border(1.0, p.hairline)
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(
                        text_input(&s.compare.followup, type_::BODY)
                            .key("compare-followup-input")
                            .single_line()
                            .placeholder("Ask a follow-up to all columns…", p.muted)
                            .grow(1.0)
                            .on_edit(|s: &mut State| &mut s.compare.followup),
                    )
                    .child(
                        crate::look::controls::button(
                            &l,
                            "Ask all",
                            crate::look::controls::Tone::Active,
                            false,
                        )
                        .key("compare-followup-send")
                        .on_press(|s: &mut State| ask_compare(s)),
                    ),
            );

        if !cmp.changes && cmp.slots.len() < MAX_COMPARE_SLOTS {
            if cmp.slots.iter().all(|c| c.mission_ids.len() <= 1) {
                let choices: Vec<ModelChoice> = available_choices(s)
                    .into_iter()
                    .filter(|c| !cmp.slots.iter().any(|sl| sl.route.runtime == c.runtime && sl.route.model == c.model))
                    .collect();
                if !choices.is_empty() {
                    follow_box = follow_box.child(
                        crate::look::controls::button(
                            &l,
                            "Add another model",
                            crate::look::controls::Tone::Neutral,
                            false,
                        )
                        .key("compare-add-model")
                        .on_press(|s: &mut State| {
                            s.compare.add_model_menu = !s.compare.add_model_menu;
                        }),
                    );
                    if s.compare.add_model_menu {
                        let mut menu = row().w_full().gap(6.0).wrap();
                        for choice in choices.into_iter().take(6) {
                            let label = choice.label.clone();
                            let ch = choice.clone();
                            menu = menu.child(
                                crate::look::controls::button(
                                    &l,
                                    &label,
                                    crate::look::controls::Tone::Neutral,
                                    false,
                                )
                                .key(format!("compare-add-choice-{}", choice.key))
                                .on_press(move |s: &mut State| add_compare_column(s, ch.clone())),
                            );
                        }
                        follow_box = follow_box.child(menu);
                    }
                }
            } else {
                follow_box = follow_box.child(
                    text("Another model can join only before the first follow-up: it would be asked the first question and miss the turns since. Start a new comparison to add one.")
                        .font(l.ty.label)
                        .color(p.muted)
                        .wrap(),
                );
            }
        }
        scroll_body = scroll_body.child(follow_box);
    }

    if let Some(origin_id) = &cmp.rerun_of {
        let oid = origin_id.clone();
        scroll_body = scroll_body.child(
            row()
                .key("compare-rerun-back")
                .w_full()
                .padding(12.0)
                .gap(10.0)
                .center_y()
                .radius(8.0)
                .fill(p.raised)
                .border(1.0, p.hairline)
                .child(
                    text("A run of your benchmark.")
                        .font(l.ty.body)
                        .color(p.muted)
                        .grow(1.0),
                )
                .child(
                    crate::look::controls::button(
                        &l,
                        "Back to benchmark",
                        crate::look::controls::Tone::Neutral,
                        false,
                    )
                    .key("compare-open-origin")
                    .on_press(move |s: &mut State| {
                        s.compare.active_id = Some(oid.clone());
                    }),
                ),
        );
    } else {
        scroll_body = scroll_body.child(render_benchmark_section(s, cmp, p, &l));
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

fn render_benchmark_section(
    s: &State,
    origin: &Compare,
    p: &Palette,
    l: &crate::look::controls::Look,
) -> El {
    let prog = s
        .compare
        .rerun_progress
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|pr| pr.origin_id == origin.compare_id)
        .cloned();
    let refusal = rerun_refusal_of(origin);
    let three_judges = origin.judge.as_ref().is_some_and(|j| !j.panel.is_empty());
    let setups = has_column_instructions(origin);
    let same_start = origin
        .base
        .as_deref()
        .map(|b| format!(", from commit {}", &b[..b.len().min(7)]))
        .unwrap_or_default();

    let runs_input = benchmark_runs_for(s, origin);
    let rows: Vec<BenchmarkRow> = benchmark_rows(&runs_input);
    let reran = runs_input.len() > 1;

    let actions = if let Some(why) = refusal {
        text(why).font(l.ty.label).color(p.muted).wrap()
    } else if let Some(pr) = &prog {
        row()
            .gap(8.0)
            .center_y()
            .child(
                text(step_words(pr, three_judges))
                    .font(l.ty.body)
                    .color(p.text),
            )
            .child(
                crate::look::controls::button(
                    l,
                    "Stop",
                    crate::look::controls::Tone::Neutral,
                    false,
                )
                .key("benchmark-stop")
                .on_press(|s: &mut State| stop_benchmark_rerun(s)),
            )
    } else {
        row()
            .gap(8.0)
            .center_y()
            .child(
                crate::look::controls::button(
                    l,
                    "Run 3 more times",
                    if reran {
                        crate::look::controls::Tone::Neutral
                    } else {
                        crate::look::controls::Tone::Active
                    },
                    false,
                )
                .key("benchmark-run-3")
                .on_press(|s: &mut State| start_benchmark_rerun(s, 3)),
            )
            .child(
                crate::look::controls::button(
                    l,
                    "Run once more",
                    crate::look::controls::Tone::Neutral,
                    false,
                )
                .key("benchmark-run-1")
                .on_press(|s: &mut State| start_benchmark_rerun(s, 1)),
            )
    };

    let mut box_col = col()
        .key("compare-benchmark")
        .w_full()
        .padding(14.0)
        .gap(10.0)
        .radius(8.0)
        .fill(p.raised)
        .border(1.0, p.hairline);

    if !reran {
        let lead = if setups {
            format!(
                "One run can be luck. Run the three setups again and keep the one whose scores hold: the same model and the same question{same_start}, each setup with its own instructions, scored by {} on the same points.",
                if three_judges { "the same three judges" } else { "the same judge" }
            )
        } else {
            format!(
                "One run can be luck. Run this comparison again and see whether the scores hold: the same models, the same question{same_start}, scored by {} on the same points.",
                if three_judges { "the same three judges" } else { "the same judge" }
            )
        };
        return box_col
            .child(text("YOUR BENCHMARK").font(l.ty.label).color(p.muted))
            .child(text(lead).font(l.ty.body).color(p.text).wrap())
            .child(actions);
    }

    box_col = box_col.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(
                text(format!("YOUR BENCHMARK · {} RUNS", runs_input.len()))
                    .font(l.ty.label)
                    .color(p.muted)
                    .grow(1.0),
            )
            .child(actions),
    );

    let best = rows.iter().filter_map(|r| r.score).max();
    let alone = best.is_some_and(|b| rows.iter().filter(|r| r.score == Some(b)).count() == 1);
    let checked = rows.iter().any(|r| r.checked > 0);
    let tm = origin
        .teammate_id
        .as_deref()
        .and_then(|id| s.teammates.iter().find(|t| t.id == id));
    let tm_key = tm.and_then(|t| Some(format!("{}:{}", t.runtime.as_deref()?, t.model.as_deref()?)));

    let mut table = col()
        .key("benchmark-table")
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .radius(6.0)
        .fill(p.bg)
        .border(1.0, p.hairline);

    let mut hdr = row()
        .w_full()
        .gap(8.0)
        .pb(4.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
        .child(
            text(if setups { "Setup" } else { "Model" })
                .font(l.ty.label)
                .color(p.muted)
                .grow(2.0),
        )
        .child(text("Score").font(l.ty.label).color(p.muted).grow(1.0))
        .child(text("Met every point").font(l.ty.label).color(p.muted).grow(1.0));
    if checked {
        hdr = hdr.child(text("Check passed").font(l.ty.label).color(p.muted).grow(1.0));
    }
    hdr = hdr
        .child(text("Time a task").font(l.ty.label).color(p.muted).grow(1.0))
        .child(text("Cost a task").font(l.ty.label).color(p.muted).grow(1.0));
    if tm.is_some() {
        hdr = hdr.child(text("").grow(1.0));
    }
    table = table.child(hdr);

    for r in &rows {
        let is_best = alone && best.is_some() && r.score == best;
        let score_txt = r
            .score
            .map_or_else(|| "not scored".to_owned(), |v| v.to_string());
        let every_txt = if r.scored == 0 {
            "—".to_owned()
        } else {
            format!("{} of {}", r.every_point, r.scored)
        };
        let time_txt = r
            .span_ms
            .map_or_else(|| "—".to_owned(), ledger::duration_text);
        let cost_txt = cost_cell(r.cost.as_ref(), &r.model);
        let mut tr = row()
            .key(format!("benchmark-row-{}", r.key))
            .w_full()
            .gap(8.0)
            .center_y()
            .child(text(r.name.clone()).font(l.ty.body).color(p.text).grow(2.0))
            .child(
                text(score_txt)
                    .font(l.ty.body)
                    .color(if is_best { p.green } else { p.text })
                    .grow(1.0),
            )
            .child(text(every_txt).font(l.ty.body).color(p.text).grow(1.0));
        if checked {
            let chk_txt = if r.checked == 0 {
                "—".to_owned()
            } else {
                format!("{} of {}", r.check_passed, r.checked)
            };
            tr = tr.child(text(chk_txt).font(l.ty.body).color(p.text).grow(1.0));
        }
        tr = tr
            .child(text(time_txt).font(l.ty.body).color(p.text).grow(1.0))
            .child(text(cost_txt).font(l.ty.body).color(p.text).grow(1.0));
        if let Some(t) = tm {
            if tm_key.as_deref() == Some(r.key.as_str()) {
                tr = tr.child(
                    text(format!("{}'s model", t.name))
                        .font(l.ty.label)
                        .color(p.muted)
                        .grow(1.0),
                );
            } else {
                let rt = r.runtime.clone();
                let mdl = r.model.clone();
                tr = tr.child(
                    crate::look::controls::button(
                        l,
                        &format!("Use for {}", t.name),
                        crate::look::controls::Tone::Neutral,
                        false,
                    )
                    .key(format!("benchmark-use-{}", r.key))
                    .on_press(move |s: &mut State| use_model_for_teammate(s, &rt, &mdl)),
                );
            }
        }
        table = table.child(tr);
    }
    box_col = box_col.child(table);

    if let Some(chart) = benchmark_chart(&rows) {
        let axis_note = match chart.axis {
            ChartAxis::Cost => "Score against cost a task: Up is better, left is cheaper.",
            ChartAxis::Time => match chart.because {
                Some(TimeBecause::SamePrice) => {
                    "Score against time a task: every one cost the same here, so the chart is against time."
                }
                _ => {
                    "Score against time a task: too few of these runs were priced in dollars to chart cost."
                }
            },
        };
        let pts_summary: Vec<String> = chart
            .points
            .iter()
            .map(|pt| {
                let x_lbl = if chart.axis == ChartAxis::Cost {
                    if pt.free {
                        "free".to_owned()
                    } else if pt.x < 0.1 {
                        format!("${:.3}", pt.x)
                    } else {
                        format!("${:.2}", pt.x)
                    }
                } else {
                    ledger::duration_text(pt.x.round() as u64)
                };
                format!("{} · {} · {x_lbl}", pt.name, pt.score)
            })
            .collect();
        box_col = box_col.child(
            col()
                .key("benchmark-chart")
                .w_full()
                .gap(4.0)
                .child(text(axis_note).font(l.ty.label).color(p.muted).wrap())
                .child(text(pts_summary.join("   ")).font(l.ty.label).color(p.text).wrap()),
        );
    }

    let mut chips = row().w_full().gap(6.0).wrap();
    for (idx, run) in runs_input.iter().enumerate() {
        let cid = run.compare_id.clone();
        let lbl = if idx == 0 {
            "Run 1 · this one".to_owned()
        } else {
            format!("Run {}", idx + 1)
        };
        chips = chips.child(
            crate::look::controls::button(
                l,
                &lbl,
                crate::look::controls::Tone::Neutral,
                false,
            )
            .key(format!("benchmark-open-run-{cid}"))
            .on_press(move |s: &mut State| {
                s.compare.active_id = Some(cid.clone());
            }),
        );
    }
    box_col.child(chips)
}
