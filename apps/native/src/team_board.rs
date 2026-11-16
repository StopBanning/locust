//! Urgency groups shared by Home and the Team roster (Electron teamBoard.ts).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    Working,
    ApprovalNeeded,
    Blocked,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    NeedsYou,
    Working,
    Finished,
    Ready,
}
impl Section {
    #[cfg(test)]
    pub fn key(self) -> &'static str {
        match self {
            Self::NeedsYou => "needs-you",
            Self::Working => "working",
            Self::Finished => "finished",
            Self::Ready => "ready",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::NeedsYou => "Needs you",
            Self::Working => "Working",
            Self::Finished => "Just finished",
            Self::Ready => "Ready",
        }
    }
}
pub fn board_section_of(status: Option<Status>, unseen: bool) -> Section {
    match status {
        Some(Status::ApprovalNeeded | Status::Blocked) => Section::NeedsYou,
        Some(Status::Working) => Section::Working,
        _ if unseen => Section::Finished,
        _ => Section::Ready,
    }
}
pub fn team_board<'a, T>(
    team: &'a [T],
    section: impl Fn(&T) -> Section,
) -> Option<Vec<(Section, Vec<&'a T>)>> {
    let groups: Vec<_> = [
        Section::NeedsYou,
        Section::Working,
        Section::Finished,
        Section::Ready,
    ]
    .into_iter()
    .map(|key| {
        (
            key,
            team.iter()
                .filter(|t| section(t) == key)
                .collect::<Vec<_>>(),
        )
    })
    .filter(|(_, members)| !members.is_empty())
    .collect();
    groups
        .iter()
        .any(|(key, _)| *key != Section::Ready)
        .then_some(groups)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn team() -> [&'static str; 5] {
        ["wren", "juno", "sable", "quill", "pip"]
    }
    #[test]
    fn puts_each_teammate_where_what_they_need_from_you_says_most_urgent_first() {
        let team = team();
        let board = team_board(&team, |id| {
            board_section_of(
                Some(match *id {
                    "wren" => Status::Working,
                    "juno" => Status::ApprovalNeeded,
                    "quill" => Status::Blocked,
                    _ => Status::Idle,
                }),
                *id == "sable",
            )
        })
        .unwrap();
        assert_eq!(
            board
                .iter()
                .map(|(s, m)| (s.title(), m.iter().map(|s| **s).collect::<Vec<_>>()))
                .collect::<Vec<_>>(),
            vec![
                ("Needs you", vec!["juno", "quill"]),
                ("Working", vec!["wren"]),
                ("Just finished", vec!["sable"]),
                ("Ready", vec!["pip"])
            ]
        );
    }
    #[test]
    fn draws_a_quiet_team_as_the_plain_grid_no_board_of_empty_columns() {
        assert!(
            team_board(&team(), |id| board_section_of(
                (*id == "wren").then_some(Status::Idle),
                false
            ))
            .is_none()
        );
    }
    #[test]
    fn leaves_out_a_section_with_nobody_in_it() {
        let team = team();
        let board = team_board(&team, |id| {
            board_section_of((*id == "wren").then_some(Status::Working), false)
        })
        .unwrap();
        assert_eq!(
            board.iter().map(|(s, _)| s.key()).collect::<Vec<_>>(),
            vec!["working", "ready"]
        );
    }
    #[test]
    fn an_unseen_finish_outranks_idle_never_a_live_run_or_a_wait() {
        assert_eq!(
            board_section_of(Some(Status::Idle), true),
            Section::Finished
        );
        assert_eq!(
            board_section_of(Some(Status::Working), true),
            Section::Working
        );
        assert_eq!(
            board_section_of(Some(Status::ApprovalNeeded), true),
            Section::NeedsYou
        );
        assert_eq!(board_section_of(None, false), Section::Ready);
    }
}
#[cfg(test)]
mod surface_tests {
    use crate::*;

    fn draw(s: State, home: bool) -> String {
        let mut rt = rune::Runtime::new(
            (s, home),
            |(s, home), _| {
                if *home {
                    crate::home_panel::home_view(s, &Palette::new(true))
                } else {
                    crate::people_panel::view(s, &Palette::new(true))
                }
            },
            (1200.0, 1800.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.elements()
            .into_iter()
            .filter_map(|e| e.text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn home_and_team_draw_urgency_sections_and_keep_a_quiet_roster_plain() {
        for home in [true, false] {
            let mut s = State::default();
            crate::team_scene(&mut s);
            for mate in &mut s.teammates {
                mate.busy = false;
            }
            s.people_panel.team = serde_json::json!({"teammates": s.teammates.iter().map(|m| serde_json::json!({"teammateId":m.id,"name":m.name,"role":m.role,"route":{"runtime":m.runtime}})).collect::<Vec<_>>()});
            s.people_panel.open = true;
            s.people_panel.tab = 0;
            // A live teammate belongs under Working on both real surfaces.
            s.teammates[0].busy = true;
            let shown = draw(s, home);
            assert!(shown.contains("Working"), "home={home}: {shown}");
            let mut s = State::default();
            crate::team_scene(&mut s);
            for mate in &mut s.teammates {
                mate.busy = false;
            }
            s.people_panel.team = serde_json::json!({"teammates": s.teammates.iter().map(|m| serde_json::json!({"teammateId":m.id,"name":m.name,"role":m.role,"route":{"runtime":m.runtime}})).collect::<Vec<_>>()});
            s.people_panel.open = true;
            s.people_panel.tab = 0;
            let shown = draw(s, home);
            assert!(
                !shown.lines().any(|line| line == "Ready"),
                "home={home}: {shown}"
            );
            assert!(!shown.contains("Just finished"), "home={home}: {shown}");
            assert!(!shown.contains("Needs you"), "home={home}: {shown}");
        }
    }
}
