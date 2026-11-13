//! Starter routines (`main/routine-templates.ts`) and chain teammate proposal (`shared/chain-proposal.ts`).

use crate::routine_file::{MAX_ROUTINE_FILE_BYTES, RoutineFile, parse_routine_file};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::Path;

pub const TEMPLATE_SUFFIX: &str = ".locust-routine.json";

pub const TEMPLATE_ORDER: &[&str] = &[
    "explain-this-project",
    "find-what-is-unfinished",
    "review-a-file",
    "council-review",
    "fix-a-bug",
    "build-a-feature",
    "make-it-faster",
    "polish-overnight",
    "plan-a-change",
    "challenge-an-idea",
    "write-a-status-update",
    "weekly-report-from-notes",
    "summarize-a-long-text",
    "draft-a-reply",
    "check-a-spreadsheet",
    "find-the-gaps-in-a-document",
];

fn what_it_does(id: &str) -> Option<&'static str> {
    Some(match id {
        "explain-this-project" => "What this folder is, how it is laid out and how to run it, for someone new.",
        "find-what-is-unfinished" => "Every TODO and half-done part, grouped by how much it matters.",
        "review-a-file" => "A careful review of one file: bugs, unclear names, missing checks.",
        "council-review" => "Three teammates, each on a different AI where your team has one, review the same change, and the third keeps what two would stand behind.",
        "fix-a-bug" => "Find the cause, fix it with the smallest change, and have the fix checked where it broke.",
        "build-a-feature" => "A plan, the build, and a review of the build against the plan.",
        "make-it-faster" => "Measure where the time goes, change the slowest part, then measure again.",
        "polish-overnight" => "Set it to run each night: it runs the tests, fixes what fails in a copy for you to keep or discard, and leaves a report for the morning.",
        "plan-a-change" => "A step-by-step plan for a change, then the same plan read back sceptically.",
        "challenge-an-idea" => "An architect lists what the idea is missing, plans it, and a checker judges the plan.",
        "write-a-status-update" => "A short update on this project for your team, manager or client.",
        "weekly-report-from-notes" => "Your notes as Done, In progress, Blocked and Next week.",
        "summarize-a-long-text" => "A two-minute summary of a long text, with its decisions and open questions.",
        "draft-a-reply" => "A short reply to a message, in the tone you choose.",
        "check-a-spreadsheet" => "Missing values, duplicates and odd numbers in a CSV, and whether to trust it.",
        "find-the-gaps-in-a-document" => "Unsupported claims, missing steps and places a reader would get stuck.",
        _ => return None,
    })
}

pub fn template_changes_files(id: &str) -> bool {
    matches!(
        id,
        "fix-a-bug" | "build-a-feature" | "make-it-faster" | "polish-overnight"
    )
}

pub fn template_on_another_agent(id: &str) -> Option<&'static [bool]> {
    match id {
        "council-review" => Some(&[false, true, true]),
        _ => None,
    }
}

