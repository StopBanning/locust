//! Sidebar list helpers — port of `renderer/src/conversationList.ts`.
//! Keeping name and group across follow-up turn ids.

use std::collections::{HashMap, HashSet};

/// One sidebar conversation row (Electron `SidebarMission` fields this piece needs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarMission {
    pub mission_id: String,
    pub title: String,
    pub phase: String,
    pub last_at: Option<String>,
    pub root_id: Option<String>,
    pub member_ids: Vec<String>,
    pub owner_id: Option<String>,
    /// What the person said across turns, for sidebar search (`status.ts` ConversationRowExtras.said).
    pub said: String,
    /// Conversation the sidebar nests this under (`nestedUnder`), when any.
    pub nested_under: Option<String>,
    /// Integrity issues on the row; board running mark uses phase, not this count.
    pub integrity_issue_count: u32,
}

/// Every id this row has ever been keyed by, newest identity first (root preferred).
pub fn conversation_keys(mission: &SidebarMission) -> Vec<String> {
    let mut keys = Vec::new();
    let mut seen = HashSet::new();
    for key in std::iter::once(mission.root_id.as_deref())
        .chain(std::iter::once(Some(mission.mission_id.as_str())))
        .chain(mission.member_ids.iter().map(|s| Some(s.as_str())))
        .flatten()
    {
        if key.is_empty() {
            continue;
        }
        if seen.insert(key.to_owned()) {
            keys.push(key.to_owned());
        }
    }
    keys
}

/// The first value stored against any id this conversation has worn.
pub fn held_for<'a, T>(mission: &SidebarMission, by_key: &'a HashMap<String, T>) -> Option<&'a T> {
    for key in conversation_keys(mission) {
        if let Some(held) = by_key.get(&key) {
            return Some(held);
        }
    }
    None
}

/// Live first, then newest first. Unknown age sorts last.
pub fn by_live_then_recent(a: &SidebarMission, b: &SidebarMission) -> std::cmp::Ordering {
    let live_a = if a.phase == "running" { 1 } else { 0 };
    let live_b = if b.phase == "running" { 1 } else { 0 };
    live_b.cmp(&live_a).then_with(|| {
        let at_a = parse_ms(a.last_at.as_deref()).unwrap_or(0);
        let at_b = parse_ms(b.last_at.as_deref()).unwrap_or(0);
        at_b.cmp(&at_a)
    })
}

pub fn conversation_rows(missions: &[SidebarMission]) -> Vec<SidebarMission> {
    let mut rows = missions.to_vec();
    rows.sort_by(by_live_then_recent);
    rows
}

/// `Atlas · "invoice" — 1 of 5`
pub fn narrowing_line(
    face_name: Option<&str>,
    query: &str,
    shown: usize,
    in_face: usize,
    all: usize,
) -> Option<String> {
    let mut narrowed_by = Vec::new();
    if let Some(name) = face_name {
        narrowed_by.push(name.to_owned());
    }
    let query = query.trim();
    if !query.is_empty() {
        narrowed_by.push(format!("\"{query}\""));
    }
    if narrowed_by.is_empty() {
        return None;
    }
    let of = if face_name.is_none() { all } else { in_face };
    Some(format!("{} — {shown} of {of}", narrowed_by.join(" · ")))
}

pub fn conversation_title_for<'a>(
    rows: &'a [SidebarMission],
    mission_id: Option<&str>,
) -> Option<&'a str> {
    let mission_id = mission_id?;
    rows.iter()
        .find(|row| conversation_keys(row).iter().any(|k| k == mission_id))
        .map(|row| row.title.as_str())
}

/// One sentence for every local file that exists and would not read.
pub fn unreadable_sentence(stores: &[&str]) -> Option<(String, String)> {
    const STORE_ORDER: [&str; 4] = ["teammates", "groups", "rooms", "routines"];
    let named: Vec<&str> = STORE_ORDER
        .into_iter()
        .filter(|store| stores.contains(store))
        .collect();
    if named.is_empty() {
        return None;
    }
    let list = if named.len() == 1 {
        named[0].to_owned()
    } else {
        format!(
            "{} and {}",
            named[..named.len() - 1].join(", "),
            named[named.len() - 1]
        )
    };
    let subject = {
        let mut chars = list.chars();
        match chars.next() {
            Some(c) => format!("{}{}", c.to_ascii_uppercase(), chars.as_str()),
            None => list,
        }
    };
    if named.len() == 1 {
        let happened = if named[0] == "groups" {
            format!("{subject} could not be read — everything is listed ungrouped.")
        } else {
            format!("{subject} could not be read.")
        };
        Some((
            happened,
            "Nothing is saved over the file until it reads again.".into(),
        ))
    } else {
        Some((
            format!("{subject} could not be read."),
            "Nothing is saved over them until they read again.".into(),
        ))
    }
}

