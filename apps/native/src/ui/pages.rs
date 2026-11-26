//! The pages the window can show beside a conversation, and the one way into any of them (live hour 1, fault 40).
//!
//! Each page is its own flag on the state and the view picks the first one set, in a fixed order (`page_shown`). So an
//! entry that sets its own flag and leaves another page's set shows nothing until that page is closed: Board pressed
//! over Routines, Team pressed over Settings. Electron has one current page; here every entry closes every page first.

use super::*;

/// What the main pane shows, in the order the view tries them (main.rs `page_shown`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    Changes,
    Routines,
    Boot,
    Board,
    Home,
    Settings,
    Team,
    Benchmarks,
    Missions,
    Compare,
    Rooms,
    Conversation,
}

/// Close every page, as choosing a teammate does: the main pane falls back to the conversation.
pub(crate) fn close_all(s: &mut State) {
    crate::room_screen::close(s);
    crate::missions_screen::close(s);
    s.compare.active_id = None;
    s.home.home_open = false;
    s.home.board_open = false;
    s.home.boot_open = false;
    s.settings_panel.open = false;
    s.people_panel.open = false;
    s.routines.open = false;
    s.benchmarks_open = false;
    s.changes.open = false;
}

/// A sidebar entry's press: every page closes, then the entry opens its own.
pub(crate) fn opening(open: fn(&mut State)) -> impl Fn(&mut State) + 'static {
    move |s: &mut State| {
        close_all(s);
        open(s);
    }
}

#[cfg(test)]
#[path = "pages_tests.rs"]
mod pages_tests;
