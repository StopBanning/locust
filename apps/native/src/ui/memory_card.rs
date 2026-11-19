//! What a conversation taught the team and what the Memory screen's notice says
//! (`conversationMemories.ts`, `MemoryCard.tsx`, `MemoryScreen.tsx::keepEvery`).

use serde_json::Value;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MemoryChange {
    Rewrite,
    Forget,
    Merge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MemoryCardLine {
    pub by: String,
    pub text: String,
    pub status: String,
    pub updated: bool,
    pub mission_id: String,
    pub change: Option<MemoryChange>,
}

pub(crate) fn memories_of_conversation(memories: &[Value], turns: &HashSet<String>) -> Vec<MemoryCardLine> {
    if turns.is_empty() {
        return Vec::new();
    }
    memories
        .iter()
        .filter_map(|m| {
            let mission_id = m["missionId"].as_str()?;
            if !turns.contains(mission_id) {
                return None;
            }
            let by = m["updatedBy"]["name"]
                .as_str()
                .or_else(|| m["by"]["name"].as_str())
                .unwrap_or("A teammate")
                .to_owned();
            let change = if m.get("merges").is_some_and(|v| !v.is_null()) {
                Some(MemoryChange::Merge)
            } else if m.get("forgets").is_some_and(|v| !v.is_null()) {
                Some(MemoryChange::Forget)
            } else if m.get("replaces").is_some_and(|v| !v.is_null()) {
                Some(MemoryChange::Rewrite)
            } else {
                None
            };
            Some(MemoryCardLine {
                by,
                text: m["text"].as_str().unwrap_or("").to_owned(),
                status: m["status"].as_str().unwrap_or("kept").to_owned(),
                updated: m.get("previousText").is_some_and(|v| v.is_string()),
                mission_id: mission_id.to_owned(),
                change,
            })
        })
        .collect()
}

pub(crate) fn memory_card_summary(lines: &[MemoryCardLine]) -> String {
    let kept = lines.iter().filter(|l| l.status == "kept" && !l.updated).count();
    let proposed: Vec<&MemoryCardLine> = lines.iter().filter(|l| l.status == "proposed").collect();
    let proposed_new = proposed.iter().filter(|l| l.change.is_none()).count();
    let proposed_changes = proposed.iter().filter(|l| l.change == Some(MemoryChange::Rewrite)).count();
    let proposed_forgets = proposed.iter().filter(|l| l.change == Some(MemoryChange::Forget)).count();
    let proposed_merges = proposed.iter().filter(|l| l.change == Some(MemoryChange::Merge)).count();
    let rewritten = lines.iter().filter(|l| l.updated && l.status == "kept").count();

    let mut names: Vec<&str> = Vec::new();
    for line in lines {
        if !names.contains(&line.by.as_str()) {
            names.push(&line.by);
        }
    }
    let by = match names.len() {
        0 => "A teammate".to_owned(),
        1 => names[0].to_owned(),
        _ => format!("{} and {}", names[..names.len() - 1].join(", "), names[names.len() - 1]),
    };
    let things = |n: usize| format!("{n} thing{}", if n == 1 { "" } else { "s" });
    let mut parts = Vec::new();
    if kept > 0 {
        parts.push(format!("remembered {}", things(kept)));
    }
    if rewritten > 0 {
        parts.push(format!("updated {} it already knew", things(rewritten)));
    }
    if proposed_new > 0 {
        parts.push(format!("wants to remember {}", things(proposed_new)));
    }
    if proposed_changes > 0 {
        parts.push(format!("wants to change {} it already knew", things(proposed_changes)));
    }
    if proposed_forgets > 0 {
        parts.push(format!("wants to forget {}", things(proposed_forgets)));
    }
    if proposed_merges > 0 {
        parts.push(format!(
            "wants to merge {proposed_merges} set{} of memories",
            if proposed_merges == 1 { "" } else { "s" }
        ));
    }
    let summary = format!("{by} {}", parts.join(" and "));
    if proposed.is_empty() {
        summary
    } else if proposed_new == proposed.len() {
        format!("{summary} — keep or forget it on the Memory screen")
    } else {
        format!("{summary} — answer it on the Memory screen")
    }
}

#[derive(Default)]
pub(crate) struct MemoryChangedUpdate<'a> {
    pub by: &'a str,
    pub kept: &'a [&'a str],
    pub proposed: &'a [&'a str],
    pub forgotten: &'a [&'a str],
    pub rewritten: &'a [&'a str],
    pub proposed_changes: &'a [&'a str],
    pub proposed_forgets: &'a [&'a str],
    pub proposed_tidy: usize,
    pub tidy_refused: &'a [&'a str],
    pub about_you_suggested: &'a [&'a str],
}

