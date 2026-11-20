//! homeTeamState.test.ts, case for case: the state on a team card (0.606), and what a waiting card says (0.610).
use super::*;
use chrono::{DateTime, Utc};

fn view(status: &'static str, label: &str, tone: &'static str) -> StatusView {
    StatusView { status, label: label.into(), tone, presence: "none" }
}
fn now() -> DateTime<Utc> {
    "2026-10-05T12:00:00.000Z".parse().unwrap()
}
fn state(word: &str, tone: &'static str, spoken: &str) -> Option<TeamCardState> {
    Some(TeamCardState { word: word.into(), tone, spoken: spoken.into() })
}

#[test]
fn says_the_live_word_while_the_teammate_works() {
    let working = view("working", "Code & Migrations · working", "live");
    assert_eq!(team_card_state(Some(&working), FaceActivity::Responding), state("replying", "live", "replying now"));
    assert_eq!(team_card_state(Some(&working), FaceActivity::Thinking), state("thinking", "live", "thinking now"));
}

#[test]
fn says_waiting_on_you_in_amber_when_a_card_asks() {
    let asking = view("approval-needed", "Money · waiting on you", "amber");
    assert_eq!(team_card_state(Some(&asking), FaceActivity::Waiting), state("waiting on you", "amber", "waiting on you"));
}

#[test]
fn says_what_blocks_the_teammate_in_red_and_nothing_while_the_agent_is_still_being_checked() {
    let signed_out = view("blocked", "Sign-in needed", "red");
    assert_eq!(team_card_state(Some(&signed_out), FaceActivity::Blocked), state("sign-in needed", "red", "sign-in needed"));
    let missing = team_card_state(Some(&view("blocked", "AI agent not installed", "red")), FaceActivity::Blocked).unwrap();
    assert_eq!((missing.word.as_str(), missing.tone), ("AI agent not installed", "red"));
    // A moment, not a state: every card went amber for the seconds after launch while the strip said "checking".
    assert_eq!(team_card_state(Some(&view("blocked", "AI agent not answering — checking again", "amber")), FaceActivity::Blocked), None);
}

#[test]
fn says_nothing_beside_the_name_of_an_idle_teammate_0_609_the_card_is_calm_when_they_are() {
    assert_eq!(team_card_state(Some(&view("idle", "Docs & QA · idle", "muted")), FaceActivity::Idle), None);
}

#[test]
fn keeps_what_they_last_did_for_the_hover_in_whole_words() {
    assert_eq!(last_worked_phrase(Some("2026-10-05T10:00:00.000Z"), now()), "last worked 2 hours ago");
    assert_eq!(last_worked_phrase(Some("2026-10-01T10:00:00.000Z"), now()), "last worked 4 days ago");
    assert_eq!(last_worked_phrase(Some("2026-10-05T11:59:30.000Z"), now()), "last worked just now");
    assert_eq!(last_worked_phrase(None, now()), "no work yet");
}

#[test]
fn says_nothing_for_a_teammate_the_app_has_no_view_of() {
    assert_eq!(team_card_state(None, FaceActivity::Working), None);
}

fn approval(what: &str, asking: bool, command: bool) -> Waiting<'_> {
    Waiting::Approval { what, asking, command }
}

#[test]
fn says_the_command_it_wants_to_run() {
    assert_eq!(waiting_line(approval("pnpm test", false, true)).as_deref(), Some("Wants to run pnpm test"));
}

#[test]
fn says_the_question_it_asks_as_asked() {
    assert_eq!(waiting_line(approval("Which folder?", true, false)).as_deref(), Some("Which folder?"));
    assert_eq!(waiting_line(Waiting::Decision { what: "Ship it?" }).as_deref(), Some("Ship it?"));
}

#[test]
fn says_an_approval_that_is_not_a_command_plainly() {
    assert_eq!(waiting_line(approval("edit FirstLaunch.tsx", false, false)).as_deref(), Some("Wants your approval: edit FirstLaunch.tsx"));
}

#[test]
fn says_nothing_for_memory_suggestions_a_scout_card_or_a_routine_waiting_for_keep() {
    assert_eq!(waiting_line(Waiting::Memory), None);
    assert_eq!(waiting_line(Waiting::Scout), None);
    assert_eq!(waiting_line(Waiting::Routine), None);
}