pub const SHIPPED_TEMPLATES: &[(&str, &str)] = &[
    ("explain-this-project", include_str!("../../assets/routines/explain-this-project.locust-routine.json")),
    ("find-what-is-unfinished", include_str!("../../assets/routines/find-what-is-unfinished.locust-routine.json")),
    ("review-a-file", include_str!("../../assets/routines/review-a-file.locust-routine.json")),
    ("council-review", include_str!("../../assets/routines/council-review.locust-routine.json")),
    ("fix-a-bug", include_str!("../../assets/routines/fix-a-bug.locust-routine.json")),
    ("build-a-feature", include_str!("../../assets/routines/build-a-feature.locust-routine.json")),
    ("make-it-faster", include_str!("../../assets/routines/make-it-faster.locust-routine.json")),
    ("polish-overnight", include_str!("../../assets/routines/polish-overnight.locust-routine.json")),
    ("plan-a-change", include_str!("../../assets/routines/plan-a-change.locust-routine.json")),
    ("challenge-an-idea", include_str!("../../assets/routines/challenge-an-idea.locust-routine.json")),
    ("write-a-status-update", include_str!("../../assets/routines/write-a-status-update.locust-routine.json")),
    ("weekly-report-from-notes", include_str!("../../assets/routines/weekly-report-from-notes.locust-routine.json")),
    ("summarize-a-long-text", include_str!("../../assets/routines/summarize-a-long-text.locust-routine.json")),
    ("draft-a-reply", include_str!("../../assets/routines/draft-a-reply.locust-routine.json")),
    ("check-a-spreadsheet", include_str!("../../assets/routines/check-a-spreadsheet.locust-routine.json")),
    ("find-the-gaps-in-a-document", include_str!("../../assets/routines/find-the-gaps-in-a-document.locust-routine.json")),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutineTemplateInfo {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub steps: usize,
    pub asks: Vec<String>,
    pub chain: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedTemplates {
    pub templates: Vec<RoutineTemplateInfo>,
    pub refused: Vec<String>,
}

fn rank(id: &str) -> usize {
    TEMPLATE_ORDER
        .iter()
        .position(|&entry| entry == id)
        .unwrap_or(TEMPLATE_ORDER.len())
}

fn summary_for(id: &str, file: &RoutineFile) -> String {
    if let Some(s) = what_it_does(id) {
        return s.to_owned();
    }
    let first = file.steps.first().map(String::as_str).unwrap_or("");
    let sentence = first
        .split_inclusive(['.', '!', '?'])
        .next()
        .unwrap_or(first);
    sentence.chars().take(160).collect()
}

fn info_from_file(id: &str, file: &RoutineFile) -> RoutineTemplateInfo {
    RoutineTemplateInfo {
        id: id.to_owned(),
        name: file.name.clone(),
        summary: summary_for(id, file),
        steps: file.steps.len(),
        asks: file.inputs.iter().map(|i| i.label.clone()).collect(),
        chain: file.hand_offs.iter().any(|h| h.check == Some(true)),
    }
}

/// All shipped starter routines, in `TEMPLATE_ORDER`.
pub fn list_shipped_templates() -> Vec<RoutineTemplateInfo> {
    let mut out = Vec::new();
    for &(id, text) in SHIPPED_TEMPLATES {
        if let Ok(file) = parse_routine_file(text) {
            out.push(info_from_file(id, &file));
        }
    }
    out.sort_by(|a, b| rank(&a.id).cmp(&rank(&b.id)).then_with(|| a.name.cmp(&b.name)));
    out
}

/// Read one shipped starter routine by id.
pub fn read_shipped_template(id: &str) -> Result<RoutineFile, String> {
    if !valid_template_id(id) {
        return Err("That template is not one Locust ships.".into());
    }
    let Some(&(_, text)) = SHIPPED_TEMPLATES.iter().find(|&&(k, _)| k == id) else {
        return Err("That template is not one Locust ships.".into());
    };
    parse_routine_file(text)
}

fn valid_template_id(id: &str) -> bool {
    (1..=80).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// List every template in `directory` that reads as a routine file, in `TEMPLATE_ORDER`.
pub fn list_routine_templates_in_dir(directory: &Path) -> ListedTemplates {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return ListedTemplates {
            templates: Vec::new(),
            refused: Vec::new(),
        };
    };
    let mut templates = Vec::new();
    let mut refused = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(TEMPLATE_SUFFIX) {
            continue;
        }
        let id = &name[..name.len() - TEMPLATE_SUFFIX.len()];
        let text = match std::fs::read_to_string(entry.path()) {
            Ok(t) => t,
            Err(_) => {
                refused.push(format!("{name}: That template could not be read."));
                continue;
            }
        };
        if text.len() > MAX_ROUTINE_FILE_BYTES {
            refused.push(format!("{name}: That template is too large to be a routine."));
            continue;
        }
        match parse_routine_file(&text) {
            Ok(file) => templates.push(info_from_file(id, &file)),
            Err(msg) => refused.push(format!("{name}: {msg}")),
        }
    }
    templates.sort_by(|a, b| rank(&a.id).cmp(&rank(&b.id)).then_with(|| a.name.cmp(&b.name)));
    ListedTemplates { templates, refused }
}

/// Read one template by id from `directory`: an id is never a path.
pub fn read_routine_template_in_dir(directory: &Path, id: &str) -> Result<RoutineFile, String> {
    if !valid_template_id(id) {
        return Err("That template is not one Locust ships.".into());
    }
    let target_name = format!("{id}{TEMPLATE_SUFFIX}");
    let exists = std::fs::read_dir(directory)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.file_name().to_string_lossy() == target_name);
    if !exists {
        return Err("That template is not one Locust ships.".into());
    }
    let text = std::fs::read_to_string(directory.join(&target_name))
        .map_err(|_| "That template could not be read.".to_string())?;
    if text.len() > MAX_ROUTINE_FILE_BYTES {
        return Err("That template is too large to be a routine.".into());
    }
    parse_routine_file(&text)
}

/// `templateMeta` from `RoutineTemplates.tsx`.
pub fn template_meta(info: &RoutineTemplateInfo) -> String {
    let steps = if info.steps == 1 {
        "1 step".to_owned()
    } else {
        format!("{} steps", info.steps)
    };
    let asks = match info.asks.len() {
        0 => "no questions".to_owned(),
        1 => "1 question".to_owned(),
        n => format!("{n} questions"),
    };
    format!("{steps} · {asks}")
}

