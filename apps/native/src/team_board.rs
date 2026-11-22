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
    #[test]
    fn home_and_team_keep_pending_agents_quiet_and_name_a_checked_missing_agent() {
        use locust_core::agents::{Found, Auth, Probe};
        for home in [true, false] {
            let mut s = State::default();
            crate::team_scene(&mut s);
            for mate in &mut s.teammates { mate.busy = false; mate.runtime = Some("opencode".into()); }
            let pending = Found { id:"opencode".into(), display_name:"opencode".into(), installed:true,
                version:Some("1.0.0".into()), auth:Auth::Unknown, ready:false, status:Probe::ProbeFailed,
                unsupported_note:None, through_cli:false, checking:true };
            s.settings_panel.agents = Some(vec![pending.clone()]);
            s.people_panel.team = serde_json::json!({"teammates": s.teammates.iter().map(|m| serde_json::json!({"teammateId":m.id,"name":m.name,"role":m.role,"route":{"runtime":m.runtime}})).collect::<Vec<_>>()});
            let shown = draw(s, home);
            assert!(!shown.contains("Needs you") && !shown.contains("Sign-in needed"), "home={home}: {shown}");

            let mut s = State::default();
            crate::team_scene(&mut s);
            for mate in &mut s.teammates { mate.busy = false; mate.runtime = Some("claude".into()); }
            s.settings_panel.agents = Some(vec![pending, Found { id:"claude".into(), display_name:"claude".into(),
                installed:false, version:None, auth:Auth::Unknown, ready:false, status:Probe::Missing,
                unsupported_note:None, through_cli:false, checking:false }]);
            s.people_panel.team = serde_json::json!({"teammates": s.teammates.iter().map(|m| serde_json::json!({"teammateId":m.id,"name":m.name,"role":m.role,"route":{"runtime":m.runtime}})).collect::<Vec<_>>()});
            let shown = draw(s, home);
            assert!(shown.contains("Needs you") && shown.contains("AI agent not installed"), "home={home}: {shown}");
        }
    }

}

