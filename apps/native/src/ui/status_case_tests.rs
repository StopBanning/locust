//! status.test.ts (apps/desktop/src/renderer/src), the cases whose code is in the window: the picker's cap on a long
//! group (`cap_rows`, as `route_menu` calls it), route search's folding (`route_search_text`), the receipt's phase
//! (`missions_screen::mission_phase_view`), the sidebar's search (`missions_matching`) and the sidebar's folding of
//! turns into conversations (`sidebar::recent_as_missions`). Expected values are Electron's, from status.test.ts.
use super::*;
use crate::conversation_list::SidebarMission;
use crate::conversation_search::missions_matching;
use crate::missions_screen::mission_phase_view;

// ---- a picker group that would not fit (capRouteRows) ----

/// What `route_menu` passes: twice the cap while searching (ROUTE_SEARCH_GROUP_LIMIT).
fn cap(len: usize, active: Option<usize>, searching: bool) -> (Vec<usize>, usize) {
    cap_rows(len, active, if searching { ROUTE_GROUP_LIMIT * 2 } else { ROUTE_GROUP_LIMIT })
}

#[test]
fn leaves_a_group_that_fits_exactly_as_it_is_and_says_nothing_about_it() {
    let (shown, hidden) = cap(ROUTE_GROUP_LIMIT, None, false);
    assert_eq!(shown, (0..ROUTE_GROUP_LIMIT).collect::<Vec<_>>());
    assert_eq!(hidden, 0);
}

#[test]
fn caps_a_long_group_and_counts_every_row_it_is_not_showing() {
    let (shown, hidden) = cap(217, None, false);
    assert_eq!(shown.len(), ROUTE_GROUP_LIMIT);
    assert_eq!(hidden, 217 - ROUTE_GROUP_LIMIT);
}

#[test]
fn keeps_the_runtimes_below_a_long_group_reachable() {
    // The cap is taken group by group (route_menu calls cap_rows once per group), so Claude Code's three rows below
    // Cursor Agent's 217 are all shown.
    let (_, cursor_hidden) = cap(217, None, false);
    let (claude, claude_hidden) = cap(3, None, false);
    assert_eq!(cursor_hidden, 217 - ROUTE_GROUP_LIMIT);
    assert_eq!((claude.len(), claude_hidden), (3, 0));
}

#[test]
fn never_hides_the_route_you_are_on_and_still_shows_the_cap() {
    let (shown, hidden) = cap(217, Some(180), false);
    assert_eq!(shown.len(), ROUTE_GROUP_LIMIT);
    assert!(shown.contains(&180), "{shown:?}");
    assert_eq!(hidden, 217 - ROUTE_GROUP_LIMIT);
}

#[test]
fn widens_the_cap_while_searching_and_still_counts_what_it_holds_back() {
    let (shown, hidden) = cap(217, None, true);
    assert_eq!(shown.len(), ROUTE_GROUP_LIMIT * 2);
    assert_eq!(hidden, 217 - ROUTE_GROUP_LIMIT * 2);
    let (few, few_hidden) = cap(9, None, true);
    assert_eq!((few.len(), few_hidden), (9, 0));
}

// ---- searching routes the way names are said ----

#[test]
fn folds_the_punctuation_a_cli_spells_a_model_with() {
    assert_eq!(route_search_text("opencode/muse-spark-1.3-contributor-free"), "opencode muse spark 1 3 contributor free");
    assert_eq!(route_search_text("  Muse   Spark "), "muse spark");
    assert!(route_search_text("OpenCode muse-spark-1.3").contains(&route_search_text("muse spark 1.3")));
}

// ---- mission phase and receipt truth ----

#[test]
fn distinguishes_every_terminal_phase() {
    assert_eq!(mission_phase_view("running", false).tag, "RUNNING");
    assert_eq!(mission_phase_view("completed", false).tag, "COMPLETED");
    assert_eq!(mission_phase_view("interrupted", false).tag, "INTERRUPTED");
    assert_eq!(mission_phase_view("failed", false).tag, "FAILED");
    assert_eq!(mission_phase_view("cancelled", false).tag, "CANCELLED");
}

#[test]
fn will_not_present_a_completed_mission_as_clean_when_its_ledger_is_not() {
    let view = mission_phase_view("completed", true);
    assert_eq!(view.tone, "amber");
    assert!(view.label.contains("incomplete"), "{}", view.label);
    assert_eq!(locust_core::inspector::ledger_verification_label(1), "record incomplete");
    assert_eq!(locust_core::inspector::ledger_verification_label(0), "record readable");
    assert_ne!(locust_core::inspector::ledger_verification_label(0), "verified");
}

// ---- searching missions ----

fn searched() -> Vec<SidebarMission> {
    [("Audit the config", "mission_abc12345"), ("Rewrite the README", "mission_def67890")]
        .into_iter()
        .map(|(title, id)| SidebarMission {
            mission_id: id.into(),
            title: title.into(),
            phase: "completed".into(),
            last_at: None,
            root_id: None,
            member_ids: vec![id.into()],
            owner_id: None,
            said: String::new(),
            nested_under: None,
            integrity_issue_count: 0,
            needs_you: false,
            app_started: false,
        })
        .collect()
}
fn matching(query: &str) -> Vec<String> {
    let rows = searched();
    missions_matching(&rows, query, None::<fn(&SidebarMission) -> Option<String>>).into_iter().map(|r| r.mission_id.clone()).collect()
}

#[test]
fn matches_the_words_a_person_can_see_whatever_the_case() {
    assert_eq!(matching("audit"), ["mission_abc12345"]);
    assert_eq!(matching("README"), ["mission_def67890"]);
}

#[test]
fn matches_an_id_so_one_pasted_from_a_receipt_finds_its_mission() {
    assert_eq!(matching("def678"), ["mission_def67890"]);
}

