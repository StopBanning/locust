//! A teammate's work is owned by recorded id, never by a display name.
use crate::{benchmark::RunCost, ledger::Summary};
use std::collections::{HashMap, HashSet};
pub const RECENT_MISSION_LIMIT: usize = 3;
pub struct Recent<'a> {
    pub mission: &'a Summary,
    pub title: String,
}
pub struct Work<'a> {
    pub mission_count: usize,
    pub last_run_at: Option<&'a str>,
    pub cost: Option<RunCost>,
    pub recent: Vec<Recent<'a>>,
}
/// How many conversations the record gives `id` (Screens.tsx `owned`): every `missionOwners` entry that names them, whether
/// or not this window has read the conversation itself. A card's Runs says this; `Work::mission_count` only counts
/// the ones read, so it reads 0 for a teammate whose history lives in Electron's record.
pub fn owned_runs(owners: &HashMap<String, String>, id: &str) -> usize {
    owners.values().filter(|owner| owner.as_str() == id).count()
}
/// When each teammate last ran, read from the records themselves (review, Words 1): for every conversation `owners`
/// gives a teammate, the time its record (`<id>.jsonl` in any of `dirs`) was last written, the newest per teammate, as
/// RFC 3339. A card's Runs counts every conversation the record names (`owned_runs`), but the conversation list leaves out
/// what is not a conversation of the person's own (Scout's looks, side chats) and stops at its cap, so a teammate whose
/// runs are all of that kind had runs and no last run: "Runs 167 · Last run never". The time on disk is the record's
/// last write, as every update appends (`ledger::list_recent` picks the newest files by it). Only file times are read.
pub fn recorded_last_runs(owners: &HashMap<String, String>, dirs: &[std::path::PathBuf]) -> HashMap<String, String> {
    let mut newest: HashMap<String, std::time::SystemTime> = HashMap::new();
    for (mission, owner) in owners {
        if !crate::ledger::safe_id(mission) {
            continue;
        }
        let written = dirs
            .iter()
            .filter_map(|dir| std::fs::metadata(dir.join(format!("{mission}.jsonl"))).ok()?.modified().ok())
            .max();
        if let Some(at) = written {
            let kept = newest.entry(owner.clone()).or_insert(at);
            if at > *kept {
                *kept = at;
            }
        }
    }
    newest
        .into_iter()
        .map(|(owner, at)| (owner, chrono::DateTime::<chrono::Utc>::from(at).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)))
        .collect()
}

/// The later of two RFC 3339 times: the listed conversations' newest and the records' (`recorded_last_runs`). One that
/// cannot be read loses to one that can; either alone is the answer.
pub fn last_run<'a>(listed: Option<&'a str>, recorded: Option<&'a str>) -> Option<&'a str> {
    let at = |t: &str| chrono::DateTime::parse_from_rfc3339(t).ok().map(|d| d.timestamp_millis());
    match (listed, recorded) {
        (Some(a), Some(b)) => Some(if at(b) > at(a) { b } else { a }),
        (a, b) => a.or(b),
    }
}

/// What a card's "Last run" says (Screens.tsx:795): how long ago, from `ago`; "never" only for a teammate with no runs;
/// "unknown" for a time that cannot be read, or runs whose records are gone, so a card with runs never says "never".
pub fn last_run_words(runs: usize, at: Option<&str>, ago: impl Fn(&str) -> Option<String>) -> String {
    match at {
        Some(at) => ago(at).unwrap_or_else(|| "unknown".into()),
        None if runs == 0 => "never".into(),
        None => "unknown".into(),
    }
}

