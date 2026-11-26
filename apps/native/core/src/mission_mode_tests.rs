//! Electron `main/a-handoff-keeps-its-mode.test.ts` (M26): a handoff and a resume keep the mode they were asked for.
//! The first case is here, on `approval::Mode::parse`; the other two (a missing or malformed mode is Ask, read the same
//! way by start, handoff and resume through `native::requested_mode`) are in `native/a_handoff_keeps_its_mode_tests.rs`.

use crate::approval::Mode;

// case: travels as itself when Locust knows it
#[test]
fn each_mode_locust_knows_travels_as_itself() {
    for mode in ["ask", "accept-edits", "approve-each", "auto", "plan"] {
        assert_eq!(Mode::parse(mode).map(Mode::as_str), Some(mode), "{mode}");
    }
}