/// Whether the notice points at something waiting for the person (Electron `noticeWaits`): it goes once nothing does.
pub(crate) fn notice_waits(update: &MemoryChangedUpdate<'_>) -> bool {
    update.proposed.len() + update.proposed_changes.len() + update.proposed_forgets.len() + update.proposed_tidy + update.about_you_suggested.len() > 0
}

pub(crate) fn memory_changed_notice(update: &MemoryChangedUpdate<'_>) -> Option<String> {
    let quoted = |texts: &[&str]| texts.iter().map(|t| format!("\"{t}\"")).collect::<Vec<_>>().join("; ");
    let mut said = Vec::new();
    if !update.kept.is_empty() {
        said.push(format!("{} remembered {}", update.by, quoted(update.kept)));
    }
    if !update.proposed.is_empty() {
        said.push(format!("{} wants to remember {}", update.by, quoted(update.proposed)));
    }
    if !update.forgotten.is_empty() {
        said.push(format!("{} forgot {}", update.by, quoted(update.forgotten)));
    }
    if !update.rewritten.is_empty() {
        said.push(format!("{} updated {}", update.by, quoted(update.rewritten)));
    }
    if !update.proposed_changes.is_empty() {
        said.push(format!("{} wants to change a memory to {}", update.by, quoted(update.proposed_changes)));
    }
    if !update.proposed_forgets.is_empty() {
        said.push(format!("{} wants to forget {}", update.by, quoted(update.proposed_forgets)));
    }
    if update.proposed_tidy > 0 {
        let n = update.proposed_tidy;
        said.push(format!(
            "{} suggested {n} change{} to memory, waiting below",
            update.by,
            if n == 1 { "" } else { "s" }
        ));
    }
    if !update.tidy_refused.is_empty() {
        said.push(format!(
            "{} could not be made: {}",
            update.tidy_refused.len(),
            update.tidy_refused.join(" ")
        ));
    }
    if !update.about_you_suggested.is_empty() {
        said.push(format!(
            "{} suggests adding to About you: {}",
            update.by,
            quoted(update.about_you_suggested)
        ));
    }
    if said.is_empty() {
        return None;
    }
    Some(
        said.into_iter()
            .map(|clause| {
                let bytes = clause.as_bytes();
                let ends_sentence = match bytes.last() {
                    Some(b'.' | b'!' | b'?') => true,
                    Some(b'"') => bytes.len() >= 2 && matches!(bytes[bytes.len() - 2], b'.' | b'!' | b'?'),
                    _ => false,
                };
                if ends_sentence { clause } else { format!("{clause}.") }
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Keep every suggestion waiting, one after another, collecting each distinct refusal once (`keepEvery`).
pub(crate) fn keep_every<F>(memory_ids: &[String], mut keep: F) -> Option<String>
where
    F: FnMut(&str) -> Option<String>,
{
    let mut reasons: Vec<String> = Vec::new();
    for memory_id in memory_ids {
        if let Some(refused) = keep(memory_id)
            && !reasons.contains(&refused)
        {
            reasons.push(refused);
        }
    }
    if reasons.is_empty() { None } else { Some(reasons.join(" ")) }
}

/// Short local date (`Sep 24`), matching `toLocaleDateString(undefined, { month: 'short', day: 'numeric' })`.
pub(crate) fn when_short(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|at| at.with_timezone(&chrono::Local).format("%b %-d").to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn kept(id: &str, text: &str) -> Value {
        json!({
            "memoryId": id,
            "text": text,
            "scope": "workspace",
            "workspaceId": "ws_shop",
            "workspaceName": "shop",
            "by": { "name": "Yurt" },
            "createdAt": "2026-09-01T10:00:00.000Z",
            "status": "kept",
            "enabled": true,
            "name": "deploy-day"
        })
    }

    #[test]
    fn proposed_change_is_named_for_what_it_asks_not_as_something_to_remember() {
        let k = kept("mem_kept", "Deploys go out on Fridays.");
        let mut rewrite = k.clone();
        rewrite["memoryId"] = json!("mem_change");
        rewrite["text"] = json!("Deploys go out on Thursdays.");
        rewrite["status"] = json!("proposed");
        rewrite["replaces"] = json!("mem_kept");
        rewrite["missionId"] = json!("mission_2");

        let mut forget = k.clone();
        forget["memoryId"] = json!("mem_forget");
        forget["status"] = json!("proposed");
        forget["forgets"] = json!("mem_kept");
        forget["missionId"] = json!("mission_2");

        let mut fresh = k.clone();
        fresh["memoryId"] = json!("mem_new");
        fresh["text"] = json!("Tests run with pnpm test.");
        fresh["status"] = json!("proposed");
        fresh["missionId"] = json!("mission_2");

        let turns: HashSet<String> = ["mission_2".to_owned()].into_iter().collect();
        let lines = memories_of_conversation(&[k, rewrite, forget, fresh.clone()], &turns);
        let changes: Vec<Option<MemoryChange>> = lines.iter().map(|l| l.change).collect();
        assert_eq!(changes, vec![Some(MemoryChange::Rewrite), Some(MemoryChange::Forget), None]);
        assert_eq!(
            memory_card_summary(&lines),
            "Yurt wants to remember 1 thing and wants to change 1 thing it already knew and wants to forget 1 thing — answer it on the Memory screen"
        );
        let fresh_lines = memories_of_conversation(&[fresh], &turns);
        assert_eq!(
            memory_card_summary(&fresh_lines),
            "Yurt wants to remember 1 thing — keep or forget it on the Memory screen"
        );
    }

    #[test]
    fn notice_names_each_change_for_what_it_is_without_double_period() {
        assert_eq!(
            memory_changed_notice(&MemoryChangedUpdate {
                by: "Booty",
                proposed_changes: &["Deploys go out on Thursdays."],
                proposed_forgets: &["The API is on port 3000."],
                ..Default::default()
            })
            .as_deref(),
            Some("Booty wants to change a memory to \"Deploys go out on Thursdays.\" Booty wants to forget \"The API is on port 3000.\"")
        );
        assert_eq!(
            memory_changed_notice(&MemoryChangedUpdate {
                by: "Wren",
                kept: &["Build with pnpm build"],
                ..Default::default()
            })
            .as_deref(),
            Some("Wren remembered \"Build with pnpm build\".")
        );
        assert_eq!(memory_changed_notice(&MemoryChangedUpdate { by: "Wren", ..Default::default() }), None);
    }

    #[test]
    fn tidy_pass_in_conversation_and_on_screen_notice() {
        let mut a = kept("mem_a", "Deploys go out on Thursdays.");
        a["by"] = json!({ "name": "you" });
        let mut b = kept("mem_b", "We deploy every Thursday.");
        b["by"] = json!({ "name": "you" });
        let mut c = kept("mem_c", "The API is on port 3000.");
        c["by"] = json!({ "name": "you" });

        let merge = json!({
            "memoryId": "mem_m",
            "text": "Deploys go out every Thursday.",
            "status": "proposed",
            "merges": ["mem_a", "mem_b"],
            "by": { "name": "Wren" },
            "missionId": "mission_tidy"
        });
        let retire = json!({
            "memoryId": "mem_r",
            "text": "The API is on port 3000.",
            "status": "proposed",
            "forgets": "mem_c",
            "reason": "the API moved to 3001 (mem_a)",
            "by": { "name": "Wren" },
            "missionId": "mission_tidy"
        });
        let turns: HashSet<String> = ["mission_tidy".to_owned()].into_iter().collect();
        let lines = memories_of_conversation(&[a, b, c, merge, retire], &turns);
        let changes: Vec<Option<MemoryChange>> = lines.iter().map(|l| l.change).collect();
        assert_eq!(changes, vec![Some(MemoryChange::Merge), Some(MemoryChange::Forget)]);
        assert_eq!(
            memory_card_summary(&lines),
            "Wren wants to forget 1 thing and wants to merge 1 set of memories — answer it on the Memory screen"
        );
        assert_eq!(
            memory_changed_notice(&MemoryChangedUpdate {
                by: "Wren",
                proposed_tidy: 2,
                tidy_refused: &["A suggestion to forget a memory named one this folder does not keep."],
                ..Default::default()
            })
            .as_deref(),
            Some("Wren suggested 2 changes to memory, waiting below. 1 could not be made: A suggestion to forget a memory named one this folder does not keep.")
        );
    }

    #[test]
    fn keep_every_keeps_each_in_order_and_says_every_refusal_once() {
        let mut asked = Vec::new();
        let stale = "What this suggestion would change has changed since it was made, so it was dropped. Everything is as it was.";
        let ids: Vec<String> = ["mem_m", "mem_r", "mem_x", "mem_y"].into_iter().map(str::to_owned).collect();
        let said = keep_every(&ids, |id| {
            asked.push(id.to_owned());
            if id == "mem_x" || id == "mem_y" { Some(stale.to_owned()) } else { None }
        });
        assert_eq!(asked, vec!["mem_m", "mem_r", "mem_x", "mem_y"]);
        assert_eq!(said.as_deref(), Some(stale));
        assert_eq!(keep_every(&["mem_m".to_owned()], |_| None), None);
    }
}
