//! Electron `main/a-handoff-keeps-its-mode.test.ts` (M26): a handoff and a resume keep the mode they were asked for.
//! Rust reads a mode through `approval::Mode::parse`; the request paths refuse a mode it does not know (the door's
//! "mode is one of: ...") rather than falling back to Ask, so only the first case carries (see the ledger note).

use crate::approval::Mode;

#[test]
fn each_mode_locust_knows_travels_as_itself() {
    for mode in ["ask", "accept-edits", "approve-each", "auto", "plan"] {
        assert_eq!(Mode::parse(mode).map(Mode::as_str), Some(mode), "{mode}");
    }
}