fn parse_ms(value: Option<&str>) -> Option<i64> {
    let value = value?;
    // Accept Electron ISO times; Date.parse-equivalent via chrono when available, else 0.
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

/// Split recent conversations into pinned (newest pin first) and the rest (Recents / Ungrouped).
/// Each conversation appears once. Empty pins → no Pinned/Recents section labels needed by the caller.
pub fn pinned_and_recent(
    missions: &[SidebarMission],
    pinned_keys: &[String],
) -> (Vec<SidebarMission>, Vec<SidebarMission>) {
    let rows = conversation_rows(missions);
    let mut pinned = Vec::new();
    let mut used = HashSet::new();
    for key in pinned_keys {
        if let Some(row) = rows.iter().find(|m| {
            conversation_keys(m).iter().any(|k| k == key) && !used.contains(&m.mission_id)
        }) {
            used.insert(row.mission_id.clone());
            pinned.push(row.clone());
        }
    }
    let recent = rows
        .into_iter()
        .filter(|m| !used.contains(&m.mission_id))
        .collect();
    (pinned, recent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(mission_id: &str, title: &str) -> SidebarMission {
        SidebarMission {
            mission_id: mission_id.into(),
            title: title.into(),
            phase: "completed".into(),
            last_at: Some("2026-10-10T05:00:00.000Z".into()),
            root_id: None,
            member_ids: Vec::new(),
            owner_id: None,
            said: String::new(),
            nested_under: None,
            integrity_issue_count: 0,
        }
    }

    // a-conversation-keeps-its-name-and-group.test.ts
    #[test]
    fn a_conversation_keeps_its_name_and_group() {
        let keys = conversation_keys(&SidebarMission {
            root_id: Some("root".into()),
            member_ids: vec!["root".into(), "mid".into(), "leaf".into()],
            ..row("leaf", "t")
        });
        assert!(keys.contains(&"root".into()));
        assert!(keys.contains(&"mid".into()));
        assert!(keys.contains(&"leaf".into()));
        assert_eq!(
            conversation_keys(&SidebarMission {
                root_id: Some("a".into()),
                member_ids: vec!["a".into()],
                ..row("a", "t")
            }),
            vec!["a".to_string()]
        );
        assert_eq!(
            conversation_keys(&SidebarMission {
                root_id: Some("root".into()),
                ..row("leaf", "t")
            })[0],
            "root"
        );

        let mission = SidebarMission {
            root_id: Some("root".into()),
            member_ids: vec!["root".into(), "leaf".into()],
            ..row("leaf", "t")
        };
        let mut titles = HashMap::new();
        titles.insert("leaf".into(), "code".to_string());
        assert_eq!(
            held_for(&mission, &titles).map(String::as_str),
            Some("code")
        );

        let mut groups = HashMap::new();
        groups.insert("root".into(), "grp_locust".to_string());
        assert_eq!(
            held_for(&mission, &groups).map(String::as_str),
            Some("grp_locust")
        );

        titles.insert("root".into(), "from the root".into());
        titles.insert("leaf".into(), "from the leaf".into());
        assert_eq!(
            held_for(&mission, &titles).map(String::as_str),
            Some("from the root")
        );
        assert!(
            held_for(
                &mission,
                &HashMap::<String, String>::from([("other".into(), "x".into())])
            )
            .is_none()
        );
        assert_eq!(
            held_for(
                &row("only", "t"),
                &HashMap::from([("only".into(), "named".into())])
            )
            .map(String::as_str),
            Some("named")
        );

        let rows = [
            SidebarMission {
                root_id: Some("root".into()),
                member_ids: vec!["root".into(), "mid".into(), "leaf".into()],
                ..row("leaf", "handoff-export-polish.md")
            },
            row("other", "Something else"),
        ];
        assert_eq!(
            conversation_title_for(&rows, Some("leaf")),
            Some("handoff-export-polish.md")
        );
        assert_eq!(
            conversation_title_for(&rows, Some("mid")),
            Some("handoff-export-polish.md")
        );
        assert_eq!(
            conversation_title_for(&rows, Some("root")),
            Some("handoff-export-polish.md")
        );
        assert!(conversation_title_for(&rows, Some("pending:3")).is_none());
        assert!(conversation_title_for(&rows, None).is_none());
    }

    // a-conversation-can-be-pinned.test.tsx (list order)
    #[test]
    fn a_conversation_can_be_pinned_order() {
        let missions = [
            row("m_a", "Alpha"),
            row("m_b", "Bravo"),
            SidebarMission {
                root_id: Some("m_root_c".into()),
                ..row("m_c", "Charlie")
            },
        ];
        let (pinned, recent) = pinned_and_recent(&missions, &["m_root_c".into(), "m_b".into()]);
        assert_eq!(
            pinned.iter().map(|m| m.title.as_str()).collect::<Vec<_>>(),
            vec!["Charlie", "Bravo"]
        );
        assert_eq!(
            recent.iter().map(|m| m.title.as_str()).collect::<Vec<_>>(),
            vec!["Alpha"]
        );
        let (none_p, all_r) = pinned_and_recent(&missions, &[]);
        assert!(none_p.is_empty());
        assert_eq!(all_r.len(), 3);
    }

    #[test]
    fn narrowing_and_unreadable_words() {
        assert_eq!(
            narrowing_line(Some("Atlas"), "invoice", 1, 5, 20).as_deref(),
            Some("Atlas · \"invoice\" — 1 of 5")
        );
        assert_eq!(
            narrowing_line(None, "invoice", 1, 5, 20).as_deref(),
            Some("\"invoice\" — 1 of 20")
        );
        let (happened, safe) = unreadable_sentence(&["groups"]).unwrap();
        assert_eq!(
            happened,
            "Groups could not be read — everything is listed ungrouped."
        );
        assert_eq!(safe, "Nothing is saved over the file until it reads again.");
    }
}