#[cfg(test)]
mod team_is_a_board_tests {
    use crate::*;
    use rune::{Env, Runtime};
    fn state() -> State {
        let mut s = State::default();
        crate::team_scene(&mut s);
        for m in &mut s.teammates {
            m.busy = false;
        }
        s.people_panel.team = serde_json::json!({"teammates":s.teammates.iter().map(|m|serde_json::json!({"teammateId":m.id,"name":m.name,"role":m.role,"route":{"runtime":m.runtime}})).collect::<Vec<_>>(),"missionOwners":{"done":"tm_done"}});
        s.people_panel.open = true;
        s.people_panel.tab = 0;
        s
    }
    fn runtime(s: State) -> Runtime<State> {
        let mut rt = Runtime::new(
            s,
            |s, _| crate::people_panel::view(s, &Palette::new(true)),
            (1200.0, 1800.0),
            Env::new(true, 1.0),
        );
        settle(&mut rt);
        rt
    }
    fn settle(rt: &mut Runtime<State>) {
        for _ in 0..40 {
            rt.frame(0.05);
        }
    }
    fn card(rt: &Runtime<State>, key: &str) -> rune::ElementInfo {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(key))
            .unwrap()
    }

    // case: has a class of its own: `.lc-board` is the room task board, a flex COLUMN
    #[test]
    fn team_grid_keeps_cards_beside_each_other() {
        let s = state();
        let a = s.teammates[0].id.clone();
        let b = s.teammates[1].id.clone();
        let rt = runtime(s);
        let a = card(&rt, &format!("team-card-{a}"));
        let b = card(&rt, &format!("team-card-{b}"));
        assert!(
            (a.rect[1] - b.rect[1]).abs() < 1.0,
            "team cards share a row: {:?} {:?}",
            a.rect,
            b.rect
        );
        assert!(
            b.rect[0] > a.rect[0],
            "the room's column layout does not stack the team cards"
        );
    }

    // case: tints only where a person should look: amber waiting, green unseen
    #[test]
    fn only_waiting_and_unseen_cards_are_tinted() {
        let mut s = state();
        let id = s.teammates[0].id.clone();
        s.people_panel.team["missionOwners"] = serde_json::json!({"done":id,"wait":id});
        let key = format!("team-card-{id}");
        let mut rt = runtime(s);
        let plain = card(&rt, &key).fill;
        rt.update(|s, _| s.teammates[0].busy = true);
        settle(&mut rt);
        assert_eq!(card(&rt, &key).fill, plain, "working has no tint");
        rt.update(|s, _| {
            s.teammates[0].busy = false;
            s.home.teammates_unseen.insert(s.teammates[0].id.clone());
        });
        settle(&mut rt);
        assert_eq!(
            card(&rt, &key).fill,
            crate::look::controls::Look::new(true).c("green-tint")
        );
        rt.update(|s, _| {
            s.home.waiting_for.insert("wait".into(), "Approve".into());
        });
        settle(&mut rt);
        assert_eq!(
            card(&rt, &key).fill,
            crate::look::controls::Look::new(true).c("amber-tint")
        );
    }

    // case: a finish is unseen only when nobody was looking, and looking clears it
    #[test]
    fn looking_clears_an_unseen_finish() {
        let mut s = state();
        let id = s.teammates[0].id.clone();
        s.people_panel.team["missionOwners"] = serde_json::json!({"done":id});
        s.recent = vec![locust_core::engine::ConversationRow {
            id: "done".into(),
            teammate: id.clone(),
            title: "Done".into(),
            started_by: "the person".into(),
            state: "running".into(),
            updated: String::new(),
        }];
        let mut next = s.recent.clone();
        next[0].state = "completed".into();
        crate::home_panel::record_finishes(&mut s, &next);
        s.recent = next;
        let mut rt = runtime(s);
        assert!(rt.state.home.teammates_unseen.contains(&id));
        let (x, y) = rt.find(&format!("team-message-{id}")).unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        settle(&mut rt);
        assert!(rt.state.home.teammates_unseen.is_empty());
        rt.update(|s, _| {
            s.home.home_open = false;
            s.home.board_open = false;
            s.home.boot_open = false;
            s.home.to_look_at.clear();
            s.conversations
                .entry(id.clone())
                .or_default()
                .conversation_id = Some("another".into());
            s.recent[0].state = "running".into();
            let mut next = s.recent.clone();
            next[0].state = "completed".into();
            crate::home_panel::record_finishes(s, &next);
        });
        settle(&mut rt);
        assert!(
            rt.state.home.teammates_unseen.is_empty(),
            "a finish belonging to the teammate being read is already seen"
        );
        assert!(
            rt.state.home.to_look_at.contains("done"),
            "another conversation keeps its own unseen mark"
        );
        rt.update(|s, _| { s.home.teammates_unseen.insert(id.clone()); });
        rt.update(|s, tx| { crate::poll(s, 0.0, tx); });
        settle(&mut rt);
        assert!(rt.state.home.teammates_unseen.is_empty(), "returning to the workroom clears its roster mark too");
    }

    // case: a live mission is running on its card, never the record's "interrupted"
    #[test]
    fn live_mission_card_overrides_interrupted_record() {
        let mut s = state();
        let id = s.teammates[0].id.clone();
        s.people_panel.team["missionOwners"] = serde_json::json!({"live":id});
        s.ledger_summaries.insert(
            "live".into(),
            locust_core::ledger::Summary {
                mission_id: "live".into(),
                title: "Still working".into(),
                cancelled: true,
                ..Default::default()
            },
        );
        s.live_turns.insert("live".into());
        let mut rt = runtime(s);
        let dot = card(&rt, "team-recent-live");
        assert_eq!(dot.label.as_deref(), Some("Running"));
        assert_eq!(dot.fill, Palette::new(true).green);
        rt.update(|s, _| {
            s.live_turns.clear();
        });
        settle(&mut rt);
        assert_eq!(
            card(&rt, "team-recent-live").label.as_deref(),
            Some("Interrupted")
        );
    }
}
