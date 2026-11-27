//! What the mode chip shows (chip-1 row 2, `composer_mode::mode_shown`): the same Claude Code conversation with no
//! teammate, run in Auto, read `Ask` here and `Auto` in Electron. Electron's answers are App.tsx's: `followRouteOf`
//! (the conversation's recorded mode), `selectTeammate` (the teammate's own), its first mode, and the `mode={...}` it
//! hands Composer. The chip drawn is in src/ui/the_mode_chip_says_the_mode_tests.rs.
use super::*;

// App.tsx `followRouteOf`: "The conversation's last turn recorded its mode; that is what the next turn should start
// on" (Colin, 2026-09-16: "ill have stocks convo on auto ... the next time it will be accept edits"). A conversation
// with no teammate has no mode of its own, so its record is the answer.
#[test]
fn a_conversation_with_no_teammate_shows_the_mode_its_newest_turn_ran_in() {
    assert_eq!(mode_shown(None, Some("auto"), "", "claude"), "auto");
    assert_eq!(mode_shown(None, Some("plan"), "", "claude"), "plan");
}

// App.tsx `selectTeammate`: "the composer shows the runtime, model and mode they last ran on" (`setMode(own.mode)`),
// for a teammate with no conversation open; one open on a turn that recorded its mode follows the record
// (`run.restoredMission?.mode ?? own.mode`).
#[test]
fn a_teammate_shows_its_own_mode_until_a_conversation_records_one() {
    assert_eq!(mode_shown(None, None, "approve-each", "claude"), "approve-each");
    assert_eq!(mode_shown(None, None, "ask", "codex"), "ask");
    assert_eq!(mode_shown(None, Some("auto"), "ask", "claude"), "auto");
}

// App.tsx `useState<MissionMode>('accept-edits')`: "Accept edits, not Ask. A person who opens a workroom and says
// 'add a discount function' means it" -- a new conversation that nothing has given a mode starts on Accept edits.
#[test]
fn a_new_conversation_that_nothing_gave_a_mode_starts_on_accept_edits() {
    assert_eq!(mode_shown(None, None, "", "claude"), "accept-edits");
    assert_eq!(FIRST_MODE, "accept-edits");
}

// Composer.tsx `onModeChange={setMode}`: a mode picked on the box is the window's mode until something sets another.
#[test]
fn a_mode_picked_on_the_box_is_the_one_shown() {
    assert_eq!(mode_shown(Some("ask"), Some("auto"), "approve-each", "claude"), "ask");
}

// App.tsx `followRouteOf`: `if (recorded !== undefined && modeRunsOn(recorded, run.data.runtime, ...))` -- a recorded
// mode the agent cannot run is not taken; and the composer is handed `modesFor(runtime)[0]` for a mode its route cannot
// run ("showing it would promise an edit the host is about to refuse"). Cursor offers no Approve each (status.ts).
#[test]
fn a_mode_the_agent_cannot_run_is_never_shown() {
    assert_eq!(mode_shown(None, Some("approve-each"), "", "cursor"), "accept-edits");
    assert_eq!(mode_shown(None, None, "approve-each", "cursor"), "ask");
    assert_eq!(mode_shown(Some("approve-each"), None, "", "cursor"), "ask");
}
