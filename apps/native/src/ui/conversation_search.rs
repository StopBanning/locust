//! Cross-conversation sidebar search — port of `status.ts` `missionsMatching`.

use crate::conversation_list::SidebarMission;

pub fn missions_matching<'a, F>(
    rows: &'a [SidebarMission],
    query: &str,
    also_called: Option<F>,
) -> Vec<&'a SidebarMission>
where
    F: Fn(&SidebarMission) -> Option<String>,
{
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return rows.iter().collect();
    }
    rows.iter()
        .filter(|row| {
            row.title.to_lowercase().contains(&needle)
                || row.mission_id.to_lowercase().contains(&needle)
                || row.said.to_lowercase().contains(&needle)
                || also_called
                    .as_ref()
                    .and_then(|f| f(row))
                    .is_some_and(|name| name.to_lowercase().contains(&needle))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation_list::SidebarMission;

    fn row(id: &str, title: &str, said: &str) -> SidebarMission {
        SidebarMission {
            mission_id: id.into(),
            title: title.into(),
            phase: "completed".into(),
            last_at: Some("2026-10-10T05:00:00.000Z".into()),
            root_id: None,
            member_ids: Vec::new(),
            owner_id: None,
            said: said.into(),
            nested_under: None,
            integrity_issue_count: 0,
        }
    }

    // search-finds-every-turn.test.tsx / missionsMatching
    #[test]
    fn a_sidebar_search_finds_title_id_said_and_teammate() {
        let rows = [
            row("m_1", "Deploy plan", "also check the signup form"),
            row("m_2", "Other", "nothing here"),
        ];
        let by_said = missions_matching(
            &rows,
            "signup",
            None::<fn(&SidebarMission) -> Option<String>>,
        );
        assert_eq!(by_said.len(), 1);
        assert_eq!(by_said[0].mission_id, "m_1");
        let by_title = missions_matching(
            &rows,
            "deploy",
            None::<fn(&SidebarMission) -> Option<String>>,
        );
        assert_eq!(by_title.len(), 1);
        let by_id = missions_matching(&rows, "m_2", None::<fn(&SidebarMission) -> Option<String>>);
        assert_eq!(by_id.len(), 1);
        let by_mate = missions_matching(
            &rows,
            "atlas",
            Some(|r: &SidebarMission| {
                if r.mission_id == "m_1" {
                    Some("Atlas".into())
                } else {
                    None
                }
            }),
        );
        assert_eq!(by_mate.len(), 1);
        assert_eq!(
            missions_matching(&rows, "   ", None::<fn(&SidebarMission) -> Option<String>>).len(),
            2
        );
    }
}