/// `templatesLine` from `RoutineTemplates.tsx`.
pub fn templates_line(templates: &[RoutineTemplateInfo]) -> String {
    let first: Vec<&str> = templates.iter().take(3).map(|t| t.name.as_str()).collect();
    if templates.len() <= 3 {
        first.join(", ")
    } else {
        format!("{} and {} more", first.join(", "), templates.len() - 3)
    }
}

// --- Chain teammate proposal (`shared/chain-proposal.ts`) ---

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChainStep {
    pub role: Option<String>,
    pub check: bool,
    pub another: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposableTeammate {
    pub teammate_id: String,
    pub role: String,
    pub role_title: Option<String>,
    pub runtime: Option<String>,
}

impl ProposableTeammate {
    pub fn role_label(&self) -> &str {
        if self.role == "Custom" {
            self.role_title
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("Custom")
        } else {
            &self.role
        }
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let id = v["teammateId"].as_str()?.to_owned();
        let role = v["role"].as_str().unwrap_or("Custom").to_owned();
        let role_title = v["roleTitle"].as_str().map(ToOwned::to_owned);
        let runtime = v["route"]["runtime"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);
        Some(Self {
            teammate_id: id,
            role,
            role_title,
            runtime,
        })
    }
}

fn same_role(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
}

/// One teammate id per step, in step order; empty when there is no one to propose.
pub fn propose_teammates(steps: &[ChainStep], team: &[ProposableTeammate]) -> Vec<String> {
    if team.is_empty() {
        return Vec::new();
    }
    let mut picked: Vec<String> = Vec::with_capacity(steps.len());
    for step in steps {
        let matching: Vec<&ProposableTeammate> = match step.role.as_deref() {
            None => Vec::new(),
            Some(role) => team
                .iter()
                .filter(|mate| same_role(mate.role_label(), role))
                .collect(),
        };
        let fresh = |mates: &[&ProposableTeammate]| -> Option<String> {
            mates
                .iter()
                .find(|mate| !picked.iter().any(|p| p == &mate.teammate_id))
                .map(|m| m.teammate_id.clone())
        };
        let used_runtimes: HashSet<Option<&str>> = picked
            .iter()
            .map(|id| {
                team.iter()
                    .find(|m| &m.teammate_id == id)
                    .and_then(|m| m.runtime.as_deref())
            })
            .collect();
        let elsewhere = |mates: &[&ProposableTeammate]| -> Option<String> {
            mates
                .iter()
                .find(|mate| !used_runtimes.contains(&mate.runtime.as_deref()))
                .map(|m| m.teammate_id.clone())
        };
        let all_refs: Vec<&ProposableTeammate> = team.iter().collect();
        let mut chosen = if step.another {
            elsewhere(&matching)
                .or_else(|| elsewhere(&all_refs))
                .or_else(|| fresh(&matching))
                .or_else(|| fresh(&all_refs))
        } else {
            None
        };
        if chosen.is_none() && !matching.is_empty() {
            chosen = if step.check { fresh(&matching) } else { None }
                .or_else(|| Some(matching[0].teammate_id.clone()));
        } else if chosen.is_none() && step.check {
            chosen = fresh(&all_refs);
        }
        if chosen.is_none() {
            let before = picked.last().cloned();
            chosen = before
                .and_then(|b| team.iter().find(|m| m.teammate_id == b).map(|m| m.teammate_id.clone()))
                .or_else(|| Some(team[0].teammate_id.clone()));
        }
        picked.push(chosen.unwrap());
    }
    picked
}

/// The proposal as a routine stores it (`owner`, `handOffs` JSON array).
pub fn proposed_hand_offs(
    steps: &[ChainStep],
    team: &[ProposableTeammate],
) -> (Option<String>, Vec<Value>) {
    let picked = propose_teammates(steps, team);
    let owner = picked.first().cloned();
    let hand_offs = steps
        .iter()
        .enumerate()
        .map(|(at, step)| {
            let mut obj = serde_json::Map::new();
            if let Some(p) = picked.get(at)
                && owner.as_ref() != Some(p)
            {
                obj.insert("teammateId".into(), json!(p));
            }
            if step.check {
                obj.insert("check".into(), json!(true));
            }
            Value::Object(obj)
        })
        .collect();
    (owner, hand_offs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routine_file::absolute_paths_in;

    fn mate(id: &str, role: &str, role_title: Option<&str>) -> ProposableTeammate {
        ProposableTeammate {
            teammate_id: id.into(),
            role: role.into(),
            role_title: role_title.map(ToOwned::to_owned),
            runtime: Some("opencode".into()),
        }
    }

    #[test]
    fn all_16_shipped_starter_routines_parse_and_meet_rules() {
        let listed = list_shipped_templates();
        assert_eq!(listed.len(), 16);
        let ids: Vec<&str> = listed.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, TEMPLATE_ORDER);

        let mut names = HashSet::new();
        for &(id, text) in SHIPPED_TEMPLATES {
            let file = parse_routine_file(text).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(file.route.runtime, None, "{id} should name no route");
            assert!(file.connectors.is_empty(), "{id} should name no connector");
            assert!(absolute_paths_in(&file.steps).is_empty(), "{id} has absolute path");
            for input in &file.inputs {
                let needle = format!("{{{{{}}}}}", input.key);
                assert!(file.steps.iter().any(|s| s.contains(&needle)), "{id}: {}", input.key);
            }
            assert!(names.insert(file.name), "duplicate name in {id}");
        }

        let challenge = listed.iter().find(|t| t.id == "challenge-an-idea").unwrap();
        assert_eq!(challenge.name, "Challenge an idea before building it");
        assert_eq!(challenge.steps, 4);
        assert_eq!(challenge.asks, vec!["The idea"]);
        assert!(listed.iter().all(|t| t.summary.len() > 10));
    }

    #[test]
    fn chain_templates_and_proposal_rules() {
        for id in ["fix-a-bug", "build-a-feature", "make-it-faster"] {
            let file = read_shipped_template(id).unwrap();
            assert_eq!(file.steps.len(), 3, "{id}");
            assert_eq!(file.inputs.len(), 1, "{id}");
            assert_eq!(file.hand_offs.last().and_then(|h| h.check), Some(true));
            assert!(template_changes_files(id));
        }
        assert!(template_changes_files("polish-overnight"));
        assert!(!template_changes_files("review-a-file"));
        assert!(!template_changes_files("challenge-an-idea"));

        let fix = vec![
            ChainStep { role: Some("Research & Briefs".into()), check: false, another: false },
            ChainStep { role: Some("Code & Migrations".into()), check: false, another: false },
            ChainStep { role: Some("Docs & QA".into()), check: true, another: false },
        ];
        let wren = mate("tm_wren", "Custom", Some("Everything"));
        assert_eq!(propose_teammates(&fix, &[wren.clone()]), vec!["tm_wren", "tm_wren", "tm_wren"]);
        let (owner, ho) = proposed_hand_offs(&fix, &[wren]);
        assert_eq!(owner.as_deref(), Some("tm_wren"));
        assert_eq!(ho, vec![json!({}), json!({}), json!({"check": true})]);

        let team = vec![
            mate("tm_wren", "Code & Migrations", None),
            mate("tm_atlas", "Research & Briefs", None),
            mate("tm_sable", "Docs & QA", None),
            mate("tm_ops", "Ops & Scheduling", None),
        ];
        assert_eq!(propose_teammates(&fix, &team), vec!["tm_atlas", "tm_wren", "tm_sable"]);
        let (owner, ho) = proposed_hand_offs(&fix, &team);
        assert_eq!(owner.as_deref(), Some("tm_atlas"));
        assert_eq!(
            ho,
            vec![json!({}), json!({"teammateId": "tm_wren"}), json!({"teammateId": "tm_sable", "check": true})]
        );

        // Prefers fresh checker when two teammates share a role.
        let qa_pair = vec![mate("tm_a", "Docs & QA", None), mate("tm_b", "Docs & QA", None)];
        assert_eq!(propose_teammates(&fix, &qa_pair), vec!["tm_a", "tm_a", "tm_b"]);

        // Empty roster proposes nobody.
        assert!(propose_teammates(&fix, &[]).is_empty());
    }

    #[test]
    fn template_meta_and_line_match_electron() {
        let listed = list_shipped_templates();
        assert_eq!(template_meta(&listed[0]), "1 step · no questions");
        let challenge = listed.iter().find(|t| t.id == "challenge-an-idea").unwrap();
        assert_eq!(template_meta(challenge), "4 steps · 1 question");
        let draft = listed.iter().find(|t| t.id == "draft-a-reply").unwrap();
        assert_eq!(template_meta(draft), "1 step · 3 questions");
        assert_eq!(
            templates_line(&listed[..2]),
            "Explain this project, Find what is unfinished"
        );
    }

    #[test]
    fn dir_reader_refuses_bad_ids_and_names_broken_files() {
        let dir = std::env::temp_dir().join(format!(
            "locust-templates-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("broken.locust-routine.json"), r#"{"format":"locust-routine"}"#).unwrap();
        std::fs::write(dir.join("good.locust-routine.json"), SHIPPED_TEMPLATES[0].1).unwrap();
        let listed = list_routine_templates_in_dir(&dir);
        assert_eq!(listed.templates.len(), 1);
        assert_eq!(listed.templates[0].id, "good");
        assert!(listed.refused[0].starts_with("broken.locust-routine.json: "));
        for bad_id in ["../routines/review-a-file", "..\\x", "C:/Windows/win.ini", "not-a-template"] {
            assert!(read_routine_template_in_dir(&dir, bad_id).is_err());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