pub fn teammate_work<'a>(
    id: &str,
    missions: &'a [Summary],
    owners: &HashMap<String, String>,
    title: impl Fn(&Summary) -> String,
) -> Work<'a> {
    let mut theirs: Vec<_> = missions
        .iter()
        .filter(|m| owners.get(&m.mission_id).is_some_and(|owner| owner == id))
        .collect();
    theirs.sort_by_key(|m| {
        std::cmp::Reverse(
            chrono::DateTime::parse_from_rfc3339(&m.updated)
                .ok()
                .map(|at| at.timestamp_millis()),
        )
    });
    let mut cost: Option<RunCost> = None;
    for c in theirs.iter().filter_map(|m| m.cost.as_ref()) {
        let total = cost.get_or_insert_with(RunCost::default);
        macro_rules! add {
            ($field:ident) => {
                if let Some(n) = c.$field {
                    total.$field = Some(total.$field.unwrap_or_default() + n);
                }
            };
        }
        add!(usd);
        add!(premium_requests);
        add!(input_tokens);
        add!(output_tokens);
        total.plan |= c.plan;
    }
    let by_id: HashMap<_, _> = missions
        .iter()
        .map(|m| (m.mission_id.as_str(), m))
        .collect();
    let mut seen = HashSet::new();
    let mut recent = Vec::new();
    for m in &theirs {
        let mut root = *m;
        let mut visited = HashSet::from([root.mission_id.as_str()]);
        while let Some(parent) = root.parent.as_deref().and_then(|p| by_id.get(p)) {
            if !visited.insert(parent.mission_id.as_str()) {
                break;
            }
            root = parent;
        }
        if !seen.insert(root.mission_id.as_str()) {
            continue;
        }
        recent.push(Recent {
            mission: m,
            title: title(root),
        });
        if recent.len() == RECENT_MISSION_LIMIT {
            break;
        }
    }
    Work {
        mission_count: theirs.len(),
        last_run_at: theirs.first().map(|m| m.updated.as_str()),
        cost,
        recent,
    }
}
#[cfg(test)]
#[path = "teammate_last_run_tests.rs"]
mod teammate_last_run_tests;
#[cfg(test)]
mod tests {
    use super::*;
    fn owners() -> HashMap<String, String> {
        [
            ("m1", "tm_atlas"),
            ("m2", "tm_atlas"),
            ("m3", "tm_bramble"),
            ("m4", "tm_atlas"),
        ]
        .into_iter()
        .map(|(m, t)| (m.into(), t.into()))
        .collect()
    }
    fn missions() -> Vec<Summary> {
        [
            ("m1", "2026-09-01T10:00:00.000Z"),
            ("m2", "2026-09-03T09:00:00.000Z"),
            ("m3", "2026-09-03T11:00:00.000Z"),
            ("m4", "2026-09-02T10:00:00.000Z"),
        ]
        .into_iter()
        .map(|(id, at)| Summary {
            mission_id: id.into(),
            words: format!("do {id}"),
            runtime: "codex".into(),
            model: "gpt".into(),
            created_at: at.into(),
            updated: at.into(),
            ..Default::default()
        })
        .collect()
    }
    fn work<'a>(id: &str, missions: &'a [Summary]) -> Work<'a> {
        teammate_work(id, missions, &owners(), |m| m.words.clone())
    }
    #[test]
    fn counts_only_the_missions_the_host_recorded_as_theirs() {
        assert_eq!(work("tm_atlas", &missions()).mission_count, 3);
        assert_eq!(work("tm_bramble", &missions()).mission_count, 1);
    }
    #[test]
    fn runs_are_the_ones_the_record_names_even_when_none_is_read() {
        // The three the record gives atlas, with no mission read: Work counts none, the card still says 3.
        assert_eq!(work("tm_atlas", &[]).mission_count, 0);
        assert_eq!(owned_runs(&owners(), "tm_atlas"), 3);
        assert_eq!(owned_runs(&owners(), "tm_bramble"), 1);
        assert_eq!(owned_runs(&owners(), "tm_nobody"), 0);
        assert_eq!(owned_runs(&HashMap::new(), "tm_atlas"), 0);
    }
    #[test]
    fn takes_the_last_run_from_the_newest_mission_not_the_first_in_the_list() {
        assert_eq!(
            work("tm_atlas", &missions()).last_run_at,
            Some("2026-09-03T09:00:00.000Z")
        );
    }
    #[test]
    fn lists_the_newest_few_newest_first() {
        let missions = missions();
        let w = work("tm_atlas", &missions);
        assert_eq!(
            w.recent
                .iter()
                .map(|r| r.mission.mission_id.as_str())
                .collect::<Vec<_>>(),
            vec!["m2", "m4", "m1"]
        );
        assert!(w.recent.len() <= RECENT_MISSION_LIMIT);
    }
    #[test]
    fn says_nothing_about_a_teammate_who_has_never_run_rather_than_zeroes() {
        let missions = missions();
        let w = work("tm_nobody", &missions);
        assert_eq!(w.mission_count, 0);
        assert_eq!(w.last_run_at, None);
        assert_eq!(w.cost, None);
    }
    #[test]
    fn adds_up_what_their_missions_reported_and_only_theirs() {
        let mut missions = missions();
        missions.truncate(3);
        missions[1].updated = "2026-09-02T10:00:00.000Z".into();
        missions[2].updated = "2026-09-02T11:00:00.000Z".into();
        for (m, (input, output)) in missions.iter_mut().zip([(100, 10), (200, 20), (999, 99)]) {
            m.cost = Some(RunCost {
                input_tokens: Some(input),
                output_tokens: Some(output),
                ..Default::default()
            });
        }
        assert_eq!(
            work("tm_atlas", &missions).cost,
            Some(RunCost {
                input_tokens: Some(300),
                output_tokens: Some(30),
                ..Default::default()
            })
        );
    }
    #[test]
    fn uses_the_callers_idea_of_a_title_so_a_briefing_is_not_shown_as_the_words_a_person_typed() {
        let missions = missions();
        let w = teammate_work("tm_bramble", &missions, &owners(), |_| "typed words".into());
        assert_eq!(w.recent[0].title, "typed words");
    }
    #[test]
    fn recent_work_groups_follow_ups_and_never_spins_on_a_cycle() {
        let mut missions = missions();
        missions[1].parent = Some("m1".into());
        let w = work("tm_atlas", &missions);
        assert_eq!(w.recent.len(), 2);
        assert_eq!(w.recent[0].title, "do m1");
        missions[0].parent = Some("m2".into());
        assert!(work("tm_atlas", &missions).recent.len() <= RECENT_MISSION_LIMIT);
    }
}