#[test]
fn shows_everything_when_nothing_was_typed() {
    assert_eq!(matching("   ").len(), 2);
}

#[test]
fn shows_nothing_when_nothing_matches_rather_than_everything() {
    assert!(matching("nonsense").is_empty());
}

// ---- the sidebar lists conversations, not turns (collapseConversations, as recent_as_missions folds them) ----

/// Turns as the sidebar reads them: the recent list, and each turn's ledger summary with what it continues.
/// `(id, title, phase, parent)`, each a second apart in the order given.
fn turns(list: &[(&str, &str, &str, Option<&str>)]) -> Vec<SidebarMission> {
    let mut s = State::default();
    s.demo = true;
    for (n, (id, title, phase, parent)) in list.iter().enumerate() {
        let at = format!("2026-11-03T10:00:{n:02}Z");
        s.recent.push(ConversationRow {
            id: (*id).into(),
            title: (*title).into(),
            teammate: String::new(),
            started_by: "native".into(),
            state: (*phase).into(),
            updated: at.clone(),
        });
        s.ledger_summaries.insert(
            (*id).into(),
            ledger::Summary {
                mission_id: (*id).into(),
                title: (*title).into(),
                words: (*title).into(),
                parent: parent.map(Into::into),
                runtime: "claude".into(),
                created_at: at.clone(),
                updated: at,
                ..Default::default()
            },
        );
    }
    crate::sidebar::recent_as_missions(&s)
}

#[test]
fn collapses_a_three_turn_exchange_into_one_row() {
    let rows = turns(&[("m1", "test", "completed", None), ("m2", "test", "completed", Some("m1")), ("m3", "test", "completed", Some("m2"))]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].mission_id, "m3");
    assert_eq!(rows[0].member_ids, ["m1", "m2", "m3"]);
}

#[test]
fn names_the_row_for_what_the_person_typed_first_and_opens_its_newest_turn() {
    let rows = turns(&[("m1", "set up the parser", "completed", None), ("m2", "now add tests", "completed", Some("m1"))]);
    assert_eq!(rows[0].title, "set up the parser");
    assert_eq!(rows[0].mission_id, "m2");
}

#[test]
fn reads_as_running_while_any_turn_of_it_is_running() {
    let rows = turns(&[("m1", "x", "completed", None), ("m2", "x", "running", Some("m1"))]);
    assert_eq!(rows[0].phase, "running");
}

#[test]
fn leaves_an_ordinary_single_run_mission_exactly_as_it_was() {
    let rows = turns(&[("m1", "one off", "completed", None)]);
    assert_eq!((rows[0].mission_id.as_str(), rows[0].title.as_str(), rows[0].member_ids.len()), ("m1", "one off", 1));
}

#[test]
fn keeps_separate_conversations_separate_in_the_order_they_were_read() {
    // Native's list then stands the newest first (conversation_rows); here that is the same order.
    let rows = turns(&[("a1", "first", "completed", None), ("b1", "second", "completed", None), ("a2", "first", "completed", Some("a1"))]);
    assert_eq!(rows.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), ["first", "second"]);
}

fn missions_status_window(
    older_has_damage: bool,
    older_is_running: bool,
    filter: &str,
) -> rune::Runtime<State> {
    let mut s = State::default();
    s.missions.open = true;
    s.missions.filter = filter.into();
    for (id, parent, at) in [
        ("m_earlier", None, "2026-11-03T10:00:00Z"),
        ("m_newest", Some("m_earlier"), "2026-11-03T10:00:01Z"),
    ] {
        s.recent.push(ConversationRow {
            id: id.into(),
            title: "Set up the parser".into(),
            teammate: "Codex".into(),
            started_by: "native".into(),
            state: "completed".into(),
            updated: at.into(),
        });
        s.ledger_summaries.insert(id.into(), ledger::Summary {
            mission_id: id.into(), title: "Set up the parser".into(),
            parent: parent.map(str::to_owned), created_at: at.into(), updated: at.into(),
            integrity_issues: older_has_damage && id == "m_earlier",
            ..Default::default()
        });
    }
    if older_is_running {
        s.missions.running.insert("m_earlier".into());
    }
    let mut rt = rune::Runtime::new(s, crate::view, (1160.0, 760.0), rune::Env::new(true, 1.0));
    for _ in 0..8 { rt.frame(0.05); }
    rt
}

fn drawn_conversation_phase(rt: &rune::Runtime<State>) -> Option<rune::ElementInfo> {
    rt.elements().into_iter().find(|el|
        !el.exiting && el.key.as_deref() == Some("missions-tag-m_earlier")
    )
}

// case: surfaces an integrity issue from any turn, not just the last
#[test]
fn missions_older_damage_changes_the_actual_conversation_row() {
    let damaged = missions_status_window(true, false, "All");
    let amber = drawn_conversation_phase(&damaged).expect("one conversation with an earlier bad turn");
    assert_eq!(amber.text.as_deref(), Some("Completed · receipt incomplete"));
    assert_eq!(amber.text_color, Some(crate::look::controls::Look::new(true).c("amber")));
    let intact = missions_status_window(false, false, "All");
    assert_eq!(drawn_conversation_phase(&intact).unwrap().text.as_deref(), Some("Completed"));
}

// The already-carried grouping case has a separate pure test above. This drives the actual list tag and filter.
#[test]
fn missions_older_running_turn_keeps_the_actual_row_on_the_running_filter() {
    assert_eq!(drawn_conversation_phase(&missions_status_window(false, true, "Running"))
        .and_then(|el| el.text).as_deref(), Some("Running"));
    assert!(drawn_conversation_phase(&missions_status_window(false, true, "Completed")).is_none());
}
