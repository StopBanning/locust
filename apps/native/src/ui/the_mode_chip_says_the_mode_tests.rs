//! THE MODE CHIP SAYS THE MODE ELECTRON'S DOES (chip-1 row 2): the same Claude Code conversation with no teammate, run in
//! Auto, read `Ask` here and `Auto` in Electron, whose App.tsx `followRouteOf` takes the conversation's recorded mode.
//! Drawn headless through `thread_view`; the rule's own cases are in core/src/the_mode_chip_is_the_conversations_tests.rs.
use super::*;
use locust_core::engine::Teammate;
use rune::{Env, Runtime};

fn teammate(id: &str, runs_in: &str) -> Teammate {
    Teammate {
        id: id.into(),
        name: if id == LEDGER_TARGET { "Claude Code".into() } else { "Wren".into() },
        role: if id == LEDGER_TARGET { "Conversation".into() } else { "Code".into() },
        runtime: Some("claude".into()),
        model: Some("opus".into()),
        account: None,
        busy: false,
        runs_in: runs_in.into(),
        folder: None,
    }
}

/// A conversation open on `header`, whose turns recorded `modes` (oldest first; None for a turn that recorded none).
fn open_on(header: Teammate, modes: &[Option<&str>]) -> State {
    let mut s = State::default();
    let mut snapshot = ledger::Snapshot::default();
    snapshot.turn_metas = modes
        .iter()
        .enumerate()
        .map(|(i, mode)| ledger::Metadata { mission_id: format!("mission_{i}"), runtime: "claude".into(), model: "opus".into(), mode: mode.map(str::to_owned), ..Default::default() })
        .collect();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header,
        conversation: Conversation { conversation_id: Some("mission_0".into()), turns: Vec::new() },
        items: Vec::new(),
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot,
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    s
}

/// The word the mode chip reads, and its spoken label.
fn mode_chip(s: State) -> (String, String) {
    fn view(s: &State, _cx: &mut Cx<State>) -> El {
        crate::thread_view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, view, (1000.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let ([x, y, w, h], _) = rt.presented("mode-chip").expect("the mode chip is drawn");
    let elements = rt.elements();
    let label = elements.iter().find(|e| e.key.as_deref() == Some("mode-chip")).and_then(|e| e.label.clone()).unwrap_or_default();
    let words = elements
        .into_iter()
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect::<Vec<_>>()
        .join(" ");
    (words, label)
}

// (a) App.tsx `followRouteOf`: "The conversation's last turn recorded its mode; that is what the next turn should start
// on." The fault's conversation: no teammate, run in Auto, so Electron's chip reads `Auto` (status.ts `modeChipLabel`).
#[test]
fn a_conversation_with_no_teammate_that_ran_in_auto_reads_auto() {
    let (words, label) = mode_chip(open_on(teammate(LEDGER_TARGET, ""), &[Some("accept-edits"), Some("auto")]));
    assert_eq!(words, "Auto");
    assert_eq!(label, "Mode: Auto");
}

// (b) App.tsx `selectTeammate`: `setMode(own.mode)` -- a teammate whose own mode is set shows it while nothing has
// recorded another (here a turn recorded before modes were, `mode` absent).
#[test]
fn a_teammate_whose_mode_is_set_reads_its_own_mode() {
    let mut s = State::default();
    s.teammates.push(teammate("tm_1", "approve-each"));
    s.selected = 0;
    s.conversations.insert("tm_1".into(), Conversation::default());
    assert_eq!(mode_chip(s).0, "Approve");
    assert_eq!(mode_chip(open_on(teammate("tm_1", "approve-each"), &[None])).0, "Approve");
}

// (c) App.tsx `useState<MissionMode>('accept-edits')`: a new conversation with no teammate that nothing has given a
// mode reads `Edit` (status.ts `modeChipLabel('accept-edits')`), not Ask.
#[test]
fn a_new_conversation_that_nothing_gave_a_mode_reads_edit() {
    assert_eq!(mode_chip(open_on(teammate(LEDGER_TARGET, ""), &[])).0, "Edit");
}

// Composer.tsx `onModeChange={setMode}`: a mode picked on the box is the one shown, over the record.
#[test]
fn a_mode_picked_on_the_box_reads_over_the_record() {
    let mut s = open_on(teammate(LEDGER_TARGET, ""), &[Some("auto")]);
    s.composer_mode = Some("plan".into());
    assert_eq!(mode_chip(s).0, "Plan");
}
