//! The mode the message box's mode chip shows (App.tsx `mode`, handed to Composer.tsx): the one the next turn is given.

/// App.tsx's mode state, read for one conversation. `chosen` is a mode picked on this message box (the menu, a slash
/// command, Shift+Tab); `recorded` is the mode the open conversation's newest turn recorded (`followRouteOf`:
/// `run.restoredMission?.mode`, taken only where it runs on the route's agent); `teammate_mode` is the teammate's own
/// (`selectTeammate`: `setMode(own.mode)`), empty for a conversation with no teammate; with none of these, App.tsx's
/// first mode, Accept edits (`useState<MissionMode>('accept-edits')`). Then the chip never shows a mode its route cannot
/// run: the first one it can, else Accept edits (Composer.tsx `effectiveMode`: `modeRunsOn(mode, route.runtime,
/// platform) ? mode : modesFor(route.runtime, platform)[0] ?? 'accept-edits'`).
pub fn mode_shown(chosen: Option<&str>, recorded: Option<&str>, teammate_mode: &str, runtime: &str) -> String {
    let runs = |mode: &str| crate::mode_facts::runs_on(mode, runtime);
    let mode = chosen
        .filter(|mode| !mode.is_empty())
        .or_else(|| recorded.filter(|mode| runs(mode)))
        .or_else(|| Some(teammate_mode).filter(|mode| !mode.is_empty()))
        .unwrap_or(FIRST_MODE);
    if runs(mode) {
        return mode.to_owned();
    }
    crate::mode_facts::modes_for(runtime, None).first().copied().unwrap_or(FIRST_MODE).to_owned()
}

/// App.tsx: the mode a window starts on, before anything has set one ("Accept edits, not Ask", 2026-09-03).
pub const FIRST_MODE: &str = "accept-edits";

#[cfg(test)]
#[path = "the_mode_chip_is_the_conversations_tests.rs"]
mod the_mode_chip_is_the_conversations_tests;
